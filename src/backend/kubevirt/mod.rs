pub mod types;

use std::collections::HashMap;

use kube::Api;
use kube::api::{DeleteParams, Patch, PatchParams};

use crate::backend::types as bt;
use crate::backend::{BackendError, VmmBackend};
use crate::config::AppConfig;

pub struct KubeVirtBackend {
    client: kube::Client,
    vms: HashMap<String, VmMapping>,
}

struct VmMapping {
    namespace: String,
    vm_name: String,
}

fn map_kube_error(e: kube::Error) -> BackendError {
    BackendError::ApiError(e.to_string())
}

impl KubeVirtBackend {
    fn mapping_for(&self, system_id: &str) -> Result<&VmMapping, BackendError> {
        self.vms.get(system_id).ok_or(BackendError::VmNotFound)
    }

    fn vm_api(&self, ns: &str) -> Api<types::VirtualMachine> {
        Api::namespaced(self.client.clone(), ns)
    }

    fn vmi_api(&self, ns: &str) -> Api<types::VirtualMachineInstance> {
        Api::namespaced(self.client.clone(), ns)
    }

    fn extract_info_from_domain(
        domain: &types::DomainSpec,
    ) -> (u32, u64, Vec<bt::DiskInfo>, Vec<bt::NicInfo>, Option<bool>) {
        let cpu_count = domain
            .cpu
            .as_ref()
            .map(|c| {
                let cores = c.cores.unwrap_or(1);
                let sockets = c.sockets.unwrap_or(1);
                let threads = c.threads.unwrap_or(1);
                cores * sockets * threads
            })
            .unwrap_or(1);

        let memory_bytes = domain
            .memory
            .as_ref()
            .and_then(|m| m.guest.as_ref())
            .map(|g| parse_memory_string(g))
            .unwrap_or(0);

        let disks: Vec<bt::DiskInfo> = domain
            .devices
            .as_ref()
            .and_then(|d| d.disks.as_ref())
            .map(|disks| {
                disks
                    .iter()
                    .enumerate()
                    .map(|(i, d)| bt::DiskInfo {
                        id: d.name.clone().unwrap_or_else(|| format!("disk-{i}")),
                        path: None,
                        capacity_bytes: None,
                        readonly: false,
                        protocol: bt::DiskProtocol::Virtio,
                        media_type: bt::DiskMediaType::Virtual,
                    })
                    .collect()
            })
            .unwrap_or_default();

        let nics: Vec<bt::NicInfo> = domain
            .devices
            .as_ref()
            .and_then(|d| d.interfaces.as_ref())
            .map(|ifaces| {
                ifaces
                    .iter()
                    .enumerate()
                    .map(|(i, iface)| bt::NicInfo {
                        id: iface.name.clone().unwrap_or_else(|| format!("nic-{i}")),
                        mac_address: iface.mac_address.clone(),
                        tap: None,
                        speed_mbps: 0,
                    })
                    .collect()
            })
            .unwrap_or_default();

        let secure_boot = domain
            .firmware
            .as_ref()
            .and_then(|f| f.bootloader.as_ref())
            .and_then(|b| b.efi.as_ref())
            .and_then(|e| e.secure_boot);

        (cpu_count, memory_bytes, disks, nics, secure_boot)
    }

    async fn subresource_put(
        &self,
        resource: &str,
        ns: &str,
        name: &str,
        action: &str,
        body: Vec<u8>,
    ) -> Result<(), BackendError> {
        let url = format!("/apis/kubevirt.io/v1/namespaces/{ns}/{resource}/{name}/{action}");
        let req = http::Request::put(url)
            .body(body)
            .map_err(|e| BackendError::ApiError(e.to_string()))?;
        self.client
            .request::<serde_json::Value>(req)
            .await
            .map_err(map_kube_error)?;
        Ok(())
    }
}

fn parse_memory_string(s: &str) -> u64 {
    let s = s.trim();
    if let Some(num) = s.strip_suffix("Gi") {
        num.parse::<u64>().unwrap_or(0) * 1024 * 1024 * 1024
    } else if let Some(num) = s.strip_suffix("Mi") {
        num.parse::<u64>().unwrap_or(0) * 1024 * 1024
    } else if let Some(num) = s.strip_suffix("Ki") {
        num.parse::<u64>().unwrap_or(0) * 1024
    } else if let Some(num) = s.strip_suffix('G') {
        num.parse::<u64>().unwrap_or(0) * 1_000_000_000
    } else if let Some(num) = s.strip_suffix('M') {
        num.parse::<u64>().unwrap_or(0) * 1_000_000
    } else if let Some(num) = s.strip_suffix('K') {
        num.parse::<u64>().unwrap_or(0) * 1_000
    } else {
        s.parse::<u64>().unwrap_or(0)
    }
}

impl VmmBackend for KubeVirtBackend {
    async fn vm_info(&self, system_id: &str) -> Result<bt::VmInfo, BackendError> {
        let m = self.mapping_for(system_id)?;
        let vmi_api = self.vmi_api(&m.namespace);

        match vmi_api.get(&m.vm_name).await {
            Ok(vmi) => {
                let phase = vmi
                    .status
                    .as_ref()
                    .and_then(|s| s.phase.as_deref())
                    .unwrap_or("Unknown");
                let power_state = types::phase_to_power_state(phase);

                let domain = vmi.spec.domain.as_ref();
                let (cpu_count, memory_bytes, disks, nics, secure_boot) = domain
                    .map(Self::extract_info_from_domain)
                    .unwrap_or_default();

                Ok(bt::VmInfo {
                    power_state,
                    cpu_count,
                    max_cpu_count: cpu_count,
                    cpu_topology: None,
                    memory_bytes,
                    memory_actual_bytes: None,
                    secure_boot,
                    disks,
                    nics,
                    pci_devices: vec![],
                    uuid: None,
                    raw: None,
                })
            }
            Err(_) => {
                // VMI doesn't exist — VM is off; fall back to VM spec
                let vm_api = self.vm_api(&m.namespace);
                let vm = vm_api.get(&m.vm_name).await.map_err(map_kube_error)?;

                let domain = vm
                    .spec
                    .template
                    .as_ref()
                    .and_then(|t| t.spec.as_ref())
                    .and_then(|s| s.domain.as_ref());

                let (cpu_count, memory_bytes, disks, nics, secure_boot) = domain
                    .map(Self::extract_info_from_domain)
                    .unwrap_or_default();

                Ok(bt::VmInfo {
                    power_state: bt::VmPowerState::Off,
                    cpu_count,
                    max_cpu_count: cpu_count,
                    cpu_topology: None,
                    memory_bytes,
                    memory_actual_bytes: None,
                    secure_boot,
                    disks,
                    nics,
                    pci_devices: vec![],
                    uuid: None,
                    raw: None,
                })
            }
        }
    }

    async fn vm_create(
        &self,
        _system_id: &str,
        _config: bt::VmCreateConfig,
    ) -> Result<(), BackendError> {
        Err(BackendError::NotSupported(
            "VMs are managed externally via kubectl or GitOps".to_string(),
        ))
    }

    async fn vm_boot(&self, system_id: &str) -> Result<(), BackendError> {
        let m = self.mapping_for(system_id)?;
        self.subresource_put("virtualmachines", &m.namespace, &m.vm_name, "start", vec![])
            .await
    }

    async fn vm_shutdown(&self, system_id: &str) -> Result<(), BackendError> {
        let m = self.mapping_for(system_id)?;
        self.subresource_put("virtualmachines", &m.namespace, &m.vm_name, "stop", vec![])
            .await
    }

    async fn vm_delete(&self, system_id: &str) -> Result<(), BackendError> {
        let m = self.mapping_for(system_id)?;
        let api = self.vm_api(&m.namespace);
        api.delete(&m.vm_name, &DeleteParams::default())
            .await
            .map_err(map_kube_error)?;
        Ok(())
    }

    async fn vm_power_button(&self, system_id: &str) -> Result<(), BackendError> {
        let m = self.mapping_for(system_id)?;
        self.subresource_put(
            "virtualmachineinstances",
            &m.namespace,
            &m.vm_name,
            "softreboot",
            vec![],
        )
        .await
    }

    async fn vm_reboot(&self, system_id: &str) -> Result<(), BackendError> {
        let m = self.mapping_for(system_id)?;
        self.subresource_put(
            "virtualmachines",
            &m.namespace,
            &m.vm_name,
            "restart",
            vec![],
        )
        .await
    }

    async fn vm_add_disk(
        &self,
        system_id: &str,
        disk: bt::DiskCreateConfig,
    ) -> Result<(), BackendError> {
        let m = self.mapping_for(system_id)?;
        let disk_id = disk.id.unwrap_or_else(|| "hotplug-disk".to_string());
        let dv_name = disk.path.unwrap_or_else(|| disk_id.clone());

        let body = serde_json::json!({
            "name": disk_id,
            "disk": {
                "name": disk_id,
                "bus": "virtio"
            },
            "volumeSource": {
                "dataVolume": {
                    "name": dv_name
                }
            }
        });

        self.subresource_put(
            "virtualmachines",
            &m.namespace,
            &m.vm_name,
            "addvolume",
            serde_json::to_vec(&body).map_err(|e| BackendError::ApiError(e.to_string()))?,
        )
        .await
    }

    async fn vm_remove_device(&self, system_id: &str, device_id: &str) -> Result<(), BackendError> {
        let m = self.mapping_for(system_id)?;
        let body = serde_json::json!({
            "name": device_id
        });

        self.subresource_put(
            "virtualmachines",
            &m.namespace,
            &m.vm_name,
            "removevolume",
            serde_json::to_vec(&body).map_err(|e| BackendError::ApiError(e.to_string()))?,
        )
        .await
    }

    async fn vmm_ping(&self, _system_id: &str) -> Result<bt::VmmPingResponse, BackendError> {
        let url = "/apis/kubevirt.io/v1";
        let req = http::Request::get(url)
            .body(vec![])
            .map_err(|e| BackendError::ApiError(e.to_string()))?;
        let resp: serde_json::Value = self.client.request(req).await.map_err(map_kube_error)?;

        let version = resp
            .get("gitVersion")
            .or_else(|| resp.get("groupVersion"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        Ok(bt::VmmPingResponse { version, pid: None })
    }

    async fn vm_counters(&self, _system_id: &str) -> Result<bt::VmCounters, BackendError> {
        Err(BackendError::NotSupported(
            "counters not available via KubeVirt API".to_string(),
        ))
    }

    async fn vm_set_secure_boot(&self, system_id: &str, enabled: bool) -> Result<(), BackendError> {
        let m = self.mapping_for(system_id)?;
        let api = self.vm_api(&m.namespace);

        let patch = serde_json::json!({
            "spec": {
                "template": {
                    "spec": {
                        "domain": {
                            "firmware": {
                                "bootloader": {
                                    "efi": {
                                        "secureBoot": enabled
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });

        api.patch(&m.vm_name, &PatchParams::default(), &Patch::Merge(patch))
            .await
            .map_err(map_kube_error)?;

        Ok(())
    }

    async fn vm_serial_console(
        &self,
        _system_id: &str,
    ) -> Result<bt::SerialConsoleInfo, BackendError> {
        Err(BackendError::NotSupported("serial console".to_string()))
    }
}

pub async fn build_backend(config: &AppConfig) -> Result<super::Backend, BackendError> {
    let client = kube::Client::try_default()
        .await
        .map_err(|e| BackendError::ConnectionFailed(format!("kube: {e}")))?;

    let vms: HashMap<String, VmMapping> = config
        .systems
        .iter()
        .map(|(id, sys)| {
            let namespace = sys
                .namespace
                .clone()
                .unwrap_or_else(|| "default".to_string());
            let vm_name = sys.vm_name.clone().unwrap_or_else(|| id.clone());
            (id.clone(), VmMapping { namespace, vm_name })
        })
        .collect();

    Ok(super::Backend::KubeVirt(KubeVirtBackend { client, vms }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_phase_to_power_state() {
        assert_eq!(types::phase_to_power_state("Running"), bt::VmPowerState::On);
        assert_eq!(
            types::phase_to_power_state("Succeeded"),
            bt::VmPowerState::Off
        );
        assert_eq!(types::phase_to_power_state("Failed"), bt::VmPowerState::Off);
        assert_eq!(
            types::phase_to_power_state("Scheduling"),
            bt::VmPowerState::Unknown
        );
        assert_eq!(
            types::phase_to_power_state("Scheduled"),
            bt::VmPowerState::Unknown
        );
        assert_eq!(
            types::phase_to_power_state("Pending"),
            bt::VmPowerState::Unknown
        );
        assert_eq!(
            types::phase_to_power_state("SomethingElse"),
            bt::VmPowerState::Unknown
        );
    }

    #[test]
    fn test_parse_memory_string() {
        assert_eq!(parse_memory_string("1Gi"), 1024 * 1024 * 1024);
        assert_eq!(parse_memory_string("512Mi"), 512 * 1024 * 1024);
        assert_eq!(parse_memory_string("2G"), 2_000_000_000);
        assert_eq!(parse_memory_string("100Ki"), 100 * 1024);
        assert_eq!(parse_memory_string("1000"), 1000);
        assert_eq!(parse_memory_string("bad"), 0);
    }

    #[test]
    fn test_parse_kubevirt_config() {
        let config =
            AppConfig::load(std::path::Path::new("examples/config-kubevirt.toml")).unwrap();
        assert_eq!(config.backend, crate::config::BackendType::KubeVirt);
        assert_eq!(config.server.port, 8000);
        assert_eq!(config.systems.len(), 1);

        let vm1 = &config.systems["vm1"];
        assert_eq!(vm1.name.as_deref(), Some("KubeVirt VM 1"));
        assert_eq!(vm1.namespace.as_deref(), Some("default"));
        assert_eq!(vm1.vm_name.as_deref(), Some("my-test-vm"));
        assert_eq!(vm1.hardware.cpu_count, 2);
        assert_eq!(vm1.hardware.memory_mib, 2048);
    }
}
