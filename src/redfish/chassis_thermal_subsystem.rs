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
}

#[derive(Debug, Serialize)]
pub struct TemperatureReading {
    #[serde(rename = "DataSourceUri")]
    pub data_source_uri: &'static str,
    #[serde(rename = "Reading")]
    pub reading: u32,
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
    #[serde(rename = "SpeedRPM")]
    pub speed_rpm: u32,
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
        speed_rpm: 3000,
        status: Status::enabled_ok(),
    })
}
