use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use serde::{Deserialize, Serialize};

use super::error::RedfishApiError;
use super::types::{Collection, ODataId, Status};
use crate::app_state::AppState;
use crate::auth::AuthenticatedUser;
use crate::auth::rbac::{Privilege, has_privilege};
use crate::state::VmState;

const MANAGER_ID: &str = "vbmc";

#[derive(Debug, Serialize)]
pub struct Manager {
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
    #[serde(rename = "ManagerType")]
    pub manager_type: &'static str,
    #[serde(rename = "FirmwareVersion")]
    pub firmware_version: &'static str,
    #[serde(rename = "Status")]
    pub status: Status,
    #[serde(rename = "DateTime")]
    pub date_time: String,
    #[serde(rename = "DateTimeLocalOffset")]
    pub date_time_local_offset: &'static str,
    #[serde(rename = "UUID")]
    pub uuid: String,
    #[serde(rename = "PowerState")]
    pub power_state: &'static str,
    #[serde(rename = "Model")]
    pub model: &'static str,
    #[serde(rename = "Manufacturer")]
    pub manufacturer: &'static str,
    #[serde(rename = "SerialNumber")]
    pub serial_number: String,
    #[serde(rename = "PartNumber")]
    pub part_number: &'static str,
    #[serde(rename = "SparePartNumber")]
    pub spare_part_number: &'static str,
    #[serde(rename = "Version")]
    pub version: &'static str,
    #[serde(rename = "ServiceEntryPointUUID")]
    pub service_entry_point_uuid: String,
    #[serde(rename = "GraphicalConsole")]
    pub graphical_console: ManagerConsole,
    #[serde(rename = "CommandShell")]
    pub command_shell: ManagerConsole,
    #[serde(rename = "LastResetTime")]
    pub last_reset_time: String,
    #[serde(rename = "LocationIndicatorActive")]
    pub location_indicator_active: bool,
    #[serde(rename = "TimeZoneName")]
    pub time_zone_name: &'static str,
    #[serde(rename = "ServiceIdentification")]
    pub service_identification: String,
    #[serde(rename = "AutoDSTEnabled")]
    pub auto_dst_enabled: bool,
    #[serde(rename = "Location")]
    pub location: super::types::RedfishLocation,
    #[serde(rename = "NetworkProtocol")]
    pub network_protocol: ODataId,
    #[serde(rename = "EthernetInterfaces")]
    pub ethernet_interfaces: ODataId,
    #[serde(rename = "LogServices", skip_serializing_if = "Option::is_none")]
    pub log_services: Option<ODataId>,
    #[serde(rename = "Links")]
    pub links: ManagerLinks,
    #[serde(rename = "Actions")]
    pub actions: ManagerActions,
}

#[derive(Debug, Serialize)]
pub struct ManagerConsole {
    #[serde(rename = "ServiceEnabled")]
    pub service_enabled: bool,
    #[serde(rename = "MaxConcurrentSessions")]
    pub max_concurrent_sessions: u32,
    #[serde(rename = "ConnectTypesSupported")]
    pub connect_types_supported: Vec<&'static str>,
}

#[derive(Debug, Serialize)]
pub struct ManagerActions {
    #[serde(rename = "#Manager.ResetToDefaults")]
    pub reset_to_defaults: ManagerResetToDefaultsAction,
}

#[derive(Debug, Serialize)]
pub struct ManagerResetToDefaultsAction {
    pub target: String,
    #[serde(rename = "ResetType@Redfish.AllowableValues")]
    pub allowable_values: Vec<&'static str>,
}

#[derive(Debug, Deserialize)]
pub struct ResetToDefaultsRequest {
    #[serde(rename = "ResetType")]
    pub reset_type: String,
}

#[derive(Debug, Serialize)]
pub struct ManagerLinks {
    #[serde(rename = "ManagerForServers")]
    pub manager_for_servers: Vec<ODataId>,
    #[serde(rename = "ManagerForChassis")]
    pub manager_for_chassis: Vec<ODataId>,
    #[serde(rename = "ManagerInChassis")]
    pub manager_in_chassis: ODataId,
    #[serde(rename = "ManagedBy")]
    pub managed_by: Vec<ODataId>,
    #[serde(rename = "ManagerForManagers")]
    pub manager_for_managers: Vec<ODataId>,
    #[serde(rename = "ManagerForSwitches")]
    pub manager_for_switches: Vec<ODataId>,
    #[serde(rename = "ActiveSoftwareImage")]
    pub active_software_image: ODataId,
    #[serde(rename = "SoftwareImages")]
    pub software_images: Vec<ODataId>,
}

pub async fn get_managers(_user: AuthenticatedUser) -> Json<Collection<ODataId>> {
    let members = vec![ODataId::new(format!("/redfish/v1/Managers/{MANAGER_ID}"))];
    Json(Collection::new(
        "/redfish/v1/Managers",
        "#ManagerCollection.ManagerCollection",
        "Manager Collection",
        members,
    ))
}

pub async fn get_manager(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
    Path(manager_id): Path<String>,
) -> Result<Json<Manager>, RedfishApiError> {
    if manager_id != MANAGER_ID {
        return Err(RedfishApiError::NotFound(format!(
            "Manager '{manager_id}' not found"
        )));
    }

    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let uuid = state.instance_uuid.clone();
    let serial = format!("VBMC-{}", uuid.split('-').next().unwrap_or("0000"));

    let manager_for_servers: Vec<ODataId> = state
        .config
        .systems
        .keys()
        .map(|id| ODataId::new(format!("/redfish/v1/Systems/{id}")))
        .collect();

    Ok(Json(Manager {
        odata_id: format!("/redfish/v1/Managers/{MANAGER_ID}"),
        odata_type: "#Manager.v1_19_0.Manager",
        id: MANAGER_ID,
        name: "vbmc-rs Virtual BMC",
        description: "vbmc-rs Virtual Baseboard Management Controller",
        manager_type: "BMC",
        firmware_version: env!("CARGO_PKG_VERSION"),
        status: Status::enabled_ok(),
        date_time: now.clone(),
        date_time_local_offset: "+00:00",
        uuid,
        power_state: "On",
        model: "Virtual BMC",
        manufacturer: "vbmc-rs",
        serial_number: serial,
        part_number: "VBMC-MGR",
        spare_part_number: "VBMC-MGR-SPARE",
        version: env!("CARGO_PKG_VERSION"),
        service_entry_point_uuid: state.instance_uuid.clone(),
        graphical_console: ManagerConsole {
            service_enabled: false,
            max_concurrent_sessions: 0,
            connect_types_supported: Vec::new(),
        },
        command_shell: ManagerConsole {
            service_enabled: false,
            max_concurrent_sessions: 0,
            connect_types_supported: Vec::new(),
        },
        last_reset_time: now.clone(),
        location_indicator_active: false,
        time_zone_name: "UTC",
        service_identification: format!(
            "vbmc-rs-{}",
            state.instance_uuid.split('-').next().unwrap_or("0000")
        ),
        auto_dst_enabled: false,
        location: super::types::RedfishLocation::new("BMC", "Embedded", 0)
            .with_config(&state.config.location),
        network_protocol: ODataId::new("/redfish/v1/Managers/vbmc/NetworkProtocol"),
        ethernet_interfaces: ODataId::new("/redfish/v1/Managers/vbmc/EthernetInterfaces"),
        log_services: Some(ODataId::new("/redfish/v1/Managers/vbmc/LogServices")),
        links: ManagerLinks {
            manager_for_servers,
            manager_for_chassis: vec![ODataId::new(format!(
                "/redfish/v1/Chassis/{}",
                state.chassis_id
            ))],
            manager_in_chassis: ODataId::new(format!("/redfish/v1/Chassis/{}", state.chassis_id)),
            managed_by: Vec::new(),
            manager_for_managers: Vec::new(),
            manager_for_switches: Vec::new(),
            active_software_image: ODataId::new(
                "/redfish/v1/UpdateService/FirmwareInventory/vbmc-rs",
            ),
            software_images: vec![ODataId::new(
                "/redfish/v1/UpdateService/FirmwareInventory/vbmc-rs",
            )],
        },
        actions: ManagerActions {
            reset_to_defaults: ManagerResetToDefaultsAction {
                target: format!(
                    "/redfish/v1/Managers/{MANAGER_ID}/Actions/Manager.ResetToDefaults"
                ),
                allowable_values: vec!["ResetAll", "PreserveNetworkAndUsers", "PreserveNetwork"],
            },
        },
    }))
}

pub async fn reset_to_defaults(
    State(state): State<Arc<AppState>>,
    user: AuthenticatedUser,
    Path(manager_id): Path<String>,
    Json(body): Json<ResetToDefaultsRequest>,
) -> Result<Json<serde_json::Value>, RedfishApiError> {
    if manager_id != MANAGER_ID {
        return Err(RedfishApiError::NotFound(format!(
            "Manager '{manager_id}' not found"
        )));
    }

    if !has_privilege(&user.role, Privilege::ConfigureManager) {
        return Err(RedfishApiError::Forbidden(
            "Insufficient privileges".to_string(),
        ));
    }

    let valid_types = ["ResetAll", "PreserveNetworkAndUsers", "PreserveNetwork"];
    if !valid_types.contains(&body.reset_type.as_str()) {
        return Err(RedfishApiError::BadRequest(format!(
            "Invalid ResetType '{}'. Allowable values: {}",
            body.reset_type,
            valid_types.join(", ")
        )));
    }

    for system_id in state.config.systems.keys() {
        let fresh = VmState::new(system_id);
        state.save_vm_state(system_id, &fresh);
    }

    tracing::info!(reset_type = %body.reset_type, "Manager.ResetToDefaults applied");

    Ok(Json(serde_json::json!({"message": "Manager reset to defaults"})))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manager_serialization() {
        let manager = Manager {
            odata_id: "/redfish/v1/Managers/vbmc".to_string(),
            odata_type: "#Manager.v1_19_0.Manager",
            id: "vbmc",
            name: "vbmc-rs Virtual BMC",
            description: "vbmc-rs Virtual Baseboard Management Controller",
            manager_type: "BMC",
            firmware_version: "1.0.0",
            status: Status::enabled_ok(),
            date_time: "2026-09-17T12:00:00Z".to_string(),
            date_time_local_offset: "+00:00",
            uuid: "00000000-0000-0000-0000-000000000000".to_string(),
            power_state: "On",
            model: "Virtual BMC",
            manufacturer: "vbmc-rs",
            serial_number: "VBMC-0000".to_string(),
            part_number: "VBMC-MGR",
            spare_part_number: "VBMC-MGR-SPARE",
            version: "1.0.0",
            service_entry_point_uuid: "00000000-0000-0000-0000-000000000000".to_string(),
            graphical_console: ManagerConsole {
                service_enabled: false,
                max_concurrent_sessions: 0,
                connect_types_supported: Vec::new(),
            },
            command_shell: ManagerConsole {
                service_enabled: true,
                max_concurrent_sessions: 2,
                connect_types_supported: vec!["SSH"],
            },
            last_reset_time: "2026-09-17T12:00:00Z".to_string(),
            location_indicator_active: false,
            time_zone_name: "UTC",
            service_identification: "vbmc-rs-0000".to_string(),
            auto_dst_enabled: false,
            location: super::super::types::RedfishLocation::new("BMC", "Embedded", 0),
            network_protocol: ODataId::new("/redfish/v1/Managers/vbmc/NetworkProtocol"),
            ethernet_interfaces: ODataId::new("/redfish/v1/Managers/vbmc/EthernetInterfaces"),
            log_services: Some(ODataId::new("/redfish/v1/Managers/vbmc/LogServices")),
            links: ManagerLinks {
                manager_for_servers: vec![ODataId::new("/redfish/v1/Systems/vm1")],
                manager_for_chassis: vec![ODataId::new("/redfish/v1/Chassis/ch1")],
                manager_in_chassis: ODataId::new("/redfish/v1/Chassis/ch1"),
                managed_by: Vec::new(),
                manager_for_managers: Vec::new(),
                manager_for_switches: Vec::new(),
                active_software_image: ODataId::new(
                    "/redfish/v1/UpdateService/FirmwareInventory/vbmc-rs",
                ),
                software_images: vec![ODataId::new(
                    "/redfish/v1/UpdateService/FirmwareInventory/vbmc-rs",
                )],
            },
            actions: ManagerActions {
                reset_to_defaults: ManagerResetToDefaultsAction {
                    target: "/redfish/v1/Managers/vbmc/Actions/Manager.ResetToDefaults"
                        .to_string(),
                    allowable_values: vec!["ResetAll", "PreserveNetworkAndUsers", "PreserveNetwork"],
                },
            },
        };

        let json = serde_json::to_value(&manager).unwrap();

        assert_eq!(json["@odata.id"], "/redfish/v1/Managers/vbmc");
        assert_eq!(json["@odata.type"], "#Manager.v1_19_0.Manager");
        assert_eq!(json["Id"], "vbmc");
        assert_eq!(json["ManagerType"], "BMC");
        assert_eq!(json["CommandShell"]["ServiceEnabled"], true);
        assert_eq!(json["CommandShell"]["MaxConcurrentSessions"], 2);
        assert_eq!(
            json["Links"]["ManagerForServers"][0]["@odata.id"],
            "/redfish/v1/Systems/vm1"
        );
    }

    #[test]
    fn test_manager_console_serialization() {
        let console = ManagerConsole {
            service_enabled: true,
            max_concurrent_sessions: 4,
            connect_types_supported: vec!["SSH", "IPMI"],
        };

        let json = serde_json::to_value(&console).unwrap();

        assert_eq!(json["ServiceEnabled"], true);
        assert_eq!(json["MaxConcurrentSessions"], 4);
        assert_eq!(json["ConnectTypesSupported"][0], "SSH");
        assert_eq!(json["ConnectTypesSupported"][1], "IPMI");
    }

    #[test]
    fn test_manager_links_serialization() {
        let links = ManagerLinks {
            manager_for_servers: vec![ODataId::new("/redfish/v1/Systems/vm1")],
            manager_for_chassis: vec![ODataId::new("/redfish/v1/Chassis/ch1")],
            manager_in_chassis: ODataId::new("/redfish/v1/Chassis/ch1"),
            managed_by: Vec::new(),
            manager_for_managers: Vec::new(),
            manager_for_switches: Vec::new(),
            active_software_image: ODataId::new(
                "/redfish/v1/UpdateService/FirmwareInventory/vbmc-rs",
            ),
            software_images: vec![ODataId::new(
                "/redfish/v1/UpdateService/FirmwareInventory/vbmc-rs",
            )],
        };

        let json = serde_json::to_value(&links).unwrap();

        assert_eq!(
            json["ManagerForServers"][0]["@odata.id"],
            "/redfish/v1/Systems/vm1"
        );
        assert_eq!(
            json["ManagerForChassis"][0]["@odata.id"],
            "/redfish/v1/Chassis/ch1"
        );
        assert_eq!(
            json["ManagerInChassis"]["@odata.id"],
            "/redfish/v1/Chassis/ch1"
        );
        assert_eq!(json["ManagedBy"].as_array().unwrap().len(), 0);
        assert_eq!(
            json["ActiveSoftwareImage"]["@odata.id"],
            "/redfish/v1/UpdateService/FirmwareInventory/vbmc-rs"
        );
    }

    #[test]
    fn test_manager_log_services_present() {
        let manager = Manager {
            odata_id: "/redfish/v1/Managers/vbmc".to_string(),
            odata_type: "#Manager.v1_19_0.Manager",
            id: "vbmc",
            name: "vbmc-rs Virtual BMC",
            description: "vbmc-rs Virtual Baseboard Management Controller",
            manager_type: "BMC",
            firmware_version: "1.0.0",
            status: Status::enabled_ok(),
            date_time: "2026-09-17T12:00:00Z".to_string(),
            date_time_local_offset: "+00:00",
            uuid: "00000000-0000-0000-0000-000000000000".to_string(),
            power_state: "On",
            model: "Virtual BMC",
            manufacturer: "vbmc-rs",
            serial_number: "VBMC-0000".to_string(),
            part_number: "VBMC-MGR",
            spare_part_number: "VBMC-MGR-SPARE",
            version: "1.0.0",
            service_entry_point_uuid: "00000000-0000-0000-0000-000000000000".to_string(),
            graphical_console: ManagerConsole {
                service_enabled: false,
                max_concurrent_sessions: 0,
                connect_types_supported: Vec::new(),
            },
            command_shell: ManagerConsole {
                service_enabled: false,
                max_concurrent_sessions: 0,
                connect_types_supported: Vec::new(),
            },
            last_reset_time: "2026-09-17T12:00:00Z".to_string(),
            location_indicator_active: false,
            time_zone_name: "UTC",
            service_identification: "vbmc-rs-0000".to_string(),
            auto_dst_enabled: false,
            location: super::super::types::RedfishLocation::new("BMC", "Embedded", 0),
            network_protocol: ODataId::new("/redfish/v1/Managers/vbmc/NetworkProtocol"),
            ethernet_interfaces: ODataId::new("/redfish/v1/Managers/vbmc/EthernetInterfaces"),
            log_services: Some(ODataId::new("/redfish/v1/Managers/vbmc/LogServices")),
            links: ManagerLinks {
                manager_for_servers: Vec::new(),
                manager_for_chassis: Vec::new(),
                manager_in_chassis: ODataId::new("/redfish/v1/Chassis/ch1"),
                managed_by: Vec::new(),
                manager_for_managers: Vec::new(),
                manager_for_switches: Vec::new(),
                active_software_image: ODataId::new(
                    "/redfish/v1/UpdateService/FirmwareInventory/vbmc-rs",
                ),
                software_images: Vec::new(),
            },
            actions: ManagerActions {
                reset_to_defaults: ManagerResetToDefaultsAction {
                    target: "/redfish/v1/Managers/vbmc/Actions/Manager.ResetToDefaults"
                        .to_string(),
                    allowable_values: vec!["ResetAll", "PreserveNetworkAndUsers", "PreserveNetwork"],
                },
            },
        };

        let json = serde_json::to_value(&manager).unwrap();
        assert_eq!(
            json["LogServices"]["@odata.id"],
            "/redfish/v1/Managers/vbmc/LogServices"
        );
    }

    #[test]
    fn test_manager_log_services_absent() {
        let manager = Manager {
            odata_id: "/redfish/v1/Managers/vbmc".to_string(),
            odata_type: "#Manager.v1_19_0.Manager",
            id: "vbmc",
            name: "vbmc-rs Virtual BMC",
            description: "vbmc-rs Virtual Baseboard Management Controller",
            manager_type: "BMC",
            firmware_version: "1.0.0",
            status: Status::enabled_ok(),
            date_time: "2026-09-17T12:00:00Z".to_string(),
            date_time_local_offset: "+00:00",
            uuid: "00000000-0000-0000-0000-000000000000".to_string(),
            power_state: "On",
            model: "Virtual BMC",
            manufacturer: "vbmc-rs",
            serial_number: "VBMC-0000".to_string(),
            part_number: "VBMC-MGR",
            spare_part_number: "VBMC-MGR-SPARE",
            version: "1.0.0",
            service_entry_point_uuid: "00000000-0000-0000-0000-000000000000".to_string(),
            graphical_console: ManagerConsole {
                service_enabled: false,
                max_concurrent_sessions: 0,
                connect_types_supported: Vec::new(),
            },
            command_shell: ManagerConsole {
                service_enabled: false,
                max_concurrent_sessions: 0,
                connect_types_supported: Vec::new(),
            },
            last_reset_time: "2026-09-17T12:00:00Z".to_string(),
            location_indicator_active: false,
            time_zone_name: "UTC",
            service_identification: "vbmc-rs-0000".to_string(),
            auto_dst_enabled: false,
            location: super::super::types::RedfishLocation::new("BMC", "Embedded", 0),
            network_protocol: ODataId::new("/redfish/v1/Managers/vbmc/NetworkProtocol"),
            ethernet_interfaces: ODataId::new("/redfish/v1/Managers/vbmc/EthernetInterfaces"),
            log_services: None,
            links: ManagerLinks {
                manager_for_servers: Vec::new(),
                manager_for_chassis: Vec::new(),
                manager_in_chassis: ODataId::new("/redfish/v1/Chassis/ch1"),
                managed_by: Vec::new(),
                manager_for_managers: Vec::new(),
                manager_for_switches: Vec::new(),
                active_software_image: ODataId::new(
                    "/redfish/v1/UpdateService/FirmwareInventory/vbmc-rs",
                ),
                software_images: Vec::new(),
            },
            actions: ManagerActions {
                reset_to_defaults: ManagerResetToDefaultsAction {
                    target: "/redfish/v1/Managers/vbmc/Actions/Manager.ResetToDefaults"
                        .to_string(),
                    allowable_values: vec!["ResetAll", "PreserveNetworkAndUsers", "PreserveNetwork"],
                },
            },
        };

        let json = serde_json::to_value(&manager).unwrap();
        assert!(json.get("LogServices").is_none());
    }

    #[test]
    fn test_manager_actions_serialization() {
        let actions = ManagerActions {
            reset_to_defaults: ManagerResetToDefaultsAction {
                target: "/redfish/v1/Managers/vbmc/Actions/Manager.ResetToDefaults".to_string(),
                allowable_values: vec!["ResetAll", "PreserveNetworkAndUsers", "PreserveNetwork"],
            },
        };
        let json = serde_json::to_value(&actions).unwrap();
        assert_eq!(
            json["#Manager.ResetToDefaults"]["target"],
            "/redfish/v1/Managers/vbmc/Actions/Manager.ResetToDefaults"
        );
        assert_eq!(
            json["#Manager.ResetToDefaults"]["ResetType@Redfish.AllowableValues"][0],
            "ResetAll"
        );
    }

    #[tokio::test]
    async fn test_reset_to_defaults_resets_all_states() {
        use axum::http::Method;
        use crate::redfish::test_harness::{self, systems_with};

        let systems = systems_with("vm1");
        let state = test_harness::app_state(crate::backend::mock::MockBackend::new(), systems);

        // Pre-set some state to verify it gets wiped
        let mut vm_state = state.get_vm_state("vm1");
        vm_state.secure_boot_enabled = true;
        vm_state.boot_override.target = Some("Cd".to_string());
        vm_state.boot_override.enabled = "Once".to_string();
        state.save_vm_state("vm1", &vm_state);

        let app = test_harness::router(state.clone());
        let (status, body, _) = test_harness::request_json(
            &app,
            Method::POST,
            "/redfish/v1/Managers/vbmc/Actions/Manager.ResetToDefaults",
            serde_json::json!({"ResetType": "ResetAll"}),
        )
        .await;

        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(body["message"], "Manager reset to defaults");

        let after = state.get_vm_state("vm1");
        assert!(!after.secure_boot_enabled);
        assert!(after.boot_override.target.is_none());
        assert_eq!(after.boot_override.enabled, "Disabled");
    }

    #[tokio::test]
    async fn test_reset_to_defaults_invalid_type() {
        use axum::http::Method;
        use crate::redfish::test_harness::{self, systems_with};

        let app = test_harness::router_with_systems(systems_with("vm1"));
        let (status, _, _) = test_harness::request_json(
            &app,
            Method::POST,
            "/redfish/v1/Managers/vbmc/Actions/Manager.ResetToDefaults",
            serde_json::json!({"ResetType": "Bogus"}),
        )
        .await;

        assert_eq!(status, axum::http::StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_reset_to_defaults_unknown_manager() {
        use axum::http::Method;
        use crate::redfish::test_harness::{self, systems_with};

        let app = test_harness::router_with_systems(systems_with("vm1"));
        let (status, _, _) = test_harness::request_json(
            &app,
            Method::POST,
            "/redfish/v1/Managers/other/Actions/Manager.ResetToDefaults",
            serde_json::json!({"ResetType": "ResetAll"}),
        )
        .await;

        assert_eq!(status, axum::http::StatusCode::NOT_FOUND);
    }
}
