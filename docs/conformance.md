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
| SecureBootEnable | Backend + Persisted | Read from backend if available (KubeVirt EFI spec, Libvirt domain XML), persisted state as fallback. CH: firmware binary swap. QEMU: read-only via qom-get |
| SecureBootCurrentBoot | Static | Derived from enable flag |
| SecureBootMode | Static | "UserMode" |

**Writable:** `PATCH /Systems/{id}/SecureBoot` accepts `SecureBootEnable`. Backend-specific behavior:

| Backend | SecureBoot write | Mechanism |
|---------|-----------------|-----------|
| Cloud-Hypervisor | Yes | Swaps firmware binary (`firmware_path` vs `secure_boot_firmware_path`) |
| QEMU | No (read-only) | Reads current state via `qom-get`; cannot modify running firmware |
| Libvirt | Yes | Rewrites domain XML: Q35 machine type, SMM enabled, pflash loader |
| KubeVirt | Yes | Patches VM spec: `domain.firmware.bootloader.efi.secureBoot` |

Configure the SecureBoot firmware path (Cloud-Hypervisor):

```toml
[defaults]
secure_boot_firmware_path = "/usr/share/OVMF/OVMF_CODE.secboot.fd"
```

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

### CertificateService (`/redfish/v1/CertificateService`)

| Property | Source | Notes |
|----------|--------|-------|
| CertificateLocations | Static | Links collection (currently empty) |
| Actions | Static | GenerateCSR and ReplaceCertificate targets |

**Actions:**

| Action | Privilege | Description |
|--------|-----------|-------------|
| `GenerateCSR` | ConfigureManager | Generates an RSA/EC key pair and returns a PEM-encoded CSR. Accepts CommonName, Organization, AlternativeNames, etc. |
| `ReplaceCertificate` | ConfigureManager | Accepts a PEM certificate string. Writes to the configured `tls_cert` path and hot-reloads the TLS configuration. Only `"PEM"` CertificateType is supported. |

Requires TLS to be configured (`tls_cert` and `tls_key` in `[server]`). A `CertificateReplaced` event is emitted on successful replacement.

### SecurityPolicy (`/redfish/v1/SecurityPolicy`)

| Property | Source | Notes |
|----------|--------|-------|
| SPDM.Enabled | Persisted | `security_policy.spdm_enabled` — gates attestation features |
| TLS.MinimumVersion | Persisted | `security_policy.tls_minimum_version` — enforced in rustls config |

**Writable:** `PATCH /SecurityPolicy` accepts `SPDM.Enabled` (bool) and `TLS.MinimumVersion` (string: `"1.2"` or `"1.3"`). Requires `ConfigureManager` privilege.

The TLS minimum version is enforced at the rustls protocol level. Changing it takes effect on new connections (existing connections are not terminated).

## Backend capability matrix

| Capability | Cloud-Hypervisor | QEMU | Libvirt | KubeVirt | Mockup |
|-----------|-----------------|------|---------|----------|--------|
| vm_info | Yes | Yes | Yes | Yes (VM/VMI spec) | Yes (from JSON) |
| vm_create | Yes | No (manage-only) | No (use `virsh define`) | No (manage-only) | No |
| vm_boot | Yes | Yes | Yes | Yes (start subresource) | Yes (sets PowerState) |
| vm_shutdown | Yes | Yes | Yes | Yes (stop subresource) | Yes (sets PowerState) |
| vm_delete | Yes | Yes (forced) | Yes | Yes (deletes VM object) | No |
| vm_power_button | Yes | Yes (QMP `system_powerdown`) | Yes | Yes (softreboot subresource) | Yes (sets PowerState) |
| vm_reboot | Yes | Yes (QMP `system_reset`) | Yes | Yes (restart subresource) | Yes (sets PowerState) |
| vm_add_disk | Yes (hot-plug) | No | Yes (`attach-device`) | Yes (addvolume subresource) | No |
| vm_remove_device | Yes (hot-unplug) | No | Yes (`detach-device`) | Yes (removevolume subresource) | No |
| vm_counters | Yes | No | Yes (block/interface stats) | No | No |
| vmm_ping | Yes | Yes | Yes (libvirt version) | Yes (KubeVirt API version) | Yes (RedfishVersion) |
| vm_set_secure_boot | Yes (firmware swap) | No (read-only) | Yes (domain XML rewrite) | Yes (VM spec patch) | Yes (patches JSON) |
| vm_serial_console | Yes | No | Yes (libvirt PTY) | No | No |

**Cloud-Hypervisor** is the only backend that supports full VM lifecycle (create through delete). QEMU, Libvirt, and KubeVirt expect VMs to be created and configured externally. The Mockup backend serves DMTF mockup directories with stateful power and PATCH mutations. KubeVirt requires the `kubevirt` feature flag.
