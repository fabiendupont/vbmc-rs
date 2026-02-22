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
    pub location: PsuLocation,
    #[serde(rename = "LineInputStatus")]
    pub line_input_status: &'static str,
    #[serde(rename = "Status")]
    pub status: Status,
}

#[derive(Debug, Serialize)]
pub struct PsuLocation {
    #[serde(rename = "Info")]
    pub info: &'static str,
    #[serde(rename = "InfoFormat")]
    pub info_format: &'static str,
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
        location: PsuLocation {
            info: "Bay 1",
            info_format: "Bay",
        },
        line_input_status: "Normal",
        status: Status::enabled_ok(),
    })
}
