use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use serde::Serialize;

use super::error::RedfishApiError;
use super::types::{Collection, ODataId, Status};
use crate::app_state::AppState;

#[derive(Debug, Serialize)]
pub struct ChassisResource {
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
    #[serde(rename = "ChassisType")]
    pub chassis_type: &'static str,
    #[serde(rename = "Status")]
    pub status: Status,
    #[serde(rename = "TrustedComponents")]
    pub trusted_components: ODataId,
    #[serde(rename = "Power")]
    pub power: ODataId,
    #[serde(rename = "Thermal")]
    pub thermal: ODataId,
    #[serde(rename = "PowerSubsystem")]
    pub power_subsystem: ODataId,
    #[serde(rename = "ThermalSubsystem")]
    pub thermal_subsystem: ODataId,
    #[serde(rename = "NetworkAdapters")]
    pub network_adapters: ODataId,
    #[serde(rename = "PowerState")]
    pub power_state: &'static str,
    #[serde(rename = "Manufacturer")]
    pub manufacturer: &'static str,
    #[serde(rename = "Model")]
    pub model: &'static str,
    #[serde(rename = "SerialNumber")]
    pub serial_number: &'static str,
    #[serde(rename = "Links")]
    pub links: ChassisLinks,
}

#[derive(Debug, Serialize)]
pub struct ChassisLinks {
    #[serde(rename = "ComputerSystems")]
    pub computer_systems: Vec<ODataId>,
    #[serde(rename = "ManagedBy")]
    pub managed_by: Vec<ODataId>,
}

pub async fn get_chassis_collection() -> Json<Collection<ODataId>> {
    let members = vec![ODataId::new("/redfish/v1/Chassis/1")];
    Json(Collection::new(
        "/redfish/v1/Chassis",
        "#ChassisCollection.ChassisCollection",
        "Chassis Collection",
        members,
    ))
}

pub async fn get_chassis(
    State(state): State<Arc<AppState>>,
) -> Json<ChassisResource> {
    let computer_systems: Vec<ODataId> = state
        .config
        .systems
        .keys()
        .map(|id| ODataId::new(format!("/redfish/v1/Systems/{id}")))
        .collect();

    Json(ChassisResource {
        odata_id: "/redfish/v1/Chassis/1".to_string(),
        odata_type: "#Chassis.v1_25_0.Chassis",
        id: "1",
        name: "Virtual Chassis",
        description: "Virtual chassis for vbmc-rs managed VMs",
        chassis_type: "RackMount",
        status: Status::enabled_ok(),
        trusted_components: ODataId::new("/redfish/v1/Chassis/1/TrustedComponents"),
        power: ODataId::new("/redfish/v1/Chassis/1/Power"),
        thermal: ODataId::new("/redfish/v1/Chassis/1/Thermal"),
        power_subsystem: ODataId::new("/redfish/v1/Chassis/1/PowerSubsystem"),
        thermal_subsystem: ODataId::new("/redfish/v1/Chassis/1/ThermalSubsystem"),
        network_adapters: ODataId::new("/redfish/v1/Chassis/1/NetworkAdapters"),
        power_state: "On",
        manufacturer: "vbmc-rs",
        model: "Virtual Chassis",
        serial_number: "VBMC-CHASSIS-001",
        links: ChassisLinks {
            computer_systems,
            managed_by: vec![ODataId::new("/redfish/v1/Managers/vbmc")],
        },
    })
}

pub async fn get_trusted_components(
    State(state): State<Arc<AppState>>,
) -> Json<Collection<ODataId>> {
    let members: Vec<ODataId> = state
        .config
        .systems
        .keys()
        .map(|id| {
            ODataId::new(format!(
                "/redfish/v1/Chassis/1/TrustedComponents/{id}"
            ))
        })
        .collect();

    Json(Collection::new(
        "/redfish/v1/Chassis/1/TrustedComponents",
        "#TrustedComponentCollection.TrustedComponentCollection",
        "Trusted Component Collection",
        members,
    ))
}

#[derive(Debug, Serialize)]
pub struct TrustedComponentResource {
    #[serde(rename = "@odata.id")]
    pub odata_id: String,
    #[serde(rename = "@odata.type")]
    pub odata_type: &'static str,
    #[serde(rename = "Id")]
    pub id: String,
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Description")]
    pub description: String,
    #[serde(rename = "TrustedComponentType")]
    pub trusted_component_type: &'static str,
    #[serde(rename = "Status")]
    pub status: Status,
}

pub async fn get_trusted_component(
    State(state): State<Arc<AppState>>,
    Path(component_id): Path<String>,
) -> Result<Json<TrustedComponentResource>, RedfishApiError> {
    if !state.config.systems.contains_key(&component_id) {
        return Err(RedfishApiError::NotFound(format!(
            "TrustedComponent '{component_id}' not found"
        )));
    }

    Ok(Json(TrustedComponentResource {
        odata_id: format!(
            "/redfish/v1/Chassis/1/TrustedComponents/{component_id}"
        ),
        odata_type: "#TrustedComponent.v1_3_0.TrustedComponent",
        id: component_id.clone(),
        name: format!("Trusted: {component_id}"),
        description: format!("Trusted component: {component_id}"),
        trusted_component_type: "Discrete",
        status: Status::enabled_ok(),
    }))
}
