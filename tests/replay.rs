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
use vbmc_rs::events::RedfishEvent;
use vbmc_rs::redfish::mockup_stream::spawn_stream;

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

#[derive(Debug, Deserialize)]
struct RequestSpec {
    method: String,
    path: String,
    #[serde(default)]
    headers: std::collections::HashMap<String, String>,
    #[serde(default)]
    body: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct ExpectSpec {
    status: u16,
    #[serde(default)]
    body_contains: Option<Value>,
    #[serde(default)]
    body_equals: Option<Value>,
    /// Dot-delimited paths (e.g. `Attributes.BootMode`) that must NOT be present.
    /// Lets a sequence assert a reset/delete removed state without pinning the
    /// whole document (which carries volatile etags).
    #[serde(default)]
    body_lacks: Option<Vec<String>>,
    /// Typed numeric matchers applied at dot-delimited paths (twin-facade P6 S4).
    /// Numbers rendered as JSON strings (e.g. a MetricReport `MetricValue`) are
    /// parsed, so the same matcher works on Readings and MetricValues alike.
    #[serde(default)]
    body_matches: Option<Vec<MatchSpec>>,
}

/// One typed matcher on a numeric field. Any subset of the constraints may be
/// present; all present constraints must hold. `monotonic` records the value
/// into a named cross-step series (defaulting to `path`) and asserts the
/// direction against the previously recorded value.
#[derive(Debug, Deserialize)]
struct MatchSpec {
    path: String,
    /// Inclusive `[min, max]`.
    #[serde(default)]
    range: Option<[f64; 2]>,
    /// Target value; paired with `tol` (default 1e-6).
    #[serde(default)]
    approx: Option<f64>,
    #[serde(default)]
    tol: Option<f64>,
    #[serde(default)]
    gte: Option<f64>,
    #[serde(default)]
    lte: Option<f64>,
    #[serde(default)]
    monotonic: Option<Direction>,
    #[serde(default)]
    series: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Direction {
    Increasing,
    Decreasing,
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

/// Recursive subset match: every key/element in `expected` must be present in
/// `actual` and match. Returns a JSON-path-ish location string on mismatch.
fn json_contains(actual: &Value, expected: &Value) -> Result<(), String> {
    match (actual, expected) {
        (Value::Object(a), Value::Object(e)) => {
            for (k, ev) in e {
                match a.get(k) {
                    Some(av) => json_contains(av, ev).map_err(|loc| format!(".{k}{loc}"))?,
                    None => return Err(format!(".{k} (missing)")),
                }
            }
            Ok(())
        }
        (Value::Array(a), Value::Array(e)) => {
            for (i, ev) in e.iter().enumerate() {
                match a.get(i) {
                    Some(av) => json_contains(av, ev).map_err(|loc| format!("[{i}]{loc}"))?,
                    None => return Err(format!("[{i}] (missing)")),
                }
            }
            Ok(())
        }
        _ if actual == expected => Ok(()),
        _ => Err(format!(" (expected {expected}, got {actual})")),
    }
}

/// Resolve a dot-delimited path, descending into objects by key and into arrays
/// by numeric index (e.g. `MetricValues.0.MetricValue`).
fn json_path_get<'a>(v: &'a Value, dotted: &str) -> Option<&'a Value> {
    let mut cur = v;
    for seg in dotted.split('.') {
        cur = match cur {
            Value::Array(arr) => arr.get(seg.parse::<usize>().ok()?)?,
            _ => cur.get(seg)?,
        };
    }
    Some(cur)
}

/// Whether a dot-delimited path resolves to a value in `v`.
fn json_path_present(v: &Value, dotted: &str) -> bool {
    json_path_get(v, dotted).is_some()
}

/// Coerce a JSON value to a number, accepting numbers rendered as strings (a
/// Redfish `MetricValue` is a string even when it carries a number).
fn as_number(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.parse().ok(),
        _ => None,
    }
}

/// Evaluate one typed matcher against the response body.
fn check_matchspec(
    actual: &Value,
    spec: &MatchSpec,
    series: &mut HashMap<String, f64>,
) -> Result<(), String> {
    let val =
        json_path_get(actual, &spec.path).ok_or_else(|| format!("path '{}' missing", spec.path))?;
    let num =
        as_number(val).ok_or_else(|| format!("path '{}' is not numeric: {val}", spec.path))?;

    if let Some([lo, hi]) = spec.range
        && (num < lo || num > hi)
    {
        return Err(format!("{}={num} not in [{lo}, {hi}]", spec.path));
    }
    if let Some(a) = spec.approx {
        let tol = spec.tol.unwrap_or(1e-6);
        if (num - a).abs() > tol {
            return Err(format!("{}={num} not ≈ {a} (tol {tol})", spec.path));
        }
    }
    if let Some(g) = spec.gte
        && num < g
    {
        return Err(format!("{}={num} < {g}", spec.path));
    }
    if let Some(l) = spec.lte
        && num > l
    {
        return Err(format!("{}={num} > {l}", spec.path));
    }
    if let Some(dir) = &spec.monotonic {
        let name = spec.series.clone().unwrap_or_else(|| spec.path.clone());
        if let Some(prev) = series.get(&name) {
            let ok = match dir {
                Direction::Increasing => num >= *prev,
                Direction::Decreasing => num <= *prev,
            };
            if !ok {
                return Err(format!(
                    "{}={num} breaks {dir:?} series '{name}' (previous {prev})",
                    spec.path
                ));
            }
        }
        series.insert(name, num);
    }
    Ok(())
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

#[derive(Debug, Deserialize)]
struct TwinSequence {
    description: String,
    /// Inline `twin.toml` (bindings + scenarios) driving the store.
    twin: String,
    /// Resource tree by Redfish path — each becomes an `index.json` the store
    /// loads, so the bound read path resolves.
    #[serde(default)]
    fixture: HashMap<String, Value>,
    steps: Vec<TwinStep>,
}

#[derive(Debug, Deserialize)]
struct TwinStep {
    name: String,
    /// Virtual seconds to advance before this step (the runtime is paused).
    #[serde(default)]
    advance_seconds: f64,
    #[serde(default)]
    request: Option<RequestSpec>,
    #[serde(default)]
    expect: Option<ExpectSpec>,
    /// Events that must have been emitted since the previous step.
    #[serde(default)]
    expect_events: Option<Vec<EventMatch>>,
}

#[derive(Debug, Deserialize)]
struct EventMatch {
    #[serde(default)]
    event_type: Option<String>,
    #[serde(default)]
    severity: Option<String>,
    #[serde(default)]
    message_id_contains: Option<String>,
    #[serde(default)]
    origin_of_condition: Option<String>,
}

fn event_matches(ev: &RedfishEvent, m: &EventMatch) -> bool {
    if let Some(t) = &m.event_type
        && &ev.event_type != t
    {
        return false;
    }
    if let Some(s) = &m.severity
        && &ev.severity != s
    {
        return false;
    }
    if let Some(mid) = &m.message_id_contains
        && !ev.message_id.contains(mid)
    {
        return false;
    }
    if let Some(o) = &m.origin_of_condition
        && ev.origin_of_condition.as_deref() != Some(o.as_str())
    {
        return false;
    }
    true
}

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
            for want in wanted {
                assert!(
                    drained.iter().any(|ev| event_matches(ev, want)),
                    "[{file}] step '{}': no event matched {want:?} (saw {})",
                    step.name,
                    drained
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
