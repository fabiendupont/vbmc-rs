//! Shared, lib-internal test harness for Redfish handler integration tests.
//!
//! Wave-1 tests exercised serialization; this harness lets each handler module
//! drive its routes end-to-end (router + `tower::oneshot`) from an inline
//! `#[cfg(test)] mod tests` without editing the shared `integration_tests.rs`.
//!
//! Note the path caveat that broke earlier agents: inside a nested `mod tests`,
//! reach types through `crate::redfish::types::...` (not `super::types::...`).
#![cfg(any(test, feature = "test-support"))]

use std::collections::HashMap;
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Method, Request, StatusCode};
use tower::ServiceExt;

use crate::app_state::AppState;
use crate::auth::accounts::AccountStore;
use crate::backend::Backend;
use crate::backend::mock::MockBackend;
use crate::backend::types::{DiskInfo, DiskMediaType, DiskProtocol, NicInfo, VmInfo, VmPowerState};
use crate::config::{
    AppConfig, AuthConfig, BackendType, DefaultsConfig, HardwareConfig, MetricsConfig,
    SecurityPolicyConfig, ServerConfig, SystemConfig,
};

/// A minimal, mock-backed [`AppConfig`] carrying the given systems.
pub fn test_config(systems: HashMap<String, SystemConfig>) -> AppConfig {
    AppConfig {
        server: ServerConfig {
            bind_address: "127.0.0.1".to_string(),
            port: 8000,
            tls_cert: None,
            tls_key: None,
            tls_client_ca: None,
        },
        backend: BackendType::CloudHypervisor,
        // `#[derive(Default)]` ignores the `#[serde(default = ...)]` field fns, so
        // `AuthConfig::default()` leaves `max_sessions`/`session_timeout_seconds` at
        // 0 — which makes every session creation fail with "Maximum sessions
        // reached". Populate sane values so session handlers can be exercised.
        auth: AuthConfig {
            session_timeout_seconds: 3600,
            max_sessions: 16,
            ..AuthConfig::default()
        },
        defaults: DefaultsConfig::default(),
        security_policy: SecurityPolicyConfig::default(),
        state_directory: std::env::temp_dir().join("vbmc-rs-test"),
        audit_log: Default::default(),
        audit_log_target: Default::default(),
        location: Default::default(),
        snmp_trap: Default::default(),
        mockup_directory: None,
        metrics: MetricsConfig::default(),
        systems,
    }
}

/// A [`SystemConfig`] with only its display name set.
pub fn system_config(name: &str) -> SystemConfig {
    SystemConfig {
        name: Some(name.to_string()),
        socket_path: None,
        firmware_path: None,
        boot_source: None,
        virtual_media_directory: None,
        hardware: HardwareConfig::default(),
        connection_uri: None,
        domain_name: None,
        namespace: None,
        vm_name: None,
        chassis_id: None,
        attestation: None,
        ipmi_socket: None,
    }
}

/// A single-entry systems map, keyed by `id`.
pub fn systems_with(id: &str) -> HashMap<String, SystemConfig> {
    let mut systems = HashMap::new();
    systems.insert(id.to_string(), system_config(id));
    systems
}

/// A powered-on VM with one disk and one NIC, for backend-hit tests.
pub fn running_vm() -> VmInfo {
    VmInfo {
        power_state: VmPowerState::On,
        cpu_count: 4,
        max_cpu_count: 8,
        cpu_topology: None,
        memory_bytes: 4 * 1024 * 1024 * 1024,
        memory_actual_bytes: Some(4 * 1024 * 1024 * 1024),
        secure_boot: None,
        disks: vec![DiskInfo {
            id: "vda".to_string(),
            path: Some("/tmp/disk.qcow2".to_string()),
            capacity_bytes: Some(10_000_000_000),
            readonly: false,
            protocol: DiskProtocol::Virtio,
            media_type: DiskMediaType::Ssd,
        }],
        nics: vec![NicInfo {
            id: "NIC0".to_string(),
            mac_address: Some("52:54:00:12:34:56".to_string()),
            tap: Some("tap0".to_string()),
            speed_mbps: 25000,
        }],
        pci_devices: vec![],
        uuid: None,
        raw: None,
    }
}

/// Build an [`AppState`] from a mock backend and a systems map.
pub fn app_state(mock: MockBackend, systems: HashMap<String, SystemConfig>) -> Arc<AppState> {
    Arc::new(AppState::new(
        test_config(systems),
        Backend::Mock(mock),
        AccountStore::default(),
        None,
        None,
    ))
}

/// Build an [`AppState`] whose account store is pre-seeded with `accounts`
/// (`(username, password, role)`).
///
/// Auth is disabled by default (`AuthConfig::default().enabled == false`), so
/// `AuthenticatedUser`-guarded handlers run as an anonymous Administrator without
/// any header. Seeded accounts are what let credential-checking handlers such as
/// `session_service::create_session` reach their success path.
pub fn app_state_with_accounts(
    mock: MockBackend,
    systems: HashMap<String, SystemConfig>,
    accounts: &[(&str, &str, &str)],
) -> Arc<AppState> {
    let mut store = AccountStore::default();
    for (username, password, role) in accounts {
        store
            .add_account(username, password, role)
            .expect("seed account");
    }
    Arc::new(AppState::new(
        test_config(systems),
        Backend::Mock(mock),
        store,
        None,
        None,
    ))
}

/// Build a `Basic` Authorization header value from credentials.
pub fn basic_auth(username: &str, password: &str) -> String {
    use base64::Engine;
    let encoded =
        base64::engine::general_purpose::STANDARD.encode(format!("{username}:{password}"));
    format!("Basic {encoded}")
}

/// Build the Redfish router directly from an [`AppState`].
pub fn router(state: Arc<AppState>) -> Router {
    crate::redfish::router(state)
}

/// Convenience: router backed by an empty mock and the given systems map.
pub fn router_with_systems(systems: HashMap<String, SystemConfig>) -> Router {
    router(app_state(MockBackend::new(), systems))
}

/// Send a request with no body and return `(status, json, headers)`.
///
/// A non-JSON or empty body decodes to `serde_json::Value::Null`.
pub async fn request(
    app: &Router,
    method: Method,
    uri: &str,
) -> (StatusCode, serde_json::Value, HeaderMap) {
    request_with_body(app, method, uri, Body::empty()).await
}

/// GET helper.
pub async fn get(app: &Router, uri: &str) -> (StatusCode, serde_json::Value, HeaderMap) {
    request(app, Method::GET, uri).await
}

/// Send a JSON body with the given method.
pub async fn request_json(
    app: &Router,
    method: Method,
    uri: &str,
    body: serde_json::Value,
) -> (StatusCode, serde_json::Value, HeaderMap) {
    let req = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    run(app, req).await
}

/// Send a JSON body with an `Authorization` header (see [`basic_auth`]).
pub async fn request_json_auth(
    app: &Router,
    method: Method,
    uri: &str,
    body: serde_json::Value,
    authorization: &str,
) -> (StatusCode, serde_json::Value, HeaderMap) {
    let req = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .header("authorization", authorization)
        .body(Body::from(body.to_string()))
        .unwrap();
    run(app, req).await
}

async fn request_with_body(
    app: &Router,
    method: Method,
    uri: &str,
    body: Body,
) -> (StatusCode, serde_json::Value, HeaderMap) {
    let req = Request::builder()
        .method(method)
        .uri(uri)
        .body(body)
        .unwrap();
    run(app, req).await
}

async fn run(app: &Router, req: Request<Body>) -> (StatusCode, serde_json::Value, HeaderMap) {
    let response = app.clone().oneshot(req).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = axum::body::to_bytes(response.into_body(), 2_000_000)
        .await
        .unwrap();
    let json = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, json, headers)
}
