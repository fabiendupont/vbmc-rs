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
    sidecar_tls: bool,
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
        None => Api::all(client),
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
                        if let Some(ep) = extract_endpoint(&pod, sidecar_port, sidecar_tls, bmc_network.as_deref())
                            && is_pod_ready(&pod)
                        {
                            info!(system_id = %ep.system_id, url = %ep.url, "Discovered sidecar pod");
                            registry.register(ep.system_id, ep.namespace, ep.vm_name, ep.url);
                        }
                    }
                    Some(Ok(Event::Delete(pod))) => {
                        if let Some(ep) = extract_endpoint(&pod, sidecar_port, sidecar_tls, bmc_network.as_deref()) {
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
    sidecar_tls: bool,
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

    let scheme = if sidecar_tls || sidecar_port == 443 || sidecar_port == 8443 {
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

#[derive(Debug, Clone)]
pub struct KubeVirtVmEntry {
    pub system_id: String,
    pub namespace: String,
    pub vm_name: String,
}

pub struct KubeVirtVmRegistry {
    vms: DashMap<String, KubeVirtVmEntry>,
}

impl KubeVirtVmRegistry {
    pub fn new() -> Self {
        Self {
            vms: DashMap::new(),
        }
    }

    pub fn register(&self, entry: KubeVirtVmEntry) {
        self.vms.insert(entry.system_id.clone(), entry);
    }

    pub fn deregister(&self, system_id: &str) {
        self.vms.remove(system_id);
    }

    pub fn get(&self, system_id: &str) -> Option<KubeVirtVmEntry> {
        self.vms.get(system_id).map(|e| e.clone())
    }

    pub fn list(&self) -> Vec<KubeVirtVmEntry> {
        self.vms.iter().map(|e| e.value().clone()).collect()
    }
}

#[cfg(feature = "aggregator")]
pub async fn start_kubevirt_vm_watcher(
    registry: Arc<KubeVirtVmRegistry>,
    namespace: Option<String>,
    cancel: tokio_util::sync::CancellationToken,
) {
    use kube::api::{ApiResource, DynamicObject};
    use kube::Api;
    use kube::runtime::watcher;
    use kube::runtime::watcher::Event;
    use tokio_stream::StreamExt;
    use tracing::{debug, warn};

    let client = match kube::Client::try_default().await {
        Ok(c) => c,
        Err(e) => {
            warn!("Failed to create Kubernetes client for VM watcher: {e}");
            return;
        }
    };

    let vm_ar = ApiResource {
        group: "kubevirt.io".to_string(),
        version: "v1".to_string(),
        api_version: "kubevirt.io/v1".to_string(),
        kind: "VirtualMachine".to_string(),
        plural: "virtualmachines".to_string(),
    };

    let api: Api<DynamicObject> = match &namespace {
        Some(ns) => Api::namespaced_with(client, ns, &vm_ar),
        None => Api::all_with(client, &vm_ar),
    };

    let mut stream = std::pin::pin!(watcher(api, watcher::Config::default()));

    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                info!("KubeVirt VM watcher cancelled");
                break;
            }
            item = stream.next() => {
                match item {
                    Some(Ok(Event::Apply(obj) | Event::InitApply(obj))) => {
                        if let Some(entry) = extract_vm_entry(&obj) {
                            info!(system_id = %entry.system_id, "Discovered KubeVirt VM");
                            registry.register(entry);
                        }
                    }
                    Some(Ok(Event::Delete(obj))) => {
                        if let Some(entry) = extract_vm_entry(&obj) {
                            info!(system_id = %entry.system_id, "KubeVirt VM removed");
                            registry.deregister(&entry.system_id);
                        }
                    }
                    Some(Ok(Event::Init | Event::InitDone)) => {
                        debug!("KubeVirt VM watcher init event");
                    }
                    Some(Err(e)) => {
                        warn!("KubeVirt VM watcher error: {e}");
                    }
                    None => break,
                }
            }
        }
    }
}

#[cfg(feature = "aggregator")]
fn extract_vm_entry(obj: &kube::api::DynamicObject) -> Option<KubeVirtVmEntry> {
    let labels = obj.metadata.labels.as_ref();
    let system_id = labels
        .and_then(|l| l.get("vbmc-rs/system-id"))
        .cloned()
        .or_else(|| obj.metadata.name.clone())?;
    let namespace = obj.metadata.namespace.clone().unwrap_or_default();
    let vm_name = obj.metadata.name.clone().unwrap_or_default();
    Some(KubeVirtVmEntry {
        system_id,
        namespace,
        vm_name,
    })
}

#[cfg(test)]
mod vm_registry_tests {
    use super::*;

    fn make_entry(system_id: &str, namespace: &str, vm_name: &str) -> KubeVirtVmEntry {
        KubeVirtVmEntry {
            system_id: system_id.to_string(),
            namespace: namespace.to_string(),
            vm_name: vm_name.to_string(),
        }
    }

    #[test]
    fn test_register_and_get() {
        let reg = KubeVirtVmRegistry::new();
        reg.register(make_entry("vm1", "default", "my-vm"));

        let entry = reg.get("vm1").unwrap();
        assert_eq!(entry.system_id, "vm1");
        assert_eq!(entry.namespace, "default");
        assert_eq!(entry.vm_name, "my-vm");
    }

    #[test]
    fn test_get_nonexistent() {
        let reg = KubeVirtVmRegistry::new();
        assert!(reg.get("missing").is_none());
    }

    #[test]
    fn test_deregister() {
        let reg = KubeVirtVmRegistry::new();
        reg.register(make_entry("vm1", "default", "my-vm"));
        reg.deregister("vm1");
        assert!(reg.get("vm1").is_none());
    }

    #[test]
    fn test_deregister_nonexistent() {
        let reg = KubeVirtVmRegistry::new();
        reg.deregister("missing");
    }

    #[test]
    fn test_list_empty() {
        let reg = KubeVirtVmRegistry::new();
        assert!(reg.list().is_empty());
    }

    #[test]
    fn test_list_multiple() {
        let reg = KubeVirtVmRegistry::new();
        reg.register(make_entry("vm1", "ns1", "my-vm-1"));
        reg.register(make_entry("vm2", "ns2", "my-vm-2"));

        let list = reg.list();
        assert_eq!(list.len(), 2);
        let ids: Vec<&str> = list.iter().map(|e| e.system_id.as_str()).collect();
        assert!(ids.contains(&"vm1"));
        assert!(ids.contains(&"vm2"));
    }

    #[test]
    fn test_register_overwrites() {
        let reg = KubeVirtVmRegistry::new();
        reg.register(make_entry("vm1", "ns1", "original"));
        reg.register(make_entry("vm1", "ns2", "updated"));

        let entry = reg.get("vm1").unwrap();
        assert_eq!(entry.vm_name, "updated");
        assert_eq!(reg.list().len(), 1);
    }
}

#[cfg(all(test, feature = "aggregator"))]
mod vm_watcher_extract_tests {
    use super::*;

    fn dynamic_obj_from_json(v: serde_json::Value) -> kube::api::DynamicObject {
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn test_extract_vm_entry_with_system_id_label() {
        let obj = dynamic_obj_from_json(serde_json::json!({
            "apiVersion": "kubevirt.io/v1",
            "kind": "VirtualMachine",
            "metadata": {
                "name": "my-vm",
                "namespace": "default",
                "labels": { "vbmc-rs/system-id": "sys-1" }
            }
        }));
        let entry = extract_vm_entry(&obj).unwrap();
        assert_eq!(entry.system_id, "sys-1");
        assert_eq!(entry.namespace, "default");
        assert_eq!(entry.vm_name, "my-vm");
    }

    #[test]
    fn test_extract_vm_entry_falls_back_to_name() {
        let obj = dynamic_obj_from_json(serde_json::json!({
            "apiVersion": "kubevirt.io/v1",
            "kind": "VirtualMachine",
            "metadata": {
                "name": "my-vm",
                "namespace": "ns1"
            }
        }));
        let entry = extract_vm_entry(&obj).unwrap();
        assert_eq!(entry.system_id, "my-vm");
        assert_eq!(entry.namespace, "ns1");
        assert_eq!(entry.vm_name, "my-vm");
    }

    #[test]
    fn test_extract_vm_entry_no_name_returns_none() {
        let obj = dynamic_obj_from_json(serde_json::json!({
            "apiVersion": "kubevirt.io/v1",
            "kind": "VirtualMachine",
            "metadata": {}
        }));
        assert!(extract_vm_entry(&obj).is_none());
    }

    #[test]
    fn test_extract_vm_entry_no_namespace_defaults_empty() {
        let obj = dynamic_obj_from_json(serde_json::json!({
            "apiVersion": "kubevirt.io/v1",
            "kind": "VirtualMachine",
            "metadata": { "name": "my-vm" }
        }));
        let entry = extract_vm_entry(&obj).unwrap();
        assert_eq!(entry.namespace, "");
    }
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

    #[test]
    fn test_register_static_endpoints() {
        let registry = SidecarRegistry::new();
        let endpoints = vec![
            super::super::config::StaticEndpoint {
                system_id: "vm1".to_string(),
                url: "http://10.0.0.1:8000".to_string(),
            },
            super::super::config::StaticEndpoint {
                system_id: "vm2".to_string(),
                url: "http://10.0.0.2:8000".to_string(),
            },
        ];

        register_static_endpoints(&registry, &endpoints);

        let list = registry.list();
        assert_eq!(list.len(), 2);

        let ep1 = registry.get("vm1").unwrap();
        assert_eq!(ep1.system_id, "vm1");
        assert_eq!(ep1.url, "http://10.0.0.1:8000");
        assert_eq!(ep1.namespace, "");
        assert_eq!(ep1.vm_name, "vm1");
    }

    #[test]
    fn test_register_static_endpoints_empty() {
        let registry = SidecarRegistry::new();
        register_static_endpoints(&registry, &[]);
        assert!(registry.list().is_empty());
    }
}

#[cfg(all(test, feature = "aggregator"))]
mod endpoint_tests {
    use super::*;
    use k8s_openapi::api::core::v1::Pod;

    fn pod_from_json(v: serde_json::Value) -> Pod {
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn test_extract_endpoint_basic_with_system_id_label() {
        let pod = pod_from_json(serde_json::json!({
            "metadata": {
                "name": "pod-1",
                "namespace": "ns1",
                "labels": {"vbmc-rs/system-id": "vm-a"}
            },
            "status": {"podIP": "10.1.2.3"}
        }));
        let ep = extract_endpoint(&pod, 8000, false, None).unwrap();
        assert_eq!(ep.system_id, "vm-a");
        assert_eq!(ep.namespace, "ns1");
        // vm_name falls back to system_id when the kubevirt label is absent
        assert_eq!(ep.vm_name, "vm-a");
        assert_eq!(ep.url, "http://10.1.2.3:8000");
    }

    #[test]
    fn test_extract_endpoint_system_id_falls_back_to_pod_name() {
        let pod = pod_from_json(serde_json::json!({
            "metadata": {
                "name": "pod-name",
                "labels": {"app": "vbmc"}
            },
            "status": {"podIP": "10.0.0.5"}
        }));
        let ep = extract_endpoint(&pod, 8000, false, None).unwrap();
        assert_eq!(ep.system_id, "pod-name");
        assert_eq!(ep.namespace, "");
    }

    #[test]
    fn test_extract_endpoint_no_labels_returns_none() {
        let pod = pod_from_json(serde_json::json!({
            "metadata": {"name": "pod-x"},
            "status": {"podIP": "10.0.0.5"}
        }));
        assert!(extract_endpoint(&pod, 8000, false, None).is_none());
    }

    #[test]
    fn test_extract_endpoint_vm_name_from_kubevirt_label() {
        let pod = pod_from_json(serde_json::json!({
            "metadata": {
                "name": "pod-1",
                "labels": {
                    "vbmc-rs/system-id": "sys-1",
                    "vm.kubevirt.io/name": "my-vm"
                }
            },
            "status": {"podIP": "10.0.0.9"}
        }));
        let ep = extract_endpoint(&pod, 8000, false, None).unwrap();
        assert_eq!(ep.system_id, "sys-1");
        assert_eq!(ep.vm_name, "my-vm");
    }

    #[test]
    fn test_extract_endpoint_bmc_network_ip_from_annotation() {
        let network_status = serde_json::json!([
            {"name": "default/pod-net", "ips": ["10.0.0.5"]},
            {"name": "default/bmc-net", "ips": ["192.168.1.10"]}
        ])
        .to_string();
        let pod = pod_from_json(serde_json::json!({
            "metadata": {
                "name": "pod-1",
                "labels": {"vbmc-rs/system-id": "sys-1"},
                "annotations": {"k8s.v1.cni.cncf.io/network-status": network_status}
            },
            "status": {"podIP": "10.0.0.5"}
        }));
        let ep = extract_endpoint(&pod, 8000, false, Some("bmc-net")).unwrap();
        assert_eq!(ep.url, "http://192.168.1.10:8000");
    }

    #[test]
    fn test_extract_endpoint_bmc_network_no_match_falls_back_to_pod_ip() {
        let network_status = serde_json::json!([
            {"name": "default/pod-net", "ips": ["10.0.0.5"]}
        ])
        .to_string();
        let pod = pod_from_json(serde_json::json!({
            "metadata": {
                "name": "pod-1",
                "labels": {"vbmc-rs/system-id": "sys-1"},
                "annotations": {"k8s.v1.cni.cncf.io/network-status": network_status}
            },
            "status": {"podIP": "10.0.0.5"}
        }));
        let ep = extract_endpoint(&pod, 8000, false, Some("bmc-net")).unwrap();
        assert_eq!(ep.url, "http://10.0.0.5:8000");
    }

    #[test]
    fn test_extract_endpoint_https_scheme_when_tls_flag_set() {
        let pod = pod_from_json(serde_json::json!({
            "metadata": {"name": "p", "labels": {"vbmc-rs/system-id": "s"}},
            "status": {"podIP": "10.0.0.5"}
        }));
        let ep = extract_endpoint(&pod, 8000, true, None).unwrap();
        assert_eq!(ep.url, "https://10.0.0.5:8000");
    }

    #[test]
    fn test_extract_endpoint_https_scheme_for_port_8443() {
        let pod = pod_from_json(serde_json::json!({
            "metadata": {"name": "p", "labels": {"vbmc-rs/system-id": "s"}},
            "status": {"podIP": "10.0.0.5"}
        }));
        let ep = extract_endpoint(&pod, 8443, false, None).unwrap();
        assert_eq!(ep.url, "https://10.0.0.5:8443");
    }

    #[test]
    fn test_extract_endpoint_https_scheme_for_port_443() {
        let pod = pod_from_json(serde_json::json!({
            "metadata": {"name": "p", "labels": {"vbmc-rs/system-id": "s"}},
            "status": {"podIP": "10.0.0.5"}
        }));
        let ep = extract_endpoint(&pod, 443, false, None).unwrap();
        assert_eq!(ep.url, "https://10.0.0.5:443");
    }

    #[test]
    fn test_extract_endpoint_no_ip_returns_none() {
        let pod = pod_from_json(serde_json::json!({
            "metadata": {"name": "p", "labels": {"vbmc-rs/system-id": "s"}},
            "status": {}
        }));
        assert!(extract_endpoint(&pod, 8000, false, None).is_none());
    }

    #[test]
    fn test_is_pod_ready_true() {
        let pod = pod_from_json(serde_json::json!({
            "metadata": {"name": "p"},
            "status": {"conditions": [{"type": "Ready", "status": "True"}]}
        }));
        assert!(is_pod_ready(&pod));
    }

    #[test]
    fn test_is_pod_ready_false_when_not_ready() {
        let pod = pod_from_json(serde_json::json!({
            "metadata": {"name": "p"},
            "status": {"conditions": [{"type": "Ready", "status": "False"}]}
        }));
        assert!(!is_pod_ready(&pod));
    }

    #[test]
    fn test_is_pod_ready_false_no_conditions() {
        let pod = pod_from_json(serde_json::json!({
            "metadata": {"name": "p"},
            "status": {}
        }));
        assert!(!is_pod_ready(&pod));
    }

    #[test]
    fn test_is_pod_ready_false_no_status() {
        let pod = pod_from_json(serde_json::json!({"metadata": {"name": "p"}}));
        assert!(!is_pod_ready(&pod));
    }
}
