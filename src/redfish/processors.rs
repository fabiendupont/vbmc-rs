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
    #[serde(rename = "ProcessorType")]
    pub processor_type: &'static str,
    #[serde(rename = "TotalCores")]
    pub total_cores: u32,
    #[serde(rename = "TotalThreads")]
    pub total_threads: u32,
    #[serde(rename = "InstructionSet")]
    pub instruction_set: &'static str,
    #[serde(rename = "Manufacturer")]
    pub manufacturer: String,
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
        Ok(info) => {
            let cpus = info.config.cpus.as_ref();
            let boot = cpus.map(|c| c.boot_vcpus as u32).unwrap_or(1);
            let max = cpus.map(|c| c.max_vcpus as u32).unwrap_or(boot);
            (boot, max)
        }
        Err(_) => (0, 0),
    };

    let manufacturer = get_host_cpu_manufacturer();

    Ok(Json(Processor {
        odata_id: format!(
            "/redfish/v1/Systems/{system_id}/Processors/CPU0"
        ),
        odata_type: "#Processor.v1_18_0.Processor",
        id: "CPU0",
        name: "Virtual CPU",
        processor_type: "CPU",
        total_cores: cores,
        total_threads: threads,
        instruction_set: "x86-64",
        manufacturer,
        status: Status::enabled_ok(),
    }))
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
