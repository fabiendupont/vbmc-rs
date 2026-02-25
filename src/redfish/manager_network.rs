use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use serde::Serialize;

use super::error::RedfishApiError;
use super::types::{Collection, ODataId, Status};
use crate::app_state::AppState;

#[derive(Debug, Serialize)]
pub struct NetworkProtocolResource {
    #[serde(rename = "@odata.id")]
    pub odata_id: &'static str,
    #[serde(rename = "@odata.type")]
    pub odata_type: &'static str,
    #[serde(rename = "Id")]
    pub id: &'static str,
    #[serde(rename = "Name")]
    pub name: &'static str,
    #[serde(rename = "Description")]
    pub description: &'static str,
    #[serde(rename = "HostName")]
    pub host_name: String,
    #[serde(rename = "FQDN")]
    pub fqdn: String,
    #[serde(rename = "HTTPS")]
    pub https: ProtocolEntry,
    #[serde(rename = "SSH")]
    pub ssh: ProtocolEntry,
    #[serde(rename = "IPMI")]
    pub ipmi: ProtocolEntry,
    #[serde(rename = "Status")]
    pub status: Status,
}

#[derive(Debug, Serialize)]
pub struct ProtocolEntry {
    #[serde(rename = "ProtocolEnabled")]
    pub protocol_enabled: bool,
    #[serde(rename = "Port")]
    pub port: u16,
}

#[derive(Debug, Serialize)]
pub struct EthernetInterfaceResource {
    #[serde(rename = "@odata.id")]
    pub odata_id: String,
    #[serde(rename = "@odata.type")]
    pub odata_type: &'static str,
    #[serde(rename = "Id")]
    pub id: String,
    #[serde(rename = "Name")]
    pub name: &'static str,
    #[serde(rename = "Description")]
    pub description: &'static str,
    #[serde(rename = "MACAddress")]
    pub mac_address: &'static str,
    #[serde(rename = "SpeedMbps")]
    pub speed_mbps: u32,
    #[serde(rename = "FullDuplex")]
    pub full_duplex: bool,
    #[serde(rename = "InterfaceEnabled")]
    pub interface_enabled: bool,
    #[serde(rename = "LinkStatus")]
    pub link_status: &'static str,
    #[serde(rename = "AutoNeg")]
    pub auto_neg: bool,
    #[serde(rename = "IPv4Addresses")]
    pub ipv4_addresses: Vec<Ipv4Address>,
    #[serde(rename = "Status")]
    pub status: Status,
}

#[derive(Debug, Serialize)]
pub struct Ipv4Address {
    #[serde(rename = "Address")]
    pub address: String,
    #[serde(rename = "SubnetMask")]
    pub subnet_mask: &'static str,
    #[serde(rename = "AddressOrigin")]
    pub address_origin: &'static str,
    #[serde(rename = "Gateway")]
    pub gateway: &'static str,
}

pub async fn get_network_protocol(
    State(state): State<Arc<AppState>>,
) -> Json<NetworkProtocolResource> {
    let hostname = std::fs::read_to_string("/etc/hostname")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "vbmc".to_string());
    let port = state.config.server.port;

    Json(NetworkProtocolResource {
        odata_id: "/redfish/v1/Managers/vbmc/NetworkProtocol",
        odata_type: "#ManagerNetworkProtocol.v1_10_0.ManagerNetworkProtocol",
        id: "NetworkProtocol",
        name: "Manager Network Protocol",
        description: "Manager network protocol settings",
        host_name: hostname.clone(),
        fqdn: hostname,
        https: ProtocolEntry {
            protocol_enabled: true,
            port,
        },
        ssh: ProtocolEntry {
            protocol_enabled: false,
            port: 22,
        },
        ipmi: ProtocolEntry {
            protocol_enabled: false,
            port: 623,
        },
        status: Status::enabled_ok(),
    })
}

pub async fn get_manager_ethernet_interfaces() -> Json<Collection<ODataId>> {
    let members = vec![ODataId::new(
        "/redfish/v1/Managers/vbmc/EthernetInterfaces/mgmt0",
    )];

    Json(Collection::new(
        "/redfish/v1/Managers/vbmc/EthernetInterfaces",
        "#EthernetInterfaceCollection.EthernetInterfaceCollection",
        "Manager Ethernet Interface Collection",
        members,
    ))
}

pub async fn get_manager_ethernet_interface(
    State(state): State<Arc<AppState>>,
    Path(nic_id): Path<String>,
) -> Result<Json<EthernetInterfaceResource>, RedfishApiError> {
    if nic_id != "mgmt0" {
        return Err(RedfishApiError::NotFound(format!(
            "EthernetInterface '{nic_id}' not found"
        )));
    }

    let bind_address = state.config.server.bind_address.clone();
    let addr = if bind_address == "0.0.0.0" {
        "127.0.0.1".to_string()
    } else {
        bind_address
    };

    Ok(Json(EthernetInterfaceResource {
        odata_id: format!("/redfish/v1/Managers/vbmc/EthernetInterfaces/{nic_id}"),
        odata_type: "#EthernetInterface.v1_12_0.EthernetInterface",
        id: nic_id,
        name: "Manager Ethernet Interface",
        description: "Management network interface",
        mac_address: "02:42:AC:11:00:02",
        speed_mbps: 1000,
        full_duplex: true,
        interface_enabled: true,
        link_status: "LinkUp",
        auto_neg: true,
        ipv4_addresses: vec![Ipv4Address {
            address: addr,
            subnet_mask: "255.255.255.0",
            address_origin: "Static",
            gateway: "0.0.0.0",
        }],
        status: Status::enabled_ok(),
    }))
}
