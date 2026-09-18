use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use serde::Serialize;

use super::error::RedfishApiError;
use super::types::{Collection, ODataId, Status};
use crate::app_state::AppState;
use crate::auth::AuthenticatedUser;

#[derive(Debug, Serialize)]
pub struct NetworkInterfaceResource {
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
    #[serde(rename = "Links")]
    pub links: NetworkInterfaceLinks,
    #[serde(rename = "Status")]
    pub status: Status,
}

#[derive(Debug, Serialize)]
pub struct NetworkInterfaceLinks {
    #[serde(rename = "NetworkAdapter")]
    pub network_adapter: ODataId,
}

pub async fn get_network_interfaces(
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
        "/redfish/v1/Systems/{system_id}/NetworkInterfaces/NIC0"
    ))];

    Ok(Json(Collection::new(
        format!("/redfish/v1/Systems/{system_id}/NetworkInterfaces"),
        "#NetworkInterfaceCollection.NetworkInterfaceCollection",
        "Network Interface Collection",
        members,
    )))
}

pub async fn get_network_interface(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
    Path((system_id, nic_id)): Path<(String, String)>,
) -> Result<Json<NetworkInterfaceResource>, RedfishApiError> {
    if !state.config.systems.contains_key(&system_id) {
        return Err(RedfishApiError::NotFound(format!(
            "System '{system_id}' not found"
        )));
    }
    if nic_id != "NIC0" {
        return Err(RedfishApiError::NotFound(format!(
            "NetworkInterface '{nic_id}' not found"
        )));
    }

    Ok(Json(NetworkInterfaceResource {
        odata_id: format!("/redfish/v1/Systems/{system_id}/NetworkInterfaces/{nic_id}"),
        odata_type: "#NetworkInterface.v1_2_0.NetworkInterface",
        id: "NIC0",
        name: "Network Interface",
        description: "System network interface",
        links: NetworkInterfaceLinks {
            network_adapter: ODataId::new(format!(
                "/redfish/v1/Chassis/{}/NetworkAdapters/{system_id}_NIC0",
                state.chassis_id
            )),
        },
        status: Status::enabled_ok(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_interface_links_serialization() {
        let links = NetworkInterfaceLinks {
            network_adapter: ODataId::new("/redfish/v1/Chassis/ch1/NetworkAdapters/vm1_NIC0"),
        };

        let json = serde_json::to_value(&links).unwrap();
        assert_eq!(
            json["NetworkAdapter"]["@odata.id"],
            "/redfish/v1/Chassis/ch1/NetworkAdapters/vm1_NIC0"
        );
    }

    #[test]
    fn test_network_interface_resource_serialization() {
        let resource = NetworkInterfaceResource {
            odata_id: "/redfish/v1/Systems/vm1/NetworkInterfaces/NIC0".to_string(),
            odata_type: "#NetworkInterface.v1_2_0.NetworkInterface",
            id: "NIC0",
            name: "Network Interface",
            description: "System network interface",
            links: NetworkInterfaceLinks {
                network_adapter: ODataId::new("/redfish/v1/Chassis/ch1/NetworkAdapters/vm1_NIC0"),
            },
            status: Status::enabled_ok(),
        };

        let json = serde_json::to_value(&resource).unwrap();
        assert_eq!(
            json["@odata.id"],
            "/redfish/v1/Systems/vm1/NetworkInterfaces/NIC0"
        );
        assert_eq!(
            json["@odata.type"],
            "#NetworkInterface.v1_2_0.NetworkInterface"
        );
        assert_eq!(json["Id"], "NIC0");
        assert_eq!(json["Name"], "Network Interface");
        assert_eq!(json["Description"], "System network interface");
        assert_eq!(
            json["Links"]["NetworkAdapter"]["@odata.id"],
            "/redfish/v1/Chassis/ch1/NetworkAdapters/vm1_NIC0"
        );
    }

    #[tokio::test]
    async fn test_get_network_interfaces_collection() {
        use crate::redfish::test_harness::*;

        let router = router_with_systems(systems_with("vm1"));
        let (status, json, _) = get(&router, "/redfish/v1/Systems/vm1/NetworkInterfaces").await;

        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(
            json["@odata.type"],
            "#NetworkInterfaceCollection.NetworkInterfaceCollection"
        );
        assert_eq!(json["Name"], "Network Interface Collection");
        assert_eq!(json["Members@odata.count"], 1);
        assert_eq!(
            json["Members"][0]["@odata.id"],
            "/redfish/v1/Systems/vm1/NetworkInterfaces/NIC0"
        );
    }

    #[tokio::test]
    async fn test_get_network_interfaces_unknown_system() {
        use crate::redfish::test_harness::*;

        let router = router_with_systems(systems_with("vm1"));
        let (status, _, _) = get(&router, "/redfish/v1/Systems/unknown/NetworkInterfaces").await;

        assert_eq!(status, axum::http::StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_get_network_interface_valid() {
        use crate::redfish::test_harness::*;

        let router = router_with_systems(systems_with("vm1"));
        let (status, json, _) =
            get(&router, "/redfish/v1/Systems/vm1/NetworkInterfaces/NIC0").await;

        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(
            json["@odata.type"],
            "#NetworkInterface.v1_2_0.NetworkInterface"
        );
        assert_eq!(json["Id"], "NIC0");
        assert_eq!(json["Name"], "Network Interface");
        assert_eq!(json["Description"], "System network interface");
        assert!(json["Links"]["NetworkAdapter"]["@odata.id"].is_string());
        assert_eq!(json["Status"]["State"], "Enabled");
        assert_eq!(json["Status"]["Health"], "OK");
    }

    #[tokio::test]
    async fn test_get_network_interface_unknown_system() {
        use crate::redfish::test_harness::*;

        let router = router_with_systems(systems_with("vm1"));
        let (status, _, _) = get(
            &router,
            "/redfish/v1/Systems/unknown/NetworkInterfaces/NIC0",
        )
        .await;

        assert_eq!(status, axum::http::StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_get_network_interface_unknown_nic() {
        use crate::redfish::test_harness::*;

        let router = router_with_systems(systems_with("vm1"));
        let (status, _, _) = get(&router, "/redfish/v1/Systems/vm1/NetworkInterfaces/NIC99").await;

        assert_eq!(status, axum::http::StatusCode::NOT_FOUND);
    }
}
