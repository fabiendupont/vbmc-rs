use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VmPowerState {
    On,
    Off,
    Paused,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiskProtocol {
    Virtio,
    NVMe,
    SATA,
    VhostUser,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiskMediaType {
    SSD,
    HDD,
    Virtual,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskInfo {
    pub id: String,
    pub path: Option<String>,
    pub capacity_bytes: Option<u64>,
    pub readonly: bool,
    pub protocol: DiskProtocol,
    pub media_type: DiskMediaType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NicInfo {
    pub id: String,
    pub mac_address: Option<String>,
    pub tap: Option<String>,
    pub speed_mbps: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PciFunctionInfo {
    pub function_id: u8,
    pub class_code: Option<String>,
    pub device_id: Option<String>,
    pub vendor_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PciDeviceInfo {
    pub bdf: String,
    pub vendor_id: Option<String>,
    pub device_id: Option<String>,
    pub class_code: Option<String>,
    pub device_name: Option<String>,
    pub is_passthrough: bool,
    pub functions: Vec<PciFunctionInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuTopologyInfo {
    pub threads_per_core: Option<u8>,
    pub cores_per_die: Option<u8>,
    pub dies_per_package: Option<u8>,
    pub packages: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmInfo {
    pub power_state: VmPowerState,
    pub cpu_count: u32,
    pub max_cpu_count: u32,
    pub cpu_topology: Option<CpuTopologyInfo>,
    pub memory_bytes: u64,
    pub memory_actual_bytes: Option<u64>,
    pub disks: Vec<DiskInfo>,
    pub nics: Vec<NicInfo>,
    pub pci_devices: Vec<PciDeviceInfo>,
    pub uuid: Option<String>,
    pub raw: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VmCreateConfig {
    pub firmware_path: Option<String>,
    pub kernel_path: Option<String>,
    pub cmdline: Option<String>,
    pub initramfs: Option<String>,
    pub cpu_count: u8,
    pub max_cpu_count: u8,
    pub memory_bytes: u64,
    pub disks: Vec<DiskCreateConfig>,
    pub nics: Vec<NicCreateConfig>,
    pub platform: Option<PlatformConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskCreateConfig {
    pub path: Option<String>,
    pub id: Option<String>,
    pub readonly: bool,
    pub vhost_user: Option<bool>,
    pub vhost_socket: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NicCreateConfig {
    pub id: Option<String>,
    pub tap: Option<String>,
    pub mac: Option<String>,
    pub ip: Option<String>,
    pub mask: Option<String>,
    pub num_queues: Option<u32>,
    pub queue_size: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformConfig {
    pub num_pci_segments: Option<u16>,
    pub iommu_segments: Option<Vec<u16>>,
    pub serial_number: Option<String>,
    pub uuid: Option<String>,
    pub oem_strings: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct VmmPingResponse {
    pub version: Option<String>,
    pub pid: Option<i64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VmCounters {
    pub cpu_cycles: Vec<u64>,
    pub instructions: Vec<u64>,
    pub block_read_bytes: u64,
    pub block_write_bytes: u64,
    pub block_read_ops: u64,
    pub block_write_ops: u64,
    pub net_rx_bytes: u64,
    pub net_tx_bytes: u64,
    pub net_rx_frames: u64,
    pub net_tx_frames: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vm_power_state_serde_roundtrip() {
        for state in [VmPowerState::On, VmPowerState::Off, VmPowerState::Paused, VmPowerState::Unknown] {
            let json = serde_json::to_string(&state).unwrap();
            let back: VmPowerState = serde_json::from_str(&json).unwrap();
            assert_eq!(state, back);
        }
    }

    #[test]
    fn test_disk_protocol_serde_roundtrip() {
        for proto in [DiskProtocol::Virtio, DiskProtocol::NVMe, DiskProtocol::SATA, DiskProtocol::VhostUser, DiskProtocol::Unknown] {
            let json = serde_json::to_string(&proto).unwrap();
            let back: DiskProtocol = serde_json::from_str(&json).unwrap();
            assert_eq!(proto, back);
        }
    }

    #[test]
    fn test_disk_media_type_serde_roundtrip() {
        for mt in [DiskMediaType::SSD, DiskMediaType::HDD, DiskMediaType::Virtual, DiskMediaType::Unknown] {
            let json = serde_json::to_string(&mt).unwrap();
            let back: DiskMediaType = serde_json::from_str(&json).unwrap();
            assert_eq!(mt, back);
        }
    }

    #[test]
    fn test_vm_create_config_default() {
        let config = VmCreateConfig::default();
        assert_eq!(config.cpu_count, 0);
        assert_eq!(config.max_cpu_count, 0);
        assert_eq!(config.memory_bytes, 0);
        assert!(config.disks.is_empty());
        assert!(config.nics.is_empty());
        assert!(config.firmware_path.is_none());
    }

    #[test]
    fn test_vm_info_serde_roundtrip() {
        let info = VmInfo {
            power_state: VmPowerState::On,
            cpu_count: 4,
            max_cpu_count: 8,
            cpu_topology: Some(CpuTopologyInfo {
                threads_per_core: Some(2),
                cores_per_die: Some(4),
                dies_per_package: Some(1),
                packages: Some(1),
            }),
            memory_bytes: 4 * 1024 * 1024 * 1024,
            memory_actual_bytes: Some(4 * 1024 * 1024 * 1024),
            disks: vec![DiskInfo {
                id: "vda".to_string(),
                path: Some("/tmp/disk.qcow2".to_string()),
                capacity_bytes: Some(10_000_000_000),
                readonly: false,
                protocol: DiskProtocol::Virtio,
                media_type: DiskMediaType::SSD,
            }],
            nics: vec![NicInfo {
                id: "NIC0".to_string(),
                mac_address: Some("52:54:00:12:34:56".to_string()),
                tap: Some("tap0".to_string()),
                speed_mbps: 25000,
            }],
            pci_devices: vec![],
            uuid: None,
            raw: None,
        };

        let json = serde_json::to_string(&info).unwrap();
        let back: VmInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(back.power_state, VmPowerState::On);
        assert_eq!(back.cpu_count, 4);
        assert_eq!(back.max_cpu_count, 8);
        assert_eq!(back.memory_bytes, 4 * 1024 * 1024 * 1024);
        assert_eq!(back.disks.len(), 1);
        assert_eq!(back.disks[0].id, "vda");
        assert_eq!(back.disks[0].protocol, DiskProtocol::Virtio);
        assert_eq!(back.nics.len(), 1);
        assert_eq!(back.nics[0].mac_address.as_deref(), Some("52:54:00:12:34:56"));
        assert_eq!(back.cpu_topology.unwrap().threads_per_core, Some(2));
    }
}
