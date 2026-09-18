use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use serde::Serialize;

use super::types::{Collection, ODataId, Status};
use crate::app_state::AppState;
use crate::auth::AuthenticatedUser;

#[derive(Debug, Serialize)]
pub struct ThermalSubsystemResource {
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
    pub odata_id: String,
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
    pub data_source_uri: Option<String>,
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
    pub data_source_uri: Option<String>,
    #[serde(rename = "DeviceName")]
    pub device_name: Option<&'static str>,
}

#[derive(Debug, Serialize)]
pub struct TemperatureReading {
    #[serde(rename = "DataSourceUri")]
    pub data_source_uri: String,
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
    pub odata_id: String,
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
    #[serde(rename = "Assembly")]
    pub assembly: ODataId,
    #[serde(rename = "Status")]
    pub status: Status,
}

pub async fn get_thermal_subsystem(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
) -> Json<ThermalSubsystemResource> {
    let cid = &state.chassis_id;
    Json(ThermalSubsystemResource {
        odata_id: format!("/redfish/v1/Chassis/{cid}/ThermalSubsystem"),
        odata_type: "#ThermalSubsystem.v1_3_0.ThermalSubsystem",
        id: "ThermalSubsystem",
        name: "Thermal Subsystem",
        description: "Thermal subsystem for virtual chassis",
        status: Status::enabled_ok(),
        thermal_metrics: ODataId::new(format!(
            "/redfish/v1/Chassis/{cid}/ThermalSubsystem/ThermalMetrics"
        )),
        fan_redundancy: Vec::new(),
        fans: ODataId::new(format!("/redfish/v1/Chassis/{cid}/ThermalSubsystem/Fans")),
    })
}

pub async fn get_thermal_metrics(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
) -> Json<ThermalMetricsResource> {
    let cid = &state.chassis_id;
    Json(ThermalMetricsResource {
        odata_id: format!("/redfish/v1/Chassis/{cid}/ThermalSubsystem/ThermalMetrics"),
        odata_type: "#ThermalMetrics.v1_3_0.ThermalMetrics",
        id: "ThermalMetrics",
        name: "Thermal Metrics",
        description: "Thermal metrics for virtual chassis",
        temperature_readings_celsius: vec![TemperatureReading {
            data_source_uri: format!("/redfish/v1/Chassis/{cid}/Sensors/CpuTemp"),
            reading: 35,
            device_name: "CPU Temperature",
            physical_context: "CPU",
            physical_sub_context: "Input",
        }],
        temperature_summary_celsius: TemperatureSummary {
            internal: SummaryReading {
                reading: 35.0,
                data_source_uri: Some(format!("/redfish/v1/Chassis/{cid}/Sensors/CpuTemp")),
                device_name: Some("CPU"),
            },
            ambient: SummaryReading {
                reading: 22.0,
                data_source_uri: Some(format!("/redfish/v1/Chassis/{cid}/Sensors/AmbientTemp")),
                device_name: Some("Ambient"),
            },
            exhaust: SummaryReading {
                reading: 28.0,
                data_source_uri: Some(format!("/redfish/v1/Chassis/{cid}/Sensors/ExhaustTemp")),
                device_name: Some("Exhaust"),
            },
            intake: SummaryReading {
                reading: 20.0,
                data_source_uri: Some(format!("/redfish/v1/Chassis/{cid}/Sensors/IntakeTemp")),
                device_name: Some("Intake"),
            },
        },
        air_flow_cubic_meters_per_minute: MetricReading {
            reading: 0.5,
            data_source_uri: None,
            device_name: Some("Chassis Airflow"),
            apparent_va: None,
            phase_angle_degrees: None,
            power_factor: None,
            reactive_var: None,
            apparent_kvah: None,
            lifetime_reading: None,
            reactive_kvarh: None,
            sensor_reset_time: None,
        },
        power_watts: MetricReading {
            reading: 50.0,
            data_source_uri: None,
            device_name: Some("Chassis Power"),
            apparent_va: Some(55.0),
            phase_angle_degrees: Some(0.0),
            power_factor: Some(0.9),
            reactive_var: Some(5.0),
            apparent_kvah: None,
            lifetime_reading: None,
            reactive_kvarh: None,
            sensor_reset_time: None,
        },
        energy_kwh: MetricReading {
            reading: 0.0,
            data_source_uri: None,
            device_name: Some("Chassis Energy"),
            apparent_va: None,
            phase_angle_degrees: None,
            power_factor: None,
            reactive_var: None,
            apparent_kvah: Some(0.0),
            lifetime_reading: Some(0.0),
            reactive_kvarh: Some(0.0),
            sensor_reset_time: Some("2026-01-01T00:00:00Z"),
        },
        delta_pressure_kpa: MetricReading {
            reading: 0.0,
            data_source_uri: None,
            device_name: Some("Differential Pressure"),
            apparent_va: None,
            phase_angle_degrees: None,
            power_factor: None,
            reactive_var: None,
            apparent_kvah: None,
            lifetime_reading: None,
            reactive_kvarh: None,
            sensor_reset_time: None,
        },
    })
}

pub async fn get_fans(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
) -> Json<Collection<ODataId>> {
    let cid = &state.chassis_id;
    let members = vec![ODataId::new(format!(
        "/redfish/v1/Chassis/{cid}/ThermalSubsystem/Fans/0"
    ))];

    Json(Collection::new(
        format!("/redfish/v1/Chassis/{cid}/ThermalSubsystem/Fans"),
        "#FanCollection.FanCollection",
        "Fan Collection",
        members,
    ))
}

pub async fn get_fan(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
) -> Json<FanResource> {
    let cid = &state.chassis_id;
    Json(FanResource {
        odata_id: format!("/redfish/v1/Chassis/{cid}/ThermalSubsystem/Fans/0"),
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
            cooling_chassis: vec![super::types::ODataId::new(format!(
                "/redfish/v1/Chassis/{cid}"
            ))],
        },
        location: super::types::RedfishLocation::new("Fan 0", "Bay", 0),
        part_number: "VBMC-FAN",
        replaceable: false,
        assembly: ODataId::new(format!(
            "/redfish/v1/Chassis/{cid}/ThermalSubsystem/Fans/0/Assembly"
        )),
        status: Status::enabled_ok(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thermal_subsystem_resource_serialization() {
        let resource = ThermalSubsystemResource {
            odata_id: "/redfish/v1/Chassis/test/ThermalSubsystem".to_string(),
            odata_type: "#ThermalSubsystem.v1_3_0.ThermalSubsystem",
            id: "ThermalSubsystem",
            name: "Thermal Subsystem",
            description: "Test thermal subsystem",
            status: Status::enabled_ok(),
            thermal_metrics: ODataId::new(
                "/redfish/v1/Chassis/test/ThermalSubsystem/ThermalMetrics".to_string(),
            ),
            fan_redundancy: Vec::new(),
            fans: ODataId::new("/redfish/v1/Chassis/test/ThermalSubsystem/Fans".to_string()),
        };

        let json = serde_json::to_value(&resource).unwrap();
        assert_eq!(
            json["@odata.id"],
            "/redfish/v1/Chassis/test/ThermalSubsystem"
        );
        assert_eq!(
            json["@odata.type"],
            "#ThermalSubsystem.v1_3_0.ThermalSubsystem"
        );
        assert_eq!(json["Id"], "ThermalSubsystem");
        assert_eq!(json["Name"], "Thermal Subsystem");
    }

    #[test]
    fn test_metric_reading_serialization_minimal() {
        let reading = MetricReading {
            reading: 42.5,
            data_source_uri: None,
            device_name: None,
            apparent_va: None,
            phase_angle_degrees: None,
            power_factor: None,
            reactive_var: None,
            apparent_kvah: None,
            lifetime_reading: None,
            reactive_kvarh: None,
            sensor_reset_time: None,
        };

        let json = serde_json::to_value(&reading).unwrap();
        assert_eq!(json["Reading"], 42.5);
        assert!(json.get("DataSourceUri").is_none());
        assert!(json.get("DeviceName").is_none());
        assert!(json.get("ApparentVA").is_none());
    }

    #[test]
    fn test_metric_reading_serialization_full() {
        let reading = MetricReading {
            reading: 100.0,
            data_source_uri: Some("/redfish/v1/Chassis/test/Sensors/Power".to_string()),
            device_name: Some("Test Device"),
            apparent_va: Some(110.0),
            phase_angle_degrees: Some(5.0),
            power_factor: Some(0.95),
            reactive_var: Some(10.0),
            apparent_kvah: Some(1.5),
            lifetime_reading: Some(1000.0),
            reactive_kvarh: Some(0.5),
            sensor_reset_time: Some("2026-01-01T00:00:00Z"),
        };

        let json = serde_json::to_value(&reading).unwrap();
        assert_eq!(json["Reading"], 100.0);
        assert_eq!(
            json["DataSourceUri"],
            "/redfish/v1/Chassis/test/Sensors/Power"
        );
        assert_eq!(json["DeviceName"], "Test Device");
        assert_eq!(json["ApparentVA"], 110.0);
        assert_eq!(json["PhaseAngleDegrees"], 5.0);
        assert_eq!(json["PowerFactor"], 0.95);
    }

    #[test]
    fn test_temperature_summary_serialization() {
        let summary = TemperatureSummary {
            internal: SummaryReading {
                reading: 35.0,
                data_source_uri: Some("/redfish/v1/Sensors/CPU".to_string()),
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
                device_name: None,
            },
            intake: SummaryReading {
                reading: 20.0,
                data_source_uri: None,
                device_name: None,
            },
        };

        let json = serde_json::to_value(&summary).unwrap();
        assert_eq!(json["Internal"]["Reading"], 35.0);
        assert_eq!(json["Ambient"]["Reading"], 22.0);
        assert_eq!(json["Exhaust"]["Reading"], 28.0);
        assert_eq!(json["Intake"]["Reading"], 20.0);
    }

    #[test]
    fn test_temperature_reading_serialization() {
        let reading = TemperatureReading {
            data_source_uri: "/redfish/v1/Chassis/test/Sensors/CpuTemp".to_string(),
            reading: 45,
            device_name: "CPU",
            physical_context: "CPU",
            physical_sub_context: "Input",
        };

        let json = serde_json::to_value(&reading).unwrap();
        assert_eq!(
            json["DataSourceUri"],
            "/redfish/v1/Chassis/test/Sensors/CpuTemp"
        );
        assert_eq!(json["Reading"], 45);
        assert_eq!(json["DeviceName"], "CPU");
        assert_eq!(json["PhysicalContext"], "CPU");
        assert_eq!(json["PhysicalSubContext"], "Input");
    }

    #[test]
    fn test_sensor_excerpt_serialization_minimal() {
        let excerpt = SensorExcerpt {
            reading: 50.0,
            speed_rpm: None,
            device_name: None,
            data_source_uri: None,
        };

        let json = serde_json::to_value(&excerpt).unwrap();
        assert_eq!(json["Reading"], 50.0);
        assert!(json.get("SpeedRPM").is_none());
        assert!(json.get("DeviceName").is_none());
    }

    #[test]
    fn test_sensor_excerpt_serialization_with_rpm() {
        let excerpt = SensorExcerpt {
            reading: 75.0,
            speed_rpm: Some(4500),
            device_name: Some("Fan 1"),
            data_source_uri: Some("/redfish/v1/Sensors/Fan1"),
        };

        let json = serde_json::to_value(&excerpt).unwrap();
        assert_eq!(json["Reading"], 75.0);
        assert_eq!(json["SpeedRPM"], 4500);
        assert_eq!(json["DeviceName"], "Fan 1");
        assert_eq!(json["DataSourceUri"], "/redfish/v1/Sensors/Fan1");
    }

    #[test]
    fn test_fan_resource_serialization() {
        let fan = FanResource {
            odata_id: "/redfish/v1/Chassis/test/ThermalSubsystem/Fans/0".to_string(),
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
                device_name: None,
                data_source_uri: None,
            },
            power_watts_fan: SensorExcerpt {
                reading: 5.0,
                speed_rpm: None,
                device_name: None,
                data_source_uri: None,
            },
            fan_links: FanLinks {
                cooling_chassis: Vec::new(),
            },
            location: crate::redfish::types::RedfishLocation::new("Fan 0", "Bay", 0),
            part_number: "VBMC-FAN",
            replaceable: false,
            assembly: ODataId::new("/redfish/v1/Assembly".to_string()),
            status: Status::enabled_ok(),
        };

        let json = serde_json::to_value(&fan).unwrap();
        assert_eq!(
            json["@odata.id"],
            "/redfish/v1/Chassis/test/ThermalSubsystem/Fans/0"
        );
        assert_eq!(json["@odata.type"], "#Fan.v1_5_0.Fan");
        assert_eq!(json["Id"], "0");
        assert_eq!(json["FanDiameterMm"], 120);
        assert_eq!(json["HotPluggable"], false);
        assert_eq!(json["SpeedPercent"]["Reading"], 50.0);
        assert_eq!(json["SpeedPercent"]["SpeedRPM"], 3000);
    }

    #[test]
    fn test_fan_links_serialization() {
        let links = FanLinks {
            cooling_chassis: vec![
                ODataId::new("/redfish/v1/Chassis/1".to_string()),
                ODataId::new("/redfish/v1/Chassis/2".to_string()),
            ],
        };

        let json = serde_json::to_value(&links).unwrap();
        assert_eq!(json["CoolingChassis"].as_array().unwrap().len(), 2);
    }
}

#[cfg(test)]
mod harness_tests {
    use crate::backend::mock::MockBackend;
    use crate::redfish::test_harness as h;
    use axum::http::StatusCode;

    #[tokio::test]
    async fn test_get_thermal_subsystem() {
        let state = h::app_state(MockBackend::new(), h::systems_with("sys"));
        let app = h::router(state);

        let (status, json, _) = h::get(&app, "/redfish/v1/Chassis/1/ThermalSubsystem").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["@odata.id"], "/redfish/v1/Chassis/1/ThermalSubsystem");
        assert_eq!(
            json["@odata.type"],
            "#ThermalSubsystem.v1_3_0.ThermalSubsystem"
        );
        assert_eq!(json["Id"], "ThermalSubsystem");
        assert_eq!(json["Name"], "Thermal Subsystem");
        assert_eq!(json["Status"]["State"], "Enabled");
        assert_eq!(
            json["ThermalMetrics"]["@odata.id"],
            "/redfish/v1/Chassis/1/ThermalSubsystem/ThermalMetrics"
        );
        assert_eq!(
            json["Fans"]["@odata.id"],
            "/redfish/v1/Chassis/1/ThermalSubsystem/Fans"
        );
    }

    #[tokio::test]
    async fn test_get_thermal_metrics() {
        let state = h::app_state(MockBackend::new(), h::systems_with("sys"));
        let app = h::router(state);

        let (status, json, _) = h::get(
            &app,
            "/redfish/v1/Chassis/1/ThermalSubsystem/ThermalMetrics",
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            json["@odata.id"],
            "/redfish/v1/Chassis/1/ThermalSubsystem/ThermalMetrics"
        );
        assert_eq!(json["@odata.type"], "#ThermalMetrics.v1_3_0.ThermalMetrics");
        assert_eq!(json["Id"], "ThermalMetrics");
        assert!(json["TemperatureReadingsCelsius"].is_array());
        assert_eq!(json["TemperatureReadingsCelsius"][0]["Reading"], 35);
        assert_eq!(
            json["TemperatureReadingsCelsius"][0]["PhysicalContext"],
            "CPU"
        );
        assert_eq!(
            json["TemperatureSummaryCelsius"]["Internal"]["Reading"],
            35.0
        );
        assert_eq!(
            json["TemperatureSummaryCelsius"]["Ambient"]["Reading"],
            22.0
        );
        assert_eq!(json["AirFlowCubicMetersPerMinute"]["Reading"], 0.5);
        assert_eq!(json["PowerWatts"]["Reading"], 50.0);
        assert_eq!(json["PowerWatts"]["ApparentVA"], 55.0);
        assert_eq!(json["PowerWatts"]["PowerFactor"], 0.9);
    }

    #[tokio::test]
    async fn test_get_fans_collection() {
        let state = h::app_state(MockBackend::new(), h::systems_with("sys"));
        let app = h::router(state);

        let (status, json, _) = h::get(&app, "/redfish/v1/Chassis/1/ThermalSubsystem/Fans").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            json["@odata.id"],
            "/redfish/v1/Chassis/1/ThermalSubsystem/Fans"
        );
        assert_eq!(json["@odata.type"], "#FanCollection.FanCollection");
        assert_eq!(json["Name"], "Fan Collection");
        assert!(json["Members"].is_array());
        assert_eq!(json["Members"].as_array().unwrap().len(), 1);
        assert_eq!(
            json["Members"][0]["@odata.id"],
            "/redfish/v1/Chassis/1/ThermalSubsystem/Fans/0"
        );
        assert_eq!(json["Members@odata.count"], 1);
    }

    #[tokio::test]
    async fn test_get_fan() {
        let state = h::app_state(MockBackend::new(), h::systems_with("sys"));
        let app = h::router(state);

        let (status, json, _) = h::get(&app, "/redfish/v1/Chassis/1/ThermalSubsystem/Fans/0").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            json["@odata.id"],
            "/redfish/v1/Chassis/1/ThermalSubsystem/Fans/0"
        );
        assert_eq!(json["@odata.type"], "#Fan.v1_5_0.Fan");
        assert_eq!(json["Id"], "0");
        assert_eq!(json["Name"], "System Fan");
        assert_eq!(json["Manufacturer"], "vbmc-rs");
        assert_eq!(json["Model"], "Virtual Fan");
        assert_eq!(json["SerialNumber"], "VBMC-FAN-001");
        assert_eq!(json["PhysicalContext"], "Exhaust");
        assert_eq!(json["FanDiameterMm"], 120);
        assert_eq!(json["HotPluggable"], false);
        assert_eq!(json["SpeedPercent"]["Reading"], 50.0);
        assert_eq!(json["SpeedPercent"]["SpeedRPM"], 3000);
        assert_eq!(json["Status"]["State"], "Enabled");
        assert!(json["Links"]["CoolingChassis"].is_array());
        assert_eq!(
            json["Assembly"]["@odata.id"],
            "/redfish/v1/Chassis/1/ThermalSubsystem/Fans/0/Assembly"
        );
    }
}
