use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use serde::Serialize;

use super::types::{ODataId, Status};
use crate::app_state::AppState;
use crate::auth::AuthenticatedUser;

#[derive(Debug, Serialize)]
pub struct PowerResource {
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
    #[serde(rename = "PowerControl")]
    pub power_control: Vec<PowerControl>,
    #[serde(rename = "PowerSupplies")]
    pub power_supplies: Vec<PowerSupply>,
    #[serde(rename = "Voltages")]
    pub voltages: Vec<Voltage>,
}

#[derive(Debug, Serialize)]
pub struct Voltage {
    #[serde(rename = "@odata.id")]
    pub odata_id: String,
    #[serde(rename = "MemberId")]
    pub member_id: &'static str,
    #[serde(rename = "Name")]
    pub name: &'static str,
    #[serde(rename = "ReadingVolts")]
    pub reading_volts: f64,
    #[serde(rename = "PhysicalContext")]
    pub physical_context: &'static str,
    #[serde(rename = "SensorNumber")]
    pub sensor_number: u32,
    #[serde(rename = "UpperThresholdCritical")]
    pub upper_threshold_critical: f64,
    #[serde(rename = "UpperThresholdNonCritical")]
    pub upper_threshold_non_critical: f64,
    #[serde(rename = "LowerThresholdCritical")]
    pub lower_threshold_critical: f64,
    #[serde(rename = "LowerThresholdNonCritical")]
    pub lower_threshold_non_critical: f64,
    #[serde(rename = "MinReadingRange")]
    pub min_reading_range: f64,
    #[serde(rename = "MaxReadingRange")]
    pub max_reading_range: f64,
    #[serde(rename = "UpperThresholdFatal")]
    pub upper_threshold_fatal: f64,
    #[serde(rename = "LowerThresholdFatal")]
    pub lower_threshold_fatal: f64,
    #[serde(rename = "Status")]
    pub status: Status,
    #[serde(rename = "RelatedItem")]
    pub related_item: Vec<ODataId>,
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
    #[serde(rename = "PowerAllocatedWatts")]
    pub power_allocated_watts: u32,
    #[serde(rename = "PowerAvailableWatts")]
    pub power_available_watts: u32,
    #[serde(rename = "PowerRequestedWatts")]
    pub power_requested_watts: u32,
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
    #[serde(rename = "CorrectionInMs")]
    pub correction_in_ms: u32,
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
    #[serde(rename = "LastPowerOutputWatts")]
    pub last_power_output_watts: u32,
    #[serde(rename = "PowerInputWatts")]
    pub power_input_watts: u32,
    #[serde(rename = "PowerOutputWatts")]
    pub power_output_watts: u32,
    #[serde(rename = "EfficiencyPercent")]
    pub efficiency_percent: u32,
    #[serde(rename = "SparePartNumber")]
    pub spare_part_number: &'static str,
    #[serde(rename = "InputRanges")]
    pub input_ranges: Vec<LegacyInputRange>,
    #[serde(rename = "Location")]
    pub location: super::types::RedfishLocation,
    #[serde(rename = "HotPluggable")]
    pub hot_pluggable: bool,
    #[serde(rename = "IndicatorLED")]
    pub indicator_led: &'static str,
    #[serde(rename = "RelatedItem")]
    pub related_item: Vec<ODataId>,
    #[serde(rename = "Status")]
    pub status: Status,
}

#[derive(Debug, Serialize)]
pub struct LegacyInputRange {
    #[serde(rename = "InputType")]
    pub input_type: &'static str,
    #[serde(rename = "MinimumVoltage")]
    pub minimum_voltage: u32,
    #[serde(rename = "MaximumVoltage")]
    pub maximum_voltage: u32,
    #[serde(rename = "MinimumFrequencyHz")]
    pub minimum_frequency_hz: u32,
    #[serde(rename = "MaximumFrequencyHz")]
    pub maximum_frequency_hz: u32,
    #[serde(rename = "OutputWattage")]
    pub output_wattage: u32,
}

pub async fn get_power(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
) -> Json<PowerResource> {
    let cid = &state.chassis_id;
    Json(PowerResource {
        odata_id: format!("/redfish/v1/Chassis/{cid}/Power"),
        odata_type: "#Power.v1_7_2.Power",
        id: "Power",
        name: "Power",
        description: "Power consumption and supplies",
        power_control: vec![PowerControl {
            odata_id: format!("/redfish/v1/Chassis/{cid}/Power#/PowerControl/0"),
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
                correction_in_ms: 1000,
            },
            power_allocated_watts: 500,
            power_available_watts: 450,
            power_requested_watts: 50,
            related_item: vec![ODataId::new(format!("/redfish/v1/Chassis/{cid}"))],
            status: Status::enabled_ok(),
        }],
        power_supplies: vec![PowerSupply {
            odata_id: format!("/redfish/v1/Chassis/{cid}/Power#/PowerSupplies/0"),
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
            last_power_output_watts: 50,
            power_input_watts: 55,
            power_output_watts: 50,
            efficiency_percent: 90,
            spare_part_number: "VBMC-PSU-SPARE",
            input_ranges: vec![LegacyInputRange {
                input_type: "AC",
                minimum_voltage: 200,
                maximum_voltage: 240,
                minimum_frequency_hz: 50,
                maximum_frequency_hz: 60,
                output_wattage: 500,
            }],
            location: super::types::RedfishLocation::new("PSU 0", "Bay", 0),
            hot_pluggable: false,
            indicator_led: "Off",
            related_item: vec![ODataId::new(format!("/redfish/v1/Chassis/{cid}"))],
            status: Status::enabled_ok(),
        }],
        voltages: vec![Voltage {
            odata_id: format!("/redfish/v1/Chassis/{cid}/Power#/Voltages/0"),
            member_id: "0",
            name: "12V Rail",
            reading_volts: 12.1,
            physical_context: "SystemBoard",
            sensor_number: 20,
            upper_threshold_critical: 13.0,
            upper_threshold_non_critical: 12.6,
            lower_threshold_critical: 10.8,
            lower_threshold_non_critical: 11.4,
            min_reading_range: 0.0,
            max_reading_range: 15.0,
            upper_threshold_fatal: 14.0,
            lower_threshold_fatal: 10.0,
            status: Status::enabled_ok(),
            related_item: vec![ODataId::new(format!("/redfish/v1/Chassis/{cid}"))],
        }],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_power_resource_serialization() {
        let resource = PowerResource {
            odata_id: "/redfish/v1/Chassis/test/Power".to_string(),
            odata_type: "#Power.v1_7_2.Power",
            id: "Power",
            name: "Power",
            description: "Test power",
            power_control: Vec::new(),
            power_supplies: Vec::new(),
            voltages: Vec::new(),
        };

        let json = serde_json::to_value(&resource).unwrap();
        assert_eq!(json["@odata.id"], "/redfish/v1/Chassis/test/Power");
        assert_eq!(json["@odata.type"], "#Power.v1_7_2.Power");
        assert_eq!(json["Id"], "Power");
        assert_eq!(json["Name"], "Power");
        assert_eq!(json["Description"], "Test power");
        assert_eq!(json["PowerControl"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn test_voltage_serialization() {
        let voltage = Voltage {
            odata_id: "/redfish/v1/Chassis/test/Power#/Voltages/0".to_string(),
            member_id: "0",
            name: "12V Rail",
            reading_volts: 12.1,
            physical_context: "SystemBoard",
            sensor_number: 20,
            upper_threshold_critical: 13.0,
            upper_threshold_non_critical: 12.6,
            lower_threshold_critical: 10.8,
            lower_threshold_non_critical: 11.4,
            min_reading_range: 0.0,
            max_reading_range: 15.0,
            upper_threshold_fatal: 14.0,
            lower_threshold_fatal: 10.0,
            status: Status::enabled_ok(),
            related_item: Vec::new(),
        };

        let json = serde_json::to_value(&voltage).unwrap();
        assert_eq!(
            json["@odata.id"],
            "/redfish/v1/Chassis/test/Power#/Voltages/0"
        );
        assert_eq!(json["MemberId"], "0");
        assert_eq!(json["Name"], "12V Rail");
        assert_eq!(json["ReadingVolts"], 12.1);
        assert_eq!(json["PhysicalContext"], "SystemBoard");
        assert_eq!(json["SensorNumber"], 20);
        assert_eq!(json["UpperThresholdCritical"], 13.0);
        assert_eq!(json["LowerThresholdCritical"], 10.8);
    }

    #[test]
    fn test_power_control_serialization() {
        let power_control = PowerControl {
            odata_id: "/redfish/v1/Chassis/test/Power#/PowerControl/0".to_string(),
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
                correction_in_ms: 1000,
            },
            power_allocated_watts: 500,
            power_available_watts: 450,
            power_requested_watts: 50,
            related_item: Vec::new(),
            status: Status::enabled_ok(),
        };

        let json = serde_json::to_value(&power_control).unwrap();
        assert_eq!(
            json["@odata.id"],
            "/redfish/v1/Chassis/test/Power#/PowerControl/0"
        );
        assert_eq!(json["PowerConsumedWatts"], 50);
        assert_eq!(json["PowerCapacityWatts"], 500);
        assert_eq!(json["PowerAllocatedWatts"], 500);
        assert_eq!(json["PowerAvailableWatts"], 450);
    }

    #[test]
    fn test_power_metrics_serialization() {
        let metrics = PowerMetrics {
            interval_in_min: 5,
            min_consumed_watts: 10,
            max_consumed_watts: 200,
            average_consumed_watts: 75,
        };

        let json = serde_json::to_value(&metrics).unwrap();
        assert_eq!(json["IntervalInMin"], 5);
        assert_eq!(json["MinConsumedWatts"], 10);
        assert_eq!(json["MaxConsumedWatts"], 200);
        assert_eq!(json["AverageConsumedWatts"], 75);
    }

    #[test]
    fn test_power_limit_serialization() {
        let limit = PowerLimit {
            limit_in_watts: 500,
            limit_exception: "HardPowerOff",
            correction_in_ms: 5000,
        };

        let json = serde_json::to_value(&limit).unwrap();
        assert_eq!(json["LimitInWatts"], 500);
        assert_eq!(json["LimitException"], "HardPowerOff");
        assert_eq!(json["CorrectionInMs"], 5000);
    }

    #[test]
    fn test_power_supply_serialization() {
        let psu = PowerSupply {
            odata_id: "/redfish/v1/Chassis/test/Power#/PowerSupplies/0".to_string(),
            member_id: "0",
            name: "PSU 1",
            power_capacity_watts: 750,
            power_supply_type: "AC",
            manufacturer: "TestVendor",
            model: "PSU-750",
            serial_number: "SN12345",
            part_number: "PN12345",
            firmware_version: "1.0",
            line_input_voltage: 230,
            last_power_output_watts: 100,
            power_input_watts: 110,
            power_output_watts: 100,
            efficiency_percent: 91,
            spare_part_number: "SPN12345",
            input_ranges: Vec::new(),
            location: crate::redfish::types::RedfishLocation::new("PSU 0", "Bay", 0),
            hot_pluggable: true,
            indicator_led: "Lit",
            related_item: Vec::new(),
            status: Status::enabled_ok(),
        };

        let json = serde_json::to_value(&psu).unwrap();
        assert_eq!(
            json["@odata.id"],
            "/redfish/v1/Chassis/test/Power#/PowerSupplies/0"
        );
        assert_eq!(json["PowerCapacityWatts"], 750);
        assert_eq!(json["PowerSupplyType"], "AC");
        assert_eq!(json["Manufacturer"], "TestVendor");
        assert_eq!(json["Model"], "PSU-750");
        assert_eq!(json["SerialNumber"], "SN12345");
        assert_eq!(json["HotPluggable"], true);
        assert_eq!(json["IndicatorLED"], "Lit");
    }

    #[test]
    fn test_legacy_input_range_serialization() {
        let range = LegacyInputRange {
            input_type: "AC",
            minimum_voltage: 200,
            maximum_voltage: 240,
            minimum_frequency_hz: 50,
            maximum_frequency_hz: 60,
            output_wattage: 500,
        };

        let json = serde_json::to_value(&range).unwrap();
        assert_eq!(json["InputType"], "AC");
        assert_eq!(json["MinimumVoltage"], 200);
        assert_eq!(json["MaximumVoltage"], 240);
        assert_eq!(json["MinimumFrequencyHz"], 50);
        assert_eq!(json["MaximumFrequencyHz"], 60);
        assert_eq!(json["OutputWattage"], 500);
    }
}
