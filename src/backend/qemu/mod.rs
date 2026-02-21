pub mod client;
pub mod types;

use std::collections::HashMap;
use std::path::PathBuf;

use crate::backend::types as bt;
use crate::backend::{BackendError, VmmBackend};
use crate::config::AppConfig;
use client::QmpClient;

pub struct QemuBackend {
    sockets: HashMap<String, PathBuf>,
}

impl QemuBackend {
    pub fn new(sockets: HashMap<String, PathBuf>) -> Self {
        Self { sockets }
    }

    fn client_for(&self, system_id: &str) -> Result<QmpClient, BackendError> {
        let path = self
            .sockets
            .get(system_id)
            .ok_or(BackendError::VmNotFound)?;
        Ok(QmpClient::new(path))
    }
}

fn qemu_status_to_power_state(status: &str) -> bt::VmPowerState {
    match status {
        "running" => bt::VmPowerState::On,
        "paused" | "suspended" => bt::VmPowerState::Paused,
        _ => bt::VmPowerState::Off,
    }
}

impl VmmBackend for QemuBackend {
    async fn vm_info(&self, system_id: &str) -> Result<bt::VmInfo, BackendError> {
        let client = self.client_for(system_id)?;

        // Query status
        let status_val = client.execute("query-status", None).await?;
        let status: types::QmpStatus = serde_json::from_value(status_val)
            .map_err(|e| BackendError::ApiError(format!("Failed to parse status: {e}")))?;

        let power_state = qemu_status_to_power_state(&status.status);

        // Query CPUs
        let cpus_val = client.execute("query-cpus-fast", None).await?;
        let cpus: Vec<types::QmpCpu> = serde_json::from_value(cpus_val).unwrap_or_default();
        let cpu_count = cpus.len() as u32;

        // Query memory
        let mem_val = client.execute("query-memory-size-summary", None).await?;
        let mem: types::QmpMemorySizeSummary = serde_json::from_value(mem_val)
            .map_err(|e| BackendError::ApiError(format!("Failed to parse memory: {e}")))?;
        let memory_bytes = mem.base_memory + mem.plugged_memory;

        // Query block devices
        let block_val = client.execute("query-block", None).await?;
        let blocks: Vec<types::QmpBlockDevice> =
            serde_json::from_value(block_val).unwrap_or_default();
        let disks: Vec<bt::DiskInfo> = blocks
            .iter()
            .enumerate()
            .filter_map(|(i, b)| {
                b.inserted.as_ref().map(|ins| bt::DiskInfo {
                    id: if b.device.is_empty() {
                        format!("disk{i}")
                    } else {
                        b.device.clone()
                    },
                    path: Some(ins.file.clone()),
                    capacity_bytes: std::fs::metadata(&ins.file).ok().map(|m| m.len()),
                    readonly: ins.ro,
                    protocol: if ins.drv.contains("nvme") {
                        bt::DiskProtocol::NVMe
                    } else {
                        bt::DiskProtocol::Virtio
                    },
                    media_type: bt::DiskMediaType::Virtual,
                })
            })
            .collect();

        // Query PCI (best effort)
        let pci_devices = if let Ok(pci_val) = client.execute("query-pci", None).await {
            let buses: Vec<types::QmpPciBus> =
                serde_json::from_value(pci_val).unwrap_or_default();
            buses
                .into_iter()
                .flat_map(|bus| {
                    bus.devices.unwrap_or_default().into_iter().map(move |dev| {
                        bt::PciDeviceInfo {
                            bdf: format!(
                                "0000:{:02x}:{:02x}.{}",
                                bus.bus, dev.slot, dev.function
                            ),
                            vendor_id: Some(format!("0x{:04x}", dev.id.vendor)),
                            device_id: Some(format!("0x{:04x}", dev.id.device)),
                            class_code: Some(format!("0x{:06x}", dev.class_info.class)),
                            device_name: if dev.qdev_id.is_empty() {
                                None
                            } else {
                                Some(dev.qdev_id)
                            },
                            is_passthrough: false,
                            functions: vec![bt::PciFunctionInfo {
                                function_id: dev.function as u8,
                                class_code: Some(format!("0x{:06x}", dev.class_info.class)),
                                device_id: Some(format!("0x{:04x}", dev.id.device)),
                                vendor_id: Some(format!("0x{:04x}", dev.id.vendor)),
                            }],
                        }
                    })
                })
                .collect()
        } else {
            Vec::new()
        };

        Ok(bt::VmInfo {
            power_state,
            cpu_count,
            max_cpu_count: cpu_count,
            cpu_topology: None,
            memory_bytes,
            memory_actual_bytes: Some(memory_bytes),
            disks,
            nics: Vec::new(), // QEMU NIC enumeration requires device model knowledge
            pci_devices,
            raw: None,
        })
    }

    async fn vm_create(
        &self,
        _system_id: &str,
        _config: bt::VmCreateConfig,
    ) -> Result<(), BackendError> {
        Err(BackendError::NotSupported(
            "QEMU backend is manage-only; vm_create is not supported".to_string(),
        ))
    }

    async fn vm_boot(&self, system_id: &str) -> Result<(), BackendError> {
        let client = self.client_for(system_id)?;
        client.execute("cont", None).await?;
        Ok(())
    }

    async fn vm_shutdown(&self, system_id: &str) -> Result<(), BackendError> {
        let client = self.client_for(system_id)?;
        client.execute("system_powerdown", None).await?;
        Ok(())
    }

    async fn vm_delete(&self, system_id: &str) -> Result<(), BackendError> {
        let client = self.client_for(system_id)?;
        client.execute("quit", None).await?;
        Ok(())
    }

    async fn vm_power_button(&self, system_id: &str) -> Result<(), BackendError> {
        let client = self.client_for(system_id)?;
        client.execute("system_powerdown", None).await?;
        Ok(())
    }

    async fn vm_reboot(&self, system_id: &str) -> Result<(), BackendError> {
        let client = self.client_for(system_id)?;
        client.execute("system_reset", None).await?;
        Ok(())
    }

    async fn vm_add_disk(
        &self,
        system_id: &str,
        disk: bt::DiskCreateConfig,
    ) -> Result<(), BackendError> {
        let client = self.client_for(system_id)?;

        let node_name = disk
            .id
            .clone()
            .unwrap_or_else(|| format!("drive-{}", uuid::Uuid::new_v4()));

        // Add block device
        let blockdev_args = serde_json::json!({
            "driver": "file",
            "node-name": format!("{node_name}-file"),
            "filename": disk.path.unwrap_or_default(),
            "read-only": disk.readonly,
        });
        client
            .execute("blockdev-add", Some(blockdev_args))
            .await?;

        // Add format layer
        let format_args = serde_json::json!({
            "driver": "raw",
            "node-name": node_name,
            "file": format!("{node_name}-file"),
            "read-only": disk.readonly,
        });
        client
            .execute("blockdev-add", Some(format_args))
            .await?;

        Ok(())
    }

    async fn vm_remove_device(
        &self,
        system_id: &str,
        device_id: &str,
    ) -> Result<(), BackendError> {
        let client = self.client_for(system_id)?;
        let args = serde_json::json!({"id": device_id});
        client.execute("device_del", Some(args)).await?;
        Ok(())
    }

    async fn vmm_ping(&self, system_id: &str) -> Result<bt::VmmPingResponse, BackendError> {
        let client = self.client_for(system_id)?;
        let status = client.execute("query-status", None).await?;
        let _ = status; // If we got here, QEMU is reachable
        Ok(bt::VmmPingResponse {
            version: Some("QEMU".to_string()),
            pid: None,
        })
    }

    async fn vm_counters(&self, _system_id: &str) -> Result<serde_json::Value, BackendError> {
        Err(BackendError::NotSupported(
            "vm_counters not supported for QEMU backend".to_string(),
        ))
    }
}

pub fn build_backend(config: &AppConfig) -> super::Backend {
    let sockets = config
        .systems
        .iter()
        .filter_map(|(id, sys)| {
            sys.socket_path.clone().map(|p| (id.clone(), p))
        })
        .collect();
    super::Backend::Qemu(QemuBackend::new(sockets))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_qemu_status_to_power_state() {
        assert_eq!(qemu_status_to_power_state("running"), bt::VmPowerState::On);
        assert_eq!(qemu_status_to_power_state("paused"), bt::VmPowerState::Paused);
        assert_eq!(qemu_status_to_power_state("suspended"), bt::VmPowerState::Paused);
        assert_eq!(qemu_status_to_power_state("shutdown"), bt::VmPowerState::Off);
        assert_eq!(qemu_status_to_power_state("inmigrate"), bt::VmPowerState::Off);
        assert_eq!(qemu_status_to_power_state("prelaunch"), bt::VmPowerState::Off);
        assert_eq!(qemu_status_to_power_state(""), bt::VmPowerState::Off);
    }

    #[test]
    fn test_client_for_missing_system() {
        let backend = QemuBackend::new(std::collections::HashMap::new());
        match backend.client_for("nonexistent") {
            Err(BackendError::VmNotFound) => {}
            Err(other) => panic!("expected VmNotFound, got: {other}"),
            Ok(_) => panic!("expected error, got Ok"),
        }
    }

    #[test]
    fn test_client_for_existing_system() {
        let mut sockets = std::collections::HashMap::new();
        sockets.insert("vm1".to_string(), std::path::PathBuf::from("/tmp/qmp.sock"));
        let backend = QemuBackend::new(sockets);
        // client_for returns a QmpClient which doesn't impl Debug,
        // so just check it doesn't error
        let result = backend.client_for("vm1");
        assert!(result.is_ok());
    }
}
