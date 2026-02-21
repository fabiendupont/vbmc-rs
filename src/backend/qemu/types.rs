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
    pub package: String,
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
    pub class: String,
    pub desc: String,
}

#[derive(Debug, Deserialize)]
pub struct QmpStatus {
    pub running: bool,
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct QmpCpu {
    #[serde(rename = "cpu-index")]
    pub cpu_index: u32,
    #[serde(rename = "thread-id")]
    pub thread_id: Option<u64>,
}

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
    pub bus: u32,
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
