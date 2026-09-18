use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use serde::Serialize;

use super::error::RedfishApiError;
use super::types::{Collection, ODataId};
use crate::app_state::AppState;
use crate::auth::AuthenticatedUser;

#[derive(Debug, Serialize)]
pub struct BootOptionResource {
    #[serde(rename = "@odata.id")]
    pub odata_id: String,
    #[serde(rename = "@odata.type")]
    pub odata_type: &'static str,
    #[serde(rename = "Id")]
    pub id: String,
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Description")]
    pub description: &'static str,
    #[serde(rename = "BootOptionReference")]
    pub boot_option_reference: String,
    #[serde(rename = "DisplayName")]
    pub display_name: String,
    #[serde(rename = "BootOptionEnabled")]
    pub boot_option_enabled: bool,
    #[serde(rename = "Alias")]
    pub alias: String,
}

struct BootOptionDef {
    id: &'static str,
    display_name: &'static str,
    alias: &'static str,
}

const BOOT_OPTIONS: &[BootOptionDef] = &[
    BootOptionDef {
        id: "Hdd",
        display_name: "Hard Disk Drive",
        alias: "Hdd",
    },
    BootOptionDef {
        id: "Pxe",
        display_name: "PXE Network Boot",
        alias: "Pxe",
    },
    BootOptionDef {
        id: "Cd",
        display_name: "CD/DVD Drive",
        alias: "Cd",
    },
    BootOptionDef {
        id: "None",
        display_name: "No Boot Device",
        alias: "None",
    },
];

pub async fn get_boot_options(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
    Path(system_id): Path<String>,
) -> Result<Json<Collection<ODataId>>, RedfishApiError> {
    if !state.config.systems.contains_key(&system_id) {
        return Err(RedfishApiError::NotFound(format!(
            "System '{system_id}' not found"
        )));
    }

    let members: Vec<ODataId> = BOOT_OPTIONS
        .iter()
        .map(|o| {
            ODataId::new(format!(
                "/redfish/v1/Systems/{system_id}/BootOptions/{}",
                o.id
            ))
        })
        .collect();

    Ok(Json(Collection::new(
        format!("/redfish/v1/Systems/{system_id}/BootOptions"),
        "#BootOptionCollection.BootOptionCollection",
        "Boot Option Collection",
        members,
    )))
}

pub async fn get_boot_option(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
    Path((system_id, option_id)): Path<(String, String)>,
) -> Result<Json<BootOptionResource>, RedfishApiError> {
    if !state.config.systems.contains_key(&system_id) {
        return Err(RedfishApiError::NotFound(format!(
            "System '{system_id}' not found"
        )));
    }

    let def = BOOT_OPTIONS
        .iter()
        .find(|o| o.id == option_id)
        .ok_or_else(|| RedfishApiError::NotFound(format!("BootOption '{option_id}' not found")))?;

    Ok(Json(BootOptionResource {
        odata_id: format!("/redfish/v1/Systems/{system_id}/BootOptions/{}", def.id),
        odata_type: "#BootOption.v1_0_4.BootOption",
        id: def.id.to_string(),
        name: def.display_name.to_string(),
        description: "Boot option entry",
        boot_option_reference: def.id.to_string(),
        display_name: def.display_name.to_string(),
        boot_option_enabled: true,
        alias: def.alias.to_string(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_boot_option_resource_serialization() {
        let resource = BootOptionResource {
            odata_id: "/redfish/v1/Systems/vm1/BootOptions/Hdd".to_string(),
            odata_type: "#BootOption.v1_0_4.BootOption",
            id: "Hdd".to_string(),
            name: "Hard Disk Drive".to_string(),
            description: "Boot option entry",
            boot_option_reference: "Hdd".to_string(),
            display_name: "Hard Disk Drive".to_string(),
            boot_option_enabled: true,
            alias: "Hdd".to_string(),
        };

        let value = serde_json::to_value(&resource).unwrap();

        assert_eq!(
            value["@odata.id"],
            "/redfish/v1/Systems/vm1/BootOptions/Hdd"
        );
        assert_eq!(value["@odata.type"], "#BootOption.v1_0_4.BootOption");
        assert_eq!(value["Id"], "Hdd");
        assert_eq!(value["Name"], "Hard Disk Drive");
        assert_eq!(value["Description"], "Boot option entry");
        assert_eq!(value["BootOptionReference"], "Hdd");
        assert_eq!(value["DisplayName"], "Hard Disk Drive");
        assert_eq!(value["BootOptionEnabled"], true);
        assert_eq!(value["Alias"], "Hdd");
    }

    #[test]
    fn test_boot_options_constants() {
        assert_eq!(BOOT_OPTIONS.len(), 4);

        let hdd = BOOT_OPTIONS.iter().find(|o| o.id == "Hdd").unwrap();
        assert_eq!(hdd.display_name, "Hard Disk Drive");
        assert_eq!(hdd.alias, "Hdd");

        let pxe = BOOT_OPTIONS.iter().find(|o| o.id == "Pxe").unwrap();
        assert_eq!(pxe.display_name, "PXE Network Boot");
        assert_eq!(pxe.alias, "Pxe");

        let cd = BOOT_OPTIONS.iter().find(|o| o.id == "Cd").unwrap();
        assert_eq!(cd.display_name, "CD/DVD Drive");
        assert_eq!(cd.alias, "Cd");

        let none = BOOT_OPTIONS.iter().find(|o| o.id == "None").unwrap();
        assert_eq!(none.display_name, "No Boot Device");
        assert_eq!(none.alias, "None");
    }

    #[test]
    fn test_all_boot_options_have_unique_ids() {
        let mut ids = std::collections::HashSet::new();
        for opt in BOOT_OPTIONS {
            assert!(ids.insert(opt.id), "Duplicate boot option ID: {}", opt.id);
        }
    }

    #[test]
    fn test_boot_option_resource_with_pxe() {
        let resource = BootOptionResource {
            odata_id: "/redfish/v1/Systems/vm1/BootOptions/Pxe".to_string(),
            odata_type: "#BootOption.v1_0_4.BootOption",
            id: "Pxe".to_string(),
            name: "PXE Network Boot".to_string(),
            description: "Boot option entry",
            boot_option_reference: "Pxe".to_string(),
            display_name: "PXE Network Boot".to_string(),
            boot_option_enabled: true,
            alias: "Pxe".to_string(),
        };

        let value = serde_json::to_value(&resource).unwrap();
        assert_eq!(value["Id"], "Pxe");
        assert_eq!(value["DisplayName"], "PXE Network Boot");
    }

    #[test]
    fn test_boot_option_resource_disabled() {
        let resource = BootOptionResource {
            odata_id: "/redfish/v1/Systems/vm1/BootOptions/None".to_string(),
            odata_type: "#BootOption.v1_0_4.BootOption",
            id: "None".to_string(),
            name: "No Boot Device".to_string(),
            description: "Boot option entry",
            boot_option_reference: "None".to_string(),
            display_name: "No Boot Device".to_string(),
            boot_option_enabled: false,
            alias: "None".to_string(),
        };

        let value = serde_json::to_value(&resource).unwrap();
        assert_eq!(value["BootOptionEnabled"], false);
    }
}
