# IPMI Extern BMC

vbmc-rs can act as an external IPMI BMC for QEMU via the `ipmi-bmc-extern` device. This exposes a real IPMI KCS (Keyboard Controller Style) interface to the guest OS, so tools like `ipmitool` work inside the VM as if it were running on physical hardware with a real BMC.

## How it works

QEMU's `ipmi-bmc-extern` device connects to an external BMC process over a socket. The BMC handles IPMI commands (chassis power, boot options, device identification) and QEMU presents the corresponding KCS interface to the guest. The guest sees SMBIOS Type 38 (IPMI Device Information) and ACPI tables automatically.

```
┌─────────────────────┐
│    Guest OS          │
│  ┌───────────────┐   │
│  │   ipmitool    │   │
│  └──────┬────────┘   │
│         │ KCS I/O    │
│  ┌──────▼────────┐   │
│  │ isa-ipmi-kcs  │   │
│  └──────┬────────┘   │
└─────────┼────────────┘
          │ ipmi-bmc-extern protocol
   ┌──────▼────────┐
   │   vbmc-rs     │
   │  IPMI server  │
   └──────┬────────┘
          │ backend operations
   ┌──────▼────────┐
   │  VM backend   │
   │  (any backend)│
   └───────────────┘
```

## Configuration

Add `ipmi_socket` to a system's configuration to enable the IPMI extern BMC:

```toml
backend = "cloud_hypervisor"

[server]
bind_address = "127.0.0.1"
port = 8000

[systems.vm1]
name = "My VM"
socket_path = "/tmp/cloud-hypervisor-vm1.sock"
ipmi_socket = "/var/run/vbmc-rs/ipmi-vm1.sock"
```

vbmc-rs will listen on the specified Unix socket. QEMU connects to it as a client.

## QEMU command line

Start QEMU with the IPMI device pointing at the vbmc-rs socket:

```sh
qemu-system-x86_64 \
  -machine q35 \
  -chardev socket,id=ipmi0,path=/var/run/vbmc-rs/ipmi-vm1.sock \
  -device ipmi-bmc-extern,id=bmc0,chardev=ipmi0 \
  -device isa-ipmi-kcs,bmc=bmc0 \
  ...
```

For PCI-based KCS (may work better on Q35 machines):

```sh
  -device pci-ipmi-kcs,bmc=bmc0
```

## Libvirt domain XML

For libvirt-managed VMs, use `<qemu:commandline>` to inject the IPMI device:

```xml
<domain type='kvm' xmlns:qemu='http://libvirt.org/schemas/domain/qemu/1.0'>
  <!-- ... normal domain config ... -->
  <qemu:commandline>
    <qemu:arg value='-chardev'/>
    <qemu:arg value='socket,id=ipmi0,path=/var/run/vbmc-rs/ipmi-vm1.sock'/>
    <qemu:arg value='-device'/>
    <qemu:arg value='ipmi-bmc-extern,id=bmc0,chardev=ipmi0'/>
    <qemu:arg value='-device'/>
    <qemu:arg value='isa-ipmi-kcs,bmc=bmc0'/>
  </qemu:commandline>
</domain>
```

## Supported IPMI commands

| Command | NetFn | Cmd | Description |
|---------|-------|-----|-------------|
| Get Device ID | App (0x06) | 0x01 | Returns BMC device identification |
| Get Channel Auth Capabilities | App (0x06) | 0x38 | Authentication capabilities |
| Get Chassis Status | Chassis (0x00) | 0x01 | Power state, last power event |
| Chassis Control | Chassis (0x00) | 0x02 | Power on/off/cycle/reset/shutdown |
| Get System Boot Options | Chassis (0x00) | 0x09 | Boot device, boot flags |
| Set System Boot Options | Chassis (0x00) | 0x08 | Set boot device for next boot |

Chassis Control actions map to vbmc-rs backend operations:

| IPMI Control | vbmc-rs action |
|-------------|----------------|
| Power Off (0x00) | `vm_shutdown` |
| Power On (0x01) | `vm_boot` |
| Power Cycle (0x02) | `vm_shutdown` + `vm_boot` |
| Hard Reset (0x03) | `vm_reboot` |
| Pulse (0x04) | `vm_power_button` |
| Soft Shutdown (0x05) | `vm_power_button` |

## KubeVirt integration

For KubeVirt VMs, the IPMI device can be injected via a hook sidecar that modifies the libvirt domain XML before QEMU starts. The vbmc-rs sidecar listens on the IPMI socket and the hook sidecar adds the `<qemu:commandline>` entries. See [KubeVirt deployment](kubevirt.md) for the sidecar architecture.

## Known limitations

- **QEMU 10.2 (Fedora 44)**: The `ipmi-bmc-extern` device may crash during initialization. The `ipmi-bmc-sim` built-in device works on the same version. This appears to be a QEMU regression in the chardev event handling during device realize. Test with your QEMU version before deploying.
- The IPMI interface is not exposed through libvirt's native XML schema — `<qemu:commandline>` passthrough is required.
- Adding the IPMI PCI device changes PCI IDs, which may affect other device ordering.

## Guest verification

Once the VM boots with the IPMI device, verify from inside the guest:

```sh
# Check SMBIOS for IPMI device
dmidecode --type 38

# Load kernel module
modprobe ipmi_si
modprobe ipmi_devintf

# Query BMC
ipmitool bmc info
ipmitool chassis status
ipmitool chassis power status
```
