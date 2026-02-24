use axum::Json;
use serde::Serialize;

use super::types::{Collection, ODataId, Status};

#[derive(Debug, Serialize)]
pub struct PowerSubsystemResource {
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
    #[serde(rename = "Status")]
    pub status: Status,
    #[serde(rename = "CapacityWatts")]
    pub capacity_watts: u32,
    #[serde(rename = "PowerSupplies")]
    pub power_supplies: ODataId,
}

#[derive(Debug, Serialize)]
pub struct PowerSupplyResource {
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
    #[serde(rename = "PowerCapacityWatts")]
    pub power_capacity_watts: u32,
    #[serde(rename = "PowerSupplyType")]
    pub power_supply_type: &'static str,
    #[serde(rename = "Manufacturer")]
    pub manufacturer: &'static str,
    #[serde(rename = "Model")]
    pub model: &'static str,
    #[serde(rename = "SerialNumber")]
    pub serial_number: &'static str,
    #[serde(rename = "PartNumber")]
    pub part_number: &'static str,
    #[serde(rename = "FirmwareVersion")]
    pub firmware_version: &'static str,
    #[serde(rename = "InputNominalVoltageType")]
    pub input_nominal_voltage_type: &'static str,
    #[serde(rename = "HotPluggable")]
    pub hot_pluggable: bool,
    #[serde(rename = "Location")]
    pub location: super::types::RedfishLocation,
    #[serde(rename = "LineInputStatus")]
    pub line_input_status: &'static str,
    #[serde(rename = "OutputNominalVoltageType")]
    pub output_nominal_voltage_type: &'static str,
    #[serde(rename = "PhaseWiringType")]
    pub phase_wiring_type: &'static str,
    #[serde(rename = "Replaceable")]
    pub replaceable: bool,
    #[serde(rename = "ProductionDate")]
    pub production_date: &'static str,
    #[serde(rename = "LocationIndicatorActive")]
    pub location_indicator_active: bool,
    #[serde(rename = "SparePartNumber")]
    pub spare_part_number: &'static str,
    #[serde(rename = "Version")]
    pub version: &'static str,
    #[serde(rename = "EfficiencyRatings")]
    pub efficiency_ratings: Vec<PsuEfficiencyRating>,
    #[serde(rename = "Links")]
    pub psu_links: PsuLinks,
    #[serde(rename = "Status")]
    pub status: Status,
}

#[derive(Debug, Serialize)]
pub struct PsuEfficiencyRating {
    #[serde(rename = "LoadPercent")]
    pub load_percent: u32,
    #[serde(rename = "EfficiencyPercent")]
    pub efficiency_percent: u32,
}

#[derive(Debug, Serialize)]
pub struct PsuLinks {
    #[serde(rename = "Oem")]
    pub oem: serde_json::Value,
}

pub async fn get_power_subsystem() -> Json<PowerSubsystemResource> {
    Json(PowerSubsystemResource {
        odata_id: "/redfish/v1/Chassis/1/PowerSubsystem",
        odata_type: "#PowerSubsystem.v1_1_0.PowerSubsystem",
        id: "PowerSubsystem",
        name: "Power Subsystem",
        description: "Power subsystem for virtual chassis",
        status: Status::enabled_ok(),
        capacity_watts: 1000,
        power_supplies: ODataId::new("/redfish/v1/Chassis/1/PowerSubsystem/PowerSupplies"),
    })
}

pub async fn get_power_supplies() -> Json<Collection<ODataId>> {
    let members = vec![ODataId::new(
        "/redfish/v1/Chassis/1/PowerSubsystem/PowerSupplies/0",
    )];

    Json(Collection::new(
        "/redfish/v1/Chassis/1/PowerSubsystem/PowerSupplies",
        "#PowerSupplyCollection.PowerSupplyCollection",
        "Power Supply Collection",
        members,
    ))
}

pub async fn get_power_supply() -> Json<PowerSupplyResource> {
    Json(PowerSupplyResource {
        odata_id: "/redfish/v1/Chassis/1/PowerSubsystem/PowerSupplies/0",
        odata_type: "#PowerSupply.v1_5_0.PowerSupply",
        id: "0",
        name: "Virtual PSU",
        description: "Virtual power supply unit",
        power_capacity_watts: 500,
        power_supply_type: "AC",
        manufacturer: "vbmc-rs",
        model: "Virtual PSU",
        serial_number: "VBMC-PSU-001",
        part_number: "VBMC-PSU",
        firmware_version: "1.0",
        input_nominal_voltage_type: "AC240V",
        hot_pluggable: false,
        location: super::types::RedfishLocation::new("Bay 1", "Bay", "PSU 0", "Bay", 0),
        line_input_status: "Normal",
        output_nominal_voltage_type: "DC12V",
        phase_wiring_type: "OnePhase3Wire",
        replaceable: false,
        production_date: "2026-01-01T00:00:00Z",
        location_indicator_active: false,
        spare_part_number: "VBMC-PSU-SPARE",
        version: "1.0",
        efficiency_ratings: vec![
            PsuEfficiencyRating { load_percent: 50, efficiency_percent: 90 },
            PsuEfficiencyRating { load_percent: 100, efficiency_percent: 85 },
        ],
        psu_links: PsuLinks {
            oem: serde_json::json!({}),
        },
        status: Status::enabled_ok(),
    })
}
