use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use serde::Serialize;

use super::error::RedfishApiError;
use super::types::{Collection, ODataId, Status};
use crate::app_state::AppState;
use crate::auth::AuthenticatedUser;

#[derive(Debug, Serialize)]
pub struct NetworkInterfaceResource {
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
    #[serde(rename = "Links")]
    pub links: NetworkInterfaceLinks,
    #[serde(rename = "Status")]
    pub status: Status,
}

#[derive(Debug, Serialize)]
pub struct NetworkInterfaceLinks {
    #[serde(rename = "NetworkAdapter")]
    pub network_adapter: ODataId,
}

pub async fn get_network_interfaces(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
    Path(system_id): Path<String>,
) -> Result<Json<Collection<ODataId>>, RedfishApiError> {
    if !state.config.systems.contains_key(&system_id) {
        return Err(RedfishApiError::NotFound(format!(
            "System '{system_id}' not found"
        )));
    }

    let members = vec![ODataId::new(format!(
        "/redfish/v1/Systems/{system_id}/NetworkInterfaces/NIC0"
    ))];

    Ok(Json(Collection::new(
        format!("/redfish/v1/Systems/{system_id}/NetworkInterfaces"),
        "#NetworkInterfaceCollection.NetworkInterfaceCollection",
        "Network Interface Collection",
        members,
    )))
}

pub async fn get_network_interface(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
    Path((system_id, nic_id)): Path<(String, String)>,
) -> Result<Json<NetworkInterfaceResource>, RedfishApiError> {
    if !state.config.systems.contains_key(&system_id) {
        return Err(RedfishApiError::NotFound(format!(
            "System '{system_id}' not found"
        )));
    }
    if nic_id != "NIC0" {
        return Err(RedfishApiError::NotFound(format!(
            "NetworkInterface '{nic_id}' not found"
        )));
    }

    Ok(Json(NetworkInterfaceResource {
        odata_id: format!("/redfish/v1/Systems/{system_id}/NetworkInterfaces/{nic_id}"),
        odata_type: "#NetworkInterface.v1_2_0.NetworkInterface",
        id: "NIC0",
        name: "Network Interface",
        description: "System network interface",
        links: NetworkInterfaceLinks {
            network_adapter: ODataId::new(format!(
                "/redfish/v1/Chassis/1/NetworkAdapters/{system_id}_NIC0"
            )),
        },
        status: Status::enabled_ok(),
    }))
}
