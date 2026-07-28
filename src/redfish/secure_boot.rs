use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use serde::{Deserialize, Serialize};

use super::error::RedfishApiError;
use crate::app_state::AppState;
use crate::auth::AuthenticatedUser;
use crate::auth::rbac::{Privilege, has_privilege};

#[derive(Debug, Serialize)]
pub struct SecureBootResource {
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
    #[serde(rename = "SecureBootEnable")]
    pub secure_boot_enable: bool,
    #[serde(rename = "SecureBootCurrentBoot")]
    pub secure_boot_current_boot: &'static str,
    #[serde(rename = "SecureBootMode")]
    pub secure_boot_mode: &'static str,
}

pub async fn get_secure_boot(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
    Path(system_id): Path<String>,
) -> Result<Json<SecureBootResource>, RedfishApiError> {
    if !state.config.systems.contains_key(&system_id) {
        return Err(RedfishApiError::NotFound(format!(
            "System '{system_id}' not found"
        )));
    }

    let vm_state = state.get_vm_state(&system_id);

    Ok(Json(SecureBootResource {
        odata_id: format!("/redfish/v1/Systems/{system_id}/SecureBoot"),
        odata_type: "#SecureBoot.v1_1_0.SecureBoot",
        id: "SecureBoot",
        name: "UEFI Secure Boot",
        description: "UEFI Secure Boot settings",
        secure_boot_enable: vm_state.secure_boot_enabled,
        secure_boot_current_boot: if vm_state.secure_boot_enabled {
            "Enabled"
        } else {
            "Disabled"
        },
        secure_boot_mode: "UserMode",
    }))
}

#[derive(Debug, Deserialize)]
pub struct PatchSecureBootRequest {
    #[serde(rename = "SecureBootEnable")]
    pub secure_boot_enable: Option<bool>,
}

pub async fn patch_secure_boot(
    State(state): State<Arc<AppState>>,
    user: AuthenticatedUser,
    Path(system_id): Path<String>,
    Json(body): Json<PatchSecureBootRequest>,
) -> Result<Json<serde_json::Value>, RedfishApiError> {
    if !has_privilege(&user.role, Privilege::ConfigureComponents) {
        return Err(RedfishApiError::Forbidden(
            "Insufficient privileges".to_string(),
        ));
    }

    if !state.config.systems.contains_key(&system_id) {
        return Err(RedfishApiError::NotFound(format!(
            "System '{system_id}' not found"
        )));
    }

    if let Some(enabled) = body.secure_boot_enable {
        let mut vm_state = state.get_vm_state(&system_id);
        vm_state.secure_boot_enabled = enabled;
        state.save_vm_state(&system_id, &vm_state);
    }

    Ok(Json(serde_json::json!({"message": "SecureBoot updated"})))
}
