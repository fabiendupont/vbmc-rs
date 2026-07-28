# KubeVirt Deployment Guide

This document covers deploying vbmc-rs with the KubeVirt backend.

## Overview

The KubeVirt backend maps Redfish operations to KubeVirt API subresource calls. VMs are created and configured externally (kubectl, Helm, GitOps) — vbmc-rs provides manage-only access through a standards-compliant Redfish interface.

Two deployment models:

| Model | Description | Use case |
|-------|-------------|----------|
| **Sidecar** | One vbmc-rs instance per VM, co-located in the virt-launcher pod | Per-VM isolation, no cross-VM access, Kubernetes-native lifecycle |
| **API backend** | Single vbmc-rs instance managing multiple VMs via kubeconfig | Centralized management, fewer pods, simpler monitoring |

The **aggregator** (`vbmc-rs-aggregator`) sits in front of sidecar instances, discovering them and presenting a unified Redfish interface.

## Build

```sh
cargo build --release --features kubevirt
```

No system dependencies beyond the Rust toolchain and network access to a Kubernetes API server.

## Sidecar deployment

Deploy vbmc-rs as a container alongside the virt-launcher in each VM's pod. The sidecar uses the pod's service account to talk to the KubeVirt API.

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
          image: vbmc-rs:latest
          args: ["-c", "/etc/vbmc-rs/config.toml"]
          ports:
            - containerPort: 8000
          volumeMounts:
            - name: vbmc-config
              mountPath: /etc/vbmc-rs
              readOnly: true
            - name: vbmc-state
              mountPath: /var/lib/vbmc-rs
      volumes:
        - name: vbmc-config
          configMap:
            name: vbmc-rs-my-vm
        - name: vbmc-state
          emptyDir: {}
```

### Per-VM configuration

Each sidecar manages a single system. The config maps the system ID to the VM's namespace and name:

```toml
backend = "kube_virt"

[server]
bind_address = "0.0.0.0"
port = 8000

[systems.my-vm]
name = "My VM"
namespace = "default"
vm_name = "my-vm"

[systems.my-vm.hardware]
cpu_count = 2
memory_mib = 2048
```

The sidecar resolves the KubeVirt API via the in-cluster service account. No kubeconfig file is needed.

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

Each sidecar has access to exactly one VM. Compromise of a sidecar affects only that VM. The service account can be scoped to a single namespace. mTLS between aggregator and sidecar ensures only authorized aggregators can send commands.

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
| Sidecar to Kubernetes API | Service account token (in-cluster) or kubeconfig |

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
