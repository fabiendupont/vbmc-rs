use std::path::{Path, PathBuf};

use serde::Deserialize;
use vbmc_rs::config::{AuthConfig, ServerConfig};

#[derive(Debug, Clone, Deserialize)]
pub struct AggregatorConfig {
    pub server: ServerConfig,
    #[serde(default)]
    pub auth: AuthConfig,
    #[serde(default = "default_auth_mode")]
    pub auth_mode: String,
    pub discovery: DiscoveryConfig,
    pub sidecar: SidecarConnectionConfig,
}

fn default_auth_mode() -> String {
    "local".to_string()
}

#[derive(Debug, Clone, Deserialize)]
pub struct DiscoveryConfig {
    #[serde(default = "default_mode")]
    pub mode: String,
    #[serde(default)]
    pub namespace: Option<String>,
    #[serde(default = "default_label_selector")]
    pub label_selector: String,
    #[serde(default)]
    pub bmc_network: Option<String>,
    #[serde(default)]
    pub endpoints: Vec<StaticEndpoint>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StaticEndpoint {
    pub system_id: String,
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SidecarConnectionConfig {
    #[serde(default = "default_sidecar_port")]
    pub port: u16,
    pub tls_ca: Option<PathBuf>,
    pub tls_cert: Option<PathBuf>,
    pub tls_key: Option<PathBuf>,
}

fn default_mode() -> String {
    "static".to_string()
}

fn default_label_selector() -> String {
    "app.kubernetes.io/name=vbmc-rs-sidecar".to_string()
}

fn default_sidecar_port() -> u16 {
    8000
}

impl AggregatorConfig {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&content)?;
        Ok(config)
    }
}
