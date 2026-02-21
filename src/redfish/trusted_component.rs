use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use serde::Serialize;

use super::error::RedfishApiError;
use super::types::{Collection, ODataId, Status};
use crate::app_state::AppState;

#[derive(Debug, Serialize)]
pub struct ChassisResource {
    #[serde(rename = "@odata.id")]
    pub odata_id: String,
    #[serde(rename = "@odata.type")]
    pub odata_type: &'static str,
    #[serde(rename = "Id")]
    pub id: &'static str,
    #[serde(rename = "Name")]
    pub name: &'static str,
    #[serde(rename = "ChassisType")]
    pub chassis_type: &'static str,
    #[serde(rename = "Status")]
    pub status: Status,
    #[serde(rename = "TrustedComponents")]
    pub trusted_components: ODataId,
}

pub async fn get_chassis_collection() -> Json<Collection<ODataId>> {
    let members = vec![ODataId::new("/redfish/v1/Chassis/1")];
    Json(Collection::new(
        "/redfish/v1/Chassis",
        "#ChassisCollection.ChassisCollection",
        "Chassis Collection",
        members,
    ))
}

pub async fn get_chassis() -> Json<ChassisResource> {
    Json(ChassisResource {
        odata_id: "/redfish/v1/Chassis/1".to_string(),
        odata_type: "#Chassis.v1_25_0.Chassis",
        id: "1",
        name: "Virtual Chassis",
        chassis_type: "RackMount",
        status: Status::enabled_ok(),
        trusted_components: ODataId::new("/redfish/v1/Chassis/1/TrustedComponents"),
    })
}

pub async fn get_trusted_components(
    State(state): State<Arc<AppState>>,
) -> Json<Collection<ODataId>> {
    let members: Vec<ODataId> = state
        .config
        .systems
        .keys()
        .map(|id| {
            ODataId::new(format!(
                "/redfish/v1/Chassis/1/TrustedComponents/{id}"
            ))
        })
        .collect();

    Json(Collection::new(
        "/redfish/v1/Chassis/1/TrustedComponents",
        "#TrustedComponentCollection.TrustedComponentCollection",
        "Trusted Component Collection",
        members,
    ))
}

#[derive(Debug, Serialize)]
pub struct TrustedComponentResource {
    #[serde(rename = "@odata.id")]
    pub odata_id: String,
    #[serde(rename = "@odata.type")]
    pub odata_type: &'static str,
    #[serde(rename = "Id")]
    pub id: String,
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "TrustedComponentType")]
    pub trusted_component_type: &'static str,
    #[serde(rename = "Status")]
    pub status: Status,
}

pub async fn get_trusted_component(
    State(state): State<Arc<AppState>>,
    Path(component_id): Path<String>,
) -> Result<Json<TrustedComponentResource>, RedfishApiError> {
    if !state.config.systems.contains_key(&component_id) {
        return Err(RedfishApiError::NotFound(format!(
            "TrustedComponent '{component_id}' not found"
        )));
    }

    Ok(Json(TrustedComponentResource {
        odata_id: format!(
            "/redfish/v1/Chassis/1/TrustedComponents/{component_id}"
        ),
        odata_type: "#TrustedComponent.v1_3_0.TrustedComponent",
        id: component_id.clone(),
        name: format!("Trusted: {component_id}"),
        trusted_component_type: "Discrete",
        status: Status::enabled_ok(),
    }))
}
