use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use serde::Serialize;

use super::error::RedfishApiError;
use super::types::{Collection, ODataId, Status};
use crate::app_state::AppState;
use crate::backend::VmmBackend;

#[derive(Debug, Serialize)]
pub struct EthernetInterface {
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
    #[serde(rename = "MACAddress", skip_serializing_if = "Option::is_none")]
    pub mac_address: Option<String>,
    #[serde(rename = "SpeedMbps")]
    pub speed_mbps: u32,
    #[serde(rename = "Status")]
    pub status: Status,
}

pub async fn get_ethernet_interfaces(
    State(state): State<Arc<AppState>>,
    Path(system_id): Path<String>,
) -> Result<Json<Collection<ODataId>>, RedfishApiError> {
    if !state.config.systems.contains_key(&system_id) {
        return Err(RedfishApiError::NotFound(format!(
            "System '{system_id}' not found"
        )));
    }

    let mut members = Vec::new();
    if let Ok(info) = state.backend.vm_info(&system_id).await {
        for (i, _nic) in info.nics.iter().enumerate() {
            members.push(ODataId::new(format!(
                "/redfish/v1/Systems/{system_id}/EthernetInterfaces/NIC{i}"
            )));
        }
    }

    Ok(Json(Collection::new(
        format!("/redfish/v1/Systems/{system_id}/EthernetInterfaces"),
        "#EthernetInterfaceCollection.EthernetInterfaceCollection",
        "Ethernet Interface Collection",
        members,
    )))
}

pub async fn get_ethernet_interface(
    State(state): State<Arc<AppState>>,
    Path((system_id, nic_id)): Path<(String, String)>,
) -> Result<Json<EthernetInterface>, RedfishApiError> {
    if !state.config.systems.contains_key(&system_id) {
        return Err(RedfishApiError::NotFound(format!(
            "System '{system_id}' not found"
        )));
    }

    let idx: usize = nic_id
        .strip_prefix("NIC")
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| RedfishApiError::NotFound(format!("NIC '{nic_id}' not found")))?;

    let info = state
        .backend
        .vm_info(&system_id)
        .await
        .map_err(|e| RedfishApiError::InternalError(e.to_string()))?;

    let nic = info
        .nics
        .get(idx)
        .ok_or_else(|| RedfishApiError::NotFound(format!("NIC '{nic_id}' not found")))?;

    Ok(Json(EthernetInterface {
        odata_id: format!(
            "/redfish/v1/Systems/{system_id}/EthernetInterfaces/{nic_id}"
        ),
        odata_type: "#EthernetInterface.v1_12_0.EthernetInterface",
        id: nic_id,
        name: nic.id.clone(),
        description: "Virtual network interface",
        mac_address: nic.mac_address.clone(),
        speed_mbps: nic.speed_mbps,
        status: Status::enabled_ok(),
    }))
}
