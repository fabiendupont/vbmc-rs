# Simulate Mode

`vbmc-rs simulate` starts a Redfish BMC fleet with **no config file and no
hypervisor**. It has two shapes:

- **Generated fleet** — spin up N in-memory servers with one flag, for fleet
  management development, monitoring dashboards, and load testing.
- **Mockup directory** — serve an existing DMTF Redfish mockup tree (`--dir`),
  optionally driven by a live digital twin.

Simulate is the mockup backend wired to a zero-config entry point. For the
config-file route and the mockup directory format, see [Mockup Mode](mockup.md).

## Quick start

```sh
# Generate a fleet of 50 servers on http://127.0.0.1:8000
vbmc-rs simulate --systems 50
curl -s http://127.0.0.1:8000/redfish/v1/Systems | jq .

# Serve an existing mockup directory
vbmc-rs simulate --dir ./my-bmc-mockup --port 8443

# A power action actually changes state
curl -X POST http://127.0.0.1:8000/redfish/v1/Systems/node-01/Actions/ComputerSystem.Reset \
  -H 'Content-Type: application/json' -d '{"ResetType": "ForceOff"}'
```

## CLI

| Flag | Default | Meaning |
|------|---------|---------|
| `--systems, -s <N>` | `1` | Number of generated servers. Ignored when `--dir` is given. |
| `--port, -p <PORT>` | `8000` | Listen port (binds `127.0.0.1`). |
| `--dir <DIR>` | — | Serve a Redfish mockup directory (`index.json` tree) instead of generating a fleet. |
| `--cert <PEM>` | — | TLS certificate. |
| `--key <PEM>` | — | TLS private key. |

TLS is enabled only when **both** `--cert` and `--key` are given; otherwise the
server is plain HTTP. Without TLS, no `--insecure` is needed by clients (including
the [behavioral harness](behavioral-conformance.md)).

## Generated fleet

`--systems N` generates N in-memory ComputerSystems (`node-01`, `node-02`, …)
with synthetic hardware, served as a full Redfish tree. State mutations
(power actions, `PATCH`) apply to the in-memory store. This is the fastest way to
put many Redfish endpoints in front of a client under test.

## Mockup directory

`--dir <DIR>` loads a DMTF Redfish mockup — a directory tree of `index.json`
files mirroring the Redfish URI structure — and serves it live, with the same
stateful power/`PATCH` mutations as the mockup backend. See
[Mockup Mode](mockup.md) for the directory format and how to create one from a
real BMC. This is what lets simulate stand in for a specific target (a GB200 NVL
tree, a BlueField DPU BMC, a scraped Dell/HPE BMC).

## Driving a digital twin

If the mockup directory contains a `twin.toml` sidecar, simulate becomes the
north-facing Redfish interface of a **digital twin** — dynamic fields resolve
from twin state, and the `EventService`/`TelemetryService` stream out changes.
The twin tick (`spawn_stream`) starts automatically when a `twin.toml` is present
and is a no-op otherwise, so a plain mockup is unaffected. See
[Digital Twin Façade](twin-facade.md) for the full design.

### `twin.toml` sidecar

Place `twin.toml` at the root of the `--dir` directory. It declares a tick
interval, per-field **bindings** (which served field is dynamic and where its
value comes from), and optional named **scenarios**.

```toml
[twin]
tick_interval_seconds = 5

# An externally-fed reading: value comes from twin ingest, clamped to [min, max].
[[twin.binding]]
path = "/redfish/v1/Chassis/GPU_0/Sensors/Temp1"
pointer = "/Reading"
source = "external"
key = "gpu.temp"
min = 0.0
max = 120.0

# A scenario-driven reading with alert thresholds and a MetricReport entry.
[[twin.binding]]
path = "/redfish/v1/Chassis/GPU_0/Sensors/Temp0"
pointer = "/Reading"
source = "scenario"
scenario = "spike"
metric = "GpuTemperature"
warning = 60.0
critical = 90.0

# A named scenario: an ordered list of segments over time.
[[scenario]]
name = "spike"
[[scenario.segment]]
kind = "nominal"
value = 20.0
for_s = 10
[[scenario.segment]]
kind = "drift"
value = 100.0
for_s = 10
[[scenario.segment]]
kind = "nominal"
value = 20.0
```

**Binding fields**

| Field | Meaning |
|-------|---------|
| `path` | Redfish resource whose field is dynamic. |
| `pointer` | JSON pointer to the field (e.g. `/Reading`). |
| `source` | `external`, `scenario`, or `formula`. |
| `key` | (`external`) ingest key the value is written under. |
| `min` / `max` | (`external`) clamp bounds; a misbehaving twin can't emit impossible values. |
| `scenario` | (`scenario`) named scenario that drives the value. |
| `warning` / `critical` | Alert thresholds; crossing them emits a Redfish alert event. |
| `metric` | Also expose the value in a MetricReport under this MetricId. |

Crossing `warning`/`critical` emits `ThresholdCrossed` alerts on the SSE stream
and clearing emits `ThresholdCleared`, so a consumer gets a real push/stream
interface, not just polling.

## Twin control-plane

When a twin is loaded, simulate exposes a small non-Redfish control-plane for
feeding state and steering scenarios. This is what the
[behavioral harness](behavioral-conformance.md)'s `twin` driver presses.

| Endpoint | Effect |
|----------|--------|
| `POST /twin/v1/state` | Ingest external readings — a flat object, or the fleet array `[{ system_id?, key, value }]`. External readings resolve **live** on the next GET. |
| `POST /twin/v1/scenario/{name}` | **Arm** a scenario: re-base its timeline to now (`404` if unknown). |
| `GET /twin/v1/scenario` | **List** every scenario and its current state. |
| `DELETE /twin/v1/scenario` | **Reset**: disarm every scenario. |

```sh
# Feed a live reading, then read it straight back.
curl -X POST http://127.0.0.1:8000/twin/v1/state \
  -H 'Content-Type: application/json' \
  -d '[{ "key": "gpu.temp", "value": 42.0 }]'
curl -s http://127.0.0.1:8000/redfish/v1/Chassis/GPU_0/Sensors/Temp1 | jq .Reading  # 42.0

# Arm a scenario and watch it drive the read path.
curl -X POST http://127.0.0.1:8000/twin/v1/scenario/spike
```

## See also

- [Mockup Mode](mockup.md) — mockup directory format, creating one from a real BMC, the config-file backend.
- [Digital Twin Façade](twin-facade.md) — the twin design: bindings, freshness/TTL, stream-out, actuation.
- [Behavioral Conformance Harness](behavioral-conformance.md) — replay scenarios against a live simulate instance and verify behavior.
