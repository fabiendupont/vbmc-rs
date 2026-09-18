use axum::Json;
use axum::extract::Path;
use serde::Serialize;

use super::error::RedfishApiError;
use super::types::{Collection, ODataId};
use crate::auth::AuthenticatedUser;

#[derive(Debug, Serialize)]
pub struct TelemetryServiceResource {
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
    #[serde(rename = "MetricDefinitions")]
    pub metric_definitions: ODataId,
    #[serde(rename = "MetricReports")]
    pub metric_reports: ODataId,
    #[serde(rename = "MaxReports")]
    pub max_reports: u32,
    #[serde(rename = "MinCollectionInterval")]
    pub min_collection_interval: &'static str,
    #[serde(rename = "SupportedCollectionFunctions")]
    pub supported_collection_functions: Vec<&'static str>,
    #[serde(rename = "Status")]
    pub status: super::types::Status,
}

pub async fn get_telemetry_service(_user: AuthenticatedUser) -> Json<TelemetryServiceResource> {
    Json(TelemetryServiceResource {
        odata_id: "/redfish/v1/TelemetryService",
        odata_type: "#TelemetryService.v1_3_0.TelemetryService",
        id: "TelemetryService",
        name: "Telemetry Service",
        description: "Telemetry and metrics service",
        service_enabled: true,
        metric_definitions: ODataId::new("/redfish/v1/TelemetryService/MetricDefinitions"),
        metric_reports: ODataId::new("/redfish/v1/TelemetryService/MetricReports"),
        max_reports: 10,
        min_collection_interval: "PT10S",
        supported_collection_functions: vec!["Average", "Maximum", "Minimum"],
        status: super::types::Status::enabled_ok(),
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct MetricDefinition {
    #[serde(rename = "@odata.id")]
    pub odata_id: String,
    #[serde(rename = "@odata.type")]
    pub odata_type: &'static str,
    #[serde(rename = "Id")]
    pub id: String,
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "MetricType")]
    pub metric_type: String,
    #[serde(rename = "Units", skip_serializing_if = "Option::is_none")]
    pub units: Option<String>,
}

fn built_in_metric_definitions() -> Vec<MetricDefinition> {
    vec![
        MetricDefinition {
            odata_id: "/redfish/v1/TelemetryService/MetricDefinitions/HttpRequestsTotal"
                .to_string(),
            odata_type: "#MetricDefinition.v1_3_0.MetricDefinition",
            id: "HttpRequestsTotal".to_string(),
            name: "HTTP Requests Total".to_string(),
            metric_type: "Counter".to_string(),
            units: Some("{requests}".to_string()),
        },
        MetricDefinition {
            odata_id: "/redfish/v1/TelemetryService/MetricDefinitions/HttpRequestDuration"
                .to_string(),
            odata_type: "#MetricDefinition.v1_3_0.MetricDefinition",
            id: "HttpRequestDuration".to_string(),
            name: "HTTP Request Duration".to_string(),
            metric_type: "Gauge".to_string(),
            units: Some("s".to_string()),
        },
        MetricDefinition {
            odata_id: "/redfish/v1/TelemetryService/MetricDefinitions/VmPowerState".to_string(),
            odata_type: "#MetricDefinition.v1_3_0.MetricDefinition",
            id: "VmPowerState".to_string(),
            name: "VM Power State".to_string(),
            metric_type: "Discrete".to_string(),
            units: None,
        },
    ]
}

pub async fn get_metric_definitions(_user: AuthenticatedUser) -> Json<Collection<ODataId>> {
    let members: Vec<ODataId> = built_in_metric_definitions()
        .iter()
        .map(|d| ODataId::new(&d.odata_id))
        .collect();

    Json(Collection::new(
        "/redfish/v1/TelemetryService/MetricDefinitions",
        "#MetricDefinitionCollection.MetricDefinitionCollection",
        "Metric Definitions",
        members,
    ))
}

pub async fn get_metric_definition(
    _user: AuthenticatedUser,
    Path(def_id): Path<String>,
) -> Result<Json<MetricDefinition>, RedfishApiError> {
    built_in_metric_definitions()
        .into_iter()
        .find(|d| d.id == def_id)
        .map(Json)
        .ok_or_else(|| RedfishApiError::NotFound(format!("Metric definition '{def_id}' not found")))
}

pub async fn get_metric_reports(_user: AuthenticatedUser) -> Json<Collection<ODataId>> {
    Json(Collection::new(
        "/redfish/v1/TelemetryService/MetricReports",
        "#MetricReportCollection.MetricReportCollection",
        "Metric Reports",
        Vec::<ODataId>::new(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_telemetry_service_resource_serialization() {
        let resource = TelemetryServiceResource {
            odata_id: "/redfish/v1/TelemetryService",
            odata_type: "#TelemetryService.v1_3_0.TelemetryService",
            id: "TelemetryService",
            name: "Telemetry Service",
            description: "Telemetry and metrics service",
            service_enabled: true,
            metric_definitions: ODataId::new("/redfish/v1/TelemetryService/MetricDefinitions"),
            metric_reports: ODataId::new("/redfish/v1/TelemetryService/MetricReports"),
            max_reports: 10,
            min_collection_interval: "PT10S",
            supported_collection_functions: vec!["Average", "Maximum", "Minimum"],
            status: super::super::types::Status::enabled_ok(),
        };

        let value = serde_json::to_value(&resource).unwrap();

        assert_eq!(value["@odata.id"], "/redfish/v1/TelemetryService");
        assert_eq!(
            value["@odata.type"],
            "#TelemetryService.v1_3_0.TelemetryService"
        );
        assert_eq!(value["Id"], "TelemetryService");
        assert_eq!(value["Name"], "Telemetry Service");
        assert_eq!(value["Description"], "Telemetry and metrics service");
        assert_eq!(value["ServiceEnabled"], true);
        assert_eq!(
            value["MetricDefinitions"]["@odata.id"],
            "/redfish/v1/TelemetryService/MetricDefinitions"
        );
        assert_eq!(
            value["MetricReports"]["@odata.id"],
            "/redfish/v1/TelemetryService/MetricReports"
        );
        assert_eq!(value["MaxReports"], 10);
        assert_eq!(value["MinCollectionInterval"], "PT10S");
        assert!(value["SupportedCollectionFunctions"].is_array());
        assert!(value["Status"].is_object());
    }

    #[test]
    fn test_metric_definition_serialization() {
        let metric = MetricDefinition {
            odata_id: "/redfish/v1/TelemetryService/MetricDefinitions/HttpRequestsTotal"
                .to_string(),
            odata_type: "#MetricDefinition.v1_3_0.MetricDefinition",
            id: "HttpRequestsTotal".to_string(),
            name: "HTTP Requests Total".to_string(),
            metric_type: "Counter".to_string(),
            units: Some("{requests}".to_string()),
        };

        let value = serde_json::to_value(&metric).unwrap();

        assert_eq!(
            value["@odata.id"],
            "/redfish/v1/TelemetryService/MetricDefinitions/HttpRequestsTotal"
        );
        assert_eq!(
            value["@odata.type"],
            "#MetricDefinition.v1_3_0.MetricDefinition"
        );
        assert_eq!(value["Id"], "HttpRequestsTotal");
        assert_eq!(value["Name"], "HTTP Requests Total");
        assert_eq!(value["MetricType"], "Counter");
        assert_eq!(value["Units"], "{requests}");
    }

    #[test]
    fn test_metric_definition_skip_none_units() {
        let metric = MetricDefinition {
            odata_id: "/redfish/v1/TelemetryService/MetricDefinitions/VmPowerState".to_string(),
            odata_type: "#MetricDefinition.v1_3_0.MetricDefinition",
            id: "VmPowerState".to_string(),
            name: "VM Power State".to_string(),
            metric_type: "Discrete".to_string(),
            units: None,
        };

        let value = serde_json::to_value(&metric).unwrap();
        assert!(!value.as_object().unwrap().contains_key("Units"));
    }

    #[test]
    fn test_metric_definition_with_units() {
        let metric = MetricDefinition {
            odata_id: "/test".to_string(),
            odata_type: "#MetricDefinition.v1_3_0.MetricDefinition",
            id: "test".to_string(),
            name: "Test".to_string(),
            metric_type: "Gauge".to_string(),
            units: Some("s".to_string()),
        };

        let value = serde_json::to_value(&metric).unwrap();
        assert_eq!(value["Units"], "s");
    }

    #[test]
    fn test_built_in_metric_definitions() {
        let defs = built_in_metric_definitions();
        assert_eq!(defs.len(), 3);

        let http_total = defs.iter().find(|d| d.id == "HttpRequestsTotal").unwrap();
        assert_eq!(http_total.name, "HTTP Requests Total");
        assert_eq!(http_total.metric_type, "Counter");
        assert_eq!(http_total.units, Some("{requests}".to_string()));

        let http_duration = defs.iter().find(|d| d.id == "HttpRequestDuration").unwrap();
        assert_eq!(http_duration.name, "HTTP Request Duration");
        assert_eq!(http_duration.metric_type, "Gauge");
        assert_eq!(http_duration.units, Some("s".to_string()));

        let vm_power = defs.iter().find(|d| d.id == "VmPowerState").unwrap();
        assert_eq!(vm_power.name, "VM Power State");
        assert_eq!(vm_power.metric_type, "Discrete");
        assert_eq!(vm_power.units, None);
    }

    #[test]
    fn test_built_in_metric_definitions_unique_ids() {
        let defs = built_in_metric_definitions();
        let mut ids = std::collections::HashSet::new();
        for def in &defs {
            assert!(
                ids.insert(def.id.clone()),
                "Duplicate metric ID: {}",
                def.id
            );
        }
    }

    #[test]
    fn test_built_in_metric_definitions_unique_odata_ids() {
        let defs = built_in_metric_definitions();
        let mut odata_ids = std::collections::HashSet::new();
        for def in &defs {
            assert!(
                odata_ids.insert(def.odata_id.clone()),
                "Duplicate @odata.id: {}",
                def.odata_id
            );
        }
    }

    #[test]
    fn test_metric_definition_clone() {
        let metric = MetricDefinition {
            odata_id: "/test".to_string(),
            odata_type: "#MetricDefinition.v1_3_0.MetricDefinition",
            id: "test".to_string(),
            name: "Test".to_string(),
            metric_type: "Counter".to_string(),
            units: Some("test_unit".to_string()),
        };

        let cloned = metric.clone();
        assert_eq!(metric.id, cloned.id);
        assert_eq!(metric.name, cloned.name);
        assert_eq!(metric.metric_type, cloned.metric_type);
        assert_eq!(metric.units, cloned.units);
    }

    #[test]
    fn test_telemetry_service_supported_functions() {
        let resource = TelemetryServiceResource {
            odata_id: "/redfish/v1/TelemetryService",
            odata_type: "#TelemetryService.v1_3_0.TelemetryService",
            id: "TelemetryService",
            name: "Telemetry Service",
            description: "Telemetry and metrics service",
            service_enabled: true,
            metric_definitions: ODataId::new("/redfish/v1/TelemetryService/MetricDefinitions"),
            metric_reports: ODataId::new("/redfish/v1/TelemetryService/MetricReports"),
            max_reports: 10,
            min_collection_interval: "PT10S",
            supported_collection_functions: vec!["Average", "Maximum", "Minimum"],
            status: super::super::types::Status::enabled_ok(),
        };

        let value = serde_json::to_value(&resource).unwrap();
        let functions = value["SupportedCollectionFunctions"].as_array().unwrap();
        assert_eq!(functions.len(), 3);
        assert!(functions.contains(&serde_json::json!("Average")));
        assert!(functions.contains(&serde_json::json!("Maximum")));
        assert!(functions.contains(&serde_json::json!("Minimum")));
    }
}
