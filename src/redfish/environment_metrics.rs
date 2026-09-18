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
            data_source_uri: ODataId::new(format!("/redfish/v1/Chassis/{cid}/Sensors/AmbientTemp")),
            reading: 25.0,
        },
        humidity_percent: SensorExcerpt {
            data_source_uri: ODataId::new(format!("/redfish/v1/Chassis/{cid}/Sensors/AmbientTemp")),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_environment_metrics_resource_serialization() {
        let resource = EnvironmentMetricsResource {
            odata_id: "/redfish/v1/Chassis/test/EnvironmentMetrics".to_string(),
            odata_type: "#EnvironmentMetrics.v1_3_0.EnvironmentMetrics",
            id: "EnvironmentMetrics",
            name: "Chassis Environment Metrics",
            description: "Environmental metrics for the virtual chassis",
            temperature_celsius: SensorExcerpt {
                data_source_uri: ODataId::new(
                    "/redfish/v1/Chassis/test/Sensors/AmbientTemp".to_string(),
                ),
                reading: 25.0,
            },
            humidity_percent: SensorExcerpt {
                data_source_uri: ODataId::new(
                    "/redfish/v1/Chassis/test/Sensors/Humidity".to_string(),
                ),
                reading: 45.0,
            },
            power_watts: SensorExcerpt {
                data_source_uri: ODataId::new("/redfish/v1/Chassis/test/Sensors/Power".to_string()),
                reading: 120.0,
            },
            fan_speeds_percent: vec![SensorExcerpt {
                data_source_uri: ODataId::new("/redfish/v1/Chassis/test/Sensors/Fan".to_string()),
                reading: 40.0,
            }],
            power_limit_watts: ControlExcerpt {
                set_point: 500,
                control_mode: "Automatic",
            },
        };

        let json = serde_json::to_value(&resource).unwrap();
        assert_eq!(
            json["@odata.id"],
            "/redfish/v1/Chassis/test/EnvironmentMetrics"
        );
        assert_eq!(
            json["@odata.type"],
            "#EnvironmentMetrics.v1_3_0.EnvironmentMetrics"
        );
        assert_eq!(json["Id"], "EnvironmentMetrics");
        assert_eq!(json["Name"], "Chassis Environment Metrics");
        assert_eq!(json["TemperatureCelsius"]["Reading"], 25.0);
        assert_eq!(json["HumidityPercent"]["Reading"], 45.0);
        assert_eq!(json["PowerWatts"]["Reading"], 120.0);
    }

    #[test]
    fn test_sensor_excerpt_serialization() {
        let excerpt = SensorExcerpt {
            data_source_uri: ODataId::new("/redfish/v1/Chassis/test/Sensors/CpuTemp".to_string()),
            reading: 42.5,
        };

        let json = serde_json::to_value(&excerpt).unwrap();
        assert_eq!(json["Reading"], 42.5);
        assert_eq!(
            json["DataSourceUri"]["@odata.id"],
            "/redfish/v1/Chassis/test/Sensors/CpuTemp"
        );
    }

    #[test]
    fn test_control_excerpt_serialization() {
        let control = ControlExcerpt {
            set_point: 750,
            control_mode: "Manual",
        };

        let json = serde_json::to_value(&control).unwrap();
        assert_eq!(json["SetPoint"], 750);
        assert_eq!(json["ControlMode"], "Manual");
    }

    #[test]
    fn test_fan_speeds_percent_array() {
        let resource = EnvironmentMetricsResource {
            odata_id: "/redfish/v1/Chassis/test/EnvironmentMetrics".to_string(),
            odata_type: "#EnvironmentMetrics.v1_3_0.EnvironmentMetrics",
            id: "EnvironmentMetrics",
            name: "Test",
            description: "Test",
            temperature_celsius: SensorExcerpt {
                data_source_uri: ODataId::new("/redfish/v1/Sensors/Temp".to_string()),
                reading: 25.0,
            },
            humidity_percent: SensorExcerpt {
                data_source_uri: ODataId::new("/redfish/v1/Sensors/Humidity".to_string()),
                reading: 50.0,
            },
            power_watts: SensorExcerpt {
                data_source_uri: ODataId::new("/redfish/v1/Sensors/Power".to_string()),
                reading: 100.0,
            },
            fan_speeds_percent: vec![
                SensorExcerpt {
                    data_source_uri: ODataId::new("/redfish/v1/Sensors/Fan1".to_string()),
                    reading: 40.0,
                },
                SensorExcerpt {
                    data_source_uri: ODataId::new("/redfish/v1/Sensors/Fan2".to_string()),
                    reading: 50.0,
                },
                SensorExcerpt {
                    data_source_uri: ODataId::new("/redfish/v1/Sensors/Fan3".to_string()),
                    reading: 60.0,
                },
            ],
            power_limit_watts: ControlExcerpt {
                set_point: 1000,
                control_mode: "Automatic",
            },
        };

        let json = serde_json::to_value(&resource).unwrap();
        let fan_speeds = json["FanSpeedsPercent"].as_array().unwrap();
        assert_eq!(fan_speeds.len(), 3);
        assert_eq!(fan_speeds[0]["Reading"], 40.0);
        assert_eq!(fan_speeds[1]["Reading"], 50.0);
        assert_eq!(fan_speeds[2]["Reading"], 60.0);
    }
}
