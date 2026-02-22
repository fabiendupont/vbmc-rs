use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use chrono::Utc;
use serde::{Deserialize, Serialize};

use super::error::RedfishApiError;
use super::types::{Collection, ODataId, Status};
use crate::app_state::AppState;
use crate::backend::types::{DiskCreateConfig, VmPowerState};
use crate::backend::VmmBackend;
use crate::events::registry::*;
use crate::events::RedfishEvent;

#[derive(Debug, Serialize)]
pub struct VirtualMediaResource {
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
    #[serde(rename = "MediaTypes")]
    pub media_types: Vec<&'static str>,
    #[serde(rename = "Inserted")]
    pub inserted: bool,
    #[serde(rename = "Image", skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(rename = "ImageName", skip_serializing_if = "Option::is_none")]
    pub image_name: Option<String>,
    #[serde(rename = "WriteProtected")]
    pub write_protected: bool,
    #[serde(rename = "Status")]
    pub status: Status,
    #[serde(rename = "Actions")]
    pub actions: VirtualMediaActions,
}

#[derive(Debug, Serialize)]
pub struct VirtualMediaActions {
    #[serde(rename = "#VirtualMedia.InsertMedia")]
    pub insert_media: ActionTarget,
    #[serde(rename = "#VirtualMedia.EjectMedia")]
    pub eject_media: ActionTarget,
}

#[derive(Debug, Serialize)]
pub struct ActionTarget {
    pub target: String,
}

pub async fn get_virtual_media_collection(
    State(state): State<Arc<AppState>>,
    Path(system_id): Path<String>,
) -> Result<Json<Collection<ODataId>>, RedfishApiError> {
    if !state.config.systems.contains_key(&system_id) {
        return Err(RedfishApiError::NotFound(format!(
            "System '{system_id}' not found"
        )));
    }

    let members = vec![ODataId::new(format!(
        "/redfish/v1/Systems/{system_id}/VirtualMedia/Cd"
    ))];

    Ok(Json(Collection::new(
        format!("/redfish/v1/Systems/{system_id}/VirtualMedia"),
        "#VirtualMediaCollection.VirtualMediaCollection",
        "Virtual Media Collection",
        members,
    )))
}

pub async fn get_virtual_media(
    State(state): State<Arc<AppState>>,
    Path((system_id, media_id)): Path<(String, String)>,
) -> Result<Json<VirtualMediaResource>, RedfishApiError> {
    if !state.config.systems.contains_key(&system_id) {
        return Err(RedfishApiError::NotFound(format!(
            "System '{system_id}' not found"
        )));
    }
    if media_id != "Cd" {
        return Err(RedfishApiError::NotFound(format!(
            "VirtualMedia '{media_id}' not found"
        )));
    }

    let vm_state = state.get_vm_state(&system_id);

    let image_name = vm_state
        .virtual_media
        .image_url
        .as_ref()
        .and_then(|url| url.rsplit('/').next().map(|s| s.to_string()));

    Ok(Json(VirtualMediaResource {
        odata_id: format!("/redfish/v1/Systems/{system_id}/VirtualMedia/Cd"),
        odata_type: "#VirtualMedia.v1_6_0.VirtualMedia",
        id: "Cd",
        name: "Virtual CD",
        description: "Virtual media device",
        media_types: vec!["CD", "DVD"],
        inserted: vm_state.virtual_media.inserted,
        image: vm_state.virtual_media.image_url.clone(),
        image_name,
        write_protected: true,
        status: Status::enabled_ok(),
        actions: VirtualMediaActions {
            insert_media: ActionTarget {
                target: format!(
                    "/redfish/v1/Systems/{system_id}/VirtualMedia/Cd/Actions/VirtualMedia.InsertMedia"
                ),
            },
            eject_media: ActionTarget {
                target: format!(
                    "/redfish/v1/Systems/{system_id}/VirtualMedia/Cd/Actions/VirtualMedia.EjectMedia"
                ),
            },
        },
    }))
}

#[derive(Debug, Deserialize)]
pub struct InsertMediaRequest {
    #[serde(rename = "Image")]
    pub image: String,
    #[serde(rename = "Inserted", default = "default_true")]
    pub inserted: bool,
    #[serde(rename = "WriteProtected", default = "default_true")]
    pub write_protected: bool,
}

fn default_true() -> bool {
    true
}

pub async fn insert_media(
    State(state): State<Arc<AppState>>,
    Path((system_id, media_id)): Path<(String, String)>,
    Json(body): Json<InsertMediaRequest>,
) -> Result<Json<serde_json::Value>, RedfishApiError> {
    if !state.config.systems.contains_key(&system_id) {
        return Err(RedfishApiError::NotFound(format!(
            "System '{system_id}' not found"
        )));
    }
    if media_id != "Cd" {
        return Err(RedfishApiError::NotFound(format!(
            "VirtualMedia '{media_id}' not found"
        )));
    }

    let _lock = state.system_lock(&system_id).await;

    // Download the image
    let download_dir = state
        .config
        .systems
        .get(&system_id)
        .and_then(|s| s.virtual_media_directory.clone())
        .unwrap_or_else(|| state.config.state_directory.join("media"));

    let image_path = crate::media::download_image(&body.image, &download_dir)
        .await
        .map_err(|e| RedfishApiError::InternalError(format!("Failed to download image: {e}")))?;

    // Try hot-plug if VM is running
    let hot_plugged = match state.backend.vm_info(&system_id).await {
        Ok(info) if info.power_state == VmPowerState::On => {
            let disk = DiskCreateConfig {
                path: Some(image_path.to_string_lossy().to_string()),
                id: Some("_vbmc_cdrom".to_string()),
                readonly: true,
                vhost_user: None,
                vhost_socket: None,
            };
            state.backend.vm_add_disk(&system_id, disk).await.is_ok()
        }
        _ => false,
    };

    // Update state
    let mut vm_state = state.get_vm_state(&system_id);
    vm_state.virtual_media.inserted = true;
    vm_state.virtual_media.image_url = Some(body.image.clone());
    vm_state.virtual_media.image_path = Some(image_path);
    vm_state.virtual_media.write_protected = true;
    vm_state.virtual_media.media_type = Some("CD".to_string());
    vm_state.virtual_media.device_id = Some("_vbmc_cdrom".to_string());
    state.save_vm_state(&system_id, &vm_state);

    state.event_bus.emit(RedfishEvent {
        event_type: EVENT_TYPE_RESOURCE_UPDATED.to_string(),
        event_id: uuid::Uuid::new_v4().to_string(),
        event_timestamp: Utc::now(),
        message_id: MSG_VIRTUAL_MEDIA_INSERTED.to_string(),
        message: format!("Virtual media inserted on system '{system_id}'"),
        origin_of_condition: Some(format!(
            "/redfish/v1/Systems/{system_id}/VirtualMedia/Cd"
        )),
        severity: SEVERITY_OK.to_string(),
        actor: None,
        payload: None,
    });

    let _ = hot_plugged;
    Ok(Json(serde_json::json!({"message": "Media inserted"})))
}

pub async fn eject_media(
    State(state): State<Arc<AppState>>,
    Path((system_id, media_id)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, RedfishApiError> {
    if !state.config.systems.contains_key(&system_id) {
        return Err(RedfishApiError::NotFound(format!(
            "System '{system_id}' not found"
        )));
    }
    if media_id != "Cd" {
        return Err(RedfishApiError::NotFound(format!(
            "VirtualMedia '{media_id}' not found"
        )));
    }

    let _lock = state.system_lock(&system_id).await;

    // Try hot-unplug if VM is running
    if let Ok(info) = state.backend.vm_info(&system_id).await {
        if info.power_state == VmPowerState::On {
            let _ = state
                .backend
                .vm_remove_device(&system_id, "_vbmc_cdrom")
                .await;
        }
    }

    // Update state
    let mut vm_state = state.get_vm_state(&system_id);
    vm_state.virtual_media = crate::state::VirtualMediaState::default();
    state.save_vm_state(&system_id, &vm_state);

    state.event_bus.emit(RedfishEvent {
        event_type: EVENT_TYPE_RESOURCE_UPDATED.to_string(),
        event_id: uuid::Uuid::new_v4().to_string(),
        event_timestamp: Utc::now(),
        message_id: MSG_VIRTUAL_MEDIA_EJECTED.to_string(),
        message: format!("Virtual media ejected from system '{system_id}'"),
        origin_of_condition: Some(format!(
            "/redfish/v1/Systems/{system_id}/VirtualMedia/Cd"
        )),
        severity: SEVERITY_OK.to_string(),
        actor: None,
        payload: None,
    });

    Ok(Json(serde_json::json!({"message": "Media ejected"})))
}
