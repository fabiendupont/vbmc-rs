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
        // Power/lifecycle actions (start/stop/restart/softreboot) and volume
        // hot-plug (addvolume/removevolume) are KubeVirt *subresources*, served
        // under the subresources.kubevirt.io API group — not kubevirt.io, which
        // only carries the object + /status. Targeting kubevirt.io/v1/.../start
        // returns 404 on a real cluster (verified on CNV/KubeVirt).
        let url =
            format!("/apis/subresources.kubevirt.io/v1/namespaces/{ns}/{resource}/{name}/{action}");
        let req = http::Request::put(url)
            .body(body)
            .map_err(|e| BackendError::ApiError(e.to_string()))?;
        // KubeVirt power/lifecycle subresources answer 202 Accepted with an EMPTY
        // body, so request::<T> (which JSON-deserializes) fails with "EOF while
        // parsing a value". Use request_text and discard the (empty) body.
        self.client
            .request_text(req)
            .await
            .map_err(map_kube_error)?;
        Ok(())
    }

    async fn api_post(&self, url: &str, body: Vec<u8>) -> Result<(), BackendError> {
        let req = http::Request::post(url)
            .header("Content-Type", "application/json")
            .body(body)
            .map_err(|e| BackendError::ApiError(e.to_string()))?;
        // Discard the response body; tolerate empty (202/204) responses.
        self.client
            .request_text(req)
            .await
            .map_err(map_kube_error)?;
        Ok(())
    }

    async fn api_delete(&self, url: &str) -> Result<(), BackendError> {
        let req = http::Request::delete(url)
            .body(vec![])
            .map_err(|e| BackendError::ApiError(e.to_string()))?;
        // Discard the response body; tolerate empty (202/204) responses.
        self.client
            .request_text(req)
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

fn sanitize_k8s_name(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

impl VmmBackend for KubeVirtBackend {
    /// KubeVirt VirtualMachines are created out-of-band (kubectl/GitOps); this
    /// backend only drives their start/stop/restart subresources. Signals the
    /// Redfish reset handler to use power-only lifecycle (no create/delete).
    fn manages_existing_vms(&self) -> bool {
        true
    }

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
        system_id: &str,
    ) -> Result<bt::SerialConsoleInfo, BackendError> {
        let m = self.mapping_for(system_id)?;
        let url = format!(
            "wss://kubernetes.default.svc/apis/subresources.kubevirt.io/v1/namespaces/{}/virtualmachineinstances/{}/console",
            m.namespace, m.vm_name
        );
        Ok(bt::SerialConsoleInfo {
            pty_path: None,
            websocket_url: Some(url),
        })
    }

    async fn vm_insert_iso(
        &self,
        system_id: &str,
        image_url: &str,
        device_id: &str,
    ) -> Result<(), BackendError> {
        let m = self.mapping_for(system_id)?;
        let ns = &m.namespace;
        let vm_name = &m.vm_name;
        let dev = sanitize_k8s_name(device_id);
        let vis_name = format!("vbmc-iso-{vm_name}-{dev}");
        let pvc_name = format!("vbmc-media-{vm_name}-{dev}");

        // Create VolumeImportSource — CDI downloads the ISO into a Block PVC.
        let vis = serde_json::json!({
            "apiVersion": "cdi.kubevirt.io/v1beta1",
            "kind": "VolumeImportSource",
            "metadata": { "name": vis_name, "namespace": ns },
            "spec": { "source": { "http": { "url": image_url } } }
        });
        self.api_post(
            &format!("/apis/cdi.kubevirt.io/v1beta1/namespaces/{ns}/volumeimportsources"),
            serde_json::to_vec(&vis).map_err(|e| BackendError::ApiError(e.to_string()))?,
        )
        .await?;

        // Create PVC that CDI will populate from the VolumeImportSource.
        let pvc = serde_json::json!({
            "apiVersion": "v1",
            "kind": "PersistentVolumeClaim",
            "metadata": { "name": pvc_name, "namespace": ns },
            "spec": {
                "accessModes": ["ReadWriteOnce"],
                "volumeMode": "Block",
                "dataSourceRef": {
                    "apiGroup": "cdi.kubevirt.io",
                    "kind": "VolumeImportSource",
                    "name": vis_name
                },
                "resources": { "requests": { "storage": "2Gi" } }
            }
        });
        self.api_post(
            &format!("/api/v1/namespaces/{ns}/persistentvolumeclaims"),
            serde_json::to_vec(&pvc).map_err(|e| BackendError::ApiError(e.to_string()))?,
        )
        .await?;

        // If the VM is running, hotplug the PVC as a CDRom.
        // If not running, patch the VM spec so the device is attached on next boot.
        if self.vmi_api(ns).get(vm_name).await.is_ok() {
            let body = serde_json::json!({
                "name": dev,
                "disk": { "name": dev, "cdrom": { "bus": "sata", "readonly": true } },
                "volumeSource": {
                    "persistentVolumeClaim": { "claimName": pvc_name, "readOnly": true }
                }
            });
            self.subresource_put(
                "virtualmachines",
                ns,
                vm_name,
                "addvolume",
                serde_json::to_vec(&body).map_err(|e| BackendError::ApiError(e.to_string()))?,
            )
            .await?;
        } else {
            let vm_api = self.vm_api(ns);
            let vm = vm_api.get(vm_name).await.map_err(map_kube_error)?;

            let template_spec = vm.spec.template.as_ref().and_then(|t| t.spec.as_ref());

            let mut disks: Vec<serde_json::Value> = template_spec
                .and_then(|s| s.domain.as_ref())
                .and_then(|d| d.devices.as_ref())
                .and_then(|d| d.disks.as_ref())
                .map(|v| {
                    v.iter()
                        .map(|d| serde_json::to_value(d).unwrap_or_default())
                        .collect()
                })
                .unwrap_or_default();
            disks.retain(|d| d.get("name").and_then(|n| n.as_str()) != Some(dev.as_str()));
            disks.push(serde_json::json!({
                "name": dev,
                "cdrom": { "bus": "sata", "readonly": true }
            }));

            let mut volumes: Vec<serde_json::Value> = template_spec
                .and_then(|s| s.volumes.as_ref())
                .map(|v| {
                    v.iter()
                        .map(|vol| serde_json::to_value(vol).unwrap_or_default())
                        .collect()
                })
                .unwrap_or_default();
            volumes.retain(|v| v.get("name").and_then(|n| n.as_str()) != Some(dev.as_str()));
            volumes.push(serde_json::json!({
                "name": dev,
                "persistentVolumeClaim": { "claimName": pvc_name, "readOnly": true }
            }));

            let patch = serde_json::json!({
                "spec": {
                    "template": {
                        "spec": {
                            "domain": { "devices": { "disks": disks } },
                            "volumes": volumes
                        }
                    }
                }
            });
            vm_api
                .patch(vm_name, &PatchParams::default(), &Patch::Merge(patch))
                .await
                .map_err(map_kube_error)?;
        }

        Ok(())
    }

    async fn vm_eject_iso(&self, system_id: &str, device_id: &str) -> Result<(), BackendError> {
        let m = self.mapping_for(system_id)?;
        let ns = &m.namespace;
        let vm_name = &m.vm_name;
        let dev = sanitize_k8s_name(device_id);
        let vis_name = format!("vbmc-iso-{vm_name}-{dev}");
        let pvc_name = format!("vbmc-media-{vm_name}-{dev}");

        // Hotunplug if running.
        if self.vmi_api(ns).get(vm_name).await.is_ok() {
            let body = serde_json::json!({ "name": dev });
            let _ = self
                .subresource_put(
                    "virtualmachines",
                    ns,
                    vm_name,
                    "removevolume",
                    serde_json::to_vec(&body).map_err(|e| BackendError::ApiError(e.to_string()))?,
                )
                .await;
        }

        // Delete PVC and VolumeImportSource (best-effort; ignore errors).
        let _ = self
            .api_delete(&format!(
                "/api/v1/namespaces/{ns}/persistentvolumeclaims/{pvc_name}"
            ))
            .await;
        let _ = self
            .api_delete(&format!(
                "/apis/cdi.kubevirt.io/v1beta1/namespaces/{ns}/volumeimportsources/{vis_name}"
            ))
            .await;

        Ok(())
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
