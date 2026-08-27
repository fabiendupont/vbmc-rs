use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use serde::{Deserialize, Serialize};

use super::error::RedfishApiError;
use super::types::{Collection, ODataId, Status, StatusRollup};
use crate::app_state::AppState;
use crate::auth::AuthenticatedUser;
use crate::auth::rbac::{Privilege, has_privilege};
use crate::backend::VmmBackend;
use crate::backend::types::VmPowerState;

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
    #[serde(rename = "NetworkInterfaces")]
    pub network_interfaces: ODataId,
    #[serde(rename = "PCIeDevices")]
    pub pcie_devices: Vec<ODataId>,
    #[serde(rename = "Bios")]
    pub bios: ODataId,
    #[serde(rename = "LogServices")]
    pub log_services: ODataId,
    #[serde(rename = "BiosVersion")]
    pub bios_version: &'static str,
    #[serde(rename = "SerialNumber")]
    pub serial_number: String,
    #[serde(rename = "HostName")]
    pub host_name: String,
    #[serde(rename = "PowerRestorePolicy")]
    pub power_restore_policy: &'static str,
    #[serde(rename = "AssetTag")]
    pub asset_tag: &'static str,
    #[serde(rename = "PartNumber")]
    pub part_number: &'static str,
    #[serde(rename = "SKU")]
    pub sku: &'static str,
    #[serde(rename = "SubModel")]
    pub sub_model: &'static str,
    #[serde(rename = "LocationIndicatorActive")]
    pub location_indicator_active: bool,
    #[serde(rename = "PowerOnDelaySeconds")]
    pub power_on_delay_seconds: f64,
    #[serde(rename = "PowerOffDelaySeconds")]
    pub power_off_delay_seconds: f64,
    #[serde(rename = "PowerCycleDelaySeconds")]
    pub power_cycle_delay_seconds: f64,
    #[serde(rename = "ManufacturingMode")]
    pub manufacturing_mode: bool,
    #[serde(rename = "IdlePowerSaver")]
    pub idle_power_saver: IdlePowerSaver,
    #[serde(rename = "PowerMode")]
    pub power_mode: &'static str,
    #[serde(rename = "BootProgress")]
    pub boot_progress: BootProgress,
    #[serde(rename = "LastResetTime")]
    pub last_reset_time: String,
    #[serde(rename = "HostWatchdogTimer")]
    pub host_watchdog_timer: HostWatchdogTimer,
    #[serde(rename = "GraphicalConsole")]
    pub graphical_console: HostGraphicalConsole,
    #[serde(rename = "SerialConsole")]
    pub serial_console: HostSerialConsole,
    #[serde(rename = "VirtualMediaConfig")]
    pub virtual_media_config: VirtualMediaConfig,
    #[serde(rename = "HostingRoles")]
    pub hosting_roles: Vec<&'static str>,
    #[serde(rename = "Links")]
    pub links: ComputerSystemLinks,
}

#[derive(Debug, Serialize)]
pub struct IdlePowerSaver {
    #[serde(rename = "Enabled")]
    pub enabled: bool,
    #[serde(rename = "EnterDwellTimeSeconds")]
    pub enter_dwell_time_seconds: u32,
    #[serde(rename = "EnterUtilizationPercent")]
    pub enter_utilization_percent: u32,
    #[serde(rename = "ExitDwellTimeSeconds")]
    pub exit_dwell_time_seconds: u32,
    #[serde(rename = "ExitUtilizationPercent")]
    pub exit_utilization_percent: u32,
}

#[derive(Debug, Serialize)]
pub struct BootProgress {
    #[serde(rename = "LastState")]
    pub last_state: &'static str,
    #[serde(rename = "LastStateTime")]
    pub last_state_time: String,
    #[serde(rename = "LastBootTimeSeconds")]
    pub last_boot_time_seconds: u32,
}

#[derive(Debug, Serialize)]
pub struct HostWatchdogTimer {
    #[serde(rename = "FunctionEnabled")]
    pub function_enabled: bool,
    #[serde(rename = "TimeoutAction")]
    pub timeout_action: &'static str,
    #[serde(rename = "WarningAction")]
    pub warning_action: &'static str,
    #[serde(rename = "Status")]
    pub status: Status,
}

#[derive(Debug, Serialize)]
pub struct HostGraphicalConsole {
    #[serde(rename = "ServiceEnabled")]
    pub service_enabled: bool,
    #[serde(rename = "MaxConcurrentSessions")]
    pub max_concurrent_sessions: u32,
    #[serde(rename = "ConnectTypesSupported")]
    pub connect_types_supported: Vec<&'static str>,
    #[serde(rename = "Port")]
    pub port: u32,
}

#[derive(Debug, Serialize)]
pub struct HostSerialConsole {
    #[serde(rename = "MaxConcurrentSessions")]
    pub max_concurrent_sessions: u32,
    #[serde(rename = "IPMI")]
    pub ipmi: ConsoleProtocol,
    #[serde(rename = "SSH")]
    pub ssh: ConsoleProtocol,
    #[serde(rename = "Telnet")]
    pub telnet: ConsoleProtocol,
}

#[derive(Debug, Serialize)]
pub struct ConsoleProtocol {
    #[serde(rename = "ServiceEnabled")]
    pub service_enabled: bool,
    #[serde(rename = "Port")]
    pub port: u32,
    #[serde(rename = "SharedWithManagerCLI")]
    pub shared_with_manager_cli: bool,
}

#[derive(Debug, Serialize)]
pub struct VirtualMediaConfig {
    #[serde(rename = "ServiceEnabled")]
    pub service_enabled: bool,
    #[serde(rename = "Port")]
    pub port: u32,
}

#[derive(Debug, Serialize)]
pub struct ComputerSystemLinks {
    #[serde(rename = "Chassis")]
    pub chassis: Vec<ODataId>,
    #[serde(rename = "ManagedBy")]
    pub managed_by: Vec<ODataId>,
    #[serde(rename = "TrustedComponents")]
    pub trusted_components: Vec<ODataId>,
    #[serde(rename = "CooledBy")]
    pub cooled_by: Vec<ODataId>,
    #[serde(rename = "PoweredBy")]
    pub powered_by: Vec<ODataId>,
    #[serde(rename = "ConsumingComputerSystems")]
    pub consuming_computer_systems: Vec<ODataId>,
    #[serde(rename = "SupplyingComputerSystems")]
    pub supplying_computer_systems: Vec<ODataId>,
    #[serde(rename = "OffloadedNetworkDeviceFunctions")]
    pub offloaded_network_device_functions: Vec<ODataId>,
    #[serde(rename = "Endpoints")]
    pub endpoints: Vec<ODataId>,
    #[serde(rename = "ResourceBlocks")]
    pub resource_blocks: Vec<ODataId>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BootOptions {
    #[serde(rename = "BootSourceOverrideTarget")]
    pub boot_source_override_target: String,
    #[serde(rename = "BootSourceOverrideEnabled")]
    pub boot_source_override_enabled: String,
    #[serde(rename = "BootSourceOverrideMode")]
    pub boot_source_override_mode: String,
    #[serde(rename = "BootSourceOverrideTarget@Redfish.AllowableValues")]
    pub allowable_targets: Vec<String>,
    #[serde(rename = "BootOrder")]
    pub boot_order: Vec<String>,
    #[serde(rename = "StopBootOnFault")]
    pub stop_boot_on_fault: &'static str,
    #[serde(rename = "AutomaticRetryConfig")]
    pub automatic_retry_config: &'static str,
    #[serde(rename = "AutomaticRetryAttempts")]
    pub automatic_retry_attempts: u32,
    #[serde(rename = "RemainingAutomaticRetryAttempts")]
    pub remaining_automatic_retry_attempts: u32,
    #[serde(rename = "HttpBootUri")]
    pub http_boot_uri: &'static str,
    #[serde(rename = "UefiTargetBootSourceOverride")]
    pub uefi_target: &'static str,
    #[serde(rename = "BootNext")]
    pub boot_next: &'static str,
    #[serde(rename = "TrustedModuleRequiredToBoot")]
    pub trusted_module_required_to_boot: &'static str,
    #[serde(rename = "BootOrderPropertySelection")]
    pub boot_order_property_selection: &'static str,
    #[serde(rename = "AliasBootOrder")]
    pub alias_boot_order: Vec<&'static str>,
    #[serde(rename = "BootOptions")]
    pub boot_options_link: ODataId,
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
    #[serde(rename = "ThreadingEnabled")]
    pub threading_enabled: bool,
}

#[derive(Debug, Serialize)]
pub struct MemorySummary {
    #[serde(rename = "TotalSystemMemoryGiB")]
    pub total_system_memory_gib: f64,
    #[serde(rename = "TotalSystemPersistentMemoryGiB")]
    pub total_system_persistent_memory_gib: f64,
    #[serde(rename = "MemoryMirroring")]
    pub memory_mirroring: &'static str,
}

#[derive(Debug, Serialize)]
pub struct SystemActions {
    #[serde(rename = "#ComputerSystem.Reset")]
    pub reset: ResetAction,
    #[serde(rename = "#ComputerSystem.SetDefaultBootOrder")]
    pub set_default_boot_order: ActionTarget,
}

#[derive(Debug, Serialize)]
pub struct ResetAction {
    pub target: String,
    #[serde(rename = "ResetType@Redfish.AllowableValues")]
    pub allowable_values: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ActionTarget {
    pub target: String,
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
    let base = match ps {
        VmPowerState::On => Status {
            state: Some("Enabled".to_string()),
            health: Some("OK".to_string()),
            health_rollup: None,
        },
        VmPowerState::Off => Status {
            state: Some("Disabled".to_string()),
            health: Some("OK".to_string()),
            health_rollup: None,
        },
        VmPowerState::Paused => Status {
            state: Some("Quiesced".to_string()),
            health: Some("OK".to_string()),
            health_rollup: None,
        },
        VmPowerState::Unknown => Status {
            state: Some("UnavailableOffline".to_string()),
            health: Some("Critical".to_string()),
            health_rollup: None,
        },
    };
    let rollup = StatusRollup::from_statuses(&[&base]);
    Status {
        health_rollup: Some(rollup.health),
        ..base
    }
}

pub async fn get_systems(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
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
    _user: AuthenticatedUser,
    Path(system_id): Path<String>,
) -> Result<Json<ComputerSystem>, RedfishApiError> {
    let sys_config = state
        .config
        .systems
        .get(&system_id)
        .ok_or_else(|| RedfishApiError::NotFound(format!("System '{system_id}' not found")))?;

    let name = sys_config.name.clone().unwrap_or_else(|| system_id.clone());

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

    let serial_number = format!("VBMC-{}", system_uuid.split('-').next().unwrap_or("0000"));
    let host_name = name.clone();
    let boot_progress_state = if power_state == "On" {
        "OSRunning"
    } else {
        "None"
    };
    let cpu_model = get_host_cpu_model();

    let boot = BootOptions {
        boot_source_override_target: vm_state
            .boot_override
            .target
            .clone()
            .unwrap_or_else(|| "None".to_string()),
        boot_source_override_enabled: vm_state.boot_override.enabled.clone(),
        boot_source_override_mode: vm_state
            .boot_override
            .mode
            .clone()
            .unwrap_or_else(|| "UEFI".to_string()),
        allowable_targets: vec![
            "None".to_string(),
            "Pxe".to_string(),
            "Cd".to_string(),
            "Hdd".to_string(),
        ],
        boot_order: vec!["Hdd".to_string(), "Pxe".to_string(), "Cd".to_string()],
        stop_boot_on_fault: "Never",
        automatic_retry_config: "Disabled",
        automatic_retry_attempts: 0,
        remaining_automatic_retry_attempts: 0,
        http_boot_uri: "",
        uefi_target: "",
        boot_next: "",
        trusted_module_required_to_boot: "Disabled",
        boot_order_property_selection: "BootOrder",
        alias_boot_order: Vec::new(),
        boot_options_link: ODataId::new(format!("/redfish/v1/Systems/{system_id}/BootOptions")),
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
            threading_enabled: max_cpu_count > cpu_count,
        },
        memory_summary: MemorySummary {
            total_system_memory_gib: memory_gib,
            total_system_persistent_memory_gib: 0.0,
            memory_mirroring: "None",
        },
        actions: SystemActions {
            reset: ResetAction {
                target: format!("/redfish/v1/Systems/{system_id}/Actions/ComputerSystem.Reset"),
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
            set_default_boot_order: ActionTarget {
                target: format!(
                    "/redfish/v1/Systems/{system_id}/Actions/ComputerSystem.SetDefaultBootOrder"
                ),
            },
        },
        ethernet_interfaces: ODataId::new(format!(
            "/redfish/v1/Systems/{system_id}/EthernetInterfaces"
        )),
        processors: ODataId::new(format!("/redfish/v1/Systems/{system_id}/Processors")),
        simple_storage: ODataId::new(format!("/redfish/v1/Systems/{system_id}/SimpleStorage")),
        virtual_media: ODataId::new(format!("/redfish/v1/Systems/{system_id}/VirtualMedia")),
        secure_boot: ODataId::new(format!("/redfish/v1/Systems/{system_id}/SecureBoot")),
        memory: ODataId::new(format!("/redfish/v1/Systems/{system_id}/Memory")),
        storage: ODataId::new(format!("/redfish/v1/Systems/{system_id}/Storage")),
        network_interfaces: ODataId::new(format!(
            "/redfish/v1/Systems/{system_id}/NetworkInterfaces"
        )),
        pcie_devices: Vec::new(),
        bios: ODataId::new(format!("/redfish/v1/Systems/{system_id}/Bios")),
        log_services: ODataId::new(format!("/redfish/v1/Systems/{system_id}/LogServices")),
        bios_version: "vbmc-rs",
        serial_number,
        host_name,
        power_restore_policy: "AlwaysOff",
        asset_tag: "",
        part_number: "VBMC-SYS",
        sku: "VBMC-VIRTUAL",
        sub_model: "Standard",
        location_indicator_active: false,
        power_on_delay_seconds: 0.0,
        power_off_delay_seconds: 0.0,
        power_cycle_delay_seconds: 0.0,
        manufacturing_mode: false,
        idle_power_saver: IdlePowerSaver {
            enabled: false,
            enter_dwell_time_seconds: 600,
            enter_utilization_percent: 8,
            exit_dwell_time_seconds: 10,
            exit_utilization_percent: 20,
        },
        power_mode: "MaximumPerformance",
        boot_progress: BootProgress {
            last_state: boot_progress_state,
            last_state_time: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
            last_boot_time_seconds: 0,
        },
        last_reset_time: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        host_watchdog_timer: HostWatchdogTimer {
            function_enabled: false,
            timeout_action: "None",
            warning_action: "None",
            status: Status::enabled_ok(),
        },
        graphical_console: HostGraphicalConsole {
            service_enabled: false,
            max_concurrent_sessions: 0,
            connect_types_supported: Vec::new(),
            port: 0,
        },
        serial_console: HostSerialConsole {
            max_concurrent_sessions: 0,
            ipmi: ConsoleProtocol {
                service_enabled: false,
                port: 0,
                shared_with_manager_cli: false,
            },
            ssh: ConsoleProtocol {
                service_enabled: false,
                port: 0,
                shared_with_manager_cli: false,
            },
            telnet: ConsoleProtocol {
                service_enabled: false,
                port: 0,
                shared_with_manager_cli: false,
            },
        },
        virtual_media_config: VirtualMediaConfig {
            service_enabled: true,
            port: 0,
        },
        hosting_roles: Vec::new(),
        links: ComputerSystemLinks {
            chassis: vec![ODataId::new("/redfish/v1/Chassis/1")],
            managed_by: vec![ODataId::new("/redfish/v1/Managers/vbmc")],
            trusted_components: vec![ODataId::new(format!(
                "/redfish/v1/Chassis/1/TrustedComponents/{system_id}"
            ))],
            cooled_by: vec![ODataId::new(
                "/redfish/v1/Chassis/1/ThermalSubsystem/Fans/0",
            )],
            powered_by: vec![ODataId::new(
                "/redfish/v1/Chassis/1/PowerSubsystem/PowerSupplies/0",
            )],
            consuming_computer_systems: Vec::new(),
            supplying_computer_systems: Vec::new(),
            offloaded_network_device_functions: Vec::new(),
            endpoints: Vec::new(),
            resource_blocks: Vec::new(),
        },
    }))
}

#[derive(Debug, Deserialize)]
pub struct PatchBootOptions {
    #[serde(rename = "BootSourceOverrideTarget")]
    pub boot_source_override_target: Option<String>,
    #[serde(rename = "BootSourceOverrideEnabled")]
    pub boot_source_override_enabled: Option<String>,
    #[serde(rename = "BootSourceOverrideMode")]
    pub boot_source_override_mode: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PatchSystemRequest {
    #[serde(rename = "Boot")]
    pub boot: Option<PatchBootOptions>,
}

pub async fn patch_system(
    State(state): State<Arc<AppState>>,
    user: AuthenticatedUser,
    Path(system_id): Path<String>,
    Json(body): Json<PatchSystemRequest>,
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

const DEFAULT_BOOT_ORDER: &[&str] = &["Hdd", "Pxe", "Cd", "None"];

pub async fn set_default_boot_order(
    State(state): State<Arc<AppState>>,
    user: AuthenticatedUser,
    Path(system_id): Path<String>,
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

    let mut vm_state = state.get_vm_state(&system_id);
    vm_state.boot_override = crate::state::BootOverride::default();
    state.save_vm_state(&system_id, &vm_state);

    Ok(Json(serde_json::json!({
        "BootOrder": DEFAULT_BOOT_ORDER,
        "message": "Boot order reset to defaults"
    })))
}
