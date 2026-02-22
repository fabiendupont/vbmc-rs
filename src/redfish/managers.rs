use axum::extract::Path;
use axum::Json;
use serde::Serialize;

use super::error::RedfishApiError;
use super::types::{Collection, ODataId, Status};

const MANAGER_ID: &str = "vbmc";

#[derive(Debug, Serialize)]
pub struct Manager {
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
    #[serde(rename = "ManagerType")]
    pub manager_type: &'static str,
    #[serde(rename = "FirmwareVersion")]
    pub firmware_version: &'static str,
    #[serde(rename = "Status")]
    pub status: Status,
    #[serde(rename = "LogServices", skip_serializing_if = "Option::is_none")]
    pub log_services: Option<ODataId>,
}

pub async fn get_managers() -> Json<Collection<ODataId>> {
    let members = vec![ODataId::new(format!("/redfish/v1/Managers/{MANAGER_ID}"))];
    Json(Collection::new(
        "/redfish/v1/Managers",
        "#ManagerCollection.ManagerCollection",
        "Manager Collection",
        members,
    ))
}

pub async fn get_manager(
    Path(manager_id): Path<String>,
) -> Result<Json<Manager>, RedfishApiError> {
    if manager_id != MANAGER_ID {
        return Err(RedfishApiError::NotFound(format!(
            "Manager '{manager_id}' not found"
        )));
    }

    Ok(Json(Manager {
        odata_id: format!("/redfish/v1/Managers/{MANAGER_ID}"),
        odata_type: "#Manager.v1_19_0.Manager",
        id: MANAGER_ID,
        name: "vbmc-rs Virtual BMC",
        description: "vbmc-rs Virtual Baseboard Management Controller",
        manager_type: "BMC",
        firmware_version: env!("CARGO_PKG_VERSION"),
        status: Status::enabled_ok(),
        log_services: Some(ODataId::new("/redfish/v1/Managers/vbmc/LogServices")),
    }))
}
