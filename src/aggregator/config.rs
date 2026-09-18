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
    /// Named chassis, each scoped to a namespace with an optional VM selector.
    /// When empty, a single chassis is synthesised from `discovery.namespace`.
    #[serde(default)]
    pub chassis: Vec<ChassisConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChassisConfig {
    pub name: String,
    pub namespace: String,
    #[serde(default)]
    pub vm_selector: VmSelectorConfig,
}

/// Criteria for filtering VMs within a chassis namespace.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct VmSelectorConfig {
    /// Label key=value pairs that VMs must carry to be included.
    #[serde(default)]
    pub labels: std::collections::BTreeMap<String, String>,
    /// Explicit VM names to include (empty = all).
    #[serde(default)]
    #[allow(dead_code)]
    pub names: Vec<String>,
}

impl VmSelectorConfig {
    /// Convert labels to a Kubernetes label-selector string (e.g. `"k=v,k2=v2"`).
    pub fn label_selector_string(&self) -> Option<String> {
        if self.labels.is_empty() {
            None
        } else {
            Some(
                self.labels
                    .iter()
                    .map(|(k, v)| format!("{k}={v}"))
                    .collect::<Vec<_>>()
                    .join(","),
            )
        }
    }
}

impl AggregatorConfig {
    /// Return the configured chassis list, or synthesise one from `discovery.namespace`
    /// for backward compatibility with configs that predate the `[[chassis]]` section.
    pub fn effective_chassis(&self) -> Vec<ChassisConfig> {
        if !self.chassis.is_empty() {
            return self.chassis.clone();
        }
        // Legacy: synthesise a single chassis from the discovery namespace.
        if let Some(ns) = &self.discovery.namespace {
            vec![ChassisConfig {
                name: ns.clone(),
                namespace: ns.clone(),
                vm_selector: VmSelectorConfig::default(),
            }]
        } else {
            vec![]
        }
    }
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

    // ---- VmSelectorConfig ----

    #[test]
    fn test_vm_selector_label_selector_empty() {
        let sel = VmSelectorConfig::default();
        assert!(sel.label_selector_string().is_none());
    }

    #[test]
    fn test_vm_selector_label_selector_single() {
        let mut sel = VmSelectorConfig::default();
        sel.labels.insert("app".to_string(), "workload".to_string());
        assert_eq!(sel.label_selector_string().unwrap(), "app=workload");
    }

    #[test]
    fn test_vm_selector_label_selector_multiple_sorted() {
        let mut sel = VmSelectorConfig::default();
        sel.labels.insert("env".to_string(), "prod".to_string());
        sel.labels.insert("app".to_string(), "web".to_string());
        // BTreeMap iterates in sorted key order
        assert_eq!(sel.label_selector_string().unwrap(), "app=web,env=prod");
    }

    // ---- AggregatorConfig::effective_chassis ----

    #[test]
    fn test_effective_chassis_uses_explicit_list() {
        let toml = r#"
[server]
bind_address = "0.0.0.0"
port = 8080

[discovery]
endpoints = []

[sidecar]

[[chassis]]
name = "tenant-a"
namespace = "ns-a"

[[chassis]]
name = "tenant-b"
namespace = "ns-b"
"#;
        let config: AggregatorConfig = toml::from_str(toml).unwrap();
        let chassis = config.effective_chassis();
        assert_eq!(chassis.len(), 2);
        assert_eq!(chassis[0].name, "tenant-a");
        assert_eq!(chassis[0].namespace, "ns-a");
        assert_eq!(chassis[1].name, "tenant-b");
        assert_eq!(chassis[1].namespace, "ns-b");
    }

    #[test]
    fn test_effective_chassis_synthesises_from_discovery_namespace() {
        let toml = r#"
[server]
bind_address = "0.0.0.0"
port = 8080

[discovery]
namespace = "my-ns"
endpoints = []

[sidecar]
"#;
        let config: AggregatorConfig = toml::from_str(toml).unwrap();
        let chassis = config.effective_chassis();
        assert_eq!(chassis.len(), 1);
        assert_eq!(chassis[0].name, "my-ns");
        assert_eq!(chassis[0].namespace, "my-ns");
    }

    #[test]
    fn test_effective_chassis_empty_when_no_namespace() {
        let toml = r#"
[server]
bind_address = "0.0.0.0"
port = 8080

[discovery]
endpoints = []

[sidecar]
"#;
        let config: AggregatorConfig = toml::from_str(toml).unwrap();
        assert!(config.effective_chassis().is_empty());
    }

    #[test]
    fn test_chassis_with_vm_selector_from_toml() {
        let toml = r#"
[server]
bind_address = "0.0.0.0"
port = 8080

[discovery]
endpoints = []

[sidecar]

[[chassis]]
name = "filtered"
namespace = "ns1"

[chassis.vm_selector]
labels = { "env" = "prod", "tier" = "backend" }
names = ["vm-1", "vm-2"]
"#;
        let config: AggregatorConfig = toml::from_str(toml).unwrap();
        let chassis = config.effective_chassis();
        assert_eq!(chassis.len(), 1);
        assert_eq!(chassis[0].vm_selector.labels.len(), 2);
        assert_eq!(chassis[0].vm_selector.labels["env"], "prod");
        assert_eq!(chassis[0].vm_selector.names, vec!["vm-1", "vm-2"]);
        let sel = chassis[0].vm_selector.label_selector_string().unwrap();
        assert!(sel.contains("env=prod"));
        assert!(sel.contains("tier=backend"));
    }
}
