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
#[allow(dead_code)]
pub struct VmmPingResponse {
    pub build_version: Option<String>,
    pub version: Option<String>,
    pub pid: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmRemoveDevice {
    pub id: String,
}

#[cfg(test)]
mod coverage_tests {
    use super::*;

    #[test]
    fn test_vm_config_default() {
        let config = VmConfig::default();
        assert!(config.payload.is_none());
        assert!(config.cpus.is_none());
        assert!(config.memory.is_none());
        assert!(config.disks.is_none());
        assert!(config.net.is_none());
    }

    #[test]
    fn test_payload_config_serialization() {
        let payload = PayloadConfig {
            firmware: Some("/usr/share/OVMF.fd".to_string()),
            kernel: Some("/boot/vmlinuz".to_string()),
            cmdline: Some("console=ttyS0".to_string()),
            initramfs: None,
        };
        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("firmware"));
        assert!(json.contains("/usr/share/OVMF.fd"));
        assert!(!json.contains("initramfs"));
    }

    #[test]
    fn test_cpus_config_deserialization() {
        let json = r#"{"boot_vcpus": 4, "max_vcpus": 8}"#;
        let cpus: CpusConfig = serde_json::from_str(json).unwrap();
        assert_eq!(cpus.boot_vcpus, 4);
        assert_eq!(cpus.max_vcpus, 8);
        assert!(cpus.topology.is_none());
    }

    #[test]
    fn test_cpu_topology_deserialization() {
        let json =
            r#"{"threads_per_core": 2, "cores_per_die": 4, "dies_per_package": 1, "packages": 2}"#;
        let topo: CpuTopology = serde_json::from_str(json).unwrap();
        assert_eq!(topo.threads_per_core, Some(2));
        assert_eq!(topo.cores_per_die, Some(4));
        assert_eq!(topo.dies_per_package, Some(1));
        assert_eq!(topo.packages, Some(2));
    }

    #[test]
    fn test_memory_config_deserialization() {
        let json = r#"{"size": 2147483648, "shared": true, "hugepages": false}"#;
        let mem: MemoryConfig = serde_json::from_str(json).unwrap();
        assert_eq!(mem.size, 2147483648);
        assert!(mem.shared);
        assert!(!mem.hugepages);
        assert!(mem.hotplug_size.is_none());
    }

    #[test]
    fn test_memory_config_defaults() {
        let json = r#"{"size": 1073741824}"#;
        let mem: MemoryConfig = serde_json::from_str(json).unwrap();
        assert_eq!(mem.size, 1073741824);
        assert!(!mem.shared);
        assert!(!mem.hugepages);
    }

    #[test]
    fn test_disk_config_deserialization() {
        let json = r#"{"path": "/tmp/disk.raw", "id": "rootdisk", "readonly": false, "vhost_user": true, "vhost_socket": "/tmp/vhost.sock"}"#;
        let disk: DiskConfig = serde_json::from_str(json).unwrap();
        assert_eq!(disk.path.as_deref(), Some("/tmp/disk.raw"));
        assert_eq!(disk.id.as_deref(), Some("rootdisk"));
        assert!(!disk.readonly);
        assert_eq!(disk.vhost_user, Some(true));
        assert_eq!(disk.vhost_socket.as_deref(), Some("/tmp/vhost.sock"));
    }

    #[test]
    fn test_disk_config_defaults() {
        let json = r#"{"path": "/tmp/simple.img"}"#;
        let disk: DiskConfig = serde_json::from_str(json).unwrap();
        assert!(!disk.readonly);
    }

    #[test]
    fn test_net_config_deserialization() {
        let json = r#"{"id": "net0", "tap": "tap0", "mac": "52:54:00:12:34:56", "ip": "192.168.1.10", "mask": "255.255.255.0", "num_queues": 2, "queue_size": 256}"#;
        let net: NetConfig = serde_json::from_str(json).unwrap();
        assert_eq!(net.id.as_deref(), Some("net0"));
        assert_eq!(net.tap.as_deref(), Some("tap0"));
        assert_eq!(net.mac.as_deref(), Some("52:54:00:12:34:56"));
        assert_eq!(net.ip.as_deref(), Some("192.168.1.10"));
        assert_eq!(net.mask.as_deref(), Some("255.255.255.0"));
        assert_eq!(net.num_queues, Some(2));
        assert_eq!(net.queue_size, Some(256));
    }

    #[test]
    fn test_console_config_deserialization() {
        let json = r#"{"mode": "Pty", "file": "/tmp/console.log"}"#;
        let console: ConsoleConfig = serde_json::from_str(json).unwrap();
        assert_eq!(console.mode, "Pty");
        assert_eq!(console.file.as_deref(), Some("/tmp/console.log"));
    }

    #[test]
    fn test_platform_config_deserialization() {
        let json = r#"{"num_pci_segments": 2, "iommu_segments": [0, 1], "serial_number": "ABC123", "uuid": "550e8400-e29b-41d4-a716-446655440000", "oem_strings": ["string1", "string2"]}"#;
        let platform: PlatformConfig = serde_json::from_str(json).unwrap();
        assert_eq!(platform.num_pci_segments, Some(2));
        assert_eq!(platform.iommu_segments, Some(vec![0, 1]));
        assert_eq!(platform.serial_number.as_deref(), Some("ABC123"));
        assert_eq!(
            platform.uuid.as_deref(),
            Some("550e8400-e29b-41d4-a716-446655440000")
        );
        assert_eq!(
            platform.oem_strings,
            Some(vec!["string1".to_string(), "string2".to_string()])
        );
    }

    #[test]
    fn test_vm_info_deserialization() {
        let json = r#"{"config": {"memory": {"size": 1073741824, "shared": false, "hugepages": false}}, "state": "Running", "memory_actual_size": 1073741824}"#;
        let info: VmInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.state, "Running");
        assert_eq!(info.memory_actual_size, Some(1073741824));
        assert!(info.device_tree.is_none());
        let mem = info.config.memory.unwrap();
        assert_eq!(mem.size, 1073741824);
    }

    #[test]
    fn test_vmm_ping_response_deserialization() {
        let json = r#"{"build_version": "v42.0", "version": "v42.0", "pid": 12345}"#;
        let ping: VmmPingResponse = serde_json::from_str(json).unwrap();
        assert_eq!(ping.build_version.as_deref(), Some("v42.0"));
        assert_eq!(ping.version.as_deref(), Some("v42.0"));
        assert_eq!(ping.pid, Some(12345));
    }

    #[test]
    fn test_vmm_ping_response_only_version() {
        let json = r#"{"version": "v41.0"}"#;
        let ping: VmmPingResponse = serde_json::from_str(json).unwrap();
        assert!(ping.build_version.is_none());
        assert_eq!(ping.version.as_deref(), Some("v41.0"));
        assert!(ping.pid.is_none());
    }

    #[test]
    fn test_vm_remove_device_serialization() {
        let remove = VmRemoveDevice {
            id: "net0".to_string(),
        };
        let json = serde_json::to_string(&remove).unwrap();
        assert!(json.contains("net0"));
    }

    #[test]
    fn test_vm_config_serialization_skip_none() {
        let config = VmConfig {
            payload: Some(PayloadConfig {
                firmware: Some("/ovmf.fd".to_string()),
                kernel: None,
                cmdline: None,
                initramfs: None,
            }),
            cpus: None,
            memory: None,
            disks: None,
            net: None,
            serial: None,
            console: None,
            platform: None,
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("payload"));
        assert!(json.contains("firmware"));
        assert!(!json.contains("cpus"));
        assert!(!json.contains("memory"));
        assert!(!json.contains("kernel"));
    }
}
