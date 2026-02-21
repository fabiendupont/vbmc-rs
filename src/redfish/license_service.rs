use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use serde::{Deserialize, Serialize};

use super::error::RedfishApiError;
use super::types::{Collection, ODataId, Status};
use crate::app_state::AppState;

#[derive(Debug, Serialize)]
pub struct LicenseServiceResource {
    #[serde(rename = "@odata.id")]
    pub odata_id: &'static str,
    #[serde(rename = "@odata.type")]
    pub odata_type: &'static str,
    #[serde(rename = "Id")]
    pub id: &'static str,
    #[serde(rename = "Name")]
    pub name: &'static str,
    #[serde(rename = "Licenses")]
    pub licenses: ODataId,
    #[serde(rename = "Status")]
    pub status: Status,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseResource {
    #[serde(rename = "@odata.id")]
    pub odata_id: String,
    #[serde(rename = "@odata.type")]
    pub odata_type: &'static str,
    #[serde(rename = "Id")]
    pub id: String,
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "LicenseType")]
    pub license_type: String,
    #[serde(rename = "Status")]
    pub status: Status,
}

pub async fn get_license_service() -> Json<LicenseServiceResource> {
    Json(LicenseServiceResource {
        odata_id: "/redfish/v1/LicenseService",
        odata_type: "#LicenseService.v1_1_0.LicenseService",
        id: "LicenseService",
        name: "License Service",
        licenses: ODataId::new("/redfish/v1/LicenseService/Licenses"),
        status: Status::enabled_ok(),
    })
}

pub async fn get_licenses(
    State(state): State<Arc<AppState>>,
) -> Json<Collection<ODataId>> {
    // Collect licenses from all system states
    let mut members = Vec::new();
    for entry in state.vm_states.iter() {
        let vm_state = entry.value();
        for lic in &vm_state.licenses {
            members.push(ODataId::new(format!(
                "/redfish/v1/LicenseService/Licenses/{}",
                lic.id
            )));
        }
    }

    Json(Collection::new(
        "/redfish/v1/LicenseService/Licenses",
        "#LicenseCollection.LicenseCollection",
        "License Collection",
        members,
    ))
}

#[derive(Debug, Deserialize)]
pub struct CreateLicenseRequest {
    #[serde(rename = "LicenseString")]
    pub license_string: String,
    #[serde(rename = "Name", default)]
    pub name: Option<String>,
}

pub async fn create_license(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateLicenseRequest>,
) -> Result<Json<serde_json::Value>, RedfishApiError> {
    let id = uuid::Uuid::new_v4().to_string();
    let name = body.name.unwrap_or_else(|| format!("License {}", &id[..8]));

    let license = crate::state::LicenseInfo {
        id: id.clone(),
        name: name.clone(),
        license_type: "Production".to_string(),
        license_string: body.license_string,
    };

    // Store in the first system's state (licenses are global)
    if let Some(first_system) = state.config.systems.keys().next() {
        let system_id = first_system.clone();
        let mut vm_state = state.get_vm_state(&system_id);
        vm_state.licenses.push(license);
        state.save_vm_state(&system_id, &vm_state);
    }

    Ok(Json(serde_json::json!({
        "@odata.id": format!("/redfish/v1/LicenseService/Licenses/{id}"),
        "Id": id,
        "Name": name,
        "message": "License created"
    })))
}

pub async fn get_license(
    State(state): State<Arc<AppState>>,
    Path(license_id): Path<String>,
) -> Result<Json<LicenseResource>, RedfishApiError> {
    for entry in state.vm_states.iter() {
        let vm_state = entry.value();
        if let Some(lic) = vm_state.licenses.iter().find(|l| l.id == license_id) {
            return Ok(Json(LicenseResource {
                odata_id: format!("/redfish/v1/LicenseService/Licenses/{license_id}"),
                odata_type: "#License.v1_1_1.License",
                id: license_id,
                name: lic.name.clone(),
                license_type: lic.license_type.clone(),
                status: Status::enabled_ok(),
            }));
        }
    }

    Err(RedfishApiError::NotFound(format!(
        "License '{license_id}' not found"
    )))
}
