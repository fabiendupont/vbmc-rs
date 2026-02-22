#![cfg(test)]

use std::collections::HashMap;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

use crate::app_state::AppState;
use crate::auth::accounts::AccountStore;
use crate::backend::mock::MockBackend;
use crate::backend::types::{
    DiskInfo, DiskMediaType, DiskProtocol, NicInfo, VmInfo, VmPowerState,
};
use crate::backend::Backend;
use crate::config::{
    AppConfig, AuthConfig, BackendType, DefaultsConfig, HardwareConfig, MetricsConfig,
    SecurityPolicyConfig, ServerConfig, SystemConfig,
};

fn make_test_config(systems: HashMap<String, SystemConfig>) -> AppConfig {
    AppConfig {
        server: ServerConfig {
            bind_address: "127.0.0.1".to_string(),
            port: 8000,
            tls_cert: None,
            tls_key: None,
            tls_client_ca: None,
        },
        backend: BackendType::CloudHypervisor,
        auth: AuthConfig::default(),
        defaults: DefaultsConfig::default(),
        security_policy: SecurityPolicyConfig::default(),
        state_directory: std::env::temp_dir().join("vbmc-rs-test"),
        audit_log: Default::default(),
        metrics: MetricsConfig::default(),
        systems,
    }
}

fn make_system_config(name: &str) -> SystemConfig {
    SystemConfig {
        name: Some(name.to_string()),
        socket_path: None,
        firmware_path: None,
        boot_source: None,
        virtual_media_directory: None,
        hardware: HardwareConfig::default(),
        connection_uri: None,
        domain_name: None,
        attestation: None,
    }
}

fn make_running_vm() -> VmInfo {
    VmInfo {
        power_state: VmPowerState::On,
        cpu_count: 4,
        max_cpu_count: 8,
        cpu_topology: None,
        memory_bytes: 4 * 1024 * 1024 * 1024,
        memory_actual_bytes: Some(4 * 1024 * 1024 * 1024),
        disks: vec![DiskInfo {
            id: "vda".to_string(),
            path: Some("/tmp/disk.qcow2".to_string()),
            capacity_bytes: Some(10_000_000_000),
            readonly: false,
            protocol: DiskProtocol::Virtio,
            media_type: DiskMediaType::SSD,
        }],
        nics: vec![NicInfo {
            id: "NIC0".to_string(),
            mac_address: Some("52:54:00:12:34:56".to_string()),
            tap: Some("tap0".to_string()),
            speed_mbps: 25000,
        }],
        pci_devices: vec![],
        raw: None,
    }
}

fn make_app_state(mock: MockBackend, systems: HashMap<String, SystemConfig>) -> Arc<AppState> {
    let config = make_test_config(systems);
    Arc::new(AppState::new(
        config,
        Backend::Mock(mock),
        AccountStore::default(),
    ))
}

fn build_app(state: Arc<AppState>) -> axum::Router {
    crate::redfish::router(state)
}

async fn get(app: &axum::Router, uri: &str) -> (StatusCode, serde_json::Value, axum::http::HeaderMap) {
    let req = Request::builder()
        .method("GET")
        .uri(uri)
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(req).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = axum::body::to_bytes(response.into_body(), 1_000_000)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
    (status, json, headers)
}

async fn get_raw(app: &axum::Router, uri: &str) -> (StatusCode, Vec<u8>, axum::http::HeaderMap) {
    let req = Request::builder()
        .method("GET")
        .uri(uri)
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(req).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = axum::body::to_bytes(response.into_body(), 1_000_000)
        .await
        .unwrap();
    (status, body.to_vec(), headers)
}

async fn head(app: &axum::Router, uri: &str) -> (StatusCode, Vec<u8>, axum::http::HeaderMap) {
    let req = Request::builder()
        .method("HEAD")
        .uri(uri)
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(req).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = axum::body::to_bytes(response.into_body(), 1_000_000)
        .await
        .unwrap();
    (status, body.to_vec(), headers)
}

async fn post_json(
    app: &axum::Router,
    uri: &str,
    body: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    let req = Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();
    let response = app.clone().oneshot(req).await.unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 1_000_000)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
    (status, json)
}

async fn patch_json(
    app: &axum::Router,
    uri: &str,
    body: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    let req = Request::builder()
        .method("PATCH")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();
    let response = app.clone().oneshot(req).await.unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 1_000_000)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
    (status, json)
}

// ── OData compliance middleware ───────────────────────────────────────

#[tokio::test]
async fn test_odata_version_header() {
    let state = make_app_state(MockBackend::new(), HashMap::new());
    let app = build_app(state);

    let (status, _, headers) = get(&app, "/redfish/v1").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers.get("odata-version").unwrap(), "4.0");
}

#[tokio::test]
async fn test_link_header() {
    let state = make_app_state(MockBackend::new(), HashMap::new());
    let app = build_app(state);

    let (_, _, headers) = get(&app, "/redfish/v1").await;
    assert_eq!(
        headers.get("link").unwrap(),
        "</redfish/v1/$metadata>; rel=describedby"
    );
}

#[tokio::test]
async fn test_head_returns_empty_body() {
    let state = make_app_state(MockBackend::new(), HashMap::new());
    let app = build_app(state);

    let (status, body, headers) = head(&app, "/redfish/v1").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.is_empty());
    assert_eq!(headers.get("odata-version").unwrap(), "4.0");
}

#[tokio::test]
async fn test_head_on_systems() {
    let mut systems = HashMap::new();
    systems.insert("vm1".to_string(), make_system_config("VM 1"));
    let state = make_app_state(MockBackend::new(), systems);
    let app = build_app(state);

    let (status, body, _) = head(&app, "/redfish/v1/Systems").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.is_empty());
}

// ── OData metadata and service document ───────────────────────────────

#[tokio::test]
async fn test_metadata_returns_xml() {
    let state = make_app_state(MockBackend::new(), HashMap::new());
    let app = build_app(state);

    let (status, body, headers) = get_raw(&app, "/redfish/v1/$metadata").await;
    assert_eq!(status, StatusCode::OK);
    let content_type = headers.get("content-type").unwrap().to_str().unwrap();
    assert!(content_type.contains("application/xml"));
    let body_str = String::from_utf8(body).unwrap();
    assert!(body_str.contains("<edmx:Edmx"));
    assert!(body_str.contains("ComputerSystem"));
}

#[tokio::test]
async fn test_odata_service_document() {
    let state = make_app_state(MockBackend::new(), HashMap::new());
    let app = build_app(state);

    let (status, json, _) = get(&app, "/redfish/v1/odata").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["@odata.context"], "/redfish/v1/$metadata");
    let values = json["value"].as_array().unwrap();
    assert!(values.iter().any(|v| v["name"] == "Systems"));
    assert!(values.iter().any(|v| v["name"] == "Chassis"));
    assert!(values.iter().any(|v| v["name"] == "Managers"));
}

// ── Service root ──────────────────────────────────────────────────────

#[tokio::test]
async fn test_redfish_root() {
    let state = make_app_state(MockBackend::new(), HashMap::new());
    let app = build_app(state);

    let (status, json, _) = get(&app, "/redfish").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["v1"], "/redfish/v1/");
}

#[tokio::test]
async fn test_service_root() {
    let state = make_app_state(MockBackend::new(), HashMap::new());
    let app = build_app(state);

    let (status, json, _) = get(&app, "/redfish/v1").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["@odata.type"], "#ServiceRoot.v1_17_0.ServiceRoot");
    assert_eq!(json["RedfishVersion"], "1.21.0");
    assert_eq!(json["Systems"]["@odata.id"], "/redfish/v1/Systems");
    assert_eq!(json["Managers"]["@odata.id"], "/redfish/v1/Managers");
    assert_eq!(json["Chassis"]["@odata.id"], "/redfish/v1/Chassis");
    assert_eq!(json["UpdateService"]["@odata.id"], "/redfish/v1/UpdateService");
    assert_eq!(json["LicenseService"]["@odata.id"], "/redfish/v1/LicenseService");
}

// ── Systems ───────────────────────────────────────────────────────────

#[tokio::test]
async fn test_systems_collection() {
    let mut systems = HashMap::new();
    systems.insert("vm1".to_string(), make_system_config("VM 1"));
    systems.insert("vm2".to_string(), make_system_config("VM 2"));
    let state = make_app_state(MockBackend::new(), systems);
    let app = build_app(state);

    let (status, json, _) = get(&app, "/redfish/v1/Systems").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["Members@odata.count"], 2);
    assert!(json["@odata.context"].as_str().unwrap().starts_with("/redfish/v1/$metadata#"));
}

#[tokio::test]
async fn test_get_system_running() {
    let mut systems = HashMap::new();
    systems.insert("vm1".to_string(), make_system_config("Test VM"));
    let mock = MockBackend::new().with_vm("vm1", make_running_vm());
    let state = make_app_state(mock, systems);
    let app = build_app(state);

    let (status, json, _) = get(&app, "/redfish/v1/Systems/vm1").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["Id"], "vm1");
    assert_eq!(json["Name"], "Test VM");
    assert_eq!(json["PowerState"], "On");
    assert_eq!(json["SystemType"], "Virtual");
    assert_eq!(json["Status"]["State"], "Enabled");
    assert_eq!(json["Status"]["Health"], "OK");
    assert_eq!(json["ProcessorSummary"]["Count"], 4);
    // 4 GiB
    let mem_gib = json["MemorySummary"]["TotalSystemMemoryGiB"].as_f64().unwrap();
    assert!((mem_gib - 4.0).abs() < 0.01);
    // Links present
    assert!(json["Memory"]["@odata.id"].as_str().is_some());
    assert!(json["Storage"]["@odata.id"].as_str().is_some());
    assert!(json["Bios"]["@odata.id"].as_str().is_some());
    assert!(json["LogServices"]["@odata.id"].as_str().is_some());
}

#[tokio::test]
async fn test_get_system_not_found() {
    let state = make_app_state(MockBackend::new(), HashMap::new());
    let app = build_app(state);

    let (status, json, _) = get(&app, "/redfish/v1/Systems/nonexistent").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(json["error"]["message"].as_str().unwrap().contains("nonexistent"));
}

#[tokio::test]
async fn test_get_system_backend_down() {
    let mut systems = HashMap::new();
    systems.insert("vm1".to_string(), make_system_config("VM 1"));
    // MockBackend has no VMs registered, so vm_info returns VmmNotRunning
    let state = make_app_state(MockBackend::new(), systems);
    let app = build_app(state);

    let (status, json, _) = get(&app, "/redfish/v1/Systems/vm1").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["PowerState"], "Off");
    assert_eq!(json["Status"]["State"], "UnavailableOffline");
}

#[tokio::test]
async fn test_patch_system_boot_override() {
    let mut systems = HashMap::new();
    systems.insert("vm1".to_string(), make_system_config("VM 1"));
    let state = make_app_state(MockBackend::new(), systems);
    let state_ref = state.clone();
    let app = build_app(state);

    let (status, _) = patch_json(
        &app,
        "/redfish/v1/Systems/vm1",
        serde_json::json!({
            "Boot": {
                "BootSourceOverrideTarget": "Cd",
                "BootSourceOverrideEnabled": "Once"
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // Verify state was saved
    let vm_state = state_ref.get_vm_state("vm1");
    assert_eq!(vm_state.boot_override.target.as_deref(), Some("Cd"));
    assert_eq!(vm_state.boot_override.enabled, "Once");
}

// ── Power actions ─────────────────────────────────────────────────────

#[tokio::test]
async fn test_reset_graceful_shutdown() {
    let mut systems = HashMap::new();
    systems.insert("vm1".to_string(), make_system_config("VM 1"));
    let mock = MockBackend::new().with_vm("vm1", make_running_vm());
    let state = make_app_state(mock, systems);
    let app = build_app(state);

    let (status, json) = post_json(
        &app,
        "/redfish/v1/Systems/vm1/Actions/ComputerSystem.Reset",
        serde_json::json!({"ResetType": "GracefulShutdown"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(json["message"].as_str().unwrap().contains("GracefulShutdown"));
}

#[tokio::test]
async fn test_reset_invalid_type() {
    let mut systems = HashMap::new();
    systems.insert("vm1".to_string(), make_system_config("VM 1"));
    let state = make_app_state(MockBackend::new(), systems);
    let app = build_app(state);

    let (status, json) = post_json(
        &app,
        "/redfish/v1/Systems/vm1/Actions/ComputerSystem.Reset",
        serde_json::json!({"ResetType": "InvalidType"}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(json["error"]["message"].as_str().unwrap().contains("InvalidType"));
}

#[tokio::test]
async fn test_reset_not_found_system() {
    let state = make_app_state(MockBackend::new(), HashMap::new());
    let app = build_app(state);

    let (status, _, ) = post_json(
        &app,
        "/redfish/v1/Systems/nonexistent/Actions/ComputerSystem.Reset",
        serde_json::json!({"ResetType": "On"}),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ── Ethernet interfaces ───────────────────────────────────────────────

#[tokio::test]
async fn test_ethernet_interfaces() {
    let mut systems = HashMap::new();
    systems.insert("vm1".to_string(), make_system_config("VM 1"));
    let mock = MockBackend::new().with_vm("vm1", make_running_vm());
    let state = make_app_state(mock, systems);
    let app = build_app(state);

    let (status, json, _) = get(&app, "/redfish/v1/Systems/vm1/EthernetInterfaces").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["Members@odata.count"], 1);

    let (status, json, _) = get(&app, "/redfish/v1/Systems/vm1/EthernetInterfaces/NIC0").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["MACAddress"], "52:54:00:12:34:56");
    assert_eq!(json["SpeedMbps"], 25000);
}

// ── Processors ────────────────────────────────────────────────────────

#[tokio::test]
async fn test_processors() {
    let mut systems = HashMap::new();
    systems.insert("vm1".to_string(), make_system_config("VM 1"));
    let mock = MockBackend::new().with_vm("vm1", make_running_vm());
    let state = make_app_state(mock, systems);
    let app = build_app(state);

    let (status, json, _) = get(&app, "/redfish/v1/Systems/vm1/Processors/CPU0").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["TotalCores"], 4);
    assert_eq!(json["TotalThreads"], 8);
}

// ── Storage ───────────────────────────────────────────────────────────

#[tokio::test]
async fn test_simple_storage() {
    let mut systems = HashMap::new();
    systems.insert("vm1".to_string(), make_system_config("VM 1"));
    let mock = MockBackend::new().with_vm("vm1", make_running_vm());
    let state = make_app_state(mock, systems);
    let app = build_app(state);

    let (status, json, _) = get(&app, "/redfish/v1/Systems/vm1/SimpleStorage/1").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["Devices"][0]["Name"], "vda");
    assert_eq!(json["Devices"][0]["CapacityBytes"], 10_000_000_000u64);
}

#[tokio::test]
async fn test_storage_collection() {
    let mut systems = HashMap::new();
    systems.insert("vm1".to_string(), make_system_config("VM 1"));
    let mock = MockBackend::new().with_vm("vm1", make_running_vm());
    let state = make_app_state(mock, systems);
    let app = build_app(state);

    let (status, json, _) = get(&app, "/redfish/v1/Systems/vm1/Storage").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["Members@odata.count"], 1);
}

// ── Memory ────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_memory() {
    let mut systems = HashMap::new();
    systems.insert("vm1".to_string(), make_system_config("VM 1"));
    let mock = MockBackend::new().with_vm("vm1", make_running_vm());
    let state = make_app_state(mock, systems);
    let app = build_app(state);

    let (status, json, _) = get(&app, "/redfish/v1/Systems/vm1/Memory/DIMM0").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["CapacityMiB"], 4096);
    assert_eq!(json["MemoryDeviceType"], "DDR4");
}

// ── Managers ──────────────────────────────────────────────────────────

#[tokio::test]
async fn test_managers() {
    let state = make_app_state(MockBackend::new(), HashMap::new());
    let app = build_app(state);

    let (status, json, _) = get(&app, "/redfish/v1/Managers/vbmc").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["ManagerType"], "BMC");
    assert!(json["LogServices"]["@odata.id"].as_str().is_some());
}

#[tokio::test]
async fn test_manager_not_found() {
    let state = make_app_state(MockBackend::new(), HashMap::new());
    let app = build_app(state);

    let (status, _, _) = get(&app, "/redfish/v1/Managers/nonexistent").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ── Chassis ───────────────────────────────────────────────────────────

#[tokio::test]
async fn test_chassis() {
    let state = make_app_state(MockBackend::new(), HashMap::new());
    let app = build_app(state);

    let (status, json, _) = get(&app, "/redfish/v1/Chassis/1").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["ChassisType"], "RackMount");
    assert!(json["Power"]["@odata.id"].as_str().is_some());
    assert!(json["Thermal"]["@odata.id"].as_str().is_some());
    assert!(json["NetworkAdapters"]["@odata.id"].as_str().is_some());
}

#[tokio::test]
async fn test_chassis_power() {
    let state = make_app_state(MockBackend::new(), HashMap::new());
    let app = build_app(state);

    let (status, json, _) = get(&app, "/redfish/v1/Chassis/1/Power").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["@odata.type"], "#Power.v1_7_2.Power");
    assert!(!json["PowerControl"].as_array().unwrap().is_empty());
    assert!(!json["PowerSupplies"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_chassis_thermal() {
    let state = make_app_state(MockBackend::new(), HashMap::new());
    let app = build_app(state);

    let (status, json, _) = get(&app, "/redfish/v1/Chassis/1/Thermal").await;
    assert_eq!(status, StatusCode::OK);
    assert!(!json["Temperatures"].as_array().unwrap().is_empty());
    assert!(!json["Fans"].as_array().unwrap().is_empty());
}

// ── BIOS ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_bios() {
    let mut systems = HashMap::new();
    systems.insert("vm1".to_string(), make_system_config("VM 1"));
    let state = make_app_state(MockBackend::new(), systems);
    let app = build_app(state);

    let (status, json, _) = get(&app, "/redfish/v1/Systems/vm1/Bios").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["@odata.type"], "#Bios.v1_2_1.Bios");
}

#[tokio::test]
async fn test_bios_settings_patch() {
    let mut systems = HashMap::new();
    systems.insert("vm1".to_string(), make_system_config("VM 1"));
    let state = make_app_state(MockBackend::new(), systems);
    let state_ref = state.clone();
    let app = build_app(state);

    let (status, _) = patch_json(
        &app,
        "/redfish/v1/Systems/vm1/Bios/Settings",
        serde_json::json!({
            "Attributes": {
                "BootOrder": "Cd,Hdd,Pxe",
                "SecureBootMode": "DeployedMode"
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // Verify persisted
    let vm_state = state_ref.get_vm_state("vm1");
    let bios = vm_state.bios_settings.unwrap();
    assert_eq!(bios.boot_order.as_deref(), Some("Cd,Hdd,Pxe"));
    assert_eq!(bios.secure_boot_mode.as_deref(), Some("DeployedMode"));
}

// ── UpdateService ─────────────────────────────────────────────────────

#[tokio::test]
async fn test_update_service() {
    let state = make_app_state(MockBackend::new(), HashMap::new());
    let app = build_app(state);

    let (status, json, _) = get(&app, "/redfish/v1/UpdateService").await;
    assert_eq!(status, StatusCode::OK);
    assert!(json["ServiceEnabled"].as_bool().unwrap());

    let (status, json, _) = get(&app, "/redfish/v1/UpdateService/FirmwareInventory/vbmc-rs").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["Id"], "vbmc-rs");
}

// ── LicenseService ────────────────────────────────────────────────────

#[tokio::test]
async fn test_license_service() {
    let state = make_app_state(MockBackend::new(), HashMap::new());
    let app = build_app(state);

    let (status, json, _) = get(&app, "/redfish/v1/LicenseService").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["@odata.type"], "#LicenseService.v1_1_0.LicenseService");
}

// ── LogServices ───────────────────────────────────────────────────────

#[tokio::test]
async fn test_system_log_services() {
    let mut systems = HashMap::new();
    systems.insert("vm1".to_string(), make_system_config("VM 1"));
    let state = make_app_state(MockBackend::new(), systems);
    let app = build_app(state);

    let (status, json, _) = get(&app, "/redfish/v1/Systems/vm1/LogServices").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["Members@odata.count"], 1);
}

#[tokio::test]
async fn test_manager_log_services() {
    let state = make_app_state(MockBackend::new(), HashMap::new());
    let app = build_app(state);

    let (status, json, _) = get(&app, "/redfish/v1/Managers/vbmc/LogServices").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["Members@odata.count"], 1);
}

// ── 404 on unknown routes ─────────────────────────────────────────────

#[tokio::test]
async fn test_unknown_route() {
    let state = make_app_state(MockBackend::new(), HashMap::new());
    let app = build_app(state);

    let req = Request::builder()
        .method("GET")
        .uri("/redfish/v1/NonExistent")
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// ── PCIe Devices ──────────────────────────────────────────────────────

#[tokio::test]
async fn test_pcie_devices_empty() {
    let mut systems = HashMap::new();
    systems.insert("vm1".to_string(), make_system_config("VM 1"));
    let mock = MockBackend::new().with_vm("vm1", make_running_vm());
    let state = make_app_state(mock, systems);
    let app = build_app(state);

    let (status, json, _) = get(&app, "/redfish/v1/Systems/vm1/PCIeDevices").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["Members@odata.count"], 0);
}

// ── DMTF Validator compliance ─────────────────────────────────────────

#[tokio::test]
async fn test_odata_context_on_individual_resource() {
    let mut systems = HashMap::new();
    systems.insert("vm1".to_string(), make_system_config("VM 1"));
    let mock = MockBackend::new().with_vm("vm1", make_running_vm());
    let state = make_app_state(mock, systems);
    let app = build_app(state);

    // ComputerSystem should have @odata.context derived from @odata.type
    let (status, json, _) = get(&app, "/redfish/v1/Systems/vm1").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        json["@odata.context"],
        "/redfish/v1/$metadata#ComputerSystem.ComputerSystem"
    );
}

#[tokio::test]
async fn test_odata_context_on_service_root() {
    let state = make_app_state(MockBackend::new(), HashMap::new());
    let app = build_app(state);

    let (_, json, _) = get(&app, "/redfish/v1").await;
    assert_eq!(
        json["@odata.context"],
        "/redfish/v1/$metadata#ServiceRoot.ServiceRoot"
    );
}

#[tokio::test]
async fn test_odata_context_on_manager() {
    let state = make_app_state(MockBackend::new(), HashMap::new());
    let app = build_app(state);

    let (_, json, _) = get(&app, "/redfish/v1/Managers/vbmc").await;
    assert_eq!(
        json["@odata.context"],
        "/redfish/v1/$metadata#Manager.Manager"
    );
}

#[tokio::test]
async fn test_odata_context_on_chassis() {
    let state = make_app_state(MockBackend::new(), HashMap::new());
    let app = build_app(state);

    let (_, json, _) = get(&app, "/redfish/v1/Chassis/1").await;
    assert_eq!(
        json["@odata.context"],
        "/redfish/v1/$metadata#Chassis.Chassis"
    );
}

#[tokio::test]
async fn test_odata_context_on_collection() {
    let state = make_app_state(MockBackend::new(), HashMap::new());
    let app = build_app(state);

    // Collection has @odata.context derived from odata_type by Collection::new
    let (_, json, _) = get(&app, "/redfish/v1/Systems").await;
    assert_eq!(
        json["@odata.context"],
        "/redfish/v1/$metadata#ComputerSystemCollection.ComputerSystemCollection"
    );
}

#[tokio::test]
async fn test_odata_context_on_processor() {
    let mut systems = HashMap::new();
    systems.insert("vm1".to_string(), make_system_config("VM 1"));
    let mock = MockBackend::new().with_vm("vm1", make_running_vm());
    let state = make_app_state(mock, systems);
    let app = build_app(state);

    let (_, json, _) = get(&app, "/redfish/v1/Systems/vm1/Processors/CPU0").await;
    assert_eq!(
        json["@odata.context"],
        "/redfish/v1/$metadata#Processor.Processor"
    );
}

#[tokio::test]
async fn test_service_root_uuid_stable() {
    let state = make_app_state(MockBackend::new(), HashMap::new());
    let app = build_app(state);

    let (_, json1, _) = get(&app, "/redfish/v1").await;
    let (_, json2, _) = get(&app, "/redfish/v1").await;

    let uuid1 = json1["UUID"].as_str().unwrap();
    let uuid2 = json2["UUID"].as_str().unwrap();
    assert_eq!(uuid1, uuid2);
    assert!(!uuid1.is_empty());
}

#[tokio::test]
async fn test_method_not_allowed() {
    let state = make_app_state(MockBackend::new(), HashMap::new());
    let app = build_app(state);

    // POST to a GET-only endpoint should return 405
    let req = Request::builder()
        .method("DELETE")
        .uri("/redfish/v1/Systems")
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
}

// ── ComponentIntegrity ────────────────────────────────────────────────

#[tokio::test]
async fn test_component_integrity_minimal_spdm() {
    let mut systems = HashMap::new();
    systems.insert("vm1".to_string(), make_system_config("VM 1"));
    let state = make_app_state(MockBackend::new(), systems);
    let app = build_app(state);

    let (status, json, _) = get(&app, "/redfish/v1/ComponentIntegrity/vm1").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["ComponentIntegrityType"], "SPDM");
    assert_eq!(json["SPDM"]["Requester"]["@odata.id"], "/redfish/v1/ComponentIntegrity/vm1");
    // No evidence → no MeasurementSet or IdentityAuthentication
    assert!(json["SPDM"]["MeasurementSet"].is_null());
    assert!(json["SPDM"]["IdentityAuthentication"].is_null());
}

#[tokio::test]
async fn test_component_integrity_with_evidence() {
    use crate::attestation::trust_chain::{AttestationEvidence, MeasurementEntry, VerificationStatus};

    let mut systems = HashMap::new();
    systems.insert("vm1".to_string(), make_system_config("VM 1"));
    let state = make_app_state(MockBackend::new(), systems);

    // Set evidence on the VM state
    let mut vm_state = state.get_vm_state("vm1");
    vm_state.attestation.verification_status = Some("Success".to_string());
    vm_state.attestation.evidence = Some(AttestationEvidence {
        measurements: vec![MeasurementEntry {
            index: 0,
            measurement_type: "ImmutableROM".to_string(),
            measurement: "dGVzdA==".to_string(),
            hash_algorithm: "SHA-256".to_string(),
            part_of_summary: true,
            last_updated: Some("2026-01-15T10:00:00Z".to_string()),
        }],
        measurement_summary: Some("summary123".to_string()),
        measurement_summary_algorithm: Some("SHA-256".to_string()),
        measurement_summary_type: Some("TCB".to_string()),
        responder_verification: Some(VerificationStatus::Success),
        provider: Some("keylime".to_string()),
    });
    state.vm_states.insert("vm1".to_string(), vm_state);

    let app = build_app(state);
    let (status, json, _) = get(&app, "/redfish/v1/ComponentIntegrity/vm1").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["Status"]["Health"], "OK");

    // Full SPDM structure present
    let spdm = &json["SPDM"];
    assert_eq!(spdm["Requester"]["@odata.id"], "/redfish/v1/ComponentIntegrity/vm1");

    let mset = &spdm["MeasurementSet"];
    assert_eq!(mset["MeasurementSpecification"], "DMTF");
    assert_eq!(mset["MeasurementSummary"], "summary123");
    assert_eq!(mset["MeasurementSummaryHashAlgorithm"], "SHA-256");
    assert_eq!(mset["MeasurementSummaryType"], "TCB");

    let m0 = &mset["Measurements"][0];
    assert_eq!(m0["MeasurementIndex"], 0);
    assert_eq!(m0["MeasurementType"], "ImmutableROM");
    assert_eq!(m0["Measurement"], "dGVzdA==");
    assert_eq!(m0["MeasurementHashAlgorithm"], "SHA-256");
    assert_eq!(m0["PartofSummaryHash"], true);
    assert_eq!(m0["LastUpdated"], "2026-01-15T10:00:00Z");

    let identity = &spdm["IdentityAuthentication"];
    assert_eq!(identity["ResponderAuthentication"]["VerificationStatus"], "Success");
}
