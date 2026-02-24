use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use serde::Serialize;

use super::error::RedfishApiError;
use super::types::{Collection, ODataId, Status};
use crate::app_state::AppState;
use crate::backend::VmmBackend;

#[derive(Debug, Serialize)]
pub struct Processor {
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
    #[serde(rename = "ProcessorType")]
    pub processor_type: &'static str,
    #[serde(rename = "TotalCores")]
    pub total_cores: u32,
    #[serde(rename = "TotalThreads")]
    pub total_threads: u32,
    #[serde(rename = "InstructionSet")]
    pub instruction_set: &'static str,
    #[serde(rename = "TotalEnabledCores")]
    pub total_enabled_cores: u32,
    #[serde(rename = "Manufacturer")]
    pub manufacturer: String,
    #[serde(rename = "Model")]
    pub model: String,
    #[serde(rename = "Socket")]
    pub socket: &'static str,
    #[serde(rename = "ProcessorArchitecture")]
    pub processor_architecture: &'static str,
    #[serde(rename = "MaxSpeedMHz")]
    pub max_speed_mhz: u32,
    #[serde(rename = "BaseSpeedMHz")]
    pub base_speed_mhz: u32,
    #[serde(rename = "OperatingSpeedMHz")]
    pub operating_speed_mhz: u32,
    #[serde(rename = "MaxTDPWatts")]
    pub max_tdp_watts: u32,
    #[serde(rename = "TDPWatts")]
    pub tdp_watts: u32,
    #[serde(rename = "SerialNumber")]
    pub serial_number: String,
    #[serde(rename = "PartNumber")]
    pub part_number: &'static str,
    #[serde(rename = "FirmwareVersion")]
    pub firmware_version: &'static str,
    #[serde(rename = "Enabled")]
    pub enabled: bool,
    #[serde(rename = "PowerState")]
    pub power_state: &'static str,
    #[serde(rename = "TurboState")]
    pub turbo_state: &'static str,
    #[serde(rename = "Throttled")]
    pub throttled: bool,
    #[serde(rename = "UUID")]
    pub uuid: String,
    #[serde(rename = "Family")]
    pub family: &'static str,
    #[serde(rename = "Version")]
    pub version: String,
    #[serde(rename = "ProcessorIndex")]
    pub processor_index: u32,
    #[serde(rename = "MinSpeedMHz")]
    pub min_speed_mhz: u32,
    #[serde(rename = "Replaceable")]
    pub replaceable: bool,
    #[serde(rename = "SpeedLimitMHz")]
    pub speed_limit_mhz: u32,
    #[serde(rename = "SpeedLocked")]
    pub speed_locked: bool,
    #[serde(rename = "ThrottleCauses")]
    pub throttle_causes: Vec<&'static str>,
    #[serde(rename = "LocationIndicatorActive")]
    pub location_indicator_active: bool,
    #[serde(rename = "OperatingSpeedRangeMHz")]
    pub operating_speed_range_mhz: SpeedRange,
    #[serde(rename = "Links")]
    pub proc_links: ProcessorLinks,
    #[serde(rename = "Location")]
    pub location: super::types::RedfishLocation,
    #[serde(rename = "ProcessorId")]
    pub processor_id: ProcessorIdInfo,
    #[serde(rename = "Status")]
    pub status: Status,
}

#[derive(Debug, Serialize)]
pub struct SpeedRange {
    #[serde(rename = "AllowableMin")]
    pub allowable_min: u32,
    #[serde(rename = "AllowableMax")]
    pub allowable_max: u32,
    #[serde(rename = "Reading")]
    pub reading: u32,
    #[serde(rename = "ReadingUnits")]
    pub reading_units: &'static str,
    #[serde(rename = "ControlMode")]
    pub control_mode: &'static str,
    #[serde(rename = "SettingMin")]
    pub setting_min: u32,
    #[serde(rename = "SettingMax")]
    pub setting_max: u32,
}

#[derive(Debug, Serialize)]
pub struct ProcessorLinks {
    #[serde(rename = "Chassis")]
    pub chassis: ODataId,
    #[serde(rename = "Memory")]
    pub memory: Vec<ODataId>,
}

#[derive(Debug, Serialize)]
pub struct ProcessorIdInfo {
    #[serde(rename = "VendorId")]
    pub vendor_id: String,
    #[serde(rename = "IdentificationRegisters")]
    pub identification_registers: &'static str,
    #[serde(rename = "EffectiveFamily")]
    pub effective_family: &'static str,
    #[serde(rename = "EffectiveModel")]
    pub effective_model: &'static str,
    #[serde(rename = "Step")]
    pub step: &'static str,
    #[serde(rename = "MicrocodeInfo")]
    pub microcode_info: &'static str,
    #[serde(rename = "ProtectedIdentificationNumber")]
    pub protected_identification_number: &'static str,
}

pub async fn get_processors(
    State(state): State<Arc<AppState>>,
    Path(system_id): Path<String>,
) -> Result<Json<Collection<ODataId>>, RedfishApiError> {
    if !state.config.systems.contains_key(&system_id) {
        return Err(RedfishApiError::NotFound(format!(
            "System '{system_id}' not found"
        )));
    }

    let members = vec![ODataId::new(format!(
        "/redfish/v1/Systems/{system_id}/Processors/CPU0"
    ))];

    Ok(Json(Collection::new(
        format!("/redfish/v1/Systems/{system_id}/Processors"),
        "#ProcessorCollection.ProcessorCollection",
        "Processor Collection",
        members,
    )))
}

pub async fn get_processor(
    State(state): State<Arc<AppState>>,
    Path((system_id, processor_id)): Path<(String, String)>,
) -> Result<Json<Processor>, RedfishApiError> {
    if !state.config.systems.contains_key(&system_id) {
        return Err(RedfishApiError::NotFound(format!(
            "System '{system_id}' not found"
        )));
    }
    if processor_id != "CPU0" {
        return Err(RedfishApiError::NotFound(format!(
            "Processor '{processor_id}' not found"
        )));
    }

    let (cores, threads) = match state.backend.vm_info(&system_id).await {
        Ok(info) => (info.cpu_count, info.max_cpu_count),
        Err(_) => (0, 0),
    };

    let manufacturer = get_host_cpu_manufacturer();
    let model = get_host_cpu_model();
    let max_speed_mhz = get_host_cpu_mhz();

    Ok(Json(Processor {
        odata_id: format!(
            "/redfish/v1/Systems/{system_id}/Processors/CPU0"
        ),
        odata_type: "#Processor.v1_18_0.Processor",
        id: "CPU0",
        name: "Virtual CPU",
        description: "Virtual CPU",
        processor_type: "CPU",
        total_cores: cores,
        total_threads: threads,
        total_enabled_cores: cores,
        instruction_set: "x86-64",
        manufacturer: manufacturer.clone(),
        model: model.clone(),
        socket: "CPU0",
        processor_architecture: "x86",
        max_speed_mhz,
        base_speed_mhz: max_speed_mhz,
        operating_speed_mhz: max_speed_mhz,
        max_tdp_watts: 125,
        tdp_watts: 125,
        serial_number: format!("VBMC-CPU-{system_id}"),
        part_number: "VBMC-CPU",
        firmware_version: "N/A",
        enabled: true,
        power_state: "On",
        turbo_state: "Disabled",
        throttled: false,
        uuid: uuid::Uuid::new_v5(
            &uuid::Uuid::NAMESPACE_URL,
            format!("vbmc-rs:cpu:{system_id}").as_bytes(),
        )
        .to_string(),
        family: "Xeon",
        version: model.to_string(),
        processor_index: 0,
        min_speed_mhz: 800,
        replaceable: false,
        speed_limit_mhz: max_speed_mhz,
        speed_locked: false,
        throttle_causes: Vec::new(),
        location_indicator_active: false,
        operating_speed_range_mhz: SpeedRange {
            allowable_min: 800,
            allowable_max: max_speed_mhz,
            reading: max_speed_mhz,
            reading_units: "MHz",
            control_mode: "Automatic",
            setting_min: 800,
            setting_max: max_speed_mhz,
        },
        proc_links: ProcessorLinks {
            chassis: ODataId::new("/redfish/v1/Chassis/1"),
            memory: vec![ODataId::new(format!(
                "/redfish/v1/Systems/{system_id}/Memory/DIMM0"
            ))],
        },
        location: super::types::RedfishLocation::new("Socket CPU0", "Socket", "CPU0", "Socket", 0),
        processor_id: ProcessorIdInfo {
            vendor_id: manufacturer.clone(),
            identification_registers: "0x00000000",
            effective_family: "0x06",
            effective_model: "0x3E",
            step: "0x04",
            microcode_info: "0x00000000",
            protected_identification_number: "0",
        },
        status: Status::enabled_ok(),
    }))
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

fn get_host_cpu_mhz() -> u32 {
    std::fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|content| {
            content
                .lines()
                .find(|l| l.starts_with("cpu MHz"))
                .and_then(|l| {
                    l.split(':')
                        .nth(1)
                        .and_then(|v| v.trim().parse::<f64>().ok())
                        .map(|v| v as u32)
                })
        })
        .unwrap_or(0)
}

fn get_host_cpu_manufacturer() -> String {
    std::fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|content| {
            content
                .lines()
                .find(|l| l.starts_with("vendor_id"))
                .map(|l| {
                    l.split(':')
                        .nth(1)
                        .unwrap_or("Unknown")
                        .trim()
                        .to_string()
                })
        })
        .unwrap_or_else(|| "Unknown".to_string())
}
