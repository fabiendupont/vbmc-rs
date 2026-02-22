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
    #[serde(rename = "Status")]
    pub status: Status,
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
        manufacturer,
        model,
        socket: "CPU0",
        processor_architecture: "x86",
        max_speed_mhz,
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
