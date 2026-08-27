use std::collections::HashMap;
use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use serde::Serialize;

use super::error::RedfishApiError;
use super::types::{Collection, ODataId, Status};
use crate::app_state::AppState;
use crate::auth::AuthenticatedUser;
use crate::backend::VmmBackend;
use crate::backend::types::{DiskInfo, DiskMediaType, DiskProtocol};

#[derive(Debug, Serialize)]
pub struct StorageResource {
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
    #[serde(rename = "Controllers")]
    pub controllers: ODataId,
    #[serde(rename = "Drives")]
    pub drives: Vec<ODataId>,
    #[serde(rename = "Volumes")]
    pub volumes: ODataId,
    #[serde(rename = "Identifiers")]
    pub identifiers: Vec<StorageIdentifier>,
    #[serde(rename = "EncryptionMode")]
    pub encryption_mode: &'static str,
    #[serde(rename = "AutoVolumeCreate")]
    pub auto_volume_create: &'static str,
    #[serde(rename = "HotspareActivationPolicy")]
    pub hotspare_activation_policy: &'static str,
    #[serde(rename = "Links")]
    pub links: StorageLinks,
    #[serde(rename = "Status")]
    pub status: Status,
}

#[derive(Debug, Serialize)]
pub struct StorageIdentifier {
    #[serde(rename = "DurableName")]
    pub durable_name: String,
    #[serde(rename = "DurableNameFormat")]
    pub durable_name_format: &'static str,
}

#[derive(Debug, Serialize)]
pub struct StorageLinks {
    #[serde(rename = "Enclosures")]
    pub enclosures: Vec<ODataId>,
    #[serde(rename = "SimpleStorage")]
    pub simple_storage: ODataId,
    #[serde(rename = "NVMeoFDiscoverySubsystems")]
    pub nvmeof_discovery_subsystems: Vec<ODataId>,
    #[serde(rename = "HostingStorageSystems")]
    pub hosting_storage_systems: Vec<ODataId>,
    #[serde(rename = "StorageServices")]
    pub storage_services: Vec<ODataId>,
}

#[derive(Debug, Serialize)]
pub struct CtrlCacheSummary {
    #[serde(rename = "TotalCacheSizeMiB")]
    pub total_cache_size_mib: u32,
    #[serde(rename = "PersistentCacheSizeMiB")]
    pub persistent_cache_size_mib: u32,
    #[serde(rename = "Status")]
    pub status: Status,
}

#[derive(Debug, Serialize)]
pub struct CtrlRates {
    #[serde(rename = "ConsistencyCheckRatePercent")]
    pub consistency_check_rate_percent: u32,
    #[serde(rename = "RebuildRatePercent")]
    pub rebuild_rate_percent: u32,
    #[serde(rename = "TransformationRatePercent")]
    pub transformation_rate_percent: u32,
}

#[derive(Debug, Serialize)]
pub struct CtrlPcieInterface {
    #[serde(rename = "MaxPCIeType")]
    pub max_pcie_type: &'static str,
    #[serde(rename = "MaxLanes")]
    pub max_lanes: u32,
    #[serde(rename = "PCIeType")]
    pub pcie_type: &'static str,
    #[serde(rename = "LanesInUse")]
    pub lanes_in_use: u32,
}

#[derive(Debug, Serialize)]
pub struct CtrlLinks {
    #[serde(rename = "Endpoints")]
    pub endpoints: Vec<ODataId>,
    #[serde(rename = "PCIeFunctions")]
    pub pcie_functions: Vec<ODataId>,
}

#[derive(Debug, Serialize)]
pub struct DriveResource {
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
    #[serde(rename = "CapacityBytes", skip_serializing_if = "Option::is_none")]
    pub capacity_bytes: Option<u64>,
    #[serde(rename = "MediaType")]
    pub media_type: String,
    #[serde(rename = "Protocol")]
    pub protocol: String,
    #[serde(rename = "Status")]
    pub status: Status,
}

#[derive(Debug, Serialize)]
pub struct VolumeResource {
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
    #[serde(rename = "CapacityBytes", skip_serializing_if = "Option::is_none")]
    pub capacity_bytes: Option<u64>,
    #[serde(rename = "Status")]
    pub status: Status,
}

fn protocol_to_string(p: DiskProtocol) -> String {
    match p {
        DiskProtocol::Virtio => "Virtio".to_string(),
        DiskProtocol::NVMe => "NVMe".to_string(),
        DiskProtocol::Sata => "SATA".to_string(),
        DiskProtocol::VhostUser => "VhostUser".to_string(),
        DiskProtocol::Unknown => "Unknown".to_string(),
    }
}

fn media_type_to_string(m: DiskMediaType) -> String {
    match m {
        DiskMediaType::Ssd => "SSD".to_string(),
        DiskMediaType::Hdd => "HDD".to_string(),
        DiskMediaType::Virtual => "Virtual".to_string(),
        DiskMediaType::Unknown => "Unknown".to_string(),
    }
}

fn protocol_to_redfish_standard(ctrl_id: &str) -> String {
    match ctrl_id {
        "Virtio" => "PCIe".to_string(),
        "NVMe" => "NVMe".to_string(),
        "SATA" => "SATA".to_string(),
        "VhostUser" => "PCIe".to_string(),
        other => other.to_string(),
    }
}

fn controller_id_for_protocol(p: DiskProtocol) -> String {
    match p {
        DiskProtocol::Virtio => "Virtio".to_string(),
        DiskProtocol::NVMe => "NVMe".to_string(),
        DiskProtocol::Sata => "SATA".to_string(),
        DiskProtocol::VhostUser => "VhostUser".to_string(),
        DiskProtocol::Unknown => "Unknown".to_string(),
    }
}

fn group_disks_by_protocol(disks: &[DiskInfo]) -> HashMap<String, Vec<&DiskInfo>> {
    let mut grouped: HashMap<String, Vec<&DiskInfo>> = HashMap::new();
    for disk in disks {
        let ctrl_id = controller_id_for_protocol(disk.protocol);
        grouped.entry(ctrl_id).or_default().push(disk);
    }
    grouped
}

pub async fn get_storage_collection(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
    Path(system_id): Path<String>,
) -> Result<Json<Collection<ODataId>>, RedfishApiError> {
    if !state.config.systems.contains_key(&system_id) {
        return Err(RedfishApiError::NotFound(format!(
            "System '{system_id}' not found"
        )));
    }

    let mut members = Vec::new();
    if let Ok(info) = state.backend.vm_info(&system_id).await {
        let grouped = group_disks_by_protocol(&info.disks);
        for ctrl_id in grouped.keys() {
            members.push(ODataId::new(format!(
                "/redfish/v1/Systems/{system_id}/Storage/{ctrl_id}"
            )));
        }
    }

    if members.is_empty() {
        members.push(ODataId::new(format!(
            "/redfish/v1/Systems/{system_id}/Storage/Virtio"
        )));
    }

    Ok(Json(Collection::new(
        format!("/redfish/v1/Systems/{system_id}/Storage"),
        "#StorageCollection.StorageCollection",
        "Storage Collection",
        members,
    )))
}

pub async fn get_storage(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
    Path((system_id, ctrl_id)): Path<(String, String)>,
) -> Result<Json<StorageResource>, RedfishApiError> {
    if !state.config.systems.contains_key(&system_id) {
        return Err(RedfishApiError::NotFound(format!(
            "System '{system_id}' not found"
        )));
    }

    let mut drives = Vec::new();
    if let Ok(info) = state.backend.vm_info(&system_id).await {
        let grouped = group_disks_by_protocol(&info.disks);
        if let Some(disks) = grouped.get(&ctrl_id) {
            for disk in disks {
                drives.push(ODataId::new(format!(
                    "/redfish/v1/Systems/{system_id}/Storage/{ctrl_id}/Drives/{}",
                    disk.id
                )));
            }
        }
    }

    Ok(Json(StorageResource {
        odata_id: format!("/redfish/v1/Systems/{system_id}/Storage/{ctrl_id}"),
        odata_type: "#Storage.v1_15_0.Storage",
        id: ctrl_id.clone(),
        name: format!("{ctrl_id} Storage Controller"),
        description: "Storage controller",
        controllers: ODataId::new(format!(
            "/redfish/v1/Systems/{system_id}/Storage/{ctrl_id}/Controllers"
        )),
        drives,
        volumes: ODataId::new(format!(
            "/redfish/v1/Systems/{system_id}/Storage/{ctrl_id}/Volumes"
        )),
        identifiers: vec![StorageIdentifier {
            durable_name: uuid::Uuid::new_v5(
                &uuid::Uuid::NAMESPACE_URL,
                format!("vbmc-rs:storage:{system_id}:{ctrl_id}").as_bytes(),
            )
            .as_simple()
            .to_string(),
            durable_name_format: "NAA",
        }],
        encryption_mode: "Disabled",
        auto_volume_create: "Disabled",
        hotspare_activation_policy: "OEM",
        links: StorageLinks {
            enclosures: vec![ODataId::new("/redfish/v1/Chassis/1")],
            simple_storage: ODataId::new(format!(
                "/redfish/v1/Systems/{system_id}/SimpleStorage/1"
            )),
            nvmeof_discovery_subsystems: Vec::new(),
            hosting_storage_systems: Vec::new(),
            storage_services: Vec::new(),
        },
        status: Status::enabled_ok(),
    }))
}

pub async fn get_drive(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
    Path((system_id, ctrl_id, drive_id)): Path<(String, String, String)>,
) -> Result<Json<DriveResource>, RedfishApiError> {
    if !state.config.systems.contains_key(&system_id) {
        return Err(RedfishApiError::NotFound(format!(
            "System '{system_id}' not found"
        )));
    }

    let info = state
        .backend
        .vm_info(&system_id)
        .await
        .map_err(|e| RedfishApiError::InternalError(e.to_string()))?;

    let disk = info
        .disks
        .iter()
        .find(|d| d.id == drive_id && controller_id_for_protocol(d.protocol) == ctrl_id)
        .ok_or_else(|| RedfishApiError::NotFound(format!("Drive '{drive_id}' not found")))?;

    Ok(Json(DriveResource {
        odata_id: format!("/redfish/v1/Systems/{system_id}/Storage/{ctrl_id}/Drives/{drive_id}"),
        odata_type: "#Drive.v1_18_0.Drive",
        id: drive_id,
        name: disk.id.clone(),
        description: "Virtual disk drive",
        capacity_bytes: disk.capacity_bytes,
        media_type: media_type_to_string(disk.media_type),
        protocol: protocol_to_string(disk.protocol),
        status: Status::enabled_ok(),
    }))
}

pub async fn get_volumes(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
    Path((system_id, ctrl_id)): Path<(String, String)>,
) -> Result<Json<Collection<ODataId>>, RedfishApiError> {
    if !state.config.systems.contains_key(&system_id) {
        return Err(RedfishApiError::NotFound(format!(
            "System '{system_id}' not found"
        )));
    }

    let mut members = Vec::new();
    if let Ok(info) = state.backend.vm_info(&system_id).await {
        let grouped = group_disks_by_protocol(&info.disks);
        if let Some(disks) = grouped.get(&ctrl_id) {
            for disk in disks {
                members.push(ODataId::new(format!(
                    "/redfish/v1/Systems/{system_id}/Storage/{ctrl_id}/Volumes/{}",
                    disk.id
                )));
            }
        }
    }

    Ok(Json(Collection::new(
        format!("/redfish/v1/Systems/{system_id}/Storage/{ctrl_id}/Volumes"),
        "#VolumeCollection.VolumeCollection",
        "Volume Collection",
        members,
    )))
}

pub async fn get_volume(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
    Path((system_id, ctrl_id, vol_id)): Path<(String, String, String)>,
) -> Result<Json<VolumeResource>, RedfishApiError> {
    if !state.config.systems.contains_key(&system_id) {
        return Err(RedfishApiError::NotFound(format!(
            "System '{system_id}' not found"
        )));
    }

    let info = state
        .backend
        .vm_info(&system_id)
        .await
        .map_err(|e| RedfishApiError::InternalError(e.to_string()))?;

    let disk = info
        .disks
        .iter()
        .find(|d| d.id == vol_id && controller_id_for_protocol(d.protocol) == ctrl_id)
        .ok_or_else(|| RedfishApiError::NotFound(format!("Volume '{vol_id}' not found")))?;

    Ok(Json(VolumeResource {
        odata_id: format!("/redfish/v1/Systems/{system_id}/Storage/{ctrl_id}/Volumes/{vol_id}"),
        odata_type: "#Volume.v1_10_0.Volume",
        id: vol_id,
        name: disk.id.clone(),
        description: "Storage volume",
        capacity_bytes: disk.capacity_bytes,
        status: Status::enabled_ok(),
    }))
}

#[derive(Debug, Serialize)]
pub struct StorageControllerResource {
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
    #[serde(rename = "SupportedDeviceProtocols")]
    pub supported_device_protocols: Vec<String>,
    #[serde(rename = "SupportedControllerProtocols")]
    pub supported_controller_protocols: Vec<&'static str>,
    #[serde(rename = "SupportedRAIDTypes")]
    pub supported_raid_types: Vec<&'static str>,
    #[serde(rename = "FirmwareVersion")]
    pub firmware_version: &'static str,
    #[serde(rename = "Manufacturer")]
    pub manufacturer: &'static str,
    #[serde(rename = "Model")]
    pub model: String,
    #[serde(rename = "SerialNumber")]
    pub serial_number: String,
    #[serde(rename = "SpeedGbps")]
    pub speed_gbps: f64,
    #[serde(rename = "AssetTag")]
    pub asset_tag: &'static str,
    #[serde(rename = "PartNumber")]
    pub part_number: &'static str,
    #[serde(rename = "SKU")]
    pub sku: &'static str,
    #[serde(rename = "Identifiers")]
    pub sc_identifiers: Vec<StorageIdentifier>,
    #[serde(rename = "CacheSummary")]
    pub sc_cache_summary: CtrlCacheSummary,
    #[serde(rename = "ControllerRates")]
    pub sc_controller_rates: CtrlRates,
    #[serde(rename = "PCIeInterface")]
    pub sc_pcie_interface: CtrlPcieInterface,
    #[serde(rename = "Location")]
    pub sc_location: super::types::RedfishLocation,
    #[serde(rename = "Assembly")]
    pub assembly: ODataId,
    #[serde(rename = "Links")]
    pub sc_links: ScLinks,
    #[serde(rename = "Status")]
    pub status: Status,
}

#[derive(Debug, Serialize)]
pub struct ScLinks {
    #[serde(rename = "Endpoints")]
    pub endpoints: Vec<ODataId>,
    #[serde(rename = "PCIeFunctions")]
    pub pcie_functions: Vec<ODataId>,
    #[serde(rename = "AttachedVolumes")]
    pub attached_volumes: Vec<ODataId>,
}

pub async fn get_controllers(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
    Path((system_id, ctrl_id)): Path<(String, String)>,
) -> Result<Json<Collection<ODataId>>, RedfishApiError> {
    if !state.config.systems.contains_key(&system_id) {
        return Err(RedfishApiError::NotFound(format!(
            "System '{system_id}' not found"
        )));
    }

    let members = vec![ODataId::new(format!(
        "/redfish/v1/Systems/{system_id}/Storage/{ctrl_id}/Controllers/0"
    ))];

    Ok(Json(Collection::new(
        format!("/redfish/v1/Systems/{system_id}/Storage/{ctrl_id}/Controllers"),
        "#StorageControllerCollection.StorageControllerCollection",
        "Storage Controller Collection",
        members,
    )))
}

pub async fn get_controller(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
    Path((system_id, ctrl_id, controller_id)): Path<(String, String, String)>,
) -> Result<Json<StorageControllerResource>, RedfishApiError> {
    if !state.config.systems.contains_key(&system_id) {
        return Err(RedfishApiError::NotFound(format!(
            "System '{system_id}' not found"
        )));
    }
    if controller_id != "0" {
        return Err(RedfishApiError::NotFound(format!(
            "Controller '{controller_id}' not found"
        )));
    }

    Ok(Json(StorageControllerResource {
        odata_id: format!(
            "/redfish/v1/Systems/{system_id}/Storage/{ctrl_id}/Controllers/{controller_id}"
        ),
        odata_type: "#StorageController.v1_7_0.StorageController",
        id: controller_id.clone(),
        name: format!("{ctrl_id} Controller"),
        description: "Storage controller",
        supported_device_protocols: vec![protocol_to_redfish_standard(&ctrl_id)],
        supported_controller_protocols: vec!["PCIe"],
        supported_raid_types: vec!["None"],
        firmware_version: "1.0",
        manufacturer: "vbmc-rs",
        model: format!("Virtual {ctrl_id} Controller"),
        serial_number: format!("VBMC-STOR-{ctrl_id}"),
        speed_gbps: 16.0,
        asset_tag: "",
        part_number: "VBMC-STOR",
        sku: "VBMC-VIRTUAL",
        sc_identifiers: vec![StorageIdentifier {
            durable_name: uuid::Uuid::new_v5(
                &uuid::Uuid::NAMESPACE_URL,
                format!("vbmc-rs:sc:{system_id}:{ctrl_id}").as_bytes(),
            )
            .to_string(),
            durable_name_format: "UUID",
        }],
        sc_cache_summary: CtrlCacheSummary {
            total_cache_size_mib: 0,
            persistent_cache_size_mib: 0,
            status: Status::enabled_ok(),
        },
        sc_controller_rates: CtrlRates {
            consistency_check_rate_percent: 0,
            rebuild_rate_percent: 0,
            transformation_rate_percent: 0,
        },
        sc_pcie_interface: CtrlPcieInterface {
            max_pcie_type: "Gen4",
            max_lanes: 4,
            pcie_type: "Gen4",
            lanes_in_use: 4,
        },
        sc_location: super::types::RedfishLocation::new(
            format!("Storage {ctrl_id}"),
            "Embedded",
            0,
        ),
        assembly: ODataId::new(format!(
            "/redfish/v1/Systems/{system_id}/Storage/{ctrl_id}/Controllers/0/Assembly"
        )),
        sc_links: ScLinks {
            endpoints: Vec::new(),
            pcie_functions: Vec::new(),
            attached_volumes: Vec::new(),
        },
        status: Status::enabled_ok(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protocol_to_string() {
        assert_eq!(protocol_to_string(DiskProtocol::Virtio), "Virtio");
        assert_eq!(protocol_to_string(DiskProtocol::NVMe), "NVMe");
        assert_eq!(protocol_to_string(DiskProtocol::Sata), "SATA");
        assert_eq!(protocol_to_string(DiskProtocol::VhostUser), "VhostUser");
        assert_eq!(protocol_to_string(DiskProtocol::Unknown), "Unknown");
    }

    #[test]
    fn test_media_type_to_string() {
        assert_eq!(media_type_to_string(DiskMediaType::Ssd), "SSD");
        assert_eq!(media_type_to_string(DiskMediaType::Hdd), "HDD");
        assert_eq!(media_type_to_string(DiskMediaType::Virtual), "Virtual");
        assert_eq!(media_type_to_string(DiskMediaType::Unknown), "Unknown");
    }

    #[test]
    fn test_controller_id_for_protocol() {
        assert_eq!(controller_id_for_protocol(DiskProtocol::Virtio), "Virtio");
        assert_eq!(controller_id_for_protocol(DiskProtocol::NVMe), "NVMe");
    }

    fn make_disk(id: &str, protocol: DiskProtocol) -> DiskInfo {
        DiskInfo {
            id: id.to_string(),
            path: None,
            capacity_bytes: None,
            readonly: false,
            protocol,
            media_type: DiskMediaType::Virtual,
        }
    }

    #[test]
    fn test_group_disks_by_protocol_single() {
        let disks = vec![
            make_disk("vda", DiskProtocol::Virtio),
            make_disk("vdb", DiskProtocol::Virtio),
        ];
        let grouped = group_disks_by_protocol(&disks);
        assert_eq!(grouped.len(), 1);
        assert_eq!(grouped["Virtio"].len(), 2);
    }

    #[test]
    fn test_group_disks_by_protocol_multiple() {
        let disks = vec![
            make_disk("vda", DiskProtocol::Virtio),
            make_disk("nvme0", DiskProtocol::NVMe),
            make_disk("vdb", DiskProtocol::Virtio),
            make_disk("sda", DiskProtocol::Sata),
        ];
        let grouped = group_disks_by_protocol(&disks);
        assert_eq!(grouped.len(), 3);
        assert_eq!(grouped["Virtio"].len(), 2);
        assert_eq!(grouped["NVMe"].len(), 1);
        assert_eq!(grouped["SATA"].len(), 1);
    }

    #[test]
    fn test_group_disks_by_protocol_empty() {
        let disks: Vec<DiskInfo> = vec![];
        let grouped = group_disks_by_protocol(&disks);
        assert!(grouped.is_empty());
    }
}
