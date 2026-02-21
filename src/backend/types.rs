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
pub struct VmmPingResponse {
    pub version: Option<String>,
    pub pid: Option<i64>,
}
