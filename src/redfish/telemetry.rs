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
