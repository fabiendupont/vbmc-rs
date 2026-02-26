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
    #[serde(rename = "NTP")]
    pub ntp: ProtocolEntry,
    #[serde(rename = "DHCP")]
    pub dhcp: ProtocolEntry,
    #[serde(rename = "DHCPv6")]
    pub dhcpv6_proto: ProtocolEntry,
    #[serde(rename = "SNMP")]
    pub snmp: ProtocolEntry,
    #[serde(rename = "HTTP")]
    pub http: ProtocolEntry,
    #[serde(rename = "Telnet")]
    pub telnet: ProtocolEntry,
    #[serde(rename = "SSDP")]
    pub ssdp: SsdpProtocol,
    #[serde(rename = "VirtualMedia")]
    pub virtual_media_proto: ProtocolEntry,
    #[serde(rename = "KVMIP")]
    pub kvmip: ProtocolEntry,
    #[serde(rename = "RDP")]
    pub rdp: ProtocolEntry,
    #[serde(rename = "RFB")]
    pub rfb: ProtocolEntry,
    #[serde(rename = "FTP")]
    pub ftp: ProtocolEntry,
    #[serde(rename = "SFTP")]
    pub sftp: ProtocolEntry,
    #[serde(rename = "FTPS")]
    pub ftps: ProtocolEntry,
    #[serde(rename = "Proxy")]
    pub proxy: ProxyConfig,
    #[serde(rename = "Status")]
    pub status: Status,
}

#[derive(Debug, Serialize)]
pub struct SsdpProtocol {
    #[serde(rename = "ProtocolEnabled")]
    pub protocol_enabled: bool,
    #[serde(rename = "Port")]
    pub port: u16,
    #[serde(rename = "NotifyMulticastIntervalSeconds")]
    pub notify_multicast_interval_seconds: u32,
    #[serde(rename = "NotifyTTL")]
    pub notify_ttl: u32,
    #[serde(rename = "NotifyIPv6Scope")]
    pub notify_ipv6_scope: &'static str,
}

#[derive(Debug, Serialize)]
pub struct ProxyConfig {
    #[serde(rename = "Enabled")]
    pub enabled: bool,
    #[serde(rename = "ProxyAutoConfigURI")]
    pub proxy_auto_config_uri: &'static str,
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
    #[serde(rename = "PermanentMACAddress")]
    pub permanent_mac_address: &'static str,
    #[serde(rename = "SpeedMbps")]
    pub speed_mbps: u32,
    #[serde(rename = "FullDuplex")]
    pub full_duplex: bool,
    #[serde(rename = "MTUSize")]
    pub mtu_size: u32,
    #[serde(rename = "InterfaceEnabled")]
    pub interface_enabled: bool,
    #[serde(rename = "LinkStatus")]
    pub link_status: &'static str,
    #[serde(rename = "AutoNeg")]
    pub auto_neg: bool,
    #[serde(rename = "EthernetInterfaceType")]
    pub ethernet_interface_type: &'static str,
    #[serde(rename = "HostName")]
    pub host_name: String,
    #[serde(rename = "FQDN")]
    pub fqdn: String,
    #[serde(rename = "NameServers")]
    pub name_servers: Vec<&'static str>,
    #[serde(rename = "StaticNameServers")]
    pub static_name_servers: Vec<&'static str>,
    #[serde(rename = "MaxIPv6StaticAddresses")]
    pub max_ipv6_static_addresses: u32,
    #[serde(rename = "IPv4Addresses")]
    pub ipv4_addresses: Vec<Ipv4Address>,
    #[serde(rename = "IPv4StaticAddresses")]
    pub ipv4_static_addresses: Vec<Ipv4Address>,
    #[serde(rename = "IPv6Enabled")]
    pub ipv6_enabled: bool,
    #[serde(rename = "IPv6Addresses")]
    pub ipv6_addresses: Vec<serde_json::Value>,
    #[serde(rename = "IPv6StaticAddresses")]
    pub ipv6_static_addresses: Vec<serde_json::Value>,
    #[serde(rename = "IPv6DefaultGateway")]
    pub ipv6_default_gateway: &'static str,
    #[serde(rename = "IPv6StaticDefaultGateways")]
    pub ipv6_static_default_gateways: Vec<serde_json::Value>,
    #[serde(rename = "IPv6AddressPolicyTable")]
    pub ipv6_address_policy_table: Vec<serde_json::Value>,
    #[serde(rename = "DHCPv4")]
    pub dhcpv4: DhcpV4Config,
    #[serde(rename = "DHCPv6")]
    pub dhcpv6: DhcpV6Config,
    #[serde(rename = "StatelessAddressAutoConfig")]
    pub stateless_address_auto_config: StatelessConfig,
    #[serde(rename = "VLAN")]
    pub vlan: VlanConfig,
    #[serde(rename = "Links")]
    pub links: EthernetLinks,
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

#[derive(Debug, Serialize)]
pub struct DhcpV4Config {
    #[serde(rename = "DHCPEnabled")]
    pub dhcp_enabled: bool,
    #[serde(rename = "UseDNSServers")]
    pub use_dns_servers: bool,
    #[serde(rename = "UseGateway")]
    pub use_gateway: bool,
    #[serde(rename = "UseNTPServers")]
    pub use_ntp_servers: bool,
    #[serde(rename = "UseDomainName")]
    pub use_domain_name: bool,
    #[serde(rename = "UseStaticRoutes")]
    pub use_static_routes: bool,
}

#[derive(Debug, Serialize)]
pub struct DhcpV6Config {
    #[serde(rename = "OperatingMode")]
    pub operating_mode: &'static str,
    #[serde(rename = "UseDNSServers")]
    pub use_dns_servers: bool,
    #[serde(rename = "UseNTPServers")]
    pub use_ntp_servers: bool,
    #[serde(rename = "UseDomainName")]
    pub use_domain_name: bool,
    #[serde(rename = "UseRapidCommit")]
    pub use_rapid_commit: bool,
}

#[derive(Debug, Serialize)]
pub struct StatelessConfig {
    #[serde(rename = "IPv4AutoConfigEnabled")]
    pub ipv4_auto_config_enabled: bool,
    #[serde(rename = "IPv6AutoConfigEnabled")]
    pub ipv6_auto_config_enabled: bool,
}

#[derive(Debug, Serialize)]
pub struct VlanConfig {
    #[serde(rename = "VLANEnable")]
    pub vlan_enable: bool,
    #[serde(rename = "VLANId")]
    pub vlan_id: u32,
    #[serde(rename = "VLANPriority")]
    pub vlan_priority: u32,
    #[serde(rename = "Tagged")]
    pub tagged: bool,
}

#[derive(Debug, Serialize)]
pub struct EthernetLinks {
    #[serde(rename = "Chassis")]
    pub chassis: ODataId,
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
        ntp: ProtocolEntry { protocol_enabled: false, port: 123 },
        dhcp: ProtocolEntry { protocol_enabled: false, port: 67 },
        dhcpv6_proto: ProtocolEntry { protocol_enabled: false, port: 547 },
        snmp: ProtocolEntry { protocol_enabled: false, port: 161 },
        http: ProtocolEntry { protocol_enabled: false, port: 80 },
        telnet: ProtocolEntry { protocol_enabled: false, port: 23 },
        ssdp: SsdpProtocol {
            protocol_enabled: false,
            port: 1900,
            notify_multicast_interval_seconds: 600,
            notify_ttl: 2,
            notify_ipv6_scope: "Site",
        },
        virtual_media_proto: ProtocolEntry { protocol_enabled: false, port: 0 },
        kvmip: ProtocolEntry { protocol_enabled: false, port: 0 },
        rdp: ProtocolEntry { protocol_enabled: false, port: 3389 },
        rfb: ProtocolEntry { protocol_enabled: false, port: 5900 },
        ftp: ProtocolEntry { protocol_enabled: false, port: 21 },
        sftp: ProtocolEntry { protocol_enabled: false, port: 22 },
        ftps: ProtocolEntry { protocol_enabled: false, port: 990 },
        proxy: ProxyConfig {
            enabled: false,
            proxy_auto_config_uri: "",
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

    let hostname = std::fs::read_to_string("/etc/hostname")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "vbmc".to_string());

    let ipv4 = Ipv4Address {
        address: addr,
        subnet_mask: "255.255.255.0",
        address_origin: "Static",
        gateway: "0.0.0.0",
    };

    Ok(Json(EthernetInterfaceResource {
        odata_id: format!("/redfish/v1/Managers/vbmc/EthernetInterfaces/{nic_id}"),
        odata_type: "#EthernetInterface.v1_12_0.EthernetInterface",
        id: nic_id,
        name: "Manager Ethernet Interface",
        description: "Management network interface",
        mac_address: "02:42:AC:11:00:02",
        permanent_mac_address: "02:42:AC:11:00:02",
        speed_mbps: 1000,
        full_duplex: true,
        mtu_size: 1500,
        interface_enabled: true,
        link_status: "LinkUp",
        auto_neg: true,
        ethernet_interface_type: "Virtual",
        host_name: hostname.clone(),
        fqdn: hostname,
        name_servers: Vec::new(),
        static_name_servers: Vec::new(),
        max_ipv6_static_addresses: 1,
        ipv4_static_addresses: vec![Ipv4Address {
            address: ipv4.address.clone(),
            subnet_mask: "255.255.255.0",
            address_origin: "Static",
            gateway: "0.0.0.0",
        }],
        ipv4_addresses: vec![ipv4],
        ipv6_enabled: false,
        ipv6_addresses: Vec::new(),
        ipv6_static_addresses: Vec::new(),
        ipv6_default_gateway: "",
        ipv6_static_default_gateways: Vec::new(),
        ipv6_address_policy_table: Vec::new(),
        dhcpv4: DhcpV4Config {
            dhcp_enabled: false,
            use_dns_servers: false,
            use_gateway: false,
            use_ntp_servers: false,
            use_domain_name: false,
            use_static_routes: false,
        },
        dhcpv6: DhcpV6Config {
            operating_mode: "Disabled",
            use_dns_servers: false,
            use_ntp_servers: false,
            use_domain_name: false,
            use_rapid_commit: false,
        },
        stateless_address_auto_config: StatelessConfig {
            ipv4_auto_config_enabled: false,
            ipv6_auto_config_enabled: false,
        },
        vlan: VlanConfig {
            vlan_enable: false,
            vlan_id: 0,
            vlan_priority: 0,
            tagged: false,
        },
        links: EthernetLinks {
            chassis: ODataId::new("/redfish/v1/Chassis/1"),
        },
        status: Status::enabled_ok(),
    }))
}
