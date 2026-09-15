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

use std::fs;
use std::path::Path;
use std::sync::Arc;

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

/// Whether a dot-delimited path resolves to a value in `v`.
fn json_path_present(v: &Value, dotted: &str) -> bool {
    let mut cur = v;
    for seg in dotted.split('.') {
        match cur.get(seg) {
            Some(next) => cur = next,
            None => return false,
        }
    }
    true
}

async fn replay(path: &Path) {
    let raw = fs::read_to_string(path).expect("read sequence file");
    let seq: Sequence = serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("{}: invalid sequence file: {e}", path.display()));
    let app = build_app(seq.systems);
    let file = path.file_name().unwrap().to_string_lossy();
    eprintln!("replaying {file}: {}", seq.description);

    for step in &seq.steps {
        let mut builder = Request::builder()
            .method(step.request.method.as_str())
            .uri(&step.request.path);
        for (k, v) in &step.request.headers {
            builder = builder.header(k, v);
        }
        let body = match &step.request.body {
            Some(json) => {
                builder = builder.header("content-type", "application/json");
                Body::from(serde_json::to_vec(json).unwrap())
            }
            None => Body::empty(),
        };
        let req = builder.body(body).unwrap();

        let response = app.clone().oneshot(req).await.unwrap();
        let status = response.status().as_u16();
        let bytes = axum::body::to_bytes(response.into_body(), 4_000_000)
            .await
            .unwrap();

        assert_eq!(
            status,
            step.expect.status,
            "[{file}] step '{}': status mismatch (body: {})",
            step.name,
            String::from_utf8_lossy(&bytes)
        );

        if step.expect.body_contains.is_some()
            || step.expect.body_equals.is_some()
            || step.expect.body_lacks.is_some()
        {
            let actual: Value = serde_json::from_slice(&bytes).unwrap_or_else(|e| {
                panic!("[{file}] step '{}': response is not JSON: {e}", step.name)
            });
            if let Some(expected) = &step.expect.body_contains {
                if let Err(loc) = json_contains(&actual, expected) {
                    panic!("[{file}] step '{}': body mismatch at {loc}", step.name);
                }
            }
            if let Some(expected) = &step.expect.body_equals {
                assert_eq!(
                    &actual, expected,
                    "[{file}] step '{}': body_equals mismatch (actual: {actual})",
                    step.name
                );
            }
            if let Some(paths) = &step.expect.body_lacks {
                for p in paths {
                    if json_path_present(&actual, p) {
                        panic!(
                            "[{file}] step '{}': expected path '{p}' to be absent (actual: {actual})",
                            step.name
                        );
                    }
                }
            }
        }
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
