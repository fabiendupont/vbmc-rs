//! Stream-out for mockup/simulate mode (twin-facade P3).
//!
//! P4 made a served field's value dynamic on the *read* path: `MockupStore::get`
//! resolves twin bindings lazily per request. That is enough for a polling
//! client, but a twin consumer wants push. This module adds the write/stream
//! side: a periodic tick that resolves every dynamic binding and
//!
//! - emits a Redfish `ResourceUpdated` event per changed resource on the shared
//!   [`EventBus`](crate::events::EventBus), which the SSE endpoint and any
//!   webhook subscription fan out;
//! - refreshes a twin [`MetricReport`] assembled from the bindings that declare
//!   a `metric`, so `TelemetryService/MetricReports/TwinMetrics` carries live
//!   values;
//! - evaluates each binding's optional `warning`/`critical` thresholds and emits
//!   an alert event when a value crosses into (or back out of) an alert band.
//!
//! The tick is only spawned when the store has twin bindings
//! ([`MockupStore::twin_stream_interval`] returns `Some`), so a mockup without a
//! `twin.toml` streams nothing and behaves exactly as before.
//!
//! The typed `EventService`/`TelemetryService` handlers are `&'static`
//! config-backed structs that would clash with the fixture, so — as with
//! `mockup_tasks`/`mockup_update` — this drives the mockup store directly and
//! reuses only the transport-level SSE handler and subscription store.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

use chrono::{DateTime, Utc};
use serde_json::{Value, json};

use crate::app_state::AppState;
use crate::backend::mockup::MockupStore;
use crate::events::{EventBus, RedfishEvent};

const EVENT_SERVICE: &str = "/redfish/v1/EventService";
const SSE_URI: &str = "/redfish/v1/EventService/SSE";
const METRIC_REPORTS_COLLECTION: &str = "/redfish/v1/TelemetryService/MetricReports";
const METRIC_REPORT_DEFINITIONS: &str = "/redfish/v1/TelemetryService/MetricReportDefinitions";
const TWIN_REPORT_ID: &str = "TwinMetrics";
const TWIN_REPORT_PATH: &str = "/redfish/v1/TelemetryService/MetricReports/TwinMetrics";
const TWIN_MRD_PATH: &str = "/redfish/v1/TelemetryService/MetricReportDefinitions/TwinMetrics";

/// Spawn the stream-out tick for a mockup store that has twin bindings.
///
/// Does nothing (no task spawned, no fixture mutation) when the store has no
/// bindings, keeping a plain mockup byte-identical to before.
pub fn spawn_stream(state: Arc<AppState>) {
    let Some(store) = state.mockup_store.clone() else {
        return;
    };
    let Some(interval) = store.twin_stream_interval() else {
        return;
    };

    // Advertise the SSE endpoint on the (fixture) EventService so a client that
    // reads EventService can discover the stream we now actually serve.
    store.patch(EVENT_SERVICE, &json!({ "ServerSentEventUri": SSE_URI }));

    // If any binding is a metric, publish the report skeleton up front so the
    // TelemetryService collection lists it before the first tick lands.
    let has_metrics = store
        .twin_stream_snapshot(Instant::now())
        .iter()
        .any(|s| s.metric.is_some());
    if has_metrics {
        register_metric_report(&store, interval);
    }

    tracing::info!(
        interval_s = interval.as_secs(),
        metrics = has_metrics,
        "Started twin stream-out tick"
    );

    tokio::spawn(async move {
        let mut alerts: HashMap<String, &'static str> = HashMap::new();
        let mut event_seq: u64 = 0;
        let mut ticker = tokio::time::interval(interval);
        loop {
            ticker.tick().await;
            run_tick(
                &store,
                &state.event_bus,
                &mut alerts,
                &mut event_seq,
                Instant::now(),
                Utc::now(),
            );
        }
    });
}

/// One stream tick: resolve bindings, emit events, refresh the MetricReport, and
/// evaluate alert thresholds. Separated from the loop so it is unit-testable.
fn run_tick(
    store: &MockupStore,
    event_bus: &EventBus,
    alerts: &mut HashMap<String, &'static str>,
    event_seq: &mut u64,
    now: Instant,
    now_utc: DateTime<Utc>,
) {
    let samples = store.twin_stream_snapshot(now);
    if samples.is_empty() {
        return;
    }

    // One ResourceUpdated per distinct resource path (a resource may have
    // several bound pointers, but it updates once).
    let mut seen: HashSet<&str> = HashSet::new();
    for sample in &samples {
        if seen.insert(sample.path.as_str()) {
            event_bus.emit(resource_updated_event(&sample.path, now_utc, event_seq));
        }
    }

    // Refresh the twin MetricReport from the metric-bearing bindings.
    if let Some(report) = build_metric_report(&samples, now_utc) {
        store.set(TWIN_REPORT_PATH, report);
    }

    // Threshold alerts, emitted only on a level transition to avoid per-tick spam.
    for sample in &samples {
        if sample.warning.is_none() && sample.critical.is_none() {
            continue;
        }
        let Some(value) = sample.value.as_f64() else {
            continue;
        };
        let key = metric_property(&sample.path, &sample.pointer);
        let level = classify(value, sample.warning, sample.critical);
        let prev = alerts.get(&key).copied().unwrap_or("OK");
        if level != prev {
            event_bus.emit(threshold_event(
                &sample.path,
                &key,
                value,
                level,
                now_utc,
                event_seq,
            ));
            alerts.insert(key, level);
        }
    }
}

/// Alert band for a value against optional upper thresholds. `critical` wins over
/// `warning`; with neither set (callers skip those), the value is always `OK`.
fn classify(value: f64, warning: Option<f64>, critical: Option<f64>) -> &'static str {
    if let Some(c) = critical
        && value >= c
    {
        return "Critical";
    }
    if let Some(w) = warning
        && value >= w
    {
        return "Warning";
    }
    "OK"
}

/// `MetricProperty` / alert key for a bound field: the resource path and JSON
/// pointer joined as a Redfish fragment reference (e.g. `.../Temp0#/Reading`).
fn metric_property(path: &str, pointer: &str) -> String {
    format!("{path}#{pointer}")
}

fn next_event_id(seq: &mut u64) -> String {
    *seq += 1;
    seq.to_string()
}

fn resource_updated_event(path: &str, ts: DateTime<Utc>, seq: &mut u64) -> RedfishEvent {
    RedfishEvent {
        event_type: "ResourceUpdated".to_string(),
        event_id: next_event_id(seq),
        event_timestamp: ts,
        message_id: "ResourceEvent.1.0.ResourceUpdated".to_string(),
        message: format!("The resource {path} has been updated."),
        origin_of_condition: Some(path.to_string()),
        severity: "OK".to_string(),
        actor: Some("twin".to_string()),
        payload: None,
    }
}

fn threshold_event(
    path: &str,
    key: &str,
    value: f64,
    level: &'static str,
    ts: DateTime<Utc>,
    seq: &mut u64,
) -> RedfishEvent {
    let (message_id, message) = if level == "OK" {
        (
            "TwinAlert.1.0.ThresholdCleared",
            format!("{key} returned to normal at {value}."),
        )
    } else {
        (
            "TwinAlert.1.0.ThresholdCrossed",
            format!("{key} crossed the {level} threshold at {value}."),
        )
    };
    RedfishEvent {
        event_type: "Alert".to_string(),
        event_id: next_event_id(seq),
        event_timestamp: ts,
        message_id: message_id.to_string(),
        message,
        origin_of_condition: Some(path.to_string()),
        severity: level.to_string(),
        actor: Some("twin".to_string()),
        payload: None,
    }
}

/// Assemble the twin MetricReport from the metric-bearing samples, or `None` if
/// no binding declares a `metric`.
fn build_metric_report(
    samples: &[crate::twin::StreamSample],
    ts: DateTime<Utc>,
) -> Option<Value> {
    let ts = ts.to_rfc3339();
    let values: Vec<Value> = samples
        .iter()
        .filter_map(|s| {
            let metric = s.metric.as_ref()?;
            Some(json!({
                "MetricId": metric,
                "MetricValue": format_metric_value(&s.value),
                "Timestamp": ts,
                "MetricProperty": metric_property(&s.path, &s.pointer),
            }))
        })
        .collect();
    if values.is_empty() {
        return None;
    }
    let count = values.len();
    Some(json!({
        "@odata.id": TWIN_REPORT_PATH,
        "@odata.type": "#MetricReport.v1_4_2.MetricReport",
        "Id": TWIN_REPORT_ID,
        "Name": "Twin Metrics",
        "MetricReportDefinition": { "@odata.id": TWIN_MRD_PATH },
        "MetricValues": values,
        "MetricValues@odata.count": count,
        "Timestamp": ts,
    }))
}

/// Redfish `MetricValue` is a string. Render numbers without quoting artifacts
/// and pass through anything else via its JSON form.
fn format_metric_value(value: &Value) -> String {
    match value {
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// Publish the twin MetricReport skeleton and its definition, and list both in
/// their collections. The fixture collections may carry `Members: null`, so this
/// rebuilds them into well-formed arrays that include our members.
fn register_metric_report(store: &MockupStore, interval: std::time::Duration) {
    // The report resource itself (empty until the first tick fills it).
    store.set(
        TWIN_REPORT_PATH,
        json!({
            "@odata.id": TWIN_REPORT_PATH,
            "@odata.type": "#MetricReport.v1_4_2.MetricReport",
            "Id": TWIN_REPORT_ID,
            "Name": "Twin Metrics",
            "MetricReportDefinition": { "@odata.id": TWIN_MRD_PATH },
            "MetricValues": [],
            "MetricValues@odata.count": 0,
        }),
    );
    ensure_collection_member(
        store,
        METRIC_REPORTS_COLLECTION,
        TWIN_REPORT_PATH,
        "#MetricReportCollection.MetricReportCollection",
        "Metric Report Collection",
    );

    // A minimal periodic definition so the report's MetricReportDefinition link
    // resolves instead of 404ing.
    store.set(
        TWIN_MRD_PATH,
        json!({
            "@odata.id": TWIN_MRD_PATH,
            "@odata.type": "#MetricReportDefinition.v1_4_2.MetricReportDefinition",
            "Id": TWIN_REPORT_ID,
            "Name": "Twin Metrics Definition",
            "MetricReportDefinitionType": "Periodic",
            "Schedule": { "RecurrenceInterval": format!("PT{}S", interval.as_secs().max(1)) },
            "MetricReport": { "@odata.id": TWIN_REPORT_PATH },
        }),
    );
    ensure_collection_member(
        store,
        METRIC_REPORT_DEFINITIONS,
        TWIN_MRD_PATH,
        "#MetricReportDefinitionCollection.MetricReportDefinitionCollection",
        "Metric Report Definition Collection",
    );
}

/// Ensure `member_path` is listed in the collection at `collection_path`,
/// rebuilding the collection into a well-formed array (the fixture may store
/// `Members: null`). Idempotent: a member already present is not duplicated.
fn ensure_collection_member(
    store: &MockupStore,
    collection_path: &str,
    member_path: &str,
    odata_type: &str,
    name: &str,
) {
    let mut members: Vec<Value> = store
        .get(collection_path)
        .and_then(|c| c.get("Members").and_then(|m| m.as_array().cloned()))
        .unwrap_or_default();

    let present = members
        .iter()
        .any(|m| m.get("@odata.id").and_then(|v| v.as_str()) == Some(member_path));
    if !present {
        members.push(json!({ "@odata.id": member_path }));
    }
    let count = members.len();
    store.set(
        collection_path,
        json!({
            "@odata.id": collection_path,
            "@odata.type": odata_type,
            "Name": name,
            "Members": members,
            "Members@odata.count": count,
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::twin::StreamSample;

    fn sample(path: &str, value: f64, metric: Option<&str>) -> StreamSample {
        StreamSample {
            path: path.to_string(),
            pointer: "/Reading".to_string(),
            value: json!(value),
            metric: metric.map(String::from),
            warning: None,
            critical: None,
        }
    }

    #[test]
    fn classify_prefers_critical_then_warning_then_ok() {
        assert_eq!(classify(90.0, Some(70.0), Some(85.0)), "Critical");
        assert_eq!(classify(75.0, Some(70.0), Some(85.0)), "Warning");
        assert_eq!(classify(60.0, Some(70.0), Some(85.0)), "OK");
        // Warning-only binding never escalates to Critical.
        assert_eq!(classify(999.0, Some(70.0), None), "Warning");
        // Exactly at the threshold counts as crossed.
        assert_eq!(classify(85.0, Some(70.0), Some(85.0)), "Critical");
    }

    #[test]
    fn build_metric_report_includes_only_metric_bindings() {
        let samples = vec![
            sample("/redfish/v1/Chassis/GPU_0/Sensors/Temp0", 57.5, Some("GpuTemperature")),
            sample("/redfish/v1/Chassis/GPU_0/Sensors/Power0", 150.0, None),
        ];
        let report = build_metric_report(&samples, Utc::now()).unwrap();
        assert_eq!(report["Id"], TWIN_REPORT_ID);
        assert_eq!(report["MetricValues@odata.count"], 1);
        let mv = &report["MetricValues"][0];
        assert_eq!(mv["MetricId"], "GpuTemperature");
        // MetricValue is rendered as a string.
        assert_eq!(mv["MetricValue"], "57.5");
        assert_eq!(
            mv["MetricProperty"],
            "/redfish/v1/Chassis/GPU_0/Sensors/Temp0#/Reading"
        );
    }

    #[test]
    fn build_metric_report_is_none_without_metric_bindings() {
        let samples = vec![sample("/x", 1.0, None)];
        assert!(build_metric_report(&samples, Utc::now()).is_none());
    }

    #[test]
    fn resource_updated_event_targets_the_path() {
        let mut seq = 0;
        let ev = resource_updated_event("/redfish/v1/Chassis/GPU_0/Sensors/Temp0", Utc::now(), &mut seq);
        assert_eq!(ev.event_type, "ResourceUpdated");
        assert_eq!(ev.event_id, "1");
        assert_eq!(
            ev.origin_of_condition.as_deref(),
            Some("/redfish/v1/Chassis/GPU_0/Sensors/Temp0")
        );
        assert_eq!(ev.severity, "OK");
    }

    #[test]
    fn threshold_event_carries_level_and_clears() {
        let mut seq = 0;
        let crossed = threshold_event("/x", "/x#/Reading", 90.0, "Critical", Utc::now(), &mut seq);
        assert_eq!(crossed.event_type, "Alert");
        assert_eq!(crossed.severity, "Critical");
        assert_eq!(crossed.message_id, "TwinAlert.1.0.ThresholdCrossed");

        let cleared = threshold_event("/x", "/x#/Reading", 10.0, "OK", Utc::now(), &mut seq);
        assert_eq!(cleared.severity, "OK");
        assert_eq!(cleared.message_id, "TwinAlert.1.0.ThresholdCleared");
    }

    #[test]
    fn run_tick_emits_event_and_writes_report() {
        // A store loaded with a metric + threshold binding drives one tick.
        let dir = tempfile::TempDir::new().unwrap();
        let sensor = dir.path().join("redfish/v1/Chassis/GPU_0/Sensors/Temp0");
        std::fs::create_dir_all(&sensor).unwrap();
        std::fs::write(
            sensor.join("index.json"),
            json!({
                "@odata.id": "/redfish/v1/Chassis/GPU_0/Sensors/Temp0",
                "Name": "Temp0",
                "Reading": 0.0,
            })
            .to_string(),
        )
        .unwrap();
        std::fs::write(
            dir.path().join("twin.toml"),
            r#"
[twin]
[[twin.binding]]
path = "/redfish/v1/Chassis/GPU_0/Sensors/Temp0"
pointer = "/Reading"
source = "formula"
formula = { kind = "sine", min = 80.0, max = 90.0, period_s = 60 }
metric = "GpuTemperature"
warning = 70.0
"#,
        )
        .unwrap();

        let store = MockupStore::load(dir.path()).unwrap();
        let bus = EventBus::default();
        let mut rx = bus.subscribe();
        let mut alerts = HashMap::new();
        let mut seq = 0;

        run_tick(&store, &bus, &mut alerts, &mut seq, Instant::now(), Utc::now());

        // MetricReport was written with the live reading.
        let report = store.get(TWIN_REPORT_PATH).unwrap();
        assert_eq!(report["MetricValues"][0]["MetricId"], "GpuTemperature");

        // A ResourceUpdated event and a Warning alert (reading starts at 80 >= 70)
        // were published.
        let mut kinds = Vec::new();
        while let Ok(ev) = rx.try_recv() {
            kinds.push((ev.event_type, ev.severity));
        }
        assert!(kinds.contains(&("ResourceUpdated".to_string(), "OK".to_string())));
        assert!(kinds.contains(&("Alert".to_string(), "Warning".to_string())));
        assert_eq!(alerts.values().next(), Some(&"Warning"));
    }

    #[test]
    fn run_tick_alert_fires_once_until_level_changes() {
        let dir = tempfile::TempDir::new().unwrap();
        let sensor = dir.path().join("redfish/v1/Chassis/GPU_0/Sensors/Temp0");
        std::fs::create_dir_all(&sensor).unwrap();
        std::fs::write(
            sensor.join("index.json"),
            json!({ "@odata.id": "/redfish/v1/Chassis/GPU_0/Sensors/Temp0", "Reading": 0.0 })
                .to_string(),
        )
        .unwrap();
        // Constant formula (min == max) so the level never changes across ticks.
        std::fs::write(
            dir.path().join("twin.toml"),
            r#"
[twin]
[[twin.binding]]
path = "/redfish/v1/Chassis/GPU_0/Sensors/Temp0"
pointer = "/Reading"
source = "formula"
formula = { kind = "sine", min = 90.0, max = 90.0, period_s = 60 }
critical = 85.0
"#,
        )
        .unwrap();

        let store = MockupStore::load(dir.path()).unwrap();
        let bus = EventBus::default();
        let mut rx = bus.subscribe();
        let mut alerts = HashMap::new();
        let mut seq = 0;
        let now = Instant::now();

        run_tick(&store, &bus, &mut alerts, &mut seq, now, Utc::now());
        run_tick(&store, &bus, &mut alerts, &mut seq, now, Utc::now());

        // Two ticks => two ResourceUpdated, but only one Alert (level unchanged).
        let mut alert_count = 0;
        let mut updated_count = 0;
        while let Ok(ev) = rx.try_recv() {
            match ev.event_type.as_str() {
                "Alert" => alert_count += 1,
                "ResourceUpdated" => updated_count += 1,
                _ => {}
            }
        }
        assert_eq!(updated_count, 2);
        assert_eq!(alert_count, 1);
    }
}
