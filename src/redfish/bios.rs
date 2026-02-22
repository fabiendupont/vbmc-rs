use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use serde::{Deserialize, Serialize};

use super::error::RedfishApiError;
use crate::app_state::AppState;

#[derive(Debug, Serialize)]
pub struct BiosResource {
    #[serde(rename = "@odata.id")]
    pub odata_id: String,
    #[serde(rename = "@odata.type")]
    pub odata_type: &'static str,
    #[serde(rename = "Id")]
    pub id: &'static str,
    #[serde(rename = "Name")]
    pub name: &'static str,
    #[serde(rename = "Description")]
    pub description: &'static str,
    #[serde(rename = "Attributes")]
    pub attributes: BiosAttributes,
    #[serde(rename = "@Redfish.Settings")]
    pub settings: SettingsObject,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BiosAttributes {
    #[serde(rename = "BootOrder", skip_serializing_if = "Option::is_none")]
    pub boot_order: Option<String>,
    #[serde(rename = "SecureBootMode", skip_serializing_if = "Option::is_none")]
    pub secure_boot_mode: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SettingsObject {
    #[serde(rename = "SettingsObject")]
    pub settings_object: super::types::ODataId,
}

pub async fn get_bios(
    State(state): State<Arc<AppState>>,
    Path(system_id): Path<String>,
) -> Result<Json<BiosResource>, RedfishApiError> {
    if !state.config.systems.contains_key(&system_id) {
        return Err(RedfishApiError::NotFound(format!(
            "System '{system_id}' not found"
        )));
    }

    let vm_state = state.get_vm_state(&system_id);
    let attrs = vm_state
        .bios_settings
        .clone()
        .unwrap_or_default();

    Ok(Json(BiosResource {
        odata_id: format!("/redfish/v1/Systems/{system_id}/Bios"),
        odata_type: "#Bios.v1_2_1.Bios",
        id: "Bios",
        name: "BIOS Configuration",
        description: "BIOS configuration",
        attributes: attrs,
        settings: SettingsObject {
            settings_object: super::types::ODataId::new(format!(
                "/redfish/v1/Systems/{system_id}/Bios/Settings"
            )),
        },
    }))
}

pub async fn get_bios_settings(
    State(state): State<Arc<AppState>>,
    Path(system_id): Path<String>,
) -> Result<Json<serde_json::Value>, RedfishApiError> {
    if !state.config.systems.contains_key(&system_id) {
        return Err(RedfishApiError::NotFound(format!(
            "System '{system_id}' not found"
        )));
    }

    let vm_state = state.get_vm_state(&system_id);
    let attrs = vm_state
        .bios_settings
        .clone()
        .unwrap_or_default();

    Ok(Json(serde_json::json!({
        "@odata.id": format!("/redfish/v1/Systems/{system_id}/Bios/Settings"),
        "@odata.type": "#Bios.v1_2_1.Bios",
        "Id": "Settings",
        "Name": "BIOS Pending Settings",
        "Attributes": attrs,
    })))
}

#[derive(Debug, Deserialize)]
pub struct PatchBiosSettingsRequest {
    #[serde(rename = "Attributes")]
    pub attributes: Option<BiosAttributes>,
}

pub async fn patch_bios_settings(
    State(state): State<Arc<AppState>>,
    Path(system_id): Path<String>,
    Json(body): Json<PatchBiosSettingsRequest>,
) -> Result<Json<serde_json::Value>, RedfishApiError> {
    if !state.config.systems.contains_key(&system_id) {
        return Err(RedfishApiError::NotFound(format!(
            "System '{system_id}' not found"
        )));
    }

    if let Some(attrs) = body.attributes {
        let mut vm_state = state.get_vm_state(&system_id);
        let mut current = vm_state.bios_settings.clone().unwrap_or_default();
        if let Some(bo) = attrs.boot_order {
            current.boot_order = Some(bo);
        }
        if let Some(sbm) = attrs.secure_boot_mode {
            current.secure_boot_mode = Some(sbm);
        }
        vm_state.bios_settings = Some(current);
        state.save_vm_state(&system_id, &vm_state);
    }

    Ok(Json(serde_json::json!({"message": "BIOS settings updated"})))
}
