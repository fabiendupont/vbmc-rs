# Remote Attestation

vbmc-rs can integrate with remote attestation services to verify the integrity of managed VMs. Attestation results are surfaced through the Redfish `ComponentIntegrity` resource.

## Overview

The attestation coordinator is a background task that periodically polls an attestation service for each configured system. When a system's verification status changes, vbmc-rs updates the persisted VM state and emits a `ComponentIntegrity.1.0.SPDMVerificationStatusChanged` event.

This is not the vbmc-rs instance attesting itself — it queries external attestation services about the VMs it manages.

## Supported providers

### Keylime

[Keylime](https://keylime.dev/) is a TPM-based remote attestation framework.

```toml
[systems.vm1.attestation]
enabled = true
poll_interval_seconds = 30
attestation_service = "keylime"
provider_url = "https://keylime-verifier:8881"
agent_id = "vm1"  # optional, defaults to system_id
```

vbmc-rs queries the Keylime verifier API at `{provider_url}/v2/agents/{agent_id}`.

**State mapping:**

| Keylime operational state | VerificationStatus |
|--------------------------|-------------------|
| 7 | Success |
| 0 | Unknown |
| Other | Failed |

**Measurements extracted:**

| PCR range | MeasurementType |
|-----------|-----------------|
| PCR 0-7 | ImmutableROM |
| PCR 8-13 | MutableFirmware |
| PCR 14 | FirmwareConfiguration |
| IMA entries | MutableFirmware |

PCR quotes are organized by hash bank (e.g., sha256). IMA (Integrity Measurement Architecture) entries are capped at 256 per poll. The coordinator tracks which PCRs are part of the measurement summary based on the TPM policy.

### Trustee

[Trustee](https://github.com/confidential-containers/trustee) (part of Confidential Containers) provides attestation for confidential VMs.

```toml
[systems.vm1.attestation]
enabled = true
poll_interval_seconds = 30
attestation_service = "trustee"
provider_url = "https://trustee-service:8080"
```

vbmc-rs sends a `POST` to `{provider_url}/kbs/v0/attest` with an empty JSON body. The response is a JWT whose claims are parsed for measurements.

**Verification:** HTTP 2xx = Success, anything else = Failed.

**Measurements extracted from JWT claims:**

| JWT claim | MeasurementType |
|-----------|-----------------|
| `tcb_status` | ImmutableROM |
| `launch_measurement` | ImmutableROM |
| `fw_config` | FirmwareConfiguration |
| `configuration` | HardwareConfiguration |
| `platform_config` | HardwareConfiguration |
| `guest_config` | HardwareConfiguration |

## Polling behavior

The attestation coordinator runs as a tokio background task. On each interval:

1. Iterates over all systems with attestation enabled
2. Calls the configured provider
3. Compares the new verification status with the previous one
4. If changed: updates `VmState.attestation`, saves state to disk, emits event

Events use severity `OK` for Success transitions and `Warning` for Failed or Unknown.

## SPDM responder (signed measurements for OOB attestation)

In addition to *consuming* attestation results, vbmc-rs can *produce* SPDM
evidence: systems configured with an `[systems.<id>.spdm]` section advertise the
`#ComponentIntegrity.SPDMGetSignedMeasurements` action, which out-of-band
attestation services (such as [Keylime's OOB
collector](https://github.com/keylime/keylime)) invoke to obtain fresh signed
measurements.

```toml
[systems.vm1.spdm]
private_key = "/path/to/device-key.pem"   # EC P-384 (PKCS#8 or SEC1 PEM)
cert_chain  = "/path/to/device-chain.blob" # DMTF DSP0274 chain blob (leaf first)
measurements = [                            # optional hex values; deterministic defaults otherwise
  "aabb...",
]
```

Each action request supplies a 32-byte hex `Nonce`, a `SlotId` and
`MeasurementIndices`. Gathering the measurements means traversing MCTP to the
device's root of trust, which is genuinely slow on real hardware — so, matching
SPDM-capable BMCs such as the NVIDIA GB300 NVL72 trays, the action is
**asynchronous**: it returns HTTP 202 with a Task resource. Clients poll the
task until `TaskState` is `Completed`, then retrieve the payload from
`GET {action target}/data`. The simulated collection latency is configurable
per system:

```toml
[systems.vm1.spdm]
collection_delay_ms = 200   # default; set 0 for instant completion
```

The payload contains the schema-conformant members (`Version`,
`HashingAlgorithm`, `SigningAlgorithm`, `SignedMeasurements`, a `Certificate`
link and a `PublicKey` PEM) plus a non-standard `CertificateChain` property
carrying the Base64 DMTF chain blob inline for clients that want everything in
one round-trip.

The device's SPDM certificate chain is also published as a Redfish Certificate
resource, the way real BMCs do it:

- `GET /redfish/v1/Chassis/{chassis_id}/Certificates` — collection
- `GET /redfish/v1/Chassis/{chassis_id}/Certificates/{system_id}` — the chain
  as concatenated PEM (`CertificateString`, `CertificateType: PEMchain`),
  linked from the ComponentIntegrity resource
  (`SPDM.IdentityAuthentication.ResponderAuthentication.ComponentCertificate`)
  and from the action response's `Certificate` member

Because the caller's nonce is inside the signed data, a response cannot be
replayed against a later attestation challenge.

Note: measurement values are taken from configuration (or derived
deterministically) — they represent the "firmware" the virtual device claims to
be running. For fidelity, point `measurements` at digests of the VM's actual
firmware images.

## Redfish resources

Attestation data is exposed through:

- `GET /redfish/v1/ComponentIntegrity` — collection of integrity records
- `GET /redfish/v1/ComponentIntegrity/{id}` — individual record with measurements

Each record includes:

- `ComponentIntegrityType` — attestation type
- `TPM` or `SPDM` verification evidence
- Individual measurements with index, type, hash algorithm, and value
- Measurement summary hash and algorithm
- Responder verification status (Success, Failed, Unknown)

## Configuration reference

| Field | Default | Description |
|-------|---------|-------------|
| `enabled` | `false` | Enable attestation for this system |
| `poll_interval_seconds` | `30` | How often to poll the attestation service |
| `attestation_service` | — | Provider type: `keylime` or `trustee` |
| `provider_url` | — | Base URL of the attestation service |
| `agent_id` | system ID | Override the agent/client identifier sent to the provider |
