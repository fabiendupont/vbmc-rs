use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use axum::http::Uri;
use serde::Serialize;

use super::error::RedfishApiError;
use crate::app_state::AppState;
use crate::auth::AuthenticatedUser;

#[derive(Debug, Serialize)]
pub struct AssemblyResource {
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
    #[serde(rename = "Assemblies")]
    pub assemblies: Vec<serde_json::Value>,
    #[serde(rename = "Assemblies@odata.count")]
    pub assemblies_count: usize,
}

pub async fn get_chassis_assembly(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
) -> Json<AssemblyResource> {
    Json(AssemblyResource {
        odata_id: format!("/redfish/v1/Chassis/{}/Assembly", state.chassis_id),
        odata_type: "#Assembly.v1_5_0.Assembly",
        id: "Assembly",
        name: "Chassis Assembly",
        description: "Virtual chassis assembly information",
        assemblies: Vec::new(),
        assemblies_count: 0,
    })
}

pub async fn get_chassis_sub_assembly(
    _user: AuthenticatedUser,
    uri: Uri,
) -> Result<Json<AssemblyResource>, RedfishApiError> {
    Ok(Json(AssemblyResource {
        odata_id: uri.path().to_string(),
        odata_type: "#Assembly.v1_5_0.Assembly",
        id: "Assembly",
        name: "Component Assembly",
        description: "Component assembly information",
        assemblies: Vec::new(),
        assemblies_count: 0,
    }))
}

pub async fn get_system_sub_assembly(
    _user: AuthenticatedUser,
    uri: Uri,
) -> Result<Json<AssemblyResource>, RedfishApiError> {
    Ok(Json(AssemblyResource {
        odata_id: uri.path().to_string(),
        odata_type: "#Assembly.v1_5_0.Assembly",
        id: "Assembly",
        name: "Component Assembly",
        description: "Component assembly information",
        assemblies: Vec::new(),
        assemblies_count: 0,
    }))
}
