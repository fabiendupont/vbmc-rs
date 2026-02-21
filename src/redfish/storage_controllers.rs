use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use serde::Serialize;

use super::error::RedfishApiError;
use super::types::{Collection, ODataId, Status};
use crate::app_state::AppState;
use crate::backend::types::{DiskInfo, DiskMediaType, DiskProtocol};
use crate::backend::VmmBackend;

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
    #[serde(rename = "StorageControllers")]
    pub storage_controllers: Vec<StorageControllerEntry>,
    #[serde(rename = "Drives")]
    pub drives: Vec<ODataId>,
    #[serde(rename = "Volumes")]
    pub volumes: ODataId,
    #[serde(rename = "Status")]
    pub status: Status,
}

#[derive(Debug, Serialize)]
pub struct StorageControllerEntry {
    #[serde(rename = "MemberId")]
    pub member_id: String,
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "SupportedDeviceProtocols")]
    pub supported_device_protocols: Vec<String>,
    #[serde(rename = "Status")]
    pub status: Status,
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
    #[serde(rename = "CapacityBytes", skip_serializing_if = "Option::is_none")]
    pub capacity_bytes: Option<u64>,
    #[serde(rename = "Status")]
    pub status: Status,
}

fn protocol_to_string(p: DiskProtocol) -> String {
    match p {
        DiskProtocol::Virtio => "Virtio".to_string(),
        DiskProtocol::NVMe => "NVMe".to_string(),
        DiskProtocol::SATA => "SATA".to_string(),
        DiskProtocol::VhostUser => "VhostUser".to_string(),
        DiskProtocol::Unknown => "Unknown".to_string(),
    }
}

fn media_type_to_string(m: DiskMediaType) -> String {
    match m {
        DiskMediaType::SSD => "SSD".to_string(),
        DiskMediaType::HDD => "HDD".to_string(),
        DiskMediaType::Virtual => "Virtual".to_string(),
        DiskMediaType::Unknown => "Unknown".to_string(),
    }
}

fn controller_id_for_protocol(p: DiskProtocol) -> String {
    match p {
        DiskProtocol::Virtio => "Virtio".to_string(),
        DiskProtocol::NVMe => "NVMe".to_string(),
        DiskProtocol::SATA => "SATA".to_string(),
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
        storage_controllers: vec![StorageControllerEntry {
            member_id: "0".to_string(),
            name: format!("{ctrl_id} Controller"),
            supported_device_protocols: vec![ctrl_id.clone()],
            status: Status::enabled_ok(),
        }],
        drives,
        volumes: ODataId::new(format!(
            "/redfish/v1/Systems/{system_id}/Storage/{ctrl_id}/Volumes"
        )),
        status: Status::enabled_ok(),
    }))
}

pub async fn get_drive(
    State(state): State<Arc<AppState>>,
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
        odata_id: format!(
            "/redfish/v1/Systems/{system_id}/Storage/{ctrl_id}/Drives/{drive_id}"
        ),
        odata_type: "#Drive.v1_18_0.Drive",
        id: drive_id,
        name: disk.id.clone(),
        capacity_bytes: disk.capacity_bytes,
        media_type: media_type_to_string(disk.media_type),
        protocol: protocol_to_string(disk.protocol),
        status: Status::enabled_ok(),
    }))
}

pub async fn get_volumes(
    State(state): State<Arc<AppState>>,
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
        odata_id: format!(
            "/redfish/v1/Systems/{system_id}/Storage/{ctrl_id}/Volumes/{vol_id}"
        ),
        odata_type: "#Volume.v1_10_0.Volume",
        id: vol_id,
        name: disk.id.clone(),
        capacity_bytes: disk.capacity_bytes,
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
        assert_eq!(protocol_to_string(DiskProtocol::SATA), "SATA");
        assert_eq!(protocol_to_string(DiskProtocol::VhostUser), "VhostUser");
        assert_eq!(protocol_to_string(DiskProtocol::Unknown), "Unknown");
    }

    #[test]
    fn test_media_type_to_string() {
        assert_eq!(media_type_to_string(DiskMediaType::SSD), "SSD");
        assert_eq!(media_type_to_string(DiskMediaType::HDD), "HDD");
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
            make_disk("sda", DiskProtocol::SATA),
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
