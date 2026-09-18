use std::sync::Arc;

use axum::Json;
use axum::Router;
use axum::body::Bytes;
use axum::extract::ws::{Message, WebSocket};
use axum::extract::{Path, State, WebSocketUpgrade};
use axum::http::{HeaderMap, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use futures_util::{SinkExt, StreamExt};
use tracing::{info, warn};

use super::discovery::{KubeVirtVmEntry, SidecarEndpoint};
use super::k8s_auth::KubernetesUser;
use super::k8s_authz;
use super::state::AggregatorState;

const DEFAULT_PAGE_SIZE: usize = 50;

#[derive(serde::Deserialize, Default)]
struct PaginationParams {
    #[serde(rename = "$skip", default)]
    skip: Option<usize>,
    #[serde(rename = "$top", default)]
    top: Option<usize>,
}

pub fn aggregator_router(state: Arc<AggregatorState>) -> Router {
    Router::new()
        .route("/redfish", get(get_redfish_root))
        .route("/redfish/v1", get(get_service_root))
        .route("/redfish/v1/", get(get_service_root))
        .route("/redfish/v1/$metadata", get(get_metadata))
        .route("/redfish/v1/odata", get(get_odata_service_document))
        .route("/redfish/v1/Systems", get(get_aggregated_systems))
        .route("/redfish/v1/Chassis", get(get_aggregated_chassis))
        .route(
            "/redfish/v1/Systems/{system_id}",
            get(proxy_system_get)
                .post(proxy_system_mutate)
                .patch(proxy_system_mutate)
                .delete(proxy_system_mutate),
        )
        .route(
            "/redfish/v1/Systems/{system_id}/SerialConsole",
            get(proxy_serial_console),
        )
        .route(
            "/redfish/v1/Systems/{system_id}/{*rest}",
            get(proxy_system_sub_get)
                .post(proxy_system_sub_mutate)
                .patch(proxy_system_sub_mutate)
                .delete(proxy_system_sub_mutate),
        )
        .route(
            "/redfish/v1/Chassis/{system_id}",
            get(proxy_chassis_get)
                .post(proxy_chassis_mutate)
                .patch(proxy_chassis_mutate)
                .delete(proxy_chassis_mutate),
        )
        .route(
            "/redfish/v1/Chassis/{system_id}/{*rest}",
            get(proxy_chassis_sub_get)
                .post(proxy_chassis_sub_mutate)
                .patch(proxy_chassis_sub_mutate)
                .delete(proxy_chassis_sub_mutate),
        )
        .route("/api/v1/revocation", post(handle_keylime_revocation))
        .route("/api/v1/config/reload", post(handle_config_reload))
        .with_state(state)
}

async fn get_redfish_root() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "v1": "/redfish/v1/"
    }))
}

async fn get_service_root(State(state): State<Arc<AggregatorState>>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "@odata.id": "/redfish/v1",
        "@odata.type": "#ServiceRoot.v1_17_0.ServiceRoot",
        "Id": "RootService",
        "Name": "vbmc-rs Aggregator Redfish Service",
        "Description": "vbmc-rs Redfish Aggregator Service Root",
        "RedfishVersion": "1.21.0",
        "UUID": state.instance_uuid,
        "Systems": { "@odata.id": "/redfish/v1/Systems" },
        "Chassis": { "@odata.id": "/redfish/v1/Chassis" },
        "Vendor": "vbmc-rs",
        "Product": "Virtual BMC Aggregator",
        "Links": {
            "Sessions": { "@odata.id": "/redfish/v1/SessionService/Sessions" }
        }
    }))
}

static METADATA_XML: &str = include_str!("../../data/metadata.xml");

async fn get_metadata() -> Response {
    (
        [(axum::http::header::CONTENT_TYPE, "application/xml")],
        METADATA_XML,
    )
        .into_response()
}

async fn get_odata_service_document() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "@odata.context": "/redfish/v1/$metadata",
        "value": [
            { "name": "Systems", "kind": "Singleton", "url": "/redfish/v1/Systems" },
            { "name": "Chassis", "kind": "Singleton", "url": "/redfish/v1/Chassis" },
        ]
    }))
}

async fn check_endpoint_access(
    state: &AggregatorState,
    user: &KubernetesUser,
    endpoint: &super::discovery::SidecarEndpoint,
) -> bool {
    if let Some(client) = &state.kube_client
        && !endpoint.namespace.is_empty()
    {
        return k8s_authz::can_access_vm(
            client,
            user,
            &endpoint.namespace,
            &endpoint.vm_name,
            &state.authz_cache,
        )
        .await;
    }
    true
}

fn strip_auth_headers(headers: &HeaderMap) -> HeaderMap {
    let mut proxy_headers = headers.clone();
    proxy_headers.remove("authorization");
    proxy_headers.remove("x-auth-token");
    proxy_headers
}

fn vm_entry_to_endpoint(entry: &KubeVirtVmEntry) -> SidecarEndpoint {
    SidecarEndpoint {
        system_id: entry.system_id.clone(),
        namespace: entry.namespace.clone(),
        vm_name: entry.vm_name.clone(),
        url: String::new(),
    }
}

fn parse_memory_mib(s: &str) -> u64 {
    let s = s.trim();
    if let Some(n) = s.strip_suffix("Gi") {
        n.parse::<u64>().unwrap_or(0) * 1024
    } else if let Some(n) = s.strip_suffix("Mi") {
        n.parse::<u64>().unwrap_or(0)
    } else if let Some(n) = s.strip_suffix("Ki") {
        n.parse::<u64>().unwrap_or(0) / 1024
    } else if let Some(n) = s.strip_suffix('G') {
        n.parse::<u64>().unwrap_or(0) * 953
    } else if let Some(n) = s.strip_suffix('M') {
        n.parse::<u64>().unwrap_or(0)
    } else {
        s.parse::<u64>().unwrap_or(0) / (1024 * 1024)
    }
}

async fn hybrid_system_get(
    state: &AggregatorState,
    entry: &KubeVirtVmEntry,
    system_id: &str,
) -> Result<Response, StatusCode> {
    let client = state
        .kube_client
        .as_ref()
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    let url = format!(
        "/apis/kubevirt.io/v1/namespaces/{}/virtualmachines/{}",
        entry.namespace, entry.vm_name
    );
    let req = http::Request::get(&url)
        .body(vec![])
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let vm: serde_json::Value = client.request(req).await.map_err(|e| {
        warn!("Failed to get VM from K8s API: {e}");
        StatusCode::BAD_GATEWAY
    })?;

    let domain = vm.pointer("/spec/template/spec/domain");

    let cpu_cores = domain
        .and_then(|d| d.pointer("/cpu/cores"))
        .and_then(|v| v.as_u64())
        .unwrap_or(1);
    let cpu_sockets = domain
        .and_then(|d| d.pointer("/cpu/sockets"))
        .and_then(|v| v.as_u64())
        .unwrap_or(1);
    let cpu_threads = domain
        .and_then(|d| d.pointer("/cpu/threads"))
        .and_then(|v| v.as_u64())
        .unwrap_or(1);
    let total_cpus = cpu_cores * cpu_sockets * cpu_threads;

    let memory_mib = domain
        .and_then(|d| d.pointer("/memory/guest"))
        .and_then(|v| v.as_str())
        .map(parse_memory_mib)
        .unwrap_or(0);

    let nics: Vec<serde_json::Value> = domain
        .and_then(|d| d.pointer("/devices/interfaces"))
        .and_then(|v| v.as_array())
        .map(|ifaces| {
            ifaces
                .iter()
                .enumerate()
                .map(|(i, iface)| {
                    let id = iface
                        .get("name")
                        .and_then(|n| n.as_str())
                        .unwrap_or(&format!("nic-{i}"))
                        .to_string();
                    let mut nic = serde_json::json!({
                        "@odata.id": format!("/redfish/v1/Systems/{system_id}/EthernetInterfaces/{id}"),
                        "Id": id,
                        "Name": id,
                    });
                    if let Some(mac) = iface.get("macAddress").and_then(|m| m.as_str()) {
                        nic["MACAddress"] = serde_json::Value::String(mac.to_string());
                    }
                    nic
                })
                .collect()
        })
        .unwrap_or_default();

    let secure_boot = domain
        .and_then(|d| d.pointer("/firmware/bootloader/efi/secureBoot"))
        .and_then(|v| v.as_bool());

    let mut body = serde_json::json!({
        "@odata.id": format!("/redfish/v1/Systems/{system_id}"),
        "@odata.type": "#ComputerSystem.v1_19_0.ComputerSystem",
        "Id": system_id,
        "Name": entry.vm_name,
        "SystemType": "Virtual",
        "PowerState": "Off",
        "Status": { "State": "StandbyOffline", "Health": "OK" },
        "ProcessorSummary": {
            "Count": total_cpus,
            "Status": { "State": "StandbyOffline", "Health": "OK" }
        },
        "MemorySummary": {
            "TotalSystemMemoryGiB": memory_mib as f64 / 1024.0,
            "Status": { "State": "StandbyOffline", "Health": "OK" }
        },
        "EthernetInterfaces": {
            "@odata.id": format!("/redfish/v1/Systems/{system_id}/EthernetInterfaces"),
            "Members": nics,
            "Members@odata.count": nics.len()
        },
        "Actions": {
            "#ComputerSystem.Reset": {
                "target": format!("/redfish/v1/Systems/{system_id}/Actions/ComputerSystem.Reset"),
                "ResetType@Redfish.AllowableValues": ["On", "ForceOff", "GracefulShutdown", "GracefulRestart", "ForceRestart"]
            }
        }
    });

    if let Some(sb) = secure_boot {
        body["SecureBoot"] = serde_json::json!({
            "@odata.id": format!("/redfish/v1/Systems/{system_id}/SecureBoot"),
            "SecureBootEnable": sb
        });
    }

    Ok(Json(body).into_response())
}

async fn hybrid_reset(
    state: &AggregatorState,
    entry: &KubeVirtVmEntry,
    body: &Bytes,
) -> Result<Response, StatusCode> {
    let client = state
        .kube_client
        .as_ref()
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    let reset_type = serde_json::from_slice::<serde_json::Value>(body)
        .ok()
        .and_then(|v| {
            v.get("ResetType")
                .and_then(|t| t.as_str())
                .map(|s| s.to_string())
        })
        .unwrap_or_default();

    let (resource, action) = match reset_type.as_str() {
        "On" => ("virtualmachines", "start"),
        "ForceOff" | "GracefulShutdown" => ("virtualmachines", "stop"),
        "GracefulRestart" | "ForceRestart" => ("virtualmachines", "restart"),
        "Nmi" => ("virtualmachineinstances", "softreboot"),
        _ => {
            return Ok((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": format!("Unknown or unsupported ResetType: {reset_type}")
                })),
            )
                .into_response());
        }
    };

    let url = format!(
        "/apis/subresources.kubevirt.io/v1/namespaces/{}/{}/{}/{}",
        entry.namespace, resource, entry.vm_name, action
    );
    let req = http::Request::put(&url)
        .body(vec![])
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    client.request_text(req).await.map_err(|e| {
        warn!("KubeVirt subresource call failed (action={action}): {e}");
        StatusCode::BAD_GATEWAY
    })?;

    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn get_aggregated_systems(
    State(state): State<Arc<AggregatorState>>,
    user: KubernetesUser,
    axum::extract::Query(params): axum::extract::Query<PaginationParams>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let mut all_members = Vec::new();

    if let Some(vm_reg) = &state.vm_registry {
        // Hybrid mode: list all VMs from the VM registry (including stopped VMs).
        for entry in vm_reg.list() {
            let ep = vm_entry_to_endpoint(&entry);
            if !check_endpoint_access(&state, &user, &ep).await {
                continue;
            }
            all_members.push(serde_json::json!({
                "@odata.id": format!("/redfish/v1/Systems/{}", entry.system_id)
            }));
        }
    } else {
        // Sidecar-only mode: discover members by forwarding to each sidecar.
        let endpoints = state.registry.list();
        for endpoint in &endpoints {
            if !check_endpoint_access(&state, &user, endpoint).await {
                continue;
            }

            match state
                .proxy
                .forward(
                    endpoint,
                    Method::GET,
                    "/redfish/v1/Systems",
                    HeaderMap::new(),
                    None,
                )
                .await
            {
                Ok(resp) => {
                    let (parts, body) = resp.into_parts();
                    if parts.status.is_success()
                        && let Ok(body_bytes) = axum::body::to_bytes(body, 1024 * 1024).await
                        && let Ok(parsed) = serde_json::from_slice::<serde_json::Value>(&body_bytes)
                        && let Some(members) = parsed.get("Members").and_then(|m| m.as_array())
                    {
                        all_members.extend(members.iter().cloned());
                    }
                }
                Err(status) => {
                    warn!(
                        system_id = %endpoint.system_id,
                        status = %status,
                        "Failed to fetch Systems from sidecar"
                    );
                }
            }
        }
    }

    let total = all_members.len();
    let skip = params.skip.unwrap_or(0);
    let top = params.top.unwrap_or(DEFAULT_PAGE_SIZE);
    let page: Vec<_> = all_members.into_iter().skip(skip).take(top).collect();
    let next_link = if skip + top < total {
        Some(format!(
            "/redfish/v1/Systems?$skip={}&$top={top}",
            skip + top
        ))
    } else {
        None
    };

    let mut resp = serde_json::json!({
        "@odata.id": "/redfish/v1/Systems",
        "@odata.type": "#ComputerSystemCollection.ComputerSystemCollection",
        "Name": "Computer System Collection",
        "Members": page,
        "Members@odata.count": total,
    });
    if let Some(link) = next_link {
        resp["Members@odata.nextLink"] = serde_json::Value::String(link);
    }
    Ok(Json(resp))
}

async fn proxy_system_get(
    State(state): State<Arc<AggregatorState>>,
    user: KubernetesUser,
    Path(system_id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    // Running VM with sidecar: proxy for full hot telemetry.
    if let Some(endpoint) = state.registry.get(&system_id) {
        if !check_endpoint_access(&state, &user, &endpoint).await {
            return Err(StatusCode::FORBIDDEN);
        }
        let path = format!("/redfish/v1/Systems/{system_id}");
        return state
            .proxy
            .forward(
                &endpoint,
                Method::GET,
                &path,
                strip_auth_headers(&headers),
                None,
            )
            .await;
    }

    // Hybrid mode: stopped VM — synthesize cold inventory from VM spec.
    if let Some(vm_reg) = &state.vm_registry
        && let Some(vm_entry) = vm_reg.get(&system_id)
    {
        let ep = vm_entry_to_endpoint(&vm_entry);
        if !check_endpoint_access(&state, &user, &ep).await {
            return Err(StatusCode::FORBIDDEN);
        }
        return hybrid_system_get(&state, &vm_entry, &system_id).await;
    }

    Err(StatusCode::NOT_FOUND)
}

async fn proxy_system_mutate(
    State(state): State<Arc<AggregatorState>>,
    user: KubernetesUser,
    method: Method,
    Path(system_id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, StatusCode> {
    let endpoint = state
        .registry
        .get(&system_id)
        .ok_or(StatusCode::NOT_FOUND)?;
    if !check_endpoint_access(&state, &user, &endpoint).await {
        return Err(StatusCode::FORBIDDEN);
    }
    let path = format!("/redfish/v1/Systems/{system_id}");
    let body_opt = if body.is_empty() { None } else { Some(body) };
    state
        .proxy
        .forward(
            &endpoint,
            method,
            &path,
            strip_auth_headers(&headers),
            body_opt,
        )
        .await
}

async fn proxy_system_sub_get(
    State(state): State<Arc<AggregatorState>>,
    user: KubernetesUser,
    Path((system_id, rest)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    let endpoint = state
        .registry
        .get(&system_id)
        .ok_or(StatusCode::NOT_FOUND)?;
    if !check_endpoint_access(&state, &user, &endpoint).await {
        return Err(StatusCode::FORBIDDEN);
    }
    let path = format!("/redfish/v1/Systems/{system_id}/{rest}");
    state
        .proxy
        .forward(
            &endpoint,
            Method::GET,
            &path,
            strip_auth_headers(&headers),
            None,
        )
        .await
}

async fn proxy_system_sub_mutate(
    State(state): State<Arc<AggregatorState>>,
    user: KubernetesUser,
    method: Method,
    Path((system_id, rest)): Path<(String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, StatusCode> {
    // Hybrid mode: Reset action always goes to KubeVirt subresource API,
    // even for stopped VMs (which have no sidecar endpoint).
    if rest == "Actions/ComputerSystem.Reset"
        && let Some(vm_reg) = &state.vm_registry
        && let Some(vm_entry) = vm_reg.get(&system_id)
    {
        let ep = vm_entry_to_endpoint(&vm_entry);
        if !check_endpoint_access(&state, &user, &ep).await {
            return Err(StatusCode::FORBIDDEN);
        }
        return hybrid_reset(&state, &vm_entry, &body).await;
    }

    let endpoint = state
        .registry
        .get(&system_id)
        .ok_or(StatusCode::NOT_FOUND)?;
    if !check_endpoint_access(&state, &user, &endpoint).await {
        return Err(StatusCode::FORBIDDEN);
    }
    let path = format!("/redfish/v1/Systems/{system_id}/{rest}");
    let body_opt = if body.is_empty() { None } else { Some(body) };
    state
        .proxy
        .forward(
            &endpoint,
            method,
            &path,
            strip_auth_headers(&headers),
            body_opt,
        )
        .await
}

async fn get_aggregated_chassis(
    State(state): State<Arc<AggregatorState>>,
    user: KubernetesUser,
) -> Json<serde_json::Value> {
    let mut members = Vec::new();

    if state.vm_registry.is_some() {
        // Hybrid mode: list chassis from config.
        let chassis_list = state.chassis_config.read().unwrap().clone();
        for chassis in chassis_list {
            // Access check: does the user have access to any VM in this chassis?
            let fake_ep = SidecarEndpoint {
                system_id: chassis.name.clone(),
                namespace: chassis.namespace.clone(),
                vm_name: String::new(),
                url: String::new(),
            };
            if check_endpoint_access(&state, &user, &fake_ep).await {
                members.push(serde_json::json!({
                    "@odata.id": format!("/redfish/v1/Chassis/{}", chassis.name)
                }));
            }
        }
    } else {
        // Sidecar-only mode: derive chassis from registered sidecar endpoints.
        let endpoints = state.registry.list();
        let mut seen = std::collections::BTreeSet::new();

        for ep in &endpoints {
            let chassis_id = if ep.namespace.is_empty() {
                ep.system_id.clone()
            } else {
                ep.namespace.clone()
            };

            if seen.contains(&chassis_id) {
                continue;
            }
            if !check_endpoint_access(&state, &user, ep).await {
                continue;
            }
            seen.insert(chassis_id.clone());
            members.push(serde_json::json!({
                "@odata.id": format!("/redfish/v1/Chassis/{chassis_id}")
            }));
        }
    }

    Json(serde_json::json!({
        "@odata.id": "/redfish/v1/Chassis",
        "@odata.type": "#ChassisCollection.ChassisCollection",
        "Name": "Chassis Collection",
        "Members": members,
        "Members@odata.count": members.len()
    }))
}

async fn proxy_chassis_get(
    State(state): State<Arc<AggregatorState>>,
    user: KubernetesUser,
    Path(chassis_id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    // Hybrid mode: serve chassis from the VM registry filtered by chassis_name.
    if let Some(vm_reg) = &state.vm_registry {
        let chassis_cfg = state
            .chassis_config
            .read()
            .unwrap()
            .iter()
            .find(|c| c.name == chassis_id)
            .cloned();

        if let Some(chassis) = chassis_cfg {
            let fake_ep = SidecarEndpoint {
                system_id: chassis.name.clone(),
                namespace: chassis.namespace.clone(),
                vm_name: String::new(),
                url: String::new(),
            };
            if !check_endpoint_access(&state, &user, &fake_ep).await {
                return Err(StatusCode::FORBIDDEN);
            }

            let mut vm_members = Vec::new();
            for entry in vm_reg.list() {
                if entry.chassis_name == chassis_id {
                    let ep = vm_entry_to_endpoint(&entry);
                    if check_endpoint_access(&state, &user, &ep).await {
                        vm_members.push(serde_json::json!({
                            "@odata.id": format!("/redfish/v1/Systems/{}", entry.system_id)
                        }));
                    }
                }
            }

            let body = serde_json::json!({
                "@odata.id": format!("/redfish/v1/Chassis/{chassis_id}"),
                "@odata.type": "#Chassis.v1_22_0.Chassis",
                "Id": chassis_id,
                "Name": chassis.name,
                "Description": format!("Kubernetes namespace: {}", chassis.namespace),
                "ChassisType": "Virtual",
                "Status": { "State": "Enabled", "Health": "OK" },
                "Links": { "ComputerSystems": vm_members }
            });
            return Ok(Json(body).into_response());
        }

        return Err(StatusCode::NOT_FOUND);
    }

    // Sidecar-only mode: synthesize chassis from endpoints sharing a namespace.
    let endpoints = state.registry.list();
    let ns_members: Vec<_> = {
        let mut v = Vec::new();
        for ep in &endpoints {
            if ep.namespace == chassis_id && check_endpoint_access(&state, &user, ep).await {
                v.push(serde_json::json!({
                    "@odata.id": format!("/redfish/v1/Systems/{}", ep.system_id)
                }));
            }
        }
        v
    };

    if !ns_members.is_empty() {
        let body = serde_json::json!({
            "@odata.id": format!("/redfish/v1/Chassis/{chassis_id}"),
            "@odata.type": "#Chassis.v1_22_0.Chassis",
            "Id": chassis_id,
            "Name": chassis_id,
            "ChassisType": "Virtual",
            "Status": { "State": "Enabled", "Health": "OK" },
            "Links": { "ComputerSystems": ns_members }
        });
        return Ok(Json(body).into_response());
    }

    // Fall back to proxying to the sidecar (chassis_id treated as system_id).
    let endpoint = state
        .registry
        .get(&chassis_id)
        .ok_or(StatusCode::NOT_FOUND)?;
    if !check_endpoint_access(&state, &user, &endpoint).await {
        return Err(StatusCode::FORBIDDEN);
    }
    let path = format!("/redfish/v1/Chassis/{chassis_id}");
    state
        .proxy
        .forward(
            &endpoint,
            Method::GET,
            &path,
            strip_auth_headers(&headers),
            None,
        )
        .await
}

async fn proxy_chassis_mutate(
    State(state): State<Arc<AggregatorState>>,
    user: KubernetesUser,
    method: Method,
    Path(system_id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, StatusCode> {
    let endpoint = state
        .registry
        .get(&system_id)
        .ok_or(StatusCode::NOT_FOUND)?;
    if !check_endpoint_access(&state, &user, &endpoint).await {
        return Err(StatusCode::FORBIDDEN);
    }
    let path = format!("/redfish/v1/Chassis/{system_id}");
    let body_opt = if body.is_empty() { None } else { Some(body) };
    state
        .proxy
        .forward(
            &endpoint,
            method,
            &path,
            strip_auth_headers(&headers),
            body_opt,
        )
        .await
}

async fn proxy_chassis_sub_get(
    State(state): State<Arc<AggregatorState>>,
    user: KubernetesUser,
    Path((system_id, rest)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    let endpoint = state
        .registry
        .get(&system_id)
        .ok_or(StatusCode::NOT_FOUND)?;
    if !check_endpoint_access(&state, &user, &endpoint).await {
        return Err(StatusCode::FORBIDDEN);
    }
    let path = format!("/redfish/v1/Chassis/{system_id}/{rest}");
    state
        .proxy
        .forward(
            &endpoint,
            Method::GET,
            &path,
            strip_auth_headers(&headers),
            None,
        )
        .await
}

async fn proxy_chassis_sub_mutate(
    State(state): State<Arc<AggregatorState>>,
    user: KubernetesUser,
    method: Method,
    Path((system_id, rest)): Path<(String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, StatusCode> {
    let endpoint = state
        .registry
        .get(&system_id)
        .ok_or(StatusCode::NOT_FOUND)?;
    if !check_endpoint_access(&state, &user, &endpoint).await {
        return Err(StatusCode::FORBIDDEN);
    }
    let path = format!("/redfish/v1/Chassis/{system_id}/{rest}");
    let body_opt = if body.is_empty() { None } else { Some(body) };
    state
        .proxy
        .forward(
            &endpoint,
            method,
            &path,
            strip_auth_headers(&headers),
            body_opt,
        )
        .await
}

#[derive(serde::Deserialize)]
struct RevocationPayload {
    agent_id: String,
}

async fn handle_keylime_revocation(
    State(state): State<Arc<AggregatorState>>,
    Json(payload): Json<RevocationPayload>,
) -> StatusCode {
    info!(
        agent_id = %payload.agent_id,
        "Received Keylime revocation — triggering ForceOff"
    );

    let endpoint = match state.registry.get(&payload.agent_id) {
        Some(ep) => ep,
        None => {
            warn!(
                agent_id = %payload.agent_id,
                "Revocation for unknown system"
            );
            return StatusCode::NOT_FOUND;
        }
    };

    let reset_body = serde_json::json!({"ResetType": "ForceOff"});
    let path = format!(
        "/redfish/v1/Systems/{}/Actions/ComputerSystem.Reset",
        payload.agent_id
    );

    match state
        .proxy
        .forward(
            &endpoint,
            Method::POST,
            &path,
            HeaderMap::new(),
            Some(Bytes::from(
                serde_json::to_vec(&reset_body).expect("json serialize"),
            )),
        )
        .await
    {
        Ok(resp) => {
            let status = resp.status();
            if status.is_success() {
                info!(agent_id = %payload.agent_id, "ForceOff triggered successfully");
                StatusCode::OK
            } else {
                warn!(
                    agent_id = %payload.agent_id,
                    status = %status,
                    "ForceOff request returned non-success"
                );
                StatusCode::BAD_GATEWAY
            }
        }
        Err(status) => {
            warn!(
                agent_id = %payload.agent_id,
                status = %status,
                "Failed to proxy ForceOff to sidecar"
            );
            status
        }
    }
}

async fn proxy_serial_console(
    State(state): State<Arc<AggregatorState>>,
    user: KubernetesUser,
    Path(system_id): Path<String>,
    ws: WebSocketUpgrade,
) -> Result<Response, StatusCode> {
    let endpoint = state
        .registry
        .get(&system_id)
        .ok_or(StatusCode::NOT_FOUND)?;
    if !check_endpoint_access(&state, &user, &endpoint).await {
        return Err(StatusCode::FORBIDDEN);
    }

    let upstream_url = format!(
        "{}/redfish/v1/Systems/{system_id}/SerialConsole",
        endpoint
            .url
            .replace("http://", "ws://")
            .replace("https://", "wss://")
    );

    info!(
        system_id = %system_id,
        upstream = %upstream_url,
        "Proxying WebSocket serial console"
    );

    Ok(ws
        .on_upgrade(move |client_socket| async move {
            let upstream = match tokio_tungstenite::connect_async(&upstream_url).await {
                Ok((ws, _)) => ws,
                Err(e) => {
                    warn!(
                        system_id = %system_id,
                        error = %e,
                        "Failed to connect to sidecar serial console"
                    );
                    return;
                }
            };

            proxy_websocket(client_socket, upstream, system_id).await;
        })
        .into_response())
}

async fn proxy_websocket(
    client: WebSocket,
    upstream: tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    system_id: String,
) {
    let (mut client_tx, mut client_rx) = client.split();
    let (mut upstream_tx, mut upstream_rx) = upstream.split();

    let sid = system_id.clone();
    let upstream_to_client = tokio::spawn(async move {
        while let Some(Ok(msg)) = upstream_rx.next().await {
            let axum_msg = match msg {
                tokio_tungstenite::tungstenite::Message::Binary(data) => Message::Binary(data),
                tokio_tungstenite::tungstenite::Message::Text(text) => {
                    Message::Text(text.to_string().into())
                }
                tokio_tungstenite::tungstenite::Message::Close(_) => break,
                _ => continue,
            };
            if client_tx.send(axum_msg).await.is_err() {
                break;
            }
        }
    });

    let sid2 = sid.clone();
    let client_to_upstream = tokio::spawn(async move {
        while let Some(Ok(msg)) = client_rx.next().await {
            let tung_msg = match msg {
                Message::Binary(data) => tokio_tungstenite::tungstenite::Message::Binary(data),
                Message::Text(text) => {
                    tokio_tungstenite::tungstenite::Message::Text(text.to_string().into())
                }
                Message::Close(_) => break,
                _ => continue,
            };
            if upstream_tx.send(tung_msg).await.is_err() {
                break;
            }
        }
        info!(system_id = %sid2, "Serial console proxy closed");
    });

    tokio::select! {
        _ = upstream_to_client => {}
        _ = client_to_upstream => {}
    }
}

/// Reload the chassis list from disk and reconcile running watchers.
/// Called on SIGHUP or `POST /api/v1/config/reload`.
pub async fn reload_chassis_config(state: &Arc<AggregatorState>) {
    use super::discovery;
    use tokio_util::sync::CancellationToken;

    let new_config = match super::config::AggregatorConfig::load(&state.config_path) {
        Ok(c) => c,
        Err(e) => {
            warn!("Config reload failed — keeping current config: {e}");
            return;
        }
    };

    let new_chassis = new_config.effective_chassis();
    let new_names: std::collections::HashSet<String> =
        new_chassis.iter().map(|c| c.name.clone()).collect();

    let mut handles = state.watcher_handles.lock().unwrap();
    let old_names: std::collections::HashSet<String> = handles.keys().cloned().collect();

    // Cancel watchers for removed chassis.
    for removed in old_names.difference(&new_names) {
        if let Some(token) = handles.remove(removed) {
            info!(chassis = %removed, "Hot-reload: stopping watcher for removed chassis");
            token.cancel();
        }
    }

    // Start watchers for added chassis (only in kubevirt-hybrid mode).
    if state.config.discovery.mode == "kubevirt-hybrid"
        && let Some(vm_reg) = &state.vm_registry
    {
        {
            let port = state.config.sidecar.port;
            let tls = state.config.sidecar.tls_enabled();
            let bmc_net = state.config.discovery.bmc_network.clone();
            let selector = state.config.discovery.label_selector.clone();

            for chassis in new_chassis.iter().filter(|c| !old_names.contains(&c.name)) {
                info!(chassis = %chassis.name, "Hot-reload: starting watcher for new chassis");
                let chassis_token = CancellationToken::new();
                handles.insert(chassis.name.clone(), chassis_token.clone());

                let reg = state.registry.clone();
                let ns = Some(chassis.namespace.clone());
                let bmc_net_c = bmc_net.clone();
                let selector_c = selector.clone();
                let token_pods = chassis_token.clone();
                tokio::spawn(async move {
                    discovery::start_kubernetes_watcher(
                        reg, ns, selector_c, port, tls, bmc_net_c, token_pods,
                    )
                    .await;
                });

                let vm_reg_c = vm_reg.clone();
                let ns_vms = Some(chassis.namespace.clone());
                let chassis_name = chassis.name.clone();
                let label_sel = chassis.vm_selector.label_selector_string();
                let token_vms = chassis_token;
                tokio::spawn(async move {
                    discovery::start_kubevirt_vm_watcher(
                        vm_reg_c,
                        ns_vms,
                        chassis_name,
                        label_sel,
                        token_vms,
                    )
                    .await;
                });
            }
        }
    }

    // Swap the chassis list atomically.
    *state.chassis_config.write().unwrap() = new_chassis;

    info!("Config reloaded successfully");
}

async fn handle_config_reload(State(state): State<Arc<AggregatorState>>) -> StatusCode {
    reload_chassis_config(&state).await;
    StatusCode::NO_CONTENT
}

#[cfg(test)]
mod coverage_tests {
    use super::*;
    use axum::body::{Body, to_bytes};
    use axum::http::Request;
    use tower::ServiceExt;

    // Test helper to build a minimal AggregatorState for testing
    fn test_state() -> Arc<AggregatorState> {
        use super::super::k8s_auth::TokenCache;
        use super::super::k8s_authz::AuthzCache;
        use super::super::proxy::ProxyClient;
        use vbmc_rs::auth::accounts::AccountStore;
        use vbmc_rs::auth::sessions::SessionStore;
        use vbmc_rs::config::{AuthConfig, ServerConfig};

        let config = super::super::config::AggregatorConfig {
            server: ServerConfig {
                bind_address: "127.0.0.1".to_string(),
                port: 8080,
                tls_cert: None,
                tls_key: None,
                tls_client_ca: None,
            },
            auth: AuthConfig::default(),
            auth_mode: "local".to_string(),
            discovery: super::super::config::DiscoveryConfig {
                mode: "static".to_string(),
                namespace: None,
                label_selector: "app=vbmc".to_string(),
                bmc_network: None,
                endpoints: vec![],
            },
            sidecar: super::super::config::SidecarConnectionConfig {
                port: 8000,
                tls_ca: None,
                tls_cert: None,
                tls_key: None,
            },
            chassis: vec![],
        };

        let registry = Arc::new(super::super::discovery::SidecarRegistry::new());
        let proxy = ProxyClient::new(&config.sidecar).unwrap();

        let chassis_config = Arc::new(std::sync::RwLock::new(config.effective_chassis()));
        Arc::new(AggregatorState {
            chassis_config,
            watcher_handles: Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
            config_path: std::path::PathBuf::from("/tmp/test-aggregator.toml"),
            config,
            registry,
            vm_registry: None,
            proxy,
            session_store: SessionStore::new(3600, 16),
            account_store: std::sync::Mutex::new(AccountStore::default()),
            instance_uuid: "test-uuid".to_string(),
            kube_client: None,
            token_cache: TokenCache::default(),
            authz_cache: AuthzCache::default(),
        })
    }

    async fn get_json(app: &Router, uri: &str) -> (StatusCode, serde_json::Value) {
        let req = Request::builder()
            .method(Method::GET)
            .uri(uri)
            .body(Body::empty())
            .unwrap();
        let response = app.clone().oneshot(req).await.unwrap();
        let status = response.status();
        let bytes = to_bytes(response.into_body(), 2_000_000).await.unwrap();
        let json = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
        (status, json)
    }

    #[tokio::test]
    async fn test_router_construction() {
        let state = test_state();
        let _router = aggregator_router(state);
        // Router constructs without error
    }

    #[tokio::test]
    async fn test_get_redfish_root() {
        let state = test_state();
        let app = aggregator_router(state);

        let (status, json) = get_json(&app, "/redfish").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["v1"], "/redfish/v1/");
    }

    #[tokio::test]
    async fn test_get_service_root() {
        let state = test_state();
        let app = aggregator_router(state);

        let (status, json) = get_json(&app, "/redfish/v1").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["@odata.id"], "/redfish/v1");
        assert_eq!(json["@odata.type"], "#ServiceRoot.v1_17_0.ServiceRoot");
        assert_eq!(json["Id"], "RootService");
        assert_eq!(json["Name"], "vbmc-rs Aggregator Redfish Service");
        assert_eq!(json["RedfishVersion"], "1.21.0");
        assert_eq!(json["UUID"], "test-uuid");
        assert_eq!(json["Systems"]["@odata.id"], "/redfish/v1/Systems");
        assert_eq!(json["Chassis"]["@odata.id"], "/redfish/v1/Chassis");
    }

    #[tokio::test]
    async fn test_get_service_root_trailing_slash() {
        let state = test_state();
        let app = aggregator_router(state);

        let (status, _json) = get_json(&app, "/redfish/v1/").await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn test_get_metadata() {
        let state = test_state();
        let app = aggregator_router(state);

        let req = Request::builder()
            .method(Method::GET)
            .uri("/redfish/v1/$metadata")
            .body(Body::empty())
            .unwrap();
        let response = app.clone().oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "application/xml"
        );
        let bytes = to_bytes(response.into_body(), 2_000_000).await.unwrap();
        assert!(!bytes.is_empty());
    }

    #[tokio::test]
    async fn test_get_odata_service_document() {
        let state = test_state();
        let app = aggregator_router(state);

        let (status, json) = get_json(&app, "/redfish/v1/odata").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["@odata.context"], "/redfish/v1/$metadata");
        let value = json["value"].as_array().unwrap();
        assert_eq!(value.len(), 2);
        assert_eq!(value[0]["name"], "Systems");
        assert_eq!(value[1]["name"], "Chassis");
    }

    #[tokio::test]
    async fn test_get_aggregated_systems_empty() {
        let state = test_state();
        let app = aggregator_router(state);

        let (status, json) = get_json(&app, "/redfish/v1/Systems").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["@odata.id"], "/redfish/v1/Systems");
        assert_eq!(
            json["@odata.type"],
            "#ComputerSystemCollection.ComputerSystemCollection"
        );
        assert_eq!(json["Members@odata.count"], 0);
        let members = json["Members"].as_array().unwrap();
        assert_eq!(members.len(), 0);
    }

    #[tokio::test]
    async fn test_get_aggregated_chassis_empty() {
        let state = test_state();
        let app = aggregator_router(state);

        let (status, json) = get_json(&app, "/redfish/v1/Chassis").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["@odata.id"], "/redfish/v1/Chassis");
        assert_eq!(json["@odata.type"], "#ChassisCollection.ChassisCollection");
        assert_eq!(json["Members@odata.count"], 0);
        let members = json["Members"].as_array().unwrap();
        assert_eq!(members.len(), 0);
    }

    #[tokio::test]
    async fn test_proxy_system_get_not_found() {
        let state = test_state();
        let app = aggregator_router(state);

        let req = Request::builder()
            .method(Method::GET)
            .uri("/redfish/v1/Systems/nonexistent")
            .body(Body::empty())
            .unwrap();
        let response = app.clone().oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_proxy_chassis_get_not_found() {
        let state = test_state();
        let app = aggregator_router(state);

        let req = Request::builder()
            .method(Method::GET)
            .uri("/redfish/v1/Chassis/nonexistent")
            .body(Body::empty())
            .unwrap();
        let response = app.clone().oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn test_strip_auth_headers() {
        let mut headers = HeaderMap::new();
        headers.insert("content-type", "application/json".parse().unwrap());
        headers.insert("authorization", "Bearer token".parse().unwrap());
        headers.insert("x-auth-token", "session-token".parse().unwrap());
        headers.insert("user-agent", "test".parse().unwrap());

        let stripped = strip_auth_headers(&headers);
        assert!(stripped.contains_key("content-type"));
        assert!(stripped.contains_key("user-agent"));
        assert!(!stripped.contains_key("authorization"));
        assert!(!stripped.contains_key("x-auth-token"));
    }

    #[test]
    fn test_pagination_params_defaults() {
        let params: PaginationParams = serde_json::from_str("{}").unwrap();
        assert!(params.skip.is_none());
        assert!(params.top.is_none());
    }

    #[test]
    fn test_pagination_params_with_values() {
        let json = r#"{"$skip": 10, "$top": 25}"#;
        let params: PaginationParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.skip, Some(10));
        assert_eq!(params.top, Some(25));
    }

    #[test]
    fn test_pagination_params_skip_only() {
        let json = r#"{"$skip": 5}"#;
        let params: PaginationParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.skip, Some(5));
        assert!(params.top.is_none());
    }

    #[test]
    fn test_revocation_payload_deserialization() {
        let json = r#"{"agent_id": "vm-123"}"#;
        let payload: RevocationPayload = serde_json::from_str(json).unwrap();
        assert_eq!(payload.agent_id, "vm-123");
    }

    #[tokio::test]
    async fn test_handle_keylime_revocation_not_found() {
        let state = test_state();
        let app = aggregator_router(state);

        let req = Request::builder()
            .method(Method::POST)
            .uri("/api/v1/revocation")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"agent_id": "unknown-vm"}"#))
            .unwrap();
        let response = app.clone().oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn test_default_page_size_constant() {
        assert_eq!(DEFAULT_PAGE_SIZE, 50);
    }

    #[tokio::test]
    async fn test_proxy_system_post_not_found() {
        let state = test_state();
        let app = aggregator_router(state);

        let req = Request::builder()
            .method(Method::POST)
            .uri("/redfish/v1/Systems/nonexistent")
            .body(Body::empty())
            .unwrap();
        let response = app.clone().oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_proxy_system_patch_not_found() {
        let state = test_state();
        let app = aggregator_router(state);

        let req = Request::builder()
            .method(Method::PATCH)
            .uri("/redfish/v1/Systems/nonexistent")
            .body(Body::empty())
            .unwrap();
        let response = app.clone().oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_proxy_system_delete_not_found() {
        let state = test_state();
        let app = aggregator_router(state);

        let req = Request::builder()
            .method(Method::DELETE)
            .uri("/redfish/v1/Systems/nonexistent")
            .body(Body::empty())
            .unwrap();
        let response = app.clone().oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_proxy_chassis_post_not_found() {
        let state = test_state();
        let app = aggregator_router(state);

        let req = Request::builder()
            .method(Method::POST)
            .uri("/redfish/v1/Chassis/nonexistent")
            .body(Body::empty())
            .unwrap();
        let response = app.clone().oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    // ---- hybrid helpers ----

    #[test]
    fn test_parse_memory_mib_gi() {
        assert_eq!(parse_memory_mib("4Gi"), 4096);
    }

    #[test]
    fn test_parse_memory_mib_mi() {
        assert_eq!(parse_memory_mib("512Mi"), 512);
    }

    #[test]
    fn test_parse_memory_mib_ki() {
        assert_eq!(parse_memory_mib("4096Ki"), 4);
    }

    #[test]
    fn test_parse_memory_mib_g() {
        assert_eq!(parse_memory_mib("2G"), 1906); // 2 * 953
    }

    #[test]
    fn test_parse_memory_mib_m() {
        assert_eq!(parse_memory_mib("1024M"), 1024);
    }

    #[test]
    fn test_parse_memory_mib_bytes() {
        assert_eq!(parse_memory_mib("1048576"), 1);
    }

    #[test]
    fn test_parse_memory_mib_invalid() {
        assert_eq!(parse_memory_mib("bad"), 0);
    }

    #[test]
    fn test_vm_entry_to_endpoint() {
        use super::super::discovery::KubeVirtVmEntry;
        let entry = KubeVirtVmEntry {
            system_id: "sys-1".to_string(),
            namespace: "ns1".to_string(),
            vm_name: "my-vm".to_string(),
            chassis_name: "ns1".to_string(),
        };
        let ep = vm_entry_to_endpoint(&entry);
        assert_eq!(ep.system_id, "sys-1");
        assert_eq!(ep.namespace, "ns1");
        assert_eq!(ep.vm_name, "my-vm");
        assert!(ep.url.is_empty());
    }

    // ---- hybrid Systems collection ----

    fn hybrid_state() -> Arc<AggregatorState> {
        use super::super::discovery::{KubeVirtVmEntry, KubeVirtVmRegistry};
        use super::super::k8s_auth::TokenCache;
        use super::super::k8s_authz::AuthzCache;
        use super::super::proxy::ProxyClient;
        use vbmc_rs::auth::accounts::AccountStore;
        use vbmc_rs::auth::sessions::SessionStore;
        use vbmc_rs::config::{AuthConfig, ServerConfig};

        let config = super::super::config::AggregatorConfig {
            server: ServerConfig {
                bind_address: "127.0.0.1".to_string(),
                port: 8080,
                tls_cert: None,
                tls_key: None,
                tls_client_ca: None,
            },
            auth: AuthConfig::default(),
            auth_mode: "local".to_string(),
            discovery: super::super::config::DiscoveryConfig {
                mode: "kubevirt-hybrid".to_string(),
                namespace: Some("default".to_string()),
                label_selector: "app=vbmc".to_string(),
                bmc_network: None,
                endpoints: vec![],
            },
            sidecar: super::super::config::SidecarConnectionConfig {
                port: 8000,
                tls_ca: None,
                tls_cert: None,
                tls_key: None,
            },
            chassis: vec![],
        };

        let registry = Arc::new(super::super::discovery::SidecarRegistry::new());
        let vm_reg = Arc::new(KubeVirtVmRegistry::new());
        vm_reg.register(KubeVirtVmEntry {
            system_id: "vm-a".to_string(),
            namespace: "default".to_string(),
            vm_name: "my-vm-a".to_string(),
            chassis_name: "default".to_string(),
        });
        vm_reg.register(KubeVirtVmEntry {
            system_id: "vm-b".to_string(),
            namespace: "default".to_string(),
            vm_name: "my-vm-b".to_string(),
            chassis_name: "default".to_string(),
        });

        let proxy = ProxyClient::new(&config.sidecar).unwrap();
        let chassis_config = Arc::new(std::sync::RwLock::new(config.effective_chassis()));
        Arc::new(AggregatorState {
            chassis_config,
            watcher_handles: Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
            config_path: std::path::PathBuf::from("/tmp/test-aggregator.toml"),
            config,
            registry,
            vm_registry: Some(vm_reg),
            proxy,
            session_store: SessionStore::new(3600, 16),
            account_store: std::sync::Mutex::new(AccountStore::default()),
            instance_uuid: "hybrid-uuid".to_string(),
            kube_client: None,
            token_cache: TokenCache::default(),
            authz_cache: AuthzCache::default(),
        })
    }

    #[tokio::test]
    async fn test_hybrid_systems_list_includes_all_vms() {
        let state = hybrid_state();
        let app = aggregator_router(state);

        let (status, json) = get_json(&app, "/redfish/v1/Systems").await;
        assert_eq!(status, StatusCode::OK);

        let count = json["Members@odata.count"].as_u64().unwrap();
        assert_eq!(count, 2);

        let members = json["Members"].as_array().unwrap();
        let ids: Vec<&str> = members
            .iter()
            .filter_map(|m| m["@odata.id"].as_str())
            .collect();
        assert!(ids.iter().any(|id| id.contains("vm-a")));
        assert!(ids.iter().any(|id| id.contains("vm-b")));
    }

    #[tokio::test]
    async fn test_hybrid_system_get_not_in_registry_returns_404() {
        let state = hybrid_state();
        let app = aggregator_router(state);

        let req = Request::builder()
            .method(Method::GET)
            .uri("/redfish/v1/Systems/nonexistent")
            .body(Body::empty())
            .unwrap();
        let response = app.clone().oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_hybrid_reset_known_vm_no_kube_client_returns_500() {
        // vm-a is in the VM registry; no kube_client → 500
        let state = hybrid_state();
        let app = aggregator_router(state);

        let req = Request::builder()
            .method(Method::POST)
            .uri("/redfish/v1/Systems/vm-a/Actions/ComputerSystem.Reset")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"ResetType":"On"}"#))
            .unwrap();
        let response = app.clone().oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn test_hybrid_reset_unknown_vm_falls_back_to_sidecar_not_found() {
        // not in vm_registry, not in sidecar registry → 404
        let state = hybrid_state();
        let app = aggregator_router(state);

        let req = Request::builder()
            .method(Method::POST)
            .uri("/redfish/v1/Systems/nonexistent/Actions/ComputerSystem.Reset")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"ResetType":"On"}"#))
            .unwrap();
        let response = app.clone().oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    // ---- hybrid chassis endpoints ----

    fn hybrid_state_with_chassis() -> Arc<AggregatorState> {
        use super::super::config::ChassisConfig;
        use super::super::discovery::{KubeVirtVmEntry, KubeVirtVmRegistry};
        use super::super::k8s_auth::TokenCache;
        use super::super::k8s_authz::AuthzCache;
        use super::super::proxy::ProxyClient;
        use vbmc_rs::auth::accounts::AccountStore;
        use vbmc_rs::auth::sessions::SessionStore;
        use vbmc_rs::config::{AuthConfig, ServerConfig};

        let config = super::super::config::AggregatorConfig {
            server: ServerConfig {
                bind_address: "127.0.0.1".to_string(),
                port: 8080,
                tls_cert: None,
                tls_key: None,
                tls_client_ca: None,
            },
            auth: AuthConfig::default(),
            auth_mode: "local".to_string(),
            discovery: super::super::config::DiscoveryConfig {
                mode: "kubevirt-hybrid".to_string(),
                namespace: None,
                label_selector: "app=vbmc".to_string(),
                bmc_network: None,
                endpoints: vec![],
            },
            sidecar: super::super::config::SidecarConnectionConfig {
                port: 8000,
                tls_ca: None,
                tls_cert: None,
                tls_key: None,
            },
            chassis: vec![
                ChassisConfig {
                    name: "ns-a".to_string(),
                    namespace: "ns-a".to_string(),
                    vm_selector: Default::default(),
                },
                ChassisConfig {
                    name: "ns-b".to_string(),
                    namespace: "ns-b".to_string(),
                    vm_selector: Default::default(),
                },
            ],
        };

        let registry = Arc::new(super::super::discovery::SidecarRegistry::new());
        let vm_reg = Arc::new(KubeVirtVmRegistry::new());
        vm_reg.register(KubeVirtVmEntry {
            system_id: "vm-1".to_string(),
            namespace: "ns-a".to_string(),
            vm_name: "vm-1".to_string(),
            chassis_name: "ns-a".to_string(),
        });
        vm_reg.register(KubeVirtVmEntry {
            system_id: "vm-2".to_string(),
            namespace: "ns-b".to_string(),
            vm_name: "vm-2".to_string(),
            chassis_name: "ns-b".to_string(),
        });

        let proxy = ProxyClient::new(&config.sidecar).unwrap();
        let chassis_config = Arc::new(std::sync::RwLock::new(config.effective_chassis()));
        Arc::new(AggregatorState {
            chassis_config,
            watcher_handles: Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
            config_path: std::path::PathBuf::from("/tmp/test-aggregator.toml"),
            config,
            registry,
            vm_registry: Some(vm_reg),
            proxy,
            session_store: SessionStore::new(3600, 16),
            account_store: std::sync::Mutex::new(AccountStore::default()),
            instance_uuid: "chassis-uuid".to_string(),
            kube_client: None,
            token_cache: TokenCache::default(),
            authz_cache: AuthzCache::default(),
        })
    }

    #[tokio::test]
    async fn test_hybrid_chassis_list_uses_config() {
        let state = hybrid_state_with_chassis();
        let app = aggregator_router(state);

        let (status, json) = get_json(&app, "/redfish/v1/Chassis").await;
        assert_eq!(status, StatusCode::OK);
        let members = json["Members"].as_array().unwrap();
        assert_eq!(members.len(), 2);
        let ids: Vec<&str> = members
            .iter()
            .filter_map(|m| m["@odata.id"].as_str())
            .collect();
        assert!(ids.iter().any(|id| id.contains("ns-a")));
        assert!(ids.iter().any(|id| id.contains("ns-b")));
    }

    #[tokio::test]
    async fn test_hybrid_chassis_get_lists_vms_in_chassis() {
        let state = hybrid_state_with_chassis();
        let app = aggregator_router(state);

        let (status, json) = get_json(&app, "/redfish/v1/Chassis/ns-a").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["Id"], "ns-a");
        let systems = json["Links"]["ComputerSystems"].as_array().unwrap();
        assert_eq!(systems.len(), 1);
        assert!(systems[0]["@odata.id"].as_str().unwrap().contains("vm-1"));
    }

    #[tokio::test]
    async fn test_hybrid_chassis_get_unknown_returns_404() {
        let state = hybrid_state_with_chassis();
        let app = aggregator_router(state);

        let req = Request::builder()
            .method(Method::GET)
            .uri("/redfish/v1/Chassis/nonexistent")
            .body(Body::empty())
            .unwrap();
        let response = app.clone().oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_hybrid_chassis_list_from_discovery_namespace_fallback() {
        // hybrid_state() uses chassis:[] + namespace:"default" → effective_chassis synthesises one
        let state = hybrid_state();
        let app = aggregator_router(state);

        let (status, json) = get_json(&app, "/redfish/v1/Chassis").await;
        assert_eq!(status, StatusCode::OK);
        let members = json["Members"].as_array().unwrap();
        assert_eq!(members.len(), 1);
        assert!(
            members[0]["@odata.id"]
                .as_str()
                .unwrap()
                .contains("default")
        );
    }

    #[tokio::test]
    async fn test_non_hybrid_chassis_list_from_sidecar_endpoints() {
        // test_state() has no vm_registry → uses sidecar endpoint namespace grouping
        let state = test_state();
        let app = aggregator_router(state);

        let (status, json) = get_json(&app, "/redfish/v1/Chassis").await;
        assert_eq!(status, StatusCode::OK);
        // No endpoints registered → empty
        assert_eq!(json["Members@odata.count"], 0);
    }

    // ---- config hot-reload endpoint ----

    #[tokio::test]
    async fn test_config_reload_endpoint_returns_204_when_file_missing() {
        // config_path points to /tmp/test-aggregator.toml which doesn't exist;
        // reload should fail gracefully and return 204 (best-effort, no crash).
        let state = test_state();
        let app = aggregator_router(state);

        let req = Request::builder()
            .method(Method::POST)
            .uri("/api/v1/config/reload")
            .body(Body::empty())
            .unwrap();
        let response = app.clone().oneshot(req).await.unwrap();
        // File missing → reload_chassis_config logs a warning and returns early;
        // handle_config_reload always returns 204.
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn test_reload_chassis_config_updates_chassis_list() {
        use super::reload_chassis_config;
        use std::io::Write;

        // Write a valid aggregator config to a temp file.
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        write!(
            tmp,
            r#"
[server]
bind_address = "127.0.0.1"
port = 8080

[discovery]
mode = "kubevirt-hybrid"
endpoints = []

[sidecar]

[[chassis]]
name = "reloaded-chassis"
namespace = "reloaded-ns"
"#
        )
        .unwrap();

        let state = {
            use super::super::k8s_auth::TokenCache;
            use super::super::k8s_authz::AuthzCache;
            use super::super::proxy::ProxyClient;
            use vbmc_rs::auth::accounts::AccountStore;
            use vbmc_rs::auth::sessions::SessionStore;
            use vbmc_rs::config::{AuthConfig, ServerConfig};

            let config = super::super::config::AggregatorConfig {
                server: ServerConfig {
                    bind_address: "127.0.0.1".to_string(),
                    port: 8080,
                    tls_cert: None,
                    tls_key: None,
                    tls_client_ca: None,
                },
                auth: AuthConfig::default(),
                auth_mode: "local".to_string(),
                discovery: super::super::config::DiscoveryConfig {
                    mode: "kubevirt-hybrid".to_string(),
                    namespace: None,
                    label_selector: "app=vbmc".to_string(),
                    bmc_network: None,
                    endpoints: vec![],
                },
                sidecar: super::super::config::SidecarConnectionConfig {
                    port: 8000,
                    tls_ca: None,
                    tls_cert: None,
                    tls_key: None,
                },
                chassis: vec![],
            };

            let proxy = ProxyClient::new(&config.sidecar).unwrap();
            let chassis_config = Arc::new(std::sync::RwLock::new(config.effective_chassis()));
            Arc::new(AggregatorState {
                chassis_config,
                watcher_handles: Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
                config_path: tmp.path().to_path_buf(),
                config,
                registry: Arc::new(super::super::discovery::SidecarRegistry::new()),
                vm_registry: None,
                proxy,
                session_store: SessionStore::new(3600, 16),
                account_store: std::sync::Mutex::new(AccountStore::default()),
                instance_uuid: "reload-uuid".to_string(),
                kube_client: None,
                token_cache: TokenCache::default(),
                authz_cache: AuthzCache::default(),
            })
        };

        // Initially empty chassis list.
        assert!(state.chassis_config.read().unwrap().is_empty());

        // Reload from the temp file.
        reload_chassis_config(&state).await;

        // Should now have the reloaded chassis.
        let chassis = state.chassis_config.read().unwrap().clone();
        assert_eq!(chassis.len(), 1);
        assert_eq!(chassis[0].name, "reloaded-chassis");
        assert_eq!(chassis[0].namespace, "reloaded-ns");
    }

    #[tokio::test]
    async fn test_reload_removes_chassis_and_cancels_token() {
        use super::reload_chassis_config;
        use std::io::Write;
        use tokio_util::sync::CancellationToken;

        // Config file with no [[chassis]] sections (and no namespace).
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        write!(
            tmp,
            r#"
[server]
bind_address = "127.0.0.1"
port = 8080

[discovery]
mode = "static"
endpoints = []

[sidecar]
"#
        )
        .unwrap();

        let state = {
            use super::super::k8s_auth::TokenCache;
            use super::super::k8s_authz::AuthzCache;
            use super::super::proxy::ProxyClient;
            use vbmc_rs::auth::accounts::AccountStore;
            use vbmc_rs::auth::sessions::SessionStore;
            use vbmc_rs::config::{AuthConfig, ServerConfig};

            let config = super::super::config::AggregatorConfig {
                server: ServerConfig {
                    bind_address: "127.0.0.1".to_string(),
                    port: 8080,
                    tls_cert: None,
                    tls_key: None,
                    tls_client_ca: None,
                },
                auth: AuthConfig::default(),
                auth_mode: "local".to_string(),
                discovery: super::super::config::DiscoveryConfig {
                    mode: "static".to_string(),
                    namespace: None,
                    label_selector: String::new(),
                    bmc_network: None,
                    endpoints: vec![],
                },
                sidecar: super::super::config::SidecarConnectionConfig {
                    port: 8000,
                    tls_ca: None,
                    tls_cert: None,
                    tls_key: None,
                },
                chassis: vec![],
            };

            let proxy = ProxyClient::new(&config.sidecar).unwrap();
            // Pre-populate watcher_handles with a token for "old-chassis".
            let old_token = CancellationToken::new();
            let mut handles = std::collections::HashMap::new();
            handles.insert("old-chassis".to_string(), old_token.clone());

            let state = Arc::new(AggregatorState {
                chassis_config: Arc::new(std::sync::RwLock::new(vec![
                    super::super::config::ChassisConfig {
                        name: "old-chassis".to_string(),
                        namespace: "old-ns".to_string(),
                        vm_selector: Default::default(),
                    },
                ])),
                watcher_handles: Arc::new(std::sync::Mutex::new(handles)),
                config_path: tmp.path().to_path_buf(),
                config,
                registry: Arc::new(super::super::discovery::SidecarRegistry::new()),
                vm_registry: None,
                proxy,
                session_store: SessionStore::new(3600, 16),
                account_store: std::sync::Mutex::new(AccountStore::default()),
                instance_uuid: "remove-uuid".to_string(),
                kube_client: None,
                token_cache: TokenCache::default(),
                authz_cache: AuthzCache::default(),
            });
            (state, old_token)
        };

        let (state, old_token) = state;
        assert!(!old_token.is_cancelled());

        // Reload: config has no chassis → old-chassis should be cancelled.
        reload_chassis_config(&state).await;

        assert!(
            old_token.is_cancelled(),
            "removed chassis token should be cancelled"
        );
        assert!(state.chassis_config.read().unwrap().is_empty());
        assert!(state.watcher_handles.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_reload_with_hybrid_mode_and_vm_registry_spawns_watchers() {
        use super::reload_chassis_config;
        use std::io::Write;

        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        write!(
            tmp,
            r#"
[server]
bind_address = "127.0.0.1"
port = 8080

[discovery]
mode = "kubevirt-hybrid"
endpoints = []

[sidecar]

[[chassis]]
name = "new-chassis"
namespace = "new-ns"
"#
        )
        .unwrap();

        let state = {
            use super::super::discovery::KubeVirtVmRegistry;
            use super::super::k8s_auth::TokenCache;
            use super::super::k8s_authz::AuthzCache;
            use super::super::proxy::ProxyClient;
            use vbmc_rs::auth::accounts::AccountStore;
            use vbmc_rs::auth::sessions::SessionStore;
            use vbmc_rs::config::{AuthConfig, ServerConfig};

            let config = super::super::config::AggregatorConfig {
                server: ServerConfig {
                    bind_address: "127.0.0.1".to_string(),
                    port: 8080,
                    tls_cert: None,
                    tls_key: None,
                    tls_client_ca: None,
                },
                auth: AuthConfig::default(),
                auth_mode: "local".to_string(),
                discovery: super::super::config::DiscoveryConfig {
                    mode: "kubevirt-hybrid".to_string(),
                    namespace: None,
                    label_selector: "app=vbmc".to_string(),
                    bmc_network: None,
                    endpoints: vec![],
                },
                sidecar: super::super::config::SidecarConnectionConfig {
                    port: 8000,
                    tls_ca: None,
                    tls_cert: None,
                    tls_key: None,
                },
                chassis: vec![],
            };

            let proxy = ProxyClient::new(&config.sidecar).unwrap();
            Arc::new(AggregatorState {
                chassis_config: Arc::new(std::sync::RwLock::new(vec![])),
                watcher_handles: Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
                config_path: tmp.path().to_path_buf(),
                config,
                registry: Arc::new(super::super::discovery::SidecarRegistry::new()),
                vm_registry: Some(Arc::new(KubeVirtVmRegistry::new())),
                proxy,
                session_store: SessionStore::new(3600, 16),
                account_store: std::sync::Mutex::new(AccountStore::default()),
                instance_uuid: "hybrid-reload-uuid".to_string(),
                kube_client: None,
                token_cache: TokenCache::default(),
                authz_cache: AuthzCache::default(),
            })
        };

        reload_chassis_config(&state).await;

        // chassis_config updated
        let chassis = state.chassis_config.read().unwrap().clone();
        assert_eq!(chassis.len(), 1);
        assert_eq!(chassis[0].name, "new-chassis");

        // watcher_handles has an entry for the new chassis
        let handles = state.watcher_handles.lock().unwrap();
        assert!(handles.contains_key("new-chassis"));
    }

    #[tokio::test]
    async fn test_config_reload_endpoint_with_valid_file() {
        use std::io::Write;

        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        write!(
            tmp,
            r#"
[server]
bind_address = "127.0.0.1"
port = 8080

[discovery]
endpoints = []

[sidecar]

[[chassis]]
name = "live-chassis"
namespace = "live-ns"
"#
        )
        .unwrap();

        let state = {
            use super::super::k8s_auth::TokenCache;
            use super::super::k8s_authz::AuthzCache;
            use super::super::proxy::ProxyClient;
            use vbmc_rs::auth::accounts::AccountStore;
            use vbmc_rs::auth::sessions::SessionStore;
            use vbmc_rs::config::{AuthConfig, ServerConfig};

            let config = super::super::config::AggregatorConfig {
                server: ServerConfig {
                    bind_address: "127.0.0.1".to_string(),
                    port: 8080,
                    tls_cert: None,
                    tls_key: None,
                    tls_client_ca: None,
                },
                auth: AuthConfig::default(),
                auth_mode: "local".to_string(),
                discovery: super::super::config::DiscoveryConfig {
                    mode: "static".to_string(),
                    namespace: None,
                    label_selector: String::new(),
                    bmc_network: None,
                    endpoints: vec![],
                },
                sidecar: super::super::config::SidecarConnectionConfig {
                    port: 8000,
                    tls_ca: None,
                    tls_cert: None,
                    tls_key: None,
                },
                chassis: vec![],
            };

            let proxy = ProxyClient::new(&config.sidecar).unwrap();
            Arc::new(AggregatorState {
                chassis_config: Arc::new(std::sync::RwLock::new(vec![])),
                watcher_handles: Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
                config_path: tmp.path().to_path_buf(),
                config,
                registry: Arc::new(super::super::discovery::SidecarRegistry::new()),
                vm_registry: None,
                proxy,
                session_store: SessionStore::new(3600, 16),
                account_store: std::sync::Mutex::new(AccountStore::default()),
                instance_uuid: "reload-valid-uuid".to_string(),
                kube_client: None,
                token_cache: TokenCache::default(),
                authz_cache: AuthzCache::default(),
            })
        };

        let app = aggregator_router(state.clone());

        let req = Request::builder()
            .method(Method::POST)
            .uri("/api/v1/config/reload")
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);

        // Chassis list updated from file
        let chassis = state.chassis_config.read().unwrap().clone();
        assert_eq!(chassis.len(), 1);
        assert_eq!(chassis[0].name, "live-chassis");
    }
}
