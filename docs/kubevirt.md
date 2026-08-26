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

Three components work together in the sidecar model:

- **Webhook** (`vbmc-rs-webhook`) — a mutating admission webhook that automatically injects the vbmc-rs sidecar container into virt-launcher pods
- **Aggregator** (`vbmc-rs-aggregator`) — discovers sidecars, authenticates clients via Kubernetes OAuth (TokenReview + SubjectAccessReview), and proxies Redfish requests
- **Sidecar** — the vbmc-rs instance running inside the virt-launcher pod

## Architecture

```
                       ┌─────────────────────┐
                       │    Redfish Client    │
                       │   (Bearer token)     │
                       └──────────┬──────────┘
                                  │
                       ┌──────────▼──────────┐
                       │     Aggregator      │
                       │  (vbmc-system ns)   │
                       │                     │
                       │ • Kubernetes OAuth   │
                       │   (TokenReview +     │
                       │    SAR filtering)    │
                       │ • Pod discovery      │
                       │ • mTLS proxy         │
                       └──────────┬──────────┘
                                  │ BMC Network (CUDN)
                  ┌───────────────┼───────────────┐
                  │               │               │
           ┌──────▼──────┐┌──────▼──────┐┌──────▼──────┐
           │  Sidecar    ││  Sidecar    ││  Sidecar    │
           │  (ns: prod) ││  (ns: dev)  ││  (ns: test) │
           │  libvirtd   ││  libvirtd   ││  libvirtd   │
           └─────────────┘└─────────────┘└─────────────┘
```

### BMC network isolation

Sidecars communicate with the aggregator over a dedicated **ClusterUserDefinedNetwork** (CUDN) using OVN-Kubernetes. This gives each sidecar a secondary network interface on an isolated L2 segment (e.g., 192.168.200.0/24), separate from the VM's primary network.

An **AdminNetworkPolicy** restricts traffic on the BMC network so only the aggregator can reach sidecars — no pod-to-pod BMC traffic.

### Kubernetes OAuth

The aggregator authenticates clients using Kubernetes service account tokens:

1. Client sends `Authorization: Bearer <token>` (a Kubernetes SA token)
2. Aggregator validates the token via **TokenReview**
3. For each system, aggregator checks access via **SubjectAccessReview** — the client must have `get` on `virtualmachines` in the system's namespace
4. Only authorized systems are returned in the Systems collection

This provides multi-tenant isolation using existing Kubernetes RBAC — no separate auth system.

## Helm chart deployment (recommended)

A Helm chart in `charts/vbmc-rs/` deploys all components:

```sh
# Build container images
podman build -f Containerfile.kubevirt --target sidecar -t vbmc-rs-sidecar .
podman build -f Containerfile.kubevirt --target aggregator -t vbmc-rs-aggregator .
podman build -f Containerfile.kubevirt --target webhook -t vbmc-rs-webhook .

# Install
helm install vbmc-rs charts/vbmc-rs/ \
    --set image.registry=ghcr.io/fabiendupont \
    --set webhook.sidecarImage=ghcr.io/fabiendupont/vbmc-rs-sidecar:latest
```

The chart creates:

| Resource | Description |
|----------|-------------|
| Namespace (`vbmc-system`) | Dedicated namespace for vbmc-rs components |
| ServiceAccount + ClusterRole/Binding | Aggregator RBAC (TokenReview, SAR, pod watch) |
| ConfigMap | Aggregator TOML configuration |
| Deployments | Aggregator and webhook |
| Services | Aggregator (HTTP) and webhook (HTTPS) |
| Secret | Auto-generated webhook TLS certificate |
| MutatingWebhookConfiguration | Sidecar injection into virt-launcher pods |
| ClusterUserDefinedNetwork | BMC management network (CUDN) |
| AdminNetworkPolicy | Restricts BMC traffic to aggregator only |

### Key values

```yaml
# Container registry for aggregator and webhook images
image:
  registry: ghcr.io/fabiendupont

# Sidecar image (fully qualified — injected into arbitrary namespaces)
webhook:
  sidecarImage: ghcr.io/fabiendupont/vbmc-rs-sidecar:latest
  # Inject TLS secret into sidecars for mTLS
  tlsSecret: ""
  # swtpm socket path for vTPM attestation
  swtpmSocket: ""
  # Keylime verifier URL
  keylimeUrl: ""

# Aggregator mTLS to sidecars
aggregator:
  mtls:
    enabled: false
    existingSecret: ""

# BMC network
bmcNetwork:
  name: vbmc-bmc
  cudn:
    enabled: true
    subnet: "192.168.200.0/24"
  adminNetworkPolicy:
    enabled: true
```

### Webhook TLS

The chart auto-generates self-signed TLS certificates for the webhook using Helm's `genCA`/`genSignedCert`. To provide your own:

```yaml
webhook:
  tls:
    certPEM: |
      -----BEGIN CERTIFICATE-----
      ...
    keyPEM: |
      -----BEGIN EC PRIVATE KEY-----
      ...
    caPEM: |
      -----BEGIN CERTIFICATE-----
      ...
```

## Sidecar injection

The mutating webhook automatically injects the vbmc-rs sidecar into any virt-launcher pod that has both `kubevirt.io: virt-launcher` and `vbmc-rs/system-id` labels. No manual pod spec changes are needed.

To enable vbmc-rs for a VM, add the `vbmc-rs/system-id` label:

```yaml
apiVersion: kubevirt.io/v1
kind: VirtualMachine
metadata:
  name: my-vm
  labels:
    vbmc-rs/system-id: my-vm
spec:
  runStrategy: Always
  template:
    metadata:
      labels:
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
```

The webhook injects:

- A `vbmc-rs` sidecar container with an inline TOML config
- A BMC network annotation (`k8s.v1.cni.cncf.io/networks`)
- The libvirt socket volume mount (reuses existing or creates one)
- TLS cert volume (if `--tls-secret` is configured)
- swtpm socket volume (if `--swtpm-socket` is configured)

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
| vTPM attestation | swtpm socket (direct) | Not available |

## Aggregator

The aggregator discovers sidecar instances across all namespaces and proxies Redfish requests to the correct sidecar based on system ID.

### Discovery

In Kubernetes mode, the aggregator watches pods cluster-wide matching the configured label selector. When `discovery.namespace` is omitted, it watches all namespaces. System ID is extracted from the `vbmc-rs/system-id` label. When a BMC network (CUDN) is configured, the aggregator uses the pod's BMC network IP from the `k8s.v1.cni.cncf.io/network-status` annotation instead of the pod IP.

### Revocation webhook

The aggregator exposes `POST /api/v1/revocation` for Keylime attestation revocation events. When Keylime detects an attestation failure, it calls this endpoint with `{"agent_id": "<system-id>"}`, and the aggregator proxies a `ForceOff` reset to the sidecar — closing the detect-to-remediate loop through the Redfish interface.

### mTLS between aggregator and sidecars

When sidecars are deployed with TLS (via the webhook's `--tls-secret` option), the aggregator needs mTLS client certificates. Enable via Helm:

```sh
helm upgrade vbmc-rs charts/vbmc-rs/ \
    --set webhook.tlsSecret=vbmc-rs-bmc-tls \
    --set aggregator.mtls.enabled=true
```

This requires a `vbmc-rs-bmc-tls` secret in the VM namespace (sidecar server cert) and in `vbmc-system` (aggregator client cert + CA). See `tests/e2e/mtls.sh` for a complete example.

## Attestation

The sidecar supports vTPM-based attestation by reading PCR values directly from the swtpm socket in the virt-launcher pod.

### swtpm (local vTPM)

PCRs 0-7 (firmware/boot chain) are read directly from the swtpm socket — no agent inside the guest is needed. Configure via Helm:

```sh
helm install vbmc-rs charts/vbmc-rs/ \
    --set webhook.swtpmSocket=/var/run/swtpm/swtpm-sock
```

The webhook mounts the swtpm socket directory and generates attestation config in the sidecar's inline TOML. Results are exposed via the Redfish `ComponentIntegrity` resource.

### Local policy validation

PCR values can be validated against expected values from config:

```toml
[systems.my-vm.attestation]
provider = "swtpm"
swtpm_socket = "/var/run/swtpm/swtpm-sock"
poll_interval_seconds = 30

[systems.my-vm.attestation.pcr_policy]
0 = "expected_base64_value_for_pcr0"
7 = "expected_base64_value_for_pcr7"
```

### Dual-source attestation

External (swtpm) and internal (Keylime agent) attestation are complementary:

| Source | PCRs | What it attests |
|--------|------|-----------------|
| swtpm (sidecar) | 0-7 | Firmware, bootloader, kernel — boot chain integrity |
| Keylime agent (guest) | 8-15 + IMA | Runtime OS integrity, file measurements |

## API backend deployment

A single vbmc-rs instance manages multiple VMs via the Kubernetes API. Each system section maps to a namespace/vm_name pair:

```toml
backend = "kube_virt"

[server]
bind_address = "0.0.0.0"
port = 8000

[systems.web-server]
name = "Web Server"
namespace = "production"
vm_name = "web-server-vm"

[systems.web-server.hardware]
cpu_count = 4
memory_mib = 8192
```

## Security model

### Multi-tenant isolation

| Layer | Mechanism |
|-------|-----------|
| Client authentication | Kubernetes SA tokens via TokenReview |
| Per-system authorization | SubjectAccessReview per namespace |
| Network isolation | AdminNetworkPolicy on BMC CUDN |
| Sidecar ↔ aggregator | mTLS with shared CA |
| Sidecar ↔ libvirtd | Unix socket (pod-local) |
| Attestation | swtpm PCR validation + Keylime revocation |

### Trust boundaries

| Boundary | Enforcement |
|----------|-------------|
| Client → aggregator | Server TLS + Kubernetes OAuth |
| Aggregator → sidecar | mTLS with shared CA |
| Sidecar → libvirtd | Unix socket (pod-local, no network exposure) |
| Keylime → aggregator | Revocation webhook → ForceOff |

## Configuration reference

### KubeVirt-specific system fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `namespace` | string | `"default"` | Kubernetes namespace containing the VM |
| `vm_name` | string | system ID | KubeVirt VirtualMachine resource name |

### Attestation fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `attestation.provider` | string | — | Attestation provider: `"swtpm"`, `"keylime"`, `"trustee"` |
| `attestation.swtpm_socket` | string | `/var/run/swtpm/swtpm-sock` | swtpm Unix socket path |
| `attestation.provider_url` | string | — | Keylime/Trustee verifier URL |
| `attestation.poll_interval_seconds` | u64 | `30` | Polling interval |
| `attestation.pcr_policy` | map | — | Expected PCR values (base64) for local validation |

### Aggregator discovery fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `discovery.mode` | string | `"static"` | Discovery mode: `"static"` or `"kubernetes"` |
| `discovery.namespace` | string | all namespaces | Namespace to watch (omit for cluster-wide) |
| `discovery.label_selector` | string | `"vbmc-rs/system-id"` | Label selector for pod discovery |
| `discovery.bmc_network` | string | — | CUDN name for BMC IP extraction |
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
