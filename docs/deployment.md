# Deployment Guide

This document covers deploying and operating vbmc-rs beyond the quick-start instructions in the README.

## Build requirements

### Default (Cloud-Hypervisor only)

```sh
cargo build --release
```

No system dependencies beyond the Rust toolchain.

### With libvirt backend

```sh
# Fedora/RHEL
sudo dnf install libvirt-devel

# Debian/Ubuntu
sudo apt install libvirt-dev

cargo build --release --features libvirt
```

The `virt` crate links against the libvirt C library at build time.

### All backends

```sh
cargo build --release --all-features
```

Requires `libvirt-dev`/`libvirt-devel` installed.

## Authentication setup

When `auth.enabled = false` (the default), all endpoints are open. To enable authentication:

### 1. Create an accounts file

```json
[
  {
    "username": "admin",
    "password_hash": "<argon2 hash>",
    "role": "Administrator"
  },
  {
    "username": "operator",
    "password_hash": "<argon2 hash>",
    "role": "Operator"
  }
]
```

Generate a password hash using any argon2 tool, or create accounts at runtime via the Redfish API once the first admin exists.

### 2. Configure authentication

```toml
[auth]
enabled = true
accounts_file = "/etc/vbmc-rs/accounts.json"
session_timeout_seconds = 3600
max_sessions = 64
lockout_threshold = 5
lockout_duration_seconds = 300
```

| Field | Default | Description |
|-------|---------|-------------|
| `enabled` | `false` | Master switch for authentication |
| `accounts_file` | — | Path to accounts JSON. Created at runtime if it doesn't exist and an account is added via API |
| `session_timeout_seconds` | `3600` | Sessions expire after this many seconds of inactivity |
| `max_sessions` | `64` | Maximum concurrent sessions |
| `lockout_threshold` | `5` | Lock account after this many consecutive failed logins |
| `lockout_duration_seconds` | `300` | Locked accounts unlock after this duration |

### 3. Authenticate

```sh
# Create a session
curl -s -X POST http://localhost:8000/redfish/v1/SessionService/Sessions \
  -H 'Content-Type: application/json' \
  -d '{"UserName": "admin", "Password": "secret"}' \
  -D - 2>/dev/null | grep X-Auth-Token

# Use the token
curl -s http://localhost:8000/redfish/v1/Systems \
  -H 'X-Auth-Token: <token>'

# Or use HTTP Basic
curl -s -u admin:secret http://localhost:8000/redfish/v1/Systems
```

### Roles and privileges

| Role | Read | Power actions | Config changes | Account mgmt |
|------|------|---------------|----------------|---------------|
| Administrator | Yes | Yes | Yes | Yes |
| Operator | Yes | Yes | Yes | No |
| ReadOnly | Yes | No | No | No |

### RBAC enforcement

When `auth.enabled = true`, every endpoint requires an `AuthenticatedUser`. The required privilege depends on the endpoint category:

| Endpoint category | Required privilege | Roles |
|---|---|---|
| GET on any resource | `Login` | All |
| POST ComputerSystem.Reset | `ConfigureComponents` | Administrator, Operator |
| PATCH ComputerSystem, Bios, SecureBoot | `ConfigureComponents` | Administrator, Operator |
| VirtualMedia InsertMedia/EjectMedia | `ConfigureComponents` | Administrator, Operator |
| POST/PATCH/DELETE AccountService accounts | `ConfigureUsers` | Administrator |
| PATCH SecurityPolicy | `ConfigureManager` | Administrator |
| CertificateService GenerateCSR/ReplaceCertificate | `ConfigureManager` | Administrator |
| PATCH own account password | `ConfigureSelf` | All |
| DELETE own session | `ConfigureSelf` | All |

Privilege mapping follows the `Privilege` enum: `Login`, `ConfigureManager`, `ConfigureUsers`, `ConfigureComponents`, `ConfigureSelf`.

### Account lockout

When `lockout_threshold` consecutive authentication failures occur for an account, the account is locked for `lockout_duration_seconds`. During lockout:

- All login attempts for that account are rejected with HTTP 401
- An `AccountLocked` event is emitted (severity: Warning)
- The account auto-unlocks after the configured duration
- An administrator can unlock accounts via PATCH to the account resource

Monitor lockouts in the audit log:

```sh
grep 'AccountLocked' /var/log/vbmc-rs/audit.jsonl | jq .
```

## TLS configuration

vbmc-rs uses rustls with axum-server. Certificates are hot-reloaded on replacement via the CertificateService API.

### Generating self-signed certificates

```sh
# Generate CA key and certificate
openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 \
  -keyout ca.key -out ca.crt -days 3650 -nodes \
  -subj "/CN=vbmc-rs CA"

# Generate server key and CSR
openssl req -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 \
  -keyout server.key -out server.csr -nodes \
  -subj "/CN=vbmc-rs"

# Sign server certificate with CA
openssl x509 -req -in server.csr -CA ca.crt -CAkey ca.key \
  -CAcreateserial -out server.crt -days 365

# Install
install -m 0600 server.key /etc/vbmc-rs/server.key
install -m 0644 server.crt /etc/vbmc-rs/server.crt
install -m 0644 ca.crt /etc/vbmc-rs/ca.crt
```

### Server TLS

```toml
[server]
tls_cert = "/etc/vbmc-rs/server.crt"
tls_key = "/etc/vbmc-rs/server.key"
```

### Mutual TLS (mTLS)

Requires clients to present a certificate signed by the specified CA:

```toml
[server]
tls_cert = "/etc/vbmc-rs/server.crt"
tls_key = "/etc/vbmc-rs/server.key"
tls_client_ca = "/etc/vbmc-rs/ca.crt"
```

Generate a client certificate:

```sh
openssl req -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 \
  -keyout client.key -out client.csr -nodes \
  -subj "/CN=redfish-client"

openssl x509 -req -in client.csr -CA ca.crt -CAkey ca.key \
  -CAcreateserial -out client.crt -days 365
```

Test with curl:

```sh
curl -s --cert client.crt --key client.key --cacert ca.crt \
  https://localhost:8000/redfish/v1/Systems
```

### TLS minimum version

Enforced via the SecurityPolicy resource:

```toml
[security_policy]
tls_minimum_version = "1.3"
```

Accepted values: `"1.2"` (default when TLS is enabled) and `"1.3"`. The minimum version is enforced in the rustls protocol version configuration.

### Certificate rotation via Redfish

Use the CertificateService API to rotate certificates without restart. Requires `ConfigureManager` privilege.

```sh
# Generate a CSR
curl -s -X POST https://localhost:8000/redfish/v1/CertificateService/Actions/CertificateService.GenerateCSR \
  -H 'X-Auth-Token: <token>' \
  -H 'Content-Type: application/json' \
  -d '{
    "CommonName": "vbmc-rs",
    "Organization": "Example Corp",
    "AlternativeNames": ["vbmc-rs.example.com"]
  }'

# Sign the CSR externally, then replace the certificate
curl -s -X POST https://localhost:8000/redfish/v1/CertificateService/Actions/CertificateService.ReplaceCertificate \
  -H 'X-Auth-Token: <token>' \
  -H 'Content-Type: application/json' \
  -d '{
    "CertificateString": "<PEM-encoded certificate>",
    "CertificateType": "PEM"
  }'
```

The server hot-reloads the certificate immediately. A `CertificateReplaced` event is emitted.

## State directory

vbmc-rs persists mutable VM state (boot override, virtual media, secure boot, BIOS settings, licenses) as JSON files:

```toml
state_directory = "/var/lib/vbmc-rs"
```

Layout:

```
/var/lib/vbmc-rs/
├── vm1.json          # VmState for system "vm1"
├── vm2.json          # VmState for system "vm2"
└── media/
    └── vm1/
        └── image.iso  # Downloaded virtual media
```

Each state file is written atomically (write to temp file, then rename). If `state_directory` is not configured, state is not persisted across restarts.

Virtual media images are stored in the per-system `virtual_media_directory` configured in the system section:

```toml
[systems.vm1]
virtual_media_directory = "/var/lib/vbmc-rs/media/vm1"
```

## KubeVirt deployment

Build with the `kubevirt` feature flag:

```sh
cargo build --release --features kubevirt
```

The KubeVirt backend talks to the Kubernetes API. It is manage-only — VMs are created externally via `kubectl`, Helm, or GitOps. vbmc-rs maps Redfish operations to KubeVirt subresource calls (start, stop, restart, softreboot, addvolume, removevolume).

### Sidecar deployment

Deploy vbmc-rs as a sidecar container in the virt-launcher pod. Each sidecar manages a single VM and communicates via the Kubernetes API from within the pod.

See [kubevirt.md](kubevirt.md) for pod spec examples and configuration details.

### API backend deployment

A single vbmc-rs instance can manage multiple KubeVirt VMs by mapping each system ID to a namespace/vm_name pair:

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

The instance uses the default kubeconfig (`KUBECONFIG` env var, `~/.kube/config`, or in-cluster service account).

## Aggregator deployment

The `vbmc-rs-aggregator` binary discovers vbmc-rs sidecar instances and presents a unified Redfish interface. It proxies requests to the appropriate sidecar based on system ID.

### Configuration

```toml
[server]
bind_address = "0.0.0.0"
port = 8443
tls_cert = "/etc/vbmc-rs/server.crt"
tls_key = "/etc/vbmc-rs/server.key"

[auth]
enabled = true
accounts_file = "/etc/vbmc-rs/accounts.json"

[discovery]
mode = "kubernetes"                   # or "static"
namespace = "kubevirt-vms"
label_selector = "app.kubernetes.io/name=vbmc-rs-sidecar"

[sidecar]
port = 8000
tls_ca = "/etc/vbmc-rs/sidecar-ca.crt"
tls_cert = "/etc/vbmc-rs/aggregator-client.crt"
tls_key = "/etc/vbmc-rs/aggregator-client.key"
```

### Discovery modes

| Mode | Description |
|------|-------------|
| `static` | Fixed list of `[[discovery.endpoints]]` with `system_id` and `url` |
| `kubernetes` | Watches pods matching `label_selector`; extracts system ID from `vbmc-rs/system-id` label (falls back to pod name); builds URL from pod IP and `sidecar.port` |

### Kubernetes RBAC

The aggregator service account needs pod list/watch permissions:

```yaml
apiVersion: v1
kind: ServiceAccount
metadata:
  name: vbmc-rs-aggregator
  namespace: kubevirt-vms
---
apiVersion: rbac.authorization.k8s.io/v1
kind: Role
metadata:
  name: vbmc-rs-aggregator
  namespace: kubevirt-vms
rules:
  - apiGroups: [""]
    resources: ["pods"]
    verbs: ["get", "list", "watch"]
---
apiVersion: rbac.authorization.k8s.io/v1
kind: RoleBinding
metadata:
  name: vbmc-rs-aggregator
  namespace: kubevirt-vms
subjects:
  - kind: ServiceAccount
    name: vbmc-rs-aggregator
roleRef:
  kind: Role
  name: vbmc-rs-aggregator
  apiGroup: rbac.authorization.k8s.io
```

### Deployment manifest

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: vbmc-rs-aggregator
  namespace: kubevirt-vms
spec:
  replicas: 1
  selector:
    matchLabels:
      app: vbmc-rs-aggregator
  template:
    metadata:
      labels:
        app: vbmc-rs-aggregator
    spec:
      serviceAccountName: vbmc-rs-aggregator
      containers:
        - name: aggregator
          image: vbmc-rs-aggregator:latest
          args: ["-c", "/etc/vbmc-rs/aggregator.toml"]
          ports:
            - containerPort: 8443
          volumeMounts:
            - name: config
              mountPath: /etc/vbmc-rs
              readOnly: true
            - name: tls
              mountPath: /etc/vbmc-rs/tls
              readOnly: true
      volumes:
        - name: config
          configMap:
            name: vbmc-rs-aggregator-config
        - name: tls
          secret:
            secretName: vbmc-rs-aggregator-tls
---
apiVersion: v1
kind: Service
metadata:
  name: vbmc-rs-aggregator
  namespace: kubevirt-vms
spec:
  selector:
    app: vbmc-rs-aggregator
  ports:
    - port: 8443
      targetPort: 8443
```

### mTLS between aggregator and sidecars

Use a shared CA to sign both the sidecar server certificates and the aggregator client certificate. Configure the sidecar with `tls_client_ca` pointing to the shared CA, and the aggregator with `sidecar.tls_ca`, `sidecar.tls_cert`, and `sidecar.tls_key`.

## systemd unit

```ini
[Unit]
Description=vbmc-rs Redfish virtual BMC
After=network.target
After=libvirtd.service

[Service]
Type=simple
ExecStart=/usr/local/bin/vbmc-rs -c /etc/vbmc-rs/config.toml
Restart=on-failure
RestartSec=5

# Security hardening
NoNewPrivileges=yes
ProtectSystem=strict
ProtectHome=yes
ReadWritePaths=/var/lib/vbmc-rs /var/log/vbmc-rs

# Logging
Environment=RUST_LOG=vbmc_rs=info
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
```

For the libvirt backend, the service user needs access to the libvirt socket. For Cloud-Hypervisor and QEMU, it needs read/write access to the hypervisor Unix sockets.

## Container deployment

Example `Containerfile` (adjust paths to your environment):

```dockerfile
FROM registry.fedoraproject.org/fedora:latest AS builder
RUN dnf install -y rust cargo libvirt-devel && dnf clean all
WORKDIR /build
COPY . .
RUN cargo build --release --all-features

FROM registry.fedoraproject.org/fedora-minimal:latest
RUN microdnf install -y libvirt-libs && microdnf clean all
COPY --from=builder /build/target/release/vbmc-rs /usr/local/bin/
ENTRYPOINT ["/usr/local/bin/vbmc-rs"]
CMD ["-c", "/etc/vbmc-rs/config.toml"]
```

Run with socket and state mounts:

```sh
podman run -d \
  -v /etc/vbmc-rs:/etc/vbmc-rs:ro \
  -v /var/lib/vbmc-rs:/var/lib/vbmc-rs \
  -v /var/log/vbmc-rs:/var/log/vbmc-rs \
  -v /tmp/cloud-hypervisor-vm1.sock:/tmp/cloud-hypervisor-vm1.sock \
  -p 8000:8000 \
  -p 9090:9090 \
  vbmc-rs
```

For libvirt, mount the libvirt socket instead:

```sh
podman run -d \
  -v /var/run/libvirt/libvirt-sock:/var/run/libvirt/libvirt-sock \
  ...
```

## Audit log

```toml
audit_log = "/var/log/vbmc-rs/audit.jsonl"
```

The audit log records all Redfish events in JSONL format (one JSON object per line). See [observability.md](observability.md) for the event format and log rotation recommendations.

## Logging

vbmc-rs uses the `tracing` crate. Control verbosity with `RUST_LOG`:

```sh
# Default (info level)
RUST_LOG=vbmc_rs=info vbmc-rs -c config.toml

# Debug (includes backend calls, state saves)
RUST_LOG=vbmc_rs=debug vbmc-rs -c config.toml

# Trace (very verbose, includes HTTP bodies)
RUST_LOG=vbmc_rs=trace vbmc-rs -c config.toml

# Per-module control
RUST_LOG=vbmc_rs::backend=debug,vbmc_rs::redfish=info vbmc-rs -c config.toml
```
