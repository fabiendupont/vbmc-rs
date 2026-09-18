use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use serde::{Deserialize, Serialize};

use super::error::RedfishApiError;
use crate::app_state::AppState;
use crate::auth::AuthenticatedUser;
use crate::auth::rbac::{Privilege, has_privilege};

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
    #[serde(rename = "AttributeRegistry")]
    pub attribute_registry: &'static str,
    #[serde(rename = "ResetBiosToDefaultsPending")]
    pub reset_bios_to_defaults_pending: bool,
    #[serde(rename = "Links")]
    pub links: BiosLinks,
    #[serde(rename = "Actions")]
    pub actions: BiosActions,
    #[serde(rename = "@Redfish.Settings")]
    pub settings: SettingsObject,
}

#[derive(Debug, Serialize)]
pub struct BiosActions {
    #[serde(rename = "#Bios.ResetBios")]
    pub reset_bios: super::systems::ActionTarget,
}

#[derive(Debug, Serialize)]
pub struct BiosLinks {
    #[serde(rename = "ActiveSoftwareImage")]
    pub active_software_image: super::types::ODataId,
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
    _user: AuthenticatedUser,
    Path(system_id): Path<String>,
) -> Result<Json<BiosResource>, RedfishApiError> {
    if !state.config.systems.contains_key(&system_id) {
        return Err(RedfishApiError::NotFound(format!(
            "System '{system_id}' not found"
        )));
    }

    let vm_state = state.get_vm_state(&system_id);
    let attrs = vm_state.bios_settings.clone().unwrap_or_default();

    Ok(Json(BiosResource {
        odata_id: format!("/redfish/v1/Systems/{system_id}/Bios"),
        odata_type: "#Bios.v1_2_1.Bios",
        id: "Bios",
        name: "BIOS Configuration",
        description: "BIOS configuration",
        attributes: attrs,
        attribute_registry: "BiosAttributeRegistryVbmc.1.0",
        reset_bios_to_defaults_pending: false,
        actions: BiosActions {
            reset_bios: super::systems::ActionTarget {
                target: format!("/redfish/v1/Systems/{system_id}/Bios/Actions/Bios.ResetBios"),
            },
        },
        links: BiosLinks {
            active_software_image: super::types::ODataId::new(
                "/redfish/v1/UpdateService/FirmwareInventory/vbmc-rs",
            ),
        },
        settings: SettingsObject {
            settings_object: super::types::ODataId::new(format!(
                "/redfish/v1/Systems/{system_id}/Bios/Settings"
            )),
        },
    }))
}

pub async fn get_bios_settings(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
    Path(system_id): Path<String>,
) -> Result<Json<serde_json::Value>, RedfishApiError> {
    if !state.config.systems.contains_key(&system_id) {
        return Err(RedfishApiError::NotFound(format!(
            "System '{system_id}' not found"
        )));
    }

    let vm_state = state.get_vm_state(&system_id);
    let attrs = vm_state.bios_settings.clone().unwrap_or_default();

    Ok(Json(serde_json::json!({
        "@odata.id": format!("/redfish/v1/Systems/{system_id}/Bios/Settings"),
        "@odata.type": "#Bios.v1_2_1.Bios",
        "Id": "Settings",
        "Name": "BIOS Pending Settings",
        "Attributes": attrs,
        "AttributeRegistry": "BiosAttributeRegistryVbmc.1.0",
        "ResetBiosToDefaultsPending": false,
        "Links": {
            "ActiveSoftwareImage": {
                "@odata.id": "/redfish/v1/UpdateService/FirmwareInventory/vbmc-rs"
            }
        },
    })))
}

#[derive(Debug, Deserialize)]
pub struct PatchBiosSettingsRequest {
    #[serde(rename = "Attributes")]
    pub attributes: Option<BiosAttributes>,
}

pub async fn patch_bios_settings(
    State(state): State<Arc<AppState>>,
    user: AuthenticatedUser,
    Path(system_id): Path<String>,
    Json(body): Json<PatchBiosSettingsRequest>,
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

    Ok(Json(
        serde_json::json!({"message": "BIOS settings updated"}),
    ))
}

pub async fn reset_bios(
    State(state): State<Arc<AppState>>,
    user: AuthenticatedUser,
    Path(system_id): Path<String>,
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

    let mut vm_state = state.get_vm_state(&system_id);
    vm_state.bios_settings = Some(BiosAttributes::default());
    state.save_vm_state(&system_id, &vm_state);

    Ok(Json(
        serde_json::json!({"message": "BIOS settings reset to defaults"}),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_bios_resource_serialization() {
        let resource = BiosResource {
            odata_id: "/redfish/v1/Systems/vm1/Bios".to_string(),
            odata_type: "#Bios.v1_2_1.Bios",
            id: "Bios",
            name: "BIOS Configuration",
            description: "BIOS configuration",
            attributes: BiosAttributes::default(),
            attribute_registry: "BiosAttributeRegistryVbmc.1.0",
            reset_bios_to_defaults_pending: false,
            links: BiosLinks {
                active_software_image: super::super::types::ODataId::new(
                    "/redfish/v1/UpdateService/FirmwareInventory/vbmc-rs",
                ),
            },
            actions: BiosActions {
                reset_bios: super::super::systems::ActionTarget {
                    target: "/redfish/v1/Systems/vm1/Bios/Actions/Bios.ResetBios".to_string(),
                },
            },
            settings: SettingsObject {
                settings_object: super::super::types::ODataId::new(
                    "/redfish/v1/Systems/vm1/Bios/Settings",
                ),
            },
        };

        let value = serde_json::to_value(&resource).unwrap();

        assert_eq!(value["@odata.id"], "/redfish/v1/Systems/vm1/Bios");
        assert_eq!(value["@odata.type"], "#Bios.v1_2_1.Bios");
        assert_eq!(value["Id"], "Bios");
        assert_eq!(value["Name"], "BIOS Configuration");
        assert_eq!(value["Description"], "BIOS configuration");
        assert_eq!(value["AttributeRegistry"], "BiosAttributeRegistryVbmc.1.0");
        assert_eq!(value["ResetBiosToDefaultsPending"], false);
        assert!(value["Attributes"].is_object());
        assert!(value["Links"].is_object());
        assert!(value["Actions"].is_object());
        assert!(value["@Redfish.Settings"].is_object());
    }

    #[test]
    fn test_bios_attributes_skip_serializing_none() {
        let attrs = BiosAttributes {
            boot_order: None,
            secure_boot_mode: None,
        };

        let value = serde_json::to_value(&attrs).unwrap();
        assert!(!value.as_object().unwrap().contains_key("BootOrder"));
        assert!(!value.as_object().unwrap().contains_key("SecureBootMode"));
    }

    #[test]
    fn test_bios_attributes_serialize_some() {
        let attrs = BiosAttributes {
            boot_order: Some("Hdd,Pxe,Cd".to_string()),
            secure_boot_mode: Some("UserMode".to_string()),
        };

        let value = serde_json::to_value(&attrs).unwrap();
        assert_eq!(value["BootOrder"], "Hdd,Pxe,Cd");
        assert_eq!(value["SecureBootMode"], "UserMode");
    }

    #[test]
    fn test_bios_attributes_partial() {
        let attrs = BiosAttributes {
            boot_order: Some("Hdd".to_string()),
            secure_boot_mode: None,
        };

        let value = serde_json::to_value(&attrs).unwrap();
        assert_eq!(value["BootOrder"], "Hdd");
        assert!(!value.as_object().unwrap().contains_key("SecureBootMode"));
    }

    #[test]
    fn test_bios_actions_serialization() {
        let actions = BiosActions {
            reset_bios: super::super::systems::ActionTarget {
                target: "/redfish/v1/Systems/vm1/Bios/Actions/Bios.ResetBios".to_string(),
            },
        };

        let value = serde_json::to_value(&actions).unwrap();
        assert_eq!(
            value["#Bios.ResetBios"]["target"],
            "/redfish/v1/Systems/vm1/Bios/Actions/Bios.ResetBios"
        );
    }

    #[test]
    fn test_bios_links_serialization() {
        let links = BiosLinks {
            active_software_image: super::super::types::ODataId::new(
                "/redfish/v1/UpdateService/FirmwareInventory/vbmc-rs",
            ),
        };

        let value = serde_json::to_value(&links).unwrap();
        assert_eq!(
            value["ActiveSoftwareImage"]["@odata.id"],
            "/redfish/v1/UpdateService/FirmwareInventory/vbmc-rs"
        );
    }

    #[test]
    fn test_settings_object_serialization() {
        let settings = SettingsObject {
            settings_object: super::super::types::ODataId::new(
                "/redfish/v1/Systems/vm1/Bios/Settings",
            ),
        };

        let value = serde_json::to_value(&settings).unwrap();
        assert_eq!(
            value["SettingsObject"]["@odata.id"],
            "/redfish/v1/Systems/vm1/Bios/Settings"
        );
    }

    #[test]
    fn test_bios_attributes_default() {
        let attrs = BiosAttributes::default();
        assert_eq!(attrs.boot_order, None);
        assert_eq!(attrs.secure_boot_mode, None);
    }

    #[test]
    fn test_bios_attributes_clone() {
        let attrs = BiosAttributes {
            boot_order: Some("test".to_string()),
            secure_boot_mode: Some("mode".to_string()),
        };
        let cloned = attrs.clone();
        assert_eq!(attrs.boot_order, cloned.boot_order);
        assert_eq!(attrs.secure_boot_mode, cloned.secure_boot_mode);
    }

    #[test]
    fn test_patch_bios_settings_request_deserialization() {
        let json = json!({
            "Attributes": {
                "BootOrder": "Hdd,Pxe",
                "SecureBootMode": "UserMode"
            }
        });

        let request: PatchBiosSettingsRequest = serde_json::from_value(json).unwrap();
        assert!(request.attributes.is_some());
        let attrs = request.attributes.unwrap();
        assert_eq!(attrs.boot_order, Some("Hdd,Pxe".to_string()));
        assert_eq!(attrs.secure_boot_mode, Some("UserMode".to_string()));
    }

    #[test]
    fn test_patch_bios_settings_request_no_attributes() {
        let json = json!({});
        let request: PatchBiosSettingsRequest = serde_json::from_value(json).unwrap();
        assert!(request.attributes.is_none());
    }
}
