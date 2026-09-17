//! Over-the-wire Redfish client for the behavioral harness (twin-facade P6 track b).
//!
//! [`RedfishProbe`] is the *verification face* of the harness: a device-agnostic
//! client that reads a live BMC with plain Redfish (`GET` reads, MetricReports,
//! the `EventService` SSE stream) and hands the results to the shared verifiers in
//! [`crate::scenario`] — `json_contains`, `check_matchspec`, `event_matches` — so
//! the over-the-wire path applies *identical logic* to the in-process replay test.
//!
//! Unlike the in-process path (paused tokio clock, in-memory `event_bus`), the
//! remote clock can't be paused, so SSE events are collected over a bounded
//! wall-clock window ([`RedfishProbe::collect_sse`]); the runner (b-S4) paces steps
//! in real time.

use std::time::Duration;

use futures_util::StreamExt;
use reqwest::Client;
use serde_json::Value;

use super::ObservedEvent;

/// A Redfish client bound to one BMC base URL.
pub struct RedfishProbe {
    base_url: String,
    client: Client,
}

impl RedfishProbe {
    /// Build a probe for `base_url` (e.g. `https://127.0.0.1:8443`). When
    /// `insecure` is set, invalid/self-signed TLS certs are accepted — simulate
    /// mode serves a self-signed cert, so the harness needs this against it.
    pub fn new(base_url: impl Into<String>, insecure: bool) -> Result<Self, String> {
        let mut builder = Client::builder();
        if insecure {
            builder = builder.danger_accept_invalid_certs(true);
        }
        let client = builder
            .build()
            .map_err(|e| format!("build HTTP client: {e}"))?;
        Ok(Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            client,
        })
    }

    fn url(&self, path: &str) -> String {
        format!("{}/{}", self.base_url, path.trim_start_matches('/'))
    }

    /// GET a Redfish resource and parse it as JSON. Errors on any non-2xx status.
    pub async fn get_json(&self, path: &str) -> Result<Value, String> {
        let resp = self
            .client
            .get(self.url(path))
            .send()
            .await
            .map_err(|e| format!("GET {path}: {e}"))?;
        let status = resp.status();
        if !status.is_success() {
            return Err(format!("GET {path}: HTTP {}", status.as_u16()));
        }
        resp.json()
            .await
            .map_err(|e| format!("GET {path}: response is not JSON: {e}"))
    }

    /// Open the `EventService` SSE stream and collect the events emitted within
    /// `window`. Each `data:` frame is a serialized `RedfishEvent` (PascalCase
    /// keys) mapped through [`ObservedEvent::from_sse_json`]; keep-alive comment
    /// frames are skipped. Returns once the window elapses or the stream closes.
    pub async fn collect_sse(&self, window: Duration) -> Result<Vec<ObservedEvent>, String> {
        let resp = self
            .client
            .get(self.url("/redfish/v1/EventService/SSE"))
            .header("accept", "text/event-stream")
            .send()
            .await
            .map_err(|e| format!("open SSE: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("open SSE: HTTP {}", resp.status().as_u16()));
        }

        let mut stream = resp.bytes_stream();
        let mut buf = String::new();
        let mut events = Vec::new();
        let deadline = tokio::time::sleep(window);
        tokio::pin!(deadline);

        loop {
            tokio::select! {
                _ = &mut deadline => break,
                chunk = stream.next() => match chunk {
                    Some(Ok(bytes)) => {
                        buf.push_str(&String::from_utf8_lossy(&bytes));
                        // Frames are separated by a blank line; drain each complete
                        // one and leave any partial tail in the buffer.
                        while let Some(idx) = buf.find("\n\n") {
                            let block: String = buf.drain(..idx + 2).collect();
                            if let Some(ev) = parse_sse_frame(&block) {
                                events.push(ev);
                            }
                        }
                    }
                    Some(Err(e)) => return Err(format!("SSE read: {e}")),
                    None => break,
                },
            }
        }
        Ok(events)
    }
}

/// Parse one SSE frame (the text between blank lines) into an [`ObservedEvent`].
/// Concatenates all `data:` lines (per the SSE spec, one optional leading space is
/// stripped) and parses the result as a `RedfishEvent` JSON object. Returns `None`
/// for keep-alive/comment frames or frames whose data is not valid event JSON.
fn parse_sse_frame(block: &str) -> Option<ObservedEvent> {
    let mut data = String::new();
    for line in block.lines() {
        if let Some(rest) = line.strip_prefix("data:") {
            let rest = rest.strip_prefix(' ').unwrap_or(rest);
            if !data.is_empty() {
                data.push('\n');
            }
            data.push_str(rest);
        }
    }
    if data.trim().is_empty() {
        return None;
    }
    let v: Value = serde_json::from_str(&data).ok()?;
    Some(ObservedEvent::from_sse_json(&v))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use crate::app_state::AppState;
    use crate::auth::accounts::AccountStore;
    use crate::backend::Backend;
    use crate::backend::mockup::{MockupBackend, MockupStore};
    use crate::config::AppConfig;

    fn test_router() -> axum::Router {
        let store = Arc::new(MockupStore::generate(1, 0, false));
        let config = AppConfig::simulate(0);
        let state = Arc::new(AppState::new(
            config,
            Backend::Mockup(MockupBackend::new(store.clone())),
            AccountStore::default(),
            None,
            Some(store),
        ));
        crate::redfish::router(state)
    }

    #[test]
    fn parse_sse_frame_reads_data_line() {
        let block = "data: {\"EventType\":\"Alert\",\"Severity\":\"Critical\",\
             \"MessageId\":\"TwinAlert.1.0.ThresholdCrossed\",\
             \"OriginOfCondition\":\"/redfish/v1/Chassis/GPU_0/Sensors/Temp0\"}\n\n";
        let ev = parse_sse_frame(block).expect("frame parses");
        assert_eq!(ev.event_type, "Alert");
        assert_eq!(ev.severity, "Critical");
        assert_eq!(ev.message_id, "TwinAlert.1.0.ThresholdCrossed");
        assert_eq!(
            ev.origin_of_condition.as_deref(),
            Some("/redfish/v1/Chassis/GPU_0/Sensors/Temp0")
        );
    }

    #[test]
    fn parse_sse_frame_skips_keepalive_and_garbage() {
        assert!(parse_sse_frame(": keep-alive\n\n").is_none());
        assert!(parse_sse_frame("\n\n").is_none());
        assert!(parse_sse_frame("data: not json\n\n").is_none());
    }

    #[tokio::test]
    async fn get_json_reads_over_tcp() {
        let app = test_router();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let probe = RedfishProbe::new(format!("http://{addr}"), false).unwrap();
        let v = probe.get_json("/redfish/v1").await.unwrap();
        assert_eq!(
            v.get("@odata.id").and_then(Value::as_str),
            Some("/redfish/v1")
        );

        // A non-existent path is a non-2xx error, not a parsed body.
        assert!(probe.get_json("/redfish/v1/DoesNotExist").await.is_err());
    }
}
