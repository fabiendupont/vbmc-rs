use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditLogTarget {
    #[default]
    File,
    Stdout,
    Both,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendType {
    #[default]
    CloudHypervisor,
    #[cfg(feature = "qemu")]
    Qemu,
    #[cfg(feature = "libvirt")]
    Libvirt,
    #[cfg(feature = "kubevirt")]
    KubeVirt,
}

impl BackendType {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::CloudHypervisor => "Cloud Hypervisor",
            #[cfg(feature = "qemu")]
            Self::Qemu => "QEMU",
            #[cfg(feature = "libvirt")]
            Self::Libvirt => "Libvirt",
            #[cfg(feature = "kubevirt")]
            Self::KubeVirt => "KubeVirt",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    #[serde(default)]
    pub backend: BackendType,
    #[serde(default)]
    pub auth: AuthConfig,
    #[serde(default)]
    pub defaults: DefaultsConfig,
    #[serde(default)]
    pub security_policy: SecurityPolicyConfig,
    #[serde(default)]
    pub state_directory: PathBuf,
    #[serde(default)]
    pub audit_log: PathBuf,
    #[serde(default)]
    pub audit_log_target: AuditLogTarget,
    #[serde(default)]
    pub metrics: MetricsConfig,
    #[serde(default)]
    pub location: LocationConfig,
    #[serde(default)]
    pub systems: HashMap<String, SystemConfig>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct LocationConfig {
    #[serde(default)]
    pub facility: Option<String>,
    #[serde(default)]
    pub room: Option<String>,
    #[serde(default)]
    pub row: Option<String>,
    #[serde(default)]
    pub rack: Option<String>,
    #[serde(default)]
    pub rack_offset: Option<u32>,
    #[serde(default)]
    pub city: Option<String>,
    #[serde(default)]
    pub state_or_province: Option<String>,
    #[serde(default)]
    pub country: Option<String>,
    #[serde(default)]
    pub postal_code: Option<String>,
    #[serde(default)]
    pub latitude: Option<f64>,
    #[serde(default)]
    pub longitude: Option<f64>,
    #[serde(default)]
    pub altitude_meters: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_bind_address")]
    pub bind_address: String,
    #[serde(default = "default_port")]
    pub port: u16,
    pub tls_cert: Option<PathBuf>,
    pub tls_key: Option<PathBuf>,
    pub tls_client_ca: Option<PathBuf>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct AuthConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_session_timeout")]
    pub session_timeout_seconds: u64,
    #[serde(default = "default_max_sessions")]
    pub max_sessions: usize,
    #[serde(default = "default_lockout_threshold")]
    pub lockout_threshold: u32,
    #[serde(default = "default_lockout_duration")]
    pub lockout_duration_seconds: u64,
    pub accounts_file: Option<PathBuf>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[allow(dead_code)]
pub struct DefaultsConfig {
    #[serde(default = "default_firmware_path")]
    pub firmware_path: String,
    #[serde(default = "default_secure_boot_firmware_path")]
    pub secure_boot_firmware_path: String,
    #[serde(default = "default_boot_source")]
    pub boot_source: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct SecurityPolicyConfig {
    #[serde(default)]
    pub spdm_enabled: bool,
    #[serde(default)]
    pub tls_minimum_version: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MetricsConfig {
    #[serde(default = "default_metrics_enabled")]
    pub enabled: bool,
    #[serde(default = "default_metrics_port")]
    pub port: u16,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            enabled: default_metrics_enabled(),
            port: default_metrics_port(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct HardwareConfig {
    #[serde(default = "default_cpu_count")]
    pub cpu_count: u8,
    #[serde(default)]
    pub max_cpu_count: Option<u8>,
    #[serde(default = "default_memory_mib")]
    pub memory_mib: u64,
    #[serde(default)]
    pub disks: Vec<DiskHardwareConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DiskHardwareConfig {
    pub path: String,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub readonly: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct SystemConfig {
    pub name: Option<String>,
    #[serde(default)]
    pub socket_path: Option<PathBuf>,
    pub firmware_path: Option<String>,
    #[serde(default)]
    pub boot_source: Option<String>,
    #[serde(default)]
    pub virtual_media_directory: Option<PathBuf>,
    #[serde(default)]
    pub hardware: HardwareConfig,
    #[serde(default)]
    pub connection_uri: Option<String>,
    #[serde(default)]
    pub domain_name: Option<String>,
    #[serde(default)]
    pub namespace: Option<String>,
    #[serde(default)]
    pub vm_name: Option<String>,
    #[serde(default)]
    pub attestation: Option<AttestationConfig>,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct AttestationConfig {
    pub provider: String,
    #[serde(default)]
    pub provider_url: String,
    pub agent_id: Option<String>,
    #[serde(default = "default_poll_interval")]
    pub poll_interval_seconds: u64,
    #[serde(default)]
    pub swtpm_socket: Option<String>,
    #[serde(default)]
    pub pcr_policy: Option<std::collections::HashMap<u32, String>>,
}

fn default_poll_interval() -> u64 {
    30
}

fn default_bind_address() -> String {
    "0.0.0.0".to_string()
}

fn default_port() -> u16 {
    8000
}

fn default_session_timeout() -> u64 {
    3600
}

fn default_max_sessions() -> usize {
    64
}

fn default_lockout_threshold() -> u32 {
    5
}

fn default_lockout_duration() -> u64 {
    300
}

fn default_firmware_path() -> String {
    "/usr/share/OVMF/OVMF_CODE.fd".to_string()
}

fn default_secure_boot_firmware_path() -> String {
    "/usr/share/OVMF/OVMF_CODE.secboot.fd".to_string()
}

fn default_boot_source() -> String {
    "Hdd".to_string()
}

fn default_metrics_enabled() -> bool {
    true
}

fn default_metrics_port() -> u16 {
    9090
}

fn default_cpu_count() -> u8 {
    2
}

fn default_memory_mib() -> u64 {
    1024
}

impl ServerConfig {
    pub fn validate_tls(&self) -> anyhow::Result<()> {
        match (&self.tls_cert, &self.tls_key) {
            (Some(_), None) => {
                anyhow::bail!("tls_cert is set but tls_key is missing");
            }
            (None, Some(_)) => {
                anyhow::bail!("tls_key is set but tls_cert is missing");
            }
            (None, None) => {
                if self.tls_client_ca.is_some() {
                    anyhow::bail!("tls_client_ca requires tls_cert and tls_key");
                }
                return Ok(());
            }
            (Some(cert), Some(key)) => {
                if !cert.exists() {
                    anyhow::bail!("tls_cert file does not exist: {}", cert.display());
                }
                if !key.exists() {
                    anyhow::bail!("tls_key file does not exist: {}", key.display());
                }
            }
        }

        if let Some(ca) = &self.tls_client_ca
            && !ca.exists()
        {
            anyhow::bail!("tls_client_ca file does not exist: {}", ca.display());
        }

        Ok(())
    }
}

impl AppConfig {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: AppConfig = toml::from_str(&content)?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cloud_hypervisor_config() {
        let config = AppConfig::load(Path::new("examples/config.toml")).unwrap();
        assert_eq!(config.backend, BackendType::CloudHypervisor);
        assert_eq!(config.server.port, 8000);
        assert_eq!(config.systems.len(), 2);

        let vm1 = &config.systems["vm1"];
        assert_eq!(vm1.name.as_deref(), Some("Test VM 1"));
        assert_eq!(
            vm1.socket_path.as_ref().unwrap().to_str().unwrap(),
            "/tmp/cloud-hypervisor-vm1.sock"
        );
        assert_eq!(vm1.hardware.cpu_count, 2);
        assert_eq!(vm1.hardware.memory_mib, 1024);
        assert_eq!(vm1.hardware.disks.len(), 1);
        assert_eq!(vm1.hardware.disks[0].id.as_deref(), Some("rootdisk"));

        let vm2 = &config.systems["vm2"];
        assert_eq!(vm2.hardware.cpu_count, 4);
        assert_eq!(vm2.hardware.memory_mib, 4096);
        assert_eq!(vm2.hardware.disks.len(), 2);
    }

    #[test]
    #[cfg(feature = "libvirt")]
    fn test_parse_libvirt_config() {
        let content = std::fs::read_to_string("examples/config-libvirt.toml").unwrap();
        // Parse without the libvirt feature — backend field should still parse
        // as a string but we test the rest of the structure
        let config: AppConfig = toml::from_str(&content).unwrap();
        assert_eq!(config.server.port, 8000);
        assert_eq!(config.systems.len(), 2);

        let vm1 = &config.systems["vm1"];
        assert_eq!(vm1.domain_name.as_deref(), Some("my-test-vm"));
        assert_eq!(vm1.connection_uri.as_deref(), Some("qemu:///system"));
        assert!(vm1.socket_path.is_none());
    }
}
