use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use serde::Serialize;

use super::error::RedfishApiError;
use super::types::{Collection, ODataId, Status};
use crate::app_state::AppState;

#[derive(Debug, Serialize)]
pub struct ComponentIntegrityResource {
    #[serde(rename = "@odata.id")]
    pub odata_id: String,
    #[serde(rename = "@odata.type")]
    pub odata_type: &'static str,
    #[serde(rename = "Id")]
    pub id: String,
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "ComponentIntegrityType")]
    pub component_integrity_type: &'static str,
    #[serde(rename = "ComponentIntegrityTypeVersion")]
    pub component_integrity_type_version: &'static str,
    #[serde(rename = "Status")]
    pub status: Status,
    #[serde(rename = "SPDMinfo", skip_serializing_if = "Option::is_none")]
    pub spdm_info: Option<SpdmInfo>,
}

#[derive(Debug, Serialize)]
pub struct SpdmInfo {
    #[serde(rename = "VerificationStatus")]
    pub verification_status: String,
}

pub async fn get_component_integrity_collection(
    State(state): State<Arc<AppState>>,
) -> Json<Collection<ODataId>> {
    let members: Vec<ODataId> = state
        .config
        .systems
        .keys()
        .map(|id| ODataId::new(format!("/redfish/v1/ComponentIntegrity/{id}")))
        .collect();

    Json(Collection::new(
        "/redfish/v1/ComponentIntegrity",
        "#ComponentIntegrityCollection.ComponentIntegrityCollection",
        "Component Integrity Collection",
        members,
    ))
}

pub async fn get_component_integrity(
    State(state): State<Arc<AppState>>,
    Path(system_id): Path<String>,
) -> Result<Json<ComponentIntegrityResource>, RedfishApiError> {
    if !state.config.systems.contains_key(&system_id) {
        return Err(RedfishApiError::NotFound(format!(
            "ComponentIntegrity '{system_id}' not found"
        )));
    }

    let vm_state = state.get_vm_state(&system_id);
    let verification_status = vm_state
        .attestation
        .verification_status
        .unwrap_or_else(|| "Unknown".to_string());

    let health = match verification_status.as_str() {
        "Success" => "OK",
        "Failed" => "Critical",
        _ => "Warning",
    };

    Ok(Json(ComponentIntegrityResource {
        odata_id: format!("/redfish/v1/ComponentIntegrity/{system_id}"),
        odata_type: "#ComponentIntegrity.v1_2_0.ComponentIntegrity",
        id: system_id.clone(),
        name: format!("Integrity: {system_id}"),
        component_integrity_type: "SPDM",
        component_integrity_type_version: "1.0",
        status: Status {
            state: Some("Enabled".to_string()),
            health: Some(health.to_string()),
            health_rollup: Some(health.to_string()),
        },
        spdm_info: Some(SpdmInfo {
            verification_status,
        }),
    }))
}
