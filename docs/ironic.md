# Testing Ironic with vbmc-rs

This guide shows how to use vbmc-rs as a virtual BMC for [OpenStack Ironic](https://ironicbaremetal.org/) bare-metal provisioning workflows. It covers both mockup mode (no hypervisor needed) and real backends (libvirt/QEMU), as well as [Metal3](https://metal3.io/) BareMetalHost integration.

## Prerequisites

- vbmc-rs binary (see [Building](../README.md#building))
- OpenStack Ironic — standalone or via DevStack. The [Ironic standalone guide](https://docs.openstack.org/ironic/latest/user/deploy.html) covers setup.
- `baremetal` CLI (`pip install python-ironicclient`)

## Option 1: Mockup mode (no hypervisor)

The fastest way to test. vbmc-rs serves a DMTF mockup directory as a live Redfish endpoint with stateful power actions.

```sh
# Generate a mockup config (or use the included example)
vbmc-rs init --backend mockup --output config-mockup.toml

# Start vbmc-rs (uses the built-in example mockup)
vbmc-rs -c examples/config-mockup.toml
```

Verify it works:

```sh
curl -s http://localhost:8000/redfish/v1/Systems | jq '.Members[]."@odata.id"'
# "/redfish/v1/Systems/node-01"
```

## Option 2: Libvirt backend (real VMs)

Create a libvirt domain first, then point vbmc-rs at it:

```sh
# Create a test VM (no OS needed — Ironic will provision it)
virt-install --name ironic-test-vm \
  --ram 4096 --vcpus 2 \
  --disk size=20 \
  --os-variant generic \
  --boot network \
  --noautoconsole \
  --nographics

# Configure vbmc-rs
cat > config-libvirt.toml <<'EOF'
backend = "libvirt"

[server]
bind_address = "0.0.0.0"
port = 8000

[systems.node-01]
name = "Ironic Test Node"
connection_uri = "qemu:///system"
domain_name = "ironic-test-vm"

[systems.node-01.hardware]
cpu_count = 2
memory_mib = 4096
EOF

# Build with libvirt support and start
cargo build --release --features libvirt
vbmc-rs -c config-libvirt.toml
```

## Enrolling a node in Ironic

Ironic uses the `redfish` hardware type to communicate with Redfish BMCs. The key `driver_info` fields are:

| Field | Description | Example |
|-------|-------------|---------|
| `redfish_address` | BMC URL (scheme + host + port) | `http://localhost:8000` |
| `redfish_system_id` | Path to the ComputerSystem resource | `/redfish/v1/Systems/node-01` |
| `redfish_username` | BMC username (optional if auth disabled) | `admin` |
| `redfish_password` | BMC password (optional if auth disabled) | `password` |
| `redfish_verify_ca` | TLS certificate verification | `false` |

### Create the node

```sh
baremetal node create \
  --driver redfish \
  --driver-info redfish_address=http://localhost:8000 \
  --driver-info redfish_system_id=/redfish/v1/Systems/node-01 \
  --driver-info redfish_verify_ca=false \
  --name test-node
```

If vbmc-rs has authentication enabled, add credentials:

```sh
baremetal node create \
  --driver redfish \
  --driver-info redfish_address=http://localhost:8000 \
  --driver-info redfish_system_id=/redfish/v1/Systems/node-01 \
  --driver-info redfish_username=admin \
  --driver-info redfish_password=password \
  --driver-info redfish_auth_type=session \
  --name test-node
```

### Validate the node

Confirm Ironic can reach the BMC:

```sh
baremetal node validate test-node
```

All interfaces (power, management, boot) should show `True`. If power or management shows `False`, check that vbmc-rs is running and the `redfish_address` is reachable from the Ironic conductor.

### Manage and inspect

```sh
# Move from enroll → manageable
baremetal node manage test-node

# Run out-of-band inspection (reads hardware info via Redfish)
baremetal node inspect test-node

# Wait for inspection to complete
baremetal node show test-node --fields provision_state
# Should show "manageable"
```

After inspection, Ironic populates the node's `properties` with hardware details read from vbmc-rs (CPU count, memory, disk size, NIC MACs).

### Make available and deploy

```sh
# Move to available (runs cleaning if configured)
baremetal node provide test-node

# Set the deploy image and deploy
baremetal node set test-node \
  --instance-info image_source=http://image-server/deploy.qcow2 \
  --instance-info root_gb=20

baremetal node deploy test-node
```

### Set boot device

Ironic uses Redfish to set the boot source override before deployment:

```sh
# Verify boot device management works
baremetal node boot device set test-node pxe
baremetal node boot device show test-node
```

This maps to a `PATCH /redfish/v1/Systems/node-01` with `Boot.BootSourceOverrideTarget: Pxe`.

## Virtual media boot

For environments without PXE, Ironic supports booting from a virtual media ISO:

```sh
baremetal node create \
  --driver redfish \
  --boot-interface redfish-virtual-media \
  --driver-info redfish_address=http://localhost:8000 \
  --driver-info redfish_system_id=/redfish/v1/Systems/node-01 \
  --name test-node-vmedia
```

vbmc-rs supports virtual media insertion and ejection via the standard Redfish VirtualMedia resource.

## Metal3 BareMetalHost

[Metal3](https://metal3.io/) uses Ironic under the hood. The `BareMetalHost` CRD's `spec.bmc` section maps directly to Ironic's `driver_info`.

### BMC credentials secret

```yaml
apiVersion: v1
kind: Secret
metadata:
  name: node-01-bmc-credentials
  namespace: metal3
type: Opaque
data:
  username: YWRtaW4=       # admin
  password: cGFzc3dvcmQ=   # password
```

### BareMetalHost with network boot

```yaml
apiVersion: metal3.io/v1alpha1
kind: BareMetalHost
metadata:
  name: node-01
  namespace: metal3
spec:
  online: true
  bootMACAddress: "52:54:00:ab:cd:01"
  bmc:
    address: redfish://vbmc-rs.metal3.svc:8000/redfish/v1/Systems/node-01
    credentialsName: node-01-bmc-credentials
    disableCertificateVerification: true
  rootDeviceHints:
    minSizeGigabytes: 20
```

### BareMetalHost with virtual media boot

```yaml
apiVersion: metal3.io/v1alpha1
kind: BareMetalHost
metadata:
  name: node-01
  namespace: metal3
spec:
  online: true
  bootMACAddress: "52:54:00:ab:cd:01"
  bmc:
    address: redfish-virtualmedia://vbmc-rs.metal3.svc:8000/redfish/v1/Systems/node-01
    credentialsName: node-01-bmc-credentials
    disableCertificateVerification: true
```

### Address format

The `bmc.address` field uses a protocol prefix to select the Ironic driver:

| Prefix | Ironic driver | Boot method |
|--------|--------------|-------------|
| `redfish://` | `redfish` | PXE / network boot |
| `redfish-virtualmedia://` | `redfish` | Virtual media (ISO) |
| `redfish+http://` | `redfish` | PXE over plain HTTP |
| `redfish-virtualmedia+http://` | `redfish` | Virtual media over plain HTTP |

When vbmc-rs runs without TLS (development), use the `+http` variants.

### KubeVirt deployment

When running Metal3 alongside KubeVirt, deploy vbmc-rs as a sidecar using the Helm chart. The aggregator presents all VMs through a single Redfish endpoint:

```yaml
bmc:
  address: redfish-virtualmedia://vbmc-aggregator.vbmc-system.svc:8443/redfish/v1/Systems/my-vm
  credentialsName: my-vm-bmc-credentials
```

See [KubeVirt deployment](kubevirt.md) for Helm chart setup.

## Troubleshooting

### "Unable to connect to redfish_address"

- Verify vbmc-rs is running: `curl -s http://localhost:8000/redfish/v1 | jq .`
- Check the Ironic conductor can reach the address (firewalls, network namespaces)
- If using containers, ensure the port is exposed

### "No ComputerSystem found"

- Verify the `redfish_system_id` path: `curl -s http://localhost:8000/redfish/v1/Systems | jq '.Members[]'`
- The path must match exactly (e.g., `/redfish/v1/Systems/node-01`, not `/redfish/v1/Systems/1`)

### "SSL certificate verify failed"

- Set `redfish_verify_ca=false` in `driver_info` for development
- For Metal3, set `disableCertificateVerification: true` in the BareMetalHost spec
- For production, configure TLS in vbmc-rs and point `redfish_verify_ca` to the CA certificate

### "Authentication failed"

- If vbmc-rs auth is disabled (`[auth] enabled = false`), omit `redfish_username` and `redfish_password`
- If auth is enabled, verify credentials match the accounts file (see [Deployment](deployment.md))
- Set `redfish_auth_type=session` for session-based auth (default is `auto`)

### Inspection returns incomplete data

- In mockup mode, inspection results depend on what resources exist in the mockup directory. Add `Processors/`, `Memory/`, `EthernetInterfaces/`, and `Storage/` subdirectories with `index.json` files for richer inspection data.
- With real backends, vbmc-rs reports live hardware info from the hypervisor.

### Power actions fail

- Check vbmc-rs logs: `RUST_LOG=vbmc_rs=debug vbmc-rs -c config.toml`
- In mockup mode, power actions change the in-memory `PowerState` but do not affect real VMs
- With libvirt, ensure the domain exists: `virsh list --all`
