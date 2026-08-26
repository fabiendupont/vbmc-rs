# vbmc-rs

A Redfish-compliant virtual BMC (Baseboard Management Controller) written in Rust. It exposes a standard Redfish REST API to manage virtual machines, supporting multiple hypervisor backends.

One vbmc-rs instance manages multiple VMs using a blade chassis model — all systems in an instance use the same backend type.

## Backends

| Backend | Feature flag | Connection | Notes |
|---------|-------------|------------|-------|
| **Cloud-Hypervisor** | `cloud-hypervisor` (default) | HTTP over Unix socket | Full lifecycle: create, boot, shutdown, delete, hot-plug |
| **QEMU** | `qemu` | QMP over Unix socket | Manage-only: controls pre-existing QEMU processes |
| **Libvirt** | `libvirt` | `virt` crate (libvirt C API) | Native bindings via `virt` crate, parses domain XML with `quick-xml`. Requires `libvirt-dev`/`libvirt-devel` at build time |
| **KubeVirt** | `kubevirt` | Kubernetes API (`kube` crate) | Manage-only; uses KubeVirt subresource APIs (start/stop/restart/softreboot/addvolume/removevolume) |

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

# Aggregator binary
cargo build --release --features aggregator

# Webhook binary
cargo build --release --features webhook

# KubeVirt container images (sidecar, aggregator, webhook)
podman build -f Containerfile.kubevirt --target sidecar -t vbmc-rs-sidecar .
podman build -f Containerfile.kubevirt --target aggregator -t vbmc-rs-aggregator .
podman build -f Containerfile.kubevirt --target webhook -t vbmc-rs-webhook .
```

`--all-features` pulls in all backends plus the aggregator and webhook.

## Configuration

vbmc-rs uses a TOML configuration file. By default it looks for `/etc/vbmc-rs/config.toml`, overridden with `-c`:

```sh
vbmc-rs -c examples/config.toml
```

### Minimal Cloud-Hypervisor config

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

### Minimal QEMU config

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

### Minimal Libvirt config

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

### Minimal KubeVirt config

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

KubeVirt VMs must already exist in the cluster. The backend uses in-cluster or kubeconfig credentials automatically.

### Minimal aggregator config

The aggregator is a separate binary (`vbmc-rs-aggregator`) that discovers vbmc-rs sidecar instances and presents a unified Redfish Systems collection.

```toml
[server]
bind_address = "0.0.0.0"
port = 8443

[discovery]
mode = "static"

[[discovery.endpoints]]
system_id = "vm1"
url = "http://localhost:8001"

[[discovery.endpoints]]
system_id = "vm2"
url = "http://localhost:8002"
```

Discovery modes: `static` (explicit endpoint list) or `kubernetes` (watches for vbmc-rs sidecar pods). mTLS between aggregator and sidecars is configured via the server TLS fields.

See `examples/` for complete annotated configuration files for each backend.

### Configuration reference

| Section | Field | Default | Description |
|---------|-------|---------|-------------|
| (top) | `backend` | `cloud_hypervisor` | Backend type: `cloud_hypervisor`, `qemu`, `libvirt`, `kube_virt` |
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
| `[security_policy]` | `tls_minimum_version` | — | Minimum TLS version (`tls12` or `tls13`); constrains rustls protocol |
| `[security_policy]` | `spdm_enabled` | `false` | Enable SPDM attestation coordinator |

## Usage

```sh
# Start with default config
vbmc-rs

# Start with custom config
vbmc-rs -c /path/to/config.toml

# Enable debug logging
RUST_LOG=vbmc_rs=debug vbmc-rs -c config.toml
```

### Quick smoke test

```sh
# Service root
curl -s http://localhost:8000/redfish/v1 | jq .

# List systems
curl -s http://localhost:8000/redfish/v1/Systems | jq .

# Get system details
curl -s http://localhost:8000/redfish/v1/Systems/vm1 | jq .

# Power on
curl -s -X POST http://localhost:8000/redfish/v1/Systems/vm1/Actions/ComputerSystem.Reset \
  -H 'Content-Type: application/json' \
  -d '{"ResetType": "On"}'

# Graceful shutdown
curl -s -X POST http://localhost:8000/redfish/v1/Systems/vm1/Actions/ComputerSystem.Reset \
  -H 'Content-Type: application/json' \
  -d '{"ResetType": "GracefulShutdown"}'
```

## Redfish resources

vbmc-rs implements the following Redfish resources:

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
| GenerateCSR | `POST /redfish/v1/CertificateService/Actions/CertificateService.GenerateCSR` |
| ReplaceCertificate | `POST /redfish/v1/CertificateService/Actions/CertificateService.ReplaceCertificate` |
| Telemetry Service | `GET /redfish/v1/TelemetryService` |
| Component Integrity | `GET /redfish/v1/ComponentIntegrity` |
| Security Policy | `GET/PATCH /redfish/v1/SecurityPolicy` |

## Documentation

| Document | Description |
|----------|-------------|
| [ARCHITECTURE.md](ARCHITECTURE.md) | Internal structure and design decisions for contributors |
| [docs/conformance.md](docs/conformance.md) | Redfish conformance profile: which properties are live, persisted, or static; backend capability matrix |
| [docs/kubevirt.md](docs/kubevirt.md) | KubeVirt deployment: Helm chart, webhook injection, OAuth, UDN, mTLS, attestation |
| [docs/deployment.md](docs/deployment.md) | Auth setup, TLS, state directory, systemd, containers, logging |
| [docs/observability.md](docs/observability.md) | Events, audit log, webhooks, SSE, Prometheus metrics |
| [docs/attestation.md](docs/attestation.md) | Keylime, Trustee, and swtpm integration for remote attestation |

## Testing

```sh
# Run all tests
cargo test --all-features

# Run only unit tests
cargo test --all-features --lib

# Run a specific test module
cargo test --all-features integration_tests
cargo test --all-features backend::libvirt::xml
```

## License

Apache-2.0
