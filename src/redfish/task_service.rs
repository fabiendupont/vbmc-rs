use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

use super::types::{Collection, ODataId};
use crate::app_state::AppState;
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
}

pub async fn get_task_service() -> Json<TaskServiceResource> {
    Json(TaskServiceResource {
        odata_id: "/redfish/v1/TaskService",
        odata_type: "#TaskService.v1_2_0.TaskService",
        id: "TaskService",
        name: "Task Service",
        description: "Task management service",
        service_enabled: true,
        tasks: ODataId::new("/redfish/v1/TaskService/Tasks"),
    })
}

pub async fn get_tasks(
    State(state): State<Arc<AppState>>,
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
        task_monitor: Some(format!(
            "/redfish/v1/TaskService/TaskMonitors/{}",
            task.id
        )),
    }))
}

pub async fn get_task_monitor(
    State(state): State<Arc<AppState>>,
    Path(task_id): Path<String>,
) -> Result<Response, RedfishApiError> {
    let task = state
        .task_manager
        .get_task(&task_id)
        .ok_or_else(|| RedfishApiError::NotFound(format!("Task '{task_id}' not found")))?;

    match task.task_state {
        TaskState::Completed => {
            let result = task.result.unwrap_or(serde_json::json!({"message": "Task completed"}));
            Ok((StatusCode::OK, Json(result)).into_response())
        }
        TaskState::Exception | TaskState::Killed => {
            let msg = task.messages.first().cloned().unwrap_or_else(|| "Task failed".to_string());
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
