use std::sync::Arc;

use dashmap::DashMap;
use tracing::info;

#[derive(Debug, Clone)]
pub struct SidecarEndpoint {
    pub system_id: String,
    pub namespace: String,
    pub vm_name: String,
    pub url: String,
}

pub struct SidecarRegistry {
    endpoints: DashMap<String, SidecarEndpoint>,
}

impl SidecarRegistry {
    pub fn new() -> Self {
        Self {
            endpoints: DashMap::new(),
        }
    }

    pub fn register(&self, system_id: String, namespace: String, vm_name: String, url: String) {
        self.endpoints.insert(
            system_id.clone(),
            SidecarEndpoint {
                system_id,
                namespace,
                vm_name,
                url,
            },
        );
    }

    pub fn deregister(&self, system_id: &str) {
        self.endpoints.remove(system_id);
    }

    pub fn get(&self, system_id: &str) -> Option<SidecarEndpoint> {
        self.endpoints.get(system_id).map(|e| e.clone())
    }

    pub fn list(&self) -> Vec<SidecarEndpoint> {
        self.endpoints.iter().map(|e| e.value().clone()).collect()
    }
}

pub fn register_static_endpoints(
    registry: &SidecarRegistry,
    endpoints: &[super::config::StaticEndpoint],
) {
    for ep in endpoints {
        info!(system_id = %ep.system_id, url = %ep.url, "Registering static sidecar endpoint");
        registry.register(
            ep.system_id.clone(),
            String::new(),
            ep.system_id.clone(),
            ep.url.clone(),
        );
    }
}

#[cfg(feature = "aggregator")]
pub async fn start_kubernetes_watcher(
    registry: Arc<SidecarRegistry>,
    namespace: Option<String>,
    label_selector: String,
    sidecar_port: u16,
    bmc_network: Option<String>,
    cancel: tokio_util::sync::CancellationToken,
) {
    use k8s_openapi::api::core::v1::Pod;
    use kube::Api;
    use kube::runtime::watcher;
    use kube::runtime::watcher::Event;
    use tokio_stream::StreamExt;
    use tracing::{debug, warn};

    let client = match kube::Client::try_default().await {
        Ok(c) => c,
        Err(e) => {
            warn!("Failed to create Kubernetes client: {e}");
            return;
        }
    };

    let pods: Api<Pod> = match &namespace {
        Some(ns) => Api::namespaced(client, ns),
        None => Api::default_namespaced(client),
    };

    let watcher_config = watcher::Config::default().labels(&label_selector);
    let mut stream = std::pin::pin!(watcher(pods, watcher_config));

    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                info!("Kubernetes watcher cancelled");
                break;
            }
            item = stream.next() => {
                match item {
                    Some(Ok(Event::Apply(pod) | Event::InitApply(pod))) => {
                        if let Some(ep) = extract_endpoint(&pod, sidecar_port, bmc_network.as_deref())
                            && is_pod_ready(&pod)
                        {
                            info!(system_id = %ep.system_id, url = %ep.url, "Discovered sidecar pod");
                            registry.register(ep.system_id, ep.namespace, ep.vm_name, ep.url);
                        }
                    }
                    Some(Ok(Event::Delete(pod))) => {
                        if let Some(ep) = extract_endpoint(&pod, sidecar_port, bmc_network.as_deref()) {
                            info!(system_id = %ep.system_id, "Sidecar pod removed");
                            registry.deregister(&ep.system_id);
                        }
                    }
                    Some(Ok(Event::Init | Event::InitDone)) => {
                        debug!("Watcher init event");
                    }
                    Some(Err(e)) => {
                        warn!("Kubernetes watcher error: {e}");
                    }
                    None => break,
                }
            }
        }
    }
}

#[cfg(feature = "aggregator")]
fn extract_endpoint(
    pod: &k8s_openapi::api::core::v1::Pod,
    sidecar_port: u16,
    bmc_network: Option<&str>,
) -> Option<SidecarEndpoint> {
    let metadata = &pod.metadata;
    let labels = metadata.labels.as_ref()?;

    let system_id = labels
        .get("vbmc-rs/system-id")
        .cloned()
        .or_else(|| metadata.name.clone())?;

    let namespace = metadata.namespace.clone().unwrap_or_default();

    let vm_name = labels
        .get("vm.kubevirt.io/name")
        .cloned()
        .unwrap_or_else(|| system_id.clone());

    let bmc_ip = bmc_network.and_then(|net| {
        let annotations = metadata.annotations.as_ref()?;
        let network_status = annotations.get("k8s.v1.cni.cncf.io/network-status")?;
        let status: Vec<serde_json::Value> = serde_json::from_str(network_status).ok()?;
        status.iter().find_map(|entry| {
            let name = entry.get("name")?.as_str()?;
            if name.contains(net) {
                entry
                    .get("ips")?
                    .as_array()?
                    .first()?
                    .as_str()
                    .map(|s| s.to_string())
            } else {
                None
            }
        })
    });

    let ip = bmc_ip.or_else(|| pod.status.as_ref()?.pod_ip.clone())?;

    let scheme = if sidecar_port == 443 || sidecar_port == 8443 {
        "https"
    } else {
        "http"
    };
    let url = format!("{scheme}://{ip}:{sidecar_port}");

    Some(SidecarEndpoint {
        system_id,
        namespace,
        vm_name,
        url,
    })
}

#[cfg(feature = "aggregator")]
fn is_pod_ready(pod: &k8s_openapi::api::core::v1::Pod) -> bool {
    pod.status
        .as_ref()
        .and_then(|s| s.conditions.as_ref())
        .map(|conditions| {
            conditions
                .iter()
                .any(|c| c.type_ == "Ready" && c.status == "True")
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn register_simple(registry: &SidecarRegistry, system_id: &str, url: &str) {
        registry.register(
            system_id.to_string(),
            "default".to_string(),
            system_id.to_string(),
            url.to_string(),
        );
    }

    #[test]
    fn test_register_and_get() {
        let registry = SidecarRegistry::new();
        register_simple(&registry, "vm1", "http://10.0.0.1:8000");

        let ep = registry.get("vm1").unwrap();
        assert_eq!(ep.system_id, "vm1");
        assert_eq!(ep.url, "http://10.0.0.1:8000");
        assert_eq!(ep.namespace, "default");
        assert_eq!(ep.vm_name, "vm1");
    }

    #[test]
    fn test_get_nonexistent() {
        let registry = SidecarRegistry::new();
        assert!(registry.get("missing").is_none());
    }

    #[test]
    fn test_deregister() {
        let registry = SidecarRegistry::new();
        register_simple(&registry, "vm1", "http://10.0.0.1:8000");
        registry.deregister("vm1");
        assert!(registry.get("vm1").is_none());
    }

    #[test]
    fn test_deregister_nonexistent() {
        let registry = SidecarRegistry::new();
        registry.deregister("missing");
    }

    #[test]
    fn test_list_empty() {
        let registry = SidecarRegistry::new();
        assert!(registry.list().is_empty());
    }

    #[test]
    fn test_list_multiple() {
        let registry = SidecarRegistry::new();
        register_simple(&registry, "vm1", "http://10.0.0.1:8000");
        register_simple(&registry, "vm2", "http://10.0.0.2:8000");

        let list = registry.list();
        assert_eq!(list.len(), 2);

        let ids: Vec<&str> = list.iter().map(|e| e.system_id.as_str()).collect();
        assert!(ids.contains(&"vm1"));
        assert!(ids.contains(&"vm2"));
    }

    #[test]
    fn test_register_overwrites() {
        let registry = SidecarRegistry::new();
        register_simple(&registry, "vm1", "http://10.0.0.1:8000");
        register_simple(&registry, "vm1", "http://10.0.0.99:8000");

        let ep = registry.get("vm1").unwrap();
        assert_eq!(ep.url, "http://10.0.0.99:8000");
        assert_eq!(registry.list().len(), 1);
    }
}
