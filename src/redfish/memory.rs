use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
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
    #[serde(rename = "IsRankSpareEnabled")]
    pub is_rank_spare_enabled: bool,
    #[serde(rename = "IsSpareDeviceEnabled")]
    pub is_spare_device_enabled: bool,
    #[serde(rename = "SpareDeviceCount")]
    pub spare_device_count: u32,
    #[serde(rename = "FirmwareApiVersion")]
    pub firmware_api_version: &'static str,
    #[serde(rename = "ModuleManufacturerID")]
    pub module_manufacturer_id: &'static str,
    #[serde(rename = "ModuleProductID")]
    pub module_product_id: &'static str,
    #[serde(rename = "MemorySubsystemControllerManufacturerID")]
    pub memory_subsystem_controller_manufacturer_id: &'static str,
    #[serde(rename = "MemorySubsystemControllerProductID")]
    pub memory_subsystem_controller_product_id: &'static str,
    #[serde(rename = "CacheSizeMiB")]
    pub cache_size_mib: u32,
    #[serde(rename = "SparePartNumber")]
    pub spare_part_number: &'static str,
    #[serde(rename = "VolatileSizeLimitMiB")]
    pub volatile_size_limit_mib: u64,
    #[serde(rename = "NonVolatileSizeLimitMiB")]
    pub non_volatile_size_limit_mib: u64,
    #[serde(rename = "VolatileRegionNumberLimit")]
    pub volatile_region_number_limit: u32,
    #[serde(rename = "PersistentRegionNumberLimit")]
    pub persistent_region_number_limit: u32,
    #[serde(rename = "VolatileRegionSizeMaxMiB")]
    pub volatile_region_size_max_mib: u64,
    #[serde(rename = "PersistentRegionSizeMaxMiB")]
    pub persistent_region_size_max_mib: u64,
    #[serde(rename = "VolatileRegionSizeLimitMiB")]
    pub volatile_region_size_limit_mib: u64,
    #[serde(rename = "PersistentRegionSizeLimitMiB")]
    pub persistent_region_size_limit_mib: u64,
    #[serde(rename = "AllocationIncrementMiB")]
    pub allocation_increment_mib: u64,
    #[serde(rename = "AllocationAlignmentMiB")]
    pub allocation_alignment_mib: u64,
    #[serde(rename = "PoisonListMaxMediaErrorRecords")]
    pub poison_list_max_media_error_records: u32,
    #[serde(rename = "SecurityCapabilities")]
    pub security_capabilities: MemSecurityCapabilities,
    #[serde(rename = "PowerManagementPolicy")]
    pub power_management_policy: MemPowerPolicy,
    #[serde(rename = "Regions")]
    pub regions: Vec<MemRegion>,
    #[serde(rename = "OperatingSpeedRangeMHz")]
    pub operating_speed_range_mhz: MemSpeedRange,
    #[serde(rename = "Metrics")]
    pub metrics: ODataId,
    #[serde(rename = "Links")]
    pub links: MemoryLinks,
    #[serde(rename = "Location")]
    pub location: super::types::RedfishLocation,
    #[serde(rename = "Status")]
    pub status: Status,
}

#[derive(Debug, Serialize)]
pub struct MemoryLinks {
    #[serde(rename = "Chassis")]
    pub chassis: ODataId,
    #[serde(rename = "Processors")]
    pub processors: Vec<ODataId>,
    #[serde(rename = "Batteries")]
    pub batteries: Vec<ODataId>,
    #[serde(rename = "MemoryMediaSources")]
    pub memory_media_sources: Vec<ODataId>,
    #[serde(rename = "MemoryRegionMediaSources")]
    pub memory_region_media_sources: Vec<ODataId>,
    #[serde(rename = "Endpoints")]
    pub endpoints: Vec<ODataId>,
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
pub struct MemSecurityCapabilities {
    #[serde(rename = "MaxPassphraseCount")]
    pub max_passphrase_count: u32,
    #[serde(rename = "PassphraseCapable")]
    pub passphrase_capable: bool,
    #[serde(rename = "SecurityStates")]
    pub security_states: Vec<&'static str>,
}

#[derive(Debug, Serialize)]
pub struct MemPowerPolicy {
    #[serde(rename = "PolicyEnabled")]
    pub policy_enabled: bool,
    #[serde(rename = "MaxTDPMilliWatts")]
    pub max_tdp_milliwatts: u32,
    #[serde(rename = "AveragePowerBudgetMilliWatts")]
    pub average_power_budget_milliwatts: u32,
    #[serde(rename = "PeakPowerBudgetMilliWatts")]
    pub peak_power_budget_milliwatts: u32,
}

#[derive(Debug, Serialize)]
pub struct MemRegion {
    #[serde(rename = "RegionId")]
    pub region_id: &'static str,
    #[serde(rename = "MemoryClassification")]
    pub memory_classification: &'static str,
    #[serde(rename = "SizeMiB")]
    pub size_mib: u64,
}

#[derive(Debug, Serialize)]
pub struct MemSpeedRange {
    #[serde(rename = "AllowableMin")]
    pub allowable_min: u32,
    #[serde(rename = "AllowableMax")]
    pub allowable_max: u32,
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
        is_rank_spare_enabled: false,
        is_spare_device_enabled: false,
        spare_device_count: 0,
        firmware_api_version: "1.0",
        module_manufacturer_id: "0x0000",
        module_product_id: "0x0000",
        memory_subsystem_controller_manufacturer_id: "0x0000",
        memory_subsystem_controller_product_id: "0x0000",
        cache_size_mib: 0,
        spare_part_number: "VBMC-DIMM-SPARE",
        volatile_size_limit_mib: capacity_mib,
        non_volatile_size_limit_mib: 0,
        volatile_region_number_limit: 0,
        persistent_region_number_limit: 0,
        volatile_region_size_max_mib: capacity_mib,
        persistent_region_size_max_mib: 0,
        volatile_region_size_limit_mib: capacity_mib,
        persistent_region_size_limit_mib: 0,
        allocation_increment_mib: 0,
        allocation_alignment_mib: 0,
        poison_list_max_media_error_records: 0,
        security_capabilities: MemSecurityCapabilities {
            max_passphrase_count: 0,
            passphrase_capable: false,
            security_states: vec!["Enabled"],
        },
        power_management_policy: MemPowerPolicy {
            policy_enabled: false,
            max_tdp_milliwatts: 12000,
            average_power_budget_milliwatts: 10000,
            peak_power_budget_milliwatts: 12000,
        },
        regions: vec![MemRegion {
            region_id: "0",
            memory_classification: "Volatile",
            size_mib: capacity_mib,
        }],
        operating_speed_range_mhz: MemSpeedRange {
            allowable_min: 2133,
            allowable_max: 3200,
        },
        metrics: ODataId::new(format!(
            "/redfish/v1/Systems/{system_id}/Memory/DIMM0/MemoryMetrics"
        )),
        links: MemoryLinks {
            chassis: ODataId::new("/redfish/v1/Chassis/1"),
            processors: vec![ODataId::new(format!(
                "/redfish/v1/Systems/{system_id}/Processors/CPU0"
            ))],
            batteries: Vec::new(),
            memory_media_sources: Vec::new(),
            memory_region_media_sources: Vec::new(),
            endpoints: Vec::new(),
        },
        location: super::types::RedfishLocation::new("DIMM 0", "DIMM", "DIMM0", "Slot", 0),
        status: Status::enabled_ok(),
    }))
}
