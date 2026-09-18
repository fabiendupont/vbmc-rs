use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use serde::Serialize;

use super::error::RedfishApiError;
use super::types::{Collection, ODataId, Status};
use crate::app_state::AppState;
use crate::auth::AuthenticatedUser;

#[derive(Debug, Serialize)]
pub struct SensorResource {
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
    #[serde(rename = "Reading")]
    pub reading: f64,
    #[serde(rename = "ReadingType")]
    pub reading_type: &'static str,
    #[serde(rename = "ReadingUnits")]
    pub reading_units: &'static str,
    #[serde(rename = "PhysicalContext")]
    pub physical_context: &'static str,
    #[serde(rename = "PhysicalSubContext")]
    pub physical_sub_context: &'static str,
    #[serde(rename = "Implementation")]
    pub implementation: &'static str,
    #[serde(rename = "ReadingBasis")]
    pub reading_basis: &'static str,
    #[serde(rename = "ReadingRangeMin")]
    pub reading_range_min: f64,
    #[serde(rename = "ReadingRangeMax")]
    pub reading_range_max: f64,
    #[serde(rename = "Precision")]
    pub precision: f64,
    #[serde(rename = "ReadingAccuracy")]
    pub reading_accuracy: f64,
    #[serde(rename = "SensingInterval", skip_serializing_if = "Option::is_none")]
    pub sensing_interval: Option<&'static str>,
    #[serde(rename = "ReadingTime")]
    pub reading_time: String,
    #[serde(rename = "PeakReading")]
    pub peak_reading: f64,
    #[serde(rename = "PeakReadingTime")]
    pub peak_reading_time: String,
    #[serde(rename = "LowestReading")]
    pub lowest_reading: f64,
    #[serde(rename = "LowestReadingTime")]
    pub lowest_reading_time: String,
    #[serde(rename = "AverageReading")]
    pub average_reading: f64,
    #[serde(rename = "AveragingInterval", skip_serializing_if = "Option::is_none")]
    pub averaging_interval: Option<&'static str>,
    #[serde(rename = "AveragingIntervalAchieved")]
    pub averaging_interval_achieved: bool,
    #[serde(rename = "SensorResetTime")]
    pub sensor_reset_time: String,
    #[serde(rename = "Thresholds")]
    pub thresholds: SensorThresholds,
    #[serde(rename = "MaxAllowableOperatingValue")]
    pub max_allowable_operating_value: f64,
    #[serde(rename = "MinAllowableOperatingValue")]
    pub min_allowable_operating_value: f64,
    #[serde(rename = "AdjustedMaxAllowableOperatingValue")]
    pub adjusted_max_allowable_operating_value: f64,
    #[serde(rename = "AdjustedMinAllowableOperatingValue")]
    pub adjusted_min_allowable_operating_value: f64,
    #[serde(rename = "LifetimeReading")]
    pub lifetime_reading: f64,
    #[serde(rename = "ElectricalContext", skip_serializing_if = "Option::is_none")]
    pub electrical_context: Option<&'static str>,
    #[serde(rename = "VoltageType", skip_serializing_if = "Option::is_none")]
    pub voltage_type: Option<&'static str>,
    #[serde(rename = "SpeedRPM", skip_serializing_if = "Option::is_none")]
    pub speed_rpm: Option<f64>,
    #[serde(rename = "CrestFactor", skip_serializing_if = "Option::is_none")]
    pub crest_factor: Option<f64>,
    #[serde(rename = "THDPercent", skip_serializing_if = "Option::is_none")]
    pub thd_percent: Option<f64>,
    #[serde(rename = "ApparentkVAh", skip_serializing_if = "Option::is_none")]
    pub apparent_kvah: Option<f64>,
    #[serde(rename = "ReactivekVARh", skip_serializing_if = "Option::is_none")]
    pub reactive_kvarh: Option<f64>,
    #[serde(rename = "PhaseAngleDegrees", skip_serializing_if = "Option::is_none")]
    pub phase_angle_degrees: Option<f64>,
    #[serde(rename = "ApparentVA", skip_serializing_if = "Option::is_none")]
    pub apparent_va: Option<f64>,
    #[serde(rename = "ReactiveVAR", skip_serializing_if = "Option::is_none")]
    pub reactive_var: Option<f64>,
    #[serde(rename = "PowerFactor", skip_serializing_if = "Option::is_none")]
    pub power_factor: Option<f64>,
    #[serde(rename = "Manufacturer")]
    pub manufacturer: &'static str,
    #[serde(rename = "Model")]
    pub model: &'static str,
    #[serde(rename = "SerialNumber")]
    pub serial_number: String,
    #[serde(rename = "PartNumber")]
    pub part_number: &'static str,
    #[serde(rename = "SKU")]
    pub sku: &'static str,
    #[serde(rename = "SparePartNumber")]
    pub spare_part_number: &'static str,
    #[serde(rename = "UserLabel")]
    pub user_label: String,
    #[serde(rename = "Calibration")]
    pub calibration: f64,
    #[serde(rename = "CalibrationTime")]
    pub calibration_time: &'static str,
    #[serde(rename = "LifetimeStartDateTime")]
    pub lifetime_start_date_time: &'static str,
    #[serde(rename = "RelatedItem")]
    pub related_item: Vec<ODataId>,
    #[serde(rename = "Location")]
    pub location: super::types::RedfishLocation,
    #[serde(rename = "Status")]
    pub status: Status,
}

#[derive(Debug, Serialize)]
pub struct SensorThresholds {
    #[serde(rename = "UpperCritical")]
    pub upper_critical: ThresholdValue,
    #[serde(rename = "UpperCaution")]
    pub upper_caution: ThresholdValue,
    #[serde(rename = "LowerCaution")]
    pub lower_caution: ThresholdValue,
    #[serde(rename = "LowerCritical")]
    pub lower_critical: ThresholdValue,
    #[serde(rename = "UpperCautionUser")]
    pub upper_caution_user: ThresholdValue,
    #[serde(rename = "UpperCriticalUser")]
    pub upper_critical_user: ThresholdValue,
    #[serde(rename = "LowerCautionUser")]
    pub lower_caution_user: ThresholdValue,
    #[serde(rename = "LowerCriticalUser")]
    pub lower_critical_user: ThresholdValue,
    #[serde(rename = "UpperFatal")]
    pub upper_fatal: ThresholdValue,
    #[serde(rename = "LowerFatal")]
    pub lower_fatal: ThresholdValue,
}

#[derive(Debug, Serialize)]
pub struct ThresholdValue {
    #[serde(rename = "Reading")]
    pub reading: f64,
    #[serde(rename = "Activation")]
    pub activation: &'static str,
    #[serde(rename = "HysteresisReading")]
    pub hysteresis_reading: f64,
    #[serde(rename = "HysteresisDuration")]
    pub hysteresis_duration: &'static str,
    #[serde(rename = "DwellTime")]
    pub dwell_time: &'static str,
}

struct SensorDef {
    id: &'static str,
    name: &'static str,
    reading: f64,
    reading_type: &'static str,
    reading_units: &'static str,
    physical_context: &'static str,
    physical_sub_context: &'static str,
    range_min: f64,
    range_max: f64,
    lower_critical: f64,
    lower_caution: f64,
    upper_caution: f64,
    upper_critical: f64,
    electrical_context: Option<&'static str>,
    voltage_type: Option<&'static str>,
    speed_rpm: Option<f64>,
    is_electrical: bool,
}

const SENSORS: &[SensorDef] = &[
    SensorDef {
        id: "CpuTemp",
        name: "CPU Temperature",
        reading: 35.0,
        reading_type: "Temperature",
        reading_units: "Cel",
        physical_context: "CPU",
        physical_sub_context: "Input",
        range_min: 0.0,
        range_max: 105.0,
        lower_critical: 5.0,
        lower_caution: 10.0,
        upper_caution: 85.0,
        upper_critical: 100.0,
        electrical_context: None,
        voltage_type: None,
        speed_rpm: None,
        is_electrical: false,
    },
    SensorDef {
        id: "AmbientTemp",
        name: "Ambient Temperature",
        reading: 22.0,
        reading_type: "Temperature",
        reading_units: "Cel",
        physical_context: "Room",
        physical_sub_context: "Input",
        range_min: 0.0,
        range_max: 60.0,
        lower_critical: 5.0,
        lower_caution: 10.0,
        upper_caution: 40.0,
        upper_critical: 50.0,
        electrical_context: None,
        voltage_type: None,
        speed_rpm: None,
        is_electrical: false,
    },
    SensorDef {
        id: "ExhaustTemp",
        name: "Exhaust Temperature",
        reading: 28.0,
        reading_type: "Temperature",
        reading_units: "Cel",
        physical_context: "Exhaust",
        physical_sub_context: "Output",
        range_min: 0.0,
        range_max: 80.0,
        lower_critical: 5.0,
        lower_caution: 10.0,
        upper_caution: 60.0,
        upper_critical: 70.0,
        electrical_context: None,
        voltage_type: None,
        speed_rpm: None,
        is_electrical: false,
    },
    SensorDef {
        id: "IntakeTemp",
        name: "Intake Temperature",
        reading: 20.0,
        reading_type: "Temperature",
        reading_units: "Cel",
        physical_context: "Intake",
        physical_sub_context: "Input",
        range_min: 0.0,
        range_max: 60.0,
        lower_critical: 5.0,
        lower_caution: 10.0,
        upper_caution: 40.0,
        upper_critical: 50.0,
        electrical_context: None,
        voltage_type: None,
        speed_rpm: None,
        is_electrical: false,
    },
    SensorDef {
        id: "SystemFanSpeed",
        name: "System Fan Speed",
        reading: 3000.0,
        reading_type: "Rotational",
        reading_units: "RPM",
        physical_context: "Exhaust",
        physical_sub_context: "Output",
        range_min: 0.0,
        range_max: 10000.0,
        lower_critical: 500.0,
        lower_caution: 1000.0,
        upper_caution: 8000.0,
        upper_critical: 9500.0,
        electrical_context: None,
        voltage_type: None,
        speed_rpm: Some(3000.0),
        is_electrical: false,
    },
    SensorDef {
        id: "ChassisPower",
        name: "Chassis Power",
        reading: 50.0,
        reading_type: "Power",
        reading_units: "W",
        physical_context: "Chassis",
        physical_sub_context: "Input",
        range_min: 0.0,
        range_max: 1000.0,
        lower_critical: 0.0,
        lower_caution: 0.0,
        upper_caution: 800.0,
        upper_critical: 950.0,
        electrical_context: Some("Line1"),
        voltage_type: None,
        speed_rpm: None,
        is_electrical: true,
    },
    SensorDef {
        id: "Voltage12V",
        name: "12V Rail Voltage",
        reading: 12.1,
        reading_type: "Voltage",
        reading_units: "V",
        physical_context: "PowerSupply",
        physical_sub_context: "Output",
        range_min: 0.0,
        range_max: 15.0,
        lower_critical: 10.8,
        lower_caution: 11.4,
        upper_caution: 12.6,
        upper_critical: 13.2,
        electrical_context: Some("Line1"),
        voltage_type: Some("DC"),
        speed_rpm: None,
        is_electrical: true,
    },
];

pub async fn get_sensors(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
) -> Json<Collection<ODataId>> {
    let cid = &state.chassis_id;
    let members: Vec<ODataId> = SENSORS
        .iter()
        .map(|s| ODataId::new(format!("/redfish/v1/Chassis/{cid}/Sensors/{}", s.id)))
        .collect();

    Json(Collection::new(
        format!("/redfish/v1/Chassis/{cid}/Sensors"),
        "#SensorCollection.SensorCollection",
        "Sensor Collection",
        members,
    ))
}

fn make_threshold(reading: f64, activation: &'static str) -> ThresholdValue {
    ThresholdValue {
        reading,
        activation,
        hysteresis_reading: 0.0,
        hysteresis_duration: "PT0S",
        dwell_time: "PT0S",
    }
}

pub async fn get_sensor(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
    Path((_, sensor_id)): Path<(String, String)>,
) -> Result<Json<SensorResource>, RedfishApiError> {
    let cid = &state.chassis_id;
    let def = SENSORS
        .iter()
        .find(|s| s.id == sensor_id)
        .ok_or_else(|| RedfishApiError::NotFound(format!("Sensor '{sensor_id}' not found")))?;

    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

    let is_power = def.is_electrical && def.reading_type == "Power";
    let is_electrical = def.is_electrical;

    Ok(Json(SensorResource {
        odata_id: format!("/redfish/v1/Chassis/{cid}/Sensors/{}", def.id),
        odata_type: "#Sensor.v1_9_0.Sensor",
        id: def.id.to_string(),
        name: def.name.to_string(),
        description: format!("{} sensor", def.name),
        reading: def.reading,
        reading_type: def.reading_type,
        reading_units: def.reading_units,
        physical_context: def.physical_context,
        physical_sub_context: def.physical_sub_context,
        implementation: "PhysicalSensor",
        reading_basis: "Zero",
        reading_range_min: def.range_min,
        reading_range_max: def.range_max,
        precision: 0.1,
        reading_accuracy: 1.0,
        sensing_interval: Some("PT1S"),
        reading_time: now.clone(),
        peak_reading: def.reading,
        peak_reading_time: now.clone(),
        lowest_reading: def.reading,
        lowest_reading_time: now.clone(),
        average_reading: def.reading,
        averaging_interval: Some("PT60S"),
        averaging_interval_achieved: true,
        sensor_reset_time: "2026-01-01T00:00:00Z".to_string(),
        thresholds: SensorThresholds {
            upper_critical: make_threshold(def.upper_critical, "Increasing"),
            upper_caution: make_threshold(def.upper_caution, "Increasing"),
            lower_caution: make_threshold(def.lower_caution, "Decreasing"),
            lower_critical: make_threshold(def.lower_critical, "Decreasing"),
            upper_caution_user: make_threshold(def.upper_caution, "Increasing"),
            upper_critical_user: make_threshold(def.upper_critical, "Increasing"),
            lower_caution_user: make_threshold(def.lower_caution, "Decreasing"),
            lower_critical_user: make_threshold(def.lower_critical, "Decreasing"),
            upper_fatal: make_threshold(def.upper_critical + 5.0, "Increasing"),
            lower_fatal: make_threshold(def.lower_critical - 5.0, "Decreasing"),
        },
        max_allowable_operating_value: def.range_max,
        min_allowable_operating_value: def.range_min,
        adjusted_max_allowable_operating_value: def.range_max,
        adjusted_min_allowable_operating_value: def.range_min,
        lifetime_reading: 0.0,
        electrical_context: def.electrical_context,
        voltage_type: def.voltage_type,
        speed_rpm: def.speed_rpm,
        crest_factor: if is_electrical { Some(1.414) } else { None },
        thd_percent: if is_electrical { Some(0.0) } else { None },
        apparent_kvah: if is_power { Some(0.0) } else { None },
        reactive_kvarh: if is_power { Some(0.0) } else { None },
        phase_angle_degrees: if is_power { Some(0.0) } else { None },
        apparent_va: if is_power { Some(0.0) } else { None },
        reactive_var: if is_power { Some(0.0) } else { None },
        power_factor: if is_power { Some(1.0) } else { None },
        manufacturer: "vbmc-rs",
        model: "Virtual Sensor",
        serial_number: format!("VBMC-SENS-{}", def.id),
        part_number: "VBMC-SENS",
        sku: "VBMC-VIRTUAL",
        spare_part_number: "VBMC-SENS-SPARE",
        user_label: def.name.to_string(),
        calibration: 0.0,
        calibration_time: "2026-01-01T00:00:00Z",
        lifetime_start_date_time: "2026-01-01T00:00:00Z",
        related_item: vec![ODataId::new(format!("/redfish/v1/Chassis/{cid}"))],
        location: super::types::RedfishLocation::new(def.id, "Embedded", 0),
        status: Status::enabled_ok(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_make_threshold() {
        let threshold = make_threshold(85.0, "Increasing");
        assert_eq!(threshold.reading, 85.0);
        assert_eq!(threshold.activation, "Increasing");
        assert_eq!(threshold.hysteresis_reading, 0.0);
        assert_eq!(threshold.hysteresis_duration, "PT0S");
        assert_eq!(threshold.dwell_time, "PT0S");
    }

    #[test]
    fn test_make_threshold_decreasing() {
        let threshold = make_threshold(10.0, "Decreasing");
        assert_eq!(threshold.reading, 10.0);
        assert_eq!(threshold.activation, "Decreasing");
    }

    #[test]
    fn test_sensor_resource_serialization() {
        let sensor = SensorResource {
            odata_id: "/redfish/v1/Chassis/test/Sensors/CpuTemp".to_string(),
            odata_type: "#Sensor.v1_9_0.Sensor",
            id: "CpuTemp".to_string(),
            name: "CPU Temperature".to_string(),
            description: "CPU sensor".to_string(),
            reading: 35.5,
            reading_type: "Temperature",
            reading_units: "Cel",
            physical_context: "CPU",
            physical_sub_context: "Input",
            implementation: "PhysicalSensor",
            reading_basis: "Zero",
            reading_range_min: 0.0,
            reading_range_max: 100.0,
            precision: 0.1,
            reading_accuracy: 1.0,
            sensing_interval: Some("PT1S"),
            reading_time: "2026-01-01T00:00:00Z".to_string(),
            peak_reading: 40.0,
            peak_reading_time: "2026-01-01T00:00:00Z".to_string(),
            lowest_reading: 30.0,
            lowest_reading_time: "2026-01-01T00:00:00Z".to_string(),
            average_reading: 35.0,
            averaging_interval: Some("PT60S"),
            averaging_interval_achieved: true,
            sensor_reset_time: "2026-01-01T00:00:00Z".to_string(),
            thresholds: SensorThresholds {
                upper_critical: make_threshold(90.0, "Increasing"),
                upper_caution: make_threshold(80.0, "Increasing"),
                lower_caution: make_threshold(10.0, "Decreasing"),
                lower_critical: make_threshold(5.0, "Decreasing"),
                upper_caution_user: make_threshold(80.0, "Increasing"),
                upper_critical_user: make_threshold(90.0, "Increasing"),
                lower_caution_user: make_threshold(10.0, "Decreasing"),
                lower_critical_user: make_threshold(5.0, "Decreasing"),
                upper_fatal: make_threshold(95.0, "Increasing"),
                lower_fatal: make_threshold(0.0, "Decreasing"),
            },
            max_allowable_operating_value: 100.0,
            min_allowable_operating_value: 0.0,
            adjusted_max_allowable_operating_value: 100.0,
            adjusted_min_allowable_operating_value: 0.0,
            lifetime_reading: 0.0,
            electrical_context: None,
            voltage_type: None,
            speed_rpm: None,
            crest_factor: None,
            thd_percent: None,
            apparent_kvah: None,
            reactive_kvarh: None,
            phase_angle_degrees: None,
            apparent_va: None,
            reactive_var: None,
            power_factor: None,
            manufacturer: "vbmc-rs",
            model: "Virtual Sensor",
            serial_number: "VBMC-SENS-001".to_string(),
            part_number: "VBMC-SENS",
            sku: "VBMC-VIRTUAL",
            spare_part_number: "VBMC-SENS-SPARE",
            user_label: "CPU Temperature".to_string(),
            calibration: 0.0,
            calibration_time: "2026-01-01T00:00:00Z",
            lifetime_start_date_time: "2026-01-01T00:00:00Z",
            related_item: Vec::new(),
            location: crate::redfish::types::RedfishLocation::new("CPU", "Embedded", 0),
            status: Status::enabled_ok(),
        };

        let json = serde_json::to_value(&sensor).unwrap();
        assert_eq!(
            json["@odata.id"],
            "/redfish/v1/Chassis/test/Sensors/CpuTemp"
        );
        assert_eq!(json["@odata.type"], "#Sensor.v1_9_0.Sensor");
        assert_eq!(json["Reading"], 35.5);
        assert_eq!(json["ReadingType"], "Temperature");
        assert_eq!(json["ReadingUnits"], "Cel");
        assert_eq!(json["PhysicalContext"], "CPU");
        assert_eq!(json["PhysicalSubContext"], "Input");
    }

    #[test]
    fn test_sensor_optional_fields_absent() {
        let sensor = SensorResource {
            odata_id: "/redfish/v1/Chassis/test/Sensors/Test".to_string(),
            odata_type: "#Sensor.v1_9_0.Sensor",
            id: "Test".to_string(),
            name: "Test".to_string(),
            description: "Test".to_string(),
            reading: 0.0,
            reading_type: "Temperature",
            reading_units: "Cel",
            physical_context: "Room",
            physical_sub_context: "Input",
            implementation: "PhysicalSensor",
            reading_basis: "Zero",
            reading_range_min: 0.0,
            reading_range_max: 100.0,
            precision: 0.1,
            reading_accuracy: 1.0,
            sensing_interval: None,
            reading_time: "2026-01-01T00:00:00Z".to_string(),
            peak_reading: 0.0,
            peak_reading_time: "2026-01-01T00:00:00Z".to_string(),
            lowest_reading: 0.0,
            lowest_reading_time: "2026-01-01T00:00:00Z".to_string(),
            average_reading: 0.0,
            averaging_interval: None,
            averaging_interval_achieved: false,
            sensor_reset_time: "2026-01-01T00:00:00Z".to_string(),
            thresholds: SensorThresholds {
                upper_critical: make_threshold(90.0, "Increasing"),
                upper_caution: make_threshold(80.0, "Increasing"),
                lower_caution: make_threshold(10.0, "Decreasing"),
                lower_critical: make_threshold(5.0, "Decreasing"),
                upper_caution_user: make_threshold(80.0, "Increasing"),
                upper_critical_user: make_threshold(90.0, "Increasing"),
                lower_caution_user: make_threshold(10.0, "Decreasing"),
                lower_critical_user: make_threshold(5.0, "Decreasing"),
                upper_fatal: make_threshold(95.0, "Increasing"),
                lower_fatal: make_threshold(0.0, "Decreasing"),
            },
            max_allowable_operating_value: 100.0,
            min_allowable_operating_value: 0.0,
            adjusted_max_allowable_operating_value: 100.0,
            adjusted_min_allowable_operating_value: 0.0,
            lifetime_reading: 0.0,
            electrical_context: None,
            voltage_type: None,
            speed_rpm: None,
            crest_factor: None,
            thd_percent: None,
            apparent_kvah: None,
            reactive_kvarh: None,
            phase_angle_degrees: None,
            apparent_va: None,
            reactive_var: None,
            power_factor: None,
            manufacturer: "vbmc-rs",
            model: "Virtual Sensor",
            serial_number: "VBMC-SENS-001".to_string(),
            part_number: "VBMC-SENS",
            sku: "VBMC-VIRTUAL",
            spare_part_number: "VBMC-SENS-SPARE",
            user_label: "Test".to_string(),
            calibration: 0.0,
            calibration_time: "2026-01-01T00:00:00Z",
            lifetime_start_date_time: "2026-01-01T00:00:00Z",
            related_item: Vec::new(),
            location: crate::redfish::types::RedfishLocation::new("Test", "Embedded", 0),
            status: Status::enabled_ok(),
        };

        let json = serde_json::to_value(&sensor).unwrap();
        assert!(json.get("SensingInterval").is_none());
        assert!(json.get("AveragingInterval").is_none());
        assert!(json.get("ElectricalContext").is_none());
        assert!(json.get("VoltageType").is_none());
        assert!(json.get("SpeedRPM").is_none());
        assert!(json.get("CrestFactor").is_none());
        assert!(json.get("THDPercent").is_none());
    }

    #[test]
    fn test_sensor_electrical_fields_present() {
        let sensor = SensorResource {
            odata_id: "/redfish/v1/Chassis/test/Sensors/Power".to_string(),
            odata_type: "#Sensor.v1_9_0.Sensor",
            id: "Power".to_string(),
            name: "Power".to_string(),
            description: "Power sensor".to_string(),
            reading: 50.0,
            reading_type: "Power",
            reading_units: "W",
            physical_context: "Chassis",
            physical_sub_context: "Input",
            implementation: "PhysicalSensor",
            reading_basis: "Zero",
            reading_range_min: 0.0,
            reading_range_max: 1000.0,
            precision: 0.1,
            reading_accuracy: 1.0,
            sensing_interval: Some("PT1S"),
            reading_time: "2026-01-01T00:00:00Z".to_string(),
            peak_reading: 50.0,
            peak_reading_time: "2026-01-01T00:00:00Z".to_string(),
            lowest_reading: 50.0,
            lowest_reading_time: "2026-01-01T00:00:00Z".to_string(),
            average_reading: 50.0,
            averaging_interval: Some("PT60S"),
            averaging_interval_achieved: true,
            sensor_reset_time: "2026-01-01T00:00:00Z".to_string(),
            thresholds: SensorThresholds {
                upper_critical: make_threshold(900.0, "Increasing"),
                upper_caution: make_threshold(800.0, "Increasing"),
                lower_caution: make_threshold(0.0, "Decreasing"),
                lower_critical: make_threshold(0.0, "Decreasing"),
                upper_caution_user: make_threshold(800.0, "Increasing"),
                upper_critical_user: make_threshold(900.0, "Increasing"),
                lower_caution_user: make_threshold(0.0, "Decreasing"),
                lower_critical_user: make_threshold(0.0, "Decreasing"),
                upper_fatal: make_threshold(950.0, "Increasing"),
                lower_fatal: make_threshold(0.0, "Decreasing"),
            },
            max_allowable_operating_value: 1000.0,
            min_allowable_operating_value: 0.0,
            adjusted_max_allowable_operating_value: 1000.0,
            adjusted_min_allowable_operating_value: 0.0,
            lifetime_reading: 0.0,
            electrical_context: Some("Line1"),
            voltage_type: None,
            speed_rpm: None,
            crest_factor: Some(1.414),
            thd_percent: Some(2.5),
            apparent_kvah: Some(0.0),
            reactive_kvarh: Some(0.0),
            phase_angle_degrees: Some(0.0),
            apparent_va: Some(55.0),
            reactive_var: Some(5.0),
            power_factor: Some(0.95),
            manufacturer: "vbmc-rs",
            model: "Virtual Sensor",
            serial_number: "VBMC-SENS-PWR".to_string(),
            part_number: "VBMC-SENS",
            sku: "VBMC-VIRTUAL",
            spare_part_number: "VBMC-SENS-SPARE",
            user_label: "Power".to_string(),
            calibration: 0.0,
            calibration_time: "2026-01-01T00:00:00Z",
            lifetime_start_date_time: "2026-01-01T00:00:00Z",
            related_item: Vec::new(),
            location: crate::redfish::types::RedfishLocation::new("Power", "Embedded", 0),
            status: Status::enabled_ok(),
        };

        let json = serde_json::to_value(&sensor).unwrap();
        assert_eq!(json["ElectricalContext"], "Line1");
        assert_eq!(json["CrestFactor"], 1.414);
        assert_eq!(json["THDPercent"], 2.5);
        assert_eq!(json["ApparentVA"], 55.0);
        assert_eq!(json["PowerFactor"], 0.95);
    }

    #[test]
    fn test_threshold_value_serialization() {
        let threshold = ThresholdValue {
            reading: 85.5,
            activation: "Increasing",
            hysteresis_reading: 2.0,
            hysteresis_duration: "PT5S",
            dwell_time: "PT10S",
        };

        let json = serde_json::to_value(&threshold).unwrap();
        assert_eq!(json["Reading"], 85.5);
        assert_eq!(json["Activation"], "Increasing");
        assert_eq!(json["HysteresisReading"], 2.0);
        assert_eq!(json["HysteresisDuration"], "PT5S");
        assert_eq!(json["DwellTime"], "PT10S");
    }

    #[test]
    fn test_sensor_thresholds_serialization() {
        let thresholds = SensorThresholds {
            upper_critical: make_threshold(90.0, "Increasing"),
            upper_caution: make_threshold(80.0, "Increasing"),
            lower_caution: make_threshold(10.0, "Decreasing"),
            lower_critical: make_threshold(5.0, "Decreasing"),
            upper_caution_user: make_threshold(75.0, "Increasing"),
            upper_critical_user: make_threshold(85.0, "Increasing"),
            lower_caution_user: make_threshold(15.0, "Decreasing"),
            lower_critical_user: make_threshold(8.0, "Decreasing"),
            upper_fatal: make_threshold(95.0, "Increasing"),
            lower_fatal: make_threshold(0.0, "Decreasing"),
        };

        let json = serde_json::to_value(&thresholds).unwrap();
        assert_eq!(json["UpperCritical"]["Reading"], 90.0);
        assert_eq!(json["UpperCaution"]["Reading"], 80.0);
        assert_eq!(json["LowerCaution"]["Reading"], 10.0);
        assert_eq!(json["LowerCritical"]["Reading"], 5.0);
        assert_eq!(json["UpperCautionUser"]["Reading"], 75.0);
        assert_eq!(json["UpperFatal"]["Reading"], 95.0);
        assert_eq!(json["LowerFatal"]["Reading"], 0.0);
    }
}
