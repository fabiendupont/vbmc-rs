use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use serde::Serialize;

use super::error::RedfishApiError;
use super::types::{Collection, ODataId, Status};
use crate::app_state::AppState;
use crate::auth::AuthenticatedUser;
use crate::auth::rbac::{Privilege, has_privilege};

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
    _user: AuthenticatedUser,
    Path(system_id): Path<String>,
) -> Result<Json<Collection<ODataId>>, RedfishApiError> {
    if !state.config.systems.contains_key(&system_id) {
        return Err(RedfishApiError::NotFound(format!(
            "System '{system_id}' not found"
        )));
    }

    let members = vec![
        ODataId::new(format!(
            "/redfish/v1/Systems/{system_id}/LogServices/Log"
        )),
        ODataId::new(format!(
            "/redfish/v1/Systems/{system_id}/LogServices/Console"
        )),
    ];

    Ok(Json(Collection::new(
        format!("/redfish/v1/Systems/{system_id}/LogServices"),
        "#LogServiceCollection.LogServiceCollection",
        "Log Service Collection",
        members,
    )))
}

pub async fn get_system_log_service(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
    Path((system_id, log_id)): Path<(String, String)>,
) -> Result<Json<LogServiceResource>, RedfishApiError> {
    if !state.config.systems.contains_key(&system_id) {
        return Err(RedfishApiError::NotFound(format!(
            "System '{system_id}' not found"
        )));
    }

    match log_id.as_str() {
        "Log" => Ok(Json(LogServiceResource {
            odata_id: format!("/redfish/v1/Systems/{system_id}/LogServices/Log"),
            odata_type: "#LogService.v1_5_0.LogService",
            id: "Log".to_string(),
            name: "System Event Log".to_string(),
            description: "BMC system event log including power-cycle events",
            entries: ODataId::new(format!(
                "/redfish/v1/Systems/{system_id}/LogServices/Log/Entries"
            )),
            service_enabled: true,
            overwrite_policy: "WrapsWhenFull",
            max_number_of_records: 1000,
            date_time: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
            date_time_local_offset: "+00:00",
            log_entry_type: "Event",
            auto_dst_enabled: false,
            syslog_filters: Vec::new(),
            log_purposes: vec!["Diagnostic", "Operations"],
            overflow: false,
            persistency: true,
            status: Status::enabled_ok(),
        })),
        "Console" => Ok(Json(LogServiceResource {
            odata_id: format!("/redfish/v1/Systems/{system_id}/LogServices/Console"),
            odata_type: "#LogService.v1_5_0.LogService",
            id: "Console".to_string(),
            name: "Console Log".to_string(),
            description: "Serial console log",
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
            syslog_filters: Vec::new(),
            log_purposes: vec!["Diagnostic"],
            overflow: false,
            persistency: false,
            status: Status::enabled_ok(),
        })),
        _ => Err(RedfishApiError::NotFound(format!(
            "LogService '{log_id}' not found"
        ))),
    }
}

pub async fn get_system_log_entries(
    State(state): State<Arc<AppState>>,
    user: AuthenticatedUser,
    Path((system_id, log_id)): Path<(String, String)>,
) -> axum::response::Response {
    use axum::response::IntoResponse as _;
    // Dispatch to the appropriate handler based on log_id.
    match log_id.as_str() {
        "Log" => {
            match get_system_log_entries_sel(
                State(state),
                user,
                Path((system_id, log_id)),
            )
            .await
            {
                Ok(json) => json.into_response(),
                Err(e) => e.into_response(),
            }
        }
        "Console" => {
            if !state.config.systems.contains_key(&system_id) {
                return RedfishApiError::NotFound(format!("System '{system_id}' not found"))
                    .into_response();
            }
            // Console entries come from serial console output; empty for now.
            Json(Collection::new(
                format!("/redfish/v1/Systems/{system_id}/LogServices/Console/Entries"),
                "#LogEntryCollection.LogEntryCollection",
                "Log Entry Collection",
                Vec::<ODataId>::new(),
            ))
            .into_response()
        }
        _ => RedfishApiError::NotFound(format!("LogService '{log_id}' not found"))
            .into_response(),
    }
}

// Manager LogServices collection
pub async fn get_manager_log_services(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
    Path(manager_id): Path<String>,
) -> Result<Json<Collection<ODataId>>, RedfishApiError> {
    if manager_id != state.config.manager_id {
        return Err(RedfishApiError::NotFound(format!(
            "Manager '{manager_id}' not found"
        )));
    }
    let mid = &state.config.manager_id;
    // Expose both "Log" (used by NICo's machine-controller to check for reset events)
    // and "Audit" (historical name). Both serve the same audit.jsonl data.
    let members = vec![
        ODataId::new(format!("/redfish/v1/Managers/{mid}/LogServices/Log")),
        ODataId::new(format!("/redfish/v1/Managers/{mid}/LogServices/Audit")),
    ];

    Ok(Json(Collection::new(
        format!("/redfish/v1/Managers/{mid}/LogServices"),
        "#LogServiceCollection.LogServiceCollection",
        "Log Service Collection",
        members,
    )))
}

pub async fn get_manager_log_service(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
    Path((manager_id, log_id)): Path<(String, String)>,
) -> Result<Json<LogServiceResource>, RedfishApiError> {
    if manager_id != state.config.manager_id {
        return Err(RedfishApiError::NotFound(format!(
            "Manager '{manager_id}' not found"
        )));
    }
    // Accept "Log" (NICo convention) and "Audit" (legacy); both serve the same log.
    if log_id != "Audit" && log_id != "Log" {
        return Err(RedfishApiError::NotFound(format!(
            "LogService '{log_id}' not found"
        )));
    }

    let mid = &state.config.manager_id;
    Ok(Json(LogServiceResource {
        odata_id: format!("/redfish/v1/Managers/{mid}/LogServices/{log_id}"),
        odata_type: "#LogService.v1_5_0.LogService",
        id: log_id.clone(),
        name: "BMC Event Log".to_string(),
        description: "BMC event log including power-cycle events",
        entries: ODataId::new(format!(
            "/redfish/v1/Managers/{mid}/LogServices/{log_id}/Entries"
        )),
        service_enabled: true,
        overwrite_policy: "WrapsWhenFull",
        max_number_of_records: 1000,
        date_time: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        date_time_local_offset: "+00:00",
        log_entry_type: "Event",
        auto_dst_enabled: false,
        syslog_filters: Vec::new(),
        log_purposes: vec!["Security", "Diagnostic"],
        overflow: false,
        persistency: true,
        status: Status::enabled_ok(),
    }))
}

pub async fn get_manager_log_entries(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
    Path((manager_id, log_id)): Path<(String, String)>,
) -> Result<Json<Collection<LogEntryResource>>, RedfishApiError> {
    if manager_id != state.config.manager_id {
        return Err(RedfishApiError::NotFound(format!(
            "Manager '{manager_id}' not found"
        )));
    }
    let mid = &state.config.manager_id;
    let collection_path = format!("/redfish/v1/Managers/{mid}/LogServices/{log_id}/Entries");

    match log_id.as_str() {
        "Log" => {
            // "Log" serves the dedicated in-memory/persistent event log that
            // contains power-cycle and BMC events.  This is what NICo's
            // machine-controller queries when checking for a post-reset entry.
            let raw = state.event_log.entries_newest_first();
            let entries: Vec<LogEntryResource> = raw
                .into_iter()
                .map(|e| LogEntryResource {
                    odata_id: format!("{collection_path}/{}", e.id),
                    odata_type: "#LogEntry.v1_16_0.LogEntry",
                    id: e.id.to_string(),
                    name: format!("Entry {}", e.id),
                    description: "Log entry",
                    entry_type: "Event",
                    message: e.message,
                    created: Some(
                        e.created
                            .format("%Y-%m-%dT%H:%M:%S%.3fZ")
                            .to_string(),
                    ),
                    severity: match e.severity.as_str() {
                        "Warning" => "Warning",
                        "Critical" => "Critical",
                        _ => "OK",
                    },
                })
                .collect();

            Ok(Json(Collection::new(
                collection_path,
                "#LogEntryCollection.LogEntryCollection",
                "Log Entry Collection",
                entries,
            )))
        }
        "Audit" => {
            // "Audit" keeps the legacy behaviour: read from audit.jsonl.
            // RedfishEvent serialises its fields with PascalCase Redfish names
            // (e.g. "Message", "EventTimestamp") — use those exact keys here.
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
                            odata_id: format!("{collection_path}/{i}"),
                            odata_type: "#LogEntry.v1_16_0.LogEntry",
                            id: i.to_string(),
                            name: format!("Entry {i}"),
                            description: "Log entry",
                            entry_type: "Event",
                            message: event
                                .get("Message")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string(),
                            created: event
                                .get("EventTimestamp")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string()),
                            severity: "OK",
                        });
                    }
                }
            }

            Ok(Json(Collection::new(
                collection_path,
                "#LogEntryCollection.LogEntryCollection",
                "Log Entry Collection",
                entries,
            )))
        }
        _ => Err(RedfishApiError::NotFound(format!(
            "LogService '{log_id}' not found"
        ))),
    }
}

/// POST /redfish/v1/Managers/{manager_id}/LogServices/{log_id}/Actions/LogService.ClearLog
pub async fn clear_manager_log(
    State(state): State<Arc<AppState>>,
    user: AuthenticatedUser,
    Path((manager_id, log_id)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, RedfishApiError> {
    if manager_id != state.config.manager_id {
        return Err(RedfishApiError::NotFound(format!(
            "Manager '{manager_id}' not found"
        )));
    }
    if !has_privilege(&user.role, Privilege::ConfigureManager) {
        return Err(RedfishApiError::Forbidden(
            "Insufficient privileges".to_string(),
        ));
    }

    match log_id.as_str() {
        "Log" => {
            state.event_log.clear();
            Ok(Json(serde_json::json!({"message": "Log cleared"})))
        }
        "Audit" => {
            // Clear the audit.jsonl file.
            let audit_path = if state.config.audit_log.as_os_str().is_empty() {
                state.config.state_directory.join("audit.jsonl")
            } else {
                state.config.audit_log.clone()
            };
            let _ = std::fs::write(&audit_path, "");
            Ok(Json(serde_json::json!({"message": "Audit log cleared"})))
        }
        _ => Err(RedfishApiError::NotFound(format!(
            "LogService '{log_id}' not found"
        ))),
    }
}

/// GET /redfish/v1/Systems/{system_id}/LogServices/Log/Entries
/// Serves the same persistent event log as the Manager LogServices/Log endpoint.
pub async fn get_system_log_entries_sel(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
    Path((system_id, log_id)): Path<(String, String)>,
) -> Result<Json<Collection<LogEntryResource>>, RedfishApiError> {
    if !state.config.systems.contains_key(&system_id) {
        return Err(RedfishApiError::NotFound(format!(
            "System '{system_id}' not found"
        )));
    }

    let collection_path =
        format!("/redfish/v1/Systems/{system_id}/LogServices/{log_id}/Entries");

    match log_id.as_str() {
        "Log" => {
            let raw = state.event_log.entries_newest_first();
            let entries: Vec<LogEntryResource> = raw
                .into_iter()
                .map(|e| LogEntryResource {
                    odata_id: format!("{collection_path}/{}", e.id),
                    odata_type: "#LogEntry.v1_16_0.LogEntry",
                    id: e.id.to_string(),
                    name: format!("Entry {}", e.id),
                    description: "Log entry",
                    entry_type: "Event",
                    message: e.message,
                    created: Some(
                        e.created
                            .format("%Y-%m-%dT%H:%M:%S%.3fZ")
                            .to_string(),
                    ),
                    severity: match e.severity.as_str() {
                        "Warning" => "Warning",
                        "Critical" => "Critical",
                        _ => "OK",
                    },
                })
                .collect();

            Ok(Json(Collection::new(
                collection_path,
                "#LogEntryCollection.LogEntryCollection",
                "Log Entry Collection",
                entries,
            )))
        }
        "Console" => {
            // Console entries: delegate to the existing handler logic.
            Ok(Json(Collection::new(
                collection_path,
                "#LogEntryCollection.LogEntryCollection",
                "Log Entry Collection",
                Vec::<LogEntryResource>::new(),
            )))
        }
        _ => Err(RedfishApiError::NotFound(format!(
            "LogService '{log_id}' not found"
        ))),
    }
}

/// GET /redfish/v1/Systems/{system_id}/LogServices/{log_id}/Entries/{entry_id}
pub async fn get_system_log_entry(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
    Path((system_id, log_id, entry_id)): Path<(String, String, String)>,
) -> Result<Json<LogEntryResource>, RedfishApiError> {
    if !state.config.systems.contains_key(&system_id) {
        return Err(RedfishApiError::NotFound(format!(
            "System '{system_id}' not found"
        )));
    }
    let id: u64 = entry_id
        .parse()
        .map_err(|_| RedfishApiError::NotFound(format!("Log entry '{entry_id}' not found")))?;
    let entry = state
        .event_log
        .entries_newest_first()
        .into_iter()
        .find(|e| e.id == id)
        .ok_or_else(|| RedfishApiError::NotFound(format!("Log entry '{entry_id}' not found")))?;
    let base = format!("/redfish/v1/Systems/{system_id}/LogServices/{log_id}/Entries");
    Ok(Json(LogEntryResource {
        odata_id: format!("{base}/{}", entry.id),
        odata_type: "#LogEntry.v1_16_0.LogEntry",
        id: entry.id.to_string(),
        name: format!("Entry {}", entry.id),
        description: "Log entry",
        entry_type: "Event",
        message: entry.message,
        created: Some(
            entry
                .created
                .to_rfc3339_opts(chrono::SecondsFormat::Micros, true)
                .to_string(),
        ),
        severity: match entry.severity.as_str() {
            "Warning" => "Warning",
            "Critical" => "Critical",
            _ => "OK",
        },
    }))
}

/// GET /redfish/v1/Managers/{manager_id}/LogServices/{log_id}/Entries/{entry_id}
pub async fn get_manager_log_entry(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
    Path((manager_id, log_id, entry_id)): Path<(String, String, String)>,
) -> Result<Json<LogEntryResource>, RedfishApiError> {
    if manager_id != state.config.manager_id {
        return Err(RedfishApiError::NotFound(format!(
            "Manager '{manager_id}' not found"
        )));
    }
    let id: u64 = entry_id
        .parse()
        .map_err(|_| RedfishApiError::NotFound(format!("Log entry '{entry_id}' not found")))?;
    let entry = state
        .event_log
        .entries_newest_first()
        .into_iter()
        .find(|e| e.id == id)
        .ok_or_else(|| RedfishApiError::NotFound(format!("Log entry '{entry_id}' not found")))?;
    let base = format!("/redfish/v1/Managers/{manager_id}/LogServices/{log_id}/Entries");
    Ok(Json(LogEntryResource {
        odata_id: format!("{base}/{}", entry.id),
        odata_type: "#LogEntry.v1_16_0.LogEntry",
        id: entry.id.to_string(),
        name: format!("Entry {}", entry.id),
        description: "Log entry",
        entry_type: "Event",
        message: entry.message,
        created: Some(
            entry
                .created
                .to_rfc3339_opts(chrono::SecondsFormat::Micros, true)
                .to_string(),
        ),
        severity: match entry.severity.as_str() {
            "Warning" => "Warning",
            "Critical" => "Critical",
            _ => "OK",
        },
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_service_serialization() {
        let service = LogServiceResource {
            odata_id: "/redfish/v1/Systems/vm1/LogServices/Console".to_string(),
            odata_type: "#LogService.v1_5_0.LogService",
            id: "Console".to_string(),
            name: "Console Log".to_string(),
            description: "Log service",
            entries: ODataId::new("/redfish/v1/Systems/vm1/LogServices/Console/Entries"),
            service_enabled: true,
            overwrite_policy: "WrapsWhenFull",
            max_number_of_records: 1000,
            date_time: "2024-01-01T00:00:00Z".to_string(),
            date_time_local_offset: "+00:00",
            log_entry_type: "Event",
            auto_dst_enabled: false,
            syslog_filters: Vec::new(),
            log_purposes: vec!["Diagnostic"],
            overflow: false,
            persistency: false,
            status: Status::enabled_ok(),
        };

        let value = serde_json::to_value(&service).unwrap();

        assert_eq!(
            value["@odata.id"],
            "/redfish/v1/Systems/vm1/LogServices/Console"
        );
        assert_eq!(value["@odata.type"], "#LogService.v1_5_0.LogService");
        assert_eq!(value["Id"], "Console");
        assert_eq!(value["Name"], "Console Log");
        assert_eq!(
            value["Entries"]["@odata.id"],
            "/redfish/v1/Systems/vm1/LogServices/Console/Entries"
        );
        assert_eq!(value["ServiceEnabled"], true);
        assert_eq!(value["OverWritePolicy"], "WrapsWhenFull");
        assert_eq!(value["MaxNumberOfRecords"], 1000);
        assert_eq!(value["DateTime"], "2024-01-01T00:00:00Z");
        assert_eq!(value["DateTimeLocalOffset"], "+00:00");
        assert_eq!(value["LogEntryType"], "Event");
        assert_eq!(value["AutoDSTEnabled"], false);
        assert_eq!(value["SyslogFilters"], serde_json::json!([]));
        assert_eq!(value["LogPurposes"], serde_json::json!(["Diagnostic"]));
        assert_eq!(value["Overflow"], false);
        assert_eq!(value["Persistency"], false);
    }

    #[test]
    fn test_log_service_with_multiple_purposes() {
        let service = LogServiceResource {
            odata_id: "/redfish/v1/Managers/1/LogServices/Audit".to_string(),
            odata_type: "#LogService.v1_5_0.LogService",
            id: "Audit".to_string(),
            name: "Audit Log".to_string(),
            description: "Log service",
            entries: ODataId::new("/redfish/v1/Managers/1/LogServices/Audit/Entries"),
            service_enabled: true,
            overwrite_policy: "WrapsWhenFull",
            max_number_of_records: 1000,
            date_time: "2024-01-01T00:00:00Z".to_string(),
            date_time_local_offset: "+00:00",
            log_entry_type: "Event",
            auto_dst_enabled: false,
            syslog_filters: Vec::new(),
            log_purposes: vec!["Security", "Diagnostic"],
            overflow: false,
            persistency: true,
            status: Status::enabled_ok(),
        };

        let value = serde_json::to_value(&service).unwrap();

        assert_eq!(
            value["LogPurposes"],
            serde_json::json!(["Security", "Diagnostic"])
        );
        assert_eq!(value["Persistency"], true);
    }

    #[test]
    fn test_log_entry_serialization() {
        let entry = LogEntryResource {
            odata_id: "/redfish/v1/Managers/1/LogServices/Audit/Entries/0".to_string(),
            odata_type: "#LogEntry.v1_16_0.LogEntry",
            id: "0".to_string(),
            name: "Audit Entry 0".to_string(),
            description: "Log entry",
            entry_type: "Event",
            message: "User logged in".to_string(),
            created: Some("2024-01-01T00:00:00Z".to_string()),
            severity: "OK",
        };

        let value = serde_json::to_value(&entry).unwrap();

        assert_eq!(
            value["@odata.id"],
            "/redfish/v1/Managers/1/LogServices/Audit/Entries/0"
        );
        assert_eq!(value["@odata.type"], "#LogEntry.v1_16_0.LogEntry");
        assert_eq!(value["Id"], "0");
        assert_eq!(value["Name"], "Audit Entry 0");
        assert_eq!(value["EntryType"], "Event");
        assert_eq!(value["Message"], "User logged in");
        assert_eq!(value["Created"], "2024-01-01T00:00:00Z");
        assert_eq!(value["Severity"], "OK");
    }

    #[test]
    fn test_log_entry_without_created() {
        let entry = LogEntryResource {
            odata_id: "/redfish/v1/test/Entries/1".to_string(),
            odata_type: "#LogEntry.v1_16_0.LogEntry",
            id: "1".to_string(),
            name: "Entry".to_string(),
            description: "Log entry",
            entry_type: "Event",
            message: "Test message".to_string(),
            created: None,
            severity: "Warning",
        };

        let value = serde_json::to_value(&entry).unwrap();

        assert_eq!(value["Message"], "Test message");
        assert_eq!(value["Severity"], "Warning");
        // Created should be absent when None
        assert!(value.get("Created").is_none());
    }
}

#[cfg(test)]
mod harness_tests {
    use crate::redfish::test_harness as h;
    use axum::http::StatusCode;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_get_system_log_services_collection() {
        let systems = h::systems_with("sys");
        let app = h::router_with_systems(systems);

        let (status, json, _) = h::get(&app, "/redfish/v1/Systems/sys/LogServices").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["@odata.id"], "/redfish/v1/Systems/sys/LogServices");
        assert_eq!(
            json["@odata.type"],
            "#LogServiceCollection.LogServiceCollection"
        );
        // "Log" (SEL) and "Console" are now both listed.
        assert_eq!(json["Members@odata.count"], 2);
        let members = json["Members"].as_array().unwrap();
        let ids: Vec<&str> = members
            .iter()
            .filter_map(|m| m["@odata.id"].as_str())
            .collect();
        assert!(ids.contains(&"/redfish/v1/Systems/sys/LogServices/Log"));
        assert!(ids.contains(&"/redfish/v1/Systems/sys/LogServices/Console"));
    }

    #[tokio::test]
    async fn test_get_system_log_services_unknown_system() {
        let systems = h::systems_with("sys");
        let app = h::router_with_systems(systems);

        let (status, json, _) = h::get(&app, "/redfish/v1/Systems/unknown/LogServices").await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(
            json["error"]["message"]
                .as_str()
                .unwrap()
                .contains("not found")
        );
    }

    #[tokio::test]
    async fn test_get_system_log_service() {
        let systems = h::systems_with("sys");
        let app = h::router_with_systems(systems);

        let (status, json, _) = h::get(&app, "/redfish/v1/Systems/sys/LogServices/Console").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            json["@odata.id"],
            "/redfish/v1/Systems/sys/LogServices/Console"
        );
        assert_eq!(json["@odata.type"], "#LogService.v1_5_0.LogService");
        assert_eq!(json["Id"], "Console");
        assert_eq!(json["Name"], "Console Log");
        assert_eq!(json["ServiceEnabled"], true);
        assert_eq!(json["OverWritePolicy"], "WrapsWhenFull");
        assert_eq!(json["MaxNumberOfRecords"], 1000);
        assert_eq!(
            json["Entries"]["@odata.id"],
            "/redfish/v1/Systems/sys/LogServices/Console/Entries"
        );
    }

    #[tokio::test]
    async fn test_get_system_log_service_unknown_log_id() {
        let systems = h::systems_with("sys");
        let app = h::router_with_systems(systems);

        let (status, json, _) = h::get(&app, "/redfish/v1/Systems/sys/LogServices/Unknown").await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(
            json["error"]["message"]
                .as_str()
                .unwrap()
                .contains("not found")
        );
    }

    #[tokio::test]
    async fn test_get_system_log_entries() {
        let systems = h::systems_with("sys");
        let app = h::router_with_systems(systems);

        let (status, json, _) =
            h::get(&app, "/redfish/v1/Systems/sys/LogServices/Console/Entries").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            json["@odata.id"],
            "/redfish/v1/Systems/sys/LogServices/Console/Entries"
        );
        assert_eq!(
            json["@odata.type"],
            "#LogEntryCollection.LogEntryCollection"
        );
        // Console entries are empty (would come from serial console)
        assert_eq!(json["Members@odata.count"], 0);
    }

    #[tokio::test]
    async fn test_get_system_log_entries_unknown_system() {
        let systems = h::systems_with("sys");
        let app = h::router_with_systems(systems);

        let (status, json, _) = h::get(
            &app,
            "/redfish/v1/Systems/unknown/LogServices/Console/Entries",
        )
        .await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(
            json["error"]["message"]
                .as_str()
                .unwrap()
                .contains("not found")
        );
    }

    #[tokio::test]
    async fn test_get_manager_log_services() {
        let app = h::router_with_systems(HashMap::new());

        let (status, json, _) = h::get(&app, "/redfish/v1/Managers/1/LogServices").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["@odata.id"], "/redfish/v1/Managers/1/LogServices");
        assert_eq!(
            json["@odata.type"],
            "#LogServiceCollection.LogServiceCollection"
        );
        // Both "Log" and "Audit" are now listed.
        assert_eq!(json["Members@odata.count"], 2);
        let members = json["Members"].as_array().unwrap();
        let ids: Vec<&str> = members
            .iter()
            .filter_map(|m| m["@odata.id"].as_str())
            .collect();
        assert!(ids.contains(&"/redfish/v1/Managers/1/LogServices/Log"));
        assert!(ids.contains(&"/redfish/v1/Managers/1/LogServices/Audit"));
    }

    #[tokio::test]
    async fn test_get_manager_log_service() {
        let app = h::router_with_systems(HashMap::new());

        // Test both "Audit" (legacy) and "Log" (NICo convention).
        for log_id in &["Audit", "Log"] {
            let (status, json, _) = h::get(
                &app,
                &format!("/redfish/v1/Managers/1/LogServices/{log_id}"),
            )
            .await;

            assert_eq!(status, StatusCode::OK, "log_id={log_id}");
            assert_eq!(
                json["@odata.id"],
                format!("/redfish/v1/Managers/1/LogServices/{log_id}"),
                "log_id={log_id}"
            );
            assert_eq!(json["@odata.type"], "#LogService.v1_5_0.LogService");
            assert_eq!(json["Id"], *log_id);
            assert_eq!(json["ServiceEnabled"], true);
            assert_eq!(json["Persistency"], true);
            assert_eq!(
                json["LogPurposes"],
                serde_json::json!(["Security", "Diagnostic"])
            );
        }
    }

    #[tokio::test]
    async fn test_get_manager_log_service_unknown_log_id() {
        let app = h::router_with_systems(HashMap::new());

        let (status, json, _) = h::get(&app, "/redfish/v1/Managers/1/LogServices/Unknown").await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(
            json["error"]["message"]
                .as_str()
                .unwrap()
                .contains("not found")
        );
    }

    #[tokio::test]
    async fn test_get_manager_log_entries() {
        let app = h::router_with_systems(HashMap::new());

        // Both "Log" and "Audit" must work; each returns entries at its own @odata.id.
        for log_id in &["Audit", "Log"] {
            let path = format!("/redfish/v1/Managers/1/LogServices/{log_id}/Entries");
            let (status, json, _) = h::get(&app, &path).await;

            assert_eq!(status, StatusCode::OK, "log_id={log_id}");
            assert_eq!(json["@odata.id"], path, "log_id={log_id}");
            assert_eq!(
                json["@odata.type"],
                "#LogEntryCollection.LogEntryCollection"
            );
            // Entries will be empty if no audit.jsonl file exists
            assert!(json["Members@odata.count"].as_u64().is_some());
        }
    }

    #[tokio::test]
    async fn test_get_manager_log_entries_unknown_log_id() {
        let app = h::router_with_systems(HashMap::new());

        let (status, json, _) = h::get(
            &app,
            "/redfish/v1/Managers/1/LogServices/Unknown/Entries",
        )
        .await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(
            json["error"]["message"]
                .as_str()
                .unwrap()
                .contains("not found")
        );
    }
}
