use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use serde::Serialize;

use super::error::RedfishApiError;
use super::types::{Collection, ODataId, Status};
use crate::app_state::AppState;
use crate::auth::AuthenticatedUser;
use crate::backend::VmmBackend;

#[derive(Debug, Serialize)]
pub struct SimpleStorage {
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
    #[serde(rename = "Devices")]
    pub devices: Vec<StorageDevice>,
    #[serde(rename = "Links")]
    pub links: SimpleStorageLinks,
    #[serde(rename = "Status")]
    pub status: Status,
}

#[derive(Debug, Serialize)]
pub struct SimpleStorageLinks {
    #[serde(rename = "Chassis")]
    pub chassis: ODataId,
    #[serde(rename = "Storage")]
    pub storage: ODataId,
}

#[derive(Debug, Serialize)]
pub struct StorageDevice {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "CapacityBytes", skip_serializing_if = "Option::is_none")]
    pub capacity_bytes: Option<u64>,
    #[serde(rename = "Status")]
    pub status: Status,
}

pub async fn get_simple_storage_collection(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
    Path(system_id): Path<String>,
) -> Result<Json<Collection<ODataId>>, RedfishApiError> {
    if !state.config.systems.contains_key(&system_id) {
        return Err(RedfishApiError::NotFound(format!(
            "System '{system_id}' not found"
        )));
    }

    let members = vec![ODataId::new(format!(
        "/redfish/v1/Systems/{system_id}/SimpleStorage/1"
    ))];

    Ok(Json(Collection::new(
        format!("/redfish/v1/Systems/{system_id}/SimpleStorage"),
        "#SimpleStorageCollection.SimpleStorageCollection",
        "Simple Storage Collection",
        members,
    )))
}

pub async fn get_simple_storage(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
    Path((system_id, storage_id)): Path<(String, String)>,
) -> Result<Json<SimpleStorage>, RedfishApiError> {
    if !state.config.systems.contains_key(&system_id) {
        return Err(RedfishApiError::NotFound(format!(
            "System '{system_id}' not found"
        )));
    }
    if storage_id != "1" {
        return Err(RedfishApiError::NotFound(format!(
            "SimpleStorage '{storage_id}' not found"
        )));
    }

    let mut devices = Vec::new();

    if let Ok(info) = state.backend.vm_info(&system_id).await {
        for disk in &info.disks {
            devices.push(StorageDevice {
                name: disk.id.clone(),
                capacity_bytes: disk.capacity_bytes,
                status: Status::enabled_ok(),
            });
        }
    }

    Ok(Json(SimpleStorage {
        odata_id: format!("/redfish/v1/Systems/{system_id}/SimpleStorage/{storage_id}"),
        odata_type: "#SimpleStorage.v1_3_0.SimpleStorage",
        id: storage_id,
        name: "Simple Storage Controller".to_string(),
        description: "Simple storage view",
        devices,
        links: SimpleStorageLinks {
            chassis: ODataId::new(format!("/redfish/v1/Chassis/{}", state.chassis_id)),
            storage: ODataId::new(format!("/redfish/v1/Systems/{system_id}/Storage/Virtio")),
        },
        status: Status::enabled_ok(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_storage_serialization() {
        let storage = SimpleStorage {
            odata_id: "/redfish/v1/Systems/vm1/SimpleStorage/1".to_string(),
            odata_type: "#SimpleStorage.v1_3_0.SimpleStorage",
            id: "1".to_string(),
            name: "Simple Storage Controller".to_string(),
            description: "Simple storage view",
            devices: vec![
                StorageDevice {
                    name: "vda".to_string(),
                    capacity_bytes: Some(10_737_418_240),
                    status: Status::enabled_ok(),
                },
                StorageDevice {
                    name: "vdb".to_string(),
                    capacity_bytes: Some(5_368_709_120),
                    status: Status::enabled_ok(),
                },
            ],
            links: SimpleStorageLinks {
                chassis: ODataId::new("/redfish/v1/Chassis/1".to_string()),
                storage: ODataId::new("/redfish/v1/Systems/vm1/Storage/Virtio".to_string()),
            },
            status: Status::enabled_ok(),
        };

        let json = serde_json::to_value(&storage).unwrap();
        assert_eq!(json["@odata.id"], "/redfish/v1/Systems/vm1/SimpleStorage/1");
        assert_eq!(json["@odata.type"], "#SimpleStorage.v1_3_0.SimpleStorage");
        assert_eq!(json["Id"], "1");
        assert_eq!(json["Name"], "Simple Storage Controller");
        assert_eq!(json["Description"], "Simple storage view");
        assert_eq!(json["Devices"].as_array().unwrap().len(), 2);
        assert_eq!(json["Devices"][0]["Name"], "vda");
        assert_eq!(json["Devices"][0]["CapacityBytes"], 10_737_418_240u64);
    }

    #[test]
    fn test_simple_storage_links_serialization() {
        let links = SimpleStorageLinks {
            chassis: ODataId::new("/redfish/v1/Chassis/1".to_string()),
            storage: ODataId::new("/redfish/v1/Systems/vm1/Storage/Virtio".to_string()),
        };

        let json = serde_json::to_value(&links).unwrap();
        assert_eq!(json["Chassis"]["@odata.id"], "/redfish/v1/Chassis/1");
        assert_eq!(
            json["Storage"]["@odata.id"],
            "/redfish/v1/Systems/vm1/Storage/Virtio"
        );
    }

    #[test]
    fn test_storage_device_serialization() {
        let device = StorageDevice {
            name: "nvme0n1".to_string(),
            capacity_bytes: Some(512_110_190_592),
            status: Status::enabled_ok(),
        };

        let json = serde_json::to_value(&device).unwrap();
        assert_eq!(json["Name"], "nvme0n1");
        assert_eq!(json["CapacityBytes"], 512_110_190_592u64);
    }

    #[test]
    fn test_storage_device_skip_serializing_if_none() {
        let device = StorageDevice {
            name: "vdc".to_string(),
            capacity_bytes: None,
            status: Status::enabled_ok(),
        };

        let json = serde_json::to_value(&device).unwrap();
        assert_eq!(json["Name"], "vdc");
        assert!(!json.as_object().unwrap().contains_key("CapacityBytes"));
    }

    #[test]
    fn test_storage_device_with_capacity() {
        let device = StorageDevice {
            name: "sda".to_string(),
            capacity_bytes: Some(1_000_000_000_000),
            status: Status::enabled_ok(),
        };

        let json = serde_json::to_value(&device).unwrap();
        let obj = json.as_object().unwrap();
        assert!(obj.contains_key("CapacityBytes"));
        assert_eq!(json["CapacityBytes"], 1_000_000_000_000u64);
    }

    #[test]
    fn test_simple_storage_empty_devices() {
        let storage = SimpleStorage {
            odata_id: "/redfish/v1/Systems/vm1/SimpleStorage/1".to_string(),
            odata_type: "#SimpleStorage.v1_3_0.SimpleStorage",
            id: "1".to_string(),
            name: "Simple Storage Controller".to_string(),
            description: "Simple storage view",
            devices: vec![],
            links: SimpleStorageLinks {
                chassis: ODataId::new("/redfish/v1/Chassis/1".to_string()),
                storage: ODataId::new("/redfish/v1/Systems/vm1/Storage/Virtio".to_string()),
            },
            status: Status::enabled_ok(),
        };

        let json = serde_json::to_value(&storage).unwrap();
        assert!(json["Devices"].as_array().unwrap().is_empty());
    }

    #[test]
    fn test_storage_device_capacity_none_vs_some() {
        let device_with_capacity = StorageDevice {
            name: "disk1".to_string(),
            capacity_bytes: Some(100_000_000),
            status: Status::enabled_ok(),
        };

        let device_without_capacity = StorageDevice {
            name: "disk2".to_string(),
            capacity_bytes: None,
            status: Status::enabled_ok(),
        };

        let json_with = serde_json::to_value(&device_with_capacity).unwrap();
        let json_without = serde_json::to_value(&device_without_capacity).unwrap();

        assert!(json_with.as_object().unwrap().contains_key("CapacityBytes"));
        assert!(
            !json_without
                .as_object()
                .unwrap()
                .contains_key("CapacityBytes")
        );
    }
}
