use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use serde::Serialize;

use super::types::ODataId;
use crate::app_state::AppState;

#[derive(Debug, Serialize)]
pub struct ServiceRoot {
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
    #[serde(rename = "RedfishVersion")]
    pub redfish_version: &'static str,
    #[serde(rename = "UUID")]
    pub uuid: String,
    #[serde(rename = "Systems")]
    pub systems: ODataId,
    #[serde(rename = "Managers")]
    pub managers: ODataId,
    #[serde(rename = "SessionService", skip_serializing_if = "Option::is_none")]
    pub session_service: Option<ODataId>,
    #[serde(rename = "AccountService", skip_serializing_if = "Option::is_none")]
    pub account_service: Option<ODataId>,
    #[serde(rename = "EventService", skip_serializing_if = "Option::is_none")]
    pub event_service: Option<ODataId>,
    #[serde(rename = "Tasks", skip_serializing_if = "Option::is_none")]
    pub task_service: Option<ODataId>,
    #[serde(rename = "TelemetryService", skip_serializing_if = "Option::is_none")]
    pub telemetry_service: Option<ODataId>,
    #[serde(rename = "CertificateService", skip_serializing_if = "Option::is_none")]
    pub certificate_service: Option<ODataId>,
    #[serde(rename = "Chassis", skip_serializing_if = "Option::is_none")]
    pub chassis: Option<ODataId>,
    #[serde(rename = "ComponentIntegrity", skip_serializing_if = "Option::is_none")]
    pub component_integrity: Option<ODataId>,
    #[serde(rename = "UpdateService", skip_serializing_if = "Option::is_none")]
    pub update_service: Option<ODataId>,
    #[serde(rename = "LicenseService", skip_serializing_if = "Option::is_none")]
    pub license_service: Option<ODataId>,
    #[serde(rename = "Registries", skip_serializing_if = "Option::is_none")]
    pub registries: Option<ODataId>,
    #[serde(rename = "ServiceIdentification")]
    pub service_identification: String,
    #[serde(rename = "Vendor")]
    pub vendor: String,
    #[serde(rename = "Product")]
    pub product: &'static str,
    #[serde(rename = "ProtocolFeaturesSupported")]
    pub protocol_features_supported: ProtocolFeatures,
    #[serde(rename = "Links")]
    pub links: ServiceRootLinks,
}

#[derive(Debug, Serialize)]
pub struct ProtocolFeatures {
    #[serde(rename = "ExpandQuery")]
    pub expand_query: ExpandQuery,
    #[serde(rename = "FilterQuery")]
    pub filter_query: bool,
    #[serde(rename = "SelectQuery")]
    pub select_query: bool,
    #[serde(rename = "OnlyMemberQuery")]
    pub only_member_query: bool,
    #[serde(rename = "ExcerptQuery")]
    pub excerpt_query: bool,
    #[serde(rename = "TopSkipQuery")]
    pub top_skip_query: bool,
    #[serde(rename = "MultipleHTTPRequests")]
    pub multiple_http_requests: bool,
    #[serde(rename = "DeepOperations")]
    pub deep_operations: DeepOperations,
}

#[derive(Debug, Serialize)]
pub struct DeepOperations {
    #[serde(rename = "DeepPATCH")]
    pub deep_patch: bool,
    #[serde(rename = "DeepPOST")]
    pub deep_post: bool,
    #[serde(rename = "MaxLevels")]
    pub max_levels: u32,
}

#[derive(Debug, Serialize)]
pub struct ExpandQuery {
    #[serde(rename = "ExpandAll")]
    pub expand_all: bool,
    #[serde(rename = "Levels")]
    pub levels: bool,
    #[serde(rename = "Links")]
    pub links: bool,
    #[serde(rename = "NoLinks")]
    pub no_links: bool,
    #[serde(rename = "MaxLevels")]
    pub max_levels: u32,
}

#[derive(Debug, Serialize)]
pub struct ServiceRootLinks {
    #[serde(rename = "Sessions")]
    pub sessions: ODataId,
    #[serde(rename = "ManagerProvidingService")]
    pub manager_providing_service: ODataId,
}

pub async fn get_service_root(State(state): State<Arc<AppState>>) -> Json<ServiceRoot> {
    Json(ServiceRoot {
        odata_id: "/redfish/v1",
        odata_type: "#ServiceRoot.v1_17_0.ServiceRoot",
        id: "RootService",
        name: "vbmc-rs Redfish Service",
        description: "vbmc-rs Redfish Service Root",
        redfish_version: "1.21.0",
        uuid: state.instance_uuid.clone(),
        systems: ODataId::new("/redfish/v1/Systems"),
        managers: ODataId::new("/redfish/v1/Managers"),
        session_service: Some(ODataId::new("/redfish/v1/SessionService")),
        account_service: Some(ODataId::new("/redfish/v1/AccountService")),
        event_service: Some(ODataId::new("/redfish/v1/EventService")),
        task_service: Some(ODataId::new("/redfish/v1/TaskService")),
        telemetry_service: Some(ODataId::new("/redfish/v1/TelemetryService")),
        certificate_service: Some(ODataId::new("/redfish/v1/CertificateService")),
        chassis: Some(ODataId::new("/redfish/v1/Chassis")),
        component_integrity: Some(ODataId::new("/redfish/v1/ComponentIntegrity")),
        update_service: Some(ODataId::new("/redfish/v1/UpdateService")),
        license_service: Some(ODataId::new("/redfish/v1/LicenseService")),
        registries: Some(ODataId::new("/redfish/v1/Registries")),
        service_identification: format!(
            "vbmc-rs-{}",
            state.instance_uuid.split('-').next().unwrap_or("0000")
        ),
        vendor: state.config.defaults.vendor.clone(),
        product: "Virtual BMC",
        protocol_features_supported: ProtocolFeatures {
            expand_query: ExpandQuery {
                expand_all: false,
                levels: false,
                links: false,
                no_links: false,
                max_levels: 1,
            },
            filter_query: false,
            select_query: false,
            only_member_query: false,
            excerpt_query: false,
            top_skip_query: false,
            multiple_http_requests: false,
            deep_operations: DeepOperations {
                deep_patch: false,
                deep_post: false,
                max_levels: 1,
            },
        },
        links: ServiceRootLinks {
            sessions: ODataId::new("/redfish/v1/SessionService/Sessions"),
            manager_providing_service: ODataId::new("/redfish/v1/Managers/vbmc"),
        },
    })
}

pub async fn get_redfish_root() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "v1": "/redfish/v1/"
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_service_root_serialization() {
        let root = ServiceRoot {
            odata_id: "/redfish/v1",
            odata_type: "#ServiceRoot.v1_17_0.ServiceRoot",
            id: "RootService",
            name: "vbmc-rs Redfish Service",
            description: "vbmc-rs Redfish Service Root",
            redfish_version: "1.21.0",
            uuid: "test-uuid-1234".to_string(),
            systems: ODataId::new("/redfish/v1/Systems"),
            managers: ODataId::new("/redfish/v1/Managers"),
            session_service: Some(ODataId::new("/redfish/v1/SessionService")),
            account_service: Some(ODataId::new("/redfish/v1/AccountService")),
            event_service: Some(ODataId::new("/redfish/v1/EventService")),
            task_service: Some(ODataId::new("/redfish/v1/TaskService")),
            telemetry_service: Some(ODataId::new("/redfish/v1/TelemetryService")),
            certificate_service: Some(ODataId::new("/redfish/v1/CertificateService")),
            chassis: Some(ODataId::new("/redfish/v1/Chassis")),
            component_integrity: Some(ODataId::new("/redfish/v1/ComponentIntegrity")),
            update_service: Some(ODataId::new("/redfish/v1/UpdateService")),
            license_service: Some(ODataId::new("/redfish/v1/LicenseService")),
            registries: Some(ODataId::new("/redfish/v1/Registries")),
            service_identification: "vbmc-rs-test".to_string(),
            vendor: "TestVendor".to_string(),
            product: "Virtual BMC",
            protocol_features_supported: ProtocolFeatures {
                expand_query: ExpandQuery {
                    expand_all: false,
                    levels: false,
                    links: false,
                    no_links: false,
                    max_levels: 1,
                },
                filter_query: false,
                select_query: false,
                only_member_query: false,
                excerpt_query: false,
                top_skip_query: false,
                multiple_http_requests: false,
                deep_operations: DeepOperations {
                    deep_patch: false,
                    deep_post: false,
                    max_levels: 1,
                },
            },
            links: ServiceRootLinks {
                sessions: ODataId::new("/redfish/v1/SessionService/Sessions"),
                manager_providing_service: ODataId::new("/redfish/v1/Managers/vbmc"),
            },
        };

        let value = serde_json::to_value(&root).unwrap();

        // Test renamed keys
        assert_eq!(value["@odata.id"], "/redfish/v1");
        assert_eq!(value["@odata.type"], "#ServiceRoot.v1_17_0.ServiceRoot");
        assert_eq!(value["Id"], "RootService");
        assert_eq!(value["Name"], "vbmc-rs Redfish Service");
        assert_eq!(value["RedfishVersion"], "1.21.0");
        assert_eq!(value["UUID"], "test-uuid-1234");
        assert_eq!(value["ServiceIdentification"], "vbmc-rs-test");
        assert_eq!(value["Vendor"], "TestVendor");
        assert_eq!(value["Product"], "Virtual BMC");

        // Test nested objects
        assert_eq!(value["Systems"]["@odata.id"], "/redfish/v1/Systems");
        assert_eq!(value["Managers"]["@odata.id"], "/redfish/v1/Managers");

        // Test optional fields present when Some
        assert_eq!(
            value["SessionService"]["@odata.id"],
            "/redfish/v1/SessionService"
        );
        assert_eq!(
            value["AccountService"]["@odata.id"],
            "/redfish/v1/AccountService"
        );

        // Test protocol features
        assert_eq!(value["ProtocolFeaturesSupported"]["FilterQuery"], false);
        assert_eq!(
            value["ProtocolFeaturesSupported"]["ExpandQuery"]["ExpandAll"],
            false
        );
        assert_eq!(
            value["ProtocolFeaturesSupported"]["ExpandQuery"]["MaxLevels"],
            1
        );
        assert_eq!(
            value["ProtocolFeaturesSupported"]["DeepOperations"]["DeepPATCH"],
            false
        );

        // Test links
        assert_eq!(
            value["Links"]["Sessions"]["@odata.id"],
            "/redfish/v1/SessionService/Sessions"
        );
    }

    #[test]
    fn test_service_root_optional_fields_none() {
        let root = ServiceRoot {
            odata_id: "/redfish/v1",
            odata_type: "#ServiceRoot.v1_17_0.ServiceRoot",
            id: "RootService",
            name: "Test",
            description: "Test",
            redfish_version: "1.21.0",
            uuid: "test-uuid".to_string(),
            systems: ODataId::new("/redfish/v1/Systems"),
            managers: ODataId::new("/redfish/v1/Managers"),
            session_service: None,
            account_service: None,
            event_service: None,
            task_service: None,
            telemetry_service: None,
            certificate_service: None,
            chassis: None,
            component_integrity: None,
            update_service: None,
            license_service: None,
            registries: None,
            service_identification: "test".to_string(),
            vendor: "Test".to_string(),
            product: "Virtual BMC",
            protocol_features_supported: ProtocolFeatures {
                expand_query: ExpandQuery {
                    expand_all: false,
                    levels: false,
                    links: false,
                    no_links: false,
                    max_levels: 1,
                },
                filter_query: false,
                select_query: false,
                only_member_query: false,
                excerpt_query: false,
                top_skip_query: false,
                multiple_http_requests: false,
                deep_operations: DeepOperations {
                    deep_patch: false,
                    deep_post: false,
                    max_levels: 1,
                },
            },
            links: ServiceRootLinks {
                sessions: ODataId::new("/redfish/v1/SessionService/Sessions"),
                manager_providing_service: ODataId::new("/redfish/v1/Managers/vbmc"),
            },
        };

        let value = serde_json::to_value(&root).unwrap();

        // Optional fields should be absent when None
        assert!(value.get("SessionService").is_none());
        assert!(value.get("AccountService").is_none());
        assert!(value.get("EventService").is_none());
        assert!(value.get("Tasks").is_none());
        assert!(value.get("Chassis").is_none());
    }

    #[test]
    fn test_protocol_features_serialization() {
        let features = ProtocolFeatures {
            expand_query: ExpandQuery {
                expand_all: true,
                levels: true,
                links: true,
                no_links: false,
                max_levels: 5,
            },
            filter_query: true,
            select_query: true,
            only_member_query: false,
            excerpt_query: false,
            top_skip_query: true,
            multiple_http_requests: true,
            deep_operations: DeepOperations {
                deep_patch: true,
                deep_post: false,
                max_levels: 3,
            },
        };

        let value = serde_json::to_value(&features).unwrap();

        assert_eq!(value["ExpandQuery"]["ExpandAll"], true);
        assert_eq!(value["ExpandQuery"]["Levels"], true);
        assert_eq!(value["ExpandQuery"]["Links"], true);
        assert_eq!(value["ExpandQuery"]["NoLinks"], false);
        assert_eq!(value["ExpandQuery"]["MaxLevels"], 5);
        assert_eq!(value["FilterQuery"], true);
        assert_eq!(value["SelectQuery"], true);
        assert_eq!(value["DeepOperations"]["DeepPATCH"], true);
        assert_eq!(value["DeepOperations"]["DeepPOST"], false);
        assert_eq!(value["DeepOperations"]["MaxLevels"], 3);
    }

    #[tokio::test]
    async fn test_get_redfish_root_response() {
        let response = get_redfish_root().await;
        let json = response.0;

        assert_eq!(json, json!({"v1": "/redfish/v1/"}));
    }
}
