# vbmc-rs

**Turn any virtual machine into a standards-compliant bare-metal server.**

vbmc-rs is a Redfish virtual BMC (Baseboard Management Controller) written in Rust. It exposes the same REST API that physical servers use — so management tools like [Ironic](https://ironicbaremetal.org/), [MAAS](https://maas.io/), and [Tinkerbell](https://tinkerbell.org/) can provision VMs exactly like real hardware.

One instance manages multiple VMs using a blade chassis model. Four hypervisor backends. 30+ Redfish resources. DMTF-validated conformance. Single static binary.

## Quick start

```sh
# Build
cargo build --release

# Generate a starter config
./target/release/vbmc-rs init --backend cloud_hypervisor

# Start the server
./target/release/vbmc-rs -c config.toml

# Power on a VM via Redfish
curl -s http://localhost:8000/redfish/v1/Systems/vm1 | jq .PowerState
curl -X POST http://localhost:8000/redfish/v1/Systems/vm1/Actions/ComputerSystem.Reset \
  -H 'Content-Type: application/json' \
  -d '{"ResetType": "On"}'
```

No hypervisor? **Simulate a fleet of BMCs** with a single command — no config file, no VMs:

```sh
./target/release/vbmc-rs simulate --systems 50
# Simulating 50 server(s) at http://127.0.0.1:8000
# Try: curl -s http://127.0.0.1:8000/redfish/v1/Systems | jq .
```

Or serve a [DMTF mockup directory](docs/mockup.md) as a live, stateful Redfish service:

```sh
vbmc-rs -c examples/config-mockup.toml
```

## Use cases

**Test bare-metal provisioning without bare metal.** Point Ironic, MAAS, or Tinkerbell at vbmc-rs instead of a real BMC. Enroll, inspect, and provision VMs through the same Redfish workflows you use in production.

**Simulate BMC fleets at scale.** Spin up hundreds of Redfish endpoints from DMTF mockup directories for fleet management development, monitoring dashboards, and load testing.

**Redfish client development and CI.** Replace the Python `redfishMockupServer.py` with a single binary that supports state mutations, TLS/mTLS, authentication, and OData compliance out of the box.

**Kubernetes-native virtual BMC.** Deploy as a sidecar alongside KubeVirt VMs, with an aggregator that presents a unified Redfish Systems collection, OAuth via TokenReview, and mTLS between components. Ships with a Helm chart.

## Why vbmc-rs?

| | **vbmc-rs** | **VirtualBMC** | **sushy-tools** |
|---|---|---|---|
| Protocol | Redfish | IPMI only | Redfish |
| Language | Rust | Python | Python |
| Backends | Cloud-Hypervisor, QEMU, libvirt, KubeVirt | libvirt | libvirt, OpenStack |
| Conformance | DMTF-validated, OData-compliant | N/A (IPMI) | Partial |
| Mockup mode | Yes — stateful, serves DMTF mockup dirs | No | No |
| Auth | Session-based, RBAC, account lockout | No | Basic |
| TLS | TLS + mTLS (rustls) | No | Optional |
| Events | SSE, webhooks, audit log | No | No |
| Attestation | Keylime, Trustee, swtpm | No | No |
| Kubernetes | Sidecar + aggregator + webhook + Helm | No | No |
| Packaging | Static binary, OCI images | pip | pip |

## Backends

| Backend | Feature flag | Connection | Notes |
|---------|-------------|------------|-------|
| **Cloud-Hypervisor** | `cloud-hypervisor` (default) | HTTP over Unix socket | Full lifecycle: create, boot, shutdown, delete, hot-plug |
| **QEMU** | `qemu` | QMP over Unix socket | Manage-only: controls pre-existing QEMU processes |
| **Libvirt** | `libvirt` | `virt` crate (libvirt C API) | Native bindings, parses domain XML. Requires `libvirt-dev` at build time |
| **KubeVirt** | `kubevirt` | Kubernetes API (`kube` crate) | Manage-only; uses KubeVirt subresource APIs |
| **Mockup** | (always available) | Filesystem | Serves DMTF mockup directories with stateful mutations |

## Features

- **30+ Redfish resources** — Systems, Chassis, Managers, Storage, BIOS, SecureBoot, VirtualMedia, Processors, Memory, EthernetInterfaces, PCIe, Sensors, and more
- **DMTF-validated** — passes the official Redfish Service Validator on every CI run
- **OData compliance** — `$metadata`, ETags, `@odata.context`, proper Link headers
- **Session auth + RBAC** — Administrator, Operator, ReadOnly roles with argon2 password hashing and account lockout
- **TLS and mTLS** — rustls-based, with hot-reload via CertificateService
- **Events** — Server-Sent Events, webhook subscriptions with exponential backoff, JSONL audit log
- **Attestation** — Keylime, Trustee, and swtpm integration for remote attestation via ComponentIntegrity
- **Prometheus metrics** — request latency, power state, backend health
- **Task service** — async operation tracking for long-running actions
- **Kubernetes-native** — sidecar injection webhook, aggregator with OAuth (TokenReview + SubjectAccessReview), Helm chart

## Building

```sh
# Default (Cloud-Hypervisor backend only)
cargo build --release

# With all backends
cargo build --release --all-features

# Specific backend
cargo build --release --features qemu
cargo build --release --features libvirt
cargo build --release --features kubevirt

# Container images
podman build -f Containerfile.kubevirt --target sidecar -t vbmc-rs-sidecar .
podman build -f Containerfile.kubevirt --target aggregator -t vbmc-rs-aggregator .
podman build -f Containerfile.kubevirt --target webhook -t vbmc-rs-webhook .
```

`--all-features` pulls in all backends plus the aggregator and webhook. The libvirt backend requires `libvirt-dev` (Debian/Ubuntu) or `libvirt-devel` (Fedora).

## Configuration

vbmc-rs uses a TOML configuration file. See `examples/` for complete annotated configs for each backend.

```sh
vbmc-rs -c examples/config.toml
```

<details>
<summary><strong>Minimal Cloud-Hypervisor config</strong></summary>

```toml
backend = "cloud_hypervisor"

[server]
bind_address = "127.0.0.1"
port = 8000

[systems.vm1]
name = "My VM"
socket_path = "/tmp/cloud-hypervisor-vm1.sock"

[systems.vm1.hardware]
cpu_count = 2
memory_mib = 1024

[[systems.vm1.hardware.disks]]
path = "/var/lib/images/vm1.qcow2"
id = "rootdisk"
```
</details>

<details>
<summary><strong>Minimal QEMU config</strong></summary>

```toml
backend = "qemu"

[server]
bind_address = "127.0.0.1"
port = 8000

[systems.vm1]
name = "My QEMU VM"
socket_path = "/tmp/qmp-vm1.sock"
```

Start QEMU separately with a QMP socket:
```sh
qemu-system-x86_64 -qmp unix:/tmp/qmp-vm1.sock,server,nowait -m 2048 -smp 2 ...
```
</details>

<details>
<summary><strong>Minimal Libvirt config</strong></summary>

```toml
backend = "libvirt"

[server]
bind_address = "127.0.0.1"
port = 8000

[systems.vm1]
name = "My Libvirt VM"
connection_uri = "qemu:///system"
domain_name = "my-domain"
```
</details>

<details>
<summary><strong>Minimal KubeVirt config</strong></summary>

```toml
backend = "kube_virt"

[server]
bind_address = "0.0.0.0"
port = 8000

[systems.vm1]
name = "KubeVirt VM 1"
namespace = "default"
vm_name = "my-test-vm"

[systems.vm1.hardware]
cpu_count = 2
memory_mib = 2048
```

KubeVirt VMs must already exist in the cluster.
</details>

<details>
<summary><strong>Minimal Mockup config</strong></summary>

```toml
backend = "mockup"
mockup_directory = "/path/to/mockup"

[server]
bind_address = "0.0.0.0"
port = 8000
```

See [Mockup Mode](docs/mockup.md) for the directory format and state mutation support.
</details>

<details>
<summary><strong>Full configuration reference</strong></summary>

| Section | Field | Default | Description |
|---------|-------|---------|-------------|
| (top) | `backend` | `cloud_hypervisor` | Backend type: `cloud_hypervisor`, `qemu`, `libvirt`, `kube_virt`, `mockup` |
| `[server]` | `bind_address` | `0.0.0.0` | Listen address |
| `[server]` | `port` | `8000` | Listen port |
| `[server]` | `tls_cert` | — | TLS certificate path |
| `[server]` | `tls_key` | — | TLS private key path |
| `[server]` | `tls_client_ca` | — | Client CA certificate path (enables mTLS) |
| `[auth]` | `enabled` | `false` | Enable session-based authentication |
| `[auth]` | `session_timeout_seconds` | `3600` | Session lifetime |
| `[auth]` | `max_sessions` | `64` | Maximum concurrent sessions |
| `[auth]` | `lockout_threshold` | `5` | Failed logins before lockout |
| `[auth]` | `lockout_duration_seconds` | `300` | Auto-unlock duration after lockout |
| `[auth]` | `accounts_file` | — | Path to accounts JSON file |
| `[defaults]` | `firmware_path` | `/usr/share/OVMF/OVMF_CODE.fd` | Default UEFI firmware |
| `[defaults]` | `boot_source` | `Hdd` | Default boot device |
| (top) | `state_directory` | (empty) | Directory for persistent VM state |
| (top) | `audit_log` | (empty) | Audit log path (JSONL) |
| (top) | `audit_log_target` | `file` | Audit log destination: `file`, `stdout`, or `both` |
| `[metrics]` | `enabled` | `true` | Enable Prometheus metrics endpoint |
| `[metrics]` | `port` | `9090` | Metrics server port |
| `[systems.<id>]` | `name` | — | Display name |
| `[systems.<id>]` | `socket_path` | — | Unix socket (CH/QEMU) |
| `[systems.<id>]` | `connection_uri` | — | Libvirt connection URI |
| `[systems.<id>]` | `domain_name` | — | Libvirt domain name |
| `[systems.<id>]` | `namespace` | — | Kubernetes namespace (KubeVirt) |
| `[systems.<id>]` | `vm_name` | — | KubeVirt VirtualMachine name |
| `[systems.<id>]` | `firmware_path` | — | Per-system firmware override |
| `[systems.<id>]` | `secure_boot_firmware_path` | — | UEFI firmware for Secure Boot |
| `[systems.<id>.hardware]` | `cpu_count` | `2` | vCPU count |
| `[systems.<id>.hardware]` | `max_cpu_count` | — | Max vCPU (for hotplug) |
| `[systems.<id>.hardware]` | `memory_mib` | `1024` | Memory in MiB |
| `[[systems.<id>.hardware.disks]]` | `path` | — | Disk image path |
| `[[systems.<id>.hardware.disks]]` | `id` | — | Disk identifier |
| `[[systems.<id>.hardware.disks]]` | `readonly` | `false` | Read-only flag |
| `[systems.<id>.attestation]` | `provider` | — | Attestation provider: `swtpm`, `keylime`, `trustee` |
| `[systems.<id>.attestation]` | `swtpm_socket` | `/var/run/swtpm/swtpm-sock` | swtpm Unix socket path |
| `[systems.<id>.attestation]` | `provider_url` | — | Keylime/Trustee verifier URL |
| `[systems.<id>.attestation]` | `poll_interval_seconds` | `30` | Attestation polling interval |
| `[systems.<id>.attestation]` | `pcr_policy` | — | Expected PCR values (map of index to base64) |
| `[location]` | `facility` | — | Data center / facility name |
| `[location]` | `room` | — | Room within facility |
| `[location]` | `row` | — | Row identifier |
| `[location]` | `rack` | — | Rack identifier |
| `[location]` | `rack_offset` | — | Rack offset (U position) |
| `[location]` | `city` | — | Physical city |
| `[location]` | `state_or_province` | — | State or province |
| `[location]` | `country` | — | Country |
| `[location]` | `postal_code` | — | Postal / ZIP code |
| `[location]` | `latitude` | — | GPS latitude |
| `[location]` | `longitude` | — | GPS longitude |
| `[location]` | `altitude_meters` | — | Altitude in meters |
| `[security_policy]` | `tls_minimum_version` | — | Minimum TLS version (`tls12` or `tls13`) |
| `[security_policy]` | `spdm_enabled` | `false` | Enable SPDM attestation coordinator |

</details>

## Redfish resources

| Resource | Endpoint |
|----------|----------|
| Service Root | `GET /redfish/v1` |
| OData Metadata | `GET /redfish/v1/$metadata` |
| Systems | `GET /redfish/v1/Systems/{id}` |
| Power Actions | `POST /redfish/v1/Systems/{id}/Actions/ComputerSystem.Reset` |
| Processors | `GET /redfish/v1/Systems/{id}/Processors/{cpu}` |
| Memory | `GET /redfish/v1/Systems/{id}/Memory/{dimm}` |
| Ethernet Interfaces | `GET /redfish/v1/Systems/{id}/EthernetInterfaces/{nic}` |
| Storage | `GET /redfish/v1/Systems/{id}/Storage/{ctrl}` |
| Drives | `GET /redfish/v1/Systems/{id}/Storage/{ctrl}/Drives/{drive}` |
| Volumes | `GET /redfish/v1/Systems/{id}/Storage/{ctrl}/Volumes/{vol}` |
| SimpleStorage | `GET /redfish/v1/Systems/{id}/SimpleStorage/1` |
| PCIe Devices | `GET /redfish/v1/Systems/{id}/PCIeDevices/{dev}` |
| PCIe Functions | `GET /redfish/v1/Systems/{id}/PCIeDevices/{dev}/PCIeFunctions/{fn}` |
| BIOS | `GET /redfish/v1/Systems/{id}/Bios` |
| BIOS Settings | `GET/PATCH /redfish/v1/Systems/{id}/Bios/Settings` |
| Secure Boot | `GET/PATCH /redfish/v1/Systems/{id}/SecureBoot` |
| Virtual Media | `GET/POST /redfish/v1/Systems/{id}/VirtualMedia/Cd` |
| Log Services | `GET /redfish/v1/Systems/{id}/LogServices` |
| Managers | `GET /redfish/v1/Managers/vbmc` |
| Chassis | `GET /redfish/v1/Chassis/1` |
| Power | `GET /redfish/v1/Chassis/1/Power` |
| Thermal | `GET /redfish/v1/Chassis/1/Thermal` |
| Network Adapters | `GET /redfish/v1/Chassis/1/NetworkAdapters` |
| Session Service | `GET/POST /redfish/v1/SessionService/Sessions` |
| Account Service | `GET/POST /redfish/v1/AccountService/Accounts` |
| Event Service | `GET/POST /redfish/v1/EventService/Subscriptions` |
| SSE | `GET /redfish/v1/EventService/SSE` |
| Task Service | `GET /redfish/v1/TaskService/Tasks` |
| Update Service | `GET /redfish/v1/UpdateService` |
| License Service | `GET/POST /redfish/v1/LicenseService/Licenses` |
| Certificate Service | `GET /redfish/v1/CertificateService` |
| Telemetry Service | `GET /redfish/v1/TelemetryService` |
| Component Integrity | `GET /redfish/v1/ComponentIntegrity` |
| Security Policy | `GET/PATCH /redfish/v1/SecurityPolicy` |

## Documentation

| Document | Description |
|----------|-------------|
| [Architecture](ARCHITECTURE.md) | Internal structure and design decisions |
| [Testing Ironic](docs/ironic.md) | Using vbmc-rs with Ironic and Metal3 for bare-metal provisioning |
| [Conformance](docs/conformance.md) | Redfish conformance profile and backend capability matrix |
| [KubeVirt deployment](docs/kubevirt.md) | Helm chart, webhook injection, OAuth, UDN, mTLS |
| [Deployment](docs/deployment.md) | Auth, TLS, state directory, systemd, containers, logging |
| [Observability](docs/observability.md) | Events, audit log, webhooks, SSE, Prometheus metrics |
| [Attestation](docs/attestation.md) | Keylime, Trustee, and swtpm integration |
| [Mockup mode](docs/mockup.md) | Serving DMTF mockup directories as live Redfish services |
| [Why vbmc-rs](docs/why-vbmc-rs.md) | Comparison with VirtualBMC, sushy-tools, and redfishMockupServer |

## Testing

```sh
cargo test --all-features              # All tests
cargo test --all-features --lib        # Unit tests only
cargo test --all-features integration  # Integration tests
```

## License

Apache-2.0
