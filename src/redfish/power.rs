use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use chrono::Utc;
use serde::Deserialize;

use super::error::RedfishApiError;
use crate::app_state::AppState;
use crate::backend::VmmBackend;
use crate::backend::types::{DiskCreateConfig, VmCreateConfig};
use crate::events::RedfishEvent;
use crate::events::registry::*;

#[derive(Debug, Deserialize)]
pub struct ResetRequest {
    #[serde(rename = "ResetType")]
    pub reset_type: String,
}

fn build_vm_config(state: &AppState, system_id: &str) -> VmCreateConfig {
    let sys_config = &state.config.systems[system_id];
    let vm_state = state.get_vm_state(system_id);

    let firmware = sys_config
        .firmware_path
        .clone()
        .unwrap_or_else(|| state.config.defaults.firmware_path.clone());

    let mut disks: Vec<DiskCreateConfig> = Vec::new();

    // If boot target is Cd and virtual media is inserted, put CD first
    let boot_from_cd =
        vm_state.boot_override.target.as_deref() == Some("Cd") && vm_state.virtual_media.inserted;

    if boot_from_cd && let Some(ref path) = vm_state.virtual_media.image_path {
        disks.push(DiskCreateConfig {
            path: Some(path.to_string_lossy().to_string()),
            id: Some("_vbmc_cdrom".to_string()),
            readonly: true,
            vhost_user: None,
            vhost_socket: None,
        });
    }

    // Add disks from hardware config
    for disk in &sys_config.hardware.disks {
        disks.push(DiskCreateConfig {
            path: Some(disk.path.clone()),
            id: disk.id.clone(),
            readonly: disk.readonly,
            vhost_user: None,
            vhost_socket: None,
        });
    }

    let cpu_count = sys_config.hardware.cpu_count;
    let max_cpu_count = sys_config.hardware.max_cpu_count.unwrap_or(cpu_count);
    let memory_bytes = sys_config.hardware.memory_mib * 1024 * 1024;

    VmCreateConfig {
        firmware_path: Some(firmware),
        kernel_path: None,
        cmdline: None,
        initramfs: None,
        cpu_count,
        max_cpu_count,
        memory_bytes,
        disks,
        nics: Vec::new(),
        platform: None,
    }
}

fn emit_power_event(state: &AppState, system_id: &str, reset_type: &str, severity: &str) {
    state.event_bus.emit(RedfishEvent {
        event_type: EVENT_TYPE_STATUS_CHANGE.to_string(),
        event_id: uuid::Uuid::new_v4().to_string(),
        event_timestamp: Utc::now(),
        message_id: MSG_SYSTEM_POWER_ON.to_string(),
        message: format!("System '{system_id}' reset action: {reset_type}"),
        origin_of_condition: Some(format!("/redfish/v1/Systems/{system_id}")),
        severity: severity.to_string(),
        actor: None,
        payload: None,
    });

    let power_state = match reset_type {
        "On" | "ForceOn" | "GracefulRestart" | "ForceRestart" => "On",
        "ForceOff" | "GracefulShutdown" => "Off",
        _ => "Unknown",
    };
    crate::telemetry::record_vm_power_state(system_id, power_state);
}

pub async fn reset_system(
    State(state): State<Arc<AppState>>,
    Path(system_id): Path<String>,
    Json(body): Json<ResetRequest>,
) -> Result<Json<serde_json::Value>, RedfishApiError> {
    if !state.config.systems.contains_key(&system_id) {
        return Err(RedfishApiError::NotFound(format!(
            "System '{system_id}' not found"
        )));
    }

    let _lock = state.system_lock(&system_id).await;

    match body.reset_type.as_str() {
        "On" | "ForceOn" => {
            // Create + boot the VM
            let config = build_vm_config(&state, &system_id);
            state
                .backend
                .vm_create(&system_id, config)
                .await
                .map_err(|e| RedfishApiError::InternalError(e.to_string()))?;
            state
                .backend
                .vm_boot(&system_id)
                .await
                .map_err(|e| RedfishApiError::InternalError(e.to_string()))?;

            // Clear Once boot override
            let mut vm_state = state.get_vm_state(&system_id);
            if vm_state.boot_override.enabled == "Once" {
                vm_state.boot_override.enabled = "Disabled".to_string();
                vm_state.boot_override.target = None;
                state.save_vm_state(&system_id, &vm_state);
            }

            emit_power_event(&state, &system_id, "On", SEVERITY_OK);
        }
        "ForceOff" => {
            // Force shutdown + delete
            let _ = state.backend.vm_shutdown(&system_id).await;
            let _ = state.backend.vm_delete(&system_id).await;
            emit_power_event(&state, &system_id, "ForceOff", SEVERITY_OK);
        }
        "GracefulShutdown" => {
            state
                .backend
                .vm_power_button(&system_id)
                .await
                .map_err(|e| RedfishApiError::InternalError(e.to_string()))?;
            emit_power_event(&state, &system_id, "GracefulShutdown", SEVERITY_OK);
        }
        "GracefulRestart" => {
            state
                .backend
                .vm_reboot(&system_id)
                .await
                .map_err(|e| RedfishApiError::InternalError(e.to_string()))?;

            let mut vm_state = state.get_vm_state(&system_id);
            if vm_state.boot_override.enabled == "Once" {
                vm_state.boot_override.enabled = "Disabled".to_string();
                vm_state.boot_override.target = None;
                state.save_vm_state(&system_id, &vm_state);
            }

            emit_power_event(&state, &system_id, "GracefulRestart", SEVERITY_OK);
        }
        "ForceRestart" => {
            let _ = state.backend.vm_shutdown(&system_id).await;
            let _ = state.backend.vm_delete(&system_id).await;

            let config = build_vm_config(&state, &system_id);
            state
                .backend
                .vm_create(&system_id, config)
                .await
                .map_err(|e| RedfishApiError::InternalError(e.to_string()))?;
            state
                .backend
                .vm_boot(&system_id)
                .await
                .map_err(|e| RedfishApiError::InternalError(e.to_string()))?;

            let mut vm_state = state.get_vm_state(&system_id);
            if vm_state.boot_override.enabled == "Once" {
                vm_state.boot_override.enabled = "Disabled".to_string();
                vm_state.boot_override.target = None;
                state.save_vm_state(&system_id, &vm_state);
            }

            emit_power_event(&state, &system_id, "ForceRestart", SEVERITY_OK);
        }
        "PushPowerButton" => {
            state
                .backend
                .vm_power_button(&system_id)
                .await
                .map_err(|e| RedfishApiError::InternalError(e.to_string()))?;
            emit_power_event(&state, &system_id, "PushPowerButton", SEVERITY_OK);
        }
        other => {
            return Err(RedfishApiError::BadRequest(format!(
                "Unsupported ResetType: {other}"
            )));
        }
    }

    Ok(Json(
        serde_json::json!({"message": format!("Reset action '{}' completed", body.reset_type)}),
    ))
}
