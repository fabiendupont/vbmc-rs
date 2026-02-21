use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use serde::Serialize;

use super::error::RedfishApiError;
use super::types::{Collection, ODataId, Status};
use crate::app_state::AppState;
use crate::backend::VmmBackend;

#[derive(Debug, Serialize)]
pub struct SimpleStorage {
    #[serde(rename = "@odata.id")]
    pub odata_id: String,
    #[serde(rename = "@odata.type")]
    pub odata_type: &'static str,
    #[serde(rename = "Id")]
    pub id: String,
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Devices")]
    pub devices: Vec<StorageDevice>,
    #[serde(rename = "Status")]
    pub status: Status,
}

#[derive(Debug, Serialize)]
pub struct StorageDevice {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "CapacityBytes", skip_serializing_if = "Option::is_none")]
    pub capacity_bytes: Option<u64>,
    #[serde(rename = "Status")]
    pub status: Status,
}

pub async fn get_simple_storage_collection(
    State(state): State<Arc<AppState>>,
    Path(system_id): Path<String>,
) -> Result<Json<Collection<ODataId>>, RedfishApiError> {
    if !state.config.systems.contains_key(&system_id) {
        return Err(RedfishApiError::NotFound(format!(
            "System '{system_id}' not found"
        )));
    }

    let members = vec![ODataId::new(format!(
        "/redfish/v1/Systems/{system_id}/SimpleStorage/1"
    ))];

    Ok(Json(Collection::new(
        format!("/redfish/v1/Systems/{system_id}/SimpleStorage"),
        "#SimpleStorageCollection.SimpleStorageCollection",
        "Simple Storage Collection",
        members,
    )))
}

pub async fn get_simple_storage(
    State(state): State<Arc<AppState>>,
    Path((system_id, storage_id)): Path<(String, String)>,
) -> Result<Json<SimpleStorage>, RedfishApiError> {
    if !state.config.systems.contains_key(&system_id) {
        return Err(RedfishApiError::NotFound(format!(
            "System '{system_id}' not found"
        )));
    }
    if storage_id != "1" {
        return Err(RedfishApiError::NotFound(format!(
            "SimpleStorage '{storage_id}' not found"
        )));
    }

    let mut devices = Vec::new();

    if let Ok(info) = state.backend.vm_info(&system_id).await {
        for disk in &info.disks {
            devices.push(StorageDevice {
                name: disk.id.clone(),
                capacity_bytes: disk.capacity_bytes,
                status: Status::enabled_ok(),
            });
        }
    }

    Ok(Json(SimpleStorage {
        odata_id: format!(
            "/redfish/v1/Systems/{system_id}/SimpleStorage/{storage_id}"
        ),
        odata_type: "#SimpleStorage.v1_3_0.SimpleStorage",
        id: storage_id,
        name: "Simple Storage Controller".to_string(),
        devices,
        status: Status::enabled_ok(),
    }))
}
