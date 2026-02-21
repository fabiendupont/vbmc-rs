use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use serde::Serialize;

use super::types::ODataId;
use crate::app_state::AppState;

#[derive(Debug, Serialize)]
pub struct ServiceRoot {
    #[serde(rename = "@odata.id")]
    pub odata_id: &'static str,
    #[serde(rename = "@odata.type")]
    pub odata_type: &'static str,
    #[serde(rename = "Id")]
    pub id: &'static str,
    #[serde(rename = "Name")]
    pub name: &'static str,
    #[serde(rename = "RedfishVersion")]
    pub redfish_version: &'static str,
    #[serde(rename = "UUID")]
    pub uuid: String,
    #[serde(rename = "Systems")]
    pub systems: ODataId,
    #[serde(rename = "Managers")]
    pub managers: ODataId,
    #[serde(rename = "SessionService", skip_serializing_if = "Option::is_none")]
    pub session_service: Option<ODataId>,
    #[serde(rename = "AccountService", skip_serializing_if = "Option::is_none")]
    pub account_service: Option<ODataId>,
    #[serde(rename = "EventService", skip_serializing_if = "Option::is_none")]
    pub event_service: Option<ODataId>,
    #[serde(rename = "TaskService", skip_serializing_if = "Option::is_none")]
    pub task_service: Option<ODataId>,
    #[serde(rename = "TelemetryService", skip_serializing_if = "Option::is_none")]
    pub telemetry_service: Option<ODataId>,
    #[serde(rename = "CertificateService", skip_serializing_if = "Option::is_none")]
    pub certificate_service: Option<ODataId>,
    #[serde(rename = "Chassis", skip_serializing_if = "Option::is_none")]
    pub chassis: Option<ODataId>,
    #[serde(rename = "ComponentIntegrity", skip_serializing_if = "Option::is_none")]
    pub component_integrity: Option<ODataId>,
    #[serde(rename = "UpdateService", skip_serializing_if = "Option::is_none")]
    pub update_service: Option<ODataId>,
    #[serde(rename = "LicenseService", skip_serializing_if = "Option::is_none")]
    pub license_service: Option<ODataId>,
}

pub async fn get_service_root(
    State(state): State<Arc<AppState>>,
) -> Json<ServiceRoot> {
    Json(ServiceRoot {
        odata_id: "/redfish/v1",
        odata_type: "#ServiceRoot.v1_16_0.ServiceRoot",
        id: "RootService",
        name: "vbmc-rs Redfish Service",
        redfish_version: "1.21.0",
        uuid: state.instance_uuid.clone(),
        systems: ODataId::new("/redfish/v1/Systems"),
        managers: ODataId::new("/redfish/v1/Managers"),
        session_service: Some(ODataId::new("/redfish/v1/SessionService")),
        account_service: Some(ODataId::new("/redfish/v1/AccountService")),
        event_service: Some(ODataId::new("/redfish/v1/EventService")),
        task_service: Some(ODataId::new("/redfish/v1/TaskService")),
        telemetry_service: Some(ODataId::new("/redfish/v1/TelemetryService")),
        certificate_service: Some(ODataId::new("/redfish/v1/CertificateService")),
        chassis: Some(ODataId::new("/redfish/v1/Chassis")),
        component_integrity: Some(ODataId::new("/redfish/v1/ComponentIntegrity")),
        update_service: Some(ODataId::new("/redfish/v1/UpdateService")),
        license_service: Some(ODataId::new("/redfish/v1/LicenseService")),
    })
}

pub async fn get_redfish_root() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "v1": "/redfish/v1/"
    }))
}
