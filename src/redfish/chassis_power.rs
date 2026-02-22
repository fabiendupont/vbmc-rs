use axum::Json;
use serde::Serialize;

use super::types::{ODataId, Status};

#[derive(Debug, Serialize)]
pub struct PowerResource {
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
    #[serde(rename = "PowerControl")]
    pub power_control: Vec<PowerControl>,
    #[serde(rename = "PowerSupplies")]
    pub power_supplies: Vec<PowerSupply>,
}

#[derive(Debug, Serialize)]
pub struct PowerControl {
    #[serde(rename = "@odata.id")]
    pub odata_id: String,
    #[serde(rename = "MemberId")]
    pub member_id: &'static str,
    #[serde(rename = "Name")]
    pub name: &'static str,
    #[serde(rename = "PowerConsumedWatts")]
    pub power_consumed_watts: u32,
    #[serde(rename = "PowerCapacityWatts")]
    pub power_capacity_watts: u32,
    #[serde(rename = "PhysicalContext")]
    pub physical_context: &'static str,
    #[serde(rename = "PowerMetrics")]
    pub power_metrics: PowerMetrics,
    #[serde(rename = "PowerLimit")]
    pub power_limit: PowerLimit,
    #[serde(rename = "RelatedItem")]
    pub related_item: Vec<ODataId>,
    #[serde(rename = "Status")]
    pub status: Status,
}

#[derive(Debug, Serialize)]
pub struct PowerMetrics {
    #[serde(rename = "IntervalInMin")]
    pub interval_in_min: u32,
    #[serde(rename = "MinConsumedWatts")]
    pub min_consumed_watts: u32,
    #[serde(rename = "MaxConsumedWatts")]
    pub max_consumed_watts: u32,
    #[serde(rename = "AverageConsumedWatts")]
    pub average_consumed_watts: u32,
}

#[derive(Debug, Serialize)]
pub struct PowerLimit {
    #[serde(rename = "LimitInWatts")]
    pub limit_in_watts: u32,
    #[serde(rename = "LimitException")]
    pub limit_exception: &'static str,
}

#[derive(Debug, Serialize)]
pub struct PowerSupply {
    #[serde(rename = "@odata.id")]
    pub odata_id: String,
    #[serde(rename = "MemberId")]
    pub member_id: &'static str,
    #[serde(rename = "Name")]
    pub name: &'static str,
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
    #[serde(rename = "LineInputVoltage")]
    pub line_input_voltage: u32,
    #[serde(rename = "LineInputVoltageType")]
    pub line_input_voltage_type: &'static str,
    #[serde(rename = "LastPowerOutputWatts")]
    pub last_power_output_watts: u32,
    #[serde(rename = "PowerInputWatts")]
    pub power_input_watts: u32,
    #[serde(rename = "PowerOutputWatts")]
    pub power_output_watts: u32,
    #[serde(rename = "EfficiencyPercent")]
    pub efficiency_percent: u32,
    #[serde(rename = "RelatedItem")]
    pub related_item: Vec<ODataId>,
    #[serde(rename = "Status")]
    pub status: Status,
}

pub async fn get_power() -> Json<PowerResource> {
    Json(PowerResource {
        odata_id: "/redfish/v1/Chassis/1/Power",
        odata_type: "#Power.v1_7_2.Power",
        id: "Power",
        name: "Power",
        description: "Power consumption and supplies",
        power_control: vec![PowerControl {
            odata_id: "/redfish/v1/Chassis/1/Power#/PowerControl/0".to_string(),
            member_id: "0",
            name: "System Power Control",
            power_consumed_watts: 50,
            power_capacity_watts: 500,
            physical_context: "Chassis",
            power_metrics: PowerMetrics {
                interval_in_min: 1,
                min_consumed_watts: 30,
                max_consumed_watts: 100,
                average_consumed_watts: 50,
            },
            power_limit: PowerLimit {
                limit_in_watts: 500,
                limit_exception: "LogEventOnly",
            },
            related_item: vec![ODataId::new("/redfish/v1/Chassis/1")],
            status: Status::enabled_ok(),
        }],
        power_supplies: vec![PowerSupply {
            odata_id: "/redfish/v1/Chassis/1/Power#/PowerSupplies/0".to_string(),
            member_id: "0",
            name: "Virtual PSU",
            power_capacity_watts: 500,
            power_supply_type: "AC",
            manufacturer: "vbmc-rs",
            model: "Virtual PSU",
            serial_number: "VBMC-PSU-001",
            part_number: "VBMC-PSU",
            firmware_version: "1.0",
            line_input_voltage: 220,
            line_input_voltage_type: "ACMidLine",
            last_power_output_watts: 50,
            power_input_watts: 55,
            power_output_watts: 50,
            efficiency_percent: 90,
            related_item: vec![ODataId::new("/redfish/v1/Chassis/1")],
            status: Status::enabled_ok(),
        }],
    })
}
