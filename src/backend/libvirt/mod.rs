pub mod xml;

use std::collections::HashMap;
use std::sync::Mutex;

use virt::connect::Connect;
use virt::domain::Domain;
use virt::sys::VIR_DOMAIN_START_PAUSED;

use crate::backend::types as bt;
use crate::backend::{BackendError, VmmBackend};
use crate::config::AppConfig;

pub struct LibvirtBackend {
    connection_uri: String,
    // Persistent connection shared across all operations in this process.
    // Re-opened on error. A shared connection avoids the per-call Connect::open
    // overhead (Unix socket connect + capability negotiation) that accumulates
    // into a connection storm when NICo's site-explorer polls many VMs
    // simultaneously — each vbmc-rs process was opening ~2 new connections per
    // poll cycle, exhausting virtqemud's worker thread pool.
    conn: Mutex<Option<Connect>>,
    domains: HashMap<String, String>, // system_id → domain_name
    /// Once overrides pending the next vm_boot: system_id → (original_first_dev, override_dev).
    /// Cleared after vm_boot applies and restores the XML.  Repopulated from VmState
    /// on startup via init_boot_state() so Once overrides survive vbmc-rs restarts.
    pending_once: Mutex<HashMap<String, (String, String)>>,
    /// Original first-boot device (libvirt dev string, e.g. "hd") before any override
    /// was applied.  Set when the first Continuous or Once override is activated;
    /// cleared and used for XML restore when BootSourceOverrideEnabled=Disabled is set.
    /// Falls back to "hd" if not populated (e.g., after vbmc-rs restart with a
    /// Continuous override already active).
    original_boot_devs: Mutex<HashMap<String, String>>,
}

// SAFETY: virt::connect::Connect and virt::domain::Domain are Send + Sync
// (they wrap a thread-safe C pointer to libvirt's connection handle).
unsafe impl Send for LibvirtBackend {}
unsafe impl Sync for LibvirtBackend {}

impl LibvirtBackend {
    pub fn new(conn: Connect, domains: HashMap<String, String>) -> Self {
        let uri = conn.get_uri().unwrap_or_default();
        Self {
            connection_uri: uri,
            conn: Mutex::new(Some(conn)),
            domains,
            pending_once: Mutex::new(HashMap::new()),
            original_boot_devs: Mutex::new(HashMap::new()),
        }
    }

    /// Reinitialise in-memory boot override state from a persisted VmState entry.
    /// Called at startup for each system after VmState is loaded so that Once
    /// overrides survive vbmc-rs restarts (pending_once is in-memory only).
    ///
    /// For Once: the domain XML was NOT modified (the override is applied
    /// transiently at vm_boot time), so the current first-boot device in the XML
    /// IS the original pre-override device.  We read it here and reconstruct the
    /// pending_once entry.
    ///
    /// For Continuous: the domain XML was already modified to put the override
    /// device first.  We don't need to touch pending_once, but we record "hd" as
    /// the restore target (best-effort fallback) so that a subsequent Disabled
    /// can restore a sane default.
    pub fn init_boot_state(&self, system_id: &str, boot_override: &crate::state::BootOverride) {
        match boot_override.enabled.as_str() {
            "Once" => {
                let Some(target) = &boot_override.target else {
                    return;
                };
                let Some(override_dev) = ipmi_target_to_libvirt_dev(target) else {
                    return;
                };

                // The domain XML was not changed for Once overrides — read it to
                // find the original (currently-stored) first-boot device.
                let Ok(domain) = self.domain_for(system_id) else {
                    return;
                };
                let Ok(domain_xml) = domain.get_xml_desc(0) else {
                    return;
                };
                let original_dev =
                    xml::get_first_boot_device(&domain_xml).unwrap_or_else(|| "hd".to_string());

                tracing::info!(
                    "Restoring pending Once boot override for {system_id}: \
                     original={original_dev} override={override_dev}"
                );
                self.pending_once.lock().unwrap().insert(
                    system_id.to_string(),
                    (original_dev.clone(), override_dev.to_string()),
                );
                self.original_boot_devs
                    .lock()
                    .unwrap()
                    .entry(system_id.to_string())
                    .or_insert(original_dev);
            }
            "Continuous" => {
                // XML is already set to the override dev.  We don't know the true
                // original without additional persistence.  Record "hd" as the
                // restore-on-Disabled fallback so that the common NICo pattern
                // (PXE → provision → Disabled) reverts cleanly to disk-first.
                self.original_boot_devs
                    .lock()
                    .unwrap()
                    .entry(system_id.to_string())
                    .or_insert_with(|| "hd".to_string());
            }
            _ => {
                // Disabled or unknown — nothing to restore.
            }
        }
    }

    /// Look up the domain for system_id, reusing the shared connection.
    ///
    /// Returns just `Domain` — libvirt's C layer refcounts the underlying
    /// virConnect* inside the domain handle, so the Domain stays valid even
    /// after the Mutex is released. This avoids opening a new virConnect for
    /// every Redfish call (the previous per-call Connect::open caused a
    /// connection storm when NICo's site-explorer polled many VMs at once).
    /// The connection is reopened only on error (stale after virtqemud restart).
    fn domain_for(&self, system_id: &str) -> Result<Domain, BackendError> {
        let mut guard = self.conn.lock().unwrap();

        // Try with the cached connection; on failure reopen once and retry.
        let try_lookup = |conn: &Connect| -> Result<Domain, BackendError> {
            if let Some(name) = self.domains.get(system_id) {
                return Domain::lookup_by_name(conn, name).map_err(map_virt_error);
            }
            let domains = conn.list_all_domains(0).map_err(map_virt_error)?;
            tracing::info!("Auto-discover: found {} domains", domains.len());
            for d in &domains {
                if let Ok(name) = d.get_name() {
                    tracing::info!("  domain: {name}");
                }
            }
            match domains.len() {
                0 => Err(BackendError::VmNotFound),
                1 => Ok(domains.into_iter().next().unwrap()),
                _ => Err(BackendError::InvalidState(
                    "multiple domains found; set domain_name in config to disambiguate".to_string(),
                )),
            }
        };

        if let Some(conn) = guard.as_ref()
            && let Ok(domain) = try_lookup(conn)
        {
            return Ok(domain);
        }

        // Connection is stale or absent — reopen.
        let fresh = Connect::open(Some(&self.connection_uri)).map_err(map_virt_error)?;
        let domain = try_lookup(&fresh)?;
        *guard = Some(fresh);
        Ok(domain)
    }

    fn map_domain_state(state: u32) -> bt::VmPowerState {
        match state {
            1 => bt::VmPowerState::On,     // VIR_DOMAIN_RUNNING
            3 => bt::VmPowerState::Paused, // VIR_DOMAIN_PAUSED
            5 => bt::VmPowerState::Off,    // VIR_DOMAIN_SHUTOFF
            6 => bt::VmPowerState::Off,    // VIR_DOMAIN_CRASHED
            _ => bt::VmPowerState::Unknown,
        }
    }

    /// Redefine the domain XML using the shared cached connection.
    fn define_xml(&self, xml: &str) -> Result<(), BackendError> {
        let guard = self.conn.lock().unwrap();
        let conn = guard.as_ref().ok_or_else(|| {
            BackendError::InvalidState("no active libvirt connection".to_string())
        })?;
        Domain::define_xml(conn, xml).map_err(map_virt_error)?;
        Ok(())
    }
}

fn map_virt_error(e: virt::error::Error) -> BackendError {
    BackendError::ApiError(e.message().to_string())
}

fn ipmi_target_to_libvirt_dev(target: &str) -> Option<&'static str> {
    match target {
        "Pxe" => Some("network"),
        "Hdd" => Some("hd"),
        "Cd" => Some("cdrom"),
        "Floppy" => Some("fd"),
        _ => None,
    }
}

impl VmmBackend for LibvirtBackend {
    fn manages_existing_vms(&self) -> bool {
        // libvirt domains are pre-existing (created via virsh define / virt-install).
        // vbmc-rs manages them but does not create them — skip vm_create on power-on.
        true
    }

    fn vm_init_boot_state(&self, system_id: &str, boot_override: &crate::state::BootOverride) {
        self.init_boot_state(system_id, boot_override);
    }

    async fn vm_info(&self, system_id: &str) -> Result<bt::VmInfo, BackendError> {
        let domain = self.domain_for(system_id)?;

        let info = domain.get_info().map_err(map_virt_error)?;
        let power_state = Self::map_domain_state(info.state);

        let domain_xml = domain.get_xml_desc(0).map_err(map_virt_error)?;
        let parsed = xml::parse_domain_xml(&domain_xml);

        Ok(bt::VmInfo {
            power_state,
            cpu_count: info.nr_virt_cpu,
            max_cpu_count: info.nr_virt_cpu,
            cpu_topology: None,
            memory_bytes: parsed.memory_bytes,
            memory_actual_bytes: Some(info.memory * 1024), // memory is in KiB
            secure_boot: parsed.secure_boot,
            disks: parsed.disks,
            nics: parsed.nics,
            pci_devices: parsed.pci_devices,
            uuid: parsed.uuid,
            raw: None,
        })
    }

    async fn vm_create(
        &self,
        _system_id: &str,
        _config: bt::VmCreateConfig,
    ) -> Result<(), BackendError> {
        Err(BackendError::NotSupported(
            "vm_create via VmCreateConfig is not supported for libvirt; use virsh define"
                .to_string(),
        ))
    }

    async fn vm_boot(&self, system_id: &str) -> Result<(), BackendError> {
        let pending = self.pending_once.lock().unwrap().get(system_id).cloned();

        if let Some((original_dev, override_dev)) = pending {
            let domain = self.domain_for(system_id)?;
            let domain_xml = domain.get_xml_desc(0).map_err(map_virt_error)?;
            let already_first =
                xml::get_first_boot_device(&domain_xml).as_deref() == Some(override_dev.as_str());
            let override_xml = xml::set_boot_order_xml(&domain_xml, &override_dev);
            if !already_first && override_xml == domain_xml {
                // No bootable device of the requested kind — defining the
                // unchanged XML would silently boot the wrong device.
                self.pending_once.lock().unwrap().remove(system_id);
                return Err(BackendError::NotSupported(format!(
                    "no bootable device of kind '{override_dev}' in domain XML"
                )));
            }
            if !already_first {
                self.define_xml(&override_xml)?;
            }

            // VIR_DOMAIN_START_PAUSED: QEMU process starts in microseconds (no
            // UEFI initialisation wait). resume() unpauses it; UEFI initialises
            // asynchronously so this call returns in ~100 ms instead of 10–30 s.
            // This frees the libvirt dispatch thread immediately, preventing the
            // worker-pool exhaustion that deadlocks virtqemud under concurrent boots.
            let create_result = domain
                .create_with_flags(VIR_DOMAIN_START_PAUSED)
                .map_err(map_virt_error)
                .and_then(|_| domain.resume().map_err(map_virt_error));

            // Restore original boot order regardless of whether create succeeded.
            if !already_first {
                let domain = self.domain_for(system_id)?;
                let domain_xml = domain.get_xml_desc(0).map_err(map_virt_error)?;
                let restored_xml = xml::set_boot_order_xml(&domain_xml, &original_dev);
                self.define_xml(&restored_xml)?;
            }
            self.pending_once.lock().unwrap().remove(system_id);

            create_result.map(|_| ())?;
        } else {
            let domain = self.domain_for(system_id)?;
            domain
                .create_with_flags(VIR_DOMAIN_START_PAUSED)
                .map_err(map_virt_error)?;
            domain.resume().map_err(map_virt_error)?;
        }
        Ok(())
    }

    // vm_get_boot_override is intentionally not overridden for the libvirt backend.
    // The VmState on disk (updated by patch_system and clear_once_boot_override) is
    // the authoritative source for the Redfish BootSourceOverrideEnabled/Target fields.
    // Returning None here causes get_system to fall back to vm_state.boot_override.
    // The libvirt backend is responsible only for translating Redfish boot overrides
    // into domain XML changes via vm_set_boot_override.

    async fn vm_set_boot_override(
        &self,
        system_id: &str,
        info: &bt::BootOverrideInfo,
    ) -> Result<(), BackendError> {
        if info.target == "None" || info.enabled == "Disabled" {
            // Clear any pending Once override.
            self.pending_once.lock().unwrap().remove(system_id);

            // Restore domain XML to the original boot order.  Use the saved
            // original device if available; fall back to "hd" (conventional
            // disk-first default used for NICo-managed domains).
            let original_dev = self
                .original_boot_devs
                .lock()
                .unwrap()
                .remove(system_id)
                .unwrap_or_else(|| "hd".to_string());

            let Ok(domain) = self.domain_for(system_id) else {
                return Ok(());
            };
            let Ok(domain_xml) = domain.get_xml_desc(0) else {
                return Ok(());
            };
            let current_dev = xml::get_first_boot_device(&domain_xml).unwrap_or_default();
            if current_dev != original_dev {
                tracing::info!(
                    "Restoring boot order for {system_id}: {current_dev} → {original_dev}"
                );
                let restored_xml = xml::set_boot_order_xml(&domain_xml, &original_dev);
                if restored_xml != domain_xml {
                    self.define_xml(&restored_xml)?;
                }
            }
            return Ok(());
        }

        let override_dev = ipmi_target_to_libvirt_dev(&info.target).ok_or_else(|| {
            BackendError::NotSupported(format!("unknown boot target: {}", info.target))
        })?;

        if info.enabled == "Continuous" {
            let domain = self.domain_for(system_id)?;
            let domain_xml = domain.get_xml_desc(0).map_err(map_virt_error)?;
            let current_first_dev =
                xml::get_first_boot_device(&domain_xml).unwrap_or_else(|| "hd".to_string());

            // Save original boot dev before applying the first override.
            // Subsequent overrides (e.g. Once after Continuous) keep the
            // stored original so that Disabled can always restore correctly.
            self.original_boot_devs
                .lock()
                .unwrap()
                .entry(system_id.to_string())
                .or_insert_with(|| current_first_dev.clone());

            let new_xml = xml::set_boot_order_xml(&domain_xml, override_dev);
            if new_xml == domain_xml && current_first_dev != override_dev {
                // No bootable device of the requested kind — defining the
                // unchanged XML would silently keep the current boot order.
                return Err(BackendError::NotSupported(format!(
                    "no bootable device of kind '{override_dev}' in domain XML"
                )));
            }
            self.define_xml(&new_xml)?;
            // Remove any pending Once entry — Continuous overrides the XML directly.
            self.pending_once.lock().unwrap().remove(system_id);
        } else {
            // Once: do NOT modify domain XML — apply the override transiently in
            // vm_boot and restore the original XML after the domain starts.
            let domain = self.domain_for(system_id)?;
            let domain_xml = domain.get_xml_desc(0).map_err(map_virt_error)?;
            let current_first_dev =
                xml::get_first_boot_device(&domain_xml).unwrap_or_else(|| "hd".to_string());

            // Save original boot dev (only on the first override; keep any
            // existing stored original so Disabled can always restore it).
            self.original_boot_devs
                .lock()
                .unwrap()
                .entry(system_id.to_string())
                .or_insert_with(|| current_first_dev.clone());

            // Validate the requested kind exists now rather than failing (or
            // silently no-op'ing) at the next vm_boot.
            let probe_xml = xml::set_boot_order_xml(&domain_xml, override_dev);
            if probe_xml == domain_xml && current_first_dev != override_dev {
                return Err(BackendError::NotSupported(format!(
                    "no bootable device of kind '{override_dev}' in domain XML"
                )));
            }

            self.pending_once.lock().unwrap().insert(
                system_id.to_string(),
                (current_first_dev, override_dev.to_string()),
            );
        }
        Ok(())
    }

    async fn vm_pause(&self, system_id: &str) -> Result<(), BackendError> {
        let domain = self.domain_for(system_id)?;
        domain.suspend().map_err(map_virt_error)?;
        Ok(())
    }

    async fn vm_resume(&self, system_id: &str) -> Result<(), BackendError> {
        let domain = self.domain_for(system_id)?;
        domain.resume().map_err(map_virt_error)?;
        Ok(())
    }

    async fn vm_shutdown(&self, system_id: &str) -> Result<(), BackendError> {
        let domain = self.domain_for(system_id)?;
        domain.shutdown().map_err(map_virt_error)?;
        Ok(())
    }

    async fn vm_force_off(&self, system_id: &str) -> Result<(), BackendError> {
        let domain = self.domain_for(system_id)?;
        domain.destroy().map_err(map_virt_error)?;
        Ok(())
    }

    async fn vm_delete(&self, system_id: &str) -> Result<(), BackendError> {
        let domain = self.domain_for(system_id)?;
        domain.destroy().map_err(map_virt_error)?;
        Ok(())
    }

    async fn vm_power_button(&self, system_id: &str) -> Result<(), BackendError> {
        let domain = self.domain_for(system_id)?;
        domain.shutdown().map_err(map_virt_error)?;
        Ok(())
    }

    async fn vm_reboot(&self, system_id: &str) -> Result<(), BackendError> {
        let domain = self.domain_for(system_id)?;
        domain.reboot(0).map_err(map_virt_error)?;
        Ok(())
    }

    async fn vm_force_reboot(&self, system_id: &str) -> Result<(), BackendError> {
        // virDomainReset: hardware-level reset with no ACPI/OS involvement.
        // Unlike virDomainReboot, this never blocks on guest ACPI processing,
        // so it cannot deadlock virtqemud even when the guest is in firmware.
        let domain = self.domain_for(system_id)?;
        domain.reset().map_err(map_virt_error)?;
        Ok(())
    }

    async fn vm_add_disk(
        &self,
        system_id: &str,
        disk: bt::DiskCreateConfig,
    ) -> Result<(), BackendError> {
        let domain = self.domain_for(system_id)?;

        let source = disk.path.unwrap_or_default();
        let target = disk.id.unwrap_or_else(|| "vdz".to_string());
        let readonly = if disk.readonly { "<readonly/>" } else { "" };

        // Use device='cdrom' on a SATA bus for read-only ISO images so that
        // OVMF/UEFI recognises the attached medium as a bootable CD-ROM.
        // Regular writable disks stay as virtio block devices.
        let xml = if disk.readonly {
            format!(
                "<disk type='file' device='cdrom'>\
                 <driver name='qemu' type='raw'/>\
                 <source file='{source}'/>\
                 <target dev='{target}' bus='sata'/>\
                 <readonly/>\
                 </disk>"
            )
        } else {
            format!(
                "<disk type='file' device='disk'>\
                 <driver name='qemu' type='raw'/>\
                 <source file='{source}'/>\
                 <target dev='{target}' bus='virtio'/>\
                 {readonly}\
                 </disk>"
            )
        };

        domain.attach_device(&xml).map_err(map_virt_error)?;
        Ok(())
    }

    async fn vm_remove_device(&self, system_id: &str, device_id: &str) -> Result<(), BackendError> {
        let domain = self.domain_for(system_id)?;

        // Try cdrom/sata first (VirtualMedia); fall back to disk/virtio.
        let xml_cdrom = format!(
            "<disk type='file' device='cdrom'>\
             <target dev='{device_id}' bus='sata'/>\
             </disk>"
        );
        if domain.detach_device(&xml_cdrom).is_err() {
            let xml_disk = format!(
                "<disk type='file' device='disk'>\
                 <target dev='{device_id}' bus='virtio'/>\
                 </disk>"
            );
            domain.detach_device(&xml_disk).map_err(map_virt_error)?;
        }
        Ok(())
    }

    async fn vmm_ping(&self, _system_id: &str) -> Result<bt::VmmPingResponse, BackendError> {
        let guard = self.conn.lock().unwrap();
        let conn = guard.as_ref().ok_or_else(|| {
            BackendError::InvalidState("no active libvirt connection".to_string())
        })?;
        let ver = conn.get_lib_version().map_err(map_virt_error)?;
        let major = ver / 1_000_000;
        let minor = (ver % 1_000_000) / 1_000;
        let release = ver % 1_000;
        Ok(bt::VmmPingResponse {
            version: Some(format!("{major}.{minor}.{release}")),
            pid: None,
        })
    }

    async fn vm_counters(&self, system_id: &str) -> Result<bt::VmCounters, BackendError> {
        let domain = self.domain_for(system_id)?;

        // Get domain XML to find disk targets and NIC interface names
        let domain_xml = domain.get_xml_desc(0).map_err(map_virt_error)?;
        let parsed = xml::parse_domain_xml(&domain_xml);

        let mut counters = bt::VmCounters::default();

        // Sum block stats across all disks
        for disk in &parsed.disks {
            if let Ok(stats) = domain.get_block_stats(&disk.id) {
                if stats.rd_bytes >= 0 {
                    counters.block_read_bytes += stats.rd_bytes as u64;
                }
                if stats.wr_bytes >= 0 {
                    counters.block_write_bytes += stats.wr_bytes as u64;
                }
                if stats.rd_req >= 0 {
                    counters.block_read_ops += stats.rd_req as u64;
                }
                if stats.wr_req >= 0 {
                    counters.block_write_ops += stats.wr_req as u64;
                }
            }
        }

        // Sum interface stats across all NICs
        for nic in &parsed.nics {
            // Use the target device name (e.g. "vnet0") if available,
            // fall back to MAC address
            let iface = match &nic.tap {
                Some(dev) => dev.as_str(),
                None => continue,
            };
            if let Ok(stats) = domain.interface_stats(iface) {
                if stats.rx_bytes >= 0 {
                    counters.net_rx_bytes += stats.rx_bytes as u64;
                }
                if stats.tx_bytes >= 0 {
                    counters.net_tx_bytes += stats.tx_bytes as u64;
                }
                if stats.rx_packets >= 0 {
                    counters.net_rx_frames += stats.rx_packets as u64;
                }
                if stats.tx_packets >= 0 {
                    counters.net_tx_frames += stats.tx_packets as u64;
                }
            }
        }

        Ok(counters)
    }

    async fn vm_set_secure_boot(&self, system_id: &str, enabled: bool) -> Result<(), BackendError> {
        let domain = self.domain_for(system_id)?;
        let domain_xml = domain.get_xml_desc(0).map_err(map_virt_error)?;

        let sb_config = if enabled {
            Some(xml::SecureBootConfig {
                firmware_path: "/usr/share/OVMF/OVMF_CODE.secboot.fd".to_string(),
                nvram_template: Some("/usr/share/OVMF/OVMF_VARS.secboot.fd".to_string()),
            })
        } else {
            None
        };

        let new_xml = xml::set_secure_boot_xml(&domain_xml, enabled, sb_config.as_ref())
            .map_err(BackendError::NotSupported)?;

        self.define_xml(&new_xml)?;
        Ok(())
    }

    async fn vm_serial_console(
        &self,
        system_id: &str,
    ) -> Result<bt::SerialConsoleInfo, BackendError> {
        let domain = self.domain_for(system_id)?;
        let domain_xml = domain.get_xml_desc(0).map_err(map_virt_error)?;
        let pty_path = xml::parse_console_pty(&domain_xml);
        Ok(bt::SerialConsoleInfo {
            pty_path,
            websocket_url: None,
        })
    }

    async fn vm_insert_iso(
        &self,
        _system_id: &str,
        _image_url: &str,
        _device_id: &str,
    ) -> Result<(), BackendError> {
        Err(BackendError::NotSupported("use download path".to_string()))
    }

    async fn vm_eject_iso(&self, _system_id: &str, _device_id: &str) -> Result<(), BackendError> {
        Err(BackendError::NotSupported(
            "use vm_remove_device".to_string(),
        ))
    }
}

pub fn build_backend(config: &AppConfig) -> Result<super::Backend, BackendError> {
    let uri = config
        .systems
        .values()
        .find_map(|s| s.connection_uri.clone())
        .unwrap_or_else(|| "qemu:///system".to_string());

    let conn = Connect::open(Some(&uri))
        .map_err(|e| BackendError::ConnectionFailed(format!("libvirt: {}", e.message())))?;

    let domains: HashMap<String, String> = config
        .systems
        .iter()
        .filter_map(|(id, sys)| sys.domain_name.clone().map(|name| (id.clone(), name)))
        .collect();

    Ok(super::Backend::Libvirt(LibvirtBackend::new(conn, domains)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_domain_state() {
        assert_eq!(LibvirtBackend::map_domain_state(1), bt::VmPowerState::On);
        assert_eq!(
            LibvirtBackend::map_domain_state(3),
            bt::VmPowerState::Paused
        );
        assert_eq!(LibvirtBackend::map_domain_state(5), bt::VmPowerState::Off);
        assert_eq!(LibvirtBackend::map_domain_state(6), bt::VmPowerState::Off);
        assert_eq!(
            LibvirtBackend::map_domain_state(0),
            bt::VmPowerState::Unknown
        );
        assert_eq!(
            LibvirtBackend::map_domain_state(99),
            bt::VmPowerState::Unknown
        );
    }
}

#[cfg(test)]
mod coverage_tests {
    use super::*;

    #[test]
    fn test_map_domain_state_all_states() {
        // VIR_DOMAIN_NOSTATE = 0
        assert_eq!(
            LibvirtBackend::map_domain_state(0),
            bt::VmPowerState::Unknown
        );
        // VIR_DOMAIN_RUNNING = 1
        assert_eq!(LibvirtBackend::map_domain_state(1), bt::VmPowerState::On);
        // VIR_DOMAIN_BLOCKED = 2
        assert_eq!(
            LibvirtBackend::map_domain_state(2),
            bt::VmPowerState::Unknown
        );
        // VIR_DOMAIN_PAUSED = 3
        assert_eq!(
            LibvirtBackend::map_domain_state(3),
            bt::VmPowerState::Paused
        );
        // VIR_DOMAIN_SHUTDOWN = 4
        assert_eq!(
            LibvirtBackend::map_domain_state(4),
            bt::VmPowerState::Unknown
        );
        // VIR_DOMAIN_SHUTOFF = 5
        assert_eq!(LibvirtBackend::map_domain_state(5), bt::VmPowerState::Off);
        // VIR_DOMAIN_CRASHED = 6
        assert_eq!(LibvirtBackend::map_domain_state(6), bt::VmPowerState::Off);
        // VIR_DOMAIN_PMSUSPENDED = 7
        assert_eq!(
            LibvirtBackend::map_domain_state(7),
            bt::VmPowerState::Unknown
        );
    }

    #[test]
    fn test_libvirt_backend_new() {
        // Test that we can construct a LibvirtBackend (without a real connection)
        // This tests the struct and new() method are properly defined
        // Note: We cannot test with a real connection in unit tests
        // We can't create a real Connect without connecting to libvirt,
        // so we just verify the API exists by checking the function signature compiles
        fn _test_signature(conn: Connect, domains: HashMap<String, String>) -> LibvirtBackend {
            LibvirtBackend::new(conn, domains)
        }
    }
}
