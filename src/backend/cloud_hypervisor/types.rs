use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VmConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<PayloadConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpus: Option<CpusConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory: Option<MemoryConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disks: Option<Vec<DiskConfig>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub net: Option<Vec<NetConfig>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub serial: Option<ConsoleConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub console: Option<ConsoleConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform: Option<PlatformConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayloadConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub firmware: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kernel: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cmdline: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initramfs: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpusConfig {
    pub boot_vcpus: u8,
    #[serde(default)]
    pub max_vcpus: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topology: Option<CpuTopology>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuTopology {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub threads_per_core: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cores_per_die: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dies_per_package: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub packages: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    pub size: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hotplug_size: Option<u64>,
    #[serde(default)]
    pub shared: bool,
    #[serde(default)]
    pub hugepages: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskConfig {
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default)]
    pub readonly: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vhost_user: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vhost_socket: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tap: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mac: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mask: Option<String>,
    #[serde(default)]
    pub num_queues: Option<u32>,
    #[serde(default)]
    pub queue_size: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsoleConfig {
    pub mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_pci_segments: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iommu_segments: Option<Vec<u16>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub serial_number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oem_strings: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmInfo {
    pub config: VmConfig,
    pub state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_actual_size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_tree: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmmPingResponse {
    pub build_version: Option<String>,
    pub version: Option<String>,
    pub pid: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmRemoveDevice {
    pub id: String,
}
