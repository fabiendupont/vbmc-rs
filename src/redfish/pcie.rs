use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use serde::Serialize;

use super::error::RedfishApiError;
use super::types::{Collection, ODataId, Status};
use crate::app_state::AppState;
use crate::backend::VmmBackend;

#[derive(Debug, Serialize)]
pub struct PCIeDeviceResource {
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
    #[serde(rename = "DeviceType")]
    pub device_type: &'static str,
    #[serde(rename = "Manufacturer", skip_serializing_if = "Option::is_none")]
    pub manufacturer: Option<String>,
    #[serde(rename = "PCIeFunctions")]
    pub pcie_functions: ODataId,
    #[serde(rename = "Status")]
    pub status: Status,
}

#[derive(Debug, Serialize)]
pub struct PCIeFunctionResource {
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
    #[serde(rename = "FunctionId")]
    pub function_id: u8,
    #[serde(rename = "VendorId", skip_serializing_if = "Option::is_none")]
    pub vendor_id: Option<String>,
    #[serde(rename = "DeviceId", skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,
    #[serde(rename = "ClassCode", skip_serializing_if = "Option::is_none")]
    pub class_code: Option<String>,
    #[serde(rename = "Status")]
    pub status: Status,
}

pub async fn get_pcie_devices(
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
        for (i, _dev) in info.pci_devices.iter().enumerate() {
            members.push(ODataId::new(format!(
                "/redfish/v1/Systems/{system_id}/PCIeDevices/dev{i}"
            )));
        }
    }

    Ok(Json(Collection::new(
        format!("/redfish/v1/Systems/{system_id}/PCIeDevices"),
        "#PCIeDeviceCollection.PCIeDeviceCollection",
        "PCIe Device Collection",
        members,
    )))
}

pub async fn get_pcie_device(
    State(state): State<Arc<AppState>>,
    Path((system_id, dev_id)): Path<(String, String)>,
) -> Result<Json<PCIeDeviceResource>, RedfishApiError> {
    if !state.config.systems.contains_key(&system_id) {
        return Err(RedfishApiError::NotFound(format!(
            "System '{system_id}' not found"
        )));
    }

    let idx: usize = dev_id
        .strip_prefix("dev")
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| RedfishApiError::NotFound(format!("PCIeDevice '{dev_id}' not found")))?;

    let info = state
        .backend
        .vm_info(&system_id)
        .await
        .map_err(|e| RedfishApiError::InternalError(e.to_string()))?;

    let dev = info
        .pci_devices
        .get(idx)
        .ok_or_else(|| RedfishApiError::NotFound(format!("PCIeDevice '{dev_id}' not found")))?;

    Ok(Json(PCIeDeviceResource {
        odata_id: format!("/redfish/v1/Systems/{system_id}/PCIeDevices/{dev_id}"),
        odata_type: "#PCIeDevice.v1_13_0.PCIeDevice",
        id: dev_id.clone(),
        name: dev
            .device_name
            .clone()
            .unwrap_or_else(|| format!("PCIe Device {}", dev.bdf)),
        description: "PCIe device",
        device_type: if dev.is_passthrough {
            "SingleFunction"
        } else {
            "SingleFunction"
        },
        manufacturer: dev.vendor_id.clone(),
        pcie_functions: ODataId::new(format!(
            "/redfish/v1/Systems/{system_id}/PCIeDevices/{dev_id}/PCIeFunctions"
        )),
        status: Status::enabled_ok(),
    }))
}

pub async fn get_pcie_functions(
    State(state): State<Arc<AppState>>,
    Path((system_id, dev_id)): Path<(String, String)>,
) -> Result<Json<Collection<ODataId>>, RedfishApiError> {
    if !state.config.systems.contains_key(&system_id) {
        return Err(RedfishApiError::NotFound(format!(
            "System '{system_id}' not found"
        )));
    }

    let idx: usize = dev_id
        .strip_prefix("dev")
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| RedfishApiError::NotFound(format!("PCIeDevice '{dev_id}' not found")))?;

    let info = state
        .backend
        .vm_info(&system_id)
        .await
        .map_err(|e| RedfishApiError::InternalError(e.to_string()))?;

    let dev = info
        .pci_devices
        .get(idx)
        .ok_or_else(|| RedfishApiError::NotFound(format!("PCIeDevice '{dev_id}' not found")))?;

    let members: Vec<ODataId> = dev
        .functions
        .iter()
        .map(|f| {
            ODataId::new(format!(
                "/redfish/v1/Systems/{system_id}/PCIeDevices/{dev_id}/PCIeFunctions/{}",
                f.function_id
            ))
        })
        .collect();

    Ok(Json(Collection::new(
        format!("/redfish/v1/Systems/{system_id}/PCIeDevices/{dev_id}/PCIeFunctions"),
        "#PCIeFunctionCollection.PCIeFunctionCollection",
        "PCIe Function Collection",
        members,
    )))
}

pub async fn get_pcie_function(
    State(state): State<Arc<AppState>>,
    Path((system_id, dev_id, func_id)): Path<(String, String, String)>,
) -> Result<Json<PCIeFunctionResource>, RedfishApiError> {
    if !state.config.systems.contains_key(&system_id) {
        return Err(RedfishApiError::NotFound(format!(
            "System '{system_id}' not found"
        )));
    }

    let dev_idx: usize = dev_id
        .strip_prefix("dev")
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| RedfishApiError::NotFound(format!("PCIeDevice '{dev_id}' not found")))?;

    let func_idx: u8 = func_id
        .parse()
        .map_err(|_| RedfishApiError::NotFound(format!("PCIeFunction '{func_id}' not found")))?;

    let info = state
        .backend
        .vm_info(&system_id)
        .await
        .map_err(|e| RedfishApiError::InternalError(e.to_string()))?;

    let dev = info
        .pci_devices
        .get(dev_idx)
        .ok_or_else(|| RedfishApiError::NotFound(format!("PCIeDevice '{dev_id}' not found")))?;

    let func = dev
        .functions
        .iter()
        .find(|f| f.function_id == func_idx)
        .ok_or_else(|| {
            RedfishApiError::NotFound(format!("PCIeFunction '{func_id}' not found"))
        })?;

    Ok(Json(PCIeFunctionResource {
        odata_id: format!(
            "/redfish/v1/Systems/{system_id}/PCIeDevices/{dev_id}/PCIeFunctions/{func_id}"
        ),
        odata_type: "#PCIeFunction.v1_5_1.PCIeFunction",
        id: func_id,
        name: format!("Function {}", func.function_id),
        description: "PCIe function",
        function_id: func.function_id,
        vendor_id: func.vendor_id.clone(),
        device_id: func.device_id.clone(),
        class_code: func.class_code.clone(),
        status: Status::enabled_ok(),
    }))
}
