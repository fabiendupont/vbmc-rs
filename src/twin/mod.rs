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
//!
//! Actuation (P4) closes the control loop: when a client issues a control action
//! (e.g. `ComputerSystem.Reset`), vbmc-rs applies its usual local optimistic
//! mutation for instant feedback and, if a `control_webhook` is configured,
//! relays a [`ControlIntent`] to the twin so the model stays authoritative.
//!
//! Fleet (P5): a node may declare its own `system_id` (`[twin] system_id`). One
//! twin then drives many nodes by keying every sample by `system_id` — see
//! [`TwinConfig::accepts_system_id`]. A node ingests only the samples addressed
//! to it (or carrying no address, i.e. broadcast) and stamps that identity on the
//! [`ControlIntent`]s it emits, so the twin can route the relayed action back to
//! the right modeled node. A node without a configured `system_id` accepts every
//! sample, so single-node behaviour is unchanged.
//!
//! Scenarios (P6 S1): a [`Source::Scenario`] binding replays a named,
//! time-sequenced timeline — the deterministic, reproducible test-case face of
//! the twin (a signal generator that injects peaks, sags and dropouts to exercise
//! a device under test). A top-level
//! `[[scenario]]` block lists ordered segments in BMC-operational terms —
//! `nominal`, `step`, `drift`, `transient`, `fault`, `stuck` — and a binding
//! references it by name. Each value-bearing segment targets either a raw `value`
//! or a named Redfish threshold `level` (e.g. `UpperCritical`), resolved once at
//! load against the sensor's own `Thresholds` so the timeline evaluates as a pure
//! function of elapsed time, exactly like a [`Source::Formula`].
//!
//! Trigger/lifecycle (P6 S2): a scenario runs from the store start by default
//! (so CI can query absolute offsets), but can be *armed* on demand — the
//! equivalent of pressing "inject" on a signal generator. `arm_scenario` re-bases
//! a scenario's timeline to run from now; `reset_scenarios` disarms every one.
//! The control plane lives in [`crate::redfish::mockup_scenario`].
//!
//! Deterministic clock (P6 S3): every twin instant — the store `start`, a
//! scenario's arm time, an external sample's timestamp, and each read/stream
//! resolution — is read through [`now`], which returns
//! [`tokio::time::Instant`]. Under `tokio::time::pause()` that clock is virtual,
//! so an integration test can `tokio::time::advance()` through a timeline and
//! assert the exact projected values and emitted events without sleeping.
//! Outside a paused runtime (i.e. in production) it is real wall-clock time, so
//! behaviour is unchanged.

use std::collections::{BTreeMap, HashMap};
use std::time::{Duration, Instant};

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Default freshness window for external samples when `twin.toml` omits it.
const DEFAULT_FRESHNESS_TTL_SECS: u64 = 10;

/// Default cadence for the stream-out tick when `twin.toml` omits it. The tick
/// drives `ResourceUpdated` events and MetricReport refreshes (see the stream
/// module); it is only spawned when the binding table is non-empty.
const DEFAULT_TICK_INTERVAL_SECS: u64 = 5;

/// The twin's monotonic clock (P6 S3).
///
/// Every twin instant is read here so they all share one clock. It returns a
/// [`std::time::Instant`] taken from [`tokio::time::Instant`], which is virtual
/// under `tokio::time::pause()` — letting a deterministic test advance through a
/// timeline — and real wall-clock time otherwise. `tokio::time::Instant` is a
/// newtype over `std::time::Instant`, so the rest of the module is unchanged.
pub fn now() -> Instant {
    tokio::time::Instant::now().into_std()
}

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

/// One segment of a [`ScenarioTimeline`], named in BMC-operational terms.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScenarioKind {
    /// Hold the target value for the segment's duration (steady healthy reading).
    Nominal,
    /// Jump to the target and hold it (an abrupt change; numerically the same as
    /// [`ScenarioKind::Nominal`], kept as a distinct intent).
    Step,
    /// Linearly ramp from the previous segment's value to the target over the
    /// segment's duration (thermal drift, load ramp).
    Drift,
    /// Excursion: ramp from the entry value up to the target at the segment's
    /// midpoint and back to the entry value by its end (a transient peak/dip).
    Transient,
    /// The reading drops out: the bound field goes null and the resource is
    /// marked `UnavailableOffline` (a sensor/telemetry fault).
    Fault,
    /// Freeze the entry value for the segment's duration (a stuck sensor that
    /// keeps reporting its last value).
    Stuck,
}

/// A segment's target value: either a literal number or a named Redfish
/// threshold level resolved from the target sensor's `Thresholds` at load time.
#[derive(Debug, Clone)]
enum Target {
    /// A literal reading value.
    Value(f64),
    /// A named Redfish threshold level (e.g. `UpperCritical`); resolved to a
    /// [`Target::Value`] by [`TwinConfig::bind_scenarios`] before evaluation.
    Level(String),
}

/// One segment of a scenario timeline.
#[derive(Debug, Clone)]
struct Segment {
    kind: ScenarioKind,
    /// Duration of the segment. `None` on the final segment means it holds
    /// indefinitely.
    duration: Option<Duration>,
    /// The value the segment drives toward. Required for value-bearing kinds
    /// (`nominal`/`step`/`drift`/`transient`); ignored for `fault`/`stuck`.
    target: Option<Target>,
}

/// A named, time-sequenced piecewise signal (P6 S1). Evaluated as a pure
/// function of elapsed time, so it drops into the seam like a [`FormulaSpec`].
#[derive(Debug, Clone)]
pub struct ScenarioTimeline {
    segments: Vec<Segment>,
}

/// Outcome of evaluating a [`ScenarioTimeline`] at a point in time.
enum ScenarioOutcome {
    /// A resolved reading value.
    Value(f64),
    /// A `fault` segment: the field drops out (null + `UnavailableOffline`).
    Offline,
}

impl ScenarioTimeline {
    /// Concrete numeric target of a (bound) segment, if it has one.
    fn segment_value(seg: &Segment) -> Option<f64> {
        match &seg.target {
            Some(Target::Value(v)) => Some(*v),
            // A `Level` should have been resolved by `bind_scenarios`; if one
            // survives, treat the segment as having no numeric target.
            Some(Target::Level(_)) | None => None,
        }
    }

    /// The value the timeline holds at the *end* of segment `i`, given the value
    /// entering it. Ramps/holds end at their target; excursions and stuck/fault
    /// segments end where they started.
    fn end_value(seg: &Segment, entry: f64) -> f64 {
        match seg.kind {
            ScenarioKind::Nominal | ScenarioKind::Step | ScenarioKind::Drift => {
                Self::segment_value(seg).unwrap_or(entry)
            }
            ScenarioKind::Transient | ScenarioKind::Stuck | ScenarioKind::Fault => entry,
        }
    }

    /// Evaluate segment `seg` at fractional progress `frac` (0.0..=1.0) given the
    /// value `entry` on entry.
    fn eval_segment(seg: &Segment, entry: f64, frac: f64) -> ScenarioOutcome {
        match seg.kind {
            ScenarioKind::Nominal | ScenarioKind::Step => {
                ScenarioOutcome::Value(Self::segment_value(seg).unwrap_or(entry))
            }
            ScenarioKind::Drift => {
                let target = Self::segment_value(seg).unwrap_or(entry);
                ScenarioOutcome::Value(entry + (target - entry) * frac)
            }
            ScenarioKind::Transient => {
                let peak = Self::segment_value(seg).unwrap_or(entry);
                // Triangle: 0 at the ends, 1 at the midpoint.
                let tri = if frac < 0.5 {
                    2.0 * frac
                } else {
                    2.0 * (1.0 - frac)
                };
                ScenarioOutcome::Value(entry + (peak - entry) * tri)
            }
            ScenarioKind::Stuck => ScenarioOutcome::Value(entry),
            ScenarioKind::Fault => ScenarioOutcome::Offline,
        }
    }

    /// Evaluate the timeline at `elapsed` since the source's reference instant.
    fn eval(&self, elapsed: Duration) -> ScenarioOutcome {
        let n = self.segments.len();
        if n == 0 {
            return ScenarioOutcome::Offline;
        }
        // Value entering the first segment: its own target (so a leading
        // `nominal`/`step` starts flat), else 0.0.
        let seed = Self::segment_value(&self.segments[0]).unwrap_or(0.0);

        let mut entry = seed;
        let mut acc = Duration::ZERO;
        for seg in &self.segments {
            match seg.duration {
                Some(d) if elapsed < acc + d => {
                    let span = d.as_secs_f64();
                    let frac = if span > 0.0 {
                        (elapsed - acc).as_secs_f64() / span
                    } else {
                        0.0
                    };
                    return Self::eval_segment(seg, entry, frac);
                }
                Some(d) => {
                    entry = Self::end_value(seg, entry);
                    acc += d;
                }
                // No duration: the final segment holds indefinitely.
                None => return Self::eval_segment(seg, entry, 1.0),
            }
        }
        // Past every finite segment: hold the last one at its end.
        let last = &self.segments[n - 1];
        Self::eval_segment(last, entry, 1.0)
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
    /// A named, time-sequenced [`ScenarioTimeline`] (P6 S1), optionally clamped
    /// to `[min, max]`. Evaluated from the scenario's reference instant: the
    /// store start until the scenario is armed (P6 S2), then the arm instant.
    Scenario {
        name: String,
        timeline: ScenarioTimeline,
        min: Option<f64>,
        max: Option<f64>,
    },
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

/// The lifecycle state of one named scenario at a point in time (P6 S2),
/// serialized by `GET`/`POST /twin/v1/scenario`.
#[derive(Debug, Clone, Serialize)]
pub struct ScenarioState {
    /// The scenario name (as referenced by a binding's `scenario = "..."`).
    pub name: String,
    /// `true` when explicitly armed (running from its arm instant); `false`
    /// when idle (running from the store start, the S1 default).
    pub armed: bool,
    /// Seconds elapsed since the scenario's current reference instant.
    pub elapsed_seconds: f64,
    /// The timeline's resolved value at this instant, or `null` when offline.
    pub value: Option<f64>,
    /// `true` when the timeline is in a `fault` (offline) segment right now.
    pub offline: bool,
}

/// A control action a Redfish client issued against a resource, relayed to the
/// external twin so it can apply the effect to its model (P4 actuation). The
/// twin reflects the authoritative result back through the next ingest.
#[derive(Debug, Clone, Serialize)]
pub struct ControlIntent {
    /// The modeled node the action targets (last segment of `path`).
    pub system_id: String,
    /// The Redfish resource the action was issued against.
    pub path: String,
    /// The action name, e.g. `"ComputerSystem.Reset"`.
    pub action: String,
    /// The action's request body (e.g. `{ "ResetType": "ForceOff" }`).
    pub params: Value,
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
    /// Optional URL a [`ControlIntent`] is POSTed to when a client issues a
    /// control action (P4 actuation). `None` keeps today's local-only behaviour.
    control_webhook: Option<String>,
    /// Optional fleet identity for this node (P5). When set, ingest accepts only
    /// samples addressed to this `system_id` (or carrying none), and outbound
    /// control intents are stamped with it. `None` = single-node: accept all.
    system_id: Option<String>,
    /// Reference instant that formula waveforms are measured from.
    start: Instant,
    /// Per-scenario arm instant (P6 S2). A scenario absent from this map runs
    /// from `start` (the S1 default); arming it re-bases its timeline to `now`.
    /// Interior-mutable so arm/reset work over `&self` on the request path.
    armed: DashMap<String, Instant>,
}

impl TwinConfig {
    /// An empty table: no bindings, so `get()` is byte-identical to today.
    pub fn empty() -> Self {
        Self {
            bindings: HashMap::new(),
            external: DashMap::new(),
            freshness_ttl: Duration::from_secs(DEFAULT_FRESHNESS_TTL_SECS),
            tick_interval: Duration::from_secs(DEFAULT_TICK_INTERVAL_SECS),
            control_webhook: None,
            system_id: None,
            start: now(),
            armed: DashMap::new(),
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

        // Build the named scenario timelines first, so bindings can reference
        // them by name. `level` targets stay symbolic here; `bind_scenarios`
        // resolves them once the base resources are loaded.
        let mut scenarios: HashMap<String, ScenarioTimeline> = HashMap::new();
        for sc in file.scenario {
            let segments = sc
                .segment
                .into_iter()
                .map(SegmentToml::into_segment)
                .collect::<anyhow::Result<Vec<_>>>()?;
            if segments.is_empty() {
                anyhow::bail!("scenario {} has no segments", sc.name);
            }
            if scenarios
                .insert(sc.name.clone(), ScenarioTimeline { segments })
                .is_some()
            {
                anyhow::bail!("duplicate scenario name {}", sc.name);
            }
        }

        let mut bindings: HashMap<String, Vec<Binding>> = HashMap::new();
        for raw in spec.binding {
            let binding = raw.into_binding(&scenarios)?;
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
            control_webhook: spec.control_webhook.filter(|s| !s.is_empty()),
            system_id: spec.system_id.filter(|s| !s.is_empty()),
            start: now(),
            armed: DashMap::new(),
        })
    }

    /// The URL control intents are relayed to, if actuation is configured.
    pub fn control_webhook(&self) -> Option<&str> {
        self.control_webhook.as_deref()
    }

    /// This node's fleet identity, if one is configured (P5).
    pub fn system_id(&self) -> Option<&str> {
        self.system_id.as_deref()
    }

    /// Whether a sample addressed to `target` should be ingested by this node.
    ///
    /// A sample with no address (`None`) is a broadcast and is always accepted.
    /// A node with no configured `system_id` has no fleet identity and accepts
    /// every sample. Otherwise the sample is kept only when its `system_id`
    /// matches this node's — a foreign node's telemetry is rejected, mirroring a
    /// real BMC that only ever observes its own hardware.
    pub fn accepts_system_id(&self, target: Option<&str>) -> bool {
        match (self.system_id.as_deref(), target) {
            (_, None) => true,
            (None, Some(_)) => true,
            (Some(mine), Some(theirs)) => mine == theirs,
        }
    }

    /// Record an external reading for `key`, timestamped now and stamped with the
    /// config's freshness TTL. Overwrites any prior reading for the same key.
    pub fn ingest(&self, key: &str, value: Value) {
        self.external.insert(
            key.to_string(),
            Sample {
                value,
                ts: now(),
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
            Source::Scenario {
                name,
                timeline,
                min,
                max,
            } => match timeline.eval(now.saturating_duration_since(self.scenario_origin(name))) {
                ScenarioOutcome::Value(v) => {
                    let v = match (min, max) {
                        (Some(lo), Some(hi)) => v.clamp(*lo, *hi),
                        (Some(lo), None) => v.max(*lo),
                        (None, Some(hi)) => v.min(*hi),
                        (None, None) => v,
                    };
                    Resolution::Set(json!(v))
                }
                ScenarioOutcome::Offline => Resolution::Offline,
            },
        }
    }

    /// Resolve any `level` targets in scenario bindings against the static
    /// `Thresholds` of their target sensor, baking them to numeric values so
    /// evaluation stays a pure function of elapsed time. Called once at load
    /// after the base resources are in place; `base_for` returns the stored
    /// resource for a binding's path. Errors if a referenced level is missing.
    pub fn bind_scenarios(
        &mut self,
        base_for: impl Fn(&str) -> Option<Value>,
    ) -> anyhow::Result<()> {
        for bindings in self.bindings.values_mut() {
            for binding in bindings.iter_mut() {
                let Source::Scenario { timeline, .. } = &mut binding.source else {
                    continue;
                };
                if !timeline
                    .segments
                    .iter()
                    .any(|s| matches!(s.target, Some(Target::Level(_))))
                {
                    continue;
                }
                let base = base_for(&binding.path).ok_or_else(|| {
                    anyhow::anyhow!(
                        "scenario binding {} references a threshold level but the resource is missing",
                        binding.path
                    )
                })?;
                for seg in timeline.segments.iter_mut() {
                    let Some(Target::Level(name)) = &seg.target else {
                        continue;
                    };
                    let ptr = format!("/Thresholds/{name}/Reading");
                    let value = base.pointer(&ptr).and_then(Value::as_f64).ok_or_else(|| {
                        anyhow::anyhow!(
                            "scenario binding {} references threshold level {} but {}{} is not a number",
                            binding.path,
                            name,
                            binding.path,
                            ptr
                        )
                    })?;
                    seg.target = Some(Target::Value(value));
                }
            }
        }
        Ok(())
    }

    /// The reference instant a scenario's timeline is measured from: its arm
    /// instant once armed (P6 S2), else the store start (the S1 default).
    fn scenario_origin(&self, name: &str) -> Instant {
        self.armed.get(name).map(|e| *e).unwrap_or(self.start)
    }

    /// The distinct scenario names driving a binding, sorted for stable output.
    pub fn scenario_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .bindings
            .values()
            .flatten()
            .filter_map(|b| match &b.source {
                Source::Scenario { name, .. } => Some(name.clone()),
                _ => None,
            })
            .collect();
        names.sort();
        names.dedup();
        names
    }

    /// Arm a scenario: re-base its timeline to run from `now`. Returns `false`
    /// (and changes nothing) when no binding drives a scenario of that name.
    pub fn arm_scenario(&self, name: &str) -> bool {
        if !self.scenario_names().iter().any(|n| n == name) {
            return false;
        }
        self.armed.insert(name.to_string(), now());
        true
    }

    /// Disarm every scenario, reverting each timeline to run from the store
    /// start (the S1 default). Returns how many scenarios were armed.
    pub fn reset_scenarios(&self) -> usize {
        let n = self.armed.len();
        self.armed.clear();
        n
    }

    /// A snapshot of every scenario's lifecycle state at `now`, sorted by name.
    /// `armed` distinguishes an explicitly armed scenario (running from its arm
    /// instant) from an idle one (running from the store start).
    pub fn scenario_states(&self, now: Instant) -> Vec<ScenarioState> {
        let mut timelines: BTreeMap<String, &ScenarioTimeline> = BTreeMap::new();
        for bindings in self.bindings.values() {
            for binding in bindings {
                if let Source::Scenario { name, timeline, .. } = &binding.source {
                    timelines.entry(name.clone()).or_insert(timeline);
                }
            }
        }
        timelines
            .into_iter()
            .map(|(name, timeline)| {
                let armed = self.armed.contains_key(&name);
                let elapsed = now.saturating_duration_since(self.scenario_origin(&name));
                let (value, offline) = match timeline.eval(elapsed) {
                    ScenarioOutcome::Value(v) => (Some(v), false),
                    ScenarioOutcome::Offline => (None, true),
                };
                ScenarioState {
                    name,
                    armed,
                    elapsed_seconds: elapsed.as_secs_f64(),
                    value,
                    offline,
                }
            })
            .collect()
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

/// Top-level `twin.toml`: a single `[twin]` table with `[[twin.binding]]`
/// entries, plus optional top-level `[[scenario]]` timelines (P6 S1).
#[derive(Deserialize)]
struct TwinFile {
    twin: TwinSpec,
    #[serde(default)]
    scenario: Vec<ScenarioToml>,
}

/// A top-level `[[scenario]]` block: a named list of `[[scenario.segment]]`.
#[derive(Deserialize)]
struct ScenarioToml {
    name: String,
    #[serde(default)]
    segment: Vec<SegmentToml>,
}

/// A `[[scenario.segment]]` entry. `value` and `level` are mutually exclusive;
/// `fault`/`stuck` need neither.
#[derive(Deserialize)]
struct SegmentToml {
    kind: ScenarioKind,
    #[serde(default)]
    for_s: Option<f64>,
    #[serde(default)]
    value: Option<f64>,
    #[serde(default)]
    level: Option<String>,
}

impl SegmentToml {
    fn into_segment(self) -> anyhow::Result<Segment> {
        let target = match (self.value, self.level) {
            (Some(_), Some(_)) => {
                anyhow::bail!("scenario segment sets both value and level; pick one")
            }
            (Some(v), None) => Some(Target::Value(v)),
            (None, Some(l)) => Some(Target::Level(l)),
            (None, None) => None,
        };
        // Value-bearing kinds need a target; fault/stuck derive from context.
        if matches!(
            self.kind,
            ScenarioKind::Nominal
                | ScenarioKind::Step
                | ScenarioKind::Drift
                | ScenarioKind::Transient
        ) && target.is_none()
        {
            anyhow::bail!(
                "scenario segment kind {:?} needs a value or level",
                self.kind
            );
        }
        let duration = match self.for_s {
            Some(s) if s.is_finite() && s >= 0.0 => Some(Duration::from_secs_f64(s)),
            Some(s) => anyhow::bail!("scenario segment for_s must be finite and >= 0 (got {s})"),
            None => None,
        };
        Ok(Segment {
            kind: self.kind,
            duration,
            target,
        })
    }
}

#[derive(Deserialize)]
struct TwinSpec {
    #[serde(default = "default_freshness_ttl_seconds")]
    freshness_ttl_seconds: u64,
    #[serde(default = "default_tick_interval_seconds")]
    tick_interval_seconds: u64,
    /// Optional URL control intents are POSTed to (P4 actuation).
    #[serde(default)]
    control_webhook: Option<String>,
    /// Optional fleet identity for this node (P5 ingest/intent routing key).
    #[serde(default)]
    system_id: Option<String>,
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
    /// Name of a top-level `[[scenario]]` block (for `source = "scenario"`).
    #[serde(default)]
    scenario: Option<String>,
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
    Scenario,
}

impl BindingToml {
    fn into_binding(
        self,
        scenarios: &HashMap<String, ScenarioTimeline>,
    ) -> anyhow::Result<Binding> {
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
            BindingSource::Scenario => {
                let name = self.scenario.ok_or_else(|| {
                    anyhow::anyhow!(
                        "binding {} has source=scenario but no scenario name",
                        self.path
                    )
                })?;
                let timeline = scenarios.get(&name).cloned().ok_or_else(|| {
                    anyhow::anyhow!("binding {} references unknown scenario {}", self.path, name)
                })?;
                Source::Scenario {
                    name,
                    timeline,
                    min: self.min,
                    max: self.max,
                }
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
    fn control_webhook_is_none_when_absent() {
        let cfg = TwinConfig::from_toml("[twin]\n").unwrap();
        assert_eq!(cfg.control_webhook(), None);
        // An empty string is treated as unset, not a valid URL.
        let blank = TwinConfig::from_toml("[twin]\ncontrol_webhook = \"\"\n").unwrap();
        assert_eq!(blank.control_webhook(), None);
    }

    #[test]
    fn control_webhook_is_parsed_when_set() {
        let cfg =
            TwinConfig::from_toml("[twin]\ncontrol_webhook = \"http://twin/intents\"\n").unwrap();
        assert_eq!(cfg.control_webhook(), Some("http://twin/intents"));
    }

    #[test]
    fn control_intent_serializes_to_expected_shape() {
        let intent = ControlIntent {
            system_id: "Server1".to_string(),
            path: "/redfish/v1/Systems/Server1".to_string(),
            action: "ComputerSystem.Reset".to_string(),
            params: json!({ "ResetType": "ForceOff" }),
        };
        assert_eq!(
            serde_json::to_value(&intent).unwrap(),
            json!({
                "system_id": "Server1",
                "path": "/redfish/v1/Systems/Server1",
                "action": "ComputerSystem.Reset",
                "params": { "ResetType": "ForceOff" },
            })
        );
    }

    #[test]
    fn system_id_is_none_when_absent_or_blank() {
        let cfg = TwinConfig::from_toml("[twin]\n").unwrap();
        assert_eq!(cfg.system_id(), None);
        let blank = TwinConfig::from_toml("[twin]\nsystem_id = \"\"\n").unwrap();
        assert_eq!(blank.system_id(), None);
    }

    #[test]
    fn system_id_is_parsed_when_set() {
        let cfg = TwinConfig::from_toml("[twin]\nsystem_id = \"tray-01\"\n").unwrap();
        assert_eq!(cfg.system_id(), Some("tray-01"));
    }

    #[test]
    fn node_with_identity_accepts_only_its_own_or_unaddressed_samples() {
        let cfg = TwinConfig::from_toml("[twin]\nsystem_id = \"tray-01\"\n").unwrap();
        // Unaddressed broadcast: always accepted.
        assert!(cfg.accepts_system_id(None));
        // Addressed to this node: accepted.
        assert!(cfg.accepts_system_id(Some("tray-01")));
        // Addressed to another node: rejected.
        assert!(!cfg.accepts_system_id(Some("tray-02")));
    }

    #[test]
    fn node_without_identity_accepts_every_sample() {
        let cfg = TwinConfig::from_toml("[twin]\n").unwrap();
        assert!(cfg.accepts_system_id(None));
        assert!(cfg.accepts_system_id(Some("tray-01")));
        assert!(cfg.accepts_system_id(Some("anything")));
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

    // A scenario: hold 20 (0-10s), drift to 80 (10-20s), excursion to 100
    // (20-30s), then hold 30 indefinitely.
    const SCENARIO_TOML: &str = r#"
        [twin]
        [[twin.binding]]
        path = "/x"
        pointer = "/Reading"
        source = "scenario"
        scenario = "ramp-test"

        [[scenario]]
        name = "ramp-test"
        [[scenario.segment]]
        kind = "nominal"
        value = 20.0
        for_s = 10
        [[scenario.segment]]
        kind = "drift"
        value = 80.0
        for_s = 10
        [[scenario.segment]]
        kind = "transient"
        value = 100.0
        for_s = 10
        [[scenario.segment]]
        kind = "nominal"
        value = 30.0
    "#;

    fn reading_at(cfg: &TwinConfig, secs: u64) -> Value {
        let mut value = json!({ "Reading": 0.0, "Status": { "State": "Enabled" } });
        cfg.resolve("/x", &mut value, cfg.start + Duration::from_secs(secs));
        value["Reading"].clone()
    }

    #[test]
    fn scenario_timeline_evaluates_exact_values_at_offsets() {
        let cfg = TwinConfig::from_toml(SCENARIO_TOML).unwrap();
        // Hold segment.
        assert_eq!(reading_at(&cfg, 0), json!(20.0));
        assert_eq!(reading_at(&cfg, 5), json!(20.0));
        // Drift 20 -> 80 over 10s: midpoint is 50.
        assert_eq!(reading_at(&cfg, 15), json!(50.0));
        // Transient peaks to 100 at its midpoint (entry was 80).
        assert_eq!(reading_at(&cfg, 25), json!(100.0));
        // Final unbounded hold.
        assert_eq!(reading_at(&cfg, 30), json!(30.0));
        assert_eq!(reading_at(&cfg, 10_000), json!(30.0));
    }

    #[test]
    fn scenario_fault_segment_marks_offline() {
        let toml = r#"
            [twin]
            [[twin.binding]]
            path = "/x"
            pointer = "/Reading"
            source = "scenario"
            scenario = "drop"
            [[scenario]]
            name = "drop"
            [[scenario.segment]]
            kind = "nominal"
            value = 50.0
            for_s = 5
            [[scenario.segment]]
            kind = "fault"
        "#;
        let cfg = TwinConfig::from_toml(toml).unwrap();
        assert_eq!(reading_at(&cfg, 0), json!(50.0));

        let mut value = json!({ "Reading": 0.0, "Status": { "State": "Enabled" } });
        cfg.resolve("/x", &mut value, cfg.start + Duration::from_secs(10));
        assert_eq!(value["Reading"], Value::Null);
        assert_eq!(value["Status"]["State"], "UnavailableOffline");
    }

    #[test]
    fn scenario_stuck_segment_freezes_entry_value() {
        let toml = r#"
            [twin]
            [[twin.binding]]
            path = "/x"
            pointer = "/Reading"
            source = "scenario"
            scenario = "freeze"
            [[scenario]]
            name = "freeze"
            [[scenario.segment]]
            kind = "nominal"
            value = 42.0
            for_s = 5
            [[scenario.segment]]
            kind = "stuck"
        "#;
        let cfg = TwinConfig::from_toml(toml).unwrap();
        // The stuck segment holds the value on entry (42) indefinitely.
        assert_eq!(reading_at(&cfg, 100), json!(42.0));
    }

    #[test]
    fn scenario_level_target_resolves_from_thresholds() {
        let toml = r#"
            [twin]
            [[twin.binding]]
            path = "/s"
            pointer = "/Reading"
            source = "scenario"
            scenario = "runaway"
            [[scenario]]
            name = "runaway"
            [[scenario.segment]]
            kind = "nominal"
            value = 20.0
            for_s = 10
            [[scenario.segment]]
            kind = "drift"
            level = "UpperCritical"
            for_s = 10
        "#;
        let mut cfg = TwinConfig::from_toml(toml).unwrap();
        cfg.bind_scenarios(|_path| {
            Some(json!({ "Thresholds": { "UpperCritical": { "Reading": 95.0 } } }))
        })
        .unwrap();

        // Drift 20 -> 95 (the resolved UpperCritical) over 10s: midpoint 57.5.
        let mut value = json!({ "Reading": 0.0 });
        cfg.resolve("/s", &mut value, cfg.start + Duration::from_secs(15));
        assert_eq!(value["Reading"], json!(57.5));
    }

    #[test]
    fn scenario_missing_level_is_a_load_error() {
        let toml = r#"
            [twin]
            [[twin.binding]]
            path = "/s"
            pointer = "/Reading"
            source = "scenario"
            scenario = "runaway"
            [[scenario]]
            name = "runaway"
            [[scenario.segment]]
            kind = "step"
            level = "UpperCritical"
        "#;
        let mut cfg = TwinConfig::from_toml(toml).unwrap();
        // Sensor without the referenced threshold => bind fails loudly.
        assert!(
            cfg.bind_scenarios(|_| Some(json!({ "Thresholds": {} })))
                .is_err()
        );
    }

    #[test]
    fn scenario_clamps_to_declared_bounds() {
        let toml = r#"
            [twin]
            [[twin.binding]]
            path = "/x"
            pointer = "/Reading"
            source = "scenario"
            scenario = "hot"
            min = 0.0
            max = 100.0
            [[scenario]]
            name = "hot"
            [[scenario.segment]]
            kind = "step"
            value = 200.0
        "#;
        let cfg = TwinConfig::from_toml(toml).unwrap();
        assert_eq!(reading_at(&cfg, 1), json!(100.0));
    }

    #[test]
    fn scenario_source_rejects_unknown_or_missing_name() {
        // References a scenario that was never declared.
        let unknown = r#"
            [twin]
            [[twin.binding]]
            path = "/x"
            pointer = "/Reading"
            source = "scenario"
            scenario = "nope"
        "#;
        assert!(TwinConfig::from_toml(unknown).is_err());

        // source = scenario but no name given.
        let nameless = r#"
            [twin]
            [[twin.binding]]
            path = "/x"
            pointer = "/Reading"
            source = "scenario"
        "#;
        assert!(TwinConfig::from_toml(nameless).is_err());

        // A value-bearing segment with neither value nor level.
        let no_target = r#"
            [twin]
            [[twin.binding]]
            path = "/x"
            pointer = "/Reading"
            source = "scenario"
            scenario = "bad"
            [[scenario]]
            name = "bad"
            [[scenario.segment]]
            kind = "nominal"
        "#;
        assert!(TwinConfig::from_toml(no_target).is_err());
    }

    #[test]
    fn scenario_appears_in_stream_snapshot() {
        let cfg = TwinConfig::from_toml(SCENARIO_TOML).unwrap();
        let samples = cfg.stream_snapshot(cfg.start + Duration::from_secs(15));
        assert_eq!(samples.len(), 1);
        assert_eq!(samples[0].path, "/x");
        assert_eq!(samples[0].value, json!(50.0));
    }

    // --- P6 S2 lifecycle (arm / list / reset) ---

    #[test]
    fn scenario_names_lists_referenced_scenarios() {
        let cfg = TwinConfig::from_toml(SCENARIO_TOML).unwrap();
        assert_eq!(cfg.scenario_names(), vec!["ramp-test".to_string()]);
    }

    #[test]
    fn arm_rebases_the_timeline_to_now() {
        let mut cfg = TwinConfig::from_toml(SCENARIO_TOML).unwrap();
        // Pretend the store started 100s ago: idle scenarios run from `start`,
        // so the timeline has already reached its final unbounded hold (30.0).
        cfg.start = Instant::now() - Duration::from_secs(100);

        let idle = &cfg.scenario_states(Instant::now())[0];
        assert_eq!(idle.name, "ramp-test");
        assert!(!idle.armed);
        assert!(idle.elapsed_seconds >= 100.0);
        assert_eq!(idle.value, Some(30.0));

        // Arming re-bases the clock to now: the timeline restarts at its seed
        // (the first nominal segment, 20.0) with a near-zero elapsed.
        assert!(cfg.arm_scenario("ramp-test"));
        let armed = &cfg.scenario_states(Instant::now())[0];
        assert!(armed.armed);
        assert!(armed.elapsed_seconds < 1.0);
        assert_eq!(armed.value, Some(20.0));
    }

    #[test]
    fn arm_unknown_scenario_is_rejected() {
        let cfg = TwinConfig::from_toml(SCENARIO_TOML).unwrap();
        assert!(!cfg.arm_scenario("no-such-scenario"));
        assert!(cfg.arm_scenario("ramp-test"));
    }

    #[test]
    fn reset_disarms_every_scenario() {
        let mut cfg = TwinConfig::from_toml(SCENARIO_TOML).unwrap();
        cfg.start = Instant::now() - Duration::from_secs(100);

        assert!(cfg.arm_scenario("ramp-test"));
        assert!(cfg.scenario_states(Instant::now())[0].armed);

        // Reset reports how many were armed and reverts them to store-start.
        assert_eq!(cfg.reset_scenarios(), 1);
        let after = &cfg.scenario_states(Instant::now())[0];
        assert!(!after.armed);
        assert_eq!(after.value, Some(30.0));
        // Nothing left to disarm the second time.
        assert_eq!(cfg.reset_scenarios(), 0);
    }

    #[test]
    fn armed_scenario_drives_the_read_path_from_arm_time() {
        let mut cfg = TwinConfig::from_toml(SCENARIO_TOML).unwrap();
        cfg.start = Instant::now() - Duration::from_secs(100);
        // Idle: read path is at the final hold.
        assert_eq!(reading_at(&cfg, 100), json!(30.0));

        // After arming, the read path resolves from the arm instant: at ~now the
        // timeline is back in its opening nominal segment.
        assert!(cfg.arm_scenario("ramp-test"));
        let mut value = json!({ "Reading": 0.0, "Status": { "State": "Enabled" } });
        cfg.resolve("/x", &mut value, Instant::now());
        assert_eq!(value["Reading"], json!(20.0));
    }

    // --- P6 S3 deterministic clock (virtual time under tokio::time::pause) ---

    #[tokio::test(start_paused = true)]
    async fn read_path_follows_the_virtual_clock() {
        // Built inside the paused runtime, so `start` is the frozen virtual base
        // and every resolution reads the same virtual clock via `now()`.
        let cfg = TwinConfig::from_toml(SCENARIO_TOML).unwrap();

        let read = |cfg: &TwinConfig| {
            let mut v = json!({ "Reading": 0.0, "Status": { "State": "Enabled" } });
            cfg.resolve("/x", &mut v, now());
            v["Reading"].clone()
        };

        // t=0: opening nominal hold.
        assert_eq!(read(&cfg), json!(20.0));

        // Advance 15s of virtual time (no sleeping): mid-drift 20 -> 80.
        tokio::time::advance(Duration::from_secs(15)).await;
        assert_eq!(read(&cfg), json!(50.0));

        // Advance to t=30: the final unbounded hold.
        tokio::time::advance(Duration::from_secs(15)).await;
        assert_eq!(read(&cfg), json!(30.0));
    }

    #[tokio::test(start_paused = true)]
    async fn arm_rebases_to_the_advanced_virtual_now() {
        let cfg = TwinConfig::from_toml(SCENARIO_TOML).unwrap();

        // Let idle virtual time run out the whole schedule.
        tokio::time::advance(Duration::from_secs(30)).await;
        let idle = &cfg.scenario_states(now())[0];
        assert!(!idle.armed);
        assert_eq!(idle.value, Some(30.0));

        // Arming re-bases to the (already advanced) virtual now: back to the seed.
        assert!(cfg.arm_scenario("ramp-test"));
        let armed = &cfg.scenario_states(now())[0];
        assert!(armed.armed);
        assert!(armed.elapsed_seconds < 1.0);
        assert_eq!(armed.value, Some(20.0));

        // Advancing 15s from the arm instant lands mid-drift again.
        tokio::time::advance(Duration::from_secs(15)).await;
        assert_eq!(cfg.scenario_states(now())[0].value, Some(50.0));
    }

    #[tokio::test(start_paused = true)]
    async fn external_feed_goes_stale_on_the_virtual_clock() {
        let toml = r#"
            [twin]
            freshness_ttl_seconds = 5
            [[twin.binding]]
            path = "/x"
            pointer = "/Reading"
            source = "external"
            key = "k"
            min = 0.0
            max = 100.0
        "#;
        let cfg = TwinConfig::from_toml(toml).unwrap();
        cfg.ingest("k", json!(42.0));

        let read = |cfg: &TwinConfig| {
            let mut v = json!({ "Reading": 0.0, "Status": { "State": "Enabled" } });
            cfg.resolve("/x", &mut v, now());
            v
        };

        // Within the TTL the reading is served.
        assert_eq!(read(&cfg)["Reading"], json!(42.0));

        // Advance past the freshness window: the sample reads stale -> offline.
        tokio::time::advance(Duration::from_secs(6)).await;
        let v = read(&cfg);
        assert_eq!(v["Reading"], Value::Null);
        assert_eq!(v["Status"]["State"], "UnavailableOffline");
    }
}
