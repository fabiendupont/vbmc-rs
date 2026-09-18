use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use serde::Serialize;

use super::error::RedfishApiError;
use super::types::{Collection, ODataId, Status};
use crate::app_state::AppState;
use crate::auth::AuthenticatedUser;

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
    _user: AuthenticatedUser,
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
        syslog_filters: Vec::new(),
        log_purposes: vec!["Diagnostic"],
        overflow: false,
        persistency: false,
        status: Status::enabled_ok(),
    }))
}

pub async fn get_system_log_entries(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
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
pub async fn get_manager_log_services(_user: AuthenticatedUser) -> Json<Collection<ODataId>> {
    let members = vec![ODataId::new("/redfish/v1/Managers/vbmc/LogServices/Audit")];

    Json(Collection::new(
        "/redfish/v1/Managers/vbmc/LogServices",
        "#LogServiceCollection.LogServiceCollection",
        "Log Service Collection",
        members,
    ))
}

pub async fn get_manager_log_service(
    _user: AuthenticatedUser,
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
                    odata_id: format!("/redfish/v1/Managers/vbmc/LogServices/Audit/Entries/{i}"),
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
            odata_id: "/redfish/v1/Managers/vbmc/LogServices/Audit".to_string(),
            odata_type: "#LogService.v1_5_0.LogService",
            id: "Audit".to_string(),
            name: "Audit Log".to_string(),
            description: "Log service",
            entries: ODataId::new("/redfish/v1/Managers/vbmc/LogServices/Audit/Entries"),
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
            odata_id: "/redfish/v1/Managers/vbmc/LogServices/Audit/Entries/0".to_string(),
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
            "/redfish/v1/Managers/vbmc/LogServices/Audit/Entries/0"
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
        assert_eq!(json["Members@odata.count"], 1);
        assert_eq!(
            json["Members"][0]["@odata.id"],
            "/redfish/v1/Systems/sys/LogServices/Console"
        );
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

        let (status, json, _) = h::get(&app, "/redfish/v1/Managers/vbmc/LogServices").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["@odata.id"], "/redfish/v1/Managers/vbmc/LogServices");
        assert_eq!(
            json["@odata.type"],
            "#LogServiceCollection.LogServiceCollection"
        );
        assert_eq!(json["Members@odata.count"], 1);
        assert_eq!(
            json["Members"][0]["@odata.id"],
            "/redfish/v1/Managers/vbmc/LogServices/Audit"
        );
    }

    #[tokio::test]
    async fn test_get_manager_log_service() {
        let app = h::router_with_systems(HashMap::new());

        let (status, json, _) = h::get(&app, "/redfish/v1/Managers/vbmc/LogServices/Audit").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            json["@odata.id"],
            "/redfish/v1/Managers/vbmc/LogServices/Audit"
        );
        assert_eq!(json["@odata.type"], "#LogService.v1_5_0.LogService");
        assert_eq!(json["Id"], "Audit");
        assert_eq!(json["Name"], "Audit Log");
        assert_eq!(json["ServiceEnabled"], true);
        assert_eq!(json["Persistency"], true);
        assert_eq!(
            json["LogPurposes"],
            serde_json::json!(["Security", "Diagnostic"])
        );
    }

    #[tokio::test]
    async fn test_get_manager_log_service_unknown_log_id() {
        let app = h::router_with_systems(HashMap::new());

        let (status, json, _) = h::get(&app, "/redfish/v1/Managers/vbmc/LogServices/Unknown").await;

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

        let (status, json, _) =
            h::get(&app, "/redfish/v1/Managers/vbmc/LogServices/Audit/Entries").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            json["@odata.id"],
            "/redfish/v1/Managers/vbmc/LogServices/Audit/Entries"
        );
        assert_eq!(
            json["@odata.type"],
            "#LogEntryCollection.LogEntryCollection"
        );
        // Entries will be empty if no audit.jsonl file exists
        assert!(json["Members@odata.count"].as_u64().is_some());
    }

    #[tokio::test]
    async fn test_get_manager_log_entries_unknown_log_id() {
        let app = h::router_with_systems(HashMap::new());

        let (status, json, _) = h::get(
            &app,
            "/redfish/v1/Managers/vbmc/LogServices/Unknown/Entries",
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
