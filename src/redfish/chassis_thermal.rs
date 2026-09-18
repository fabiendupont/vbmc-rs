use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use serde::Serialize;

use super::types::{ODataId, Status};
use crate::app_state::AppState;
use crate::auth::AuthenticatedUser;

#[derive(Debug, Serialize)]
pub struct ThermalResource {
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

pub async fn get_thermal(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
) -> Json<ThermalResource> {
    let cid = &state.chassis_id;
    Json(ThermalResource {
        odata_id: format!("/redfish/v1/Chassis/{cid}/Thermal"),
        odata_type: "#Thermal.v1_7_2.Thermal",
        id: "Thermal",
        name: "Thermal",
        description: "Thermal sensors and fans",
        temperatures: vec![Temperature {
            odata_id: format!("/redfish/v1/Chassis/{cid}/Thermal#/Temperatures/0"),
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
            related_item: vec![ODataId::new(format!("/redfish/v1/Chassis/{cid}"))],
            status: Status::enabled_ok(),
        }],
        fans: vec![Fan {
            odata_id: format!("/redfish/v1/Chassis/{cid}/Thermal#/Fans/0"),
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
            related_item: vec![ODataId::new(format!("/redfish/v1/Chassis/{cid}"))],
            spare_part_number: "VBMC-FAN-SPARE",
            location: super::types::RedfishLocation::new("Fan 0", "Bay", 0),
            hot_pluggable: false,
            indicator_led: "Off",
            redundancy: Vec::new(),
            status: Status::enabled_ok(),
        }],
        status: Status::enabled_ok(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thermal_resource_serialization() {
        let resource = ThermalResource {
            odata_id: "/redfish/v1/Chassis/test/Thermal".to_string(),
            odata_type: "#Thermal.v1_7_2.Thermal",
            id: "Thermal",
            name: "Thermal",
            description: "Thermal sensors and fans",
            temperatures: Vec::new(),
            fans: Vec::new(),
            status: Status::enabled_ok(),
        };

        let json = serde_json::to_value(&resource).unwrap();
        assert_eq!(json["@odata.id"], "/redfish/v1/Chassis/test/Thermal");
        assert_eq!(json["@odata.type"], "#Thermal.v1_7_2.Thermal");
        assert_eq!(json["Id"], "Thermal");
        assert_eq!(json["Name"], "Thermal");
        assert_eq!(json["Temperatures"].as_array().unwrap().len(), 0);
        assert_eq!(json["Fans"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn test_temperature_serialization() {
        let temp = Temperature {
            odata_id: "/redfish/v1/Chassis/test/Thermal#/Temperatures/0".to_string(),
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
            related_item: Vec::new(),
            status: Status::enabled_ok(),
        };

        let json = serde_json::to_value(&temp).unwrap();
        assert_eq!(
            json["@odata.id"],
            "/redfish/v1/Chassis/test/Thermal#/Temperatures/0"
        );
        assert_eq!(json["MemberId"], "0");
        assert_eq!(json["Name"], "CPU Temperature");
        assert_eq!(json["ReadingCelsius"], 35);
        assert_eq!(json["UpperThresholdCritical"], 90);
        assert_eq!(json["UpperThresholdNonCritical"], 75);
        assert_eq!(json["PhysicalContext"], "CPU");
        assert_eq!(json["SensorNumber"], 1);
        assert_eq!(json["DeltaPhysicalContext"], "Exhaust");
    }

    #[test]
    fn test_temperature_negative_ranges() {
        let temp = Temperature {
            odata_id: "/redfish/v1/Chassis/test/Thermal#/Temperatures/0".to_string(),
            member_id: "0",
            name: "Test",
            reading_celsius: 20,
            upper_threshold_critical: 50,
            upper_threshold_non_critical: 40,
            lower_threshold_critical: 0,
            lower_threshold_non_critical: 5,
            min_reading_range_temp: -20,
            max_reading_range_temp: 80,
            upper_threshold_fatal: 60,
            lower_threshold_fatal: -10,
            max_allowable_operating_value: 50,
            min_allowable_operating_value: -5,
            adjusted_max_allowable_operating_value: 50,
            adjusted_min_allowable_operating_value: -5,
            upper_threshold_user: 45,
            lower_threshold_user: -2,
            delta_reading_celsius: 5,
            delta_physical_context: "Exhaust",
            physical_context: "Ambient",
            sensor_number: 2,
            related_item: Vec::new(),
            status: Status::enabled_ok(),
        };

        let json = serde_json::to_value(&temp).unwrap();
        assert_eq!(json["MinReadingRangeTemp"], -20);
        assert_eq!(json["LowerThresholdFatal"], -10);
        assert_eq!(json["MinAllowableOperatingValue"], -5);
        assert_eq!(json["DeltaReadingCelsius"], 5);
    }

    #[test]
    fn test_fan_serialization() {
        let fan = Fan {
            odata_id: "/redfish/v1/Chassis/test/Thermal#/Fans/0".to_string(),
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
            related_item: Vec::new(),
            spare_part_number: "VBMC-FAN-SPARE",
            location: crate::redfish::types::RedfishLocation::new("Fan 0", "Bay", 0),
            hot_pluggable: false,
            indicator_led: "Off",
            redundancy: Vec::new(),
            status: Status::enabled_ok(),
        };

        let json = serde_json::to_value(&fan).unwrap();
        assert_eq!(
            json["@odata.id"],
            "/redfish/v1/Chassis/test/Thermal#/Fans/0"
        );
        assert_eq!(json["MemberId"], "0");
        assert_eq!(json["Name"], "System Fan");
        assert_eq!(json["Reading"], 3000);
        assert_eq!(json["ReadingUnits"], "RPM");
        assert_eq!(json["PhysicalContext"], "Exhaust");
        assert_eq!(json["Manufacturer"], "vbmc-rs");
        assert_eq!(json["HotPluggable"], false);
        assert_eq!(json["IndicatorLED"], "Off");
    }

    #[test]
    fn test_fan_thresholds() {
        let fan = Fan {
            odata_id: "/redfish/v1/Chassis/test/Thermal#/Fans/0".to_string(),
            member_id: "0",
            name: "Test Fan",
            reading: 5000,
            reading_units: "RPM",
            physical_context: "Intake",
            sensor_number: 20,
            manufacturer: "TestCorp",
            model: "TF-100",
            serial_number: "SN001",
            part_number: "PN001",
            min_reading_range: 500,
            max_reading_range: 8000,
            upper_threshold_critical: 7500,
            upper_threshold_fatal: 8000,
            upper_threshold_non_critical: 7000,
            lower_threshold_critical: 1000,
            lower_threshold_fatal: 500,
            lower_threshold_non_critical: 1500,
            related_item: Vec::new(),
            spare_part_number: "SPN001",
            location: crate::redfish::types::RedfishLocation::new("Fan 1", "Bay", 1),
            hot_pluggable: true,
            indicator_led: "Lit",
            redundancy: Vec::new(),
            status: Status::enabled_ok(),
        };

        let json = serde_json::to_value(&fan).unwrap();
        assert_eq!(json["MinReadingRange"], 500);
        assert_eq!(json["MaxReadingRange"], 8000);
        assert_eq!(json["UpperThresholdCritical"], 7500);
        assert_eq!(json["LowerThresholdCritical"], 1000);
        assert_eq!(json["UpperThresholdFatal"], 8000);
        assert_eq!(json["LowerThresholdFatal"], 500);
    }
}
