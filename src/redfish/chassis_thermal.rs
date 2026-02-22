use axum::Json;
use serde::Serialize;

use super::types::{ODataId, Status};

#[derive(Debug, Serialize)]
pub struct ThermalResource {
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
    #[serde(rename = "Temperatures")]
    pub temperatures: Vec<Temperature>,
    #[serde(rename = "Fans")]
    pub fans: Vec<Fan>,
}

#[derive(Debug, Serialize)]
pub struct Temperature {
    #[serde(rename = "@odata.id")]
    pub odata_id: String,
    #[serde(rename = "MemberId")]
    pub member_id: &'static str,
    #[serde(rename = "Name")]
    pub name: &'static str,
    #[serde(rename = "ReadingCelsius")]
    pub reading_celsius: u32,
    #[serde(rename = "UpperThresholdCritical")]
    pub upper_threshold_critical: u32,
    #[serde(rename = "UpperThresholdNonCritical")]
    pub upper_threshold_non_critical: u32,
    #[serde(rename = "LowerThresholdCritical")]
    pub lower_threshold_critical: u32,
    #[serde(rename = "LowerThresholdNonCritical")]
    pub lower_threshold_non_critical: u32,
    #[serde(rename = "MinReadingRangeTemp")]
    pub min_reading_range_temp: i32,
    #[serde(rename = "MaxReadingRangeTemp")]
    pub max_reading_range_temp: u32,
    #[serde(rename = "PhysicalContext")]
    pub physical_context: &'static str,
    #[serde(rename = "SensorNumber")]
    pub sensor_number: u32,
    #[serde(rename = "RelatedItem")]
    pub related_item: Vec<ODataId>,
    #[serde(rename = "Status")]
    pub status: Status,
}

#[derive(Debug, Serialize)]
pub struct Fan {
    #[serde(rename = "@odata.id")]
    pub odata_id: String,
    #[serde(rename = "MemberId")]
    pub member_id: &'static str,
    #[serde(rename = "Name")]
    pub name: &'static str,
    #[serde(rename = "Reading")]
    pub reading: u32,
    #[serde(rename = "ReadingUnits")]
    pub reading_units: &'static str,
    #[serde(rename = "PhysicalContext")]
    pub physical_context: &'static str,
    #[serde(rename = "SensorNumber")]
    pub sensor_number: u32,
    #[serde(rename = "Manufacturer")]
    pub manufacturer: &'static str,
    #[serde(rename = "Model")]
    pub model: &'static str,
    #[serde(rename = "SerialNumber")]
    pub serial_number: &'static str,
    #[serde(rename = "PartNumber")]
    pub part_number: &'static str,
    #[serde(rename = "RelatedItem")]
    pub related_item: Vec<ODataId>,
    #[serde(rename = "Redundancy")]
    pub redundancy: Vec<serde_json::Value>,
    #[serde(rename = "Status")]
    pub status: Status,
}

pub async fn get_thermal() -> Json<ThermalResource> {
    Json(ThermalResource {
        odata_id: "/redfish/v1/Chassis/1/Thermal",
        odata_type: "#Thermal.v1_7_2.Thermal",
        id: "Thermal",
        name: "Thermal",
        description: "Thermal sensors and fans",
        temperatures: vec![Temperature {
            odata_id: "/redfish/v1/Chassis/1/Thermal#/Temperatures/0".to_string(),
            member_id: "0",
            name: "CPU Temperature",
            reading_celsius: 35,
            upper_threshold_critical: 90,
            upper_threshold_non_critical: 75,
            lower_threshold_critical: 0,
            lower_threshold_non_critical: 5,
            min_reading_range_temp: -10,
            max_reading_range_temp: 120,
            physical_context: "CPU",
            sensor_number: 1,
            related_item: vec![ODataId::new("/redfish/v1/Chassis/1")],
            status: Status::enabled_ok(),
        }],
        fans: vec![Fan {
            odata_id: "/redfish/v1/Chassis/1/Thermal#/Fans/0".to_string(),
            member_id: "0",
            name: "System Fan",
            reading: 3000,
            reading_units: "RPM",
            physical_context: "Exhaust",
            sensor_number: 10,
            manufacturer: "vbmc-rs",
            model: "Virtual Fan",
            serial_number: "VBMC-FAN-001",
            part_number: "VBMC-FAN",
            related_item: vec![ODataId::new("/redfish/v1/Chassis/1")],
            redundancy: Vec::new(),
            status: Status::enabled_ok(),
        }],
    })
}
