use axum::Json;
use serde::Serialize;

use super::types::Status;

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
            status: Status::enabled_ok(),
        }],
        fans: vec![Fan {
            odata_id: "/redfish/v1/Chassis/1/Thermal#/Fans/0".to_string(),
            member_id: "0",
            name: "System Fan",
            reading: 3000,
            reading_units: "RPM",
            status: Status::enabled_ok(),
        }],
    })
}
