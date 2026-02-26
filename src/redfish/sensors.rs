use axum::extract::Path;
use axum::Json;
use serde::Serialize;

use super::error::RedfishApiError;
use super::types::{Collection, ODataId, Status};

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

pub async fn get_sensors() -> Json<Collection<ODataId>> {
    let members: Vec<ODataId> = SENSORS
        .iter()
        .map(|s| ODataId::new(format!("/redfish/v1/Chassis/1/Sensors/{}", s.id)))
        .collect();

    Json(Collection::new(
        "/redfish/v1/Chassis/1/Sensors",
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
    Path(sensor_id): Path<String>,
) -> Result<Json<SensorResource>, RedfishApiError> {
    let def = SENSORS
        .iter()
        .find(|s| s.id == sensor_id)
        .ok_or_else(|| RedfishApiError::NotFound(format!("Sensor '{sensor_id}' not found")))?;

    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

    let is_power = def.is_electrical && def.reading_type == "Power";
    let is_electrical = def.is_electrical;

    Ok(Json(SensorResource {
        odata_id: format!("/redfish/v1/Chassis/1/Sensors/{}", def.id),
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
        related_item: vec![ODataId::new("/redfish/v1/Chassis/1")],
        location: super::types::RedfishLocation::new(
            def.name, "Embedded", def.id, "Embedded", 0,
        ),
        status: Status::enabled_ok(),
    }))
}
