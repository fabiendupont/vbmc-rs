use axum::Json;
use serde::Serialize;

use super::types::{Collection, ODataId};

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

pub async fn get_telemetry_service() -> Json<TelemetryServiceResource> {
    Json(TelemetryServiceResource {
        odata_id: "/redfish/v1/TelemetryService",
        odata_type: "#TelemetryService.v1_3_0.TelemetryService",
        id: "TelemetryService",
        name: "Telemetry Service",
        description: "Telemetry and metrics service",
        service_enabled: true,
        metric_definitions: ODataId::new(
            "/redfish/v1/TelemetryService/MetricDefinitions",
        ),
        metric_reports: ODataId::new("/redfish/v1/TelemetryService/MetricReports"),
        max_reports: 10,
        min_collection_interval: "PT10S",
        supported_collection_functions: vec!["Average", "Maximum", "Minimum"],
        status: super::types::Status::enabled_ok(),
    })
}

#[derive(Debug, Serialize)]
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

pub async fn get_metric_definitions() -> Json<Collection<ODataId>> {
    Json(Collection::new(
        "/redfish/v1/TelemetryService/MetricDefinitions",
        "#MetricDefinitionCollection.MetricDefinitionCollection",
        "Metric Definitions",
        Vec::<ODataId>::new(),
    ))
}

pub async fn get_metric_reports() -> Json<Collection<ODataId>> {
    Json(Collection::new(
        "/redfish/v1/TelemetryService/MetricReports",
        "#MetricReportCollection.MetricReportCollection",
        "Metric Reports",
        Vec::<ODataId>::new(),
    ))
}
