use std::sync::Arc;

use axum::Json;
use axum::Router;
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use tracing::{info, warn};

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
        .route(
            "/redfish/v1/Systems/{system_id}",
            get(proxy_system_get)
                .post(proxy_system_mutate)
                .patch(proxy_system_mutate)
                .delete(proxy_system_mutate),
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

async fn get_aggregated_systems(
    State(state): State<Arc<AggregatorState>>,
    user: KubernetesUser,
    axum::extract::Query(params): axum::extract::Query<PaginationParams>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let endpoints = state.registry.list();
    let mut all_members = Vec::new();

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
    let endpoint = state
        .registry
        .get(&system_id)
        .ok_or(StatusCode::NOT_FOUND)?;
    if !check_endpoint_access(&state, &user, &endpoint).await {
        return Err(StatusCode::FORBIDDEN);
    }
    let path = format!("/redfish/v1/Systems/{system_id}");
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

async fn proxy_chassis_get(
    State(state): State<Arc<AggregatorState>>,
    user: KubernetesUser,
    Path(system_id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    let endpoint = state
        .registry
        .get(&system_id)
        .ok_or(StatusCode::NOT_FOUND)?;
    if !check_endpoint_access(&state, &user, &endpoint).await {
        return Err(StatusCode::FORBIDDEN);
    }
    let path = format!("/redfish/v1/Chassis/{system_id}");
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
