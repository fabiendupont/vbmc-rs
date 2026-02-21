pub mod audit_log;
pub mod registry;
pub mod subscriptions;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedfishEvent {
    #[serde(rename = "EventType")]
    pub event_type: String,
    #[serde(rename = "EventId")]
    pub event_id: String,
    #[serde(rename = "EventTimestamp")]
    pub event_timestamp: DateTime<Utc>,
    #[serde(rename = "MessageId")]
    pub message_id: String,
    #[serde(rename = "Message")]
    pub message: String,
    #[serde(rename = "OriginOfCondition", skip_serializing_if = "Option::is_none")]
    pub origin_of_condition: Option<String>,
    #[serde(rename = "Severity")]
    pub severity: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<serde_json::Value>,
}

pub struct EventBus {
    sender: broadcast::Sender<RedfishEvent>,
}

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    pub fn emit(&self, event: RedfishEvent) {
        let _ = self.sender.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<RedfishEvent> {
        self.sender.subscribe()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new(1024)
    }
}
