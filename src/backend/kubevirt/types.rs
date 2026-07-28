use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::backend::types::VmPowerState;

#[derive(CustomResource, Clone, Debug, Serialize, Deserialize, Default, JsonSchema)]
#[kube(
    group = "kubevirt.io",
    version = "v1",
    kind = "VirtualMachine",
    plural = "virtualmachines",
    namespaced
)]
pub struct VirtualMachineSpec {
    #[serde(default)]
    pub running: Option<bool>,
    #[serde(default)]
    pub run_strategy: Option<String>,
    #[serde(default)]
    pub template: Option<VMTemplate>,
}

#[derive(CustomResource, Clone, Debug, Serialize, Deserialize, Default, JsonSchema)]
#[kube(
    group = "kubevirt.io",
    version = "v1",
    kind = "VirtualMachineInstance",
    plural = "virtualmachineinstances",
    namespaced,
    status = "VirtualMachineInstanceStatus"
)]
pub struct VirtualMachineInstanceSpec {
    #[serde(default)]
    pub domain: Option<DomainSpec>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default, JsonSchema)]
pub struct VirtualMachineInstanceStatus {
    #[serde(default)]
    pub phase: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default, JsonSchema)]
pub struct VMTemplate {
    #[serde(default)]
    pub spec: Option<VMTemplateSpec>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default, JsonSchema)]
pub struct VMTemplateSpec {
    #[serde(default)]
    pub domain: Option<DomainSpec>,
    #[serde(default)]
    pub volumes: Option<Vec<Volume>>,
    #[serde(default)]
    pub networks: Option<Vec<Network>>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default, JsonSchema)]
pub struct DomainSpec {
    #[serde(default)]
    pub cpu: Option<CPU>,
    #[serde(default)]
    pub memory: Option<Memory>,
    #[serde(default)]
    pub resources: Option<ResourceRequirements>,
    #[serde(default)]
    pub devices: Option<Devices>,
    #[serde(default)]
    pub firmware: Option<Firmware>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default, JsonSchema)]
pub struct CPU {
    #[serde(default)]
    pub cores: Option<u32>,
    #[serde(default)]
    pub sockets: Option<u32>,
    #[serde(default)]
    pub threads: Option<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default, JsonSchema)]
pub struct Memory {
    #[serde(default)]
    pub guest: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default, JsonSchema)]
pub struct ResourceRequirements {
    #[serde(default)]
    pub requests: Option<std::collections::BTreeMap<String, String>>,
    #[serde(default)]
    pub limits: Option<std::collections::BTreeMap<String, String>>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default, JsonSchema)]
pub struct Devices {
    #[serde(default)]
    pub disks: Option<Vec<Disk>>,
    #[serde(default)]
    pub interfaces: Option<Vec<Interface>>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default, JsonSchema)]
pub struct Disk {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub bus: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default, JsonSchema)]
pub struct Volume {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default, rename = "dataVolume")]
    pub data_volume: Option<DataVolumeSource>,
    #[serde(default, rename = "containerDisk")]
    pub container_disk: Option<ContainerDiskSource>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default, JsonSchema)]
pub struct DataVolumeSource {
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default, JsonSchema)]
pub struct ContainerDiskSource {
    #[serde(default)]
    pub image: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default, JsonSchema)]
pub struct Interface {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default, rename = "macAddress")]
    pub mac_address: Option<String>,
    #[serde(default)]
    pub bridge: Option<serde_json::Value>,
    #[serde(default)]
    pub masquerade: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default, JsonSchema)]
pub struct Network {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub pod: Option<serde_json::Value>,
    #[serde(default)]
    pub multus: Option<MultusNetwork>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default, JsonSchema)]
pub struct MultusNetwork {
    #[serde(default, rename = "networkName")]
    pub network_name: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default, JsonSchema)]
pub struct Firmware {
    #[serde(default)]
    pub bootloader: Option<Bootloader>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default, JsonSchema)]
pub struct Bootloader {
    #[serde(default)]
    pub efi: Option<EFI>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default, JsonSchema)]
pub struct EFI {
    #[serde(default, rename = "secureBoot")]
    pub secure_boot: Option<bool>,
}

pub fn phase_to_power_state(phase: &str) -> VmPowerState {
    match phase {
        "Running" => VmPowerState::On,
        "Succeeded" | "Failed" => VmPowerState::Off,
        "Scheduling" | "Scheduled" | "Pending" => VmPowerState::Unknown,
        _ => VmPowerState::Unknown,
    }
}
