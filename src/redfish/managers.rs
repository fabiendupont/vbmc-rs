use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use serde::Serialize;

use super::error::RedfishApiError;
use super::types::{Collection, ODataId, Status};
use crate::app_state::AppState;
use crate::auth::AuthenticatedUser;

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
            manager_for_chassis: vec![ODataId::new("/redfish/v1/Chassis/1")],
            manager_in_chassis: ODataId::new("/redfish/v1/Chassis/1"),
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
    }))
}
