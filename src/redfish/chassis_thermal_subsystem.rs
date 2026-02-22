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
}

#[derive(Debug, Serialize)]
pub struct TemperatureSummary {
    #[serde(rename = "Internal")]
    pub internal: SummaryReading,
    #[serde(rename = "Ambient")]
    pub ambient: SummaryReading,
}

#[derive(Debug, Serialize)]
pub struct SummaryReading {
    #[serde(rename = "Reading")]
    pub reading: f64,
    #[serde(rename = "DataSourceUri", skip_serializing_if = "Option::is_none")]
    pub data_source_uri: Option<&'static str>,
}

#[derive(Debug, Serialize)]
pub struct TemperatureReading {
    #[serde(rename = "DataSourceUri")]
    pub data_source_uri: &'static str,
    #[serde(rename = "Reading")]
    pub reading: u32,
}

#[derive(Debug, Serialize)]
pub struct SensorExcerpt {
    #[serde(rename = "Reading")]
    pub reading: f64,
    #[serde(rename = "SpeedRPM", skip_serializing_if = "Option::is_none")]
    pub speed_rpm: Option<u32>,
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
    #[serde(rename = "Location")]
    pub location: FanLocation,
    #[serde(rename = "PartNumber")]
    pub part_number: &'static str,
    #[serde(rename = "Replaceable")]
    pub replaceable: bool,
    #[serde(rename = "Status")]
    pub status: Status,
}

#[derive(Debug, Serialize)]
pub struct FanLocation {
    #[serde(rename = "Info")]
    pub info: &'static str,
    #[serde(rename = "InfoFormat")]
    pub info_format: &'static str,
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
        }],
        temperature_summary_celsius: TemperatureSummary {
            internal: SummaryReading {
                reading: 35.0,
                data_source_uri: None,
            },
            ambient: SummaryReading {
                reading: 22.0,
                data_source_uri: None,
            },
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
        },
        manufacturer: "vbmc-rs",
        model: "Virtual Fan",
        serial_number: "VBMC-FAN-001",
        physical_context: "Exhaust",
        hot_pluggable: false,
        location: FanLocation {
            info: "Bay 1",
            info_format: "Bay",
        },
        part_number: "VBMC-FAN",
        replaceable: false,
        status: Status::enabled_ok(),
    })
}
