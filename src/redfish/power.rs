use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use super::error::RedfishApiError;
use crate::app_state::AppState;
use crate::auth::AuthenticatedUser;
use crate::auth::rbac::{Privilege, has_privilege};
use crate::backend::VmmBackend;
use crate::backend::types::{DiskCreateConfig, VmCreateConfig};
use crate::events::RedfishEvent;
use crate::events::registry::*;

#[derive(Debug, Deserialize, Serialize)]
pub struct ResetRequest {
    #[serde(rename = "ResetType")]
    pub reset_type: String,
}

fn build_vm_config(state: &AppState, system_id: &str) -> VmCreateConfig {
    let sys_config = &state.config.systems[system_id];
    let vm_state = state.get_vm_state(system_id);

    let firmware = if vm_state.secure_boot_enabled {
        state.config.defaults.secure_boot_firmware_path.clone()
    } else {
        sys_config
            .firmware_path
            .clone()
            .unwrap_or_else(|| state.config.defaults.firmware_path.clone())
    };

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
        secure_boot: vm_state.secure_boot_enabled,
        disks,
        nics: Vec::new(),
        platform: None,
    }
}

fn emit_power_event(
    state: &AppState,
    system_id: &str,
    reset_type: &str,
    severity: &str,
    actor: Option<String>,
) {
    state.event_bus.emit(RedfishEvent {
        event_type: EVENT_TYPE_STATUS_CHANGE.to_string(),
        event_id: uuid::Uuid::new_v4().to_string(),
        event_timestamp: Utc::now(),
        message_id: MSG_SYSTEM_POWER_ON.to_string(),
        message: format!("System '{system_id}' reset action: {reset_type}"),
        origin_of_condition: Some(format!("/redfish/v1/Systems/{system_id}")),
        severity: severity.to_string(),
        actor,
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
    user: AuthenticatedUser,
    Path(system_id): Path<String>,
    Json(body): Json<ResetRequest>,
) -> Result<Json<serde_json::Value>, RedfishApiError> {
    if !has_privilege(&user.role, Privilege::ConfigureComponents) {
        return Err(RedfishApiError::Forbidden(
            "Insufficient privileges".to_string(),
        ));
    }

    if !state.config.systems.contains_key(&system_id) {
        return Err(RedfishApiError::NotFound(format!(
            "System '{system_id}' not found"
        )));
    }

    let _lock = state.system_lock(&system_id).await;

    // Manage-only backends (KubeVirt) drive the lifecycle of externally-owned
    // VMs via power subresources; they must never create or delete the VM object
    // (create is unsupported and delete would destroy a GitOps/kubectl-owned VM).
    // Ephemeral backends (cloud-hypervisor/qemu/libvirt) keep the create+boot /
    // shutdown+delete semantics that back their transient VMs.
    let managed = state.backend.manages_existing_vms();

    match body.reset_type.as_str() {
        "On" | "ForceOn" => {
            // Power on: manage-only backends just boot the existing VM; ephemeral
            // backends create the transient VM first, then boot it.
            if !managed {
                let config = build_vm_config(&state, &system_id);
                state
                    .backend
                    .vm_create(&system_id, config)
                    .await
                    .map_err(|e| RedfishApiError::InternalError(e.to_string()))?;
            }
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

            emit_power_event(
                &state,
                &system_id,
                "On",
                SEVERITY_OK,
                Some(user.username.clone()),
            );
        }
        "ForceOff" => {
            // Power off: manage-only backends only stop the VM; ephemeral
            // backends also delete the transient VM they created.
            let _ = state.backend.vm_shutdown(&system_id).await;
            if !managed {
                let _ = state.backend.vm_delete(&system_id).await;
            }
            emit_power_event(
                &state,
                &system_id,
                "ForceOff",
                SEVERITY_OK,
                Some(user.username.clone()),
            );
        }
        "GracefulShutdown" => {
            // Graceful power-off. Manage-only backends must actually stop the VM
            // (their vm_power_button is a soft *reboot*, which would leave the VM
            // running and contradict the emitted "Off" state); ephemeral backends
            // keep the ACPI power-button behaviour.
            if managed {
                state
                    .backend
                    .vm_shutdown(&system_id)
                    .await
                    .map_err(|e| RedfishApiError::InternalError(e.to_string()))?;
            } else {
                state
                    .backend
                    .vm_power_button(&system_id)
                    .await
                    .map_err(|e| RedfishApiError::InternalError(e.to_string()))?;
            }
            emit_power_event(
                &state,
                &system_id,
                "GracefulShutdown",
                SEVERITY_OK,
                Some(user.username.clone()),
            );
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

            emit_power_event(
                &state,
                &system_id,
                "GracefulRestart",
                SEVERITY_OK,
                Some(user.username.clone()),
            );
        }
        "ForceRestart" => {
            // Power cycle: manage-only backends issue a single restart
            // subresource; ephemeral backends tear the VM down and rebuild it.
            if managed {
                state
                    .backend
                    .vm_reboot(&system_id)
                    .await
                    .map_err(|e| RedfishApiError::InternalError(e.to_string()))?;
            } else {
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
            }

            let mut vm_state = state.get_vm_state(&system_id);
            if vm_state.boot_override.enabled == "Once" {
                vm_state.boot_override.enabled = "Disabled".to_string();
                vm_state.boot_override.target = None;
                state.save_vm_state(&system_id, &vm_state);
            }

            emit_power_event(
                &state,
                &system_id,
                "ForceRestart",
                SEVERITY_OK,
                Some(user.username.clone()),
            );
        }
        "PushPowerButton" => {
            state
                .backend
                .vm_power_button(&system_id)
                .await
                .map_err(|e| RedfishApiError::InternalError(e.to_string()))?;
            emit_power_event(
                &state,
                &system_id,
                "PushPowerButton",
                SEVERITY_OK,
                Some(user.username.clone()),
            );
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reset_request_deserialization() {
        let json = r#"{"ResetType": "On"}"#;
        let req: ResetRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.reset_type, "On");
    }

    #[test]
    fn test_reset_request_deserialization_all_types() {
        let reset_types = vec![
            "On",
            "ForceOn",
            "ForceOff",
            "GracefulShutdown",
            "GracefulRestart",
            "ForceRestart",
            "PushPowerButton",
        ];

        for reset_type in reset_types {
            let json = format!(r#"{{"ResetType": "{}"}}"#, reset_type);
            let req: ResetRequest = serde_json::from_str(&json).unwrap();
            assert_eq!(req.reset_type, reset_type);
        }
    }

    #[test]
    fn test_reset_request_serialization() {
        let req = ResetRequest {
            reset_type: "On".to_string(),
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["ResetType"], "On");
    }

    // Integration tests using the test harness
    #[tokio::test]
    async fn test_reset_system_power_on() {
        use crate::backend::mock::MockBackend;
        use crate::redfish::test_harness::*;
        use axum::http::{Method, StatusCode};

        let mock = MockBackend::new().with_vm("test-system", running_vm());
        let router = router(app_state(mock, systems_with("test-system")));

        let (status, body, _) = request_json(
            &router,
            Method::POST,
            "/redfish/v1/Systems/test-system/Actions/ComputerSystem.Reset",
            serde_json::json!({"ResetType": "On"}),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert!(body["message"].as_str().unwrap().contains("Reset action"));
        assert!(body["message"].as_str().unwrap().contains("On"));
    }

    #[tokio::test]
    async fn test_reset_system_graceful_restart() {
        use crate::backend::mock::MockBackend;
        use crate::redfish::test_harness::*;
        use axum::http::{Method, StatusCode};

        let mock = MockBackend::new().with_vm("test-system", running_vm());
        let router = router(app_state(mock, systems_with("test-system")));

        let (status, body, _) = request_json(
            &router,
            Method::POST,
            "/redfish/v1/Systems/test-system/Actions/ComputerSystem.Reset",
            serde_json::json!({"ResetType": "GracefulRestart"}),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert!(body["message"].as_str().unwrap().contains("Reset action"));
        assert!(
            body["message"]
                .as_str()
                .unwrap()
                .contains("GracefulRestart")
        );
    }

    #[tokio::test]
    async fn test_reset_system_force_off() {
        use crate::backend::mock::MockBackend;
        use crate::redfish::test_harness::*;
        use axum::http::{Method, StatusCode};

        let mock = MockBackend::new().with_vm("test-system", running_vm());
        let router = router(app_state(mock, systems_with("test-system")));

        let (status, body, _) = request_json(
            &router,
            Method::POST,
            "/redfish/v1/Systems/test-system/Actions/ComputerSystem.Reset",
            serde_json::json!({"ResetType": "ForceOff"}),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert!(body["message"].as_str().unwrap().contains("Reset action"));
        assert!(body["message"].as_str().unwrap().contains("ForceOff"));
    }

    #[tokio::test]
    async fn test_reset_system_invalid_type() {
        use crate::backend::mock::MockBackend;
        use crate::redfish::test_harness::*;
        use axum::http::Method;

        let mock = MockBackend::new().with_vm("test-system", running_vm());
        let router = router(app_state(mock, systems_with("test-system")));

        let (status, body, _) = request_json(
            &router,
            Method::POST,
            "/redfish/v1/Systems/test-system/Actions/ComputerSystem.Reset",
            serde_json::json!({"ResetType": "InvalidType"}),
        )
        .await;

        assert!(status.is_client_error());
        let error_msg = body["error"]["message"].as_str().unwrap().to_lowercase();
        assert!(error_msg.contains("unsupported") || error_msg.contains("invalid"));
    }

    #[tokio::test]
    async fn test_reset_system_malformed_body() {
        use crate::backend::mock::MockBackend;
        use crate::redfish::test_harness::*;
        use axum::http::Method;

        let mock = MockBackend::new().with_vm("test-system", running_vm());
        let router = router(app_state(mock, systems_with("test-system")));

        let (status, _, _) = request_json(
            &router,
            Method::POST,
            "/redfish/v1/Systems/test-system/Actions/ComputerSystem.Reset",
            serde_json::json!({"WrongField": "On"}),
        )
        .await;

        assert!(status.is_client_error());
    }

    #[tokio::test]
    async fn test_reset_system_not_found() {
        use crate::backend::mock::MockBackend;
        use crate::redfish::test_harness::*;
        use axum::http::Method;

        let mock = MockBackend::new();
        let router = router(app_state(mock, systems_with("test-system")));

        let (status, body, _) = request_json(
            &router,
            Method::POST,
            "/redfish/v1/Systems/unknown-system/Actions/ComputerSystem.Reset",
            serde_json::json!({"ResetType": "On"}),
        )
        .await;

        assert!(status.is_client_error());
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap()
                .to_lowercase()
                .contains("not found")
        );
    }
}
