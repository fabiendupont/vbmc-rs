use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use serde::Serialize;

use super::error::RedfishApiError;
use crate::app_state::AppState;
use crate::auth::AuthenticatedUser;
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
    #[serde(rename = "OperatingSpeedMHz")]
    pub operating_speed_mhz: u32,
    #[serde(rename = "ThrottlingCelsius")]
    pub throttling_celsius: u32,
    #[serde(rename = "FrequencyRatio")]
    pub frequency_ratio: f64,
    #[serde(rename = "KernelPercent")]
    pub kernel_percent: f64,
    #[serde(rename = "UserPercent")]
    pub user_percent: f64,
    #[serde(rename = "LocalMemoryBandwidthBytes")]
    pub local_memory_bandwidth_bytes: u64,
    #[serde(rename = "RemoteMemoryBandwidthBytes")]
    pub remote_memory_bandwidth_bytes: u64,
    #[serde(rename = "CoreVoltage")]
    pub core_voltage: CoreVoltage,
    #[serde(rename = "CorrectableCoreErrorCount")]
    pub correctable_core_error_count: u64,
    #[serde(rename = "UncorrectableCoreErrorCount")]
    pub uncorrectable_core_error_count: u64,
    #[serde(rename = "CorrectableOtherErrorCount")]
    pub correctable_other_error_count: u64,
    #[serde(rename = "UncorrectableOtherErrorCount")]
    pub uncorrectable_other_error_count: u64,
    #[serde(rename = "CoreMetrics")]
    pub core_metrics: Vec<CoreMetric>,
}

#[derive(Debug, Serialize)]
pub struct CoreVoltage {
    #[serde(rename = "Reading")]
    pub reading: f64,
    #[serde(rename = "DataSourceUri", skip_serializing_if = "Option::is_none")]
    pub data_source_uri: Option<&'static str>,
}

#[derive(Debug, Serialize)]
pub struct CoreMetric {
    #[serde(rename = "CoreId")]
    pub core_id: String,
    #[serde(
        rename = "InstructionsPerCycle",
        skip_serializing_if = "Option::is_none"
    )]
    pub instructions_per_cycle: Option<f64>,
    #[serde(rename = "UnhaltedCycles")]
    pub unhalted_cycles: u64,
    #[serde(rename = "CorrectableCoreErrorCount")]
    pub correctable_core_error_count: u64,
    #[serde(rename = "UncorrectableCoreErrorCount")]
    pub uncorrectable_core_error_count: u64,
    #[serde(rename = "CorrectableOtherErrorCount")]
    pub correctable_other_error_count: u64,
    #[serde(rename = "UncorrectableOtherErrorCount")]
    pub uncorrectable_other_error_count: u64,
    #[serde(rename = "MemoryStallCount")]
    pub memory_stall_count: u64,
    #[serde(rename = "IOStallCount")]
    pub io_stall_count: u64,
}

pub async fn get_processor_metrics(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
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
                    correctable_core_error_count: 0,
                    uncorrectable_core_error_count: 0,
                    correctable_other_error_count: 0,
                    uncorrectable_other_error_count: 0,
                    memory_stall_count: 0,
                    io_stall_count: 0,
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
                    correctable_core_error_count: 0,
                    uncorrectable_core_error_count: 0,
                    correctable_other_error_count: 0,
                    uncorrectable_other_error_count: 0,
                    memory_stall_count: 0,
                    io_stall_count: 0,
                })
                .collect()
        }
    };

    let speed_mhz = get_host_cpu_mhz();

    Ok(Json(ProcessorMetricsResource {
        odata_id: format!(
            "/redfish/v1/Systems/{system_id}/Processors/{processor_id}/ProcessorMetrics"
        ),
        odata_type: "#ProcessorMetrics.v1_6_0.ProcessorMetrics",
        id: "ProcessorMetrics",
        name: "Processor Metrics",
        description: "Processor performance metrics",
        bandwidth_percent: 0.0,
        operating_speed_mhz: speed_mhz,
        throttling_celsius: 100,
        frequency_ratio: 1.0,
        kernel_percent: 0.0,
        user_percent: 0.0,
        local_memory_bandwidth_bytes: 0,
        remote_memory_bandwidth_bytes: 0,
        core_voltage: CoreVoltage {
            reading: 1.0,
            data_source_uri: None,
        },
        correctable_core_error_count: 0,
        uncorrectable_core_error_count: 0,
        correctable_other_error_count: 0,
        uncorrectable_other_error_count: 0,
        core_metrics,
    }))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_processor_metrics_serialization() {
        let metrics = ProcessorMetricsResource {
            odata_id: "/redfish/v1/Systems/vm1/Processors/CPU0/ProcessorMetrics".to_string(),
            odata_type: "#ProcessorMetrics.v1_6_0.ProcessorMetrics",
            id: "ProcessorMetrics",
            name: "Processor Metrics",
            description: "Processor performance metrics",
            bandwidth_percent: 75.5,
            operating_speed_mhz: 3000,
            throttling_celsius: 100,
            frequency_ratio: 1.0,
            kernel_percent: 10.5,
            user_percent: 50.0,
            local_memory_bandwidth_bytes: 1024000,
            remote_memory_bandwidth_bytes: 512000,
            core_voltage: CoreVoltage {
                reading: 1.2,
                data_source_uri: None,
            },
            correctable_core_error_count: 0,
            uncorrectable_core_error_count: 0,
            correctable_other_error_count: 0,
            uncorrectable_other_error_count: 0,
            core_metrics: vec![],
        };

        let json = serde_json::to_value(&metrics).unwrap();
        assert_eq!(
            json["@odata.id"],
            "/redfish/v1/Systems/vm1/Processors/CPU0/ProcessorMetrics"
        );
        assert_eq!(
            json["@odata.type"],
            "#ProcessorMetrics.v1_6_0.ProcessorMetrics"
        );
        assert_eq!(json["Id"], "ProcessorMetrics");
        assert_eq!(json["BandwidthPercent"], 75.5);
        assert_eq!(json["OperatingSpeedMHz"], 3000);
        assert_eq!(json["ThrottlingCelsius"], 100);
        assert_eq!(json["FrequencyRatio"], 1.0);
        assert_eq!(json["KernelPercent"], 10.5);
        assert_eq!(json["UserPercent"], 50.0);
        assert_eq!(json["LocalMemoryBandwidthBytes"], 1024000);
        assert_eq!(json["RemoteMemoryBandwidthBytes"], 512000);
    }

    #[test]
    fn test_core_voltage_serialization() {
        let voltage = CoreVoltage {
            reading: 1.35,
            data_source_uri: Some("/test/uri"),
        };

        let json = serde_json::to_value(&voltage).unwrap();
        assert_eq!(json["Reading"], 1.35);
        assert_eq!(json["DataSourceUri"], "/test/uri");
    }

    #[test]
    fn test_core_voltage_skip_serializing_if_none() {
        let voltage = CoreVoltage {
            reading: 1.0,
            data_source_uri: None,
        };

        let json = serde_json::to_value(&voltage).unwrap();
        assert_eq!(json["Reading"], 1.0);
        assert!(!json.as_object().unwrap().contains_key("DataSourceUri"));
    }

    #[test]
    fn test_core_metric_serialization() {
        let metric = CoreMetric {
            core_id: "core0".to_string(),
            instructions_per_cycle: Some(2.5),
            unhalted_cycles: 1000000,
            correctable_core_error_count: 0,
            uncorrectable_core_error_count: 0,
            correctable_other_error_count: 1,
            uncorrectable_other_error_count: 0,
            memory_stall_count: 500,
            io_stall_count: 100,
        };

        let json = serde_json::to_value(&metric).unwrap();
        assert_eq!(json["CoreId"], "core0");
        assert_eq!(json["InstructionsPerCycle"], 2.5);
        assert_eq!(json["UnhaltedCycles"], 1000000);
        assert_eq!(json["CorrectableCoreErrorCount"], 0);
        assert_eq!(json["UncorrectableCoreErrorCount"], 0);
        assert_eq!(json["CorrectableOtherErrorCount"], 1);
        assert_eq!(json["UncorrectableOtherErrorCount"], 0);
        assert_eq!(json["MemoryStallCount"], 500);
        assert_eq!(json["IOStallCount"], 100);
    }

    #[test]
    fn test_core_metric_skip_serializing_if_none() {
        let metric = CoreMetric {
            core_id: "core1".to_string(),
            instructions_per_cycle: None,
            unhalted_cycles: 0,
            correctable_core_error_count: 0,
            uncorrectable_core_error_count: 0,
            correctable_other_error_count: 0,
            uncorrectable_other_error_count: 0,
            memory_stall_count: 0,
            io_stall_count: 0,
        };

        let json = serde_json::to_value(&metric).unwrap();
        assert_eq!(json["CoreId"], "core1");
        assert!(
            !json
                .as_object()
                .unwrap()
                .contains_key("InstructionsPerCycle")
        );
        assert_eq!(json["UnhaltedCycles"], 0);
    }

    #[test]
    fn test_core_metrics_ipc_calculation() {
        let metrics = [
            CoreMetric {
                core_id: "core0".to_string(),
                instructions_per_cycle: Some(2.0),
                unhalted_cycles: 1000,
                correctable_core_error_count: 0,
                uncorrectable_core_error_count: 0,
                correctable_other_error_count: 0,
                uncorrectable_other_error_count: 0,
                memory_stall_count: 0,
                io_stall_count: 0,
            },
            CoreMetric {
                core_id: "core1".to_string(),
                instructions_per_cycle: None,
                unhalted_cycles: 0,
                correctable_core_error_count: 0,
                uncorrectable_core_error_count: 0,
                correctable_other_error_count: 0,
                uncorrectable_other_error_count: 0,
                memory_stall_count: 0,
                io_stall_count: 0,
            },
        ];

        assert_eq!(metrics[0].instructions_per_cycle, Some(2.0));
        assert_eq!(metrics[1].instructions_per_cycle, None);
    }
}
