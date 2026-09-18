use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use serde::Serialize;

use super::error::RedfishApiError;
use super::types::{Collection, ODataId, Status};
use crate::app_state::AppState;
use crate::auth::AuthenticatedUser;

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
    pub snmp: SnmpConfig,
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
pub struct SnmpConfig {
    #[serde(rename = "ProtocolEnabled")]
    pub protocol_enabled: bool,
    #[serde(rename = "Port")]
    pub port: u16,
    #[serde(rename = "EnableSNMPv1")]
    pub enable_snmpv1: bool,
    #[serde(rename = "EnableSNMPv2c")]
    pub enable_snmpv2c: bool,
    #[serde(rename = "EnableSNMPv3")]
    pub enable_snmpv3: bool,
    #[serde(rename = "EngineId")]
    pub engine_id: SnmpEngineId,
    #[serde(rename = "AuthenticationProtocol")]
    pub authentication_protocol: &'static str,
    #[serde(rename = "EncryptionProtocol")]
    pub encryption_protocol: &'static str,
    #[serde(rename = "HideCommunityStrings")]
    pub hide_community_strings: bool,
    #[serde(rename = "CommunityStrings")]
    pub community_strings: Vec<SnmpCommunityString>,
    #[serde(rename = "TrapPort")]
    pub trap_port: u16,
}

#[derive(Debug, Serialize)]
pub struct SnmpEngineId {
    #[serde(rename = "EnterpriseSpecificMethod")]
    pub enterprise_specific_method: &'static str,
}

#[derive(Debug, Serialize)]
pub struct SnmpCommunityString {
    #[serde(rename = "CommunityString")]
    pub community_string: &'static str,
    #[serde(rename = "AccessMode")]
    pub access_mode: &'static str,
    #[serde(rename = "Name")]
    pub name: &'static str,
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
    #[serde(rename = "IPv6DefaultGateway", skip_serializing_if = "Option::is_none")]
    pub ipv6_default_gateway: Option<&'static str>,
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
    _user: AuthenticatedUser,
) -> Json<NetworkProtocolResource> {
    let hostname = std::fs::read_to_string("/etc/hostname")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "vbmc".to_string());
    let port = state.config.server.port;

    Json(NetworkProtocolResource {
        odata_id: "/redfish/v1/Managers/vbmc/NetworkProtocol",
        odata_type: "#ManagerNetworkProtocol.v1_13_0.ManagerNetworkProtocol",
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
        ntp: ProtocolEntry {
            protocol_enabled: false,
            port: 123,
        },
        dhcp: ProtocolEntry {
            protocol_enabled: false,
            port: 67,
        },
        dhcpv6_proto: ProtocolEntry {
            protocol_enabled: false,
            port: 547,
        },
        snmp: SnmpConfig {
            protocol_enabled: false,
            port: 161,
            enable_snmpv1: false,
            enable_snmpv2c: false,
            enable_snmpv3: false,
            engine_id: SnmpEngineId {
                enterprise_specific_method: "76 62 6D 63 2D 72 73 00",
            },
            authentication_protocol: "None",
            encryption_protocol: "CBC_DES",
            hide_community_strings: true,
            community_strings: vec![SnmpCommunityString {
                community_string: "public",
                access_mode: "Limited",
                name: "default",
            }],
            trap_port: 162,
        },
        http: ProtocolEntry {
            protocol_enabled: false,
            port: 80,
        },
        telnet: ProtocolEntry {
            protocol_enabled: false,
            port: 23,
        },
        ssdp: SsdpProtocol {
            protocol_enabled: false,
            port: 1900,
            notify_multicast_interval_seconds: 600,
            notify_ttl: 2,
            notify_ipv6_scope: "Site",
        },
        virtual_media_proto: ProtocolEntry {
            protocol_enabled: false,
            port: 0,
        },
        kvmip: ProtocolEntry {
            protocol_enabled: false,
            port: 0,
        },
        rdp: ProtocolEntry {
            protocol_enabled: false,
            port: 3389,
        },
        rfb: ProtocolEntry {
            protocol_enabled: false,
            port: 5900,
        },
        ftp: ProtocolEntry {
            protocol_enabled: false,
            port: 21,
        },
        sftp: ProtocolEntry {
            protocol_enabled: false,
            port: 22,
        },
        ftps: ProtocolEntry {
            protocol_enabled: false,
            port: 990,
        },
        proxy: ProxyConfig {
            enabled: false,
            proxy_auto_config_uri: "",
        },
        status: Status::enabled_ok(),
    })
}

pub async fn get_manager_ethernet_interfaces(
    _user: AuthenticatedUser,
) -> Json<Collection<ODataId>> {
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
    _user: AuthenticatedUser,
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
        ipv6_default_gateway: None,
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
            chassis: ODataId::new(format!("/redfish/v1/Chassis/{}", state.chassis_id)),
        },
        status: Status::enabled_ok(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protocol_entry_serialization() {
        let entry = ProtocolEntry {
            protocol_enabled: true,
            port: 443,
        };

        let json = serde_json::to_value(&entry).unwrap();
        assert_eq!(json["ProtocolEnabled"], true);
        assert_eq!(json["Port"], 443);
    }

    #[test]
    fn test_ssdp_protocol_serialization() {
        let ssdp = SsdpProtocol {
            protocol_enabled: true,
            port: 1900,
            notify_multicast_interval_seconds: 600,
            notify_ttl: 2,
            notify_ipv6_scope: "Site",
        };

        let json = serde_json::to_value(&ssdp).unwrap();
        assert_eq!(json["ProtocolEnabled"], true);
        assert_eq!(json["Port"], 1900);
        assert_eq!(json["NotifyMulticastIntervalSeconds"], 600);
        assert_eq!(json["NotifyTTL"], 2);
        assert_eq!(json["NotifyIPv6Scope"], "Site");
    }

    #[test]
    fn test_proxy_config_serialization() {
        let proxy = ProxyConfig {
            enabled: true,
            proxy_auto_config_uri: "http://proxy.example.com/wpad.dat",
        };

        let json = serde_json::to_value(&proxy).unwrap();
        assert_eq!(json["Enabled"], true);
        assert_eq!(
            json["ProxyAutoConfigURI"],
            "http://proxy.example.com/wpad.dat"
        );
    }

    #[test]
    fn test_snmp_engine_id_serialization() {
        let engine_id = SnmpEngineId {
            enterprise_specific_method: "76 62 6D 63 2D 72 73 00",
        };

        let json = serde_json::to_value(&engine_id).unwrap();
        assert_eq!(json["EnterpriseSpecificMethod"], "76 62 6D 63 2D 72 73 00");
    }

    #[test]
    fn test_snmp_community_string_serialization() {
        let community = SnmpCommunityString {
            community_string: "public",
            access_mode: "Limited",
            name: "default",
        };

        let json = serde_json::to_value(&community).unwrap();
        assert_eq!(json["CommunityString"], "public");
        assert_eq!(json["AccessMode"], "Limited");
        assert_eq!(json["Name"], "default");
    }

    #[test]
    fn test_snmp_config_serialization() {
        let snmp = SnmpConfig {
            protocol_enabled: true,
            port: 161,
            enable_snmpv1: false,
            enable_snmpv2c: true,
            enable_snmpv3: true,
            engine_id: SnmpEngineId {
                enterprise_specific_method: "76 62 6D 63 2D 72 73 00",
            },
            authentication_protocol: "HMAC_SHA96",
            encryption_protocol: "CBC_DES",
            hide_community_strings: true,
            community_strings: vec![SnmpCommunityString {
                community_string: "public",
                access_mode: "Limited",
                name: "default",
            }],
            trap_port: 162,
        };

        let json = serde_json::to_value(&snmp).unwrap();
        assert_eq!(json["ProtocolEnabled"], true);
        assert_eq!(json["Port"], 161);
        assert_eq!(json["EnableSNMPv1"], false);
        assert_eq!(json["EnableSNMPv2c"], true);
        assert_eq!(json["EnableSNMPv3"], true);
        assert_eq!(
            json["EngineId"]["EnterpriseSpecificMethod"],
            "76 62 6D 63 2D 72 73 00"
        );
        assert_eq!(json["AuthenticationProtocol"], "HMAC_SHA96");
        assert_eq!(json["EncryptionProtocol"], "CBC_DES");
        assert_eq!(json["HideCommunityStrings"], true);
        assert_eq!(json["CommunityStrings"][0]["CommunityString"], "public");
        assert_eq!(json["TrapPort"], 162);
    }

    #[test]
    fn test_ipv4_address_serialization() {
        let ipv4 = Ipv4Address {
            address: "192.168.1.100".to_string(),
            subnet_mask: "255.255.255.0",
            address_origin: "Static",
            gateway: "192.168.1.1",
        };

        let json = serde_json::to_value(&ipv4).unwrap();
        assert_eq!(json["Address"], "192.168.1.100");
        assert_eq!(json["SubnetMask"], "255.255.255.0");
        assert_eq!(json["AddressOrigin"], "Static");
        assert_eq!(json["Gateway"], "192.168.1.1");
    }

    #[test]
    fn test_dhcpv4_config_serialization() {
        let dhcp = DhcpV4Config {
            dhcp_enabled: true,
            use_dns_servers: true,
            use_gateway: true,
            use_ntp_servers: false,
            use_domain_name: true,
            use_static_routes: false,
        };

        let json = serde_json::to_value(&dhcp).unwrap();
        assert_eq!(json["DHCPEnabled"], true);
        assert_eq!(json["UseDNSServers"], true);
        assert_eq!(json["UseGateway"], true);
        assert_eq!(json["UseNTPServers"], false);
        assert_eq!(json["UseDomainName"], true);
        assert_eq!(json["UseStaticRoutes"], false);
    }

    #[test]
    fn test_dhcpv6_config_serialization() {
        let dhcp = DhcpV6Config {
            operating_mode: "Stateful",
            use_dns_servers: true,
            use_ntp_servers: true,
            use_domain_name: false,
            use_rapid_commit: true,
        };

        let json = serde_json::to_value(&dhcp).unwrap();
        assert_eq!(json["OperatingMode"], "Stateful");
        assert_eq!(json["UseDNSServers"], true);
        assert_eq!(json["UseNTPServers"], true);
        assert_eq!(json["UseDomainName"], false);
        assert_eq!(json["UseRapidCommit"], true);
    }

    #[test]
    fn test_stateless_config_serialization() {
        let config = StatelessConfig {
            ipv4_auto_config_enabled: false,
            ipv6_auto_config_enabled: true,
        };

        let json = serde_json::to_value(&config).unwrap();
        assert_eq!(json["IPv4AutoConfigEnabled"], false);
        assert_eq!(json["IPv6AutoConfigEnabled"], true);
    }

    #[test]
    fn test_vlan_config_serialization() {
        let vlan = VlanConfig {
            vlan_enable: true,
            vlan_id: 100,
            vlan_priority: 7,
            tagged: true,
        };

        let json = serde_json::to_value(&vlan).unwrap();
        assert_eq!(json["VLANEnable"], true);
        assert_eq!(json["VLANId"], 100);
        assert_eq!(json["VLANPriority"], 7);
        assert_eq!(json["Tagged"], true);
    }

    #[test]
    fn test_ethernet_links_serialization() {
        let links = EthernetLinks {
            chassis: ODataId::new("/redfish/v1/Chassis/ch1"),
        };

        let json = serde_json::to_value(&links).unwrap();
        assert_eq!(json["Chassis"]["@odata.id"], "/redfish/v1/Chassis/ch1");
    }

    #[test]
    fn test_network_protocol_resource_serialization() {
        let resource = NetworkProtocolResource {
            odata_id: "/redfish/v1/Managers/vbmc/NetworkProtocol",
            odata_type: "#ManagerNetworkProtocol.v1_13_0.ManagerNetworkProtocol",
            id: "NetworkProtocol",
            name: "Manager Network Protocol",
            description: "Manager network protocol settings",
            host_name: "vbmc-host".to_string(),
            fqdn: "vbmc-host.example.com".to_string(),
            https: ProtocolEntry {
                protocol_enabled: true,
                port: 443,
            },
            ssh: ProtocolEntry {
                protocol_enabled: false,
                port: 22,
            },
            ipmi: ProtocolEntry {
                protocol_enabled: false,
                port: 623,
            },
            ntp: ProtocolEntry {
                protocol_enabled: false,
                port: 123,
            },
            dhcp: ProtocolEntry {
                protocol_enabled: false,
                port: 67,
            },
            dhcpv6_proto: ProtocolEntry {
                protocol_enabled: false,
                port: 547,
            },
            snmp: SnmpConfig {
                protocol_enabled: false,
                port: 161,
                enable_snmpv1: false,
                enable_snmpv2c: false,
                enable_snmpv3: false,
                engine_id: SnmpEngineId {
                    enterprise_specific_method: "76 62 6D 63 2D 72 73 00",
                },
                authentication_protocol: "None",
                encryption_protocol: "CBC_DES",
                hide_community_strings: true,
                community_strings: vec![],
                trap_port: 162,
            },
            http: ProtocolEntry {
                protocol_enabled: false,
                port: 80,
            },
            telnet: ProtocolEntry {
                protocol_enabled: false,
                port: 23,
            },
            ssdp: SsdpProtocol {
                protocol_enabled: false,
                port: 1900,
                notify_multicast_interval_seconds: 600,
                notify_ttl: 2,
                notify_ipv6_scope: "Site",
            },
            virtual_media_proto: ProtocolEntry {
                protocol_enabled: false,
                port: 0,
            },
            kvmip: ProtocolEntry {
                protocol_enabled: false,
                port: 0,
            },
            rdp: ProtocolEntry {
                protocol_enabled: false,
                port: 3389,
            },
            rfb: ProtocolEntry {
                protocol_enabled: false,
                port: 5900,
            },
            ftp: ProtocolEntry {
                protocol_enabled: false,
                port: 21,
            },
            sftp: ProtocolEntry {
                protocol_enabled: false,
                port: 22,
            },
            ftps: ProtocolEntry {
                protocol_enabled: false,
                port: 990,
            },
            proxy: ProxyConfig {
                enabled: false,
                proxy_auto_config_uri: "",
            },
            status: Status::enabled_ok(),
        };

        let json = serde_json::to_value(&resource).unwrap();
        assert_eq!(
            json["@odata.id"],
            "/redfish/v1/Managers/vbmc/NetworkProtocol"
        );
        assert_eq!(
            json["@odata.type"],
            "#ManagerNetworkProtocol.v1_13_0.ManagerNetworkProtocol"
        );
        assert_eq!(json["Id"], "NetworkProtocol");
        assert_eq!(json["HostName"], "vbmc-host");
        assert_eq!(json["FQDN"], "vbmc-host.example.com");
        assert_eq!(json["HTTPS"]["Port"], 443);
        assert_eq!(json["DHCPv6"]["Port"], 547);
        assert_eq!(json["VirtualMedia"]["Port"], 0);
    }

    #[test]
    fn test_ethernet_interface_resource_serialization() {
        let resource = EthernetInterfaceResource {
            odata_id: "/redfish/v1/Managers/vbmc/EthernetInterfaces/mgmt0".to_string(),
            odata_type: "#EthernetInterface.v1_12_0.EthernetInterface",
            id: "mgmt0".to_string(),
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
            host_name: "vbmc-host".to_string(),
            fqdn: "vbmc-host.example.com".to_string(),
            name_servers: Vec::new(),
            static_name_servers: Vec::new(),
            max_ipv6_static_addresses: 1,
            ipv4_addresses: vec![Ipv4Address {
                address: "192.168.1.100".to_string(),
                subnet_mask: "255.255.255.0",
                address_origin: "Static",
                gateway: "192.168.1.1",
            }],
            ipv4_static_addresses: Vec::new(),
            ipv6_enabled: false,
            ipv6_addresses: Vec::new(),
            ipv6_static_addresses: Vec::new(),
            ipv6_default_gateway: None,
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
                chassis: ODataId::new("/redfish/v1/Chassis/ch1"),
            },
            status: Status::enabled_ok(),
        };

        let json = serde_json::to_value(&resource).unwrap();
        assert_eq!(
            json["@odata.id"],
            "/redfish/v1/Managers/vbmc/EthernetInterfaces/mgmt0"
        );
        assert_eq!(
            json["@odata.type"],
            "#EthernetInterface.v1_12_0.EthernetInterface"
        );
        assert_eq!(json["Id"], "mgmt0");
        assert_eq!(json["MACAddress"], "02:42:AC:11:00:02");
        assert_eq!(json["SpeedMbps"], 1000);
        assert_eq!(json["FullDuplex"], true);
        assert_eq!(json["LinkStatus"], "LinkUp");
        assert_eq!(json["IPv4Addresses"][0]["Address"], "192.168.1.100");
        assert_eq!(json["DHCPv4"]["DHCPEnabled"], false);
    }

    #[test]
    fn test_ethernet_interface_ipv6_default_gateway_present() {
        let resource = EthernetInterfaceResource {
            odata_id: "/redfish/v1/Managers/vbmc/EthernetInterfaces/mgmt0".to_string(),
            odata_type: "#EthernetInterface.v1_12_0.EthernetInterface",
            id: "mgmt0".to_string(),
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
            host_name: "vbmc-host".to_string(),
            fqdn: "vbmc-host.example.com".to_string(),
            name_servers: Vec::new(),
            static_name_servers: Vec::new(),
            max_ipv6_static_addresses: 1,
            ipv4_addresses: Vec::new(),
            ipv4_static_addresses: Vec::new(),
            ipv6_enabled: true,
            ipv6_addresses: Vec::new(),
            ipv6_static_addresses: Vec::new(),
            ipv6_default_gateway: Some("fe80::1"),
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
                chassis: ODataId::new("/redfish/v1/Chassis/ch1"),
            },
            status: Status::enabled_ok(),
        };

        let json = serde_json::to_value(&resource).unwrap();
        assert_eq!(json["IPv6DefaultGateway"], "fe80::1");
    }

    #[test]
    fn test_ethernet_interface_ipv6_default_gateway_absent() {
        let resource = EthernetInterfaceResource {
            odata_id: "/redfish/v1/Managers/vbmc/EthernetInterfaces/mgmt0".to_string(),
            odata_type: "#EthernetInterface.v1_12_0.EthernetInterface",
            id: "mgmt0".to_string(),
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
            host_name: "vbmc-host".to_string(),
            fqdn: "vbmc-host.example.com".to_string(),
            name_servers: Vec::new(),
            static_name_servers: Vec::new(),
            max_ipv6_static_addresses: 1,
            ipv4_addresses: Vec::new(),
            ipv4_static_addresses: Vec::new(),
            ipv6_enabled: false,
            ipv6_addresses: Vec::new(),
            ipv6_static_addresses: Vec::new(),
            ipv6_default_gateway: None,
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
                chassis: ODataId::new("/redfish/v1/Chassis/ch1"),
            },
            status: Status::enabled_ok(),
        };

        let json = serde_json::to_value(&resource).unwrap();
        assert!(json.get("IPv6DefaultGateway").is_none());
    }
}
