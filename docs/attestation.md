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

### Advertised versions

A real SPDM responder does not have "a version": it answers GET_VERSION
with the list of versions it supports and runs subsequent messages at the
version the requester selects. The versions advertised are configured per
system (default: every version the responder implements):

```toml
[systems.vm1.spdm]
spdm_versions = ["1.1", "1.2", "1.3", "1.4"]  # default: all implemented versions
spdm_versions = ["1.1"]                       # single-version pin (e.g. the GB300
                                               # harness reproduces the CEC173x eRoT)
```

The MEASUREMENTS layout itself (3-byte uint24 MeasurementRecordLength,
OpaqueDataLength field) is identical in every SPDM version, as in libspdm
(DMTF's reference DSP0274 implementation); from SPDM 1.3 onwards requests
and responses additionally carry an 8-byte requester-context echo. Where
there is no live negotiation — the Redfish action path — responses are
emitted at the highest configured version.

### SPDM over TCP endpoint (DSP0287)

Systems can additionally expose the responder over a raw TCP socket, so that
DMTF's SPDM tooling — libspdm, `spdm_requester_emu` and the SPDM Responder
Validator (`spdm_device_validator_sample`) — can exercise it directly, the
closest stand-in for the device eRoT's MCTP endpoint:

```toml
[systems.vm1.spdm]
spdm_tcp_listen = "0.0.0.0:4194"   # disabled by default
```

The endpoint speaks the spdm-emu "platform" socket protocol (command,
transport type and length words framing each direction) carrying DSP0287
SPDM-over-TCP payloads, and serves the full OOB flow on one connection:
GET_VERSION (advertising `spdm_versions`), GET_CAPABILITIES,
NEGOTIATE_ALGORITHMS, GET_DIGESTS, GET_CERTIFICATE, CHALLENGE and
GET_MEASUREMENTS. Signed responses (CHALLENGE_AUTH, MEASUREMENTS) carry
ECDSA P-384 signatures over the SHA-384 transcript hashes (M1, L1/L2)
computed from the actual messages exchanged on the connection — including
the SPDM 1.2+ signing-context domain separation — so signature verification
passes against DMTF's reference requester. Session-based messages
(KEY_EXCHANGE and friends) are out of scope and answered with
`UnsupportedRequest`.

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
