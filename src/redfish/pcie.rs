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
    _user: AuthenticatedUser,
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
        device_type: "SingleFunction",
        manufacturer: dev.vendor_id.clone(),
        pcie_functions: ODataId::new(format!(
            "/redfish/v1/Systems/{system_id}/PCIeDevices/{dev_id}/PCIeFunctions"
        )),
        status: Status::enabled_ok(),
    }))
}

pub async fn get_pcie_functions(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
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
    _user: AuthenticatedUser,
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
        .ok_or_else(|| RedfishApiError::NotFound(format!("PCIeFunction '{func_id}' not found")))?;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pcie_device_serialization() {
        let device = PCIeDeviceResource {
            odata_id: "/redfish/v1/Systems/vm1/PCIeDevices/dev0".to_string(),
            odata_type: "#PCIeDevice.v1_13_0.PCIeDevice",
            id: "dev0".to_string(),
            name: "Network Adapter".to_string(),
            description: "PCIe device",
            device_type: "SingleFunction",
            manufacturer: Some("Intel".to_string()),
            pcie_functions: ODataId::new(
                "/redfish/v1/Systems/vm1/PCIeDevices/dev0/PCIeFunctions".to_string(),
            ),
            status: Status::enabled_ok(),
        };

        let json = serde_json::to_value(&device).unwrap();
        assert_eq!(
            json["@odata.id"],
            "/redfish/v1/Systems/vm1/PCIeDevices/dev0"
        );
        assert_eq!(json["@odata.type"], "#PCIeDevice.v1_13_0.PCIeDevice");
        assert_eq!(json["Id"], "dev0");
        assert_eq!(json["Name"], "Network Adapter");
        assert_eq!(json["Description"], "PCIe device");
        assert_eq!(json["DeviceType"], "SingleFunction");
        assert_eq!(json["Manufacturer"], "Intel");
        assert_eq!(
            json["PCIeFunctions"]["@odata.id"],
            "/redfish/v1/Systems/vm1/PCIeDevices/dev0/PCIeFunctions"
        );
    }

    #[test]
    fn test_pcie_device_skip_serializing_if_none() {
        let device = PCIeDeviceResource {
            odata_id: "/redfish/v1/Systems/vm1/PCIeDevices/dev0".to_string(),
            odata_type: "#PCIeDevice.v1_13_0.PCIeDevice",
            id: "dev0".to_string(),
            name: "Unknown Device".to_string(),
            description: "PCIe device",
            device_type: "SingleFunction",
            manufacturer: None,
            pcie_functions: ODataId::new(
                "/redfish/v1/Systems/vm1/PCIeDevices/dev0/PCIeFunctions".to_string(),
            ),
            status: Status::enabled_ok(),
        };

        let json = serde_json::to_value(&device).unwrap();
        assert!(!json.as_object().unwrap().contains_key("Manufacturer"));
    }

    #[test]
    fn test_pcie_function_serialization() {
        let function = PCIeFunctionResource {
            odata_id: "/redfish/v1/Systems/vm1/PCIeDevices/dev0/PCIeFunctions/0".to_string(),
            odata_type: "#PCIeFunction.v1_5_1.PCIeFunction",
            id: "0".to_string(),
            name: "Function 0".to_string(),
            description: "PCIe function",
            function_id: 0,
            vendor_id: Some("8086".to_string()),
            device_id: Some("10d3".to_string()),
            class_code: Some("0x020000".to_string()),
            status: Status::enabled_ok(),
        };

        let json = serde_json::to_value(&function).unwrap();
        assert_eq!(
            json["@odata.id"],
            "/redfish/v1/Systems/vm1/PCIeDevices/dev0/PCIeFunctions/0"
        );
        assert_eq!(json["@odata.type"], "#PCIeFunction.v1_5_1.PCIeFunction");
        assert_eq!(json["Id"], "0");
        assert_eq!(json["FunctionId"], 0);
        assert_eq!(json["VendorId"], "8086");
        assert_eq!(json["DeviceId"], "10d3");
        assert_eq!(json["ClassCode"], "0x020000");
    }

    #[test]
    fn test_pcie_function_skip_serializing_if_none() {
        let function = PCIeFunctionResource {
            odata_id: "/redfish/v1/Systems/vm1/PCIeDevices/dev0/PCIeFunctions/0".to_string(),
            odata_type: "#PCIeFunction.v1_5_1.PCIeFunction",
            id: "0".to_string(),
            name: "Function 0".to_string(),
            description: "PCIe function",
            function_id: 0,
            vendor_id: None,
            device_id: None,
            class_code: None,
            status: Status::enabled_ok(),
        };

        let json = serde_json::to_value(&function).unwrap();
        assert!(!json.as_object().unwrap().contains_key("VendorId"));
        assert!(!json.as_object().unwrap().contains_key("DeviceId"));
        assert!(!json.as_object().unwrap().contains_key("ClassCode"));
    }

    #[test]
    fn test_pcie_function_all_fields_present() {
        let function = PCIeFunctionResource {
            odata_id: "/redfish/v1/Systems/vm1/PCIeDevices/dev0/PCIeFunctions/1".to_string(),
            odata_type: "#PCIeFunction.v1_5_1.PCIeFunction",
            id: "1".to_string(),
            name: "Function 1".to_string(),
            description: "PCIe function",
            function_id: 1,
            vendor_id: Some("10de".to_string()),
            device_id: Some("1234".to_string()),
            class_code: Some("0x030000".to_string()),
            status: Status::enabled_ok(),
        };

        let json = serde_json::to_value(&function).unwrap();
        let obj = json.as_object().unwrap();
        assert!(obj.contains_key("VendorId"));
        assert!(obj.contains_key("DeviceId"));
        assert!(obj.contains_key("ClassCode"));
        assert_eq!(json["VendorId"], "10de");
        assert_eq!(json["DeviceId"], "1234");
        assert_eq!(json["ClassCode"], "0x030000");
    }

    fn vm_with_pci_devices() -> crate::backend::types::VmInfo {
        use crate::backend::types::{PciDeviceInfo, PciFunctionInfo};

        let mut vm = crate::redfish::test_harness::running_vm();
        vm.pci_devices = vec![
            PciDeviceInfo {
                bdf: "0000:00:1f.0".to_string(),
                vendor_id: Some("8086".to_string()),
                device_id: Some("10d3".to_string()),
                class_code: Some("0x020000".to_string()),
                device_name: Some("Network Adapter".to_string()),
                is_passthrough: false,
                functions: vec![PciFunctionInfo {
                    function_id: 0,
                    class_code: Some("0x020000".to_string()),
                    device_id: Some("10d3".to_string()),
                    vendor_id: Some("8086".to_string()),
                }],
            },
            PciDeviceInfo {
                bdf: "0000:01:00.0".to_string(),
                vendor_id: Some("10de".to_string()),
                device_id: Some("1234".to_string()),
                class_code: Some("0x030000".to_string()),
                device_name: Some("GPU".to_string()),
                is_passthrough: false,
                functions: vec![
                    PciFunctionInfo {
                        function_id: 0,
                        class_code: Some("0x030000".to_string()),
                        device_id: Some("1234".to_string()),
                        vendor_id: Some("10de".to_string()),
                    },
                    PciFunctionInfo {
                        function_id: 1,
                        class_code: Some("0x040300".to_string()),
                        device_id: Some("1235".to_string()),
                        vendor_id: Some("10de".to_string()),
                    },
                ],
            },
        ];
        vm
    }

    #[tokio::test]
    async fn test_get_pcie_devices_collection() {
        use crate::redfish::test_harness::*;

        let mock = crate::backend::mock::MockBackend::new().with_vm("vm1", vm_with_pci_devices());
        let router = router(app_state(mock, systems_with("vm1")));
        let (status, json, _) = get(&router, "/redfish/v1/Systems/vm1/PCIeDevices").await;

        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(
            json["@odata.type"],
            "#PCIeDeviceCollection.PCIeDeviceCollection"
        );
        assert_eq!(json["Name"], "PCIe Device Collection");
        assert_eq!(json["Members@odata.count"], 2);
        assert_eq!(
            json["Members"][0]["@odata.id"],
            "/redfish/v1/Systems/vm1/PCIeDevices/dev0"
        );
        assert_eq!(
            json["Members"][1]["@odata.id"],
            "/redfish/v1/Systems/vm1/PCIeDevices/dev1"
        );
    }

    #[tokio::test]
    async fn test_get_pcie_devices_unknown_system() {
        use crate::redfish::test_harness::*;

        let mock = crate::backend::mock::MockBackend::new().with_vm("vm1", vm_with_pci_devices());
        let router = router(app_state(mock, systems_with("vm1")));
        let (status, _, _) = get(&router, "/redfish/v1/Systems/unknown/PCIeDevices").await;

        assert_eq!(status, axum::http::StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_get_pcie_device_valid() {
        use crate::redfish::test_harness::*;

        let mock = crate::backend::mock::MockBackend::new().with_vm("vm1", vm_with_pci_devices());
        let router = router(app_state(mock, systems_with("vm1")));
        let (status, json, _) = get(&router, "/redfish/v1/Systems/vm1/PCIeDevices/dev0").await;

        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(json["@odata.type"], "#PCIeDevice.v1_13_0.PCIeDevice");
        assert_eq!(json["Id"], "dev0");
        assert_eq!(json["Name"], "Network Adapter");
        assert_eq!(json["Description"], "PCIe device");
        assert_eq!(json["DeviceType"], "SingleFunction");
        assert_eq!(json["Manufacturer"], "8086");
        assert_eq!(
            json["PCIeFunctions"]["@odata.id"],
            "/redfish/v1/Systems/vm1/PCIeDevices/dev0/PCIeFunctions"
        );
        assert_eq!(json["Status"]["State"], "Enabled");
    }

    #[tokio::test]
    async fn test_get_pcie_device_unknown_system() {
        use crate::redfish::test_harness::*;

        let mock = crate::backend::mock::MockBackend::new().with_vm("vm1", vm_with_pci_devices());
        let router = router(app_state(mock, systems_with("vm1")));
        let (status, _, _) = get(&router, "/redfish/v1/Systems/unknown/PCIeDevices/dev0").await;

        assert_eq!(status, axum::http::StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_get_pcie_device_unknown_device() {
        use crate::redfish::test_harness::*;

        let mock = crate::backend::mock::MockBackend::new().with_vm("vm1", vm_with_pci_devices());
        let router = router(app_state(mock, systems_with("vm1")));
        let (status, _, _) = get(&router, "/redfish/v1/Systems/vm1/PCIeDevices/dev99").await;

        assert_eq!(status, axum::http::StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_get_pcie_functions_collection() {
        use crate::redfish::test_harness::*;

        let mock = crate::backend::mock::MockBackend::new().with_vm("vm1", vm_with_pci_devices());
        let router = router(app_state(mock, systems_with("vm1")));
        let (status, json, _) = get(
            &router,
            "/redfish/v1/Systems/vm1/PCIeDevices/dev1/PCIeFunctions",
        )
        .await;

        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(
            json["@odata.type"],
            "#PCIeFunctionCollection.PCIeFunctionCollection"
        );
        assert_eq!(json["Name"], "PCIe Function Collection");
        assert_eq!(json["Members@odata.count"], 2);
        assert_eq!(
            json["Members"][0]["@odata.id"],
            "/redfish/v1/Systems/vm1/PCIeDevices/dev1/PCIeFunctions/0"
        );
        assert_eq!(
            json["Members"][1]["@odata.id"],
            "/redfish/v1/Systems/vm1/PCIeDevices/dev1/PCIeFunctions/1"
        );
    }

    #[tokio::test]
    async fn test_get_pcie_functions_unknown_system() {
        use crate::redfish::test_harness::*;

        let mock = crate::backend::mock::MockBackend::new().with_vm("vm1", vm_with_pci_devices());
        let router = router(app_state(mock, systems_with("vm1")));
        let (status, _, _) = get(
            &router,
            "/redfish/v1/Systems/unknown/PCIeDevices/dev0/PCIeFunctions",
        )
        .await;

        assert_eq!(status, axum::http::StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_get_pcie_functions_unknown_device() {
        use crate::redfish::test_harness::*;

        let mock = crate::backend::mock::MockBackend::new().with_vm("vm1", vm_with_pci_devices());
        let router = router(app_state(mock, systems_with("vm1")));
        let (status, _, _) = get(
            &router,
            "/redfish/v1/Systems/vm1/PCIeDevices/dev99/PCIeFunctions",
        )
        .await;

        assert_eq!(status, axum::http::StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_get_pcie_function_valid() {
        use crate::redfish::test_harness::*;

        let mock = crate::backend::mock::MockBackend::new().with_vm("vm1", vm_with_pci_devices());
        let router = router(app_state(mock, systems_with("vm1")));
        let (status, json, _) = get(
            &router,
            "/redfish/v1/Systems/vm1/PCIeDevices/dev0/PCIeFunctions/0",
        )
        .await;

        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(json["@odata.type"], "#PCIeFunction.v1_5_1.PCIeFunction");
        assert_eq!(json["Id"], "0");
        assert_eq!(json["FunctionId"], 0);
        assert_eq!(json["VendorId"], "8086");
        assert_eq!(json["DeviceId"], "10d3");
        assert_eq!(json["ClassCode"], "0x020000");
        assert_eq!(json["Status"]["State"], "Enabled");
    }

    #[tokio::test]
    async fn test_get_pcie_function_unknown_function() {
        use crate::redfish::test_harness::*;

        let mock = crate::backend::mock::MockBackend::new().with_vm("vm1", vm_with_pci_devices());
        let router = router(app_state(mock, systems_with("vm1")));
        let (status, _, _) = get(
            &router,
            "/redfish/v1/Systems/vm1/PCIeDevices/dev0/PCIeFunctions/99",
        )
        .await;

        assert_eq!(status, axum::http::StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_get_pcie_function_multi_function_device() {
        use crate::redfish::test_harness::*;

        let mock = crate::backend::mock::MockBackend::new().with_vm("vm1", vm_with_pci_devices());
        let router = router(app_state(mock, systems_with("vm1")));
        let (status, json, _) = get(
            &router,
            "/redfish/v1/Systems/vm1/PCIeDevices/dev1/PCIeFunctions/1",
        )
        .await;

        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(json["@odata.type"], "#PCIeFunction.v1_5_1.PCIeFunction");
        assert_eq!(json["Id"], "1");
        assert_eq!(json["FunctionId"], 1);
        assert_eq!(json["VendorId"], "10de");
        assert_eq!(json["DeviceId"], "1235");
        assert_eq!(json["ClassCode"], "0x040300");
    }
}
