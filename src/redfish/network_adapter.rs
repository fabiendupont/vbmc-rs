use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use serde::Serialize;

use super::error::RedfishApiError;
use super::types::{Collection, ODataId, Status};
use crate::app_state::AppState;
use crate::auth::AuthenticatedUser;
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
    #[serde(rename = "Assembly")]
    pub assembly: ODataId,
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

pub async fn get_network_adapters(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
) -> Json<Collection<ODataId>> {
    // We aggregate NICs across all systems into chassis-level adapters
    let mut members = Vec::new();
    for system_id in state.config.systems.keys() {
        if let Ok(info) = state.backend.vm_info(system_id).await {
            for (i, _nic) in info.nics.iter().enumerate() {
                members.push(ODataId::new(format!(
                    "/redfish/v1/Chassis/{}/NetworkAdapters/{system_id}_NIC{i}",
                    state.chassis_id
                )));
            }
        }
    }

    Json(Collection::new(
        format!("/redfish/v1/Chassis/{}/NetworkAdapters", state.chassis_id),
        "#NetworkAdapterCollection.NetworkAdapterCollection",
        "Network Adapter Collection",
        members,
    ))
}

pub async fn get_network_adapter(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
    Path((_, adapter_id)): Path<(String, String)>,
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
        odata_id: format!(
            "/redfish/v1/Chassis/{}/NetworkAdapters/{adapter_id}",
            state.chassis_id
        ),
        odata_type: "#NetworkAdapter.v1_10_0.NetworkAdapter",
        id: adapter_id.clone(),
        name: format!("Network Adapter {adapter_id}"),
        description: "Virtual network adapter",
        manufacturer: "Virtual",
        network_device_functions: ODataId::new(format!(
            "/redfish/v1/Chassis/{}/NetworkAdapters/{adapter_id}/NetworkDeviceFunctions",
            state.chassis_id
        )),
        assembly: ODataId::new(format!(
            "/redfish/v1/Chassis/{}/NetworkAdapters/{adapter_id}/Assembly",
            state.chassis_id
        )),
        status: Status::enabled_ok(),
    }))
}

pub async fn get_network_device_functions(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
    Path((_, adapter_id)): Path<(String, String)>,
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
        "/redfish/v1/Chassis/{}/NetworkAdapters/{adapter_id}/NetworkDeviceFunctions/0",
        state.chassis_id
    ))];

    Ok(Json(Collection::new(
        format!(
            "/redfish/v1/Chassis/{}/NetworkAdapters/{adapter_id}/NetworkDeviceFunctions",
            state.chassis_id
        ),
        "#NetworkDeviceFunctionCollection.NetworkDeviceFunctionCollection",
        "Network Device Function Collection",
        members,
    )))
}

pub async fn get_network_device_function(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
    Path((_, adapter_id, func_id)): Path<(String, String, String)>,
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
            "/redfish/v1/Chassis/{}/NetworkAdapters/{adapter_id}/NetworkDeviceFunctions/{func_id}",
            state.chassis_id
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_adapter_resource_serialization() {
        let resource = NetworkAdapterResource {
            odata_id: "/redfish/v1/Chassis/ch1/NetworkAdapters/vm1_NIC0".to_string(),
            odata_type: "#NetworkAdapter.v1_10_0.NetworkAdapter",
            id: "vm1_NIC0".to_string(),
            name: "Network Adapter vm1_NIC0".to_string(),
            description: "Virtual network adapter",
            manufacturer: "Virtual",
            network_device_functions: ODataId::new(
                "/redfish/v1/Chassis/ch1/NetworkAdapters/vm1_NIC0/NetworkDeviceFunctions",
            ),
            assembly: ODataId::new("/redfish/v1/Chassis/ch1/NetworkAdapters/vm1_NIC0/Assembly"),
            status: Status::enabled_ok(),
        };

        let json = serde_json::to_value(&resource).unwrap();
        assert_eq!(
            json["@odata.id"],
            "/redfish/v1/Chassis/ch1/NetworkAdapters/vm1_NIC0"
        );
        assert_eq!(
            json["@odata.type"],
            "#NetworkAdapter.v1_10_0.NetworkAdapter"
        );
        assert_eq!(json["Id"], "vm1_NIC0");
        assert_eq!(json["Name"], "Network Adapter vm1_NIC0");
        assert_eq!(json["Manufacturer"], "Virtual");
        assert_eq!(
            json["NetworkDeviceFunctions"]["@odata.id"],
            "/redfish/v1/Chassis/ch1/NetworkAdapters/vm1_NIC0/NetworkDeviceFunctions"
        );
        assert_eq!(
            json["Assembly"]["@odata.id"],
            "/redfish/v1/Chassis/ch1/NetworkAdapters/vm1_NIC0/Assembly"
        );
    }

    #[test]
    fn test_ndf_vlan_serialization() {
        let vlan = NdfVlan {
            vlan_enable: true,
            vlan_id: 100,
        };

        let json = serde_json::to_value(&vlan).unwrap();
        assert_eq!(json["VLANEnable"], true);
        assert_eq!(json["VLANId"], 100);
    }

    #[test]
    fn test_ethernet_properties_serialization() {
        let ethernet = EthernetProperties {
            mac_address: "52:54:00:12:34:56".to_string(),
            permanent_mac_address: "52:54:00:12:34:56".to_string(),
            mtu_size: 1500,
            mtu_size_maximum: 9000,
            vlan: NdfVlan {
                vlan_enable: false,
                vlan_id: 0,
            },
            ethernet_interfaces: ODataId::new("/redfish/v1/Systems/vm1/EthernetInterfaces"),
        };

        let json = serde_json::to_value(&ethernet).unwrap();
        assert_eq!(json["MACAddress"], "52:54:00:12:34:56");
        assert_eq!(json["PermanentMACAddress"], "52:54:00:12:34:56");
        assert_eq!(json["MTUSize"], 1500);
        assert_eq!(json["MTUSizeMaximum"], 9000);
        assert_eq!(json["VLAN"]["VLANEnable"], false);
        assert_eq!(
            json["EthernetInterfaces"]["@odata.id"],
            "/redfish/v1/Systems/vm1/EthernetInterfaces"
        );
    }

    #[test]
    fn test_ndf_links_with_all_fields() {
        let links = NdfLinks {
            endpoints: vec![ODataId::new("/redfish/v1/Fabrics/Fabric1/Endpoints/EP1")],
            pcie_function: Some(ODataId::new(
                "/redfish/v1/Chassis/ch1/PCIeDevices/dev1/PCIeFunctions/0",
            )),
            physical_network_port_assignment: Some(ODataId::new(
                "/redfish/v1/Chassis/ch1/NetworkAdapters/nic1/Ports/1",
            )),
            ethernet_interfaces: vec![ODataId::new(
                "/redfish/v1/Systems/vm1/EthernetInterfaces/NIC0",
            )],
        };

        let json = serde_json::to_value(&links).unwrap();
        assert_eq!(
            json["Endpoints"][0]["@odata.id"],
            "/redfish/v1/Fabrics/Fabric1/Endpoints/EP1"
        );
        assert_eq!(
            json["PCIeFunction"]["@odata.id"],
            "/redfish/v1/Chassis/ch1/PCIeDevices/dev1/PCIeFunctions/0"
        );
        assert_eq!(
            json["PhysicalNetworkPortAssignment"]["@odata.id"],
            "/redfish/v1/Chassis/ch1/NetworkAdapters/nic1/Ports/1"
        );
        assert_eq!(
            json["EthernetInterfaces"][0]["@odata.id"],
            "/redfish/v1/Systems/vm1/EthernetInterfaces/NIC0"
        );
    }

    #[test]
    fn test_ndf_links_with_none_fields() {
        let links = NdfLinks {
            endpoints: Vec::new(),
            pcie_function: None,
            physical_network_port_assignment: None,
            ethernet_interfaces: Vec::new(),
        };

        let json = serde_json::to_value(&links).unwrap();
        assert!(json["Endpoints"].as_array().unwrap().is_empty());
        assert!(json.get("PCIeFunction").is_none());
        assert!(json.get("PhysicalNetworkPortAssignment").is_none());
        assert!(json["EthernetInterfaces"].as_array().unwrap().is_empty());
    }

    #[test]
    fn test_network_device_function_serialization() {
        let ndf = NetworkDeviceFunction {
            odata_id: "/redfish/v1/Chassis/ch1/NetworkAdapters/vm1_NIC0/NetworkDeviceFunctions/0"
                .to_string(),
            odata_type: "#NetworkDeviceFunction.v1_9_0.NetworkDeviceFunction",
            id: "0".to_string(),
            name: "Network Device Function vm1_NIC0".to_string(),
            description: "Virtual network device function",
            net_dev_func_type: "Ethernet",
            net_dev_func_capabilities: vec!["Ethernet"],
            device_enabled: true,
            boot_mode: "Disabled",
            virtual_functions_enabled: false,
            max_virtual_functions: 0,
            ethernet: EthernetProperties {
                mac_address: "52:54:00:12:34:56".to_string(),
                permanent_mac_address: "52:54:00:12:34:56".to_string(),
                mtu_size: 1500,
                mtu_size_maximum: 9000,
                vlan: NdfVlan {
                    vlan_enable: false,
                    vlan_id: 0,
                },
                ethernet_interfaces: ODataId::new("/redfish/v1/Systems/vm1/EthernetInterfaces"),
            },
            ndf_links: NdfLinks {
                endpoints: Vec::new(),
                pcie_function: None,
                physical_network_port_assignment: None,
                ethernet_interfaces: Vec::new(),
            },
            status: Status::enabled_ok(),
        };

        let json = serde_json::to_value(&ndf).unwrap();
        assert_eq!(
            json["@odata.id"],
            "/redfish/v1/Chassis/ch1/NetworkAdapters/vm1_NIC0/NetworkDeviceFunctions/0"
        );
        assert_eq!(
            json["@odata.type"],
            "#NetworkDeviceFunction.v1_9_0.NetworkDeviceFunction"
        );
        assert_eq!(json["Id"], "0");
        assert_eq!(json["NetDevFuncType"], "Ethernet");
        assert_eq!(json["NetDevFuncCapabilities"][0], "Ethernet");
        assert_eq!(json["DeviceEnabled"], true);
        assert_eq!(json["BootMode"], "Disabled");
        assert_eq!(json["VirtualFunctionsEnabled"], false);
        assert_eq!(json["MaxVirtualFunctions"], 0);
        assert_eq!(json["Ethernet"]["MACAddress"], "52:54:00:12:34:56");
    }

    #[tokio::test]
    async fn test_get_network_adapters_collection() {
        use crate::redfish::test_harness::*;

        let mock = crate::backend::mock::MockBackend::new().with_vm("vm1", running_vm());
        let router = router(app_state(mock, systems_with("vm1")));
        let (status, json, _) = get(&router, "/redfish/v1/Chassis/1/NetworkAdapters").await;

        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(
            json["@odata.type"],
            "#NetworkAdapterCollection.NetworkAdapterCollection"
        );
        assert_eq!(json["Name"], "Network Adapter Collection");
        assert!(json["Members@odata.count"].as_u64().unwrap() >= 1);
        assert!(
            json["Members"][0]["@odata.id"]
                .as_str()
                .unwrap()
                .contains("NetworkAdapters")
        );
    }

    #[tokio::test]
    async fn test_get_network_adapter_valid() {
        use crate::redfish::test_harness::*;

        let mock = crate::backend::mock::MockBackend::new().with_vm("vm1", running_vm());
        let router = router(app_state(mock, systems_with("vm1")));
        let (status, json, _) =
            get(&router, "/redfish/v1/Chassis/1/NetworkAdapters/vm1_NIC0").await;

        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(
            json["@odata.type"],
            "#NetworkAdapter.v1_10_0.NetworkAdapter"
        );
        assert_eq!(json["Id"], "vm1_NIC0");
        assert_eq!(json["Name"], "Network Adapter vm1_NIC0");
        assert_eq!(json["Description"], "Virtual network adapter");
        assert_eq!(json["Manufacturer"], "Virtual");
        assert_eq!(
            json["NetworkDeviceFunctions"]["@odata.id"],
            "/redfish/v1/Chassis/1/NetworkAdapters/vm1_NIC0/NetworkDeviceFunctions"
        );
        assert_eq!(json["Status"]["State"], "Enabled");
    }

    #[tokio::test]
    async fn test_get_network_adapter_invalid_id() {
        use crate::redfish::test_harness::*;

        let mock = crate::backend::mock::MockBackend::new().with_vm("vm1", running_vm());
        let router = router(app_state(mock, systems_with("vm1")));
        let (status, _, _) = get(&router, "/redfish/v1/Chassis/1/NetworkAdapters/invalid").await;

        assert_eq!(status, axum::http::StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_get_network_adapter_unknown_system() {
        use crate::redfish::test_harness::*;

        let mock = crate::backend::mock::MockBackend::new().with_vm("vm1", running_vm());
        let router = router(app_state(mock, systems_with("vm1")));
        let (status, _, _) = get(
            &router,
            "/redfish/v1/Chassis/1/NetworkAdapters/unknown_NIC0",
        )
        .await;

        assert_eq!(status, axum::http::StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_get_network_device_functions_collection() {
        use crate::redfish::test_harness::*;

        let mock = crate::backend::mock::MockBackend::new().with_vm("vm1", running_vm());
        let router = router(app_state(mock, systems_with("vm1")));
        let (status, json, _) = get(
            &router,
            "/redfish/v1/Chassis/1/NetworkAdapters/vm1_NIC0/NetworkDeviceFunctions",
        )
        .await;

        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(
            json["@odata.type"],
            "#NetworkDeviceFunctionCollection.NetworkDeviceFunctionCollection"
        );
        assert_eq!(json["Name"], "Network Device Function Collection");
        assert_eq!(json["Members@odata.count"], 1);
        assert_eq!(
            json["Members"][0]["@odata.id"],
            "/redfish/v1/Chassis/1/NetworkAdapters/vm1_NIC0/NetworkDeviceFunctions/0"
        );
    }

    #[tokio::test]
    async fn test_get_network_device_functions_invalid_adapter() {
        use crate::redfish::test_harness::*;

        let mock = crate::backend::mock::MockBackend::new().with_vm("vm1", running_vm());
        let router = router(app_state(mock, systems_with("vm1")));
        let (status, _, _) = get(
            &router,
            "/redfish/v1/Chassis/1/NetworkAdapters/invalid/NetworkDeviceFunctions",
        )
        .await;

        assert_eq!(status, axum::http::StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_get_network_device_function_valid() {
        use crate::redfish::test_harness::*;

        let mock = crate::backend::mock::MockBackend::new().with_vm("vm1", running_vm());
        let router = router(app_state(mock, systems_with("vm1")));
        let (status, json, _) = get(
            &router,
            "/redfish/v1/Chassis/1/NetworkAdapters/vm1_NIC0/NetworkDeviceFunctions/0",
        )
        .await;

        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(
            json["@odata.type"],
            "#NetworkDeviceFunction.v1_9_0.NetworkDeviceFunction"
        );
        assert_eq!(json["Id"], "0");
        assert_eq!(json["NetDevFuncType"], "Ethernet");
        assert_eq!(json["NetDevFuncCapabilities"][0], "Ethernet");
        assert_eq!(json["DeviceEnabled"], true);
        assert_eq!(json["Ethernet"]["MACAddress"], "52:54:00:12:34:56");
        assert_eq!(json["Ethernet"]["PermanentMACAddress"], "52:54:00:12:34:56");
        assert_eq!(json["Ethernet"]["MTUSize"], 1500);
        assert_eq!(json["Status"]["State"], "Enabled");
    }

    #[tokio::test]
    async fn test_get_network_device_function_invalid_func_id() {
        use crate::redfish::test_harness::*;

        let mock = crate::backend::mock::MockBackend::new().with_vm("vm1", running_vm());
        let router = router(app_state(mock, systems_with("vm1")));
        let (status, _, _) = get(
            &router,
            "/redfish/v1/Chassis/1/NetworkAdapters/vm1_NIC0/NetworkDeviceFunctions/99",
        )
        .await;

        assert_eq!(status, axum::http::StatusCode::NOT_FOUND);
    }
}
