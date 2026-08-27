pub mod audit_log;
pub mod registry;
pub mod snmp_trap;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_event(msg: &str) -> RedfishEvent {
        RedfishEvent {
            event_type: "StatusChange".to_string(),
            event_id: "1".to_string(),
            event_timestamp: Utc::now(),
            message_id: "Test.1.0.TestMessage".to_string(),
            message: msg.to_string(),
            origin_of_condition: None,
            severity: "OK".to_string(),
            actor: None,
            payload: None,
        }
    }

    #[test]
    fn test_emit_and_receive() {
        let bus = EventBus::default();
        let mut rx = bus.subscribe();

        bus.emit(make_event("hello"));

        let event = rx.try_recv().unwrap();
        assert_eq!(event.message, "hello");
    }

    #[test]
    fn test_multiple_subscribers() {
        let bus = EventBus::default();
        let mut rx1 = bus.subscribe();
        let mut rx2 = bus.subscribe();

        bus.emit(make_event("broadcast"));

        assert_eq!(rx1.try_recv().unwrap().message, "broadcast");
        assert_eq!(rx2.try_recv().unwrap().message, "broadcast");
    }

    #[test]
    fn test_emit_without_subscribers_does_not_panic() {
        let bus = EventBus::default();
        bus.emit(make_event("nobody listening")); // should not panic
    }

    #[test]
    fn test_event_serde_roundtrip() {
        let event = make_event("roundtrip test");
        let json = serde_json::to_string(&event).unwrap();
        let back: RedfishEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(back.message, "roundtrip test");
        assert_eq!(back.event_type, "StatusChange");
        assert_eq!(back.severity, "OK");
    }
}
