use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use serde::{Deserialize, Serialize};

use super::error::RedfishApiError;
use crate::app_state::AppState;
use crate::auth::AuthenticatedUser;
use crate::auth::rbac::{Privilege, has_privilege};
use crate::backend::VmmBackend;

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

    let current_boot = match state.backend.vm_info(&system_id).await {
        Ok(info) => match info.secure_boot {
            Some(true) => "Enabled",
            Some(false) => "Disabled",
            None => {
                if vm_state.secure_boot_enabled {
                    "Enabled"
                } else {
                    "Disabled"
                }
            }
        },
        Err(_) => {
            if vm_state.secure_boot_enabled {
                "Enabled"
            } else {
                "Disabled"
            }
        }
    };

    Ok(Json(SecureBootResource {
        odata_id: format!("/redfish/v1/Systems/{system_id}/SecureBoot"),
        odata_type: "#SecureBoot.v1_1_0.SecureBoot",
        id: "SecureBoot",
        name: "UEFI Secure Boot",
        description: "UEFI Secure Boot settings",
        secure_boot_enable: vm_state.secure_boot_enabled,
        secure_boot_current_boot: current_boot,
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
        // Stage the change in the backend (takes effect on next boot)
        if let Err(e) = state.backend.vm_set_secure_boot(&system_id, enabled).await {
            tracing::warn!("Backend could not stage secure boot change for '{system_id}': {e}");
        }

        let mut vm_state = state.get_vm_state(&system_id);
        vm_state.secure_boot_enabled = enabled;
        state.save_vm_state(&system_id, &vm_state);
    }

    Ok(Json(serde_json::json!({"message": "SecureBoot updated"})))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secure_boot_resource_serialization() {
        let resource = SecureBootResource {
            odata_id: "/redfish/v1/Systems/vm1/SecureBoot".to_string(),
            odata_type: "#SecureBoot.v1_1_0.SecureBoot",
            id: "SecureBoot",
            name: "UEFI Secure Boot",
            description: "UEFI Secure Boot settings",
            secure_boot_enable: true,
            secure_boot_current_boot: "Enabled",
            secure_boot_mode: "UserMode",
        };

        let value = serde_json::to_value(&resource).unwrap();

        assert_eq!(value["@odata.id"], "/redfish/v1/Systems/vm1/SecureBoot");
        assert_eq!(value["@odata.type"], "#SecureBoot.v1_1_0.SecureBoot");
        assert_eq!(value["Id"], "SecureBoot");
        assert_eq!(value["Name"], "UEFI Secure Boot");
        assert_eq!(value["Description"], "UEFI Secure Boot settings");
        assert_eq!(value["SecureBootEnable"], true);
        assert_eq!(value["SecureBootCurrentBoot"], "Enabled");
        assert_eq!(value["SecureBootMode"], "UserMode");
    }

    #[test]
    fn test_secure_boot_resource_disabled() {
        let resource = SecureBootResource {
            odata_id: "/redfish/v1/Systems/vm1/SecureBoot".to_string(),
            odata_type: "#SecureBoot.v1_1_0.SecureBoot",
            id: "SecureBoot",
            name: "UEFI Secure Boot",
            description: "UEFI Secure Boot settings",
            secure_boot_enable: false,
            secure_boot_current_boot: "Disabled",
            secure_boot_mode: "UserMode",
        };

        let value = serde_json::to_value(&resource).unwrap();
        assert_eq!(value["SecureBootEnable"], false);
        assert_eq!(value["SecureBootCurrentBoot"], "Disabled");
    }

    #[test]
    fn test_patch_secure_boot_request_deserialization() {
        let json = serde_json::json!({
            "SecureBootEnable": true
        });

        let request: PatchSecureBootRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.secure_boot_enable, Some(true));
    }

    #[test]
    fn test_patch_secure_boot_request_false() {
        let json = serde_json::json!({
            "SecureBootEnable": false
        });

        let request: PatchSecureBootRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.secure_boot_enable, Some(false));
    }

    #[test]
    fn test_patch_secure_boot_request_none() {
        let json = serde_json::json!({});
        let request: PatchSecureBootRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.secure_boot_enable, None);
    }

    #[test]
    fn test_secure_boot_current_boot_values() {
        let enabled = SecureBootResource {
            odata_id: "/redfish/v1/Systems/vm1/SecureBoot".to_string(),
            odata_type: "#SecureBoot.v1_1_0.SecureBoot",
            id: "SecureBoot",
            name: "UEFI Secure Boot",
            description: "UEFI Secure Boot settings",
            secure_boot_enable: true,
            secure_boot_current_boot: "Enabled",
            secure_boot_mode: "UserMode",
        };

        let value = serde_json::to_value(&enabled).unwrap();
        assert_eq!(value["SecureBootCurrentBoot"], "Enabled");

        let disabled = SecureBootResource {
            secure_boot_current_boot: "Disabled",
            ..enabled
        };

        let value = serde_json::to_value(&disabled).unwrap();
        assert_eq!(value["SecureBootCurrentBoot"], "Disabled");
    }
}

#[cfg(test)]
mod harness_tests {
    use crate::backend::mock::MockBackend;
    use crate::redfish::test_harness as h;
    use axum::http::{Method, StatusCode};

    #[tokio::test]
    async fn test_get_secure_boot() {
        let mock = MockBackend::new().with_vm("sys", h::running_vm());
        let state = h::app_state(mock, h::systems_with("sys"));
        let app = h::router(state);

        let (status, json, _) = h::get(&app, "/redfish/v1/Systems/sys/SecureBoot").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["@odata.id"], "/redfish/v1/Systems/sys/SecureBoot");
        assert_eq!(json["@odata.type"], "#SecureBoot.v1_1_0.SecureBoot");
        assert_eq!(json["Id"], "SecureBoot");
        assert_eq!(json["Name"], "UEFI Secure Boot");
        assert_eq!(json["SecureBootMode"], "UserMode");
        assert!(json["SecureBootEnable"].is_boolean());
        assert!(json["SecureBootCurrentBoot"].is_string());
    }

    #[tokio::test]
    async fn test_get_secure_boot_unknown_system() {
        let state = h::app_state(MockBackend::new(), h::systems_with("sys"));
        let app = h::router(state);

        let (status, json, _) = h::get(&app, "/redfish/v1/Systems/unknown/SecureBoot").await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(json["error"]["code"], "Base.1.0.GeneralError");
        assert!(
            json["error"]["message"]
                .as_str()
                .unwrap()
                .contains("unknown")
        );
    }

    #[tokio::test]
    async fn test_patch_secure_boot_enable() {
        let mock = MockBackend::new().with_vm("sys", h::running_vm());
        let state = h::app_state(mock, h::systems_with("sys"));
        let app = h::router(state.clone());

        let body = serde_json::json!({
            "SecureBootEnable": true
        });

        let (status, json, _) = h::request_json(
            &app,
            Method::PATCH,
            "/redfish/v1/Systems/sys/SecureBoot",
            body,
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert!(json["message"].as_str().is_some());

        // Verify the state was updated
        let vm_state = state.get_vm_state("sys");
        assert!(vm_state.secure_boot_enabled);
    }

    #[tokio::test]
    async fn test_patch_secure_boot_disable() {
        let mock = MockBackend::new().with_vm("sys", h::running_vm());
        let state = h::app_state(mock, h::systems_with("sys"));
        let app = h::router(state.clone());

        let body = serde_json::json!({
            "SecureBootEnable": false
        });

        let (status, json, _) = h::request_json(
            &app,
            Method::PATCH,
            "/redfish/v1/Systems/sys/SecureBoot",
            body,
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert!(json["message"].as_str().is_some());

        // Verify the state was updated
        let vm_state = state.get_vm_state("sys");
        assert!(!vm_state.secure_boot_enabled);
    }

    #[tokio::test]
    async fn test_patch_secure_boot_unknown_system() {
        let state = h::app_state(MockBackend::new(), h::systems_with("sys"));
        let app = h::router(state);

        let body = serde_json::json!({
            "SecureBootEnable": true
        });

        let (status, json, _) = h::request_json(
            &app,
            Method::PATCH,
            "/redfish/v1/Systems/unknown/SecureBoot",
            body,
        )
        .await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(json["error"]["code"], "Base.1.0.GeneralError");
        assert!(
            json["error"]["message"]
                .as_str()
                .unwrap()
                .contains("unknown")
        );
    }

    #[tokio::test]
    async fn test_patch_secure_boot_empty_body() {
        let mock = MockBackend::new().with_vm("sys", h::running_vm());
        let state = h::app_state(mock, h::systems_with("sys"));
        let app = h::router(state);

        let body = serde_json::json!({});

        let (status, json, _) = h::request_json(
            &app,
            Method::PATCH,
            "/redfish/v1/Systems/sys/SecureBoot",
            body,
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert!(json["message"].as_str().is_some());
    }
}
