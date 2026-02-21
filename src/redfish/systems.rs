use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use serde::{Deserialize, Serialize};

use super::error::RedfishApiError;
use super::types::{Collection, ODataId, Status};
use crate::app_state::AppState;
use crate::backend::VmmBackend;

#[derive(Debug, Serialize)]
pub struct ComputerSystem {
    #[serde(rename = "@odata.id")]
    pub odata_id: String,
    #[serde(rename = "@odata.type")]
    pub odata_type: &'static str,
    #[serde(rename = "Id")]
    pub id: String,
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "SystemType")]
    pub system_type: &'static str,
    #[serde(rename = "Status")]
    pub status: Status,
    #[serde(rename = "PowerState")]
    pub power_state: String,
    #[serde(rename = "Boot")]
    pub boot: BootOptions,
    #[serde(rename = "ProcessorSummary")]
    pub processor_summary: ProcessorSummary,
    #[serde(rename = "MemorySummary")]
    pub memory_summary: MemorySummary,
    #[serde(rename = "Actions")]
    pub actions: SystemActions,
    #[serde(rename = "EthernetInterfaces")]
    pub ethernet_interfaces: ODataId,
    #[serde(rename = "Processors")]
    pub processors: ODataId,
    #[serde(rename = "SimpleStorage")]
    pub simple_storage: ODataId,
    #[serde(rename = "VirtualMedia")]
    pub virtual_media: ODataId,
    #[serde(rename = "SecureBoot")]
    pub secure_boot: ODataId,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BootOptions {
    #[serde(rename = "BootSourceOverrideTarget", skip_serializing_if = "Option::is_none")]
    pub boot_source_override_target: Option<String>,
    #[serde(rename = "BootSourceOverrideEnabled", skip_serializing_if = "Option::is_none")]
    pub boot_source_override_enabled: Option<String>,
    #[serde(rename = "BootSourceOverrideMode", skip_serializing_if = "Option::is_none")]
    pub boot_source_override_mode: Option<String>,
    #[serde(rename = "BootSourceOverrideTarget@Redfish.AllowableValues", skip_serializing_if = "Option::is_none")]
    pub allowable_targets: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
pub struct ProcessorSummary {
    #[serde(rename = "Count")]
    pub count: u32,
    #[serde(rename = "Status")]
    pub status: Status,
}

#[derive(Debug, Serialize)]
pub struct MemorySummary {
    #[serde(rename = "TotalSystemMemoryGiB")]
    pub total_system_memory_gib: f64,
    #[serde(rename = "Status")]
    pub status: Status,
}

#[derive(Debug, Serialize)]
pub struct SystemActions {
    #[serde(rename = "#ComputerSystem.Reset")]
    pub reset: ResetAction,
}

#[derive(Debug, Serialize)]
pub struct ResetAction {
    pub target: String,
    #[serde(rename = "ResetType@Redfish.AllowableValues")]
    pub allowable_values: Vec<String>,
}

fn ch_state_to_power_state(state: &str) -> String {
    match state {
        "Running" => "On".to_string(),
        "Shutdown" | "Created" => "Off".to_string(),
        "Paused" => "Paused".to_string(),
        _ => "Off".to_string(),
    }
}

fn ch_state_to_status(state: &str) -> Status {
    match state {
        "Running" => Status::enabled_ok(),
        "Shutdown" | "Created" => Status {
            state: Some("Disabled".to_string()),
            health: Some("OK".to_string()),
            health_rollup: Some("OK".to_string()),
        },
        "Paused" => Status {
            state: Some("Quiesced".to_string()),
            health: Some("OK".to_string()),
            health_rollup: Some("OK".to_string()),
        },
        _ => Status::unavailable_critical(),
    }
}

pub async fn get_systems(
    State(state): State<Arc<AppState>>,
) -> Json<Collection<ODataId>> {
    let members: Vec<ODataId> = state
        .config
        .systems
        .keys()
        .map(|id| ODataId::new(format!("/redfish/v1/Systems/{id}")))
        .collect();

    Json(Collection::new(
        "/redfish/v1/Systems",
        "#ComputerSystemCollection.ComputerSystemCollection",
        "Computer System Collection",
        members,
    ))
}

pub async fn get_system(
    State(state): State<Arc<AppState>>,
    Path(system_id): Path<String>,
) -> Result<Json<ComputerSystem>, RedfishApiError> {
    let sys_config = state
        .config
        .systems
        .get(&system_id)
        .ok_or_else(|| RedfishApiError::NotFound(format!("System '{system_id}' not found")))?;

    let name = sys_config
        .name
        .clone()
        .unwrap_or_else(|| system_id.clone());

    let vm_state = state.get_vm_state(&system_id);

    let (power_state, status, cpu_count, memory_gib) =
        match state.backend.vm_info(&system_id).await {
            Ok(info) => {
                let ps = ch_state_to_power_state(&info.state);
                let st = ch_state_to_status(&info.state);
                let cpus = info
                    .config
                    .cpus
                    .as_ref()
                    .map(|c| c.boot_vcpus as u32)
                    .unwrap_or(0);
                let mem = info
                    .config
                    .memory
                    .as_ref()
                    .map(|m| m.size as f64 / (1024.0 * 1024.0 * 1024.0))
                    .unwrap_or(0.0);
                (ps, st, cpus, mem)
            }
            Err(_) => (
                "Off".to_string(),
                Status::unavailable_critical(),
                0,
                0.0,
            ),
        };

    let boot = BootOptions {
        boot_source_override_target: vm_state.boot_override.target.clone(),
        boot_source_override_enabled: Some(vm_state.boot_override.enabled.clone()),
        boot_source_override_mode: vm_state.boot_override.mode.clone(),
        allowable_targets: Some(vec![
            "None".to_string(),
            "Pxe".to_string(),
            "Cd".to_string(),
            "Hdd".to_string(),
        ]),
    };

    Ok(Json(ComputerSystem {
        odata_id: format!("/redfish/v1/Systems/{system_id}"),
        odata_type: "#ComputerSystem.v1_20_0.ComputerSystem",
        id: system_id.clone(),
        name,
        system_type: "Virtual",
        status,
        power_state,
        boot,
        processor_summary: ProcessorSummary {
            count: cpu_count,
            status: Status::enabled_ok(),
        },
        memory_summary: MemorySummary {
            total_system_memory_gib: memory_gib,
            status: Status::enabled_ok(),
        },
        actions: SystemActions {
            reset: ResetAction {
                target: format!(
                    "/redfish/v1/Systems/{system_id}/Actions/ComputerSystem.Reset"
                ),
                allowable_values: vec![
                    "On".to_string(),
                    "ForceOff".to_string(),
                    "GracefulShutdown".to_string(),
                    "GracefulRestart".to_string(),
                    "ForceRestart".to_string(),
                    "ForceOn".to_string(),
                    "PushPowerButton".to_string(),
                ],
            },
        },
        ethernet_interfaces: ODataId::new(format!(
            "/redfish/v1/Systems/{system_id}/EthernetInterfaces"
        )),
        processors: ODataId::new(format!("/redfish/v1/Systems/{system_id}/Processors")),
        simple_storage: ODataId::new(format!(
            "/redfish/v1/Systems/{system_id}/SimpleStorage"
        )),
        virtual_media: ODataId::new(format!(
            "/redfish/v1/Systems/{system_id}/VirtualMedia"
        )),
        secure_boot: ODataId::new(format!(
            "/redfish/v1/Systems/{system_id}/SecureBoot"
        )),
    }))
}

#[derive(Debug, Deserialize)]
pub struct PatchSystemRequest {
    #[serde(rename = "Boot")]
    pub boot: Option<BootOptions>,
}

pub async fn patch_system(
    State(state): State<Arc<AppState>>,
    Path(system_id): Path<String>,
    Json(body): Json<PatchSystemRequest>,
) -> Result<Json<serde_json::Value>, RedfishApiError> {
    if !state.config.systems.contains_key(&system_id) {
        return Err(RedfishApiError::NotFound(format!(
            "System '{system_id}' not found"
        )));
    }

    if let Some(boot) = body.boot {
        let mut vm_state = state.get_vm_state(&system_id);
        if let Some(target) = boot.boot_source_override_target {
            vm_state.boot_override.target = Some(target);
        }
        if let Some(enabled) = boot.boot_source_override_enabled {
            vm_state.boot_override.enabled = enabled;
        }
        if let Some(mode) = boot.boot_source_override_mode {
            vm_state.boot_override.mode = Some(mode);
        }
        state.save_vm_state(&system_id, &vm_state);
    }

    Ok(Json(serde_json::json!({"message": "System updated"})))
}
