use std::sync::atomic::{AtomicU64, Ordering};

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum TaskState {
    New,
    Running,
    Completed,
    Exception,
    Killed,
}

#[derive(Debug, Clone, Serialize)]
pub struct Task {
    #[serde(rename = "Id")]
    pub id: String,
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "TaskState")]
    pub task_state: TaskState,
    #[serde(rename = "TaskStatus")]
    pub task_status: String,
    #[serde(rename = "StartTime")]
    pub start_time: DateTime<Utc>,
    #[serde(rename = "EndTime", skip_serializing_if = "Option::is_none")]
    pub end_time: Option<DateTime<Utc>>,
    #[serde(rename = "PercentComplete", skip_serializing_if = "Option::is_none")]
    pub percent_complete: Option<u8>,
    #[serde(rename = "Messages", skip_serializing_if = "Vec::is_empty")]
    pub messages: Vec<String>,
    #[serde(skip)]
    pub result: Option<serde_json::Value>,
}

pub struct TaskManager {
    tasks: DashMap<String, Task>,
    next_id: AtomicU64,
}

impl TaskManager {
    pub fn new() -> Self {
        Self {
            tasks: DashMap::new(),
            next_id: AtomicU64::new(1),
        }
    }

    pub fn create_task(&self, name: &str) -> String {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed).to_string();
        let task = Task {
            id: id.clone(),
            name: name.to_string(),
            task_state: TaskState::Running,
            task_status: "OK".to_string(),
            start_time: Utc::now(),
            end_time: None,
            percent_complete: Some(0),
            messages: Vec::new(),
            result: None,
        };
        self.tasks.insert(id.clone(), task);
        id
    }

    pub fn complete_task(&self, id: &str, result: Option<serde_json::Value>) {
        if let Some(mut task) = self.tasks.get_mut(id) {
            task.task_state = TaskState::Completed;
            task.end_time = Some(Utc::now());
            task.percent_complete = Some(100);
            task.result = result;
        }
    }

    pub fn fail_task(&self, id: &str, message: &str) {
        if let Some(mut task) = self.tasks.get_mut(id) {
            task.task_state = TaskState::Exception;
            task.task_status = "Critical".to_string();
            task.end_time = Some(Utc::now());
            task.messages.push(message.to_string());
        }
    }

    pub fn get_task(&self, id: &str) -> Option<Task> {
        self.tasks.get(id).map(|t| t.clone())
    }

    pub fn list_tasks(&self) -> Vec<Task> {
        self.tasks.iter().map(|t| t.value().clone()).collect()
    }
}

impl Default for TaskManager {
    fn default() -> Self {
        Self::new()
    }
}
