//! External-twin ingest for mockup/simulate mode (twin-facade P2).
//!
//! P1 made a served field dynamic from a local formula; P2 lets an *external*
//! feed drive it. A binding declared `source = "external"` resolves from a value
//! map that this endpoint fills: `POST /twin/v1/state` with a flat JSON object of
//! `{ "<key>": <value>, ... }`. Each entry updates the reading for every binding
//! whose `key` matches; at resolution the value is clamped to the binding's
//! `[min, max]`, and a reading that is missing or older than the twin's freshness
//! TTL marks the field offline (see [`crate::twin`]).
//!
//! This lives outside the `/redfish` tree on purpose: it is a twin control-plane
//! endpoint, not a Redfish resource, so it never clashes with the fixture.

use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};

use crate::app_state::AppState;

/// Path of the batch ingest endpoint.
pub const INGEST_PATH: &str = "/twin/v1/state";

/// `POST /twin/v1/state` — batch external-twin ingest.
///
/// Accepts a flat JSON object of key/value readings. Keys bound by a
/// `source = "external"` binding are recorded; unbound keys are reported in the
/// response but do not fail the request (a sim feed may carry extra channels).
pub async fn ingest_state(State(state): State<Arc<AppState>>, body: axum::body::Bytes) -> Response {
    let Some(store) = state.mockup_store.clone() else {
        return StatusCode::NOT_FOUND.into_response();
    };

    let readings = match serde_json::from_slice::<Value>(&body) {
        Ok(Value::Object(map)) => map,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                "expected a JSON object of { key: value } readings",
            )
                .into_response();
        }
    };

    let mut accepted: Vec<String> = Vec::new();
    let mut unknown: Vec<String> = Vec::new();
    for (key, value) in readings {
        if store.twin_has_external_key(&key) {
            store.twin_ingest(&key, value);
            accepted.push(key);
        } else {
            unknown.push(key);
        }
    }

    let count = accepted.len();
    axum::Json(json!({
        "Accepted": count,
        "AcceptedKeys": accepted,
        "UnknownKeys": unknown,
    }))
    .into_response()
}
