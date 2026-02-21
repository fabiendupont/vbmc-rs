use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use serde::Serialize;

use super::error::RedfishApiError;
use super::types::{Collection, ODataId, Status};
use crate::app_state::AppState;
use crate::backend::VmmBackend;

#[derive(Debug, Serialize)]
pub struct MemoryResource {
    #[serde(rename = "@odata.id")]
    pub odata_id: String,
    #[serde(rename = "@odata.type")]
    pub odata_type: &'static str,
    #[serde(rename = "Id")]
    pub id: String,
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "CapacityMiB")]
    pub capacity_mib: u64,
    #[serde(rename = "MemoryDeviceType")]
    pub memory_device_type: &'static str,
    #[serde(rename = "Status")]
    pub status: Status,
}

pub async fn get_memory_collection(
    State(state): State<Arc<AppState>>,
    Path(system_id): Path<String>,
) -> Result<Json<Collection<ODataId>>, RedfishApiError> {
    if !state.config.systems.contains_key(&system_id) {
        return Err(RedfishApiError::NotFound(format!(
            "System '{system_id}' not found"
        )));
    }

    let members = vec![ODataId::new(format!(
        "/redfish/v1/Systems/{system_id}/Memory/DIMM0"
    ))];

    Ok(Json(Collection::new(
        format!("/redfish/v1/Systems/{system_id}/Memory"),
        "#MemoryCollection.MemoryCollection",
        "Memory Collection",
        members,
    )))
}

pub async fn get_memory(
    State(state): State<Arc<AppState>>,
    Path((system_id, dimm_id)): Path<(String, String)>,
) -> Result<Json<MemoryResource>, RedfishApiError> {
    if !state.config.systems.contains_key(&system_id) {
        return Err(RedfishApiError::NotFound(format!(
            "System '{system_id}' not found"
        )));
    }
    if dimm_id != "DIMM0" {
        return Err(RedfishApiError::NotFound(format!(
            "Memory '{dimm_id}' not found"
        )));
    }

    let capacity_mib = match state.backend.vm_info(&system_id).await {
        Ok(info) => info.memory_bytes / (1024 * 1024),
        Err(_) => 0,
    };

    Ok(Json(MemoryResource {
        odata_id: format!("/redfish/v1/Systems/{system_id}/Memory/{dimm_id}"),
        odata_type: "#Memory.v1_19_0.Memory",
        id: dimm_id,
        name: "Virtual DIMM 0".to_string(),
        capacity_mib,
        memory_device_type: "DDR4",
        status: Status::enabled_ok(),
    }))
}
