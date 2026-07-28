use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
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
    #[serde(rename = "Description")]
    pub description: &'static str,
    #[serde(rename = "NetDevFuncType")]
    pub net_dev_func_type: &'static str,
    #[serde(rename = "NetDevFuncCapabilities")]
    pub net_dev_func_capabilities: Vec<&'static str>,
    #[serde(rename = "DeviceEnabled")]
    pub device_enabled: bool,
    #[serde(rename = "BootMode")]
    pub boot_mode: &'static str,
    #[serde(rename = "VirtualFunctionsEnabled")]
    pub virtual_functions_enabled: bool,
    #[serde(rename = "MaxVirtualFunctions")]
    pub max_virtual_functions: u32,
    #[serde(rename = "Ethernet")]
    pub ethernet: EthernetProperties,
    #[serde(rename = "Links")]
    pub ndf_links: NdfLinks,
    #[serde(rename = "Status")]
    pub status: Status,
}

#[derive(Debug, Serialize)]
pub struct EthernetProperties {
    #[serde(rename = "MACAddress")]
    pub mac_address: String,
    #[serde(rename = "PermanentMACAddress")]
    pub permanent_mac_address: String,
    #[serde(rename = "MTUSize")]
    pub mtu_size: u32,
    #[serde(rename = "MTUSizeMaximum")]
    pub mtu_size_maximum: u32,
    #[serde(rename = "VLAN")]
    pub vlan: NdfVlan,
    #[serde(rename = "EthernetInterfaces")]
    pub ethernet_interfaces: ODataId,
}

#[derive(Debug, Serialize)]
pub struct NdfVlan {
    #[serde(rename = "VLANEnable")]
    pub vlan_enable: bool,
    #[serde(rename = "VLANId")]
    pub vlan_id: u32,
}

#[derive(Debug, Serialize)]
pub struct NdfLinks {
    #[serde(rename = "Endpoints")]
    pub endpoints: Vec<ODataId>,
    #[serde(rename = "PCIeFunction", skip_serializing_if = "Option::is_none")]
    pub pcie_function: Option<ODataId>,
    #[serde(
        rename = "PhysicalNetworkPortAssignment",
        skip_serializing_if = "Option::is_none"
    )]
    pub physical_network_port_assignment: Option<ODataId>,
    #[serde(rename = "EthernetInterfaces")]
    pub ethernet_interfaces: Vec<ODataId>,
}

pub async fn get_network_adapters(State(state): State<Arc<AppState>>) -> Json<Collection<ODataId>> {
    // We aggregate NICs across all systems into chassis-level adapters
    let mut members = Vec::new();
    for system_id in state.config.systems.keys() {
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
    let (system_id, nic_suffix) = adapter_id.rsplit_once('_').ok_or_else(|| {
        RedfishApiError::NotFound(format!("NetworkAdapter '{adapter_id}' not found"))
    })?;

    let _idx: usize = nic_suffix
        .strip_prefix("NIC")
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| {
            RedfishApiError::NotFound(format!("NetworkAdapter '{adapter_id}' not found"))
        })?;

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
    let (system_id, nic_suffix) = adapter_id.rsplit_once('_').ok_or_else(|| {
        RedfishApiError::NotFound(format!("NetworkAdapter '{adapter_id}' not found"))
    })?;

    let _idx: usize = nic_suffix
        .strip_prefix("NIC")
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| {
            RedfishApiError::NotFound(format!("NetworkAdapter '{adapter_id}' not found"))
        })?;

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

pub async fn get_network_device_function(
    State(state): State<Arc<AppState>>,
    Path((adapter_id, func_id)): Path<(String, String)>,
) -> Result<Json<NetworkDeviceFunction>, RedfishApiError> {
    let (system_id, nic_suffix) = adapter_id.rsplit_once('_').ok_or_else(|| {
        RedfishApiError::NotFound(format!("NetworkAdapter '{adapter_id}' not found"))
    })?;

    let idx: usize = nic_suffix
        .strip_prefix("NIC")
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| {
            RedfishApiError::NotFound(format!("NetworkAdapter '{adapter_id}' not found"))
        })?;

    if !state.config.systems.contains_key(system_id) {
        return Err(RedfishApiError::NotFound(format!(
            "NetworkAdapter '{adapter_id}' not found"
        )));
    }
    if func_id != "0" {
        return Err(RedfishApiError::NotFound(format!(
            "NetworkDeviceFunction '{func_id}' not found"
        )));
    }

    let mac = state
        .backend
        .vm_info(system_id)
        .await
        .ok()
        .and_then(|info| info.nics.get(idx).and_then(|n| n.mac_address.clone()))
        .unwrap_or_else(|| "00:00:00:00:00:00".to_string());

    Ok(Json(NetworkDeviceFunction {
        odata_id: format!(
            "/redfish/v1/Chassis/1/NetworkAdapters/{adapter_id}/NetworkDeviceFunctions/{func_id}"
        ),
        odata_type: "#NetworkDeviceFunction.v1_9_0.NetworkDeviceFunction",
        id: func_id,
        name: format!("Network Device Function {adapter_id}"),
        description: "Virtual network device function",
        net_dev_func_type: "Ethernet",
        net_dev_func_capabilities: vec!["Ethernet"],
        device_enabled: true,
        boot_mode: "Disabled",
        virtual_functions_enabled: false,
        max_virtual_functions: 0,
        ethernet: EthernetProperties {
            permanent_mac_address: mac.clone(),
            mac_address: mac,
            mtu_size: 1500,
            mtu_size_maximum: 9000,
            vlan: NdfVlan {
                vlan_enable: false,
                vlan_id: 0,
            },
            ethernet_interfaces: ODataId::new(format!(
                "/redfish/v1/Systems/{system_id}/EthernetInterfaces"
            )),
        },
        ndf_links: NdfLinks {
            endpoints: Vec::new(),
            pcie_function: None,
            physical_network_port_assignment: None,
            ethernet_interfaces: Vec::new(),
        },
        status: Status::enabled_ok(),
    }))
}
