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
