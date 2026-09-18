use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

use super::types::{Collection, ODataId};
use crate::app_state::AppState;
use crate::auth::AuthenticatedUser;
use crate::redfish::error::RedfishApiError;
use crate::tasks::TaskState;

#[derive(Debug, Serialize)]
pub struct TaskServiceResource {
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
    #[serde(rename = "Tasks")]
    pub tasks: ODataId,
    #[serde(rename = "CompletedTaskOverWritePolicy")]
    pub completed_task_overwrite_policy: &'static str,
    #[serde(rename = "TaskAutoDeleteTimeoutMinutes")]
    pub task_auto_delete_timeout_minutes: u32,
    #[serde(rename = "DateTime")]
    pub date_time: String,
    #[serde(rename = "LifeCycleEventOnTaskStateChange")]
    pub life_cycle_event_on_task_state_change: bool,
    #[serde(rename = "Status")]
    pub status: super::types::Status,
}

pub async fn get_task_service(_user: AuthenticatedUser) -> Json<TaskServiceResource> {
    Json(TaskServiceResource {
        odata_id: "/redfish/v1/TaskService",
        odata_type: "#TaskService.v1_2_0.TaskService",
        id: "TaskService",
        name: "Task Service",
        description: "Task management service",
        service_enabled: true,
        tasks: ODataId::new("/redfish/v1/TaskService/Tasks"),
        completed_task_overwrite_policy: "Oldest",
        task_auto_delete_timeout_minutes: 60,
        date_time: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        life_cycle_event_on_task_state_change: false,
        status: super::types::Status::enabled_ok(),
    })
}

pub async fn get_tasks(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
) -> Json<Collection<ODataId>> {
    let tasks = state.task_manager.list_tasks();
    let members: Vec<ODataId> = tasks
        .iter()
        .map(|t| ODataId::new(format!("/redfish/v1/TaskService/Tasks/{}", t.id)))
        .collect();

    Json(Collection::new(
        "/redfish/v1/TaskService/Tasks",
        "#TaskCollection.TaskCollection",
        "Task Collection",
        members,
    ))
}

#[derive(Debug, Serialize)]
pub struct TaskResource {
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
    #[serde(rename = "TaskState")]
    pub task_state: TaskState,
    #[serde(rename = "TaskStatus")]
    pub task_status: String,
    #[serde(rename = "StartTime")]
    pub start_time: String,
    #[serde(rename = "EndTime", skip_serializing_if = "Option::is_none")]
    pub end_time: Option<String>,
    #[serde(rename = "PercentComplete", skip_serializing_if = "Option::is_none")]
    pub percent_complete: Option<u8>,
    #[serde(rename = "TaskMonitor", skip_serializing_if = "Option::is_none")]
    pub task_monitor: Option<String>,
}

pub async fn get_task(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
    Path(task_id): Path<String>,
) -> Result<Json<TaskResource>, RedfishApiError> {
    let task = state
        .task_manager
        .get_task(&task_id)
        .ok_or_else(|| RedfishApiError::NotFound(format!("Task '{task_id}' not found")))?;

    Ok(Json(TaskResource {
        odata_id: format!("/redfish/v1/TaskService/Tasks/{}", task.id),
        odata_type: "#Task.v1_7_0.Task",
        id: task.id.clone(),
        name: task.name,
        description: "Background task",
        task_state: task.task_state,
        task_status: task.task_status,
        start_time: task.start_time.to_rfc3339(),
        end_time: task.end_time.map(|t| t.to_rfc3339()),
        percent_complete: task.percent_complete,
        task_monitor: Some(format!("/redfish/v1/TaskService/TaskMonitors/{}", task.id)),
    }))
}

pub async fn get_task_monitor(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
    Path(task_id): Path<String>,
) -> Result<Response, RedfishApiError> {
    let task = state
        .task_manager
        .get_task(&task_id)
        .ok_or_else(|| RedfishApiError::NotFound(format!("Task '{task_id}' not found")))?;

    match task.task_state {
        TaskState::Completed => {
            let result = task
                .result
                .unwrap_or(serde_json::json!({"message": "Task completed"}));
            Ok((StatusCode::OK, Json(result)).into_response())
        }
        TaskState::Exception | TaskState::Killed => {
            let msg = task
                .messages
                .first()
                .cloned()
                .unwrap_or_else(|| "Task failed".to_string());
            Ok((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": msg})),
            )
                .into_response())
        }
        _ => {
            // Still running: return 202 with Location header
            let body = serde_json::json!({
                "TaskState": task.task_state,
                "PercentComplete": task.percent_complete,
            });
            Ok((
                StatusCode::ACCEPTED,
                [(
                    "Location",
                    format!("/redfish/v1/TaskService/TaskMonitors/{}", task.id),
                )],
                Json(body),
            )
                .into_response())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_service_serialization() {
        let service = TaskServiceResource {
            odata_id: "/redfish/v1/TaskService",
            odata_type: "#TaskService.v1_2_0.TaskService",
            id: "TaskService",
            name: "Task Service",
            description: "Task management service",
            service_enabled: true,
            tasks: ODataId::new("/redfish/v1/TaskService/Tasks"),
            completed_task_overwrite_policy: "Oldest",
            task_auto_delete_timeout_minutes: 60,
            date_time: "2024-01-01T00:00:00Z".to_string(),
            life_cycle_event_on_task_state_change: false,
            status: super::super::types::Status::enabled_ok(),
        };

        let value = serde_json::to_value(&service).unwrap();

        assert_eq!(value["@odata.id"], "/redfish/v1/TaskService");
        assert_eq!(value["@odata.type"], "#TaskService.v1_2_0.TaskService");
        assert_eq!(value["Id"], "TaskService");
        assert_eq!(value["Name"], "Task Service");
        assert_eq!(value["ServiceEnabled"], true);
        assert_eq!(value["Tasks"]["@odata.id"], "/redfish/v1/TaskService/Tasks");
        assert_eq!(value["CompletedTaskOverWritePolicy"], "Oldest");
        assert_eq!(value["TaskAutoDeleteTimeoutMinutes"], 60);
        assert_eq!(value["DateTime"], "2024-01-01T00:00:00Z");
        assert_eq!(value["LifeCycleEventOnTaskStateChange"], false);
    }

    #[test]
    fn test_task_resource_serialization() {
        let task = TaskResource {
            odata_id: "/redfish/v1/TaskService/Tasks/123".to_string(),
            odata_type: "#Task.v1_7_0.Task",
            id: "123".to_string(),
            name: "Update firmware".to_string(),
            description: "Background task",
            task_state: TaskState::Running,
            task_status: "Running".to_string(),
            start_time: "2024-01-01T00:00:00Z".to_string(),
            end_time: None,
            percent_complete: Some(50),
            task_monitor: Some("/redfish/v1/TaskService/TaskMonitors/123".to_string()),
        };

        let value = serde_json::to_value(&task).unwrap();

        assert_eq!(value["@odata.id"], "/redfish/v1/TaskService/Tasks/123");
        assert_eq!(value["@odata.type"], "#Task.v1_7_0.Task");
        assert_eq!(value["Id"], "123");
        assert_eq!(value["Name"], "Update firmware");
        assert_eq!(value["TaskState"], "Running");
        assert_eq!(value["StartTime"], "2024-01-01T00:00:00Z");
        assert_eq!(value["PercentComplete"], 50);
        assert_eq!(
            value["TaskMonitor"],
            "/redfish/v1/TaskService/TaskMonitors/123"
        );
        // EndTime should be absent when None
        assert!(value.get("EndTime").is_none());
    }

    #[test]
    fn test_task_resource_with_end_time() {
        let task = TaskResource {
            odata_id: "/redfish/v1/TaskService/Tasks/123".to_string(),
            odata_type: "#Task.v1_7_0.Task",
            id: "123".to_string(),
            name: "Update firmware".to_string(),
            description: "Background task",
            task_state: TaskState::Completed,
            task_status: "OK".to_string(),
            start_time: "2024-01-01T00:00:00Z".to_string(),
            end_time: Some("2024-01-01T00:05:00Z".to_string()),
            percent_complete: Some(100),
            task_monitor: None,
        };

        let value = serde_json::to_value(&task).unwrap();

        assert_eq!(value["TaskState"], "Completed");
        assert_eq!(value["EndTime"], "2024-01-01T00:05:00Z");
        assert_eq!(value["PercentComplete"], 100);
        // TaskMonitor should be absent when None
        assert!(value.get("TaskMonitor").is_none());
    }

    #[test]
    fn test_task_resource_optional_fields_none() {
        let task = TaskResource {
            odata_id: "/redfish/v1/TaskService/Tasks/999".to_string(),
            odata_type: "#Task.v1_7_0.Task",
            id: "999".to_string(),
            name: "Test".to_string(),
            description: "Background task",
            task_state: TaskState::New,
            task_status: "Pending".to_string(),
            start_time: "2024-01-01T00:00:00Z".to_string(),
            end_time: None,
            percent_complete: None,
            task_monitor: None,
        };

        let value = serde_json::to_value(&task).unwrap();

        // All three optional fields should be absent
        assert!(value.get("EndTime").is_none());
        assert!(value.get("PercentComplete").is_none());
        assert!(value.get("TaskMonitor").is_none());
    }
}

#[cfg(test)]
mod harness_tests {
    use crate::backend::mock::MockBackend;
    use crate::redfish::test_harness as h;
    use axum::http::StatusCode;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_get_task_service() {
        let app = h::router_with_systems(HashMap::new());

        let (status, json, _) = h::get(&app, "/redfish/v1/TaskService").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["@odata.id"], "/redfish/v1/TaskService");
        assert_eq!(json["@odata.type"], "#TaskService.v1_2_0.TaskService");
        assert_eq!(json["Id"], "TaskService");
        assert_eq!(json["Name"], "Task Service");
        assert_eq!(json["ServiceEnabled"], true);
        assert_eq!(json["Tasks"]["@odata.id"], "/redfish/v1/TaskService/Tasks");
        assert_eq!(json["CompletedTaskOverWritePolicy"], "Oldest");
        assert_eq!(json["TaskAutoDeleteTimeoutMinutes"], 60);
        assert!(json["DateTime"].is_string());
    }

    #[tokio::test]
    async fn test_get_tasks_empty() {
        let app = h::router_with_systems(HashMap::new());

        let (status, json, _) = h::get(&app, "/redfish/v1/TaskService/Tasks").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["@odata.id"], "/redfish/v1/TaskService/Tasks");
        assert_eq!(json["@odata.type"], "#TaskCollection.TaskCollection");
        assert_eq!(json["Members@odata.count"], 0);
    }

    #[tokio::test]
    async fn test_get_tasks_with_tasks() {
        let mock = MockBackend::new();
        let state = h::app_state(mock, HashMap::new());

        // Create some tasks
        let task_id_1 = state.task_manager.create_task("Task 1");
        let task_id_2 = state.task_manager.create_task("Task 2");
        state
            .task_manager
            .complete_task(&task_id_2, Some(serde_json::json!({"result": "ok"})));

        let app = h::router(state);
        let (status, json, _) = h::get(&app, "/redfish/v1/TaskService/Tasks").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["Members@odata.count"], 2);
        // Order is not guaranteed with DashMap, so check both tasks are present
        let members: Vec<String> = json["Members"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["@odata.id"].as_str().unwrap().to_string())
            .collect();
        assert!(members.contains(&format!("/redfish/v1/TaskService/Tasks/{}", task_id_1)));
        assert!(members.contains(&format!("/redfish/v1/TaskService/Tasks/{}", task_id_2)));
    }

    #[tokio::test]
    async fn test_get_task_running() {
        let mock = MockBackend::new();
        let state = h::app_state(mock, HashMap::new());

        let task_id = state.task_manager.create_task("Test Task");

        let app = h::router(state);
        let (status, json, _) =
            h::get(&app, &format!("/redfish/v1/TaskService/Tasks/{}", task_id)).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            json["@odata.id"],
            format!("/redfish/v1/TaskService/Tasks/{}", task_id)
        );
        assert_eq!(json["@odata.type"], "#Task.v1_7_0.Task");
        assert_eq!(json["Id"], task_id);
        assert_eq!(json["Name"], "Test Task");
        assert_eq!(json["TaskState"], "Running");
        assert_eq!(json["TaskStatus"], "OK");
        assert!(json["StartTime"].is_string());
        assert_eq!(json["PercentComplete"], 0);
        assert_eq!(
            json["TaskMonitor"],
            format!("/redfish/v1/TaskService/TaskMonitors/{}", task_id)
        );
    }

    #[tokio::test]
    async fn test_get_task_completed() {
        let mock = MockBackend::new();
        let state = h::app_state(mock, HashMap::new());

        let task_id = state.task_manager.create_task("Completed Task");
        state
            .task_manager
            .complete_task(&task_id, Some(serde_json::json!({"status": "success"})));

        let app = h::router(state);
        let (status, json, _) =
            h::get(&app, &format!("/redfish/v1/TaskService/Tasks/{}", task_id)).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["TaskState"], "Completed");
        assert_eq!(json["PercentComplete"], 100);
        assert!(json["EndTime"].is_string());
    }

    #[tokio::test]
    async fn test_get_task_not_found() {
        let app = h::router_with_systems(HashMap::new());

        let (status, json, _) = h::get(&app, "/redfish/v1/TaskService/Tasks/9999").await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(
            json["error"]["message"]
                .as_str()
                .unwrap()
                .contains("not found")
        );
    }

    #[tokio::test]
    async fn test_get_task_monitor_running() {
        let mock = MockBackend::new();
        let state = h::app_state(mock, HashMap::new());

        let task_id = state.task_manager.create_task("Monitor Test");

        let app = h::router(state);
        let (status, json, headers) = h::get(
            &app,
            &format!("/redfish/v1/TaskService/TaskMonitors/{}", task_id),
        )
        .await;

        assert_eq!(status, StatusCode::ACCEPTED);
        assert_eq!(json["TaskState"], "Running");
        assert_eq!(json["PercentComplete"], 0);
        assert!(headers.contains_key("location"));
        assert_eq!(
            headers["location"],
            format!("/redfish/v1/TaskService/TaskMonitors/{}", task_id)
        );
    }

    #[tokio::test]
    async fn test_get_task_monitor_completed() {
        let mock = MockBackend::new();
        let state = h::app_state(mock, HashMap::new());

        let task_id = state.task_manager.create_task("Completed Monitor");
        state
            .task_manager
            .complete_task(&task_id, Some(serde_json::json!({"message": "done"})));

        let app = h::router(state);
        let (status, json, _) = h::get(
            &app,
            &format!("/redfish/v1/TaskService/TaskMonitors/{}", task_id),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["message"], "done");
    }

    #[tokio::test]
    async fn test_get_task_monitor_failed() {
        let mock = MockBackend::new();
        let state = h::app_state(mock, HashMap::new());

        let task_id = state.task_manager.create_task("Failed Task");
        state
            .task_manager
            .fail_task(&task_id, "Something went wrong");

        let app = h::router(state);
        let (status, json, _) = h::get(
            &app,
            &format!("/redfish/v1/TaskService/TaskMonitors/{}", task_id),
        )
        .await;

        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(json["error"], "Something went wrong");
    }

    #[tokio::test]
    async fn test_get_task_monitor_not_found() {
        let app = h::router_with_systems(HashMap::new());

        let (status, json, _) = h::get(&app, "/redfish/v1/TaskService/TaskMonitors/9999").await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(
            json["error"]["message"]
                .as_str()
                .unwrap()
                .contains("not found")
        );
    }
}
