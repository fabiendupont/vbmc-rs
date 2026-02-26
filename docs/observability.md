# Observability

This document covers the event system, audit logging, and Prometheus metrics in vbmc-rs.

## Event system

vbmc-rs emits Redfish events on state changes. Events are delivered through three channels:

1. **Audit log** — every event appended to a JSONL file
2. **Webhook subscriptions** — HTTP POST to registered endpoints
3. **Server-Sent Events (SSE)** — streaming endpoint for real-time consumers

### Event types

| Message ID | Trigger | Severity |
|-----------|---------|----------|
| `ResourceEvent.1.0.ResourcePowerStateChanged` | System power on, off, reset | OK |
| `ResourceEvent.1.0.ResourceChanged` | Virtual media insert/eject, boot override change | OK |
| `Security.1.0.SessionCreated` | User session created | OK |
| `Security.1.0.SessionTerminated` | User session ended | OK |
| `Security.1.0.AuthenticationFailure` | Failed login attempt | Warning |
| `Security.1.0.AccountLocked` | Account locked after repeated failures | Warning |
| `ComponentIntegrity.1.0.SPDMVerificationStatusChanged` | Attestation status change | OK or Warning |
| `Security.1.0.CertificateReplaced` | Certificate replacement | OK |

### Event payload

Each event contains:

```json
{
  "EventType": "StatusChange",
  "EventId": "550e8400-e29b-41d4-a716-446655440000",
  "EventTimestamp": "2026-02-26T12:00:00Z",
  "MessageId": "ResourceEvent.1.0.ResourcePowerStateChanged",
  "Message": "System vm1 powered on",
  "OriginOfCondition": "/redfish/v1/Systems/vm1",
  "Severity": "OK",
  "Actor": "admin",
  "Payload": null
}
```

| Field | Description |
|-------|-------------|
| EventType | Category: StatusChange, ResourceUpdated, ResourceAdded, ResourceRemoved, Alert |
| EventId | UUID v4, unique per event |
| EventTimestamp | RFC 3339 UTC timestamp |
| MessageId | Redfish-standard message identifier |
| Message | Human-readable description |
| OriginOfCondition | Redfish resource path that triggered the event |
| Severity | OK, Warning, or Critical |
| Actor | Username that triggered the event (if authenticated) |
| Payload | Optional additional context (JSON) |

## Audit log

Configure the audit log path:

```toml
audit_log = "/var/log/vbmc-rs/audit.jsonl"
```

Format: one JSON object per line (JSONL), with the fields listed above. Each line is flushed immediately after write.

The audit log grows indefinitely. Use external log rotation:

```
# /etc/logrotate.d/vbmc-rs
/var/log/vbmc-rs/audit.jsonl {
    daily
    rotate 30
    compress
    missingok
    notifempty
    copytruncate
}
```

`copytruncate` is required because vbmc-rs keeps the file open.

### Querying the audit log

```sh
# Last 10 events
tail -10 /var/log/vbmc-rs/audit.jsonl | jq .

# Power state changes only
grep 'ResourcePowerStateChanged' /var/log/vbmc-rs/audit.jsonl | jq .

# Events for a specific system
grep 'vm1' /var/log/vbmc-rs/audit.jsonl | jq .

# Authentication failures
grep 'AuthenticationFailure' /var/log/vbmc-rs/audit.jsonl | jq .
```

## Webhook subscriptions

### Create a subscription

```sh
curl -s -X POST http://localhost:8000/redfish/v1/EventService/Subscriptions \
  -H 'Content-Type: application/json' \
  -d '{
    "Destination": "https://my-webhook.example.com/events",
    "Protocol": "Redfish",
    "EventTypes": ["StatusChange"]
  }'
```

If `EventTypes` is omitted or empty, all events are delivered.

### Webhook delivery

Events are wrapped in a Redfish event envelope:

```json
{
  "@odata.type": "#Event.v1_9_0.Event",
  "Events": [
    {
      "EventType": "StatusChange",
      "EventId": "...",
      "EventTimestamp": "...",
      "MessageId": "...",
      "Message": "...",
      "OriginOfCondition": "...",
      "Severity": "..."
    }
  ]
}
```

Delivery uses exponential backoff: retries at 1s, 5s, and 30s after failure. After three failed attempts, the event is dropped for that subscription. A success is any HTTP 2xx response.

If a subscriber falls behind (events produced faster than consumed), missed events are logged as warnings.

### Manage subscriptions

```sh
# List subscriptions
curl -s http://localhost:8000/redfish/v1/EventService/Subscriptions | jq .

# Delete a subscription
curl -s -X DELETE http://localhost:8000/redfish/v1/EventService/Subscriptions/1
```

## Server-Sent Events (SSE)

For real-time event streaming:

```sh
curl -N http://localhost:8000/redfish/v1/EventService/SSE
```

Events are delivered as SSE `data:` lines containing the JSON event payload. The connection stays open until the client disconnects.

## Prometheus metrics

### Configuration

```toml
[metrics]
enabled = true
port = 9090
```

The metrics server listens on `0.0.0.0:{port}` and exposes a `/metrics` endpoint in Prometheus text format.

### Exposed metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `vbmc_rs_http_requests_total` | Counter | `method`, `path`, `status` | Total HTTP requests processed |
| `vbmc_rs_http_request_duration_seconds` | Histogram | `method`, `path` | Request processing time |
| `vbmc_rs_vm_power_state` | Counter | `system`, `state` | Power state transitions per system |
| `vbmc_rs_auth_attempts_total` | Counter | `result` | Authentication attempts (success/failure) |

### Scrape configuration

```yaml
# prometheus.yml
scrape_configs:
  - job_name: 'vbmc-rs'
    scrape_interval: 15s
    static_configs:
      - targets: ['localhost:9090']
```

### Example queries

```promql
# Request rate by endpoint
rate(vbmc_rs_http_requests_total[5m])

# P95 latency for power actions
histogram_quantile(0.95,
  rate(vbmc_rs_http_request_duration_seconds_bucket{path=~".*Reset.*"}[5m])
)

# Authentication failure rate
rate(vbmc_rs_auth_attempts_total{result="failure"}[5m])
```
