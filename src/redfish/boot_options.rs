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
