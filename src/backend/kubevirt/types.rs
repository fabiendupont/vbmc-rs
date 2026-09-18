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
    #[serde(default, rename = "rebootPolicy", skip_serializing_if = "Option::is_none")]
    pub reboot_policy: Option<String>,
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
    #[serde(default, rename = "bootOrder", skip_serializing_if = "Option::is_none")]
    pub boot_order: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cdrom: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disk: Option<serde_json::Value>,
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

#[cfg(test)]
mod coverage_tests {
    use super::*;

    #[test]
    fn test_virtual_machine_spec_serde() {
        let spec = VirtualMachineSpec {
            running: Some(true),
            run_strategy: Some("Always".to_string()),
            template: Some(VMTemplate {
                spec: Some(VMTemplateSpec {
                    domain: Some(DomainSpec {
                        cpu: Some(CPU {
                            cores: Some(2),
                            sockets: Some(1),
                            threads: Some(1),
                        }),
                        memory: Some(Memory {
                            guest: Some("4Gi".to_string()),
                        }),
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
            }),
        };

        let json = serde_json::to_string(&spec).unwrap();
        let deserialized: VirtualMachineSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.running, Some(true));
        assert_eq!(deserialized.run_strategy.as_deref(), Some("Always"));
    }

    #[test]
    fn test_virtual_machine_instance_spec_serde() {
        let spec = VirtualMachineInstanceSpec {
            domain: Some(DomainSpec {
                cpu: Some(CPU {
                    cores: Some(4),
                    sockets: Some(2),
                    threads: Some(1),
                }),
                ..Default::default()
            }),
        };

        let json = serde_json::to_string(&spec).unwrap();
        let deserialized: VirtualMachineInstanceSpec = serde_json::from_str(&json).unwrap();
        assert!(deserialized.domain.is_some());
        assert_eq!(deserialized.domain.unwrap().cpu.unwrap().cores, Some(4));
    }

    #[test]
    fn test_virtual_machine_instance_status_serde() {
        let status = VirtualMachineInstanceStatus {
            phase: Some("Running".to_string()),
        };

        let json = serde_json::to_string(&status).unwrap();
        let deserialized: VirtualMachineInstanceStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.phase.as_deref(), Some("Running"));
    }

    #[test]
    fn test_domain_spec_with_devices_serde() {
        let spec = DomainSpec {
            devices: Some(Devices {
                disks: Some(vec![Disk {
                    name: Some("disk0".to_string()),
                    bus: Some("virtio".to_string()),
                    ..Default::default()
                }]),
                interfaces: Some(vec![Interface {
                    name: Some("eth0".to_string()),
                    mac_address: Some("52:54:00:12:34:56".to_string()),
                    bridge: Some(serde_json::json!({})),
                    masquerade: None,
                }]),
            }),
            firmware: Some(Firmware {
                bootloader: Some(Bootloader {
                    efi: Some(EFI {
                        secure_boot: Some(true),
                    }),
                }),
            }),
            ..Default::default()
        };

        let json = serde_json::to_string(&spec).unwrap();
        let deserialized: DomainSpec = serde_json::from_str(&json).unwrap();
        assert!(deserialized.devices.is_some());
        assert!(deserialized.firmware.is_some());
    }

    #[test]
    fn test_volume_serde() {
        let volume = Volume {
            name: Some("disk1".to_string()),
            data_volume: Some(DataVolumeSource {
                name: Some("dv-disk1".to_string()),
            }),
            container_disk: None,
        };

        let json = serde_json::to_string(&volume).unwrap();
        assert!(json.contains("dataVolume")); // Check camelCase renaming
        let deserialized: Volume = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name.as_deref(), Some("disk1"));
        assert_eq!(
            deserialized.data_volume.unwrap().name.as_deref(),
            Some("dv-disk1")
        );
    }

    #[test]
    fn test_interface_mac_address_rename_serde() {
        let interface = Interface {
            name: Some("eth0".to_string()),
            mac_address: Some("aa:bb:cc:dd:ee:ff".to_string()),
            bridge: None,
            masquerade: None,
        };

        let json = serde_json::to_string(&interface).unwrap();
        assert!(json.contains("macAddress")); // Check camelCase renaming
        let deserialized: Interface = serde_json::from_str(&json).unwrap();
        assert_eq!(
            deserialized.mac_address.as_deref(),
            Some("aa:bb:cc:dd:ee:ff")
        );
    }

    #[test]
    fn test_network_serde() {
        let network = Network {
            name: Some("default".to_string()),
            pod: Some(serde_json::json!({})),
            multus: None,
        };

        let json = serde_json::to_string(&network).unwrap();
        let deserialized: Network = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name.as_deref(), Some("default"));
    }

    #[test]
    fn test_multus_network_serde() {
        let multus = MultusNetwork {
            network_name: Some("nad-1".to_string()),
        };

        let json = serde_json::to_string(&multus).unwrap();
        assert!(json.contains("networkName")); // Check camelCase renaming
        let deserialized: MultusNetwork = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.network_name.as_deref(), Some("nad-1"));
    }

    #[test]
    fn test_efi_secure_boot_rename_serde() {
        let efi = EFI {
            secure_boot: Some(true),
        };

        let json = serde_json::to_string(&efi).unwrap();
        assert!(json.contains("secureBoot")); // Check camelCase renaming
        let deserialized: EFI = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.secure_boot, Some(true));
    }

    #[test]
    fn test_phase_to_power_state_comprehensive() {
        assert_eq!(phase_to_power_state("Running"), VmPowerState::On);
        assert_eq!(phase_to_power_state("Succeeded"), VmPowerState::Off);
        assert_eq!(phase_to_power_state("Failed"), VmPowerState::Off);
        assert_eq!(phase_to_power_state("Scheduling"), VmPowerState::Unknown);
        assert_eq!(phase_to_power_state("Scheduled"), VmPowerState::Unknown);
        assert_eq!(phase_to_power_state("Pending"), VmPowerState::Unknown);
        assert_eq!(phase_to_power_state("Unknown"), VmPowerState::Unknown);
        assert_eq!(phase_to_power_state(""), VmPowerState::Unknown);
        assert_eq!(phase_to_power_state("Invalid"), VmPowerState::Unknown);
    }

    #[test]
    fn test_defaults() {
        let vm_spec = VirtualMachineSpec::default();
        assert_eq!(vm_spec.running, None);
        assert_eq!(vm_spec.run_strategy, None);
        assert!(vm_spec.template.is_none());

        let vmi_spec = VirtualMachineInstanceSpec::default();
        assert!(vmi_spec.domain.is_none());

        let status = VirtualMachineInstanceStatus::default();
        assert_eq!(status.phase, None);

        let domain = DomainSpec::default();
        assert!(domain.cpu.is_none());
        assert!(domain.memory.is_none());
    }

    #[test]
    fn test_container_disk_source() {
        let volume = Volume {
            name: Some("cdrom".to_string()),
            data_volume: None,
            container_disk: Some(ContainerDiskSource {
                image: Some("quay.io/kubevirt/cirros:latest".to_string()),
            }),
        };

        let json = serde_json::to_string(&volume).unwrap();
        assert!(json.contains("containerDisk"));
        let deserialized: Volume = serde_json::from_str(&json).unwrap();
        assert!(deserialized.container_disk.is_some());
        assert_eq!(
            deserialized.container_disk.unwrap().image.as_deref(),
            Some("quay.io/kubevirt/cirros:latest")
        );
    }
}
