pub mod xml;

use std::collections::HashMap;

use tokio::process::Command;

use crate::backend::types as bt;
use crate::backend::{BackendError, VmmBackend};
use crate::config::AppConfig;

pub struct LibvirtBackend {
    connection_uri: String,
    domains: HashMap<String, String>, // system_id → domain_name
}

impl LibvirtBackend {
    pub fn new(connection_uri: String, domains: HashMap<String, String>) -> Self {
        Self {
            connection_uri,
            domains,
        }
    }

    fn domain_for(&self, system_id: &str) -> Result<&str, BackendError> {
        self.domains
            .get(system_id)
            .map(|s| s.as_str())
            .ok_or(BackendError::VmNotFound)
    }

    async fn virsh(&self, args: &[&str]) -> Result<String, BackendError> {
        let mut cmd = Command::new("virsh");
        cmd.arg("--connect").arg(&self.connection_uri);
        for arg in args {
            cmd.arg(arg);
        }

        let output = cmd.output().await.map_err(|e| {
            BackendError::ConnectionFailed(format!("Failed to execute virsh: {e}"))
        })?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("failed to connect")
                || stderr.contains("unable to connect")
            {
                Err(BackendError::VmmNotRunning)
            } else {
                Err(BackendError::ApiError(format!(
                    "virsh {} failed: {}",
                    args.join(" "),
                    stderr
                )))
            }
        }
    }

    fn parse_dominfo_state(output: &str) -> bt::VmPowerState {
        for line in output.lines() {
            if let Some(state) = line.strip_prefix("State:") {
                let state = state.trim();
                return match state {
                    "running" => bt::VmPowerState::On,
                    "paused" => bt::VmPowerState::Paused,
                    "shut off" | "crashed" => bt::VmPowerState::Off,
                    _ => bt::VmPowerState::Unknown,
                };
            }
        }
        bt::VmPowerState::Unknown
    }
}

impl VmmBackend for LibvirtBackend {
    async fn vm_info(&self, system_id: &str) -> Result<bt::VmInfo, BackendError> {
        let domain = self.domain_for(system_id)?;

        // Get domain info for power state
        let dominfo = self.virsh(&["dominfo", domain]).await?;
        let power_state = Self::parse_dominfo_state(&dominfo);

        // Get domain XML for hardware details
        let domain_xml = self.virsh(&["dumpxml", domain]).await?;
        let parsed = xml::parse_domain_xml(&domain_xml);

        Ok(bt::VmInfo {
            power_state,
            cpu_count: parsed.vcpu_count,
            max_cpu_count: parsed.vcpu_count,
            cpu_topology: None,
            memory_bytes: parsed.memory_bytes,
            memory_actual_bytes: Some(parsed.memory_bytes),
            disks: parsed.disks,
            nics: parsed.nics,
            pci_devices: parsed.pci_devices,
            raw: None,
        })
    }

    async fn vm_create(
        &self,
        _system_id: &str,
        _config: bt::VmCreateConfig,
    ) -> Result<(), BackendError> {
        Err(BackendError::NotSupported(
            "vm_create via VmCreateConfig is not supported for libvirt; use virsh define".to_string(),
        ))
    }

    async fn vm_boot(&self, system_id: &str) -> Result<(), BackendError> {
        let domain = self.domain_for(system_id)?;
        self.virsh(&["start", domain]).await?;
        Ok(())
    }

    async fn vm_shutdown(&self, system_id: &str) -> Result<(), BackendError> {
        let domain = self.domain_for(system_id)?;
        self.virsh(&["shutdown", domain]).await?;
        Ok(())
    }

    async fn vm_delete(&self, system_id: &str) -> Result<(), BackendError> {
        let domain = self.domain_for(system_id)?;
        self.virsh(&["destroy", domain]).await?;
        Ok(())
    }

    async fn vm_power_button(&self, system_id: &str) -> Result<(), BackendError> {
        let domain = self.domain_for(system_id)?;
        self.virsh(&["shutdown", domain]).await?;
        Ok(())
    }

    async fn vm_reboot(&self, system_id: &str) -> Result<(), BackendError> {
        let domain = self.domain_for(system_id)?;
        self.virsh(&["reboot", domain]).await?;
        Ok(())
    }

    async fn vm_add_disk(
        &self,
        system_id: &str,
        disk: bt::DiskCreateConfig,
    ) -> Result<(), BackendError> {
        let domain = self.domain_for(system_id)?;

        // Build XML fragment for attach-device
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

        // Write XML to temp file
        let tmp = std::env::temp_dir().join(format!("vbmc-attach-{}.xml", uuid::Uuid::new_v4()));
        std::fs::write(&tmp, &xml)
            .map_err(|e| BackendError::ApiError(format!("Failed to write temp XML: {e}")))?;

        let tmp_str = tmp.to_string_lossy().to_string();
        let result = self.virsh(&["attach-device", domain, &tmp_str]).await;

        // Clean up temp file
        let _ = std::fs::remove_file(&tmp);

        result?;
        Ok(())
    }

    async fn vm_remove_device(
        &self,
        system_id: &str,
        device_id: &str,
    ) -> Result<(), BackendError> {
        let domain = self.domain_for(system_id)?;

        // Build XML fragment for detach-device
        let xml = format!(
            "<disk type='file' device='disk'>\
             <target dev='{device_id}' bus='virtio'/>\
             </disk>"
        );

        let tmp = std::env::temp_dir().join(format!("vbmc-detach-{}.xml", uuid::Uuid::new_v4()));
        std::fs::write(&tmp, &xml)
            .map_err(|e| BackendError::ApiError(format!("Failed to write temp XML: {e}")))?;

        let tmp_str = tmp.to_string_lossy().to_string();
        let result = self.virsh(&["detach-device", domain, &tmp_str]).await;

        let _ = std::fs::remove_file(&tmp);

        result?;
        Ok(())
    }

    async fn vmm_ping(&self, _system_id: &str) -> Result<bt::VmmPingResponse, BackendError> {
        let output = self.virsh(&["version"]).await?;
        let version = output.lines().next().map(|l| l.to_string());
        Ok(bt::VmmPingResponse { version, pid: None })
    }

    async fn vm_counters(&self, _system_id: &str) -> Result<serde_json::Value, BackendError> {
        Err(BackendError::NotSupported(
            "vm_counters not supported for libvirt backend".to_string(),
        ))
    }
}

pub fn build_backend(config: &AppConfig) -> super::Backend {
    let uri = config
        .systems
        .values()
        .find_map(|s| s.connection_uri.clone())
        .unwrap_or_else(|| "qemu:///system".to_string());

    let domains: HashMap<String, String> = config
        .systems
        .iter()
        .map(|(id, sys)| {
            let domain_name = sys
                .domain_name
                .clone()
                .unwrap_or_else(|| id.clone());
            (id.clone(), domain_name)
        })
        .collect();

    super::Backend::Libvirt(LibvirtBackend::new(uri, domains))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_dominfo_state_running() {
        let output = "Id:             3\nName:           test-vm\nState:          running\n";
        assert_eq!(LibvirtBackend::parse_dominfo_state(output), bt::VmPowerState::On);
    }

    #[test]
    fn test_parse_dominfo_state_shut_off() {
        let output = "Id:             -\nName:           test-vm\nState:          shut off\n";
        assert_eq!(LibvirtBackend::parse_dominfo_state(output), bt::VmPowerState::Off);
    }

    #[test]
    fn test_parse_dominfo_state_paused() {
        let output = "State:          paused\n";
        assert_eq!(LibvirtBackend::parse_dominfo_state(output), bt::VmPowerState::Paused);
    }

    #[test]
    fn test_parse_dominfo_state_crashed() {
        let output = "State:          crashed\n";
        assert_eq!(LibvirtBackend::parse_dominfo_state(output), bt::VmPowerState::Off);
    }

    #[test]
    fn test_parse_dominfo_state_unknown() {
        let output = "State:          pmsuspended\n";
        assert_eq!(LibvirtBackend::parse_dominfo_state(output), bt::VmPowerState::Unknown);
    }

    #[test]
    fn test_parse_dominfo_state_missing() {
        let output = "Id:             3\nName:           test-vm\n";
        assert_eq!(LibvirtBackend::parse_dominfo_state(output), bt::VmPowerState::Unknown);
    }

    #[test]
    fn test_parse_dominfo_state_empty() {
        assert_eq!(LibvirtBackend::parse_dominfo_state(""), bt::VmPowerState::Unknown);
    }

    #[test]
    fn test_domain_for_missing() {
        let backend = LibvirtBackend::new("qemu:///system".to_string(), HashMap::new());
        let err = backend.domain_for("nonexistent").unwrap_err();
        assert!(matches!(err, BackendError::VmNotFound));
    }

    #[test]
    fn test_domain_for_existing() {
        let mut domains = HashMap::new();
        domains.insert("vm1".to_string(), "my-domain".to_string());
        let backend = LibvirtBackend::new("qemu:///system".to_string(), domains);
        assert_eq!(backend.domain_for("vm1").unwrap(), "my-domain");
    }
}
