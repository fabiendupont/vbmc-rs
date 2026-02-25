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
    #[serde(rename = "Status")]
    pub status: Status,
}

struct SensorDef {
    id: &'static str,
    name: &'static str,
    reading: f64,
    reading_type: &'static str,
    reading_units: &'static str,
    physical_context: &'static str,
}

const SENSORS: &[SensorDef] = &[
    SensorDef {
        id: "CpuTemp",
        name: "CPU Temperature",
        reading: 35.0,
        reading_type: "Temperature",
        reading_units: "Cel",
        physical_context: "CPU",
    },
    SensorDef {
        id: "AmbientTemp",
        name: "Ambient Temperature",
        reading: 22.0,
        reading_type: "Temperature",
        reading_units: "Cel",
        physical_context: "Room",
    },
    SensorDef {
        id: "ExhaustTemp",
        name: "Exhaust Temperature",
        reading: 28.0,
        reading_type: "Temperature",
        reading_units: "Cel",
        physical_context: "Exhaust",
    },
    SensorDef {
        id: "IntakeTemp",
        name: "Intake Temperature",
        reading: 20.0,
        reading_type: "Temperature",
        reading_units: "Cel",
        physical_context: "Intake",
    },
    SensorDef {
        id: "SystemFanSpeed",
        name: "System Fan Speed",
        reading: 3000.0,
        reading_type: "Rotational",
        reading_units: "RPM",
        physical_context: "Exhaust",
    },
    SensorDef {
        id: "ChassisPower",
        name: "Chassis Power",
        reading: 50.0,
        reading_type: "Power",
        reading_units: "W",
        physical_context: "Chassis",
    },
    SensorDef {
        id: "Voltage12V",
        name: "12V Rail Voltage",
        reading: 12.1,
        reading_type: "Voltage",
        reading_units: "V",
        physical_context: "PowerSupply",
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
        status: Status::enabled_ok(),
    }))
}
