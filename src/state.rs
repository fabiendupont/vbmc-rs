use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmState {
    pub system_id: String,
    #[serde(default)]
    pub boot_override: BootOverride,
    #[serde(default)]
    pub virtual_media: VirtualMediaState,
    #[serde(default)]
    pub secure_boot_enabled: bool,
    #[serde(default)]
    pub attestation: AttestationState,
    #[serde(default)]
    pub bios_settings: Option<crate::redfish::bios::BiosAttributes>,
    #[serde(default)]
    pub licenses: Vec<LicenseInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseInfo {
    pub id: String,
    pub name: String,
    pub license_type: String,
    pub license_string: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootOverride {
    pub target: Option<String>,
    #[serde(default = "default_boot_override_enabled")]
    pub enabled: String,
    pub mode: Option<String>,
}

fn default_boot_override_enabled() -> String {
    "Disabled".to_string()
}

impl Default for BootOverride {
    fn default() -> Self {
        Self {
            target: None,
            enabled: default_boot_override_enabled(),
            mode: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VirtualMediaState {
    pub inserted: bool,
    pub image_url: Option<String>,
    pub image_path: Option<PathBuf>,
    pub media_type: Option<String>,
    pub write_protected: bool,
    pub device_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AttestationState {
    pub verification_status: Option<String>,
    pub last_checked: Option<String>,
    pub component_integrity_id: Option<String>,
}

impl VmState {
    pub fn new(system_id: &str) -> Self {
        Self {
            system_id: system_id.to_string(),
            boot_override: BootOverride::default(),
            virtual_media: VirtualMediaState::default(),
            secure_boot_enabled: false,
            attestation: AttestationState::default(),
            bios_settings: None,
            licenses: Vec::new(),
        }
    }

    pub fn load(state_dir: &Path, system_id: &str) -> anyhow::Result<Self> {
        let path = state_dir.join(format!("{system_id}.json"));
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            let state: VmState = serde_json::from_str(&content)?;
            Ok(state)
        } else {
            Ok(Self::new(system_id))
        }
    }

    pub fn save(&self, state_dir: &Path) -> anyhow::Result<()> {
        std::fs::create_dir_all(state_dir)?;
        let path = state_dir.join(format!("{}.json", self.system_id));
        let tmp_path = state_dir.join(format!(".{}.json.tmp", self.system_id));
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&tmp_path, content)?;
        std::fs::rename(&tmp_path, &path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let mut state = VmState::new("test-vm");
        state.boot_override.target = Some("Cd".to_string());
        state.boot_override.enabled = "Once".to_string();
        state.virtual_media.inserted = true;
        state.virtual_media.image_url = Some("http://example.com/test.iso".to_string());
        state.secure_boot_enabled = true;

        state.save(dir.path()).unwrap();

        let loaded = VmState::load(dir.path(), "test-vm").unwrap();
        assert_eq!(loaded.system_id, "test-vm");
        assert_eq!(loaded.boot_override.target.as_deref(), Some("Cd"));
        assert_eq!(loaded.boot_override.enabled, "Once");
        assert!(loaded.virtual_media.inserted);
        assert!(loaded.secure_boot_enabled);
    }

    #[test]
    fn test_load_missing_returns_default() {
        let dir = tempfile::tempdir().unwrap();
        let state = VmState::load(dir.path(), "nonexistent").unwrap();
        assert_eq!(state.system_id, "nonexistent");
        assert!(!state.virtual_media.inserted);
        assert!(!state.secure_boot_enabled);
    }
}
