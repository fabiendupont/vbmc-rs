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
            Err(BackendError::ApiError(format!(
                "HTTP {status}: {msg}"
            )))
        }
    }

    fn check_success(status: StatusCode, body: &[u8]) -> Result<(), BackendError> {
        if status.is_success() {
            Ok(())
        } else {
            let msg = String::from_utf8_lossy(body);
            Err(BackendError::ApiError(format!(
                "HTTP {status}: {msg}"
            )))
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

    let memory_bytes = ch_info
        .config
        .memory
        .as_ref()
        .map(|m| m.size)
        .unwrap_or(0);

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
                        capacity_bytes: d.path.as_ref().and_then(|p| {
                            std::fs::metadata(p).ok().map(|m| m.len())
                        }),
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

    let raw = serde_json::to_value(&ch_info).ok();

    bt::VmInfo {
        power_state,
        cpu_count,
        max_cpu_count,
        cpu_topology,
        memory_bytes,
        memory_actual_bytes: ch_info.memory_actual_size,
        disks,
        nics,
        pci_devices: Vec::new(),
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
        let payload = serde_json::to_vec(&ch_config)
            .map_err(|e| BackendError::ApiError(e.to_string()))?;
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
        let payload = serde_json::to_vec(&ch_disk)
            .map_err(|e| BackendError::ApiError(e.to_string()))?;
        let (status, body) = client.put("/api/v1/vm.add-disk", &payload).await?;
        Self::check_success(status, &body)
    }

    async fn vm_remove_device(
        &self,
        system_id: &str,
        device_id: &str,
    ) -> Result<(), BackendError> {
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

    async fn vm_counters(&self, system_id: &str) -> Result<serde_json::Value, BackendError> {
        let client = self.client_for(system_id)?;
        let (status, body) = client.get("/api/v1/vm.counters").await?;
        Self::parse_response(status, &body)
    }
}
