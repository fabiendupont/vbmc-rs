//! Digital-twin seam for mockup/simulate mode.
//!
//! Today a served Redfish field's value comes verbatim from the mockup store.
//! The twin façade (see `docs/twin-facade.md`) generalizes that into one seam:
//! *where a served field's value comes from*. A [`TwinConfig`] holds a table of
//! [`Binding`]s — each says that a JSON pointer inside a stored resource is
//! dynamic and names its [`Source`].
//!
//! [`MockupStore::get`](crate::backend::mockup::MockupStore::get) clones the base
//! resource, then applies every binding for that path by writing the resolved
//! value at its JSON pointer. With no bindings the clone is returned untouched,
//! so a store without a `twin.toml` behaves byte-for-byte as before.
//!
//! This module implements the seam (P0), the local [`Source::Formula`] waveform
//! (P1), and the external-twin feed (P2): [`Source::External`] resolves from an
//! ingested value map ([`Sample`]) fed over `POST /twin/v1/state`, clamped to
//! the binding's bounds, going offline when a reading is missing or stale.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use serde::Deserialize;
use serde_json::{Value, json};

/// Default freshness window for external samples when `twin.toml` omits it.
const DEFAULT_FRESHNESS_TTL_SECS: u64 = 10;

/// Default cadence for the stream-out tick when `twin.toml` omits it. The tick
/// drives `ResourceUpdated` events and MetricReport refreshes (see the stream
/// module); it is only spawned when the binding table is non-empty.
const DEFAULT_TICK_INTERVAL_SECS: u64 = 5;

/// Shape of a local waveform for a [`Source::Formula`] binding.
///
/// The value sweeps `[min, max]` over `period_s`, starting at `min` at t=0.
#[derive(Debug, Clone, Deserialize)]
pub struct FormulaSpec {
    pub kind: FormulaKind,
    pub min: f64,
    pub max: f64,
    pub period_s: f64,
}

/// Waveform kind for a [`FormulaSpec`].
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FormulaKind {
    /// Smooth cosine sweep: min at t=0, max at half period, back to min.
    Sine,
    /// Linear ramp min→max over each period, then a discontinuous drop to min.
    Sawtooth,
    /// Linear ramp min→max→min across each period.
    Triangle,
}

impl FormulaSpec {
    /// Evaluate the waveform at `elapsed` since the store started.
    fn eval(&self, elapsed: Duration) -> f64 {
        // Fraction through the current period, 0.0..1.0.
        let phase = if self.period_s > 0.0 {
            (elapsed.as_secs_f64() % self.period_s) / self.period_s
        } else {
            0.0
        };
        // Unit amplitude 0.0..1.0, starting at 0.0.
        let unit = match self.kind {
            FormulaKind::Sine => 0.5 - 0.5 * (std::f64::consts::TAU * phase).cos(),
            FormulaKind::Sawtooth => phase,
            FormulaKind::Triangle => 1.0 - (2.0 * phase - 1.0).abs(),
        };
        self.min + unit * (self.max - self.min)
    }
}

/// Where a bound field's value comes from.
#[derive(Debug, Clone)]
pub enum Source {
    /// Not dynamic — leave the base value in place. Lets a binding be declared
    /// (e.g. for documentation) without overriding the fixture.
    Static,
    /// A local waveform computed from the store clock; no external twin.
    Formula(FormulaSpec),
    /// The value comes from external-twin ingest (`POST /twin/v1/state`),
    /// clamped to `[min, max]`. A fresh sample overrides the base value; a
    /// missing or stale reading marks the field offline.
    External { key: String, min: f64, max: f64 },
}

/// Outcome of resolving one binding at a point in time.
enum Resolution {
    /// Keep the base value untouched (Static source).
    Keep,
    /// Overwrite the bound pointer with this value.
    Set(Value),
    /// The external feed is missing or stale: null the bound pointer and mark
    /// the resource `UnavailableOffline`.
    Offline,
}

/// A dynamic field: a JSON `pointer` inside the resource at `path`, fed by
/// `source`.
#[derive(Debug, Clone)]
pub struct Binding {
    pub path: String,
    pub pointer: String,
    pub source: Source,
    /// Optional MetricId. When set, the resolved value is also published in the
    /// twin MetricReport and streamed via the TelemetryService (P5).
    pub metric: Option<String>,
    /// Optional upper threshold: a resolved value at or above this raises a
    /// `Warning` alert event (cleared when it drops back below).
    pub warning: Option<f64>,
    /// Optional upper threshold: a resolved value at or above this raises a
    /// `Critical` alert event. Takes precedence over `warning`.
    pub critical: Option<f64>,
}

/// One resolved dynamic value at tick time, consumed by the stream-out loop to
/// emit events, refresh the MetricReport, and evaluate alert thresholds.
#[derive(Debug, Clone)]
pub struct StreamSample {
    pub path: String,
    pub pointer: String,
    pub value: Value,
    pub metric: Option<String>,
    pub warning: Option<f64>,
    pub critical: Option<f64>,
}

/// One external-twin reading, with the freshness metadata used to detect a
/// stale feed. Written by ingest, read during resolution.
pub struct Sample {
    pub value: Value,
    pub ts: Instant,
    pub ttl: Duration,
}

/// The resolved binding table for a mockup store, keyed by resource path.
pub struct TwinConfig {
    /// Bindings grouped by the resource path they apply to.
    bindings: HashMap<String, Vec<Binding>>,
    /// Latest external reading per key, written by `POST /twin/v1/state` ingest
    /// and read during resolution. Interior-mutable so ingest works over `&self`.
    external: DashMap<String, Sample>,
    /// Freshness window applied to external samples at ingest: past it a reading
    /// is stale and its binding goes offline.
    freshness_ttl: Duration,
    /// Cadence of the stream-out tick (events + MetricReport refresh).
    tick_interval: Duration,
    /// Reference instant that formula waveforms are measured from.
    start: Instant,
}

impl TwinConfig {
    /// An empty table: no bindings, so `get()` is byte-identical to today.
    pub fn empty() -> Self {
        Self {
            bindings: HashMap::new(),
            external: DashMap::new(),
            freshness_ttl: Duration::from_secs(DEFAULT_FRESHNESS_TTL_SECS),
            tick_interval: Duration::from_secs(DEFAULT_TICK_INTERVAL_SECS),
            start: Instant::now(),
        }
    }

    /// Whether there are no bindings (the common no-`twin.toml` case).
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }

    /// Parse a `twin.toml` sidecar into a binding table.
    pub fn from_toml(text: &str) -> anyhow::Result<Self> {
        let file: TwinFile = toml::from_str(text)?;
        let spec = file.twin;

        let mut bindings: HashMap<String, Vec<Binding>> = HashMap::new();
        for raw in spec.binding {
            let binding = raw.into_binding()?;
            bindings
                .entry(binding.path.clone())
                .or_default()
                .push(binding);
        }

        Ok(Self {
            bindings,
            external: DashMap::new(),
            freshness_ttl: Duration::from_secs(spec.freshness_ttl_seconds),
            tick_interval: Duration::from_secs(spec.tick_interval_seconds.max(1)),
            start: Instant::now(),
        })
    }

    /// Record an external reading for `key`, timestamped now and stamped with the
    /// config's freshness TTL. Overwrites any prior reading for the same key.
    pub fn ingest(&self, key: &str, value: Value) {
        self.external.insert(
            key.to_string(),
            Sample {
                value,
                ts: Instant::now(),
                ttl: self.freshness_ttl,
            },
        );
    }

    /// Whether any binding consumes external readings for `key` (used to report
    /// unknown keys on ingest without failing the batch).
    pub fn has_external_key(&self, key: &str) -> bool {
        self.bindings
            .values()
            .flatten()
            .any(|b| matches!(&b.source, Source::External { key: k, .. } if k == key))
    }

    /// Resolve one binding's source at `now`. Formula uses `elapsed` since start;
    /// External looks up the (possibly stale) ingested reading.
    fn resolve_source(&self, source: &Source, elapsed: Duration, now: Instant) -> Resolution {
        match source {
            Source::Static => Resolution::Keep,
            Source::Formula(spec) => Resolution::Set(json!(spec.eval(elapsed))),
            Source::External { key, min, max } => match self.external.get(key) {
                Some(sample) if now.saturating_duration_since(sample.ts) < sample.ttl => {
                    match sample.value.as_f64() {
                        // Numeric readings are clamped to the declared bounds.
                        Some(v) => Resolution::Set(json!(v.clamp(*min, *max))),
                        // Non-numeric readings (e.g. a state string) pass through.
                        None => Resolution::Set(sample.value.clone()),
                    }
                }
                // Never ingested, or the last reading has aged past its TTL.
                _ => Resolution::Offline,
            },
        }
    }

    /// Cadence of the stream-out tick (events + MetricReport refresh).
    pub fn tick_interval(&self) -> Duration {
        self.tick_interval
    }

    /// Resolve every dynamic binding at `now` into a flat list of samples,
    /// sorted by `(path, pointer)` for deterministic ordering. `Static` bindings
    /// (and sources that yield no value) are skipped. Used by the stream-out
    /// loop; `resolve` remains the read-path (per-`get`) entry point.
    pub fn stream_snapshot(&self, now: Instant) -> Vec<StreamSample> {
        let elapsed = now.saturating_duration_since(self.start);
        let mut out = Vec::new();
        for bindings in self.bindings.values() {
            for binding in bindings {
                let value = match self.resolve_source(&binding.source, elapsed, now) {
                    Resolution::Keep => continue,
                    Resolution::Set(value) => value,
                    // A stale/absent external feed still streams: it reports the
                    // field as offline (null) so consumers see the drop-out.
                    Resolution::Offline => Value::Null,
                };
                out.push(StreamSample {
                    path: binding.path.clone(),
                    pointer: binding.pointer.clone(),
                    value,
                    metric: binding.metric.clone(),
                    warning: binding.warning,
                    critical: binding.critical,
                });
            }
        }
        out.sort_by(|a, b| (&a.path, &a.pointer).cmp(&(&b.path, &b.pointer)));
        out
    }

    /// Apply every binding for `path`, writing each resolved value at its JSON
    /// pointer in `base`. Bindings whose pointer does not resolve, or whose
    /// source yields no value, are skipped.
    pub fn resolve(&self, path: &str, base: &mut Value, now: Instant) {
        let Some(bindings) = self.bindings.get(path) else {
            return;
        };
        let elapsed = now.saturating_duration_since(self.start);
        for binding in bindings {
            match self.resolve_source(&binding.source, elapsed, now) {
                Resolution::Keep => {}
                Resolution::Set(value) => {
                    if let Some(slot) = base.pointer_mut(&binding.pointer) {
                        *slot = value;
                    }
                }
                Resolution::Offline => {
                    // Null the bound reading and, if the resource carries a
                    // Status, mark it offline so a stale feed is unmistakable.
                    if let Some(slot) = base.pointer_mut(&binding.pointer) {
                        *slot = Value::Null;
                    }
                    if let Some(slot) = base.pointer_mut("/Status/State") {
                        *slot = json!("UnavailableOffline");
                    }
                }
            }
        }
    }
}

// --- twin.toml deserialization ---------------------------------------------

/// Top-level `twin.toml`: a single `[twin]` table with `[[twin.binding]]` entries.
#[derive(Deserialize)]
struct TwinFile {
    twin: TwinSpec,
}

#[derive(Deserialize)]
struct TwinSpec {
    #[serde(default = "default_freshness_ttl_seconds")]
    freshness_ttl_seconds: u64,
    #[serde(default = "default_tick_interval_seconds")]
    tick_interval_seconds: u64,
    #[serde(default)]
    binding: Vec<BindingToml>,
}

fn default_freshness_ttl_seconds() -> u64 {
    DEFAULT_FRESHNESS_TTL_SECS
}

fn default_tick_interval_seconds() -> u64 {
    DEFAULT_TICK_INTERVAL_SECS
}

/// A `[[twin.binding]]` entry. `source` selects which of the variant-specific
/// fields are required (`formula` table, or `key`/`min`/`max`).
#[derive(Deserialize)]
struct BindingToml {
    path: String,
    pointer: String,
    #[serde(default)]
    source: BindingSource,
    #[serde(default)]
    formula: Option<FormulaSpec>,
    #[serde(default)]
    key: Option<String>,
    #[serde(default)]
    min: Option<f64>,
    #[serde(default)]
    max: Option<f64>,
    #[serde(default)]
    metric: Option<String>,
    #[serde(default)]
    warning: Option<f64>,
    #[serde(default)]
    critical: Option<f64>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
enum BindingSource {
    #[default]
    Static,
    Formula,
    External,
}

impl BindingToml {
    fn into_binding(self) -> anyhow::Result<Binding> {
        let source = match self.source {
            BindingSource::Static => Source::Static,
            BindingSource::Formula => {
                let spec = self.formula.ok_or_else(|| {
                    anyhow::anyhow!(
                        "binding {} has source=formula but no [formula] table",
                        self.path
                    )
                })?;
                Source::Formula(spec)
            }
            BindingSource::External => {
                let key = self.key.ok_or_else(|| {
                    anyhow::anyhow!("binding {} has source=external but no key", self.path)
                })?;
                let min = self.min.ok_or_else(|| {
                    anyhow::anyhow!("binding {} has source=external but no min", self.path)
                })?;
                let max = self.max.ok_or_else(|| {
                    anyhow::anyhow!("binding {} has source=external but no max", self.path)
                })?;
                Source::External { key, min, max }
            }
        };
        Ok(Binding {
            path: self.path,
            pointer: self.pointer,
            source,
            metric: self.metric,
            warning: self.warning,
            critical: self.critical,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_config_has_no_bindings() {
        let cfg = TwinConfig::empty();
        assert!(cfg.is_empty());
    }

    #[test]
    fn resolve_without_bindings_leaves_value_untouched() {
        let cfg = TwinConfig::empty();
        let mut value = json!({ "Reading": 42 });
        cfg.resolve(
            "/redfish/v1/Chassis/GPU_0/Sensors/Temp0",
            &mut value,
            Instant::now(),
        );
        assert_eq!(value, json!({ "Reading": 42 }));
    }

    #[test]
    fn formula_binding_overwrites_pointer_within_bounds() {
        let toml = r#"
            [twin]
            [[twin.binding]]
            path = "/redfish/v1/Chassis/GPU_0/Sensors/Temp0"
            pointer = "/Reading"
            source = "formula"
            formula = { kind = "sine", min = 20.0, max = 90.0, period_s = 60 }
        "#;
        let cfg = TwinConfig::from_toml(toml).unwrap();
        assert!(!cfg.is_empty());

        let mut value = json!({ "Reading": 0, "Name": "Temp0" });
        cfg.resolve(
            "/redfish/v1/Chassis/GPU_0/Sensors/Temp0",
            &mut value,
            Instant::now(),
        );

        let reading = value["Reading"].as_f64().unwrap();
        assert!(
            (20.0..=90.0).contains(&reading),
            "reading {reading} out of bounds"
        );
        // Untouched fields are preserved.
        assert_eq!(value["Name"], "Temp0");
    }

    #[test]
    fn formula_binding_only_affects_its_own_path() {
        let toml = r#"
            [twin]
            [[twin.binding]]
            path = "/redfish/v1/Chassis/GPU_0/Sensors/Temp0"
            pointer = "/Reading"
            source = "formula"
            formula = { kind = "sine", min = 20.0, max = 90.0, period_s = 60 }
        "#;
        let cfg = TwinConfig::from_toml(toml).unwrap();

        let mut other = json!({ "Reading": 7 });
        cfg.resolve(
            "/redfish/v1/Chassis/GPU_0/Sensors/Power0",
            &mut other,
            Instant::now(),
        );
        assert_eq!(other, json!({ "Reading": 7 }));
    }

    #[test]
    fn resolve_skips_pointer_that_does_not_exist() {
        let toml = r#"
            [twin]
            [[twin.binding]]
            path = "/x"
            pointer = "/Nested/Reading"
            source = "formula"
            formula = { kind = "sawtooth", min = 0.0, max = 1.0, period_s = 10 }
        "#;
        let cfg = TwinConfig::from_toml(toml).unwrap();
        let mut value = json!({ "Other": 1 });
        cfg.resolve("/x", &mut value, Instant::now());
        // Missing pointer => no panic, value unchanged.
        assert_eq!(value, json!({ "Other": 1 }));
    }

    #[test]
    fn static_source_keeps_base_value() {
        let toml = r#"
            [twin]
            [[twin.binding]]
            path = "/x"
            pointer = "/Reading"
            source = "static"
        "#;
        let cfg = TwinConfig::from_toml(toml).unwrap();
        let mut value = json!({ "Reading": 99 });
        cfg.resolve("/x", &mut value, Instant::now());
        assert_eq!(value["Reading"], 99);
    }

    #[test]
    fn formula_source_requires_a_formula_table() {
        let toml = r#"
            [twin]
            [[twin.binding]]
            path = "/x"
            pointer = "/Reading"
            source = "formula"
        "#;
        assert!(TwinConfig::from_toml(toml).is_err());
    }

    #[test]
    fn external_source_requires_key_min_max() {
        let toml = r#"
            [twin]
            [[twin.binding]]
            path = "/x"
            pointer = "/Reading"
            source = "external"
            key = "gpu0.temp_c"
        "#;
        assert!(TwinConfig::from_toml(toml).is_err());
    }

    #[test]
    fn sine_starts_at_min_and_peaks_at_half_period() {
        let spec = FormulaSpec {
            kind: FormulaKind::Sine,
            min: 10.0,
            max: 50.0,
            period_s: 100.0,
        };
        assert!((spec.eval(Duration::from_secs(0)) - 10.0).abs() < 1e-9);
        assert!((spec.eval(Duration::from_secs(50)) - 50.0).abs() < 1e-9);
    }

    #[test]
    fn triangle_peaks_at_half_period() {
        let spec = FormulaSpec {
            kind: FormulaKind::Triangle,
            min: 0.0,
            max: 10.0,
            period_s: 100.0,
        };
        assert!((spec.eval(Duration::from_secs(0)) - 0.0).abs() < 1e-9);
        assert!((spec.eval(Duration::from_secs(50)) - 10.0).abs() < 1e-9);
        assert!((spec.eval(Duration::from_secs(100)) - 0.0).abs() < 1e-9);
    }

    #[test]
    fn stream_snapshot_returns_dynamic_samples_sorted_with_metadata() {
        let toml = r#"
            [twin]
            tick_interval_seconds = 2
            [[twin.binding]]
            path = "/redfish/v1/Chassis/GPU_0/Sensors/Power0"
            pointer = "/Reading"
            source = "formula"
            formula = { kind = "sawtooth", min = 0.0, max = 10.0, period_s = 10 }
            [[twin.binding]]
            path = "/redfish/v1/Chassis/GPU_0/Sensors/Temp0"
            pointer = "/Reading"
            source = "formula"
            formula = { kind = "sine", min = 20.0, max = 90.0, period_s = 60 }
            metric = "GpuTemperature"
            warning = 70.0
            critical = 85.0
            [[twin.binding]]
            path = "/redfish/v1/Chassis/GPU_0/Sensors/Static0"
            pointer = "/Reading"
            source = "static"
        "#;
        let cfg = TwinConfig::from_toml(toml).unwrap();
        assert_eq!(cfg.tick_interval(), Duration::from_secs(2));

        let samples = cfg.stream_snapshot(cfg.start);
        // Static binding yields no value, so only the two formulas appear,
        // sorted by (path, pointer): Power0 before Temp0.
        assert_eq!(samples.len(), 2);
        assert_eq!(samples[0].path, "/redfish/v1/Chassis/GPU_0/Sensors/Power0");
        assert_eq!(samples[0].metric, None);
        assert_eq!(samples[1].path, "/redfish/v1/Chassis/GPU_0/Sensors/Temp0");
        assert_eq!(samples[1].metric.as_deref(), Some("GpuTemperature"));
        assert_eq!(samples[1].warning, Some(70.0));
        assert_eq!(samples[1].critical, Some(85.0));
    }

    #[test]
    fn tick_interval_defaults_and_is_clamped_to_at_least_one_second() {
        let cfg = TwinConfig::from_toml("[twin]\n").unwrap();
        assert_eq!(
            cfg.tick_interval(),
            Duration::from_secs(DEFAULT_TICK_INTERVAL_SECS)
        );

        let zero = TwinConfig::from_toml("[twin]\ntick_interval_seconds = 0\n").unwrap();
        assert_eq!(zero.tick_interval(), Duration::from_secs(1));
    }

    #[test]
    fn external_fresh_sample_clamps_within_bounds() {
        let toml = r#"
            [twin]
            [[twin.binding]]
            path = "/redfish/v1/Chassis/GPU_0/Sensors/Temp0"
            pointer = "/Reading"
            source = "external"
            key = "gpu0.temp_c"
            min = 0.0
            max = 100.0
        "#;
        let cfg = TwinConfig::from_toml(toml).unwrap();
        assert!(cfg.has_external_key("gpu0.temp_c"));
        assert!(!cfg.has_external_key("nope"));

        // Over-max reading is clamped to the bound.
        cfg.ingest("gpu0.temp_c", json!(150.0));
        let mut value = json!({ "Reading": 0.0, "Status": { "State": "Enabled" } });
        cfg.resolve(
            "/redfish/v1/Chassis/GPU_0/Sensors/Temp0",
            &mut value,
            Instant::now(),
        );
        assert_eq!(value["Reading"], json!(100.0));
        // A fresh feed leaves the Status untouched.
        assert_eq!(value["Status"]["State"], "Enabled");
    }

    #[test]
    fn external_without_ingest_marks_offline() {
        let toml = r#"
            [twin]
            [[twin.binding]]
            path = "/redfish/v1/Chassis/GPU_0/Sensors/Temp0"
            pointer = "/Reading"
            source = "external"
            key = "gpu0.temp_c"
            min = 0.0
            max = 100.0
        "#;
        let cfg = TwinConfig::from_toml(toml).unwrap();
        let mut value = json!({ "Reading": 42.0, "Status": { "State": "Enabled" } });
        cfg.resolve(
            "/redfish/v1/Chassis/GPU_0/Sensors/Temp0",
            &mut value,
            Instant::now(),
        );
        // No reading ever ingested => offline.
        assert_eq!(value["Reading"], Value::Null);
        assert_eq!(value["Status"]["State"], "UnavailableOffline");
    }

    #[test]
    fn external_stale_sample_goes_offline() {
        // freshness_ttl_seconds = 0 => any sample is stale the instant it lands.
        let toml = r#"
            [twin]
            freshness_ttl_seconds = 0
            [[twin.binding]]
            path = "/x"
            pointer = "/Reading"
            source = "external"
            key = "k"
            min = 0.0
            max = 10.0
        "#;
        let cfg = TwinConfig::from_toml(toml).unwrap();
        cfg.ingest("k", json!(5.0));
        let mut value = json!({ "Reading": 1.0, "Status": { "State": "Enabled" } });
        cfg.resolve("/x", &mut value, Instant::now());
        assert_eq!(value["Reading"], Value::Null);
        assert_eq!(value["Status"]["State"], "UnavailableOffline");
    }

    #[test]
    fn external_reingest_overrides_previous_reading() {
        let toml = r#"
            [twin]
            [[twin.binding]]
            path = "/x"
            pointer = "/Reading"
            source = "external"
            key = "k"
            min = 0.0
            max = 100.0
        "#;
        let cfg = TwinConfig::from_toml(toml).unwrap();
        cfg.ingest("k", json!(10.0));
        cfg.ingest("k", json!(20.0));
        let mut value = json!({ "Reading": 0.0 });
        cfg.resolve("/x", &mut value, Instant::now());
        assert_eq!(value["Reading"], json!(20.0));
    }

    #[test]
    fn sawtooth_ramps_then_resets() {
        let spec = FormulaSpec {
            kind: FormulaKind::Sawtooth,
            min: 0.0,
            max: 100.0,
            period_s: 100.0,
        };
        assert!((spec.eval(Duration::from_secs(0)) - 0.0).abs() < 1e-9);
        assert!((spec.eval(Duration::from_secs(25)) - 25.0).abs() < 1e-9);
        assert!((spec.eval(Duration::from_secs(75)) - 75.0).abs() < 1e-9);
    }
}
