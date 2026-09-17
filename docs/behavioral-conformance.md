# Behavioral Conformance Harness

Schema conformance (see [Redfish Conformance](conformance.md)) asks *"is the
document shaped right?"* — it validates a resource against the Redfish schema at
a single point in time. The **behavioral conformance harness** asks the dynamic
question: *"does the device behave right as its state evolves?"* It replays a
timeline of stimulus against a **live** BMC and asserts on how readings,
MetricReports, and events change over that timeline.

The harness ships as `vbmc-rs-scenario`, a Redfish **client** — it is not part of
the served BMC. It is compiled only when the `scenario-harness` feature is
enabled, so default builds and the served product are byte-identical and never
carry harness code. This preserves the twin-façade boundary (see
[Digital Twin Façade](twin-facade.md)): vbmc-rs projects state, it does not test
itself from the inside.

## Stimulus and observation

Any behavioral test splits into two halves: a **stimulus** that makes the system
change, and an **observation** that checks how it responds — a signal generator
and an oscilloscope, a load step and a strain gauge. The two halves have very
different portability, and the harness keeps them separate for exactly that
reason:

- **Stimulus face** — *"make state move."* How you drive a device is
  device-specific. This is isolated behind a pluggable `StimulusDriver`, chosen
  once via `--driver`, and is the only simulator-aware part of the harness.
- **Verification face** — *"check the response."* How you observe is **standard
  Redfish** (GET reads, MetricReports, the `EventService` SSE stream) and
  therefore **device-agnostic** — the same verification runs against any BMC.

Because the verification face is plain Redfish, the harness works against real
hardware, not just vbmc-rs. Only the stimulus driver changes.

### The verifier is shared, not re-implemented

The in-process replay test (`tests/replay.rs`, deterministic CI) and this
over-the-wire harness consume **one** source of truth: the scenario schema and
the pure verifiers (`json_contains`, `check_matchspec`, `event_matches`) in
`src/scenario/mod.rs`. Event matching is normalized through an `ObservedEvent`
that both an in-process `RedfishEvent` (Rust field names) and an over-the-wire
SSE frame (PascalCase `EventType`/`Severity`/…) map into, so the same
`EventMatch` criteria evaluate identically regardless of source. The two callers
cannot drift apart because they run the same code.

## Building and running

```sh
# The harness is behind a default-off feature.
cargo build --features scenario-harness --bin vbmc-rs-scenario

vbmc-rs-scenario \
  --target https://127.0.0.1:8443 --insecure --driver twin \
  --scenario tests/sequences/twin/scenario_read_path.json
```

Point it at a live [simulate-mode](simulate.md) instance (or any Redfish BMC) and
it prints a per-step pass/fail report, exiting non-zero if any step fails:

```text
harness: target=https://127.0.0.1:8443 driver=Twin timescale=1 — 1 scenario file(s)
▶ scenario_read_path.json: <description>
  ok    <step name>
  FAIL  <step name>: Reading=99 not in [30, 40]

1 passed, 1 failed (2 steps)
```

### CLI

| Flag | Default | Meaning |
|------|---------|---------|
| `--target <url>` | *(required)* | Base URL of the BMC under test, e.g. `https://127.0.0.1:8443`. |
| `--scenario <FILE\|DIR>` | *(required, repeatable)* | A scenario file, or a directory whose `*.json` files are run in sorted order. |
| `--driver twin\|observe` | `twin` | Which stimulus driver presses "inject" (see below). |
| `--insecure` | off | Accept self-signed/invalid TLS certs (simulate mode serves self-signed). |
| `--timescale <f64>` | `1.0` | Real seconds per scenario second. |

## Drivers (the stimulus face)

`--driver` selects one `StimulusDriver` for the whole run. Each driver implements
three verbs — `arm`, `ingest`, `reset` — any of which a driver may legitimately
skip.

### `twin` — `TwinIngestDriver`

Drives a vbmc-rs [simulate](simulate.md) instance through its twin control-plane:

| Verb | Call |
|------|------|
| `arm` | `POST {target}/twin/v1/scenario/{name}` — re-base a named scenario's timeline to now (`404` if the scenario is unknown). |
| `ingest` | `POST {target}/twin/v1/state` — inject external-twin readings as the fleet array `[{ system_id?, key, value }]`. |
| `reset` | `DELETE {target}/twin/v1/scenario` — disarm every scenario. |

### `observe` — `ObserveOnlyDriver`

A genuine no-op for **real hardware**: an operator injects stimulus out-of-band
(a bench, a thermal chamber), and the harness only observes. Every verb logs that
stimulus is external and returns success, so the same scenario file runs the
verification face against a real BMC while the physical stimulus is applied by
hand. This proves the driver seam end-to-end without a physics engine in vbmc-rs.

## Step pacing over the wire

Unlike the in-process path — which runs under a paused tokio clock and drains an
in-memory event bus — the remote clock **cannot be paused**. So each step's
`advance_seconds` becomes a real sleep of `advance_seconds * timescale` seconds,
and when a step expects events the harness spends that window reading a freshly
opened SSE stream (opened *after* stimulus, so alerts the window produces are
captured).

`--timescale` scales that wall-clock window. `2.0` runs a scenario at half speed
(useful when a slow tick interval needs room); below `1.0` runs faster than
scenario time and is only sound if the BMC's tick interval is scaled to match.

Two consequences follow from a clock you can't pause:

- **Prefer `ingest`/`arm` stimulus for exact-value assertions.** External-twin
  readings resolve *live* on GET (ingest `42` → the next GET reads `42.0`), so an
  ingest-driven step is deterministic over the wire regardless of latency.
- **A scenario armed at store-start drifts.** A time-based scenario keeps running
  on the server's wall clock from boot, so uncontrolled boot→harness latency
  makes exact-value assertions at tick granularity flaky. Re-base it with a step
  `arm` (or assert with tolerant `range`/`approx` matchers) rather than pinning an
  exact value against the free-running boot clock.

## Scenario file format

The harness replays the **same** `tests/sequences/twin/*.json` files as the
in-process replay test. A file is a `TwinSequence`: an inline `twin.toml`, a small
resource fixture, and an ordered list of steps.

```json
{
  "description": "A temperature spike crosses Warning then Critical then clears",
  "twin": "[twin]\ntick_interval_seconds = 5\n...",
  "fixture": {
    "/redfish/v1/Chassis/GPU_0/Sensors/Temp0": {
      "@odata.id": "/redfish/v1/Chassis/GPU_0/Sensors/Temp0",
      "Id": "Temp0", "ReadingType": "Temperature", "Reading": 0.0
    }
  },
  "steps": [
    {
      "name": "peak at t=20 emits a Critical alert and the report reads 100",
      "advance_seconds": 5,
      "request": {
        "method": "GET",
        "path": "/redfish/v1/TelemetryService/MetricReports/TwinMetrics"
      },
      "expect": {
        "status": 200,
        "body_contains": { "Id": "TwinMetrics" },
        "body_matches": [
          { "path": "MetricValues.0.MetricValue", "approx": 100.0, "tol": 0.5 }
        ]
      },
      "expect_events": [
        { "event_type": "Alert", "severity": "Critical",
          "message_id_contains": "ThresholdCrossed" }
      ]
    }
  ]
}
```

- `description` — human-readable label printed as the file header.
- `twin` — inline `twin.toml` (bindings + scenarios) driving the store. See
  [Simulate Mode](simulate.md) for the binding schema. Used by the in-process
  replay test; when running over the wire, the target's own `twin.toml` supplies
  the bindings and this field documents intent.
- `fixture` — resource tree by Redfish path; each becomes an `index.json` the
  store loads so the bound read path resolves.
- `steps` — the timeline.

### Step fields

| Field | Applies to | Meaning |
|-------|-----------|---------|
| `name` | all | Step label in the report. |
| `advance_seconds` | all | Wall-clock seconds to advance (× `--timescale`) before verifying. |
| `arm` | `twin` driver | Scenario name to re-base to now before this step. `observe` skips it. |
| `ingest` | `twin` driver | External-twin samples `[{ system_id?, key, value }]` to inject. `observe` skips them. |
| `request` | verification | A `GET` read path to verify. (The over-the-wire harness verifies read paths; non-`GET` methods are rejected.) |
| `expect` | verification | Assertions on the response (see below). |
| `expect_events` | verification | Events that must appear on the SSE stream during the step window. |

`arm` and `ingest` are additive and default-absent, so existing scenario files —
whose scenarios are armed at store-start via inline `twin.toml` — remain valid for
the in-process test unchanged.

### `expect` assertions

| Field | Meaning |
|-------|---------|
| `status` | Required HTTP status (the twin read/MetricReport paths return `200`). |
| `body_contains` | Recursive subset: every key/element must be present and match. |
| `body_equals` | Exact document equality. |
| `body_lacks` | Dot-delimited paths that must be **absent** (assert a reset/delete without pinning volatile etags). |
| `body_matches` | Typed numeric matchers (below). |

### `body_matches` — typed numeric matchers

Each matcher targets a dot-delimited `path` (descending arrays by index, e.g.
`MetricValues.0.MetricValue`). Values rendered as JSON strings — a Redfish
`MetricValue` is a string even when numeric — are parsed, so one matcher works on
Readings and MetricValues alike. Any subset of constraints may be present; all
present constraints must hold.

| Constraint | Meaning |
|------------|---------|
| `range: [min, max]` | Inclusive bounds. |
| `approx` + `tol` | Within tolerance of a target (`tol` default `1e-6`). |
| `gte` / `lte` | Lower / upper bound. |
| `monotonic: increasing\|decreasing` (+ `series`) | Records the value into a named cross-step series (default: the `path`) and asserts direction vs. the previous observation. |

Tolerant matchers (`range`, `approx`, `monotonic`) are what make twin-driven
behavior assertable without pinning exact volatile values.

### `expect_events` — event matching

Every listed `EventMatch` must be satisfied by some event observed on the SSE
stream during the step window. Each present field must match:

| Field | Matches against |
|-------|-----------------|
| `event_type` | `EventType` (exact). |
| `severity` | `Severity` (exact). |
| `message_id_contains` | Substring of `MessageId`. |
| `origin_of_condition` | `OriginOfCondition` (exact). |

## Where it fits

| | In-process (track a) | Over the wire (track b) |
|---|---|---|
| Runner | `cargo test --test replay` | `vbmc-rs-scenario` |
| Clock | Paused tokio virtual clock | Real wall clock (`--timescale`) |
| Events | In-memory event bus | `EventService` SSE stream |
| Stimulus | Armed at store-start via `twin.toml` | `StimulusDriver` (`twin`/`observe`) |
| Target | A `spawn_stream` task in the test process | Any live Redfish BMC |
| Verifier | `src/scenario/mod.rs` (shared) | `src/scenario/mod.rs` (shared) |

The in-process test gives deterministic CI; the harness proves the same behavior
against a running server — and, with `--driver observe`, against real hardware.
```
