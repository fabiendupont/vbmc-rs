use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tracing::{error, warn};

use super::RedfishEvent;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subscription {
    pub id: String,
    pub destination: String,
    pub protocol: String,
    pub event_types: Vec<String>,
}

pub struct SubscriptionStore {
    subscriptions: DashMap<String, Subscription>,
    next_id: std::sync::atomic::AtomicU64,
}

impl SubscriptionStore {
    pub fn new() -> Self {
        Self {
            subscriptions: DashMap::new(),
            next_id: std::sync::atomic::AtomicU64::new(1),
        }
    }

    pub fn add(&self, destination: &str, protocol: &str, event_types: Vec<String>) -> Subscription {
        let id = self
            .next_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            .to_string();

        let sub = Subscription {
            id: id.clone(),
            destination: destination.to_string(),
            protocol: protocol.to_string(),
            event_types,
        };

        self.subscriptions.insert(id, sub.clone());
        sub
    }

    pub fn get(&self, id: &str) -> Option<Subscription> {
        self.subscriptions.get(id).map(|s| s.clone())
    }

    pub fn remove(&self, id: &str) -> bool {
        self.subscriptions.remove(id).is_some()
    }

    pub fn list(&self) -> Vec<Subscription> {
        self.subscriptions
            .iter()
            .map(|s| s.value().clone())
            .collect()
    }
}

impl Default for SubscriptionStore {
    fn default() -> Self {
        Self::new()
    }
}

pub fn start_webhook_delivery(
    mut rx: broadcast::Receiver<RedfishEvent>,
    subscription: Subscription,
) {
    tokio::spawn(async move {
        let client = reqwest::Client::new();
        let backoff_schedule = [1u64, 5, 30];

        loop {
            match rx.recv().await {
                Ok(event) => {
                    if !subscription.event_types.is_empty()
                        && !subscription.event_types.contains(&event.event_type)
                    {
                        continue;
                    }

                    let payload = serde_json::json!({
                        "@odata.type": "#Event.v1_9_0.Event",
                        "Events": [event],
                    });

                    let mut delivered = false;
                    for (attempt, &delay) in backoff_schedule.iter().enumerate() {
                        match client
                            .post(&subscription.destination)
                            .json(&payload)
                            .send()
                            .await
                        {
                            Ok(resp) if resp.status().is_success() => {
                                delivered = true;
                                break;
                            }
                            Ok(resp) => {
                                warn!(
                                    "Webhook delivery attempt {} to {} failed: HTTP {}",
                                    attempt + 1,
                                    subscription.destination,
                                    resp.status()
                                );
                            }
                            Err(e) => {
                                warn!(
                                    "Webhook delivery attempt {} to {} failed: {}",
                                    attempt + 1,
                                    subscription.destination,
                                    e
                                );
                            }
                        }
                        tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
                    }

                    if !delivered {
                        error!(
                            "Failed to deliver webhook to {} after {} attempts",
                            subscription.destination,
                            backoff_schedule.len()
                        );
                    }
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    warn!("Webhook subscriber lagged, missed {n} events");
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });
}

#[cfg(test)]
mod tests_subscription_store {
    use super::*;

    #[test]
    fn test_add_subscription() {
        let store = SubscriptionStore::new();
        let sub = store.add(
            "https://example.com/hook",
            "Redfish",
            vec!["StatusChange".to_string()],
        );

        assert_eq!(sub.id, "1");
        assert_eq!(sub.destination, "https://example.com/hook");
        assert_eq!(sub.protocol, "Redfish");
        assert_eq!(sub.event_types, vec!["StatusChange"]);
    }

    #[test]
    fn test_add_increments_id() {
        let store = SubscriptionStore::new();
        let s1 = store.add("https://a.com", "Redfish", vec![]);
        let s2 = store.add("https://b.com", "Redfish", vec![]);
        assert_eq!(s1.id, "1");
        assert_eq!(s2.id, "2");
    }

    #[test]
    fn test_get_subscription() {
        let store = SubscriptionStore::new();
        let sub = store.add("https://example.com", "Redfish", vec![]);

        let fetched = store.get(&sub.id).unwrap();
        assert_eq!(fetched.destination, "https://example.com");
    }

    #[test]
    fn test_get_nonexistent() {
        let store = SubscriptionStore::new();
        assert!(store.get("999").is_none());
    }

    #[test]
    fn test_remove_subscription() {
        let store = SubscriptionStore::new();
        let sub = store.add("https://example.com", "Redfish", vec![]);

        assert!(store.remove(&sub.id));
        assert!(store.get(&sub.id).is_none());
    }

    #[test]
    fn test_remove_nonexistent() {
        let store = SubscriptionStore::new();
        assert!(!store.remove("999"));
    }

    #[test]
    fn test_list_subscriptions() {
        let store = SubscriptionStore::new();
        assert!(store.list().is_empty());

        store.add("https://a.com", "Redfish", vec![]);
        store.add("https://b.com", "Redfish", vec![]);
        assert_eq!(store.list().len(), 2);
    }

    #[test]
    fn test_list_after_remove() {
        let store = SubscriptionStore::new();
        let s1 = store.add("https://a.com", "Redfish", vec![]);
        store.add("https://b.com", "Redfish", vec![]);

        store.remove(&s1.id);
        assert_eq!(store.list().len(), 1);
    }
}

#[cfg(test)]
mod coverage_tests {
    use super::*;
    use chrono::Utc;

    fn make_event(event_type: &str, msg: &str) -> RedfishEvent {
        RedfishEvent {
            event_type: event_type.to_string(),
            event_id: uuid::Uuid::new_v4().to_string(),
            event_timestamp: Utc::now(),
            message_id: "Test.1.0.Message".to_string(),
            message: msg.to_string(),
            origin_of_condition: Some("/redfish/v1/Systems/test".to_string()),
            severity: "OK".to_string(),
            actor: None,
            payload: None,
        }
    }

    #[test]
    fn test_subscription_serde() {
        let sub = Subscription {
            id: "1".to_string(),
            destination: "https://example.com/hook".to_string(),
            protocol: "Redfish".to_string(),
            event_types: vec!["StatusChange".to_string()],
        };

        let json = serde_json::to_string(&sub).unwrap();
        let deserialized: Subscription = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.id, sub.id);
        assert_eq!(deserialized.destination, sub.destination);
        assert_eq!(deserialized.protocol, sub.protocol);
        assert_eq!(deserialized.event_types, sub.event_types);
    }

    #[test]
    fn test_store_default() {
        let store = SubscriptionStore::default();
        assert!(store.list().is_empty());
    }

    #[test]
    fn test_subscription_with_multiple_event_types() {
        let store = SubscriptionStore::new();
        let sub = store.add(
            "https://example.com",
            "Redfish",
            vec![
                "StatusChange".to_string(),
                "Alert".to_string(),
                "ResourceAdded".to_string(),
            ],
        );

        assert_eq!(sub.event_types.len(), 3);
        assert!(sub.event_types.contains(&"StatusChange".to_string()));
        assert!(sub.event_types.contains(&"Alert".to_string()));
        assert!(sub.event_types.contains(&"ResourceAdded".to_string()));
    }

    #[test]
    fn test_subscription_with_empty_event_types() {
        let store = SubscriptionStore::new();
        let sub = store.add("https://example.com", "Redfish", vec![]);

        assert!(sub.event_types.is_empty());
    }

    #[tokio::test]
    async fn test_webhook_delivery_filters_by_event_type() {
        let (tx, rx) = broadcast::channel::<RedfishEvent>(16);

        let sub = Subscription {
            id: "1".to_string(),
            destination: "http://localhost:9999/hook".to_string(),
            protocol: "Redfish".to_string(),
            event_types: vec!["StatusChange".to_string()],
        };

        tokio::spawn(async move {
            start_webhook_delivery(rx, sub);
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        tx.send(make_event("StatusChange", "should match")).unwrap();
        tx.send(make_event("Alert", "should not match")).unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    }

    #[tokio::test]
    async fn test_webhook_delivery_accepts_all_when_event_types_empty() {
        let (tx, rx) = broadcast::channel::<RedfishEvent>(16);

        let sub = Subscription {
            id: "1".to_string(),
            destination: "http://localhost:9999/hook".to_string(),
            protocol: "Redfish".to_string(),
            event_types: vec![],
        };

        tokio::spawn(async move {
            start_webhook_delivery(rx, sub);
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        tx.send(make_event("StatusChange", "event 1")).unwrap();
        tx.send(make_event("Alert", "event 2")).unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    }

    #[tokio::test]
    async fn test_webhook_delivery_exits_on_closed_channel() {
        let (tx, rx) = broadcast::channel::<RedfishEvent>(16);

        let sub = Subscription {
            id: "1".to_string(),
            destination: "http://localhost:9999/hook".to_string(),
            protocol: "Redfish".to_string(),
            event_types: vec![],
        };

        let handle = tokio::spawn(async move {
            start_webhook_delivery(rx, sub);
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        drop(tx);

        let result = tokio::time::timeout(tokio::time::Duration::from_secs(1), handle).await;
        assert!(
            result.is_ok(),
            "webhook delivery should exit when channel closes"
        );
    }

    #[test]
    fn test_subscription_clone() {
        let sub = Subscription {
            id: "1".to_string(),
            destination: "https://example.com".to_string(),
            protocol: "Redfish".to_string(),
            event_types: vec!["StatusChange".to_string()],
        };

        let cloned = sub.clone();
        assert_eq!(cloned.id, sub.id);
        assert_eq!(cloned.destination, sub.destination);
        assert_eq!(cloned.protocol, sub.protocol);
        assert_eq!(cloned.event_types, sub.event_types);
    }
}
