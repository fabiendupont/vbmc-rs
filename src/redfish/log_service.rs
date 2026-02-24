use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use serde::Serialize;

use super::error::RedfishApiError;
use super::types::{Collection, ODataId, Status};
use crate::app_state::AppState;

#[derive(Debug, Serialize)]
pub struct LogServiceResource {
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
    #[serde(rename = "Entries")]
    pub entries: ODataId,
    #[serde(rename = "ServiceEnabled")]
    pub service_enabled: bool,
    #[serde(rename = "OverWritePolicy")]
    pub overwrite_policy: &'static str,
    #[serde(rename = "MaxNumberOfRecords")]
    pub max_number_of_records: u32,
    #[serde(rename = "DateTime")]
    pub date_time: String,
    #[serde(rename = "DateTimeLocalOffset")]
    pub date_time_local_offset: &'static str,
    #[serde(rename = "LogEntryType")]
    pub log_entry_type: &'static str,
    #[serde(rename = "AutoDSTEnabled")]
    pub auto_dst_enabled: bool,
    #[serde(rename = "OEMLogPurpose")]
    pub oem_log_purpose: &'static str,
    #[serde(rename = "SyslogFilters")]
    pub syslog_filters: Vec<serde_json::Value>,
    #[serde(rename = "LogPurposes")]
    pub log_purposes: Vec<&'static str>,
    #[serde(rename = "Overflow")]
    pub overflow: bool,
    #[serde(rename = "Persistency")]
    pub persistency: bool,
    #[serde(rename = "Status")]
    pub status: Status,
}

#[derive(Debug, Serialize)]
pub struct LogEntryResource {
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
    #[serde(rename = "EntryType")]
    pub entry_type: &'static str,
    #[serde(rename = "Message")]
    pub message: String,
    #[serde(rename = "Created", skip_serializing_if = "Option::is_none")]
    pub created: Option<String>,
    #[serde(rename = "Severity")]
    pub severity: &'static str,
}

// System LogServices collection
pub async fn get_system_log_services(
    State(state): State<Arc<AppState>>,
    Path(system_id): Path<String>,
) -> Result<Json<Collection<ODataId>>, RedfishApiError> {
    if !state.config.systems.contains_key(&system_id) {
        return Err(RedfishApiError::NotFound(format!(
            "System '{system_id}' not found"
        )));
    }

    let members = vec![ODataId::new(format!(
        "/redfish/v1/Systems/{system_id}/LogServices/Console"
    ))];

    Ok(Json(Collection::new(
        format!("/redfish/v1/Systems/{system_id}/LogServices"),
        "#LogServiceCollection.LogServiceCollection",
        "Log Service Collection",
        members,
    )))
}

pub async fn get_system_log_service(
    State(state): State<Arc<AppState>>,
    Path((system_id, log_id)): Path<(String, String)>,
) -> Result<Json<LogServiceResource>, RedfishApiError> {
    if !state.config.systems.contains_key(&system_id) {
        return Err(RedfishApiError::NotFound(format!(
            "System '{system_id}' not found"
        )));
    }
    if log_id != "Console" {
        return Err(RedfishApiError::NotFound(format!(
            "LogService '{log_id}' not found"
        )));
    }

    Ok(Json(LogServiceResource {
        odata_id: format!("/redfish/v1/Systems/{system_id}/LogServices/Console"),
        odata_type: "#LogService.v1_5_0.LogService",
        id: "Console".to_string(),
        name: "Console Log".to_string(),
        description: "Log service",
        entries: ODataId::new(format!(
            "/redfish/v1/Systems/{system_id}/LogServices/Console/Entries"
        )),
        service_enabled: true,
        overwrite_policy: "WrapsWhenFull",
        max_number_of_records: 1000,
        date_time: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        date_time_local_offset: "+00:00",
        log_entry_type: "Event",
        auto_dst_enabled: false,
        oem_log_purpose: "",
        syslog_filters: Vec::new(),
        log_purposes: vec!["Diagnostic"],
        overflow: false,
        persistency: false,
        status: Status::enabled_ok(),
    }))
}

pub async fn get_system_log_entries(
    State(state): State<Arc<AppState>>,
    Path((system_id, log_id)): Path<(String, String)>,
) -> Result<Json<Collection<ODataId>>, RedfishApiError> {
    if !state.config.systems.contains_key(&system_id) {
        return Err(RedfishApiError::NotFound(format!(
            "System '{system_id}' not found"
        )));
    }
    if log_id != "Console" {
        return Err(RedfishApiError::NotFound(format!(
            "LogService '{log_id}' not found"
        )));
    }

    // Console entries would come from serial console output; empty for now
    Ok(Json(Collection::new(
        format!("/redfish/v1/Systems/{system_id}/LogServices/{log_id}/Entries"),
        "#LogEntryCollection.LogEntryCollection",
        "Log Entry Collection",
        Vec::<ODataId>::new(),
    )))
}

// Manager LogServices collection
pub async fn get_manager_log_services() -> Json<Collection<ODataId>> {
    let members = vec![ODataId::new(
        "/redfish/v1/Managers/vbmc/LogServices/Audit",
    )];

    Json(Collection::new(
        "/redfish/v1/Managers/vbmc/LogServices",
        "#LogServiceCollection.LogServiceCollection",
        "Log Service Collection",
        members,
    ))
}

pub async fn get_manager_log_service(
    Path(log_id): Path<String>,
) -> Result<Json<LogServiceResource>, RedfishApiError> {
    if log_id != "Audit" {
        return Err(RedfishApiError::NotFound(format!(
            "LogService '{log_id}' not found"
        )));
    }

    Ok(Json(LogServiceResource {
        odata_id: "/redfish/v1/Managers/vbmc/LogServices/Audit".to_string(),
        odata_type: "#LogService.v1_5_0.LogService",
        id: "Audit".to_string(),
        name: "Audit Log".to_string(),
        description: "Log service",
        entries: ODataId::new("/redfish/v1/Managers/vbmc/LogServices/Audit/Entries"),
        service_enabled: true,
        overwrite_policy: "WrapsWhenFull",
        max_number_of_records: 1000,
        date_time: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        date_time_local_offset: "+00:00",
        log_entry_type: "Event",
        auto_dst_enabled: false,
        oem_log_purpose: "",
        syslog_filters: Vec::new(),
        log_purposes: vec!["Security", "Diagnostic"],
        overflow: false,
        persistency: true,
        status: Status::enabled_ok(),
    }))
}

pub async fn get_manager_log_entries(
    State(state): State<Arc<AppState>>,
    Path(log_id): Path<String>,
) -> Result<Json<Collection<LogEntryResource>>, RedfishApiError> {
    if log_id != "Audit" {
        return Err(RedfishApiError::NotFound(format!(
            "LogService '{log_id}' not found"
        )));
    }

    // Parse audit.jsonl entries
    let audit_path = if state.config.audit_log.as_os_str().is_empty() {
        state.config.state_directory.join("audit.jsonl")
    } else {
        state.config.audit_log.clone()
    };

    let mut entries = Vec::new();
    if let Ok(content) = std::fs::read_to_string(&audit_path) {
        for (i, line) in content.lines().rev().take(50).enumerate() {
            if let Ok(event) = serde_json::from_str::<serde_json::Value>(line) {
                entries.push(LogEntryResource {
                    odata_id: format!(
                        "/redfish/v1/Managers/vbmc/LogServices/Audit/Entries/{i}"
                    ),
                    odata_type: "#LogEntry.v1_16_0.LogEntry",
                    id: i.to_string(),
                    name: format!("Audit Entry {i}"),
                    description: "Log entry",
                    entry_type: "Event",
                    message: event
                        .get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    created: event
                        .get("event_timestamp")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    severity: "OK",
                });
            }
        }
    }

    Ok(Json(Collection::new(
        "/redfish/v1/Managers/vbmc/LogServices/Audit/Entries",
        "#LogEntryCollection.LogEntryCollection",
        "Log Entry Collection",
        entries,
    )))
}
