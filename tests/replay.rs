//! File-based Redfish sequence replay.
//!
//! Each `tests/sequences/*.json` file is one scenario: an ordered list of HTTP
//! requests replayed against a freshly generated simulate fleet, with the
//! expected status and a partial (subset) match on the JSON response body.
//! Sequences are data, not code — add a scenario by dropping in a file.
//!
//! Body matching is a recursive subset: every key/element named in
//! `body_contains` must be present and equal, so volatile fields (etags,
//! UUIDs) can be omitted. State persists across steps within a file, so a
//! PATCH followed by a GET can assert a transition.
//!
//! Beyond exact matching, a step may assert typed `body_matches` on numeric
//! fields — `range`, `approx`/`tol`, `gte`/`lte`, and `monotonic` (a named
//! series checked across steps). This is what lets a *dynamic* value (a twin
//! reading that moves with time) be asserted without pinning an exact number
//! (twin-facade P6 S4).
//!
//! Twin sequences under `tests/sequences/twin/` add the deterministic-clock
//! dimension: each file carries an inline `twin.toml` and a small resource
//! `fixture`, and its steps `advance_seconds` of virtual time (the runtime is
//! paused, so no wall-clock sleeping) before asserting the resolved read path,
//! the refreshed MetricReport, and the events/alerts the stream emitted.
//!
//! The scenario schema and pure verifiers live in `vbmc_rs::scenario` (the
//! *identical verification half*) so the over-the-wire `scenario-harness` binary
//! (twin-facade P6 track (b)) replays the same files with the same matchers.

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::Request;
use serde::Deserialize;
use serde_json::Value;
use tower::ServiceExt;

use vbmc_rs::app_state::AppState;
use vbmc_rs::auth::accounts::AccountStore;
use vbmc_rs::backend::Backend;
use vbmc_rs::backend::mockup::{MockupBackend, MockupStore};
use vbmc_rs::config::AppConfig;
use vbmc_rs::redfish::mockup_stream::spawn_stream;
use vbmc_rs::scenario::{
    ExpectSpec, ObservedEvent, RequestSpec, TwinSequence, check_matchspec, event_matches,
    json_contains, json_path_present,
};

#[derive(Debug, Deserialize)]
struct Sequence {
    description: String,
    #[serde(default = "one")]
    systems: usize,
    steps: Vec<Step>,
}

#[derive(Debug, Deserialize)]
struct Step {
    name: String,
    request: RequestSpec,
    expect: ExpectSpec,
}

fn one() -> usize {
    1
}

/// Build a router over a freshly generated `systems`-server simulate fleet.
fn build_app(systems: usize) -> Router {
    let store = Arc::new(MockupStore::generate(systems, 0, false));
    let config = AppConfig::simulate(0);
    let state = Arc::new(AppState::new(
        config,
        Backend::Mockup(MockupBackend::new(store.clone())),
        AccountStore::default(),
        None,
        Some(store),
    ));
    vbmc_rs::redfish::router(state)
}

/// Issue one request against the app and return `(status, body_bytes)`.
async fn send_request(app: &Router, req: &RequestSpec) -> (u16, Vec<u8>) {
    let mut builder = Request::builder()
        .method(req.method.as_str())
        .uri(&req.path);
    for (k, v) in &req.headers {
        builder = builder.header(k, v);
    }
    let body = match &req.body {
        Some(json) => {
            builder = builder.header("content-type", "application/json");
            Body::from(serde_json::to_vec(json).unwrap())
        }
        None => Body::empty(),
    };
    let request = builder.body(body).unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status().as_u16();
    let bytes = axum::body::to_bytes(response.into_body(), 4_000_000)
        .await
        .unwrap();
    (status, bytes.to_vec())
}

/// Assert a response against a step's expectations. `series` carries cross-step
/// monotonic state.
fn assert_expect(
    file: &str,
    name: &str,
    status: u16,
    bytes: &[u8],
    expect: &ExpectSpec,
    series: &mut HashMap<String, f64>,
) {
    assert_eq!(
        status,
        expect.status,
        "[{file}] step '{name}': status mismatch (body: {})",
        String::from_utf8_lossy(bytes)
    );

    if expect.body_contains.is_some()
        || expect.body_equals.is_some()
        || expect.body_lacks.is_some()
        || expect.body_matches.is_some()
    {
        let actual: Value = serde_json::from_slice(bytes)
            .unwrap_or_else(|e| panic!("[{file}] step '{name}': response is not JSON: {e}"));
        if let Some(expected) = &expect.body_contains
            && let Err(loc) = json_contains(&actual, expected)
        {
            panic!("[{file}] step '{name}': body mismatch at {loc}");
        }
        if let Some(expected) = &expect.body_equals {
            assert_eq!(
                &actual, expected,
                "[{file}] step '{name}': body_equals mismatch (actual: {actual})"
            );
        }
        if let Some(paths) = &expect.body_lacks {
            for p in paths {
                if json_path_present(&actual, p) {
                    panic!(
                        "[{file}] step '{name}': expected path '{p}' to be absent (actual: {actual})"
                    );
                }
            }
        }
        if let Some(specs) = &expect.body_matches {
            for spec in specs {
                if let Err(e) = check_matchspec(&actual, spec, series) {
                    panic!("[{file}] step '{name}': matcher failed: {e}");
                }
            }
        }
    }
}

async fn replay(path: &Path) {
    let raw = fs::read_to_string(path).expect("read sequence file");
    let seq: Sequence = serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("{}: invalid sequence file: {e}", path.display()));
    let app = build_app(seq.systems);
    let file = path.file_name().unwrap().to_string_lossy();
    eprintln!("replaying {file}: {}", seq.description);

    let mut series: HashMap<String, f64> = HashMap::new();
    for step in &seq.steps {
        let (status, bytes) = send_request(&app, &step.request).await;
        assert_expect(&file, &step.name, status, &bytes, &step.expect, &mut series);
    }
}

#[tokio::test]
async fn replay_sequences() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/sequences");
    let mut files: Vec<_> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("read {}: {e}", dir.display()))
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no sequence files in {}", dir.display());

    for path in &files {
        replay(path).await;
    }
    eprintln!("replayed {} sequence file(s)", files.len());
}

// --- Twin sequences: deterministic virtual clock + event/alert assertions ---

/// Write the fixture + twin.toml to a tempdir, load a store, and build a router
/// with the stream tick spawned. The tempdir is returned so it outlives the load
/// (it is read into memory, but keeping it avoids surprises).
fn build_twin_app(
    twin_toml: &str,
    fixture: &HashMap<String, Value>,
) -> (Router, Arc<AppState>, tempfile::TempDir) {
    let dir = tempfile::TempDir::new().unwrap();
    for (path, body) in fixture {
        let resource_dir = dir.path().join(path.trim_start_matches('/'));
        std::fs::create_dir_all(&resource_dir).unwrap();
        std::fs::write(
            resource_dir.join("index.json"),
            serde_json::to_vec(body).unwrap(),
        )
        .unwrap();
    }
    std::fs::write(dir.path().join("twin.toml"), twin_toml).unwrap();

    let store = Arc::new(MockupStore::load(dir.path()).unwrap());
    let config = AppConfig::simulate(0);
    let state = Arc::new(AppState::new(
        config,
        Backend::Mockup(MockupBackend::new(store.clone())),
        AccountStore::default(),
        None,
        Some(store),
    ));
    let app = vbmc_rs::redfish::router(state.clone());
    (app, state, dir)
}

/// Let the paused runtime run the spawned stream task so any ticks whose timers
/// just elapsed actually fire (and emit) before we drain events.
async fn flush_ticks() {
    for _ in 0..16 {
        tokio::task::yield_now().await;
    }
}

async fn replay_twin(path: &Path) {
    let raw = fs::read_to_string(path).expect("read twin sequence file");
    let seq: TwinSequence = serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("{}: invalid twin sequence file: {e}", path.display()));
    let file = path.file_name().unwrap().to_string_lossy();
    eprintln!("replaying twin {file}: {}", seq.description);

    let (app, state, _dir) = build_twin_app(&seq.twin, &seq.fixture);
    // Subscribe before the stream starts so no emitted event is missed.
    let mut rx = state.event_bus.subscribe();
    spawn_stream(state.clone());

    let mut series: HashMap<String, f64> = HashMap::new();
    for step in &seq.steps {
        if step.advance_seconds > 0.0 {
            tokio::time::advance(Duration::from_secs_f64(step.advance_seconds)).await;
        }
        flush_ticks().await;

        if let Some(req) = &step.request {
            let (status, bytes) = send_request(&app, req).await;
            let expect = step.expect.as_ref().unwrap_or_else(|| {
                panic!(
                    "[{file}] step '{}': a request step needs `expect`",
                    step.name
                )
            });
            assert_expect(&file, &step.name, status, &bytes, expect, &mut series);
        }

        if let Some(wanted) = &step.expect_events {
            let mut drained = Vec::new();
            while let Ok(ev) = rx.try_recv() {
                drained.push(ev);
            }
            let observed: Vec<ObservedEvent> = drained
                .iter()
                .map(ObservedEvent::from_redfish_event)
                .collect();
            for want in wanted {
                assert!(
                    observed.iter().any(|ev| event_matches(ev, want)),
                    "[{file}] step '{}': no event matched {want:?} (saw {})",
                    step.name,
                    observed
                        .iter()
                        .map(|e| format!("{}/{}/{}", e.event_type, e.severity, e.message_id))
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
        }
    }
}

#[tokio::test(start_paused = true)]
async fn replay_twin_sequences() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/sequences/twin");
    let mut files: Vec<_> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("read {}: {e}", dir.display()))
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    files.sort();
    assert!(
        !files.is_empty(),
        "no twin sequence files in {}",
        dir.display()
    );

    for path in &files {
        replay_twin(path).await;
    }
    eprintln!("replayed {} twin sequence file(s)", files.len());
}
