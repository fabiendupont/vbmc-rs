# Redfish Conformance Profile

This document describes which Redfish resources vbmc-rs implements, where property values come from, and what is writable. It complements the DMTF Redfish specification — it does not re-document it.

## Data source categories

Every Redfish property in vbmc-rs comes from one of four sources:

| Source | Description |
|--------|-------------|
| **Backend** | Live data from the hypervisor via `VmmBackend` trait (power state, CPU count, disk list, NIC MACs, etc.) |
| **Host** | Read from the host machine at request time (CPU model from `/proc/cpuinfo`) |
| **Persisted** | Mutable state saved as JSON in `state_directory` (boot override, secure boot, virtual media, BIOS settings, licenses) |
| **Static** | Hardcoded constants (manufacturer strings, device types, synthetic sensor readings) |

## Resource conformance

### ComputerSystem (`/redfish/v1/Systems/{id}`)

| Property | Source | Notes |
|----------|--------|-------|
| PowerState | Backend | `vm_info().power_state` |
| UUID | Backend | From hypervisor if available, otherwise derived from system ID |
| ProcessorSummary.Count | Backend | `vm_info().cpu_count` |
| ProcessorSummary.LogicalProcessorCount | Backend | `vm_info().max_cpu_count` |
| MemorySummary.TotalSystemMemoryGiB | Backend | `vm_info().memory_bytes` converted |
| Boot.BootSourceOverrideTarget | Persisted | Boot override target (Hdd, Cd, Pxe) |
| Boot.BootSourceOverrideEnabled | Persisted | Once, Continuous, or Disabled |
| Boot.BootSourceOverrideMode | Persisted | UEFI or Legacy |
| SerialNumber | Static | Derived from UUID |
| Manufacturer, Model, SystemType | Static | Hardcoded |
| BiosVersion | Static | "vbmc-rs" |
| BootProgress | Static | Derived from power state |
| LastResetTime | Static | Current UTC time per request |

**Writable:** `PATCH /Systems/{id}` accepts `Boot.BootSourceOverrideTarget`, `Boot.BootSourceOverrideEnabled`, `Boot.BootSourceOverrideMode`.

### ComputerSystem.Reset (`/redfish/v1/Systems/{id}/Actions/ComputerSystem.Reset`)

Supported `ResetType` values:

| ResetType | Backend method | Behavior |
|-----------|---------------|----------|
| On, ForceOn | `vm_create()` + `vm_boot()` | Creates and boots the VM |
| ForceOff | `vm_shutdown()` + `vm_delete()` | Immediate power off and cleanup |
| GracefulShutdown | `vm_power_button()` | ACPI power button signal |
| GracefulRestart | `vm_reboot()` | Guest-initiated reboot |
| ForceRestart | `vm_delete()` + `vm_create()` + `vm_boot()` | Hard restart |
| PushPowerButton | `vm_power_button()` | ACPI power button signal |

Side effects: clears one-time boot override after reset, emits power state change event.

### Processors (`/redfish/v1/Systems/{id}/Processors/CPU0`)

| Property | Source | Notes |
|----------|--------|-------|
| TotalCores, TotalThreads | Backend | From `vm_info().cpu_count` |
| TotalEnabledCores | Backend | From `vm_info().max_cpu_count` |
| Manufacturer, Model, MaxSpeedMHz | Host | Parsed from `/proc/cpuinfo` |
| ProcessorType | Static | "CPU" |
| InstructionSet | Static | "x86-64" |
| Socket, ProcessorIndex | Static | Always CPU0 |

**Read-only.** Single processor resource regardless of CPU count.

### Memory (`/redfish/v1/Systems/{id}/Memory/DIMM0`)

| Property | Source | Notes |
|----------|--------|-------|
| CapacityMiB | Backend | `vm_info().memory_bytes / (1024*1024)` |
| MemoryDeviceType | Static | "DDR4" |
| DataWidthBits | Static | 64 |
| ErrorCorrection | Static | "NoECC" |
| OperatingSpeedMhz | Static | 3200 |
| Manufacturer | Static | "Virtual" |

**Read-only.** Single DIMM resource regardless of memory topology.

### EthernetInterfaces (`/redfish/v1/Systems/{id}/EthernetInterfaces/{nic}`)

| Property | Source | Notes |
|----------|--------|-------|
| MACAddress | Backend | `vm_info().nics[i].mac_address` |
| SpeedMbps | Backend | `vm_info().nics[i].speed_mbps` |
| Status | Static | Always Enabled/OK |

**Read-only.** Collection size matches backend NIC count.

### Storage (`/redfish/v1/Systems/{id}/Storage/{ctrl}`)

| Property | Source | Notes |
|----------|--------|-------|
| Drive list | Backend | From `vm_info().disks` |
| Drive.Protocol | Backend | Virtio, NVMe, SATA, VhostUser |
| Drive.MediaType | Backend | From `disk.media_type` |
| Drive.CapacityBytes | Backend | From `disk.capacity_bytes` |
| Controller properties | Static | Firmware version, manufacturer, model |

**Read-only.** Drives grouped by protocol into separate storage controllers. Each drive is also exposed as a volume.

### PCIeDevices (`/redfish/v1/Systems/{id}/PCIeDevices/{dev}`)

| Property | Source | Notes |
|----------|--------|-------|
| Device list | Backend | From `vm_info().pci_devices` |
| VendorId, DeviceId | Backend | From PCI device info |
| ClassCode | Backend | From PCI device info |
| Functions | Backend | From `device.functions` |

**Read-only.** Collection size matches backend PCI device count.

### BIOS (`/redfish/v1/Systems/{id}/Bios`)

| Property | Source | Notes |
|----------|--------|-------|
| Attributes.BootOrder | Persisted | From `vm_state.bios_settings` |
| Attributes.SecureBootMode | Persisted | From `vm_state.bios_settings` |
| AttributeRegistry | Static | Hardcoded |

**Writable:** `PATCH /Systems/{id}/Bios/Settings` accepts `Attributes.BootOrder` and `Attributes.SecureBootMode`. Changes are pending until next boot.

### SecureBoot (`/redfish/v1/Systems/{id}/SecureBoot`)

| Property | Source | Notes |
|----------|--------|-------|
| SecureBootEnable | Persisted | `vm_state.secure_boot_enabled` |
| SecureBootCurrentBoot | Static | Derived from enable flag |
| SecureBootMode | Static | "UserMode" |

**Writable:** `PATCH /Systems/{id}/SecureBoot` accepts `SecureBootEnable`.

### VirtualMedia (`/redfish/v1/Systems/{id}/VirtualMedia/Cd`)

| Property | Source | Notes |
|----------|--------|-------|
| Inserted | Persisted | `vm_state.virtual_media.inserted` |
| Image | Persisted | `vm_state.virtual_media.image_url` |
| MediaTypes | Static | ["CD", "DVD"] |
| WriteProtected | Static | Always true |

**Writable:** `POST .../Actions/VirtualMedia.InsertMedia` downloads ISO and hot-plugs if VM is running. `POST .../Actions/VirtualMedia.EjectMedia` removes and hot-unplugs.

### Chassis Power (`/redfish/v1/Chassis/1/Power`)

**All properties are static.** Power readings (watts), voltage readings (12.0V fixed), power supply info — all hardcoded defaults. No live data from the hypervisor.

### Chassis Thermal (`/redfish/v1/Chassis/1/Thermal`)

**All properties are static.** Temperature (25C fixed), fan speed (5000 RPM fixed) — all hardcoded defaults. No live data from the hypervisor.

### Sensors (`/redfish/v1/Systems/{id}/Sensors`)

**All properties are static.** Fixed readings for temperature, voltage, and current sensors. No live data from the hypervisor.

### Managers (`/redfish/v1/Managers/vbmc`)

| Property | Source | Notes |
|----------|--------|-------|
| DateTime | Static | Current UTC time per request |
| ManagerType | Static | "BMC" |
| FirmwareVersion | Static | Hardcoded |
| All other properties | Static | Hardcoded |

**Read-only.** Single manager resource representing the vbmc-rs instance.

### SessionService, AccountService, EventService

These implement standard Redfish session/account/event management. Data comes from in-memory stores (sessions, accounts, subscriptions) backed by configuration. See [deployment.md](deployment.md) for auth setup and [observability.md](observability.md) for event details.

## Backend capability matrix

| Capability | Cloud-Hypervisor | QEMU | Libvirt |
|-----------|-----------------|------|---------|
| vm_info | Yes | Yes | Yes |
| vm_create | Yes | No (manage-only) | No (use `virsh define`) |
| vm_boot | Yes | Yes | Yes |
| vm_shutdown | Yes | Yes | Yes |
| vm_delete | Yes | Yes (forced) | Yes |
| vm_power_button | Yes | Yes (QMP `system_powerdown`) | Yes |
| vm_reboot | Yes | Yes (QMP `system_reset`) | Yes |
| vm_add_disk | Yes (hot-plug) | No | Yes (`attach-device`) |
| vm_remove_device | Yes (hot-unplug) | No | Yes (`detach-device`) |
| vm_counters | Yes | No | Yes (block/interface stats) |
| vmm_ping | Yes | Yes | Yes (libvirt version) |

**Cloud-Hypervisor** is the only backend that supports full VM lifecycle (create through delete). QEMU and Libvirt expect VMs to be created and configured externally.
