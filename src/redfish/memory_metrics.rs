use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use serde::Serialize;

use super::error::RedfishApiError;
use crate::app_state::AppState;
use crate::backend::VmmBackend;

#[derive(Debug, Serialize)]
pub struct MemoryMetricsResource {
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
    #[serde(rename = "OperatingSpeedMHz")]
    pub operating_speed_mhz: u32,
    #[serde(rename = "BandwidthPercent")]
    pub bandwidth_percent: f64,
    #[serde(rename = "BlockSizeBytes")]
    pub block_size_bytes: u64,
    #[serde(rename = "CapacityUtilizationPercent")]
    pub capacity_utilization_percent: f64,
    #[serde(rename = "CorrectedVolatileErrorCount")]
    pub corrected_volatile_error_count: u64,
    #[serde(rename = "CorrectedPersistentErrorCount")]
    pub corrected_persistent_error_count: u64,
    #[serde(rename = "DirtyShutdownCount")]
    pub dirty_shutdown_count: u64,
    #[serde(rename = "HealthData")]
    pub health_data: HealthData,
    #[serde(rename = "LifeTime")]
    pub life_time: LifeTime,
    #[serde(rename = "CurrentPeriod")]
    pub current_period: CurrentPeriod,
}

#[derive(Debug, Serialize)]
pub struct HealthData {
    #[serde(rename = "RemainingSpareBlockPercentage")]
    pub remaining_spare_block_percentage: f64,
    #[serde(rename = "LastShutdownSuccess")]
    pub last_shutdown_success: bool,
    #[serde(rename = "DataLossDetected")]
    pub data_loss_detected: bool,
    #[serde(rename = "PerformanceDegraded")]
    pub performance_degraded: bool,
    #[serde(rename = "PredictedMediaLifeLeftPercent")]
    pub predicted_media_life_left_percent: f64,
}

#[derive(Debug, Serialize)]
pub struct LifeTime {
    #[serde(rename = "BlocksRead")]
    pub blocks_read: u64,
    #[serde(rename = "BlocksWritten")]
    pub blocks_written: u64,
}

#[derive(Debug, Serialize)]
pub struct CurrentPeriod {
    #[serde(rename = "BlocksRead")]
    pub blocks_read: u64,
    #[serde(rename = "BlocksWritten")]
    pub blocks_written: u64,
}

pub async fn get_memory_metrics(
    State(state): State<Arc<AppState>>,
    Path((system_id, dimm_id)): Path<(String, String)>,
) -> Result<Json<MemoryMetricsResource>, RedfishApiError> {
    if !state.config.systems.contains_key(&system_id) {
        return Err(RedfishApiError::NotFound(format!(
            "System '{system_id}' not found"
        )));
    }
    if dimm_id != "DIMM0" {
        return Err(RedfishApiError::NotFound(format!(
            "Memory '{dimm_id}' not found"
        )));
    }

    let (blocks_read, blocks_written) = match state.backend.vm_counters(&system_id).await {
        Ok(c) => (c.block_read_ops, c.block_write_ops),
        Err(_) => (0, 0),
    };

    Ok(Json(MemoryMetricsResource {
        odata_id: format!("/redfish/v1/Systems/{system_id}/Memory/{dimm_id}/MemoryMetrics"),
        odata_type: "#MemoryMetrics.v1_7_0.MemoryMetrics",
        id: "MemoryMetrics",
        name: "Memory Metrics",
        description: "Memory performance metrics",
        operating_speed_mhz: 3200,
        bandwidth_percent: 0.0,
        block_size_bytes: 64,
        capacity_utilization_percent: 0.0,
        corrected_volatile_error_count: 0,
        corrected_persistent_error_count: 0,
        dirty_shutdown_count: 0,
        health_data: HealthData {
            remaining_spare_block_percentage: 100.0,
            last_shutdown_success: true,
            data_loss_detected: false,
            performance_degraded: false,
            predicted_media_life_left_percent: 100.0,
        },
        life_time: LifeTime {
            blocks_read,
            blocks_written,
        },
        current_period: CurrentPeriod {
            blocks_read,
            blocks_written,
        },
    }))
}
