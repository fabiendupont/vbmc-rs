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

    pub fn add(
        &self,
        destination: &str,
        protocol: &str,
        event_types: Vec<String>,
    ) -> Subscription {
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
                    // Filter by event type if specified
                    if !subscription.event_types.is_empty()
                        && !subscription
                            .event_types
                            .contains(&event.event_type)
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
