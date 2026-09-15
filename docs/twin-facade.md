# Digital Twin Façade

vbmc-rs can act as the **north-facing Redfish interface of a digital twin**. A
twin is a live model of physical infrastructure — thermal, power, workload,
aging. What a twin lacks is a standards-compliant management surface that
existing tools (nv-rms, Ironic, OpenShift, monitoring stacks) can talk to as if
it were real hardware. vbmc-rs already *is* that surface.

This document describes how to drive vbmc-rs's served state from an external
twin instead of from static mockup files or local formulas.

## Design principle

The twin owns the model; vbmc-rs owns the interface. No physics enters vbmc-rs.
The twin computes state and pushes it; vbmc-rs projects that state onto Redfish
and relays control actions back to the twin. Everything below is one seam:
**where a served field's value comes from.**

Chosen deployment shape:

- **External twin process.** The twin is a separate service (any language) that
  feeds vbmc-rs over HTTP. This keeps the model decoupled from vbmc-rs's release
  cycle and language, and scales to many nodes.
- **Push with freshness TTL.** The twin pushes state on its own tick. vbmc-rs
  serves the last-known value and marks a resource offline when its sample goes
  stale. The read path never blocks on the twin, and many event/SSE subscribers
  can fan out from one cached state.

## The seam: base document + overlay resolution

Today `MockupStore::get()` returns a verbatim clone of a stored resource. The
twin façade splits the store into a structural base and a resolved overlay:

- **Base store** (`resources: DashMap<String, Value>`) — structure and static
  fields, populated by `generate()` or `load()`. Unchanged.
- **Binding table** — a list of `Binding { path, pointer, source }` describing
  which fields are dynamic and where their values come from.
- **External value map** — `DashMap<(SystemId, Key), Sample>`, written by
  ingest, read during resolution.

```rust
pub enum Source {
    Static,
    Formula(FormulaSpec),                       // local waveform, no twin
    External { key: String, min: f64, max: f64 }, // value comes from twin ingest
}

pub struct Binding {
    pub path: String,      // e.g. "/redfish/v1/Chassis/GPU_0/Sensors/Temp0"
    pub pointer: String,   // JSON pointer, e.g. "/Reading"
    pub source: Source,
}

pub struct Sample {
    pub value: serde_json::Value,
    pub ts: std::time::Instant,
    pub ttl: std::time::Duration,
}
```

`get(path)` clones the base, then applies every binding for `path` by writing
its resolved value at the JSON pointer. `Formula` and `External` are two source
kinds behind one mechanism — a local formula is the degenerate, zero-model twin;
an external twin is the same hook with a live feed. With no bindings, behaviour
is identical to today, so the refactor is regression-safe.

## Read path and freshness

Each `External` sample carries a timestamp and TTL. On `get()`, if a sample is
stale past its TTL, vbmc-rs does **not** serve stale-but-plausible data. It sets
the resource to an offline state (`Status.State = "UnavailableOffline"`,
`Reading = null`). This surfaces twin-feed gaps instead of hiding them, and
mirrors how a real BMC reports a sensor whose source has dropped.

`External` values are clamped to the binding's `[min, max]` so a
misbehaving twin cannot emit physically impossible readings, and the same bounds
double as alert thresholds (see Streaming out).

## Ingest: twin to vbmc-rs

The primary channel is a dedicated, non-Redfish ingest endpoint:

- `POST /twin/v1/state` with a batch body:

  ```json
  [
    { "system_id": "tray-01", "key": "gpu0.temp_c",  "value": 74.2, "ts": "..." },
    { "system_id": "tray-01", "key": "gpu0.power_w", "value": 312.0, "ts": "..." }
  ]
  ```

Each accepted sample writes the external value map, stamps freshness, and
publishes a `StateChange` on an in-process broadcast bus. Batching keeps the
twin's per-tick cost to one request per node (or one for a fleet), the base
store stays pristine, and this is the natural emit point for events and metric
samples.

Redfish `PATCH` (already handled by `mockup_fallback`) remains available for
ad-hoc or manual state pokes, but it mutates the base and blurs the
structure-versus-telemetry split, so it is not the twin's main channel.

## Streaming out: vbmc-rs to consumers

Today the mockup/simulate router (`mockup_router`) is fallback-only, so the
typed `EventService`/SSE and `TelemetryService` handlers are not mounted. The
façade mounts them over the mockup store and drives them from the `StateChange`
bus:

- **Events / SSE** — `ResourceUpdated` notifications and threshold-crossing
  alerts are pushed to the existing subscription store and the
  `/redfish/v1/EventService/SSE` endpoint.
- **MetricReports** — `MetricDefinition`s are declared for the physical signals
  (GPU temperature, power, utilization, ECC, fan), and MetricReports are
  assembled from current overlay values, periodically or on change.
- **Bounds do double duty** — a binding's `[min, max]` is both the clamp and the
  alert threshold; crossing it emits a Redfish alert event.

This gives a twin consumer a real push/stream interface in simulate mode, not
just polling.

## Actuation: consumers back to the twin

`mockup_fallback` already intercepts `ComputerSystem.Reset`. The façade
generalizes this into an interception layer that emits a
`ControlIntent { system_id, path, action, params }` to the twin over a webhook
(`POST` to a configured URL). The twin applies the intent to its model and
reflects the result back through ingest.

The local optimistic mutation (an immediate `PowerState` flip) is kept as
instant client feedback; the twin remains authoritative on the next ingest.
This closes the what-if/control loop through standard Redfish verbs: a client
issues a reset, the twin models the response, and the client observes it through
the same surface.

## Fleet and scale

One modeled node maps to one store/endpoint. The aggregator and per-VM sidecar
discovery (see [KubeVirt](kubevirt.md)) map a modeled rack onto a fleet of
addressable BMCs. Ingest samples and control intents are keyed by `system_id`,
so a single twin drives many nodes. Example targets:

- A **GB200 NVL** rack: the GB200 Redfish mockup tree for structure, plus
  twin-driven per-tray telemetry.
- A **BlueField BMC** (DPU): an OpenBMC-based Redfish tree for structure, plus
  twin-driven DPU telemetry. Standard Redfish clients — and nv-rms, which
  recognizes BlueField targets — talk to it unchanged.

## Configuration

A `[twin]` section (or a `twin.toml` sidecar in the mockup directory):

```toml
[twin]
mode = "external"                 # external twin process
ingest = "http"                   # POST /twin/v1/state
control = "webhook"
control_webhook = "http://twin/intents"
freshness_ttl_seconds = 10

[[twin.binding]]
path = "/redfish/v1/Chassis/GPU_0/Sensors/Temp0"
pointer = "/Reading"
source = "external"
key = "gpu0.temp_c"
min = 0.0
max = 120.0                       # clamp and alert threshold
metric = "GpuTemperature"        # also expose via TelemetryService

[[twin.binding]]
path = "/redfish/v1/Chassis/GPU_0/Sensors/Power0"
pointer = "/Reading"
source = "formula"               # local fallback when no twin is attached
formula = { kind = "sine", min = 100.0, max = 350.0, period_s = 60 }
```

## Data flow

```text
      control intents (Reset/boot/…)          ingest (batched state, ts)
  ┌──────────────◄──────────────────┐     ┌──────────────►──────────────┐
  │                                 │     │                             │
Redfish client             ┌────────┴─────┴──────────┐            External TWIN
(nv-rms, Ironic,   ◄────►  │        vbmc-rs          │            (the model:
 monitoring)   Redfish     │  base store + bindings  │             thermal/power/
  GET/PATCH/SSE/           │  external value map     │             workload/aging)
  MetricReports            │  StateChange bus ──► SSE/Events/MetricReports
                           └─────────────────────────┘
```

## Implementation phases

| Phase | Scope |
|-------|-------|
| P0 | Seam refactor: base + binding-table resolution in `MockupStore::get()`; `Static` reproduces today's behaviour. Guarded by existing replay sequences. |
| P1 | Formula source: local waveforms with bounds. Proves the seam with no twin dependency. |
| P2 | External source and ingest: external value map, `POST /twin/v1/state`, freshness/TTL, offline-on-stale. |
| P3 | Stream out: mount EventService/SSE and TelemetryService over the mockup store; `StateChange` drives events and MetricReports; bound crossings raise alerts. |
| P4 | Actuation: `ControlIntent` webhook; close the control loop. |
| P5 | Fleet: per-node ingest and intent routing through the aggregator. |

## Testing

Extend the file-based replay harness (`tests/replay.rs`) with range, approximate,
and monotonic matchers (`body_matches`) so twin-driven behaviour is assertable
without pinning exact volatile values. For deterministic assertions, seed
`Formula` sources from a mock clock so a sequence can assert exact values.

## Boundary

vbmc-rs is the data and control plane, not the twin's model. It must not grow a
physics or simulation engine: the twin computes, vbmc-rs projects. Within that
boundary the plumbing largely exists already — PATCH ingest, SSE, the
TelemetryService, and the per-node aggregator fleet — and the façade is mostly a
matter of wiring the seam and mounting the streaming routes over the mockup
store.
