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

impl SidecarConnectionConfig {
    pub fn tls_enabled(&self) -> bool {
        self.tls_ca.is_some() && self.tls_cert.is_some() && self.tls_key.is_some()
    }
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

#[cfg(test)]
mod coverage_tests {
    use super::*;

    #[test]
    fn test_default_auth_mode() {
        assert_eq!(default_auth_mode(), "local");
    }

    #[test]
    fn test_default_mode() {
        assert_eq!(default_mode(), "static");
    }

    #[test]
    fn test_default_label_selector() {
        assert_eq!(
            default_label_selector(),
            "app.kubernetes.io/name=vbmc-rs-sidecar"
        );
    }

    #[test]
    fn test_default_sidecar_port() {
        assert_eq!(default_sidecar_port(), 8000);
    }

    #[test]
    fn test_sidecar_tls_enabled_all_set() {
        let config = SidecarConnectionConfig {
            port: 8000,
            tls_ca: Some(PathBuf::from("/tmp/ca.pem")),
            tls_cert: Some(PathBuf::from("/tmp/cert.pem")),
            tls_key: Some(PathBuf::from("/tmp/key.pem")),
        };
        assert!(config.tls_enabled());
    }

    #[test]
    fn test_sidecar_tls_enabled_missing_ca() {
        let config = SidecarConnectionConfig {
            port: 8000,
            tls_ca: None,
            tls_cert: Some(PathBuf::from("/tmp/cert.pem")),
            tls_key: Some(PathBuf::from("/tmp/key.pem")),
        };
        assert!(!config.tls_enabled());
    }

    #[test]
    fn test_sidecar_tls_enabled_missing_cert() {
        let config = SidecarConnectionConfig {
            port: 8000,
            tls_ca: Some(PathBuf::from("/tmp/ca.pem")),
            tls_cert: None,
            tls_key: Some(PathBuf::from("/tmp/key.pem")),
        };
        assert!(!config.tls_enabled());
    }

    #[test]
    fn test_sidecar_tls_enabled_missing_key() {
        let config = SidecarConnectionConfig {
            port: 8000,
            tls_ca: Some(PathBuf::from("/tmp/ca.pem")),
            tls_cert: Some(PathBuf::from("/tmp/cert.pem")),
            tls_key: None,
        };
        assert!(!config.tls_enabled());
    }

    #[test]
    fn test_config_defaults_from_toml() {
        let toml = r#"
[server]
bind_address = "0.0.0.0"
port = 8080

[discovery]
endpoints = []

[sidecar]
"#;
        let config: AggregatorConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.auth_mode, "local");
        assert_eq!(config.discovery.mode, "static");
        assert_eq!(
            config.discovery.label_selector,
            "app.kubernetes.io/name=vbmc-rs-sidecar"
        );
        assert_eq!(config.sidecar.port, 8000);
    }

    #[test]
    fn test_config_with_static_endpoints() {
        let toml = r#"
[server]
bind_address = "0.0.0.0"
port = 8080

[discovery]
mode = "static"
endpoints = [
    { system_id = "vm1", url = "http://10.0.0.1:8000" },
    { system_id = "vm2", url = "http://10.0.0.2:8000" },
]

[sidecar]
port = 9000
"#;
        let config: AggregatorConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.discovery.endpoints.len(), 2);
        assert_eq!(config.discovery.endpoints[0].system_id, "vm1");
        assert_eq!(config.discovery.endpoints[0].url, "http://10.0.0.1:8000");
        assert_eq!(config.sidecar.port, 9000);
    }

    #[test]
    fn test_config_with_kubernetes_discovery() {
        let toml = r#"
[server]
bind_address = "0.0.0.0"
port = 8080

[auth]
enabled = true

[discovery]
mode = "kubernetes"
namespace = "default"
label_selector = "app=my-vbmc"
bmc_network = "bmc-network"
endpoints = []

[sidecar]
"#;
        let config: AggregatorConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.discovery.mode, "kubernetes");
        assert_eq!(config.discovery.namespace.as_ref().unwrap(), "default");
        assert_eq!(config.discovery.label_selector, "app=my-vbmc");
        assert_eq!(
            config.discovery.bmc_network.as_ref().unwrap(),
            "bmc-network"
        );
    }

    #[test]
    fn test_config_load_missing_file() {
        let result = AggregatorConfig::load(Path::new("/nonexistent/config.toml"));
        assert!(result.is_err());
    }
}
