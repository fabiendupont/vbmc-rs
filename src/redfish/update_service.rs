use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use serde::Serialize;

use super::types::{Collection, ODataId, Status};
use crate::app_state::AppState;

#[derive(Debug, Serialize)]
pub struct UpdateServiceResource {
    #[serde(rename = "@odata.id")]
    pub odata_id: &'static str,
    #[serde(rename = "@odata.type")]
    pub odata_type: &'static str,
    #[serde(rename = "Id")]
    pub id: &'static str,
    #[serde(rename = "Name")]
    pub name: &'static str,
    #[serde(rename = "ServiceEnabled")]
    pub service_enabled: bool,
    #[serde(rename = "FirmwareInventory")]
    pub firmware_inventory: ODataId,
    #[serde(rename = "Status")]
    pub status: Status,
}

#[derive(Debug, Serialize)]
pub struct SoftwareInventoryResource {
    #[serde(rename = "@odata.id")]
    pub odata_id: String,
    #[serde(rename = "@odata.type")]
    pub odata_type: &'static str,
    #[serde(rename = "Id")]
    pub id: String,
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Version")]
    pub version: &'static str,
    #[serde(rename = "Updateable")]
    pub updateable: bool,
    #[serde(rename = "Status")]
    pub status: Status,
}

pub async fn get_update_service() -> Json<UpdateServiceResource> {
    Json(UpdateServiceResource {
        odata_id: "/redfish/v1/UpdateService",
        odata_type: "#UpdateService.v1_14_0.UpdateService",
        id: "UpdateService",
        name: "Update Service",
        service_enabled: true,
        firmware_inventory: ODataId::new("/redfish/v1/UpdateService/FirmwareInventory"),
        status: Status::enabled_ok(),
    })
}

pub async fn get_firmware_inventory() -> Json<Collection<ODataId>> {
    let members = vec![ODataId::new(
        "/redfish/v1/UpdateService/FirmwareInventory/vbmc-rs",
    )];

    Json(Collection::new(
        "/redfish/v1/UpdateService/FirmwareInventory",
        "#SoftwareInventoryCollection.SoftwareInventoryCollection",
        "Firmware Inventory",
        members,
    ))
}

pub async fn get_firmware_inventory_item(
    axum::extract::Path(item_id): axum::extract::Path<String>,
) -> Result<Json<SoftwareInventoryResource>, super::error::RedfishApiError> {
    if item_id == "vbmc-rs" {
        return Ok(Json(SoftwareInventoryResource {
            odata_id: "/redfish/v1/UpdateService/FirmwareInventory/vbmc-rs".to_string(),
            odata_type: "#SoftwareInventory.v1_10_0.SoftwareInventory",
            id: "vbmc-rs".to_string(),
            name: "vbmc-rs BMC Firmware".to_string(),
            version: env!("CARGO_PKG_VERSION"),
            updateable: false,
            status: Status::enabled_ok(),
        }));
    }

    Err(super::error::RedfishApiError::NotFound(format!(
        "FirmwareInventory '{item_id}' not found"
    )))
}
