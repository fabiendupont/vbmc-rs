use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use serde::Serialize;

use super::error::RedfishApiError;
use super::types::{Collection, ODataId, Status};
use crate::app_state::AppState;

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
    #[serde(rename = "LogServices", skip_serializing_if = "Option::is_none")]
    pub log_services: Option<ODataId>,
    #[serde(rename = "Links")]
    pub links: ManagerLinks,
}

#[derive(Debug, Serialize)]
pub struct ManagerLinks {
    #[serde(rename = "ManagerForServers")]
    pub manager_for_servers: Vec<ODataId>,
    #[serde(rename = "ManagerForChassis")]
    pub manager_for_chassis: Vec<ODataId>,
}

pub async fn get_managers() -> Json<Collection<ODataId>> {
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
        date_time: now,
        date_time_local_offset: "+00:00",
        uuid,
        power_state: "On",
        model: "Virtual BMC",
        manufacturer: "vbmc-rs",
        serial_number: serial,
        log_services: Some(ODataId::new("/redfish/v1/Managers/vbmc/LogServices")),
        links: ManagerLinks {
            manager_for_servers,
            manager_for_chassis: vec![ODataId::new("/redfish/v1/Chassis/1")],
        },
    }))
}
