pub mod xml;

use std::collections::HashMap;

use virt::connect::Connect;
use virt::domain::Domain;

use crate::backend::types as bt;
use crate::backend::{BackendError, VmmBackend};
use crate::config::AppConfig;

pub struct LibvirtBackend {
    conn: Connect,
    domains: HashMap<String, String>, // system_id → domain_name
}

// SAFETY: virt::connect::Connect and virt::domain::Domain are Send + Sync
// (they wrap a thread-safe C pointer to libvirt's connection handle).
unsafe impl Send for LibvirtBackend {}
unsafe impl Sync for LibvirtBackend {}

impl LibvirtBackend {
    pub fn new(conn: Connect, domains: HashMap<String, String>) -> Self {
        Self { conn, domains }
    }

    fn domain_for(&self, system_id: &str) -> Result<Domain, BackendError> {
        if let Some(name) = self.domains.get(system_id) {
            return Domain::lookup_by_name(&self.conn, name).map_err(map_virt_error);
        }
        // Auto-discover: if no domain_name is configured, use the only domain present
        let domains = self.conn.list_all_domains(0).map_err(map_virt_error)?;
        match domains.len() {
            0 => Err(BackendError::VmNotFound),
            1 => Ok(domains.into_iter().next().unwrap()),
            _ => Err(BackendError::InvalidState(
                "multiple domains found; set domain_name in config to disambiguate".to_string(),
            )),
        }
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
}

fn map_virt_error(e: virt::error::Error) -> BackendError {
    BackendError::ApiError(e.message().to_string())
}

impl VmmBackend for LibvirtBackend {
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
            uuid: None,
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
        let domain = self.domain_for(system_id)?;
        domain.create().map_err(map_virt_error)?;
        Ok(())
    }

    async fn vm_shutdown(&self, system_id: &str) -> Result<(), BackendError> {
        let domain = self.domain_for(system_id)?;
        domain.shutdown().map_err(map_virt_error)?;
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

    async fn vm_add_disk(
        &self,
        system_id: &str,
        disk: bt::DiskCreateConfig,
    ) -> Result<(), BackendError> {
        let domain = self.domain_for(system_id)?;

        let source = disk.path.unwrap_or_default();
        let target = disk.id.unwrap_or_else(|| "vdz".to_string());
        let readonly = if disk.readonly { "<readonly/>" } else { "" };

        let xml = format!(
            "<disk type='file' device='disk'>\
             <driver name='qemu' type='raw'/>\
             <source file='{source}'/>\
             <target dev='{target}' bus='virtio'/>\
             {readonly}\
             </disk>"
        );

        domain.attach_device(&xml).map_err(map_virt_error)?;
        Ok(())
    }

    async fn vm_remove_device(&self, system_id: &str, device_id: &str) -> Result<(), BackendError> {
        let domain = self.domain_for(system_id)?;

        let xml = format!(
            "<disk type='file' device='disk'>\
             <target dev='{device_id}' bus='virtio'/>\
             </disk>"
        );

        domain.detach_device(&xml).map_err(map_virt_error)?;
        Ok(())
    }

    async fn vmm_ping(&self, _system_id: &str) -> Result<bt::VmmPingResponse, BackendError> {
        let ver = self.conn.get_lib_version().map_err(map_virt_error)?;
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

        Domain::define_xml(&self.conn, &new_xml).map_err(map_virt_error)?;
        Ok(())
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
        .map(|(id, sys)| {
            let domain_name = sys.domain_name.clone().unwrap_or_else(|| id.clone());
            (id.clone(), domain_name)
        })
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
