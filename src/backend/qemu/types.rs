use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct QmpGreeting {
    #[serde(rename = "QMP")]
    pub qmp: QmpGreetingInner,
}

#[derive(Debug, Deserialize)]
pub struct QmpGreetingInner {
    pub version: QmpVersion,
}

#[derive(Debug, Deserialize)]
pub struct QmpVersion {
    pub qemu: QmpQemuVersion,
}

#[derive(Debug, Deserialize)]
pub struct QmpQemuVersion {
    pub major: u32,
    pub minor: u32,
    pub micro: u32,
}

#[derive(Debug, Deserialize)]
pub struct QmpResponse<T> {
    #[serde(rename = "return")]
    pub result: Option<T>,
    pub error: Option<QmpError>,
}

#[derive(Debug, Deserialize)]
pub struct QmpError {
    pub desc: String,
}

#[derive(Debug, Deserialize)]
pub struct QmpStatus {
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct QmpCpu {}

#[derive(Debug, Deserialize)]
pub struct QmpMemorySizeSummary {
    #[serde(rename = "base-memory")]
    pub base_memory: u64,
    #[serde(rename = "plugged-memory", default)]
    pub plugged_memory: u64,
}

#[derive(Debug, Deserialize)]
pub struct QmpBlockDevice {
    pub device: String,
    pub inserted: Option<QmpBlockInserted>,
}

#[derive(Debug, Deserialize)]
pub struct QmpBlockInserted {
    pub file: String,
    pub ro: bool,
    #[serde(default)]
    pub drv: String,
}

#[derive(Debug, Deserialize)]
pub struct QmpPciBus {
    pub bus: u32,
    pub devices: Option<Vec<QmpPciDevice>>,
}

#[derive(Debug, Deserialize)]
pub struct QmpPciDevice {
    pub slot: u32,
    pub function: u32,
    pub id: QmpPciId,
    pub class_info: QmpPciClassInfo,
    #[serde(rename = "qdev_id", default)]
    pub qdev_id: String,
}

#[derive(Debug, Deserialize)]
pub struct QmpPciId {
    pub device: u32,
    pub vendor: u32,
}

#[derive(Debug, Deserialize)]
pub struct QmpPciClassInfo {
    pub class: u32,
}

#[derive(Debug, Deserialize)]
pub struct QmpBlockStats {
    pub stats: QmpBlockStatsInner,
}

#[derive(Debug, Deserialize)]
pub struct QmpBlockStatsInner {
    #[serde(rename = "rd_bytes", default)]
    pub rd_bytes: u64,
    #[serde(rename = "wr_bytes", default)]
    pub wr_bytes: u64,
    #[serde(rename = "rd_operations", default)]
    pub rd_operations: u64,
    #[serde(rename = "wr_operations", default)]
    pub wr_operations: u64,
}

#[cfg(test)]
mod coverage_tests {
    use super::*;

    #[test]
    fn test_qmp_greeting_deserialization() {
        let json = r#"{"QMP": {"version": {"qemu": {"major": 8, "minor": 2, "micro": 0}}}}"#;
        let greeting: QmpGreeting = serde_json::from_str(json).unwrap();
        assert_eq!(greeting.qmp.version.qemu.major, 8);
        assert_eq!(greeting.qmp.version.qemu.minor, 2);
        assert_eq!(greeting.qmp.version.qemu.micro, 0);
    }

    #[test]
    fn test_qmp_response_with_result() {
        let json = r#"{"return": {"status": "running"}}"#;
        let resp: QmpResponse<QmpStatus> = serde_json::from_str(json).unwrap();
        assert!(resp.result.is_some());
        assert!(resp.error.is_none());
        assert_eq!(resp.result.unwrap().status, "running");
    }

    #[test]
    fn test_qmp_response_with_error() {
        let json = r#"{"error": {"desc": "Command not found"}}"#;
        let resp: QmpResponse<serde_json::Value> = serde_json::from_str(json).unwrap();
        assert!(resp.result.is_none());
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap().desc, "Command not found");
    }

    #[test]
    fn test_qmp_status_deserialization() {
        let json = r#"{"status": "paused"}"#;
        let status: QmpStatus = serde_json::from_str(json).unwrap();
        assert_eq!(status.status, "paused");
    }

    #[test]
    fn test_qmp_cpu_deserialization() {
        let json = r#"{}"#;
        let _cpu: QmpCpu = serde_json::from_str(json).unwrap();
    }

    #[test]
    fn test_qmp_memory_size_summary_deserialization() {
        let json = r#"{"base-memory": 2147483648, "plugged-memory": 1073741824}"#;
        let mem: QmpMemorySizeSummary = serde_json::from_str(json).unwrap();
        assert_eq!(mem.base_memory, 2147483648);
        assert_eq!(mem.plugged_memory, 1073741824);
    }

    #[test]
    fn test_qmp_memory_size_summary_no_plugged() {
        let json = r#"{"base-memory": 2147483648}"#;
        let mem: QmpMemorySizeSummary = serde_json::from_str(json).unwrap();
        assert_eq!(mem.base_memory, 2147483648);
        assert_eq!(mem.plugged_memory, 0);
    }

    #[test]
    fn test_qmp_block_device_deserialization() {
        let json = r#"{"device": "virtio0", "inserted": {"file": "/tmp/disk.qcow2", "ro": false, "drv": "qcow2"}}"#;
        let block: QmpBlockDevice = serde_json::from_str(json).unwrap();
        assert_eq!(block.device, "virtio0");
        let inserted = block.inserted.unwrap();
        assert_eq!(inserted.file, "/tmp/disk.qcow2");
        assert!(!inserted.ro);
        assert_eq!(inserted.drv, "qcow2");
    }

    #[test]
    fn test_qmp_block_device_no_inserted() {
        let json = r#"{"device": "cd0"}"#;
        let block: QmpBlockDevice = serde_json::from_str(json).unwrap();
        assert_eq!(block.device, "cd0");
        assert!(block.inserted.is_none());
    }

    #[test]
    fn test_qmp_pci_bus_deserialization() {
        let json = r#"{"bus": 0, "devices": [{"slot": 3, "function": 0, "id": {"device": 4660, "vendor": 32902}, "class_info": {"class": 131072}, "qdev_id": "net0"}]}"#;
        let bus: QmpPciBus = serde_json::from_str(json).unwrap();
        assert_eq!(bus.bus, 0);
        let devices = bus.devices.unwrap();
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].slot, 3);
        assert_eq!(devices[0].function, 0);
        assert_eq!(devices[0].id.device, 4660);
        assert_eq!(devices[0].id.vendor, 32902);
        assert_eq!(devices[0].class_info.class, 131072);
        assert_eq!(devices[0].qdev_id, "net0");
    }

    #[test]
    fn test_qmp_pci_device_no_qdev_id() {
        let json = r#"{"slot": 1, "function": 0, "id": {"device": 1234, "vendor": 5678}, "class_info": {"class": 196608}}"#;
        let dev: QmpPciDevice = serde_json::from_str(json).unwrap();
        assert_eq!(dev.qdev_id, "");
    }

    #[test]
    fn test_qmp_block_stats_deserialization() {
        let json = r#"{"stats": {"rd_bytes": 1048576, "wr_bytes": 2097152, "rd_operations": 100, "wr_operations": 200}}"#;
        let stats: QmpBlockStats = serde_json::from_str(json).unwrap();
        assert_eq!(stats.stats.rd_bytes, 1048576);
        assert_eq!(stats.stats.wr_bytes, 2097152);
        assert_eq!(stats.stats.rd_operations, 100);
        assert_eq!(stats.stats.wr_operations, 200);
    }

    #[test]
    fn test_qmp_block_stats_defaults() {
        let json = r#"{"stats": {}}"#;
        let stats: QmpBlockStats = serde_json::from_str(json).unwrap();
        assert_eq!(stats.stats.rd_bytes, 0);
        assert_eq!(stats.stats.wr_bytes, 0);
        assert_eq!(stats.stats.rd_operations, 0);
        assert_eq!(stats.stats.wr_operations, 0);
    }
}
