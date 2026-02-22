use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use serde::{Deserialize, Serialize};

use super::error::RedfishApiError;
use super::types::{Collection, ODataId, Status};
use crate::app_state::AppState;
use crate::backend::types::VmPowerState;
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
    #[serde(rename = "Description")]
    pub description: &'static str,
    #[serde(rename = "Manufacturer")]
    pub manufacturer: &'static str,
    #[serde(rename = "Model")]
    pub model: &'static str,
    #[serde(rename = "UUID", skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
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
    #[serde(rename = "Memory")]
    pub memory: ODataId,
    #[serde(rename = "Storage")]
    pub storage: ODataId,
    #[serde(rename = "PCIeDevices")]
    pub pcie_devices: Vec<ODataId>,
    #[serde(rename = "Bios")]
    pub bios: ODataId,
    #[serde(rename = "LogServices")]
    pub log_services: ODataId,
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
    #[serde(rename = "Model")]
    pub model: String,
    #[serde(rename = "CoreCount")]
    pub core_count: u32,
    #[serde(rename = "LogicalProcessorCount")]
    pub logical_processor_count: u32,
    #[serde(rename = "Status")]
    pub status: Status,
}

#[derive(Debug, Serialize)]
pub struct MemorySummary {
    #[serde(rename = "TotalSystemMemoryGiB")]
    pub total_system_memory_gib: f64,
    #[serde(rename = "TotalSystemPersistentMemoryGiB")]
    pub total_system_persistent_memory_gib: f64,
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

fn get_host_cpu_model() -> String {
    std::fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|content| {
            content
                .lines()
                .find(|l| l.starts_with("model name"))
                .map(|l| {
                    l.split(':')
                        .nth(1)
                        .unwrap_or("Virtual CPU")
                        .trim()
                        .to_string()
                })
        })
        .unwrap_or_else(|| "Virtual CPU".to_string())
}

fn power_state_to_redfish(ps: VmPowerState) -> String {
    match ps {
        VmPowerState::On => "On".to_string(),
        VmPowerState::Off => "Off".to_string(),
        VmPowerState::Paused => "Paused".to_string(),
        VmPowerState::Unknown => "Off".to_string(),
    }
}

fn power_state_to_status(ps: VmPowerState) -> Status {
    match ps {
        VmPowerState::On => Status::enabled_ok(),
        VmPowerState::Off => Status {
            state: Some("Disabled".to_string()),
            health: Some("OK".to_string()),
            health_rollup: Some("OK".to_string()),
        },
        VmPowerState::Paused => Status {
            state: Some("Quiesced".to_string()),
            health: Some("OK".to_string()),
            health_rollup: Some("OK".to_string()),
        },
        VmPowerState::Unknown => Status::unavailable_critical(),
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

    let (power_state, status, cpu_count, max_cpu_count, memory_gib, vm_uuid) =
        match state.backend.vm_info(&system_id).await {
            Ok(info) => {
                let ps = info.power_state;
                let st = power_state_to_status(ps);
                let cpus = info.cpu_count;
                let max_cpus = info.max_cpu_count;
                let mem = info.memory_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
                let uuid = info.uuid.clone();
                (power_state_to_redfish(ps), st, cpus, max_cpus, mem, uuid)
            }
            Err(_) => (
                "Off".to_string(),
                Status::unavailable_critical(),
                0,
                0,
                0.0,
                None,
            ),
        };

    let system_uuid = vm_uuid.unwrap_or_else(|| {
        uuid::Uuid::new_v5(
            &uuid::Uuid::NAMESPACE_URL,
            format!("vbmc-rs:system:{system_id}").as_bytes(),
        )
        .to_string()
    });

    let cpu_model = get_host_cpu_model();

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
        description: "Virtual machine managed by vbmc-rs",
        manufacturer: state.config.backend.display_name(),
        model: "vBMC",
        uuid: Some(system_uuid),
        system_type: "Virtual",
        status,
        power_state,
        boot,
        processor_summary: ProcessorSummary {
            count: cpu_count,
            model: cpu_model,
            core_count: cpu_count,
            logical_processor_count: max_cpu_count,
            status: Status::enabled_ok(),
        },
        memory_summary: MemorySummary {
            total_system_memory_gib: memory_gib,
            total_system_persistent_memory_gib: 0.0,
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
        memory: ODataId::new(format!(
            "/redfish/v1/Systems/{system_id}/Memory"
        )),
        storage: ODataId::new(format!(
            "/redfish/v1/Systems/{system_id}/Storage"
        )),
        pcie_devices: Vec::new(),
        bios: ODataId::new(format!(
            "/redfish/v1/Systems/{system_id}/Bios"
        )),
        log_services: ODataId::new(format!(
            "/redfish/v1/Systems/{system_id}/LogServices"
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_power_state_to_redfish() {
        assert_eq!(power_state_to_redfish(VmPowerState::On), "On");
        assert_eq!(power_state_to_redfish(VmPowerState::Off), "Off");
        assert_eq!(power_state_to_redfish(VmPowerState::Paused), "Paused");
        assert_eq!(power_state_to_redfish(VmPowerState::Unknown), "Off");
    }

    #[test]
    fn test_power_state_to_status_on() {
        let s = power_state_to_status(VmPowerState::On);
        assert_eq!(s.state.as_deref(), Some("Enabled"));
        assert_eq!(s.health.as_deref(), Some("OK"));
    }

    #[test]
    fn test_power_state_to_status_off() {
        let s = power_state_to_status(VmPowerState::Off);
        assert_eq!(s.state.as_deref(), Some("Disabled"));
        assert_eq!(s.health.as_deref(), Some("OK"));
    }

    #[test]
    fn test_power_state_to_status_paused() {
        let s = power_state_to_status(VmPowerState::Paused);
        assert_eq!(s.state.as_deref(), Some("Quiesced"));
    }

    #[test]
    fn test_power_state_to_status_unknown() {
        let s = power_state_to_status(VmPowerState::Unknown);
        assert_eq!(s.state.as_deref(), Some("UnavailableOffline"));
        assert_eq!(s.health.as_deref(), Some("Critical"));
    }
}
