pub mod client;
pub mod types;

use std::collections::HashMap;
use std::path::PathBuf;

use axum::http::StatusCode;

use crate::backend::types as bt;
use crate::backend::{BackendError, VmmBackend};
use client::UnixClient;
use types::{VmRemoveDevice, VmmPingResponse as ChVmmPingResponse};

pub struct CloudHypervisorBackend {
    sockets: HashMap<String, PathBuf>,
}

impl CloudHypervisorBackend {
    pub fn new(sockets: HashMap<String, PathBuf>) -> Self {
        Self { sockets }
    }

    fn client_for(&self, system_id: &str) -> Result<UnixClient, BackendError> {
        let path = self
            .sockets
            .get(system_id)
            .ok_or(BackendError::VmNotFound)?;
        Ok(UnixClient::new(path))
    }

    fn parse_response<T: serde::de::DeserializeOwned>(
        status: StatusCode,
        body: &[u8],
    ) -> Result<T, BackendError> {
        if status.is_success() {
            serde_json::from_slice(body)
                .map_err(|e| BackendError::ApiError(format!("Failed to parse response: {e}")))
        } else {
            let msg = String::from_utf8_lossy(body);
            Err(BackendError::ApiError(format!("HTTP {status}: {msg}")))
        }
    }

    fn check_success(status: StatusCode, body: &[u8]) -> Result<(), BackendError> {
        if status.is_success() {
            Ok(())
        } else {
            let msg = String::from_utf8_lossy(body);
            Err(BackendError::ApiError(format!("HTTP {status}: {msg}")))
        }
    }
}

fn ch_state_to_power_state(state: &str) -> bt::VmPowerState {
    match state {
        "Running" => bt::VmPowerState::On,
        "Shutdown" | "Created" => bt::VmPowerState::Off,
        "Paused" => bt::VmPowerState::Paused,
        _ => bt::VmPowerState::Unknown,
    }
}

fn ch_vm_info_to_vm_info(ch_info: types::VmInfo) -> bt::VmInfo {
    let power_state = ch_state_to_power_state(&ch_info.state);

    let cpu_count = ch_info
        .config
        .cpus
        .as_ref()
        .map(|c| c.boot_vcpus as u32)
        .unwrap_or(0);

    let max_cpu_count = ch_info
        .config
        .cpus
        .as_ref()
        .map(|c| c.max_vcpus as u32)
        .unwrap_or(cpu_count);

    let cpu_topology = ch_info.config.cpus.as_ref().and_then(|c| {
        c.topology.as_ref().map(|t| bt::CpuTopologyInfo {
            threads_per_core: t.threads_per_core,
            cores_per_die: t.cores_per_die,
            dies_per_package: t.dies_per_package,
            packages: t.packages,
        })
    });

    let memory_bytes = ch_info.config.memory.as_ref().map(|m| m.size).unwrap_or(0);

    let disks = ch_info
        .config
        .disks
        .as_ref()
        .map(|ds| {
            ds.iter()
                .enumerate()
                .map(|(i, d)| {
                    let protocol = if d.vhost_user == Some(true) {
                        bt::DiskProtocol::VhostUser
                    } else {
                        bt::DiskProtocol::Virtio
                    };
                    bt::DiskInfo {
                        id: d.id.clone().unwrap_or_else(|| format!("disk{i}")),
                        path: d.path.clone(),
                        capacity_bytes: d
                            .path
                            .as_ref()
                            .and_then(|p| std::fs::metadata(p).ok().map(|m| m.len())),
                        readonly: d.readonly,
                        protocol,
                        media_type: bt::DiskMediaType::Virtual,
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    let nics = ch_info
        .config
        .net
        .as_ref()
        .map(|ns| {
            ns.iter()
                .enumerate()
                .map(|(i, n)| bt::NicInfo {
                    id: n.id.clone().unwrap_or_else(|| format!("NIC{i}")),
                    mac_address: n.mac.clone(),
                    tap: n.tap.clone(),
                    speed_mbps: 25000,
                })
                .collect()
        })
        .unwrap_or_default();

    let uuid = ch_info
        .config
        .platform
        .as_ref()
        .and_then(|p| p.uuid.clone());

    let raw = serde_json::to_value(&ch_info).ok();

    bt::VmInfo {
        power_state,
        cpu_count,
        max_cpu_count,
        cpu_topology,
        memory_bytes,
        memory_actual_bytes: ch_info.memory_actual_size,
        secure_boot: None,
        disks,
        nics,
        pci_devices: Vec::new(),
        uuid,
        raw,
    }
}

fn vm_create_config_to_ch(config: bt::VmCreateConfig) -> types::VmConfig {
    let payload = Some(types::PayloadConfig {
        firmware: config.firmware_path,
        kernel: config.kernel_path,
        cmdline: config.cmdline,
        initramfs: config.initramfs,
    });

    let cpus = Some(types::CpusConfig {
        boot_vcpus: config.cpu_count,
        max_vcpus: config.max_cpu_count,
        topology: None,
    });

    let memory = Some(types::MemoryConfig {
        size: config.memory_bytes,
        hotplug_size: None,
        shared: false,
        hugepages: false,
    });

    let disks = if config.disks.is_empty() {
        None
    } else {
        Some(
            config
                .disks
                .into_iter()
                .map(|d| types::DiskConfig {
                    path: d.path,
                    id: d.id,
                    readonly: d.readonly,
                    vhost_user: d.vhost_user,
                    vhost_socket: d.vhost_socket,
                })
                .collect(),
        )
    };

    let net = if config.nics.is_empty() {
        None
    } else {
        Some(
            config
                .nics
                .into_iter()
                .map(|n| types::NetConfig {
                    id: n.id,
                    tap: n.tap,
                    mac: n.mac,
                    ip: n.ip,
                    mask: n.mask,
                    num_queues: n.num_queues,
                    queue_size: n.queue_size,
                })
                .collect(),
        )
    };

    let platform = config.platform.map(|p| types::PlatformConfig {
        num_pci_segments: p.num_pci_segments,
        iommu_segments: p.iommu_segments,
        serial_number: p.serial_number,
        uuid: p.uuid,
        oem_strings: p.oem_strings,
    });

    types::VmConfig {
        payload,
        cpus,
        memory,
        disks,
        net,
        serial: None,
        console: None,
        platform,
    }
}

fn disk_create_config_to_ch(disk: bt::DiskCreateConfig) -> types::DiskConfig {
    types::DiskConfig {
        path: disk.path,
        id: disk.id,
        readonly: disk.readonly,
        vhost_user: disk.vhost_user,
        vhost_socket: disk.vhost_socket,
    }
}

fn parse_ch_counters(raw: &serde_json::Value) -> bt::VmCounters {
    let mut counters = bt::VmCounters::default();
    let obj = match raw.as_object() {
        Some(o) => o,
        None => return counters,
    };

    for (key, val) in obj {
        let inner = match val.as_object() {
            Some(o) => o,
            None => continue,
        };
        if key.starts_with("vcpu") {
            let idx: usize = match key.strip_prefix("vcpu").and_then(|s| s.parse().ok()) {
                Some(i) => i,
                None => continue,
            };
            while counters.cpu_cycles.len() <= idx {
                counters.cpu_cycles.push(0);
                counters.instructions.push(0);
            }
            if let Some(v) = inner.get("cpu_cycles").and_then(|v| v.as_u64()) {
                counters.cpu_cycles[idx] = v;
            }
            if let Some(v) = inner.get("instructions").and_then(|v| v.as_u64()) {
                counters.instructions[idx] = v;
            }
        } else if key.starts_with("block") {
            if let Some(v) = inner.get("read_bytes").and_then(|v| v.as_u64()) {
                counters.block_read_bytes += v;
            }
            if let Some(v) = inner.get("write_bytes").and_then(|v| v.as_u64()) {
                counters.block_write_bytes += v;
            }
            if let Some(v) = inner.get("read_ops").and_then(|v| v.as_u64()) {
                counters.block_read_ops += v;
            }
            if let Some(v) = inner.get("write_ops").and_then(|v| v.as_u64()) {
                counters.block_write_ops += v;
            }
        } else if key.starts_with("net") {
            if let Some(v) = inner.get("rx_bytes").and_then(|v| v.as_u64()) {
                counters.net_rx_bytes += v;
            }
            if let Some(v) = inner.get("tx_bytes").and_then(|v| v.as_u64()) {
                counters.net_tx_bytes += v;
            }
            if let Some(v) = inner.get("rx_frames").and_then(|v| v.as_u64()) {
                counters.net_rx_frames += v;
            }
            if let Some(v) = inner.get("tx_frames").and_then(|v| v.as_u64()) {
                counters.net_tx_frames += v;
            }
        }
    }
    counters
}

impl VmmBackend for CloudHypervisorBackend {
    async fn vm_info(&self, system_id: &str) -> Result<bt::VmInfo, BackendError> {
        let client = self.client_for(system_id)?;
        let (status, body) = client.get("/api/v1/vm.info").await?;
        let ch_info: types::VmInfo = Self::parse_response(status, &body)?;
        Ok(ch_vm_info_to_vm_info(ch_info))
    }

    async fn vm_create(
        &self,
        system_id: &str,
        config: bt::VmCreateConfig,
    ) -> Result<(), BackendError> {
        let client = self.client_for(system_id)?;
        let ch_config = vm_create_config_to_ch(config);
        let payload =
            serde_json::to_vec(&ch_config).map_err(|e| BackendError::ApiError(e.to_string()))?;
        let (status, body) = client.put("/api/v1/vm.create", &payload).await?;
        Self::check_success(status, &body)
    }

    async fn vm_boot(&self, system_id: &str) -> Result<(), BackendError> {
        let client = self.client_for(system_id)?;
        let (status, body) = client.put("/api/v1/vm.boot", b"").await?;
        Self::check_success(status, &body)
    }

    async fn vm_shutdown(&self, system_id: &str) -> Result<(), BackendError> {
        let client = self.client_for(system_id)?;
        let (status, body) = client.put("/api/v1/vm.shutdown", b"").await?;
        Self::check_success(status, &body)
    }

    async fn vm_delete(&self, system_id: &str) -> Result<(), BackendError> {
        let client = self.client_for(system_id)?;
        let (status, body) = client.put("/api/v1/vm.delete", b"").await?;
        Self::check_success(status, &body)
    }

    async fn vm_power_button(&self, system_id: &str) -> Result<(), BackendError> {
        let client = self.client_for(system_id)?;
        let (status, body) = client.put("/api/v1/vm.power-button", b"").await?;
        Self::check_success(status, &body)
    }

    async fn vm_reboot(&self, system_id: &str) -> Result<(), BackendError> {
        let client = self.client_for(system_id)?;
        let (status, body) = client.put("/api/v1/vm.reboot", b"").await?;
        Self::check_success(status, &body)
    }

    async fn vm_add_disk(
        &self,
        system_id: &str,
        disk: bt::DiskCreateConfig,
    ) -> Result<(), BackendError> {
        let client = self.client_for(system_id)?;
        let ch_disk = disk_create_config_to_ch(disk);
        let payload =
            serde_json::to_vec(&ch_disk).map_err(|e| BackendError::ApiError(e.to_string()))?;
        let (status, body) = client.put("/api/v1/vm.add-disk", &payload).await?;
        Self::check_success(status, &body)
    }

    async fn vm_remove_device(&self, system_id: &str, device_id: &str) -> Result<(), BackendError> {
        let client = self.client_for(system_id)?;
        let payload = serde_json::to_vec(&VmRemoveDevice {
            id: device_id.to_string(),
        })
        .map_err(|e| BackendError::ApiError(e.to_string()))?;
        let (status, body) = client.put("/api/v1/vm.remove-device", &payload).await?;
        Self::check_success(status, &body)
    }

    async fn vmm_ping(&self, system_id: &str) -> Result<bt::VmmPingResponse, BackendError> {
        let client = self.client_for(system_id)?;
        let (status, body) = client.get("/api/v1/vmm.ping").await?;
        let ch_ping: ChVmmPingResponse = Self::parse_response(status, &body)?;
        Ok(bt::VmmPingResponse {
            version: ch_ping.version.or(ch_ping.build_version),
            pid: ch_ping.pid,
        })
    }

    async fn vm_counters(&self, system_id: &str) -> Result<bt::VmCounters, BackendError> {
        let client = self.client_for(system_id)?;
        let (status, body) = client.get("/api/v1/vm.counters").await?;
        let raw: serde_json::Value = Self::parse_response(status, &body)?;
        Ok(parse_ch_counters(&raw))
    }

    // Secure boot is handled at VM creation time via firmware selection;
    // Cloud Hypervisor has no runtime API to toggle it on an existing VM.
    async fn vm_set_secure_boot(
        &self,
        _system_id: &str,
        _enabled: bool,
    ) -> Result<(), BackendError> {
        Ok(())
    }

    async fn vm_serial_console(
        &self,
        _system_id: &str,
    ) -> Result<bt::SerialConsoleInfo, BackendError> {
        Err(BackendError::NotSupported("serial console".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ch_state_to_power_state() {
        assert_eq!(ch_state_to_power_state("Running"), bt::VmPowerState::On);
        assert_eq!(ch_state_to_power_state("Shutdown"), bt::VmPowerState::Off);
        assert_eq!(ch_state_to_power_state("Created"), bt::VmPowerState::Off);
        assert_eq!(ch_state_to_power_state("Paused"), bt::VmPowerState::Paused);
        assert_eq!(
            ch_state_to_power_state("SomethingElse"),
            bt::VmPowerState::Unknown
        );
        assert_eq!(ch_state_to_power_state(""), bt::VmPowerState::Unknown);
    }

    #[test]
    fn test_ch_vm_info_to_vm_info_running_with_full_config() {
        let ch_info = types::VmInfo {
            config: types::VmConfig {
                cpus: Some(types::CpusConfig {
                    boot_vcpus: 4,
                    max_vcpus: 8,
                    topology: Some(types::CpuTopology {
                        threads_per_core: Some(2),
                        cores_per_die: Some(2),
                        dies_per_package: Some(1),
                        packages: Some(1),
                    }),
                }),
                memory: Some(types::MemoryConfig {
                    size: 2 * 1024 * 1024 * 1024,
                    hotplug_size: None,
                    shared: false,
                    hugepages: false,
                }),
                disks: Some(vec![
                    types::DiskConfig {
                        path: Some("/nonexistent/disk.raw".to_string()),
                        id: Some("rootdisk".to_string()),
                        readonly: false,
                        vhost_user: None,
                        vhost_socket: None,
                    },
                    types::DiskConfig {
                        path: None,
                        id: None,
                        readonly: true,
                        vhost_user: Some(true),
                        vhost_socket: Some("/tmp/vhost.sock".to_string()),
                    },
                ]),
                net: Some(vec![types::NetConfig {
                    id: Some("eth0".to_string()),
                    tap: Some("tap0".to_string()),
                    mac: Some("52:54:00:ab:cd:ef".to_string()),
                    ip: None,
                    mask: None,
                    num_queues: None,
                    queue_size: None,
                }]),
                ..Default::default()
            },
            state: "Running".to_string(),
            memory_actual_size: Some(2 * 1024 * 1024 * 1024),
            device_tree: None,
        };

        let info = ch_vm_info_to_vm_info(ch_info);

        assert_eq!(info.power_state, bt::VmPowerState::On);
        assert_eq!(info.cpu_count, 4);
        assert_eq!(info.max_cpu_count, 8);
        assert_eq!(info.memory_bytes, 2 * 1024 * 1024 * 1024);
        assert_eq!(info.memory_actual_bytes, Some(2 * 1024 * 1024 * 1024));

        // CPU topology
        let topo = info.cpu_topology.unwrap();
        assert_eq!(topo.threads_per_core, Some(2));
        assert_eq!(topo.cores_per_die, Some(2));

        // Disks
        assert_eq!(info.disks.len(), 2);
        assert_eq!(info.disks[0].id, "rootdisk");
        assert_eq!(info.disks[0].protocol, bt::DiskProtocol::Virtio);
        assert!(!info.disks[0].readonly);
        assert_eq!(info.disks[1].id, "disk1"); // auto-generated
        assert_eq!(info.disks[1].protocol, bt::DiskProtocol::VhostUser);
        assert!(info.disks[1].readonly);

        // NICs
        assert_eq!(info.nics.len(), 1);
        assert_eq!(info.nics[0].id, "eth0");
        assert_eq!(
            info.nics[0].mac_address.as_deref(),
            Some("52:54:00:ab:cd:ef")
        );
        assert_eq!(info.nics[0].tap.as_deref(), Some("tap0"));
        assert_eq!(info.nics[0].speed_mbps, 25000);

        // Raw should be present
        assert!(info.raw.is_some());
    }

    #[test]
    fn test_ch_vm_info_to_vm_info_empty_config() {
        let ch_info = types::VmInfo {
            config: types::VmConfig::default(),
            state: "Shutdown".to_string(),
            memory_actual_size: None,
            device_tree: None,
        };

        let info = ch_vm_info_to_vm_info(ch_info);

        assert_eq!(info.power_state, bt::VmPowerState::Off);
        assert_eq!(info.cpu_count, 0);
        assert_eq!(info.max_cpu_count, 0);
        assert_eq!(info.memory_bytes, 0);
        assert!(info.cpu_topology.is_none());
        assert!(info.disks.is_empty());
        assert!(info.nics.is_empty());
        assert!(info.pci_devices.is_empty());
    }

    #[test]
    fn test_vm_create_config_to_ch() {
        let config = bt::VmCreateConfig {
            firmware_path: Some("/usr/share/OVMF/OVMF_CODE.fd".to_string()),
            kernel_path: Some("/boot/vmlinuz".to_string()),
            cmdline: Some("console=ttyS0".to_string()),
            initramfs: None,
            cpu_count: 2,
            max_cpu_count: 4,
            memory_bytes: 1024 * 1024 * 1024,
            secure_boot: false,
            disks: vec![bt::DiskCreateConfig {
                path: Some("/tmp/disk.raw".to_string()),
                id: Some("root".to_string()),
                readonly: false,
                vhost_user: None,
                vhost_socket: None,
            }],
            nics: vec![bt::NicCreateConfig {
                id: Some("net0".to_string()),
                tap: Some("tap0".to_string()),
                mac: Some("52:54:00:00:00:01".to_string()),
                ip: None,
                mask: None,
                num_queues: None,
                queue_size: None,
            }],
            platform: None,
        };

        let ch = vm_create_config_to_ch(config);

        let payload = ch.payload.unwrap();
        assert_eq!(
            payload.firmware.as_deref(),
            Some("/usr/share/OVMF/OVMF_CODE.fd")
        );
        assert_eq!(payload.kernel.as_deref(), Some("/boot/vmlinuz"));
        assert_eq!(payload.cmdline.as_deref(), Some("console=ttyS0"));
        assert!(payload.initramfs.is_none());

        let cpus = ch.cpus.unwrap();
        assert_eq!(cpus.boot_vcpus, 2);
        assert_eq!(cpus.max_vcpus, 4);

        let memory = ch.memory.unwrap();
        assert_eq!(memory.size, 1024 * 1024 * 1024);

        let disks = ch.disks.unwrap();
        assert_eq!(disks.len(), 1);
        assert_eq!(disks[0].id.as_deref(), Some("root"));
        assert_eq!(disks[0].path.as_deref(), Some("/tmp/disk.raw"));

        let net = ch.net.unwrap();
        assert_eq!(net.len(), 1);
        assert_eq!(net[0].mac.as_deref(), Some("52:54:00:00:00:01"));
    }

    #[test]
    fn test_vm_create_config_to_ch_empty_disks_and_nics() {
        let config = bt::VmCreateConfig::default();
        let ch = vm_create_config_to_ch(config);

        assert!(ch.disks.is_none());
        assert!(ch.net.is_none());
    }

    #[test]
    fn test_disk_create_config_to_ch() {
        let disk = bt::DiskCreateConfig {
            path: Some("/tmp/test.iso".to_string()),
            id: Some("cdrom".to_string()),
            readonly: true,
            vhost_user: Some(false),
            vhost_socket: None,
        };

        let ch = disk_create_config_to_ch(disk);
        assert_eq!(ch.path.as_deref(), Some("/tmp/test.iso"));
        assert_eq!(ch.id.as_deref(), Some("cdrom"));
        assert!(ch.readonly);
        assert_eq!(ch.vhost_user, Some(false));
        assert!(ch.vhost_socket.is_none());
    }
}
