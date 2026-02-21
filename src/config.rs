use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
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
    pub metrics: MetricsConfig,
    #[serde(default)]
    pub systems: HashMap<String, SystemConfig>,
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
pub struct DefaultsConfig {
    #[serde(default = "default_firmware_path")]
    pub firmware_path: String,
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

#[derive(Debug, Clone, Deserialize)]
pub struct SystemConfig {
    pub name: Option<String>,
    pub socket_path: PathBuf,
    pub firmware_path: Option<String>,
    #[serde(default)]
    pub boot_source: Option<String>,
    #[serde(default)]
    pub virtual_media_directory: Option<PathBuf>,
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

fn default_boot_source() -> String {
    "Hdd".to_string()
}

fn default_metrics_enabled() -> bool {
    true
}

fn default_metrics_port() -> u16 {
    9090
}

impl AppConfig {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: AppConfig = toml::from_str(&content)?;
        Ok(config)
    }
}
