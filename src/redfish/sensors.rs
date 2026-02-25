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
    #[serde(rename = "ElectricalContext", skip_serializing_if = "Option::is_none")]
    pub electrical_context: Option<&'static str>,
    #[serde(rename = "VoltageType", skip_serializing_if = "Option::is_none")]
    pub voltage_type: Option<&'static str>,
    #[serde(rename = "SpeedRPM", skip_serializing_if = "Option::is_none")]
    pub speed_rpm: Option<f64>,
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
    #[serde(rename = "LifetimeStartDateTime")]
    pub lifetime_start_date_time: &'static str,
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
}

#[derive(Debug, Serialize)]
pub struct ThresholdValue {
    #[serde(rename = "Reading")]
    pub reading: f64,
    #[serde(rename = "Activation")]
    pub activation: &'static str,
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

pub async fn get_sensor(
    Path(sensor_id): Path<String>,
) -> Result<Json<SensorResource>, RedfishApiError> {
    let def = SENSORS
        .iter()
        .find(|s| s.id == sensor_id)
        .ok_or_else(|| RedfishApiError::NotFound(format!("Sensor '{sensor_id}' not found")))?;

    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

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
            upper_critical: ThresholdValue {
                reading: def.upper_critical,
                activation: "Increasing",
            },
            upper_caution: ThresholdValue {
                reading: def.upper_caution,
                activation: "Increasing",
            },
            lower_caution: ThresholdValue {
                reading: def.lower_caution,
                activation: "Decreasing",
            },
            lower_critical: ThresholdValue {
                reading: def.lower_critical,
                activation: "Decreasing",
            },
        },
        electrical_context: def.electrical_context,
        voltage_type: def.voltage_type,
        speed_rpm: def.speed_rpm,
        manufacturer: "vbmc-rs",
        model: "Virtual Sensor",
        serial_number: format!("VBMC-SENS-{}", def.id),
        part_number: "VBMC-SENS",
        sku: "VBMC-VIRTUAL",
        spare_part_number: "VBMC-SENS-SPARE",
        user_label: def.name.to_string(),
        lifetime_start_date_time: "2026-01-01T00:00:00Z",
        location: super::types::RedfishLocation::new(
            def.name, "Embedded", def.id, "Embedded", 0,
        ),
        status: Status::enabled_ok(),
    }))
}
