//! Scenario trigger/lifecycle for mockup/simulate mode (twin-facade P6 S2).
//!
//! S1 gave the twin a named, time-sequenced [`Source::Scenario`] timeline
//! ([`crate::twin`]) that evaluated as a pure function of elapsed time, measured
//! from the store start. That is enough for CI, which queries absolute offsets,
//! but not for interactive use: you cannot "press inject" on demand. S2 adds the
//! lifecycle control — the equivalent of arming a protection-relay test set:
//!
//! - `POST /twin/v1/scenario/{name}` — **arm** the scenario: re-base its timeline
//!   to run from now. A scenario that no binding drives is a 404.
//! - `GET  /twin/v1/scenario` — **list** every scenario and its current state
//!   (armed vs idle, elapsed since its reference instant, current value/offline).
//! - `DELETE /twin/v1/scenario` — **reset**: disarm every scenario, reverting each
//!   timeline to run from the store start (the S1 default).
//!
//! Arming only re-bases the timeline's clock; it never mutates the fixture. A
//! store without scenario bindings serves an empty list and 404s any arm, so the
//! endpoint is inert unless a `twin.toml` declares scenarios. Like the ingest
//! endpoint, this lives outside the `/redfish` tree: it is a twin control-plane
//! endpoint, not a Redfish resource.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;
use std::time::Instant;

use crate::app_state::AppState;

/// List/reset endpoint path.
pub const SCENARIO_PATH: &str = "/twin/v1/scenario";
/// Arm endpoint path (one scenario by name).
pub const SCENARIO_ARM_PATH: &str = "/twin/v1/scenario/{name}";

/// `GET /twin/v1/scenario` — list every scenario and its lifecycle state.
pub async fn list_scenarios(State(state): State<Arc<AppState>>) -> Response {
    let Some(store) = state.mockup_store.clone() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let states = store.twin_scenario_states(Instant::now());
    axum::Json(json!({ "Scenarios": states })).into_response()
}

/// `POST /twin/v1/scenario/{name}` — arm one scenario (re-base to now).
///
/// Returns the armed scenario's fresh state, or 404 when no binding drives a
/// scenario of that name.
pub async fn arm_scenario(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Response {
    let Some(store) = state.mockup_store.clone() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if !store.twin_arm_scenario(&name) {
        return (StatusCode::NOT_FOUND, format!("unknown scenario {name}")).into_response();
    }
    let state = store
        .twin_scenario_states(Instant::now())
        .into_iter()
        .find(|s| s.name == name);
    axum::Json(json!({ "Armed": name, "State": state })).into_response()
}

/// `DELETE /twin/v1/scenario` — disarm every scenario (revert to store start).
pub async fn reset_scenarios(State(state): State<Arc<AppState>>) -> Response {
    let Some(store) = state.mockup_store.clone() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let cleared = store.twin_reset_scenarios();
    axum::Json(json!({ "Reset": cleared })).into_response()
}
