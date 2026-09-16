//! External-twin ingest for mockup/simulate mode (twin-facade P2, fleet P5).
//!
//! P1 made a served field dynamic from a local formula; P2 lets an *external*
//! feed drive it. A binding declared `source = "external"` resolves from a value
//! map that this endpoint fills: `POST /twin/v1/state`. Two body shapes are
//! accepted:
//!
//! - **Flat object** `{ "<key>": <value>, ... }` — single-node form. Every entry
//!   is addressed to this node (there is no fleet routing to do).
//! - **Fleet array** `[ { "system_id": "tray-01", "key": "gpu0.temp_c",
//!   "value": 74.2 }, ... ]` — the canonical twin batch (P5). One twin drives
//!   many nodes by keying each sample by `system_id`; every node receives the
//!   same batch and self-selects. A sample is ingested only when this node
//!   [`accepts`](crate::twin::TwinConfig::accepts_system_id) its address — its
//!   own `system_id`, or none (broadcast); samples for other nodes are reported
//!   as `ForeignKeys` and ignored. An optional `ts` field is accepted and
//!   ignored (freshness is stamped locally at ingest).
//!
//! For any accepted sample, the reading updates every binding whose `key`
//! matches; at resolution the value is clamped to the binding's `[min, max]`,
//! and a reading that is missing or older than the twin's freshness TTL marks
//! the field offline (see [`crate::twin`]).
//!
//! This lives outside the `/redfish` tree on purpose: it is a twin control-plane
//! endpoint, not a Redfish resource, so it never clashes with the fixture.

use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};

use crate::app_state::AppState;
use crate::backend::mockup::MockupStore;

/// Path of the batch ingest endpoint.
pub const INGEST_PATH: &str = "/twin/v1/state";

/// Tallies of how each key in an ingest batch was routed.
#[derive(Default)]
struct IngestTally {
    /// Keys recorded (bound and addressed to this node).
    accepted: Vec<String>,
    /// Keys not consumed by any binding on this node.
    unknown: Vec<String>,
    /// Keys addressed to a different node's `system_id` (fleet form only).
    foreign: Vec<String>,
}

/// `POST /twin/v1/state` — batch external-twin ingest.
///
/// Accepts the flat-object or fleet-array body (see the module docs). Keys bound
/// by a `source = "external"` binding and addressed to this node are recorded;
/// unbound keys are reported but do not fail the request (a sim feed may carry
/// extra channels), and samples for other nodes are reported as foreign.
pub async fn ingest_state(State(state): State<Arc<AppState>>, body: axum::body::Bytes) -> Response {
    let Some(store) = state.mockup_store.clone() else {
        return StatusCode::NOT_FOUND.into_response();
    };

    let mut tally = IngestTally::default();
    match serde_json::from_slice::<Value>(&body) {
        // Single-node flat object: every reading is addressed to this node.
        Ok(Value::Object(map)) => {
            for (key, value) in map {
                route_sample(&store, None, key, value, &mut tally);
            }
        }
        // Fleet array: each element carries its own key/value and optional
        // `system_id`; this node self-selects the samples addressed to it.
        Ok(Value::Array(items)) => {
            for item in items {
                let Value::Object(mut obj) = item else {
                    return bad_request("each fleet sample must be a JSON object");
                };
                let Some(Value::String(key)) = obj.remove("key") else {
                    return bad_request("each fleet sample needs a string \"key\"");
                };
                let value = obj.remove("value").unwrap_or(Value::Null);
                let system_id = match obj.remove("system_id") {
                    Some(Value::String(s)) => Some(s),
                    None | Some(Value::Null) => None,
                    Some(_) => return bad_request("\"system_id\" must be a string"),
                };
                route_sample(&store, system_id.as_deref(), key, value, &mut tally);
            }
        }
        _ => {
            return bad_request(
                "expected a JSON object of { key: value } or an array of { system_id, key, value }",
            );
        }
    }

    axum::Json(json!({
        "Accepted": tally.accepted.len(),
        "AcceptedKeys": tally.accepted,
        "UnknownKeys": tally.unknown,
        "ForeignKeys": tally.foreign,
    }))
    .into_response()
}

/// Route one sample by its address, then by whether a binding consumes its key.
fn route_sample(
    store: &MockupStore,
    system_id: Option<&str>,
    key: String,
    value: Value,
    tally: &mut IngestTally,
) {
    // A sample for another node is ignored here; some other endpoint owns it.
    if !store.twin_accepts_system_id(system_id) {
        tally.foreign.push(key);
        return;
    }
    if store.twin_has_external_key(&key) {
        store.twin_ingest(&key, value);
        tally.accepted.push(key);
    } else {
        tally.unknown.push(key);
    }
}

fn bad_request(message: &'static str) -> Response {
    (StatusCode::BAD_REQUEST, message).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::twin::TwinConfig;

    fn store_for(toml: &str) -> MockupStore {
        MockupStore::for_test_with_twin(TwinConfig::from_toml(toml).unwrap())
    }

    // A node identified as "tray-01" with one external binding on key "temp".
    const FLEET_TOML: &str = r#"
        [twin]
        system_id = "tray-01"
        [[twin.binding]]
        path = "/redfish/v1/Chassis/C/Sensors/Temp"
        pointer = "/Reading"
        source = "external"
        key = "temp"
        min = 0.0
        max = 100.0
    "#;

    const SENSOR_PATH: &str = "/redfish/v1/Chassis/C/Sensors/Temp";

    #[test]
    fn fleet_sample_for_this_node_is_accepted_and_ingested() {
        let store = store_for(FLEET_TOML);
        store.set(SENSOR_PATH, json!({ "Reading": 0.0 }));

        let mut tally = IngestTally::default();
        route_sample(
            &store,
            Some("tray-01"),
            "temp".to_string(),
            json!(42.0),
            &mut tally,
        );
        assert_eq!(tally.accepted, vec!["temp"]);
        assert!(tally.unknown.is_empty());
        assert!(tally.foreign.is_empty());

        // The reading actually landed: get() resolves it onto the bound pointer.
        let resolved = store.get(SENSOR_PATH).unwrap();
        assert_eq!(resolved["Reading"], json!(42.0));
    }

    #[test]
    fn fleet_sample_for_another_node_is_foreign_and_ignored() {
        let store = store_for(FLEET_TOML);
        store.set(
            SENSOR_PATH,
            json!({ "Reading": 0.0, "Status": { "State": "Enabled" } }),
        );

        let mut tally = IngestTally::default();
        route_sample(
            &store,
            Some("tray-02"),
            "temp".to_string(),
            json!(42.0),
            &mut tally,
        );
        assert_eq!(tally.foreign, vec!["temp"]);
        assert!(tally.accepted.is_empty());
        assert!(tally.unknown.is_empty());

        // Nothing was ingested for the foreign sample, so the external feed is
        // absent and the bound reading resolves offline (not to 42.0).
        let resolved = store.get(SENSOR_PATH).unwrap();
        assert_eq!(resolved["Reading"], Value::Null);
        assert_eq!(resolved["Status"]["State"], "UnavailableOffline");
    }

    #[test]
    fn unaddressed_sample_is_accepted_by_a_fleet_node() {
        let store = store_for(FLEET_TOML);
        let mut tally = IngestTally::default();
        route_sample(&store, None, "temp".to_string(), json!(1.0), &mut tally);
        assert_eq!(tally.accepted, vec!["temp"]);
    }

    #[test]
    fn unbound_key_addressed_to_this_node_is_unknown_not_foreign() {
        let store = store_for(FLEET_TOML);
        let mut tally = IngestTally::default();
        route_sample(
            &store,
            Some("tray-01"),
            "bogus".to_string(),
            json!(1.0),
            &mut tally,
        );
        assert_eq!(tally.unknown, vec!["bogus"]);
        assert!(tally.accepted.is_empty());
        assert!(tally.foreign.is_empty());
    }

    #[test]
    fn single_node_without_identity_accepts_addressed_samples() {
        // No [twin] system_id: this node has no fleet identity, so any address
        // (or none) is accepted — the flat single-node contract is preserved.
        let store = store_for(
            r#"
            [twin]
            [[twin.binding]]
            path = "/x"
            pointer = "/Reading"
            source = "external"
            key = "temp"
            min = 0.0
            max = 100.0
        "#,
        );
        let mut tally = IngestTally::default();
        route_sample(
            &store,
            Some("whatever"),
            "temp".to_string(),
            json!(1.0),
            &mut tally,
        );
        assert_eq!(tally.accepted, vec!["temp"]);
    }
}
