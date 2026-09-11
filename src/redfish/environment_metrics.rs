use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use serde::Serialize;

use super::types::ODataId;
use crate::app_state::AppState;
use crate::auth::AuthenticatedUser;

#[derive(Debug, Serialize)]
pub struct EnvironmentMetricsResource {
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
    #[serde(rename = "TemperatureCelsius")]
    pub temperature_celsius: SensorExcerpt,
    #[serde(rename = "HumidityPercent")]
    pub humidity_percent: SensorExcerpt,
    #[serde(rename = "PowerWatts")]
    pub power_watts: SensorExcerpt,
    #[serde(rename = "FanSpeedsPercent")]
    pub fan_speeds_percent: Vec<SensorExcerpt>,
    #[serde(rename = "PowerLimitWatts")]
    pub power_limit_watts: ControlExcerpt,
}

#[derive(Debug, Serialize)]
pub struct SensorExcerpt {
    #[serde(rename = "DataSourceUri")]
    pub data_source_uri: ODataId,
    #[serde(rename = "Reading")]
    pub reading: f64,
}

#[derive(Debug, Serialize)]
pub struct ControlExcerpt {
    #[serde(rename = "SetPoint")]
    pub set_point: u32,
    #[serde(rename = "ControlMode")]
    pub control_mode: &'static str,
}

pub async fn get_environment_metrics(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
) -> Json<EnvironmentMetricsResource> {
    let cid = &state.chassis_id;
    Json(EnvironmentMetricsResource {
        odata_id: format!("/redfish/v1/Chassis/{cid}/EnvironmentMetrics"),
        odata_type: "#EnvironmentMetrics.v1_3_0.EnvironmentMetrics",
        id: "EnvironmentMetrics",
        name: "Chassis Environment Metrics",
        description: "Environmental metrics for the virtual chassis",
        temperature_celsius: SensorExcerpt {
            data_source_uri: ODataId::new(format!(
                "/redfish/v1/Chassis/{cid}/Sensors/AmbientTemp"
            )),
            reading: 25.0,
        },
        humidity_percent: SensorExcerpt {
            data_source_uri: ODataId::new(format!(
                "/redfish/v1/Chassis/{cid}/Sensors/AmbientTemp"
            )),
            reading: 45.0,
        },
        power_watts: SensorExcerpt {
            data_source_uri: ODataId::new(format!(
                "/redfish/v1/Chassis/{cid}/Sensors/ChassisPower"
            )),
            reading: 120.0,
        },
        fan_speeds_percent: vec![SensorExcerpt {
            data_source_uri: ODataId::new(format!(
                "/redfish/v1/Chassis/{cid}/Sensors/SystemFanSpeed"
            )),
            reading: 40.0,
        }],
        power_limit_watts: ControlExcerpt {
            set_point: 500,
            control_mode: "Automatic",
        },
    })
}
