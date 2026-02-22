use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use serde::Serialize;

use super::error::RedfishApiError;
use super::types::{Collection, ODataId, Status};
use crate::app_state::AppState;
use crate::backend::VmmBackend;

#[derive(Debug, Serialize)]
pub struct NetworkAdapterResource {
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
    #[serde(rename = "Manufacturer")]
    pub manufacturer: &'static str,
    #[serde(rename = "NetworkDeviceFunctions")]
    pub network_device_functions: ODataId,
    #[serde(rename = "Status")]
    pub status: Status,
}

#[derive(Debug, Serialize)]
pub struct NetworkDeviceFunction {
    #[serde(rename = "@odata.id")]
    pub odata_id: String,
    #[serde(rename = "@odata.type")]
    pub odata_type: &'static str,
    #[serde(rename = "Id")]
    pub id: String,
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Ethernet")]
    pub ethernet: EthernetProperties,
    #[serde(rename = "Status")]
    pub status: Status,
}

#[derive(Debug, Serialize)]
pub struct EthernetProperties {
    #[serde(rename = "MACAddress", skip_serializing_if = "Option::is_none")]
    pub mac_address: Option<String>,
}

pub async fn get_network_adapters(
    State(state): State<Arc<AppState>>,
) -> Json<Collection<ODataId>> {
    // We aggregate NICs across all systems into chassis-level adapters
    let mut members = Vec::new();
    for (system_id, _) in &state.config.systems {
        if let Ok(info) = state.backend.vm_info(system_id).await {
            for (i, _nic) in info.nics.iter().enumerate() {
                members.push(ODataId::new(format!(
                    "/redfish/v1/Chassis/1/NetworkAdapters/{system_id}_NIC{i}"
                )));
            }
        }
    }

    Json(Collection::new(
        "/redfish/v1/Chassis/1/NetworkAdapters",
        "#NetworkAdapterCollection.NetworkAdapterCollection",
        "Network Adapter Collection",
        members,
    ))
}

pub async fn get_network_adapter(
    State(state): State<Arc<AppState>>,
    Path(adapter_id): Path<String>,
) -> Result<Json<NetworkAdapterResource>, RedfishApiError> {
    // adapter_id format: "{system_id}_NIC{idx}"
    let (system_id, nic_suffix) = adapter_id
        .rsplit_once('_')
        .ok_or_else(|| RedfishApiError::NotFound(format!("NetworkAdapter '{adapter_id}' not found")))?;

    let _idx: usize = nic_suffix
        .strip_prefix("NIC")
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| RedfishApiError::NotFound(format!("NetworkAdapter '{adapter_id}' not found")))?;

    if !state.config.systems.contains_key(system_id) {
        return Err(RedfishApiError::NotFound(format!(
            "NetworkAdapter '{adapter_id}' not found"
        )));
    }

    Ok(Json(NetworkAdapterResource {
        odata_id: format!("/redfish/v1/Chassis/1/NetworkAdapters/{adapter_id}"),
        odata_type: "#NetworkAdapter.v1_10_0.NetworkAdapter",
        id: adapter_id.clone(),
        name: format!("Network Adapter {adapter_id}"),
        description: "Virtual network adapter",
        manufacturer: "Virtual",
        network_device_functions: ODataId::new(format!(
            "/redfish/v1/Chassis/1/NetworkAdapters/{adapter_id}/NetworkDeviceFunctions"
        )),
        status: Status::enabled_ok(),
    }))
}

pub async fn get_network_device_functions(
    State(state): State<Arc<AppState>>,
    Path(adapter_id): Path<String>,
) -> Result<Json<Collection<ODataId>>, RedfishApiError> {
    let (system_id, nic_suffix) = adapter_id
        .rsplit_once('_')
        .ok_or_else(|| RedfishApiError::NotFound(format!("NetworkAdapter '{adapter_id}' not found")))?;

    let _idx: usize = nic_suffix
        .strip_prefix("NIC")
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| RedfishApiError::NotFound(format!("NetworkAdapter '{adapter_id}' not found")))?;

    if !state.config.systems.contains_key(system_id) {
        return Err(RedfishApiError::NotFound(format!(
            "NetworkAdapter '{adapter_id}' not found"
        )));
    }

    let members = vec![ODataId::new(format!(
        "/redfish/v1/Chassis/1/NetworkAdapters/{adapter_id}/NetworkDeviceFunctions/0"
    ))];

    Ok(Json(Collection::new(
        format!("/redfish/v1/Chassis/1/NetworkAdapters/{adapter_id}/NetworkDeviceFunctions"),
        "#NetworkDeviceFunctionCollection.NetworkDeviceFunctionCollection",
        "Network Device Function Collection",
        members,
    )))
}
