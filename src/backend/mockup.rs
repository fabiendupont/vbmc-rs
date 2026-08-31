use std::path::Path;
use std::sync::Arc;

use dashmap::DashMap;
use tracing::info;

use super::types as bt;
use super::{BackendError, VmmBackend};

pub struct MockupStore {
    resources: DashMap<String, serde_json::Value>,
}

impl MockupStore {
    pub fn generate(count: usize) -> Self {
        let store = Self {
            resources: DashMap::new(),
        };

        let mut members = Vec::new();
        for i in 1..=count {
            let id = format!("Server{i}");
            let uuid = uuid::Uuid::new_v5(
                &uuid::Uuid::NAMESPACE_DNS,
                format!("vbmc-rs-simulate-{i}").as_bytes(),
            );
            let serial = format!("VBMC{i:06}");
            let mac1 = format!("52:54:00:00:{:02x}:{:02x}", i / 256, i % 256);
            let mac2 = format!("52:54:00:01:{:02x}:{:02x}", i / 256, i % 256);

            store.resources.insert(
                format!("/redfish/v1/Systems/{id}"),
                serde_json::json!({
                    "@odata.id": format!("/redfish/v1/Systems/{id}"),
                    "@odata.type": "#ComputerSystem.v1_20_0.ComputerSystem",
                    "Id": id,
                    "Name": format!("Simulated Server {i}"),
                    "SystemType": "Physical",
                    "Manufacturer": "vbmc-rs",
                    "Model": "Virtual Server 1U",
                    "SerialNumber": serial,
                    "UUID": uuid.to_string(),
                    "PowerState": "Off",
                    "BiosVersion": "vbmc-rs 0.1.0",
                    "ProcessorSummary": {
                        "Count": 2,
                        "Model": "Virtual CPU",
                        "LogicalProcessorCount": 4,
                        "Status": {"State": "Enabled", "Health": "OK"}
                    },
                    "MemorySummary": {
                        "TotalSystemMemoryGiB": 64,
                        "Status": {"State": "Enabled", "Health": "OK"}
                    },
                    "Boot": {
                        "BootSourceOverrideTarget": "None",
                        "BootSourceOverrideEnabled": "Disabled",
                        "BootSourceOverrideMode": "UEFI",
                        "BootSourceOverrideTarget@Redfish.AllowableValues": ["None", "Pxe", "Hdd", "Cd"]
                    },
                    "Status": {"State": "Enabled", "Health": "OK"},
                    "Actions": {
                        "#ComputerSystem.Reset": {
                            "target": format!("/redfish/v1/Systems/{id}/Actions/ComputerSystem.Reset"),
                            "ResetType@Redfish.AllowableValues": ["On", "ForceOff", "GracefulShutdown", "GracefulRestart", "ForceRestart", "PushPowerButton"]
                        }
                    },
                    "Processors": {"@odata.id": format!("/redfish/v1/Systems/{id}/Processors")},
                    "Memory": {"@odata.id": format!("/redfish/v1/Systems/{id}/Memory")},
                    "EthernetInterfaces": {"@odata.id": format!("/redfish/v1/Systems/{id}/EthernetInterfaces")},
                    "Storage": {"@odata.id": format!("/redfish/v1/Systems/{id}/Storage")},
                    "Bios": {"@odata.id": format!("/redfish/v1/Systems/{id}/Bios")},
                    "SecureBoot": {"@odata.id": format!("/redfish/v1/Systems/{id}/SecureBoot")}
                }),
            );

            store.resources.insert(
                format!("/redfish/v1/Systems/{id}/Processors"),
                serde_json::json!({
                    "@odata.id": format!("/redfish/v1/Systems/{id}/Processors"),
                    "@odata.type": "#ProcessorCollection.ProcessorCollection",
                    "Name": "Processor Collection",
                    "Members": [{"@odata.id": format!("/redfish/v1/Systems/{id}/Processors/CPU0")}],
                    "Members@odata.count": 1
                }),
            );

            store.resources.insert(
                format!("/redfish/v1/Systems/{id}/Processors/CPU0"),
                serde_json::json!({
                    "@odata.id": format!("/redfish/v1/Systems/{id}/Processors/CPU0"),
                    "@odata.type": "#Processor.v1_18_0.Processor",
                    "Id": "CPU0",
                    "Name": "CPU 0",
                    "ProcessorType": "CPU",
                    "TotalCores": 2,
                    "TotalThreads": 4,
                    "MaxSpeedMHz": 3600,
                    "Manufacturer": "vbmc-rs",
                    "Model": "Virtual CPU",
                    "InstructionSet": "x86-64",
                    "Status": {"State": "Enabled", "Health": "OK"}
                }),
            );

            store.resources.insert(
                format!("/redfish/v1/Systems/{id}/Memory"),
                serde_json::json!({
                    "@odata.id": format!("/redfish/v1/Systems/{id}/Memory"),
                    "@odata.type": "#MemoryCollection.MemoryCollection",
                    "Name": "Memory Collection",
                    "Members": [{"@odata.id": format!("/redfish/v1/Systems/{id}/Memory/DIMM0")}],
                    "Members@odata.count": 1
                }),
            );

            store.resources.insert(
                format!("/redfish/v1/Systems/{id}/Memory/DIMM0"),
                serde_json::json!({
                    "@odata.id": format!("/redfish/v1/Systems/{id}/Memory/DIMM0"),
                    "@odata.type": "#Memory.v1_16_0.Memory",
                    "Id": "DIMM0",
                    "Name": "DIMM 0",
                    "CapacityMiB": 65536,
                    "MemoryDeviceType": "DDR5",
                    "DataWidthBits": 64,
                    "OperatingSpeedMhz": 4800,
                    "Manufacturer": "Virtual",
                    "Status": {"State": "Enabled", "Health": "OK"}
                }),
            );

            store.resources.insert(
                format!("/redfish/v1/Systems/{id}/EthernetInterfaces"),
                serde_json::json!({
                    "@odata.id": format!("/redfish/v1/Systems/{id}/EthernetInterfaces"),
                    "@odata.type": "#EthernetInterfaceCollection.EthernetInterfaceCollection",
                    "Name": "Ethernet Interface Collection",
                    "Members": [
                        {"@odata.id": format!("/redfish/v1/Systems/{id}/EthernetInterfaces/NIC0")},
                        {"@odata.id": format!("/redfish/v1/Systems/{id}/EthernetInterfaces/NIC1")}
                    ],
                    "Members@odata.count": 2
                }),
            );

            store.resources.insert(
                format!("/redfish/v1/Systems/{id}/EthernetInterfaces/NIC0"),
                serde_json::json!({
                    "@odata.id": format!("/redfish/v1/Systems/{id}/EthernetInterfaces/NIC0"),
                    "@odata.type": "#EthernetInterface.v1_9_0.EthernetInterface",
                    "Id": "NIC0",
                    "Name": "Ethernet Interface 0",
                    "MACAddress": mac1,
                    "SpeedMbps": 25000,
                    "Status": {"State": "Enabled", "Health": "OK"}
                }),
            );

            store.resources.insert(
                format!("/redfish/v1/Systems/{id}/EthernetInterfaces/NIC1"),
                serde_json::json!({
                    "@odata.id": format!("/redfish/v1/Systems/{id}/EthernetInterfaces/NIC1"),
                    "@odata.type": "#EthernetInterface.v1_9_0.EthernetInterface",
                    "Id": "NIC1",
                    "Name": "Ethernet Interface 1",
                    "MACAddress": mac2,
                    "SpeedMbps": 25000,
                    "Status": {"State": "Enabled", "Health": "OK"}
                }),
            );

            store.resources.insert(
                format!("/redfish/v1/Systems/{id}/Storage"),
                serde_json::json!({
                    "@odata.id": format!("/redfish/v1/Systems/{id}/Storage"),
                    "@odata.type": "#StorageCollection.StorageCollection",
                    "Name": "Storage Collection",
                    "Members": [{"@odata.id": format!("/redfish/v1/Systems/{id}/Storage/NVMe")}],
                    "Members@odata.count": 1
                }),
            );

            store.resources.insert(
                format!("/redfish/v1/Systems/{id}/SecureBoot"),
                serde_json::json!({
                    "@odata.id": format!("/redfish/v1/Systems/{id}/SecureBoot"),
                    "@odata.type": "#SecureBoot.v1_1_0.SecureBoot",
                    "Id": "SecureBoot",
                    "Name": "UEFI Secure Boot",
                    "SecureBootEnable": false,
                    "SecureBootCurrentBoot": "Disabled",
                    "SecureBootMode": "UserMode"
                }),
            );

            members.push(serde_json::json!({"@odata.id": format!("/redfish/v1/Systems/{id}")}));
        }

        store.resources.insert(
            "/redfish/v1/Systems".to_string(),
            serde_json::json!({
                "@odata.id": "/redfish/v1/Systems",
                "@odata.type": "#ComputerSystemCollection.ComputerSystemCollection",
                "Name": "Computer System Collection",
                "Members": members,
                "Members@odata.count": count
            }),
        );

        store.resources.insert(
            "/redfish/v1".to_string(),
            serde_json::json!({
                "@odata.id": "/redfish/v1",
                "@odata.type": "#ServiceRoot.v1_16_0.ServiceRoot",
                "Id": "RootService",
                "Name": "vbmc-rs Simulated BMC",
                "RedfishVersion": "1.21.0",
                "UUID": uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_DNS, b"vbmc-rs-simulate").to_string(),
                "Systems": {"@odata.id": "/redfish/v1/Systems"},
                "Chassis": {"@odata.id": "/redfish/v1/Chassis"},
                "Managers": {"@odata.id": "/redfish/v1/Managers"}
            }),
        );

        store.resources.insert(
            "/redfish".to_string(),
            serde_json::json!({"v1": "/redfish/v1"}),
        );

        info!(systems = count, "Generated simulated BMC fleet");
        store
    }

    pub fn load(dir: &Path) -> anyhow::Result<Self> {
        let store = Self {
            resources: DashMap::new(),
        };
        let dir_str = dir
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("mockup directory path is not valid UTF-8"))?;

        let mut count = 0;
        for entry in walkdir(dir)? {
            let rel = entry
                .strip_prefix(dir_str)
                .unwrap_or(&entry)
                .trim_start_matches('/');

            let path = if let Some(stripped) = rel.strip_suffix("/index.json") {
                format!("/{stripped}")
            } else if rel == "index.json" {
                "/".to_string()
            } else {
                continue;
            };

            let content = std::fs::read_to_string(&entry)?;
            let json: serde_json::Value = serde_json::from_str(&content)?;
            store.resources.insert(path, json);
            count += 1;
        }

        info!(directory = %dir.display(), resources = count, "Loaded mockup data");
        Ok(store)
    }

    pub fn get(&self, path: &str) -> Option<serde_json::Value> {
        self.resources.get(path).map(|v| v.clone())
    }

    pub fn patch(&self, path: &str, patch: &serde_json::Value) {
        if let Some(mut entry) = self.resources.get_mut(path) {
            merge_json(entry.value_mut(), patch);
        }
    }

    pub fn system_ids(&self) -> Vec<String> {
        let mut ids = Vec::new();
        for entry in self.resources.iter() {
            let path = entry.key();
            if let Some(rest) = path.strip_prefix("/redfish/v1/Systems/")
                && !rest.contains('/')
                && !rest.is_empty()
            {
                ids.push(rest.to_string());
            }
        }
        ids
    }

    fn system_path(&self, system_id: &str) -> String {
        format!("/redfish/v1/Systems/{system_id}")
    }

    fn get_system(&self, system_id: &str) -> Option<serde_json::Value> {
        self.get(&self.system_path(system_id))
    }

    fn set_power_state(&self, system_id: &str, state: &str) {
        let path = self.system_path(system_id);
        if let Some(mut entry) = self.resources.get_mut(&path)
            && let Some(obj) = entry.value_mut().as_object_mut()
        {
            obj.insert("PowerState".to_string(), serde_json::json!(state));
        }
    }
}

fn merge_json(target: &mut serde_json::Value, patch: &serde_json::Value) {
    if let (Some(target_obj), Some(patch_obj)) = (target.as_object_mut(), patch.as_object()) {
        for (key, value) in patch_obj {
            if value.is_null() {
                target_obj.remove(key);
            } else if value.is_object() && target_obj.get(key).is_some_and(|v| v.is_object()) {
                merge_json(target_obj.get_mut(key).expect("checked above"), value);
            } else {
                target_obj.insert(key.clone(), value.clone());
            }
        }
    }
}

fn walkdir(dir: &Path) -> anyhow::Result<Vec<String>> {
    let mut files = Vec::new();
    walkdir_inner(dir, &mut files)?;
    Ok(files)
}

fn walkdir_inner(dir: &Path, files: &mut Vec<String>) -> anyhow::Result<()> {
    if !dir.is_dir() {
        anyhow::bail!("not a directory: {}", dir.display());
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            walkdir_inner(&path, files)?;
        } else if path.file_name().is_some_and(|n| n == "index.json")
            && let Some(s) = path.to_str()
        {
            files.push(s.to_string());
        }
    }
    Ok(())
}

pub struct MockupBackend {
    store: Arc<MockupStore>,
}

impl MockupBackend {
    pub fn new(store: Arc<MockupStore>) -> Self {
        Self { store }
    }

    pub fn store(&self) -> &Arc<MockupStore> {
        &self.store
    }
}

impl VmmBackend for MockupBackend {
    async fn vm_info(&self, system_id: &str) -> Result<bt::VmInfo, BackendError> {
        let sys = self
            .store
            .get_system(system_id)
            .ok_or(BackendError::VmNotFound)?;

        let power_state = match sys.get("PowerState").and_then(|v| v.as_str()) {
            Some("On") => bt::VmPowerState::On,
            Some("Off") | Some("GracefulShutdown") => bt::VmPowerState::Off,
            Some("Paused") => bt::VmPowerState::Paused,
            _ => bt::VmPowerState::Unknown,
        };

        let cpu_count = sys
            .pointer("/ProcessorSummary/Count")
            .and_then(|v| v.as_u64())
            .unwrap_or(1) as u32;

        let memory_gib = sys
            .pointer("/MemorySummary/TotalSystemMemoryGiB")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let memory_bytes = (memory_gib * 1024.0 * 1024.0 * 1024.0) as u64;

        let secure_boot = sys
            .pointer("/SecureBoot/SecureBootEnable")
            .and_then(|v| v.as_bool());

        let disks = self.extract_disks(system_id);
        let nics = self.extract_nics(system_id);

        Ok(bt::VmInfo {
            power_state,
            cpu_count,
            max_cpu_count: cpu_count,
            cpu_topology: None,
            memory_bytes,
            memory_actual_bytes: Some(memory_bytes),
            secure_boot,
            disks,
            nics,
            pci_devices: vec![],
            uuid: sys.get("UUID").and_then(|v| v.as_str()).map(String::from),
            raw: Some(sys),
        })
    }

    async fn vm_create(
        &self,
        _system_id: &str,
        _config: bt::VmCreateConfig,
    ) -> Result<(), BackendError> {
        Err(BackendError::NotSupported(
            "vm_create not supported in mockup backend".to_string(),
        ))
    }

    async fn vm_boot(&self, system_id: &str) -> Result<(), BackendError> {
        if self.store.get_system(system_id).is_none() {
            return Err(BackendError::VmNotFound);
        }
        self.store.set_power_state(system_id, "On");
        Ok(())
    }

    async fn vm_shutdown(&self, system_id: &str) -> Result<(), BackendError> {
        if self.store.get_system(system_id).is_none() {
            return Err(BackendError::VmNotFound);
        }
        self.store.set_power_state(system_id, "Off");
        Ok(())
    }

    async fn vm_delete(&self, _system_id: &str) -> Result<(), BackendError> {
        Err(BackendError::NotSupported(
            "vm_delete not supported in mockup backend".to_string(),
        ))
    }

    async fn vm_power_button(&self, system_id: &str) -> Result<(), BackendError> {
        if self.store.get_system(system_id).is_none() {
            return Err(BackendError::VmNotFound);
        }
        self.store.set_power_state(system_id, "Off");
        Ok(())
    }

    async fn vm_reboot(&self, system_id: &str) -> Result<(), BackendError> {
        if self.store.get_system(system_id).is_none() {
            return Err(BackendError::VmNotFound);
        }
        self.store.set_power_state(system_id, "On");
        Ok(())
    }

    async fn vm_add_disk(
        &self,
        _system_id: &str,
        _disk: bt::DiskCreateConfig,
    ) -> Result<(), BackendError> {
        Err(BackendError::NotSupported(
            "vm_add_disk not supported in mockup backend".to_string(),
        ))
    }

    async fn vm_remove_device(
        &self,
        _system_id: &str,
        _device_id: &str,
    ) -> Result<(), BackendError> {
        Err(BackendError::NotSupported(
            "vm_remove_device not supported in mockup backend".to_string(),
        ))
    }

    async fn vmm_ping(&self, _system_id: &str) -> Result<bt::VmmPingResponse, BackendError> {
        let version = self.store.get("/redfish/v1").and_then(|v| {
            v.get("RedfishVersion")
                .and_then(|v| v.as_str())
                .map(String::from)
        });
        Ok(bt::VmmPingResponse { version, pid: None })
    }

    async fn vm_counters(&self, _system_id: &str) -> Result<bt::VmCounters, BackendError> {
        Ok(bt::VmCounters::default())
    }

    async fn vm_set_secure_boot(&self, system_id: &str, enabled: bool) -> Result<(), BackendError> {
        let sb_path = format!("/redfish/v1/Systems/{system_id}/SecureBoot");
        self.store
            .patch(&sb_path, &serde_json::json!({"SecureBootEnable": enabled}));
        Ok(())
    }

    async fn vm_serial_console(
        &self,
        _system_id: &str,
    ) -> Result<bt::SerialConsoleInfo, BackendError> {
        Err(BackendError::NotSupported(
            "serial console not available in mockup backend".to_string(),
        ))
    }
}

impl MockupBackend {
    fn extract_disks(&self, system_id: &str) -> Vec<bt::DiskInfo> {
        let mut disks = Vec::new();
        let storage_path = format!("/redfish/v1/Systems/{system_id}/Storage");
        if let Some(collection) = self.store.get(&storage_path)
            && let Some(members) = collection.get("Members").and_then(|m| m.as_array())
        {
            for (i, member) in members.iter().enumerate() {
                let id = member
                    .get("@odata.id")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.rsplit('/').next())
                    .unwrap_or("disk")
                    .to_string();
                disks.push(bt::DiskInfo {
                    id: format!("{id}-{i}"),
                    path: None,
                    capacity_bytes: None,
                    readonly: false,
                    protocol: bt::DiskProtocol::Virtio,
                    media_type: bt::DiskMediaType::Virtual,
                });
            }
        }
        if disks.is_empty() {
            disks.push(bt::DiskInfo {
                id: "disk-0".to_string(),
                path: None,
                capacity_bytes: None,
                readonly: false,
                protocol: bt::DiskProtocol::Virtio,
                media_type: bt::DiskMediaType::Virtual,
            });
        }
        disks
    }

    fn extract_nics(&self, system_id: &str) -> Vec<bt::NicInfo> {
        let mut nics = Vec::new();
        let nic_path = format!("/redfish/v1/Systems/{system_id}/EthernetInterfaces");
        if let Some(collection) = self.store.get(&nic_path)
            && let Some(members) = collection.get("Members").and_then(|m| m.as_array())
        {
            for member in members {
                let id = member
                    .get("@odata.id")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.rsplit('/').next())
                    .unwrap_or("NIC0")
                    .to_string();
                nics.push(bt::NicInfo {
                    id,
                    mac_address: None,
                    tap: None,
                    speed_mbps: 0,
                });
            }
        }
        nics
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_mockup_dir() -> TempDir {
        let dir = TempDir::new().unwrap();

        let service_root = dir.path().join("redfish/v1");
        std::fs::create_dir_all(&service_root).unwrap();
        std::fs::write(
            service_root.join("index.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "@odata.id": "/redfish/v1",
                "@odata.type": "#ServiceRoot.v1_10_0.ServiceRoot",
                "RedfishVersion": "1.21.0",
                "Systems": {"@odata.id": "/redfish/v1/Systems"},
            }))
            .unwrap(),
        )
        .unwrap();

        let systems = dir.path().join("redfish/v1/Systems");
        std::fs::create_dir_all(&systems).unwrap();
        std::fs::write(
            systems.join("index.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "@odata.id": "/redfish/v1/Systems",
                "Members": [{"@odata.id": "/redfish/v1/Systems/Server1"}],
                "Members@odata.count": 1,
            }))
            .unwrap(),
        )
        .unwrap();

        let system1 = dir.path().join("redfish/v1/Systems/Server1");
        std::fs::create_dir_all(&system1).unwrap();
        std::fs::write(
            system1.join("index.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "@odata.id": "/redfish/v1/Systems/Server1",
                "@odata.type": "#ComputerSystem.v1_20_0.ComputerSystem",
                "Id": "Server1",
                "Name": "Test Server",
                "PowerState": "On",
                "SystemType": "Physical",
                "ProcessorSummary": {"Count": 2},
                "MemorySummary": {"TotalSystemMemoryGiB": 64.0},
                "UUID": "12345678-1234-1234-1234-123456789012",
            }))
            .unwrap(),
        )
        .unwrap();

        dir
    }

    #[test]
    fn test_load_mockup() {
        let dir = create_mockup_dir();
        let store = MockupStore::load(dir.path()).unwrap();

        assert!(store.get("/redfish/v1").is_some());
        assert!(store.get("/redfish/v1/Systems").is_some());
        assert!(store.get("/redfish/v1/Systems/Server1").is_some());
        assert!(store.get("/nonexistent").is_none());
    }

    #[test]
    fn test_system_ids() {
        let dir = create_mockup_dir();
        let store = MockupStore::load(dir.path()).unwrap();
        let ids = store.system_ids();
        assert_eq!(ids, vec!["Server1"]);
    }

    #[test]
    fn test_set_power_state() {
        let dir = create_mockup_dir();
        let store = MockupStore::load(dir.path()).unwrap();

        let sys = store.get_system("Server1").unwrap();
        assert_eq!(sys["PowerState"], "On");

        store.set_power_state("Server1", "Off");
        let sys = store.get_system("Server1").unwrap();
        assert_eq!(sys["PowerState"], "Off");
    }

    #[test]
    fn test_patch() {
        let dir = create_mockup_dir();
        let store = MockupStore::load(dir.path()).unwrap();

        store.patch(
            "/redfish/v1/Systems/Server1",
            &serde_json::json!({"AssetTag": "MyTag"}),
        );
        let sys = store.get("/redfish/v1/Systems/Server1").unwrap();
        assert_eq!(sys["AssetTag"], "MyTag");
        assert_eq!(sys["PowerState"], "On");
    }

    #[test]
    fn test_merge_json_nested() {
        let mut target = serde_json::json!({"a": {"b": 1, "c": 2}, "d": 3});
        let patch = serde_json::json!({"a": {"b": 10}, "e": 5});
        merge_json(&mut target, &patch);
        assert_eq!(target["a"]["b"], 10);
        assert_eq!(target["a"]["c"], 2);
        assert_eq!(target["d"], 3);
        assert_eq!(target["e"], 5);
    }

    #[test]
    fn test_merge_json_delete() {
        let mut target = serde_json::json!({"a": 1, "b": 2});
        let patch = serde_json::json!({"b": null});
        merge_json(&mut target, &patch);
        assert_eq!(target["a"], 1);
        assert!(target.get("b").is_none());
    }

    #[tokio::test]
    async fn test_mockup_backend_vm_info() {
        let dir = create_mockup_dir();
        let store = Arc::new(MockupStore::load(dir.path()).unwrap());
        let backend = MockupBackend::new(store);

        let info = backend.vm_info("Server1").await.unwrap();
        assert_eq!(info.power_state, bt::VmPowerState::On);
        assert_eq!(info.cpu_count, 2);
        assert_eq!(info.memory_bytes, 64 * 1024 * 1024 * 1024);
        assert_eq!(
            info.uuid.as_deref(),
            Some("12345678-1234-1234-1234-123456789012")
        );
    }

    #[tokio::test]
    async fn test_mockup_backend_vm_not_found() {
        let dir = create_mockup_dir();
        let store = Arc::new(MockupStore::load(dir.path()).unwrap());
        let backend = MockupBackend::new(store);

        let result = backend.vm_info("Nonexistent").await;
        assert!(matches!(result, Err(BackendError::VmNotFound)));
    }

    #[tokio::test]
    async fn test_mockup_backend_boot_shutdown() {
        let dir = create_mockup_dir();
        let store = Arc::new(MockupStore::load(dir.path()).unwrap());
        let backend = MockupBackend::new(store);

        let info = backend.vm_info("Server1").await.unwrap();
        assert_eq!(info.power_state, bt::VmPowerState::On);

        backend.vm_shutdown("Server1").await.unwrap();
        let info = backend.vm_info("Server1").await.unwrap();
        assert_eq!(info.power_state, bt::VmPowerState::Off);

        backend.vm_boot("Server1").await.unwrap();
        let info = backend.vm_info("Server1").await.unwrap();
        assert_eq!(info.power_state, bt::VmPowerState::On);
    }

    #[tokio::test]
    async fn test_mockup_backend_vmm_ping() {
        let dir = create_mockup_dir();
        let store = Arc::new(MockupStore::load(dir.path()).unwrap());
        let backend = MockupBackend::new(store);

        let ping = backend.vmm_ping("Server1").await.unwrap();
        assert_eq!(ping.version.as_deref(), Some("1.21.0"));
    }
}
