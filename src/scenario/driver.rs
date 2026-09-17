//! Stimulus drivers for the behavioral harness (twin-facade P6 track b).
//!
//! The stimulus/observation split: the *verification* face ([`super::probe`]) is
//! standard Redfish and device-agnostic, but the *stimulus* face is simulator-
//! specific — how you "press inject" depends on the device under test. That
//! variation is isolated behind [`StimulusDriver`], so the runner (b-S4) picks a
//! driver once and drives every step through the same three verbs.
//!
//! Two drivers ship:
//!
//! - [`TwinIngestDriver`] drives a vbmc-rs simulate instance through its twin
//!   control-plane (`POST /twin/v1/scenario/{name}` to arm, `POST /twin/v1/state`
//!   to ingest, `DELETE /twin/v1/scenario` to reset).
//! - [`ObserveOnlyDriver`] is a genuine no-op for real hardware: an operator
//!   injects stimulus out-of-band (a bench, a thermal chamber), so the harness
//!   only observes. It logs that stimulus is external and returns `Ok`.

use std::future::Future;

use reqwest::Client;
use serde_json::{Map, Value, json};

use super::Sample;

/// The stimulus face of the harness. Each verb is a no-op-able step: a driver may
/// legitimately do nothing (see [`ObserveOnlyDriver`]).
///
/// Methods are desugared to `-> impl Future + Send` (rather than `async fn`) so
/// the returned futures carry an explicit `Send` bound — the runner may run steps
/// on any executor. Implementations still write them as `async fn`.
pub trait StimulusDriver {
    /// Arm a named scenario (re-base its timeline to now).
    fn arm(&self, scenario: &str) -> impl Future<Output = Result<(), String>> + Send;
    /// Inject external-twin samples.
    fn ingest(&self, samples: &[Sample]) -> impl Future<Output = Result<(), String>> + Send;
    /// Disarm every scenario, reverting timelines to the store start.
    fn reset(&self) -> impl Future<Output = Result<(), String>> + Send;
}

/// Drives a live vbmc-rs simulate instance through its twin control-plane.
pub struct TwinIngestDriver {
    base_url: String,
    client: Client,
}

impl TwinIngestDriver {
    /// Build a driver for `base_url` (e.g. `https://127.0.0.1:8443`). `insecure`
    /// accepts self-signed TLS, matching [`super::probe::RedfishProbe::new`].
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
}

impl StimulusDriver for TwinIngestDriver {
    async fn arm(&self, scenario: &str) -> Result<(), String> {
        let resp = self
            .client
            .post(self.url(&format!("/twin/v1/scenario/{scenario}")))
            .send()
            .await
            .map_err(|e| format!("arm '{scenario}': {e}"))?;
        match resp.status().as_u16() {
            200 => Ok(()),
            404 => Err(format!(
                "arm '{scenario}': unknown scenario (404) — no binding drives it"
            )),
            other => Err(format!("arm '{scenario}': HTTP {other}")),
        }
    }

    async fn ingest(&self, samples: &[Sample]) -> Result<(), String> {
        // Serialize to the fleet-array form `/twin/v1/state` accepts:
        // `[{ system_id?, key, value }]`.
        let body: Vec<Value> = samples
            .iter()
            .map(|s| {
                let mut obj = Map::new();
                if let Some(sid) = &s.system_id {
                    obj.insert("system_id".into(), json!(sid));
                }
                obj.insert("key".into(), json!(s.key));
                obj.insert("value".into(), s.value.clone());
                Value::Object(obj)
            })
            .collect();
        let resp = self
            .client
            .post(self.url("/twin/v1/state"))
            .json(&Value::Array(body))
            .send()
            .await
            .map_err(|e| format!("ingest: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("ingest: HTTP {}", resp.status().as_u16()));
        }
        Ok(())
    }

    async fn reset(&self) -> Result<(), String> {
        let resp = self
            .client
            .delete(self.url("/twin/v1/scenario"))
            .send()
            .await
            .map_err(|e| format!("reset: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("reset: HTTP {}", resp.status().as_u16()));
        }
        Ok(())
    }
}

/// A no-op driver for real hardware: the operator injects stimulus out-of-band, so
/// the harness only observes. Every verb logs that stimulus is external and
/// succeeds, proving the driver seam end-to-end without a physics engine.
pub struct ObserveOnlyDriver;

impl StimulusDriver for ObserveOnlyDriver {
    async fn arm(&self, scenario: &str) -> Result<(), String> {
        tracing::info!(
            scenario,
            "observe-only: stimulus is external (operator arms out-of-band); skipping arm"
        );
        Ok(())
    }

    async fn ingest(&self, samples: &[Sample]) -> Result<(), String> {
        tracing::info!(
            samples = samples.len(),
            "observe-only: stimulus is external; skipping ingest"
        );
        Ok(())
    }

    async fn reset(&self) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Arc;

    use crate::app_state::AppState;
    use crate::auth::accounts::AccountStore;
    use crate::backend::Backend;
    use crate::backend::mockup::{MockupBackend, MockupStore};
    use crate::config::AppConfig;

    /// A twin fixture with a scenario (`spike`, so `arm` has something to re-base)
    /// and an external binding on key `gpu.temp` (so `ingest` is accepted).
    const TWIN_TOML: &str = r#"
[twin]
tick_interval_seconds = 5

[[twin.binding]]
path = "/redfish/v1/Chassis/GPU_0/Sensors/Temp0"
pointer = "/Reading"
source = "scenario"
scenario = "spike"

[[twin.binding]]
path = "/redfish/v1/Chassis/GPU_0/Sensors/Temp1"
pointer = "/Reading"
source = "external"
key = "gpu.temp"
min = 0.0
max = 120.0

[[scenario]]
name = "spike"
[[scenario.segment]]
kind = "nominal"
value = 40.0
for_s = 10
[[scenario.segment]]
kind = "drift"
value = 90.0
"#;

    fn fixture() -> HashMap<String, Value> {
        let mut f = HashMap::new();
        for path in [
            "/redfish/v1/Chassis/GPU_0/Sensors/Temp0",
            "/redfish/v1/Chassis/GPU_0/Sensors/Temp1",
        ] {
            f.insert(
                path.to_string(),
                json!({ "@odata.id": path, "Reading": 0.0 }),
            );
        }
        f
    }

    /// Serve a twin-backed router over an ephemeral TCP port; returns its base URL.
    /// The tempdir is returned so it outlives the (in-memory) store load.
    async fn serve_twin() -> (String, tempfile::TempDir) {
        let dir = tempfile::TempDir::new().unwrap();
        for (path, body) in fixture() {
            let resource_dir = dir.path().join(path.trim_start_matches('/'));
            std::fs::create_dir_all(&resource_dir).unwrap();
            std::fs::write(
                resource_dir.join("index.json"),
                serde_json::to_vec(&body).unwrap(),
            )
            .unwrap();
        }
        std::fs::write(dir.path().join("twin.toml"), TWIN_TOML).unwrap();

        let store = Arc::new(MockupStore::load(dir.path()).unwrap());
        let config = AppConfig::simulate(0);
        let state = Arc::new(AppState::new(
            config,
            Backend::Mockup(MockupBackend::new(store.clone())),
            AccountStore::default(),
            None,
            Some(store),
        ));
        let app = crate::redfish::router(state);

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        (format!("http://{addr}"), dir)
    }

    /// Generic over the trait: proves dispatch works for any driver.
    async fn drive<D: StimulusDriver>(d: &D, samples: &[Sample]) -> Result<(), String> {
        d.arm("spike").await?;
        d.ingest(samples).await?;
        d.reset().await
    }

    #[tokio::test]
    async fn twin_driver_arm_ingest_reset_land() {
        let (base, _dir) = serve_twin().await;
        let driver = TwinIngestDriver::new(base, false).unwrap();
        let samples = vec![Sample {
            system_id: None,
            key: "gpu.temp".to_string(),
            value: json!(75.0),
        }];
        drive(&driver, &samples).await.expect("all verbs land");
    }

    #[tokio::test]
    async fn twin_driver_arm_unknown_scenario_is_error() {
        let (base, _dir) = serve_twin().await;
        let driver = TwinIngestDriver::new(base, false).unwrap();
        let err = driver.arm("nope").await.unwrap_err();
        assert!(err.contains("404"), "expected 404 error, got: {err}");
    }

    #[tokio::test]
    async fn observe_only_driver_is_noop() {
        let driver = ObserveOnlyDriver;
        let samples = vec![Sample {
            system_id: Some("node-1".to_string()),
            key: "gpu.temp".to_string(),
            value: json!(75.0),
        }];
        // No server: every verb must still succeed (it does nothing).
        drive(&driver, &samples).await.expect("no-op succeeds");
    }
}
