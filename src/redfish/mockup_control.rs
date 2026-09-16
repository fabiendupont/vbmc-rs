//! Actuation for mockup/simulate mode (twin-facade P4).
//!
//! P2/P3 carried twin state *in* (ingest) and *out* (SSE / MetricReports); this
//! closes the control loop the other way. When a Redfish client issues a control
//! action against the mockup store (today `ComputerSystem.Reset`), the fallback
//! keeps its local optimistic mutation — an immediate `PowerState` flip — as
//! instant client feedback, then relays a [`ControlIntent`] to the twin's
//! `control_webhook` if one is configured. The twin applies the intent to its
//! model and reflects the authoritative result back through the next ingest.
//!
//! The relay is fire-and-forget: it never blocks the response, and a webhook
//! that is down or slow degrades to local-only behaviour (a warning is logged).
//! With no `control_webhook` in `twin.toml`, behaviour is byte-identical to
//! before this phase.

use tracing::{debug, warn};

use crate::twin::ControlIntent;

/// Relay `intent` to `webhook` without blocking the caller.
///
/// Spawns the POST on the tokio runtime so the Redfish response returns
/// immediately; delivery failures are logged, not surfaced to the client (the
/// local optimistic mutation already gave it feedback).
pub fn emit_control_intent(webhook: String, intent: ControlIntent) {
    tokio::spawn(async move {
        let client = reqwest::Client::new();
        match client.post(&webhook).json(&intent).send().await {
            Ok(resp) => debug!(
                webhook = %webhook,
                status = %resp.status(),
                action = %intent.action,
                "control intent relayed to twin"
            ),
            Err(err) => warn!(
                webhook = %webhook,
                error = %err,
                action = %intent.action,
                "control intent webhook failed; twin left uninformed"
            ),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[tokio::test]
    async fn emit_control_intent_posts_json_to_webhook() {
        // Stand up a one-shot raw-HTTP listener that captures the request the
        // webhook receives, so this exercises the real reqwest POST end to end.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (tx, rx) = tokio::sync::oneshot::channel();

        tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 8192];
            let n = sock.read(&mut buf).await.unwrap();
            let req = String::from_utf8_lossy(&buf[..n]).to_string();
            sock.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
                .await
                .unwrap();
            let _ = tx.send(req);
        });

        emit_control_intent(
            format!("http://{addr}/intents"),
            ControlIntent {
                system_id: "Server1".to_string(),
                path: "/redfish/v1/Systems/Server1".to_string(),
                action: "ComputerSystem.Reset".to_string(),
                params: serde_json::json!({ "ResetType": "ForceOff" }),
            },
        );

        let req = tokio::time::timeout(std::time::Duration::from_secs(5), rx)
            .await
            .expect("webhook not called within timeout")
            .expect("listener dropped without capturing request");

        assert!(
            req.starts_with("POST /intents "),
            "unexpected request: {req}"
        );
        assert!(req.contains(r#""system_id":"Server1""#), "body: {req}");
        assert!(req.contains(r#""path":"/redfish/v1/Systems/Server1""#));
        assert!(req.contains(r#""action":"ComputerSystem.Reset""#));
        assert!(req.contains(r#""ResetType":"ForceOff""#));
    }
}
