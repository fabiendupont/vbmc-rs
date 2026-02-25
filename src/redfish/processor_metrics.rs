use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use serde::Serialize;

use super::error::RedfishApiError;
use crate::app_state::AppState;
use crate::backend::VmmBackend;

#[derive(Debug, Serialize)]
pub struct ProcessorMetricsResource {
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
    #[serde(rename = "BandwidthPercent")]
    pub bandwidth_percent: f64,
    #[serde(rename = "CoreMetrics")]
    pub core_metrics: Vec<CoreMetric>,
}

#[derive(Debug, Serialize)]
pub struct CoreMetric {
    #[serde(rename = "CoreId")]
    pub core_id: String,
    #[serde(rename = "InstructionsPerCycle", skip_serializing_if = "Option::is_none")]
    pub instructions_per_cycle: Option<f64>,
    #[serde(rename = "UnhaltedCycles")]
    pub unhalted_cycles: u64,
}

pub async fn get_processor_metrics(
    State(state): State<Arc<AppState>>,
    Path((system_id, processor_id)): Path<(String, String)>,
) -> Result<Json<ProcessorMetricsResource>, RedfishApiError> {
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

    let counters = state.backend.vm_counters(&system_id).await.ok();

    let core_metrics = match &counters {
        Some(c) if !c.cpu_cycles.is_empty() => c
            .cpu_cycles
            .iter()
            .enumerate()
            .map(|(i, &cycles)| {
                let instructions = c.instructions.get(i).copied().unwrap_or(0);
                let ipc = if cycles > 0 {
                    Some(instructions as f64 / cycles as f64)
                } else {
                    None
                };
                CoreMetric {
                    core_id: format!("core{i}"),
                    instructions_per_cycle: ipc,
                    unhalted_cycles: cycles,
                }
            })
            .collect(),
        _ => {
            let cpu_count = state
                .backend
                .vm_info(&system_id)
                .await
                .map(|info| info.cpu_count)
                .unwrap_or(1);
            (0..cpu_count)
                .map(|i| CoreMetric {
                    core_id: format!("core{i}"),
                    instructions_per_cycle: None,
                    unhalted_cycles: 0,
                })
                .collect()
        }
    };

    Ok(Json(ProcessorMetricsResource {
        odata_id: format!(
            "/redfish/v1/Systems/{system_id}/Processors/{processor_id}/ProcessorMetrics"
        ),
        odata_type: "#ProcessorMetrics.v1_6_0.ProcessorMetrics",
        id: "ProcessorMetrics",
        name: "Processor Metrics",
        description: "Processor performance metrics",
        bandwidth_percent: 0.0,
        core_metrics,
    }))
}
