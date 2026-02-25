use axum::Json;
use serde::Serialize;

use super::types::{Collection, ODataId, Status};

#[derive(Debug, Serialize)]
pub struct ThermalSubsystemResource {
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
    #[serde(rename = "ThermalMetrics")]
    pub thermal_metrics: ODataId,
    #[serde(rename = "FanRedundancy")]
    pub fan_redundancy: Vec<serde_json::Value>,
    #[serde(rename = "Fans")]
    pub fans: ODataId,
}

#[derive(Debug, Serialize)]
pub struct ThermalMetricsResource {
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
    #[serde(rename = "TemperatureReadingsCelsius")]
    pub temperature_readings_celsius: Vec<TemperatureReading>,
    #[serde(rename = "TemperatureSummaryCelsius")]
    pub temperature_summary_celsius: TemperatureSummary,
    #[serde(rename = "AirFlowCubicMetersPerMinute")]
    pub air_flow_cubic_meters_per_minute: MetricReading,
    #[serde(rename = "PowerWatts")]
    pub power_watts: MetricReading,
    #[serde(rename = "EnergykWh")]
    pub energy_kwh: MetricReading,
    #[serde(rename = "DeltaPressurekPa")]
    pub delta_pressure_kpa: MetricReading,
}

#[derive(Debug, Serialize)]
pub struct MetricReading {
    #[serde(rename = "Reading")]
    pub reading: f64,
    #[serde(rename = "DataSourceUri", skip_serializing_if = "Option::is_none")]
    pub data_source_uri: Option<&'static str>,
    #[serde(rename = "DeviceName", skip_serializing_if = "Option::is_none")]
    pub device_name: Option<&'static str>,
    #[serde(rename = "ApparentVA", skip_serializing_if = "Option::is_none")]
    pub apparent_va: Option<f64>,
    #[serde(rename = "PhaseAngleDegrees", skip_serializing_if = "Option::is_none")]
    pub phase_angle_degrees: Option<f64>,
    #[serde(rename = "PowerFactor", skip_serializing_if = "Option::is_none")]
    pub power_factor: Option<f64>,
    #[serde(rename = "ReactiveVAR", skip_serializing_if = "Option::is_none")]
    pub reactive_var: Option<f64>,
    #[serde(rename = "ApparentkVAh", skip_serializing_if = "Option::is_none")]
    pub apparent_kvah: Option<f64>,
    #[serde(rename = "LifetimeReading", skip_serializing_if = "Option::is_none")]
    pub lifetime_reading: Option<f64>,
    #[serde(rename = "ReactivekVARh", skip_serializing_if = "Option::is_none")]
    pub reactive_kvarh: Option<f64>,
    #[serde(rename = "SensorResetTime", skip_serializing_if = "Option::is_none")]
    pub sensor_reset_time: Option<&'static str>,
}

#[derive(Debug, Serialize)]
pub struct TemperatureSummary {
    #[serde(rename = "Internal")]
    pub internal: SummaryReading,
    #[serde(rename = "Ambient")]
    pub ambient: SummaryReading,
    #[serde(rename = "Exhaust")]
    pub exhaust: SummaryReading,
    #[serde(rename = "Intake")]
    pub intake: SummaryReading,
}

#[derive(Debug, Serialize)]
pub struct SummaryReading {
    #[serde(rename = "Reading")]
    pub reading: f64,
    #[serde(rename = "DataSourceUri")]
    pub data_source_uri: Option<&'static str>,
    #[serde(rename = "DeviceName")]
    pub device_name: Option<&'static str>,
}

#[derive(Debug, Serialize)]
pub struct TemperatureReading {
    #[serde(rename = "DataSourceUri")]
    pub data_source_uri: &'static str,
    #[serde(rename = "Reading")]
    pub reading: u32,
    #[serde(rename = "DeviceName")]
    pub device_name: &'static str,
    #[serde(rename = "PhysicalContext")]
    pub physical_context: &'static str,
    #[serde(rename = "PhysicalSubContext")]
    pub physical_sub_context: &'static str,
}

#[derive(Debug, Serialize)]
pub struct SensorExcerpt {
    #[serde(rename = "Reading")]
    pub reading: f64,
    #[serde(rename = "SpeedRPM", skip_serializing_if = "Option::is_none")]
    pub speed_rpm: Option<u32>,
    #[serde(rename = "DeviceName", skip_serializing_if = "Option::is_none")]
    pub device_name: Option<&'static str>,
    #[serde(rename = "DataSourceUri", skip_serializing_if = "Option::is_none")]
    pub data_source_uri: Option<&'static str>,
}

#[derive(Debug, Serialize)]
pub struct FanLinks {
    #[serde(rename = "CoolingChassis")]
    pub cooling_chassis: Vec<super::types::ODataId>,
}

#[derive(Debug, Serialize)]
pub struct FanResource {
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
    #[serde(rename = "SpeedPercent")]
    pub speed_percent: SensorExcerpt,
    #[serde(rename = "Manufacturer")]
    pub manufacturer: &'static str,
    #[serde(rename = "Model")]
    pub model: &'static str,
    #[serde(rename = "SerialNumber")]
    pub serial_number: &'static str,
    #[serde(rename = "PhysicalContext")]
    pub physical_context: &'static str,
    #[serde(rename = "HotPluggable")]
    pub hot_pluggable: bool,
    #[serde(rename = "FanDiameterMm")]
    pub fan_diameter_mm: u32,
    #[serde(rename = "LocationIndicatorActive")]
    pub location_indicator_active: bool,
    #[serde(rename = "SparePartNumber")]
    pub spare_part_number: &'static str,
    #[serde(rename = "SecondarySpeedPercent")]
    pub secondary_speed_percent: SensorExcerpt,
    #[serde(rename = "PowerWatts")]
    pub power_watts_fan: SensorExcerpt,
    #[serde(rename = "Links")]
    pub fan_links: FanLinks,
    #[serde(rename = "Location")]
    pub location: super::types::RedfishLocation,
    #[serde(rename = "PartNumber")]
    pub part_number: &'static str,
    #[serde(rename = "Replaceable")]
    pub replaceable: bool,
    #[serde(rename = "Status")]
    pub status: Status,
}


pub async fn get_thermal_subsystem() -> Json<ThermalSubsystemResource> {
    Json(ThermalSubsystemResource {
        odata_id: "/redfish/v1/Chassis/1/ThermalSubsystem",
        odata_type: "#ThermalSubsystem.v1_3_0.ThermalSubsystem",
        id: "ThermalSubsystem",
        name: "Thermal Subsystem",
        description: "Thermal subsystem for virtual chassis",
        status: Status::enabled_ok(),
        thermal_metrics: ODataId::new(
            "/redfish/v1/Chassis/1/ThermalSubsystem/ThermalMetrics",
        ),
        fan_redundancy: Vec::new(),
        fans: ODataId::new("/redfish/v1/Chassis/1/ThermalSubsystem/Fans"),
    })
}

pub async fn get_thermal_metrics() -> Json<ThermalMetricsResource> {
    Json(ThermalMetricsResource {
        odata_id: "/redfish/v1/Chassis/1/ThermalSubsystem/ThermalMetrics",
        odata_type: "#ThermalMetrics.v1_3_0.ThermalMetrics",
        id: "ThermalMetrics",
        name: "Thermal Metrics",
        description: "Thermal metrics for virtual chassis",
        temperature_readings_celsius: vec![TemperatureReading {
            data_source_uri: "/redfish/v1/Chassis/1/Thermal#/Temperatures/0",
            reading: 35,
            device_name: "CPU Temperature",
            physical_context: "CPU",
            physical_sub_context: "Input",
        }],
        temperature_summary_celsius: TemperatureSummary {
            internal: SummaryReading {
                reading: 35.0,
                data_source_uri: None,
                device_name: Some("CPU"),
            },
            ambient: SummaryReading {
                reading: 22.0,
                data_source_uri: None,
                device_name: Some("Ambient"),
            },
            exhaust: SummaryReading {
                reading: 28.0,
                data_source_uri: None,
                device_name: Some("Exhaust"),
            },
            intake: SummaryReading {
                reading: 20.0,
                data_source_uri: None,
                device_name: Some("Intake"),
            },
        },
        air_flow_cubic_meters_per_minute: MetricReading {
            reading: 0.5,
            data_source_uri: None,
            device_name: Some("Chassis Airflow"),
            apparent_va: None, phase_angle_degrees: None, power_factor: None,
            reactive_var: None, apparent_kvah: None, lifetime_reading: None,
            reactive_kvarh: None, sensor_reset_time: None,
        },
        power_watts: MetricReading {
            reading: 50.0,
            data_source_uri: None,
            device_name: Some("Chassis Power"),
            apparent_va: Some(55.0), phase_angle_degrees: Some(0.0),
            power_factor: Some(0.9), reactive_var: Some(5.0),
            apparent_kvah: None, lifetime_reading: None,
            reactive_kvarh: None, sensor_reset_time: None,
        },
        energy_kwh: MetricReading {
            reading: 0.0,
            data_source_uri: None,
            device_name: Some("Chassis Energy"),
            apparent_va: None, phase_angle_degrees: None, power_factor: None,
            reactive_var: None, apparent_kvah: Some(0.0),
            lifetime_reading: Some(0.0), reactive_kvarh: Some(0.0),
            sensor_reset_time: Some("2026-01-01T00:00:00Z"),
        },
        delta_pressure_kpa: MetricReading {
            reading: 0.0,
            data_source_uri: None, device_name: Some("Differential Pressure"),
            apparent_va: None, phase_angle_degrees: None, power_factor: None,
            reactive_var: None, apparent_kvah: None, lifetime_reading: None,
            reactive_kvarh: None, sensor_reset_time: None,
        },
    })
}

pub async fn get_fans() -> Json<Collection<ODataId>> {
    let members = vec![ODataId::new(
        "/redfish/v1/Chassis/1/ThermalSubsystem/Fans/0",
    )];

    Json(Collection::new(
        "/redfish/v1/Chassis/1/ThermalSubsystem/Fans",
        "#FanCollection.FanCollection",
        "Fan Collection",
        members,
    ))
}

pub async fn get_fan() -> Json<FanResource> {
    Json(FanResource {
        odata_id: "/redfish/v1/Chassis/1/ThermalSubsystem/Fans/0",
        odata_type: "#Fan.v1_5_0.Fan",
        id: "0",
        name: "System Fan",
        description: "Virtual cooling fan",
        speed_percent: SensorExcerpt {
            reading: 50.0,
            speed_rpm: Some(3000),
            device_name: Some("System Fan"),
            data_source_uri: None,
        },
        manufacturer: "vbmc-rs",
        model: "Virtual Fan",
        serial_number: "VBMC-FAN-001",
        physical_context: "Exhaust",
        hot_pluggable: false,
        fan_diameter_mm: 120,
        location_indicator_active: false,
        spare_part_number: "VBMC-FAN-SPARE",
        secondary_speed_percent: SensorExcerpt {
            reading: 50.0,
            speed_rpm: None,
            device_name: Some("System Fan Secondary"),
            data_source_uri: None,
        },
        power_watts_fan: SensorExcerpt {
            reading: 5.0,
            speed_rpm: None,
            device_name: Some("Fan Power"),
            data_source_uri: None,
        },
        fan_links: FanLinks {
            cooling_chassis: vec![super::types::ODataId::new("/redfish/v1/Chassis/1")],
        },
        location: super::types::RedfishLocation::new("Bay 1", "Bay", "Fan 0", "Bay", 0),
        part_number: "VBMC-FAN",
        replaceable: false,
        status: Status::enabled_ok(),
    })
}
