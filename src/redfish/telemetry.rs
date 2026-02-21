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
    #[serde(rename = "ServiceEnabled")]
    pub service_enabled: bool,
    #[serde(rename = "MetricDefinitions")]
    pub metric_definitions: ODataId,
    #[serde(rename = "MetricReports")]
    pub metric_reports: ODataId,
}

pub async fn get_telemetry_service() -> Json<TelemetryServiceResource> {
    Json(TelemetryServiceResource {
        odata_id: "/redfish/v1/TelemetryService",
        odata_type: "#TelemetryService.v1_3_0.TelemetryService",
        id: "TelemetryService",
        name: "Telemetry Service",
        service_enabled: true,
        metric_definitions: ODataId::new(
            "/redfish/v1/TelemetryService/MetricDefinitions",
        ),
        metric_reports: ODataId::new("/redfish/v1/TelemetryService/MetricReports"),
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
