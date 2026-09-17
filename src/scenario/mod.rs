//! Shared scenario model + verification half for the twin behavioral harness.
//!
//! This is the *identical verification half* used by two callers:
//!
//! - **In-process** (`tests/replay.rs`, twin-facade P6 track (a)): replays scenario
//!   files against a paused-clock `spawn_stream` and asserts on the in-memory
//!   `event_bus`.
//! - **Over the wire** (the `scenario-harness` binary, twin-facade P6 track (b)):
//!   replays the *same* files against a *live* Redfish BMC and asserts on real HTTP
//!   responses, MetricReports, and the SSE event stream.
//!
//! To keep both callers honest against one source of truth, the scenario schema
//! (`TwinSequence`/`TwinStep`/`ExpectSpec`/`MatchSpec`/`EventMatch`) and the pure
//! verifiers (`json_contains`, `check_matchspec`, `event_matches`) live here. Event
//! matching is normalized through [`ObservedEvent`], which both an in-process
//! [`crate::events::RedfishEvent`] (Rust field names) and an over-the-wire SSE JSON
//! frame (PascalCase `EventType`/`Severity`/… keys) map into — so the same
//! `EventMatch` criteria evaluate identically regardless of source.

use std::collections::HashMap;

use serde::Deserialize;
use serde_json::Value;

pub mod probe;

// --- Request/response schema ------------------------------------------------

/// One HTTP request in a scenario step.
#[derive(Debug, Deserialize)]
pub struct RequestSpec {
    pub method: String,
    pub path: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub body: Option<Value>,
}

/// What a step expects of a response.
#[derive(Debug, Deserialize)]
pub struct ExpectSpec {
    pub status: u16,
    #[serde(default)]
    pub body_contains: Option<Value>,
    #[serde(default)]
    pub body_equals: Option<Value>,
    /// Dot-delimited paths (e.g. `Attributes.BootMode`) that must NOT be present.
    /// Lets a sequence assert a reset/delete removed state without pinning the
    /// whole document (which carries volatile etags).
    #[serde(default)]
    pub body_lacks: Option<Vec<String>>,
    /// Typed numeric matchers applied at dot-delimited paths (twin-facade P6 S4).
    /// Numbers rendered as JSON strings (e.g. a MetricReport `MetricValue`) are
    /// parsed, so the same matcher works on Readings and MetricValues alike.
    #[serde(default)]
    pub body_matches: Option<Vec<MatchSpec>>,
}

/// One typed matcher on a numeric field. Any subset of the constraints may be
/// present; all present constraints must hold. `monotonic` records the value
/// into a named cross-step series (defaulting to `path`) and asserts the
/// direction against the previously recorded value.
#[derive(Debug, Deserialize)]
pub struct MatchSpec {
    pub path: String,
    /// Inclusive `[min, max]`.
    #[serde(default)]
    pub range: Option<[f64; 2]>,
    /// Target value; paired with `tol` (default 1e-6).
    #[serde(default)]
    pub approx: Option<f64>,
    #[serde(default)]
    pub tol: Option<f64>,
    #[serde(default)]
    pub gte: Option<f64>,
    #[serde(default)]
    pub lte: Option<f64>,
    #[serde(default)]
    pub monotonic: Option<Direction>,
    #[serde(default)]
    pub series: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Increasing,
    Decreasing,
}

// --- Twin sequence schema ---------------------------------------------------

/// A twin scenario file: an inline `twin.toml`, a small resource fixture, and an
/// ordered list of virtual-time steps.
#[derive(Debug, Deserialize)]
pub struct TwinSequence {
    pub description: String,
    /// Inline `twin.toml` (bindings + scenarios) driving the store.
    pub twin: String,
    /// Resource tree by Redfish path — each becomes an `index.json` the store
    /// loads, so the bound read path resolves.
    #[serde(default)]
    pub fixture: HashMap<String, Value>,
    pub steps: Vec<TwinStep>,
}

#[derive(Debug, Deserialize)]
pub struct TwinStep {
    pub name: String,
    /// Virtual/wall-clock seconds to advance before this step.
    #[serde(default)]
    pub advance_seconds: f64,
    #[serde(default)]
    pub request: Option<RequestSpec>,
    #[serde(default)]
    pub expect: Option<ExpectSpec>,
    /// Events that must have been emitted since the previous step.
    #[serde(default)]
    pub expect_events: Option<Vec<EventMatch>>,
}

/// Match criteria for an emitted event; every present field must match.
#[derive(Debug, Deserialize)]
pub struct EventMatch {
    #[serde(default)]
    pub event_type: Option<String>,
    #[serde(default)]
    pub severity: Option<String>,
    #[serde(default)]
    pub message_id_contains: Option<String>,
    #[serde(default)]
    pub origin_of_condition: Option<String>,
}

/// A source-agnostic view of an emitted event. Both the in-process
/// [`crate::events::RedfishEvent`] and an over-the-wire SSE JSON frame normalize
/// into this shape so [`event_matches`] evaluates identically for either caller.
#[derive(Debug, Clone)]
pub struct ObservedEvent {
    pub event_type: String,
    pub severity: String,
    pub message_id: String,
    pub origin_of_condition: Option<String>,
}

impl ObservedEvent {
    /// From the in-process event bus (Rust field names).
    pub fn from_redfish_event(ev: &crate::events::RedfishEvent) -> Self {
        Self {
            event_type: ev.event_type.clone(),
            severity: ev.severity.clone(),
            message_id: ev.message_id.clone(),
            origin_of_condition: ev.origin_of_condition.clone(),
        }
    }

    /// From an SSE `data:` frame: a JSON object with PascalCase Redfish keys
    /// (`EventType`/`Severity`/`MessageId`/`OriginOfCondition`), as serialized by
    /// `src/redfish/event_service.rs`. Missing string fields default to empty.
    pub fn from_sse_json(v: &Value) -> Self {
        let s = |k: &str| {
            v.get(k)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string()
        };
        Self {
            event_type: s("EventType"),
            severity: s("Severity"),
            message_id: s("MessageId"),
            origin_of_condition: v
                .get("OriginOfCondition")
                .and_then(Value::as_str)
                .map(str::to_string),
        }
    }
}

/// Whether an observed event satisfies every present criterion of a match.
pub fn event_matches(ev: &ObservedEvent, m: &EventMatch) -> bool {
    if let Some(t) = &m.event_type
        && &ev.event_type != t
    {
        return false;
    }
    if let Some(sev) = &m.severity
        && &ev.severity != sev
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

// --- Pure verifiers ---------------------------------------------------------

/// Recursive subset match: every key/element in `expected` must be present in
/// `actual` and match. Returns a JSON-path-ish location string on mismatch.
pub fn json_contains(actual: &Value, expected: &Value) -> Result<(), String> {
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
pub fn json_path_get<'a>(v: &'a Value, dotted: &str) -> Option<&'a Value> {
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
pub fn json_path_present(v: &Value, dotted: &str) -> bool {
    json_path_get(v, dotted).is_some()
}

/// Coerce a JSON value to a number, accepting numbers rendered as strings (a
/// Redfish `MetricValue` is a string even when it carries a number).
pub fn as_number(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.parse().ok(),
        _ => None,
    }
}

/// Evaluate one typed matcher against a response body. `series` carries cross-step
/// monotonic state (the value at each named series' previous observation).
pub fn check_matchspec(
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn json_path_get_descends_objects_and_arrays() {
        let v = json!({ "MetricValues": [ { "MetricValue": "100.0" } ] });
        assert_eq!(
            json_path_get(&v, "MetricValues.0.MetricValue").unwrap(),
            &json!("100.0")
        );
        assert!(json_path_get(&v, "MetricValues.1.MetricValue").is_none());
    }

    #[test]
    fn as_number_parses_stringified_numbers() {
        assert_eq!(as_number(&json!("42.5")), Some(42.5));
        assert_eq!(as_number(&json!(42.5)), Some(42.5));
        assert_eq!(as_number(&json!("nope")), None);
    }

    #[test]
    fn check_matchspec_range_approx_and_monotonic() {
        let mut series = HashMap::new();
        let body = json!({ "Reading": 35.0 });
        let spec: MatchSpec = serde_json::from_value(json!({
            "path": "Reading", "range": [30.0, 40.0], "approx": 35.0, "tol": 0.001,
            "monotonic": "increasing", "series": "temp"
        }))
        .unwrap();
        check_matchspec(&body, &spec, &mut series).unwrap();

        // A lower next reading breaks the increasing series.
        let body2 = json!({ "Reading": 20.0 });
        let spec2: MatchSpec = serde_json::from_value(json!({
            "path": "Reading", "monotonic": "increasing", "series": "temp"
        }))
        .unwrap();
        assert!(check_matchspec(&body2, &spec2, &mut series).is_err());
    }

    #[test]
    fn event_matches_all_present_criteria() {
        let ev = ObservedEvent {
            event_type: "Alert".into(),
            severity: "Warning".into(),
            message_id: "TwinAlert.1.0.ThresholdCrossed".into(),
            origin_of_condition: Some("/redfish/v1/Chassis/GPU_0/Sensors/Temp0".into()),
        };
        let m: EventMatch = serde_json::from_value(json!({
            "event_type": "Alert", "severity": "Warning",
            "message_id_contains": "ThresholdCrossed"
        }))
        .unwrap();
        assert!(event_matches(&ev, &m));

        let no: EventMatch = serde_json::from_value(json!({ "severity": "Critical" })).unwrap();
        assert!(!event_matches(&ev, &no));
    }

    #[test]
    fn observed_event_from_sse_json_reads_pascalcase() {
        let frame = json!({
            "EventType": "Alert",
            "Severity": "Critical",
            "MessageId": "TwinAlert.1.0.ThresholdCrossed",
            "OriginOfCondition": "/redfish/v1/Chassis/GPU_0/Sensors/Temp0"
        });
        let ev = ObservedEvent::from_sse_json(&frame);
        assert_eq!(ev.event_type, "Alert");
        assert_eq!(ev.severity, "Critical");
        assert_eq!(ev.message_id, "TwinAlert.1.0.ThresholdCrossed");
        assert_eq!(
            ev.origin_of_condition.as_deref(),
            Some("/redfish/v1/Chassis/GPU_0/Sensors/Temp0")
        );
    }
}
