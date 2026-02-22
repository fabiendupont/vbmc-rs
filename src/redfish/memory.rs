use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use serde::Serialize;

use super::error::RedfishApiError;
use super::types::{Collection, ODataId, Status};
use crate::app_state::AppState;
use crate::backend::VmmBackend;

#[derive(Debug, Serialize)]
pub struct MemoryResource {
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
    #[serde(rename = "CapacityMiB")]
    pub capacity_mib: u64,
    #[serde(rename = "MemoryDeviceType")]
    pub memory_device_type: &'static str,
    #[serde(rename = "MemoryType")]
    pub memory_type: &'static str,
    #[serde(rename = "DataWidthBits")]
    pub data_width_bits: u32,
    #[serde(rename = "BusWidthBits")]
    pub bus_width_bits: u32,
    #[serde(rename = "ErrorCorrection")]
    pub error_correction: &'static str,
    #[serde(rename = "OperatingSpeedMhz")]
    pub operating_speed_mhz: u32,
    #[serde(rename = "Manufacturer")]
    pub manufacturer: &'static str,
    #[serde(rename = "SerialNumber")]
    pub serial_number: String,
    #[serde(rename = "PartNumber")]
    pub part_number: &'static str,
    #[serde(rename = "Model")]
    pub model: &'static str,
    #[serde(rename = "RankCount")]
    pub rank_count: u32,
    #[serde(rename = "OperatingMemoryModes")]
    pub operating_memory_modes: Vec<&'static str>,
    #[serde(rename = "MemoryMedia")]
    pub memory_media: Vec<&'static str>,
    #[serde(rename = "SecurityState")]
    pub security_state: &'static str,
    #[serde(rename = "Enabled")]
    pub enabled: bool,
    #[serde(rename = "VolatileSizeMiB")]
    pub volatile_size_mib: u64,
    #[serde(rename = "NonVolatileSizeMiB")]
    pub non_volatile_size_mib: u64,
    #[serde(rename = "BaseModuleType")]
    pub base_module_type: &'static str,
    #[serde(rename = "LogicalSizeMiB")]
    pub logical_size_mib: u64,
    #[serde(rename = "ConfigurationLocked")]
    pub configuration_locked: bool,
    #[serde(rename = "MaxTDPMilliWatts")]
    pub max_tdp_milliwatts: Vec<u32>,
    #[serde(rename = "LocationIndicatorActive")]
    pub location_indicator_active: bool,
    #[serde(rename = "FirmwareRevision")]
    pub firmware_revision: &'static str,
    #[serde(rename = "AllowedSpeedsMHz")]
    pub allowed_speeds_mhz: Vec<u32>,
    #[serde(rename = "MemoryLocation")]
    pub memory_location: MemoryLocation,
    #[serde(rename = "Location")]
    pub location: MemDimmLocation,
    #[serde(rename = "Status")]
    pub status: Status,
}

#[derive(Debug, Serialize)]
pub struct MemoryLocation {
    #[serde(rename = "Socket")]
    pub socket: u32,
    #[serde(rename = "MemoryController")]
    pub memory_controller: u32,
    #[serde(rename = "Channel")]
    pub channel: u32,
    #[serde(rename = "Slot")]
    pub slot: u32,
}

#[derive(Debug, Serialize)]
pub struct MemDimmLocation {
    #[serde(rename = "Info")]
    pub info: &'static str,
    #[serde(rename = "InfoFormat")]
    pub info_format: &'static str,
}

pub async fn get_memory_collection(
    State(state): State<Arc<AppState>>,
    Path(system_id): Path<String>,
) -> Result<Json<Collection<ODataId>>, RedfishApiError> {
    if !state.config.systems.contains_key(&system_id) {
        return Err(RedfishApiError::NotFound(format!(
            "System '{system_id}' not found"
        )));
    }

    let members = vec![ODataId::new(format!(
        "/redfish/v1/Systems/{system_id}/Memory/DIMM0"
    ))];

    Ok(Json(Collection::new(
        format!("/redfish/v1/Systems/{system_id}/Memory"),
        "#MemoryCollection.MemoryCollection",
        "Memory Collection",
        members,
    )))
}

pub async fn get_memory(
    State(state): State<Arc<AppState>>,
    Path((system_id, dimm_id)): Path<(String, String)>,
) -> Result<Json<MemoryResource>, RedfishApiError> {
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

    let capacity_mib = match state.backend.vm_info(&system_id).await {
        Ok(info) => info.memory_bytes / (1024 * 1024),
        Err(_) => 0,
    };

    Ok(Json(MemoryResource {
        odata_id: format!("/redfish/v1/Systems/{system_id}/Memory/{dimm_id}"),
        odata_type: "#Memory.v1_19_0.Memory",
        id: dimm_id,
        name: "Virtual DIMM 0".to_string(),
        description: "Virtual memory module",
        capacity_mib,
        memory_device_type: "DDR4",
        memory_type: "DRAM",
        data_width_bits: 64,
        bus_width_bits: 72,
        error_correction: "NoECC",
        operating_speed_mhz: 3200,
        manufacturer: "Virtual",
        serial_number: format!("VBMC-MEM-{system_id}-0"),
        part_number: "VBMC-DIMM",
        model: "Virtual DIMM",
        rank_count: 1,
        operating_memory_modes: vec!["Volatile"],
        memory_media: vec!["DRAM"],
        security_state: "Enabled",
        enabled: true,
        volatile_size_mib: capacity_mib,
        non_volatile_size_mib: 0,
        base_module_type: "RDIMM",
        logical_size_mib: capacity_mib,
        configuration_locked: false,
        max_tdp_milliwatts: vec![12000],
        location_indicator_active: false,
        firmware_revision: "1.0",
        allowed_speeds_mhz: vec![2133, 2400, 2666, 3200],
        memory_location: MemoryLocation {
            socket: 0,
            memory_controller: 0,
            channel: 0,
            slot: 0,
        },
        location: MemDimmLocation {
            info: "DIMM 0",
            info_format: "DIMM",
        },
        status: Status::enabled_ok(),
    }))
}
