use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use serde::Serialize;

use super::error::RedfishApiError;
use super::types::{Collection, ODataId, Status};
use crate::app_state::AppState;
use crate::auth::AuthenticatedUser;

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
    #[serde(rename = "Sensors")]
    pub sensors: ODataId,
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
    #[serde(rename = "AssetTag")]
    pub asset_tag: &'static str,
    #[serde(rename = "Version")]
    pub version: &'static str,
    #[serde(rename = "PartNumber")]
    pub part_number: &'static str,
    #[serde(rename = "SKU")]
    pub sku: &'static str,
    #[serde(rename = "SparePartNumber")]
    pub spare_part_number: &'static str,
    #[serde(rename = "UUID")]
    pub uuid: String,
    #[serde(rename = "HeightMm")]
    pub height_mm: f64,
    #[serde(rename = "WidthMm")]
    pub width_mm: f64,
    #[serde(rename = "DepthMm")]
    pub depth_mm: f64,
    #[serde(rename = "WeightKg")]
    pub weight_kg: f64,
    #[serde(rename = "EnvironmentalClass")]
    pub environmental_class: &'static str,
    #[serde(rename = "LocationIndicatorActive")]
    pub location_indicator_active: bool,
    #[serde(rename = "MaxPowerWatts")]
    pub max_power_watts: u32,
    #[serde(rename = "MinPowerWatts")]
    pub min_power_watts: u32,
    #[serde(rename = "HotPluggable")]
    pub hot_pluggable: bool,
    #[serde(rename = "Replaceable")]
    pub replaceable: bool,
    #[serde(rename = "ThermalDirection")]
    pub thermal_direction: &'static str,
    #[serde(rename = "ThermalManagedByParent")]
    pub thermal_managed_by_parent: bool,
    #[serde(rename = "PoweredByParent")]
    pub powered_by_parent: bool,
    #[serde(rename = "ElectricalSourceManagerURIs")]
    pub electrical_source_manager_uris: Vec<&'static str>,
    #[serde(rename = "ElectricalSourceNames")]
    pub electrical_source_names: Vec<&'static str>,
    #[serde(rename = "Location")]
    pub location: super::types::RedfishLocation,
    #[serde(rename = "PhysicalSecurity")]
    pub physical_security: PhysicalSecurity,
    #[serde(rename = "Links")]
    pub links: ChassisLinks,
}

#[derive(Debug, Serialize)]
pub struct PhysicalSecurity {
    #[serde(rename = "IntrusionSensorNumber")]
    pub intrusion_sensor_number: u32,
    #[serde(rename = "IntrusionSensor")]
    pub intrusion_sensor: &'static str,
    #[serde(rename = "IntrusionSensorReArm")]
    pub intrusion_sensor_re_arm: &'static str,
}

#[derive(Debug, Serialize)]
pub struct ChassisLinks {
    #[serde(rename = "ComputerSystems")]
    pub computer_systems: Vec<ODataId>,
    #[serde(rename = "ManagedBy")]
    pub managed_by: Vec<ODataId>,
    #[serde(rename = "ManagersInChassis")]
    pub managers_in_chassis: Vec<ODataId>,
    #[serde(rename = "Drives")]
    pub drives: Vec<ODataId>,
    #[serde(rename = "Storage")]
    pub storage: Vec<ODataId>,
    #[serde(rename = "Fans")]
    pub fans: Vec<ODataId>,
    #[serde(rename = "PowerSupplies")]
    pub power_supplies: Vec<ODataId>,
    #[serde(rename = "Processors")]
    pub processors: Vec<ODataId>,
    #[serde(rename = "Contains")]
    pub contains: Vec<ODataId>,
}

pub async fn get_chassis_collection(_user: AuthenticatedUser) -> Json<Collection<ODataId>> {
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
    _user: AuthenticatedUser,
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
        chassis_type: "Other",
        status: Status::enabled_ok(),
        trusted_components: ODataId::new("/redfish/v1/Chassis/1/TrustedComponents"),
        power: ODataId::new("/redfish/v1/Chassis/1/Power"),
        thermal: ODataId::new("/redfish/v1/Chassis/1/Thermal"),
        power_subsystem: ODataId::new("/redfish/v1/Chassis/1/PowerSubsystem"),
        thermal_subsystem: ODataId::new("/redfish/v1/Chassis/1/ThermalSubsystem"),
        sensors: ODataId::new("/redfish/v1/Chassis/1/Sensors"),
        network_adapters: ODataId::new("/redfish/v1/Chassis/1/NetworkAdapters"),
        power_state: "On",
        manufacturer: "vbmc-rs",
        model: "Virtual Chassis",
        serial_number: "VBMC-CHASSIS-001",
        asset_tag: "",
        version: "1.0",
        part_number: "VBMC-CHS",
        sku: "VBMC-VIRTUAL",
        spare_part_number: "VBMC-CHS-SPARE",
        uuid: state.instance_uuid.clone(),
        height_mm: 0.0,
        width_mm: 0.0,
        depth_mm: 0.0,
        weight_kg: 0.0,
        environmental_class: "A1",
        location_indicator_active: false,
        max_power_watts: 0,
        min_power_watts: 0,
        hot_pluggable: false,
        replaceable: false,
        thermal_direction: "FrontToBack",
        thermal_managed_by_parent: true,
        powered_by_parent: true,
        electrical_source_manager_uris: Vec::new(),
        electrical_source_names: Vec::new(),
        location: super::types::RedfishLocation::new(
            "Virtual",
            "Virtual",
            "Virtual Chassis",
            "Embedded",
            0,
        ),
        physical_security: PhysicalSecurity {
            intrusion_sensor_number: 1,
            intrusion_sensor: "Normal",
            intrusion_sensor_re_arm: "Manual",
        },
        links: ChassisLinks {
            computer_systems,
            managed_by: vec![ODataId::new("/redfish/v1/Managers/vbmc")],
            managers_in_chassis: vec![ODataId::new("/redfish/v1/Managers/vbmc")],
            drives: Vec::new(),
            storage: Vec::new(),
            fans: vec![ODataId::new(
                "/redfish/v1/Chassis/1/ThermalSubsystem/Fans/0",
            )],
            power_supplies: vec![ODataId::new(
                "/redfish/v1/Chassis/1/PowerSubsystem/PowerSupplies/0",
            )],
            processors: Vec::new(),
            contains: Vec::new(),
        },
    })
}

pub async fn get_trusted_components(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
) -> Json<Collection<ODataId>> {
    let members: Vec<ODataId> = state
        .config
        .systems
        .keys()
        .map(|id| ODataId::new(format!("/redfish/v1/Chassis/1/TrustedComponents/{id}")))
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
    #[serde(rename = "Manufacturer")]
    pub manufacturer: &'static str,
    #[serde(rename = "Model")]
    pub model: &'static str,
    #[serde(rename = "SerialNumber")]
    pub serial_number: String,
    #[serde(rename = "FirmwareVersion")]
    pub firmware_version: &'static str,
    #[serde(rename = "UUID")]
    pub uuid: String,
    #[serde(rename = "PartNumber")]
    pub part_number: &'static str,
    #[serde(rename = "SKU")]
    pub sku: &'static str,
    #[serde(rename = "Links")]
    pub tc_links: TrustedComponentLinks,
    #[serde(rename = "Status")]
    pub status: Status,
}

#[derive(Debug, Serialize)]
pub struct TrustedComponentLinks {
    #[serde(rename = "ComponentIntegrity")]
    pub component_integrity: Vec<ODataId>,
    #[serde(rename = "IntegratedInto")]
    pub integrated_into: ODataId,
    #[serde(rename = "Owner")]
    pub owner: ODataId,
    #[serde(rename = "ComponentsProtected")]
    pub components_protected: Vec<ODataId>,
    #[serde(rename = "ActiveSoftwareImage")]
    pub active_software_image: ODataId,
    #[serde(rename = "SoftwareImages")]
    pub software_images: Vec<ODataId>,
}

pub async fn get_trusted_component(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
    Path(component_id): Path<String>,
) -> Result<Json<TrustedComponentResource>, RedfishApiError> {
    if !state.config.systems.contains_key(&component_id) {
        return Err(RedfishApiError::NotFound(format!(
            "TrustedComponent '{component_id}' not found"
        )));
    }

    Ok(Json(TrustedComponentResource {
        odata_id: format!("/redfish/v1/Chassis/1/TrustedComponents/{component_id}"),
        odata_type: "#TrustedComponent.v1_3_0.TrustedComponent",
        id: component_id.clone(),
        name: format!("Trusted: {component_id}"),
        description: format!("Trusted component: {component_id}"),
        trusted_component_type: "Discrete",
        manufacturer: "vbmc-rs",
        model: "Virtual TPM",
        serial_number: format!("VBMC-TC-{component_id}"),
        firmware_version: env!("CARGO_PKG_VERSION"),
        uuid: uuid::Uuid::new_v5(
            &uuid::Uuid::NAMESPACE_URL,
            format!("vbmc-rs:tc:{component_id}").as_bytes(),
        )
        .to_string(),
        part_number: "VBMC-TC",
        sku: "VBMC-VIRTUAL",
        tc_links: TrustedComponentLinks {
            component_integrity: vec![ODataId::new(format!(
                "/redfish/v1/ComponentIntegrity/{component_id}"
            ))],
            integrated_into: ODataId::new("/redfish/v1/Chassis/1"),
            owner: ODataId::new("/redfish/v1/Chassis/1"),
            components_protected: vec![ODataId::new(format!("/redfish/v1/Systems/{component_id}"))],
            active_software_image: ODataId::new(
                "/redfish/v1/UpdateService/FirmwareInventory/vbmc-rs",
            ),
            software_images: vec![ODataId::new(
                "/redfish/v1/UpdateService/FirmwareInventory/vbmc-rs",
            )],
        },
        status: Status::enabled_ok(),
    }))
}
