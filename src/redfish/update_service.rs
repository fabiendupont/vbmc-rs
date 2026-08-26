use axum::Json;
use serde::Serialize;

use super::types::{Collection, ODataId, Status};
use crate::auth::AuthenticatedUser;

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
    #[serde(rename = "Description")]
    pub description: &'static str,
    #[serde(rename = "ServiceEnabled")]
    pub service_enabled: bool,
    #[serde(rename = "FirmwareInventory")]
    pub firmware_inventory: ODataId,
    #[serde(rename = "MaxImageSizeBytes")]
    pub max_image_size_bytes: u64,
    #[serde(rename = "MultipartHttpPushUri")]
    pub multipart_http_push_uri: &'static str,
    #[serde(rename = "VerifyRemoteServerCertificate")]
    pub verify_remote_server_certificate: bool,
    #[serde(rename = "VerifyRemoteServerSSHKey")]
    pub verify_remote_server_ssh_key: bool,
    #[serde(rename = "SupportedUpdateImageFormats")]
    pub supported_update_image_formats: Vec<&'static str>,
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
    #[serde(rename = "Description")]
    pub description: &'static str,
    #[serde(rename = "Version")]
    pub version: &'static str,
    #[serde(rename = "Updateable")]
    pub updateable: bool,
    #[serde(rename = "Manufacturer")]
    pub manufacturer: &'static str,
    #[serde(rename = "ReleaseDate")]
    pub release_date: &'static str,
    #[serde(rename = "SoftwareId")]
    pub software_id: &'static str,
    #[serde(rename = "LowestSupportedVersion")]
    pub lowest_supported_version: &'static str,
    #[serde(rename = "VersionScheme")]
    pub version_scheme: &'static str,
    #[serde(rename = "ReleaseType")]
    pub release_type: &'static str,
    #[serde(rename = "WriteProtected")]
    pub write_protected: bool,
    #[serde(rename = "RelatedItem")]
    pub related_item: Vec<ODataId>,
    #[serde(rename = "AssociatedPhysicalContext")]
    pub associated_physical_context: &'static str,
    #[serde(rename = "AdditionalVersions")]
    pub additional_versions: FwAdditionalVersions,
    #[serde(rename = "Status")]
    pub status: Status,
}

#[derive(Debug, Serialize)]
pub struct FwAdditionalVersions {
    #[serde(rename = "Bootloader", skip_serializing_if = "str::is_empty")]
    pub bootloader: &'static str,
    #[serde(rename = "Microcode", skip_serializing_if = "str::is_empty")]
    pub microcode: &'static str,
}

pub async fn get_update_service(_user: AuthenticatedUser) -> Json<UpdateServiceResource> {
    Json(UpdateServiceResource {
        odata_id: "/redfish/v1/UpdateService",
        odata_type: "#UpdateService.v1_14_0.UpdateService",
        id: "UpdateService",
        name: "Update Service",
        description: "Firmware update service",
        service_enabled: true,
        firmware_inventory: ODataId::new("/redfish/v1/UpdateService/FirmwareInventory"),
        max_image_size_bytes: 0,
        multipart_http_push_uri: "/redfish/v1/UpdateService/upload",
        verify_remote_server_certificate: false,
        verify_remote_server_ssh_key: false,
        supported_update_image_formats: Vec::new(),
        status: Status::enabled_ok(),
    })
}

pub async fn get_firmware_inventory(_user: AuthenticatedUser) -> Json<Collection<ODataId>> {
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
    _user: AuthenticatedUser,
    axum::extract::Path(item_id): axum::extract::Path<String>,
) -> Result<Json<SoftwareInventoryResource>, super::error::RedfishApiError> {
    if item_id == "vbmc-rs" {
        return Ok(Json(SoftwareInventoryResource {
            odata_id: "/redfish/v1/UpdateService/FirmwareInventory/vbmc-rs".to_string(),
            odata_type: "#SoftwareInventory.v1_10_0.SoftwareInventory",
            id: "vbmc-rs".to_string(),
            name: "vbmc-rs BMC Firmware".to_string(),
            description: "Software component",
            version: env!("CARGO_PKG_VERSION"),
            updateable: false,
            manufacturer: "vbmc-rs",
            release_date: "2026-01-01T00:00:00Z",
            software_id: "vbmc-rs",
            lowest_supported_version: "0.1.0",
            version_scheme: "SemVer",
            release_type: "Production",
            write_protected: true,
            related_item: vec![ODataId::new("/redfish/v1/Managers/vbmc")],
            associated_physical_context: "Chassis",
            additional_versions: FwAdditionalVersions {
                bootloader: "",
                microcode: "",
            },
            status: Status::enabled_ok(),
        }));
    }

    Err(super::error::RedfishApiError::NotFound(format!(
        "FirmwareInventory '{item_id}' not found"
    )))
}
