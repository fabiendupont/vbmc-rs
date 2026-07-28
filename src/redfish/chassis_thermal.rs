use axum::Json;
use serde::Serialize;

use super::types::{ODataId, Status};
use crate::auth::AuthenticatedUser;

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
    #[serde(rename = "Status")]
    pub status: Status,
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
    #[serde(rename = "UpperThresholdFatal")]
    pub upper_threshold_fatal: u32,
    #[serde(rename = "LowerThresholdFatal")]
    pub lower_threshold_fatal: i32,
    #[serde(rename = "MaxAllowableOperatingValue")]
    pub max_allowable_operating_value: u32,
    #[serde(rename = "MinAllowableOperatingValue")]
    pub min_allowable_operating_value: i32,
    #[serde(rename = "AdjustedMaxAllowableOperatingValue")]
    pub adjusted_max_allowable_operating_value: u32,
    #[serde(rename = "AdjustedMinAllowableOperatingValue")]
    pub adjusted_min_allowable_operating_value: i32,
    #[serde(rename = "DeltaReadingCelsius")]
    pub delta_reading_celsius: i32,
    #[serde(rename = "UpperThresholdUser")]
    pub upper_threshold_user: u32,
    #[serde(rename = "LowerThresholdUser")]
    pub lower_threshold_user: i32,
    #[serde(rename = "DeltaPhysicalContext")]
    pub delta_physical_context: &'static str,
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
    #[serde(rename = "MinReadingRange")]
    pub min_reading_range: u32,
    #[serde(rename = "MaxReadingRange")]
    pub max_reading_range: u32,
    #[serde(rename = "UpperThresholdCritical")]
    pub upper_threshold_critical: u32,
    #[serde(rename = "UpperThresholdFatal")]
    pub upper_threshold_fatal: u32,
    #[serde(rename = "UpperThresholdNonCritical")]
    pub upper_threshold_non_critical: u32,
    #[serde(rename = "LowerThresholdCritical")]
    pub lower_threshold_critical: u32,
    #[serde(rename = "LowerThresholdFatal")]
    pub lower_threshold_fatal: u32,
    #[serde(rename = "LowerThresholdNonCritical")]
    pub lower_threshold_non_critical: u32,
    #[serde(rename = "RelatedItem")]
    pub related_item: Vec<ODataId>,
    #[serde(rename = "SparePartNumber")]
    pub spare_part_number: &'static str,
    #[serde(rename = "Location")]
    pub location: super::types::RedfishLocation,
    #[serde(rename = "HotPluggable")]
    pub hot_pluggable: bool,
    #[serde(rename = "IndicatorLED")]
    pub indicator_led: &'static str,
    #[serde(rename = "Redundancy")]
    pub redundancy: Vec<serde_json::Value>,
    #[serde(rename = "Status")]
    pub status: Status,
}

pub async fn get_thermal(_user: AuthenticatedUser) -> Json<ThermalResource> {
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
            upper_threshold_fatal: 100,
            lower_threshold_fatal: -20,
            max_allowable_operating_value: 85,
            min_allowable_operating_value: 0,
            adjusted_max_allowable_operating_value: 85,
            adjusted_min_allowable_operating_value: 0,
            upper_threshold_user: 85,
            lower_threshold_user: 0,
            delta_reading_celsius: 0,
            delta_physical_context: "Exhaust",
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
            min_reading_range: 0,
            max_reading_range: 10000,
            upper_threshold_critical: 9000,
            upper_threshold_fatal: 10000,
            upper_threshold_non_critical: 8000,
            lower_threshold_critical: 500,
            lower_threshold_fatal: 0,
            lower_threshold_non_critical: 1000,
            related_item: vec![ODataId::new("/redfish/v1/Chassis/1")],
            spare_part_number: "VBMC-FAN-SPARE",
            location: super::types::RedfishLocation::new("Bay 1", "Bay", "Fan 0", "Bay", 0),
            hot_pluggable: false,
            indicator_led: "Off",
            redundancy: Vec::new(),
            status: Status::enabled_ok(),
        }],
        status: Status::enabled_ok(),
    })
}
