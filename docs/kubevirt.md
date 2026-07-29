# KubeVirt Deployment Guide

This document covers deploying vbmc-rs with the KubeVirt backend.

## Overview

The KubeVirt backend maps Redfish operations to KubeVirt API subresource calls. VMs are created and configured externally (kubectl, Helm, GitOps) — vbmc-rs provides manage-only access through a standards-compliant Redfish interface.

Two deployment models:

| Model | Backend | Description | Use case |
|-------|---------|-------------|----------|
| **Sidecar** | libvirt | One vbmc-rs per VM in the virt-launcher pod, talks to the local libvirtd | Per-VM isolation, full metrics (block + network I/O), secure boot control |
| **API backend** | kubevirt | Single instance managing multiple VMs via Kubernetes API | Centralized management, fewer pods, no libvirt dependency |

The **sidecar model is recommended**. Each KubeVirt virt-launcher pod runs a local libvirtd that manages the QEMU process. The sidecar connects to this libvirtd via its Unix socket, giving access to the full feature set including network I/O counters that the KubeVirt API does not expose.

The **aggregator** (`vbmc-rs-aggregator`) sits in front of sidecar instances, discovering them and presenting a unified Redfish interface.

## Build

```sh
# Sidecar image (libvirt backend)
podman build -f Containerfile.kubevirt --target sidecar -t vbmc-rs-kubevirt-sidecar .

# Aggregator image (static binary)
podman build -f Containerfile.kubevirt --target aggregator -t vbmc-rs-kubevirt-aggregator .

# API backend (no libvirt needed)
cargo build --release --features kubevirt
```

## Sidecar deployment

The sidecar runs alongside the VM in the virt-launcher pod. It uses the **libvirt backend** to connect to the local libvirtd socket, providing full BMC functionality including per-disk block I/O and per-NIC network counters.

### Feature comparison: sidecar (libvirt) vs API backend

| Feature | Sidecar (libvirt) | API backend (kubevirt) |
|---------|-------------------|------------------------|
| Power control | libvirt domain API | KubeVirt subresources |
| CPU/memory info | domain XML + get_info | VMI spec |
| Disk inventory | domain XML (path, bus, type) | VM spec (names only) |
| NIC inventory + MACs | domain XML | VM spec |
| Block I/O counters | get_block_stats per disk | Not available |
| Network I/O counters | interface_stats per NIC | Not available |
| PCI passthrough devices | domain XML hostdev | Not available |
| Secure boot state | domain XML loader secure= | VM spec |
| Secure boot toggle | domain XML redefine | spec patch |
| Hot-plug disk | attach_device | addvolume subresource |

### Pod spec

```yaml
apiVersion: kubevirt.io/v1
kind: VirtualMachine
metadata:
  name: my-vm
  namespace: default
  labels:
    app.kubernetes.io/name: vbmc-rs-sidecar
    vbmc-rs/system-id: my-vm
spec:
  running: true
  template:
    metadata:
      labels:
        app.kubernetes.io/name: vbmc-rs-sidecar
        vbmc-rs/system-id: my-vm
    spec:
      domain:
        cpu:
          cores: 2
        memory:
          guest: "2Gi"
        devices:
          disks:
            - name: rootdisk
              disk:
                bus: virtio
      volumes:
        - name: rootdisk
          containerDisk:
            image: quay.io/containerdisks/fedora:latest
      containers:
        - name: vbmc-rs
          image: ghcr.io/fabiendupont/vbmc-rs-kubevirt-sidecar:latest
          args: ["-c", "/etc/vbmc-rs/config.toml"]
          ports:
            - containerPort: 8000
          volumeMounts:
            - name: vbmc-config
              mountPath: /etc/vbmc-rs
              readOnly: true
            - name: vbmc-state
              mountPath: /var/lib/vbmc-rs
            - name: libvirt-sock
              mountPath: /var/run/libvirt
      volumes:
        - name: vbmc-config
          configMap:
            name: vbmc-rs-my-vm
        - name: vbmc-state
          emptyDir: {}
        - name: libvirt-sock
          emptyDir: {}
```

The `libvirt-sock` volume shares the libvirtd socket between the virt-launcher container and the vbmc-rs sidecar.

### Per-VM configuration

Each sidecar manages a single system using the libvirt backend. The `connection_uri` points to the local libvirtd socket, and `domain_name` matches the VM's libvirt domain:

```toml
backend = "libvirt"

[server]
bind_address = "0.0.0.0"
port = 8000

[systems.my-vm]
name = "My VM"
connection_uri = "qemu:///system"
domain_name = "default_my-vm"

[systems.my-vm.hardware]
cpu_count = 2
memory_mib = 2048
```

KubeVirt names libvirt domains as `{namespace}_{vm-name}`. The `connection_uri` defaults to `qemu:///system` if omitted.

## API backend deployment

A single vbmc-rs instance manages multiple VMs. Each system section maps to a namespace/vm_name pair:

```toml
backend = "kube_virt"

[server]
bind_address = "0.0.0.0"
port = 8000
tls_cert = "/etc/vbmc-rs/server.crt"
tls_key = "/etc/vbmc-rs/server.key"

[auth]
enabled = true
accounts_file = "/etc/vbmc-rs/accounts.json"

[systems.web-server]
name = "Web Server"
namespace = "production"
vm_name = "web-server-vm"

[systems.web-server.hardware]
cpu_count = 4
memory_mib = 8192

[systems.db-server]
name = "Database Server"
namespace = "production"
vm_name = "db-server-vm"

[systems.db-server.hardware]
cpu_count = 8
memory_mib = 16384
```

The instance uses the default kubeconfig resolution order:

1. `KUBECONFIG` environment variable
2. `~/.kube/config`
3. In-cluster service account (when running inside a pod)

## Aggregator deployment

The aggregator discovers sidecar instances and proxies Redfish requests to the correct sidecar based on system ID.

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
mode = "kubernetes"
namespace = "default"
label_selector = "app.kubernetes.io/name=vbmc-rs-sidecar"

[sidecar]
port = 8000
tls_ca = "/etc/vbmc-rs/sidecar-ca.crt"
tls_cert = "/etc/vbmc-rs/aggregator-client.crt"
tls_key = "/etc/vbmc-rs/aggregator-client.key"
```

### Discovery modes

**Static** — fixed endpoint list, no Kubernetes access required:

```toml
[discovery]
mode = "static"

[[discovery.endpoints]]
system_id = "vm1"
url = "http://10.0.0.1:8000"

[[discovery.endpoints]]
system_id = "vm2"
url = "http://10.0.0.2:8000"
```

**Kubernetes** — watches pods matching label selector. System ID is extracted from the `vbmc-rs/system-id` label (falls back to pod name). URL is built from pod IP and `sidecar.port`. Only Ready pods are registered.

### Kubernetes RBAC

The aggregator needs pod list/watch permissions in the target namespace:

```yaml
apiVersion: v1
kind: ServiceAccount
metadata:
  name: vbmc-rs-aggregator
---
apiVersion: rbac.authorization.k8s.io/v1
kind: Role
rules:
  - apiGroups: [""]
    resources: ["pods"]
    verbs: ["get", "list", "watch"]
---
apiVersion: rbac.authorization.k8s.io/v1
kind: RoleBinding
subjects:
  - kind: ServiceAccount
    name: vbmc-rs-aggregator
roleRef:
  kind: Role
  name: vbmc-rs-aggregator
  apiGroup: rbac.authorization.k8s.io
```

### mTLS between aggregator and sidecars

Generate a shared CA and sign both the sidecar server certificates and the aggregator client certificate:

```sh
# Shared CA
openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 \
  -keyout sidecar-ca.key -out sidecar-ca.crt -days 3650 -nodes \
  -subj "/CN=vbmc-rs sidecar CA"

# Sidecar server cert (one per sidecar, or wildcard)
openssl req -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 \
  -keyout sidecar-server.key -out sidecar-server.csr -nodes \
  -subj "/CN=vbmc-rs-sidecar"
openssl x509 -req -in sidecar-server.csr -CA sidecar-ca.crt -CAkey sidecar-ca.key \
  -CAcreateserial -out sidecar-server.crt -days 365

# Aggregator client cert
openssl req -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 \
  -keyout aggregator-client.key -out aggregator-client.csr -nodes \
  -subj "/CN=vbmc-rs-aggregator"
openssl x509 -req -in aggregator-client.csr -CA sidecar-ca.crt -CAkey sidecar-ca.key \
  -CAcreateserial -out aggregator-client.crt -days 365
```

Configure each sidecar to require client certificates:

```toml
[server]
tls_cert = "/etc/vbmc-rs/sidecar-server.crt"
tls_key = "/etc/vbmc-rs/sidecar-server.key"
tls_client_ca = "/etc/vbmc-rs/sidecar-ca.crt"
```

## Security model

### Per-VM isolation (sidecar)

Each sidecar connects to the local libvirtd socket inside the virt-launcher pod. It has access to exactly one VM's domain — no Kubernetes API access needed, no service account required. Compromise of a sidecar affects only that VM. mTLS between aggregator and sidecar ensures only authorized aggregators can send commands.

### Service account RBAC (API backend)

The API backend service account needs broader permissions:

```yaml
apiVersion: rbac.authorization.k8s.io/v1
kind: Role
rules:
  - apiGroups: ["kubevirt.io"]
    resources: ["virtualmachines", "virtualmachineinstances"]
    verbs: ["get", "list", "patch", "delete"]
  - apiGroups: ["kubevirt.io"]
    resources: ["virtualmachines/start", "virtualmachines/stop", "virtualmachines/restart", "virtualmachines/addvolume", "virtualmachines/removevolume"]
    verbs: ["update"]
  - apiGroups: ["kubevirt.io"]
    resources: ["virtualmachineinstances/softreboot"]
    verbs: ["update"]
```

### mTLS trust boundaries

| Boundary | Enforcement |
|----------|-------------|
| Client to aggregator | Server TLS + Redfish auth (session/basic) |
| Aggregator to sidecar | mTLS with shared CA |
| Sidecar to libvirtd | Unix socket (pod-local, no network exposure) |

## Configuration reference

### KubeVirt-specific system fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `namespace` | string | `"default"` | Kubernetes namespace containing the VM |
| `vm_name` | string | system ID | KubeVirt VirtualMachine resource name |

### Aggregator discovery fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `discovery.mode` | string | `"static"` | Discovery mode: `"static"` or `"kubernetes"` |
| `discovery.namespace` | string | default namespace | Namespace to watch for sidecar pods |
| `discovery.label_selector` | string | `"app.kubernetes.io/name=vbmc-rs-sidecar"` | Label selector for pod discovery |
| `discovery.endpoints` | array | `[]` | Static endpoint list (static mode only) |
| `sidecar.port` | u16 | `8000` | Port to connect to on discovered sidecar pods |
| `sidecar.tls_ca` | path | — | CA certificate to verify sidecar server certificates |
| `sidecar.tls_cert` | path | — | Client certificate for mTLS to sidecars |
| `sidecar.tls_key` | path | — | Client key for mTLS to sidecars |

### KubeVirt subresource mapping

| Redfish operation | KubeVirt subresource | Resource |
|---|---|---|
| Reset (On/ForceOn) | `start` | `virtualmachines` |
| Reset (ForceOff/GracefulShutdown) | `stop` | `virtualmachines` |
| Reset (GracefulRestart/ForceRestart) | `restart` | `virtualmachines` |
| Reset (PushPowerButton) | `softreboot` | `virtualmachineinstances` |
| Add disk (VirtualMedia, Storage) | `addvolume` | `virtualmachines` |
| Remove device | `removevolume` | `virtualmachines` |
| SecureBoot enable/disable | `patch` (merge) | `virtualmachines` |
