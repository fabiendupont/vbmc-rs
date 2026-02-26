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

## TLS configuration

```toml
[server]
tls_cert = "/etc/vbmc-rs/server.crt"
tls_key = "/etc/vbmc-rs/server.key"
```

For mutual TLS (client certificate verification):

```toml
[server]
tls_cert = "/etc/vbmc-rs/server.crt"
tls_key = "/etc/vbmc-rs/server.key"
tls_client_ca = "/etc/vbmc-rs/ca.crt"
```

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
