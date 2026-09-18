use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

pub struct WebhookConfig {
    pub sidecar_image: String,
    pub bmc_network: String,
    pub tls_secret: Option<String>,
    pub keylime_url: Option<String>,
    pub swtpm_socket: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdmissionReview {
    pub api_version: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request: Option<AdmissionRequest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response: Option<AdmissionResponse>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdmissionRequest {
    pub uid: String,
    #[serde(default)]
    pub object: serde_json::Value,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdmissionResponse {
    pub uid: String,
    pub allowed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub patch_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub patch: Option<String>,
}

pub async fn handle_mutate(
    State(config): State<Arc<WebhookConfig>>,
    Json(review): Json<AdmissionReview>,
) -> Json<AdmissionReview> {
    let request = match &review.request {
        Some(req) => req,
        None => {
            warn!("AdmissionReview missing request");
            return Json(AdmissionReview {
                api_version: review.api_version,
                kind: review.kind,
                request: None,
                response: Some(AdmissionResponse {
                    uid: String::new(),
                    allowed: true,
                    patch_type: None,
                    patch: None,
                }),
            });
        }
    };

    let uid = request.uid.clone();

    let labels = request
        .object
        .pointer("/metadata/labels")
        .and_then(|v| v.as_object());

    let is_virt_launcher = labels
        .and_then(|l| l.get("kubevirt.io"))
        .and_then(|v| v.as_str())
        == Some("virt-launcher");

    let system_id = labels
        .and_then(|l| l.get("vbmc-rs/system-id"))
        .and_then(|v| v.as_str());

    let namespace = request
        .object
        .pointer("/metadata/namespace")
        .and_then(|v| v.as_str())
        .unwrap_or("default")
        .to_string();

    let (is_match, system_id_value) = match (is_virt_launcher, system_id) {
        (true, Some(id)) => (true, id.to_string()),
        _ => (false, String::new()),
    };

    if !is_match {
        return Json(AdmissionReview {
            api_version: review.api_version,
            kind: review.kind,
            request: None,
            response: Some(AdmissionResponse {
                uid,
                allowed: true,
                patch_type: None,
                patch: None,
            }),
        });
    }

    info!(
        system_id = %system_id_value,
        "Injecting vbmc-rs sidecar into virt-launcher pod"
    );

    let patch = build_patch(
        &config.sidecar_image,
        &config.bmc_network,
        config.tls_secret.as_deref(),
        config.keylime_url.as_deref(),
        config.swtpm_socket.as_deref(),
        &system_id_value,
        &namespace,
        &request.object,
    );
    let patch_json = serde_json::to_string(&patch).expect("patch serialization cannot fail");
    let patch_base64 = BASE64.encode(patch_json.as_bytes());

    Json(AdmissionReview {
        api_version: review.api_version,
        kind: review.kind,
        request: None,
        response: Some(AdmissionResponse {
            uid,
            allowed: true,
            patch_type: Some("JSONPatch".to_string()),
            patch: Some(patch_base64),
        }),
    })
}

fn has_libvirt_volume(pod: &serde_json::Value) -> Option<String> {
    let volumes = pod.pointer("/spec/volumes")?.as_array()?;
    for vol in volumes {
        let mounts_path = vol.pointer("/name").and_then(|n| n.as_str()).unwrap_or("");
        let is_emptydir = vol.get("emptyDir").is_some();
        let is_hostpath = vol
            .get("hostPath")
            .and_then(|hp| hp.get("path"))
            .and_then(|p| p.as_str())
            .is_some_and(|p| p.contains("libvirt"));

        if is_emptydir || is_hostpath {
            let containers = pod.pointer("/spec/containers")?.as_array()?;
            for container in containers {
                let volume_mounts = container.get("volumeMounts")?.as_array()?;
                for vm in volume_mounts {
                    let mount_path = vm.get("mountPath").and_then(|p| p.as_str())?;
                    let vol_name = vm.get("name").and_then(|n| n.as_str())?;
                    if mount_path == "/var/run/libvirt" && vol_name == mounts_path {
                        return Some(mounts_path.to_string());
                    }
                }
            }
        }
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn build_patch(
    sidecar_image: &str,
    bmc_network: &str,
    tls_secret: Option<&str>,
    keylime_url: Option<&str>,
    swtpm_socket: Option<&str>,
    system_id: &str,
    namespace: &str,
    pod: &serde_json::Value,
) -> Vec<serde_json::Value> {
    let mut patch = Vec::new();

    let has_annotations = pod.pointer("/metadata/annotations").is_some();
    if !has_annotations {
        patch.push(serde_json::json!({
            "op": "add",
            "path": "/metadata/annotations",
            "value": {}
        }));
    }

    let existing_networks = pod
        .pointer("/metadata/annotations/k8s.v1.cni.cncf.io~1networks")
        .and_then(|v| v.as_str());

    let network_value = match existing_networks {
        Some(existing) => format!("{existing},{bmc_network}"),
        None => bmc_network.to_string(),
    };

    patch.push(serde_json::json!({
        "op": "add",
        "path": "/metadata/annotations/k8s.v1.cni.cncf.io~1networks",
        "value": network_value
    }));

    let libvirt_volume_name = has_libvirt_volume(pod);

    let libvirt_mount_name = libvirt_volume_name.as_deref().unwrap_or("libvirt-runtime");

    let tls_config = if tls_secret.is_some() {
        "tls_cert = \"/etc/vbmc-tls/tls.crt\"\n\
         tls_key = \"/etc/vbmc-tls/tls.key\"\n\
         tls_client_ca = \"/etc/vbmc-tls/ca.crt\"\n"
    } else {
        ""
    };

    let spdm_enabled = keylime_url.is_some() || swtpm_socket.is_some();

    let inline_config = format!(
        "backend = \"libvirt\"\n\
         state_directory = \"/var/run/kubevirt-private/vbmc-state\"\n\
         audit_log = \"/var/run/kubevirt-private/vbmc-audit.jsonl\"\n\
         audit_log_target = \"stdout\"\n\
         \n\
         [server]\n\
         bind_address = \"::\"\n\
         port = 8000\n\
         {tls_config}\
         \n\
         [auth]\n\
         enabled = false\n\
         \n\
         [metrics]\n\
         enabled = false\n\
         \n\
         [security_policy]\n\
         spdm_enabled = {spdm_enabled}\n\
         \n\
         [systems.{system_id}]\n\
         name = \"{system_id}\"\n\
         chassis_id = \"{namespace}\"\n\
         connection_uri = \"qemu+unix:///session?socket=/var/run/libvirt/virtqemud-sock\"\n"
    );

    let attestation_config = if let Some(url) = keylime_url {
        format!(
            "\n[systems.{system_id}.attestation]\n\
             provider = \"keylime\"\n\
             provider_url = \"{url}\"\n\
             poll_interval_seconds = 30\n"
        )
    } else if let Some(socket) = swtpm_socket {
        format!(
            "\n[systems.{system_id}.attestation]\n\
             provider = \"swtpm\"\n\
             swtpm_socket = \"{socket}\"\n\
             poll_interval_seconds = 30\n"
        )
    } else {
        String::new()
    };

    let inline_config = format!("{inline_config}{attestation_config}");

    let startup_script = format!(
        "while [ ! -S /var/run/libvirt/virtqemud-sock ]; do sleep 1; done; \
         printf '%s' '{}' > /tmp/vbmc-config.toml; \
         exec /usr/local/bin/vbmc-rs -c /tmp/vbmc-config.toml",
        inline_config.replace('\'', "'\\''")
    );

    let mut volume_mounts = vec![
        serde_json::json!({"name": libvirt_mount_name, "mountPath": "/var/run/libvirt"}),
        serde_json::json!({"name": "private", "mountPath": "/var/run/kubevirt-private"}),
    ];
    if tls_secret.is_some() {
        volume_mounts.push(
            serde_json::json!({"name": "vbmc-tls", "mountPath": "/etc/vbmc-tls", "readOnly": true}),
        );
    }
    if swtpm_socket.is_some() {
        volume_mounts.push(
            serde_json::json!({"name": "swtpm-sock", "mountPath": "/var/run/swtpm", "readOnly": true}),
        );
    }

    let container = serde_json::json!({
        "name": "vbmc-rs",
        "image": sidecar_image,
        "command": ["/bin/sh", "-c", startup_script],
        "env": [
            {"name": "XDG_CACHE_HOME", "value": "/var/run/kubevirt-private"},
            {"name": "XDG_CONFIG_HOME", "value": "/var/run/kubevirt-private"},
            {"name": "XDG_RUNTIME_DIR", "value": "/var/run"},
            {"name": "HOME", "value": "/var/run/kubevirt-private"}
        ],
        "ports": [{"containerPort": 8000, "name": "redfish"}],
        "volumeMounts": volume_mounts
    });

    patch.push(serde_json::json!({
        "op": "add",
        "path": "/spec/containers/-",
        "value": container
    }));

    if let Some(secret_name) = tls_secret {
        patch.push(serde_json::json!({
            "op": "add",
            "path": "/spec/volumes/-",
            "value": {
                "name": "vbmc-tls",
                "secret": {
                    "secretName": secret_name
                }
            }
        }));
    }

    if swtpm_socket.is_some() {
        patch.push(serde_json::json!({
            "op": "add",
            "path": "/spec/volumes/-",
            "value": {
                "name": "swtpm-sock",
                "hostPath": {
                    "path": "/var/run/swtpm",
                    "type": "Directory"
                }
            }
        }));
    }

    if libvirt_volume_name.is_none() {
        patch.push(serde_json::json!({
            "op": "add",
            "path": "/spec/volumes/-",
            "value": {
                "name": "libvirt-runtime",
                "emptyDir": {}
            }
        }));
    }

    patch
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_pod(
        labels: serde_json::Value,
        volumes: Option<serde_json::Value>,
    ) -> serde_json::Value {
        let mut pod = serde_json::json!({
            "metadata": {
                "name": "virt-launcher-test-vm-abc123",
                "labels": labels
            },
            "spec": {
                "containers": [{
                    "name": "compute",
                    "image": "registry.kubevirt.io/virt-launcher:latest"
                }],
                "volumes": []
            }
        });

        if let Some(vols) = volumes {
            pod["spec"]["volumes"] = vols;
        }

        pod
    }

    fn make_review(pod: serde_json::Value) -> AdmissionReview {
        AdmissionReview {
            api_version: "admission.k8s.io/v1".to_string(),
            kind: "AdmissionReview".to_string(),
            request: Some(AdmissionRequest {
                uid: "test-uid-123".to_string(),
                object: pod,
            }),
            response: None,
        }
    }

    #[tokio::test]
    async fn test_non_matching_pod_allowed_without_patch() {
        let config = Arc::new(WebhookConfig {
            sidecar_image: "vbmc-rs-sidecar:latest".to_string(),
            bmc_network: "vbmc-bmc".to_string(),
            tls_secret: None,
            keylime_url: None,
            swtpm_socket: None,
        });

        let pod = make_pod(serde_json::json!({"app": "nginx"}), None);
        let review = make_review(pod);

        let result = handle_mutate(State(config), Json(review)).await;
        let resp = result.0.response.unwrap();
        assert!(resp.allowed);
        assert!(resp.patch.is_none());
    }

    #[tokio::test]
    async fn test_virt_launcher_without_system_id_not_patched() {
        let config = Arc::new(WebhookConfig {
            sidecar_image: "vbmc-rs-sidecar:latest".to_string(),
            bmc_network: "vbmc-bmc".to_string(),
            tls_secret: None,
            keylime_url: None,
            swtpm_socket: None,
        });

        let pod = make_pod(serde_json::json!({"kubevirt.io": "virt-launcher"}), None);
        let review = make_review(pod);

        let result = handle_mutate(State(config), Json(review)).await;
        let resp = result.0.response.unwrap();
        assert!(resp.allowed);
        assert!(resp.patch.is_none());
    }

    #[tokio::test]
    async fn test_matching_pod_gets_sidecar_injected() {
        let config = Arc::new(WebhookConfig {
            sidecar_image: "vbmc-rs-sidecar:latest".to_string(),
            bmc_network: "vbmc-bmc".to_string(),
            tls_secret: None,
            keylime_url: None,
            swtpm_socket: None,
        });

        let pod = make_pod(
            serde_json::json!({
                "kubevirt.io": "virt-launcher",
                "vbmc-rs/system-id": "my-vm"
            }),
            None,
        );
        let review = make_review(pod);

        let result = handle_mutate(State(config), Json(review)).await;
        let resp = result.0.response.unwrap();
        assert!(resp.allowed);
        assert_eq!(resp.patch_type.as_deref(), Some("JSONPatch"));

        let patch_bytes = BASE64.decode(resp.patch.unwrap()).unwrap();
        let patch: Vec<serde_json::Value> = serde_json::from_slice(&patch_bytes).unwrap();

        assert_eq!(patch.len(), 4);
        assert_eq!(patch[0]["op"], "add");
        assert_eq!(patch[0]["path"], "/metadata/annotations");

        assert_eq!(patch[1]["op"], "add");
        assert_eq!(
            patch[1]["path"],
            "/metadata/annotations/k8s.v1.cni.cncf.io~1networks"
        );
        assert_eq!(patch[1]["value"], "vbmc-bmc");

        assert_eq!(patch[2]["op"], "add");
        assert_eq!(patch[2]["path"], "/spec/containers/-");
        assert_eq!(patch[2]["value"]["name"], "vbmc-rs");
        assert_eq!(patch[2]["value"]["image"], "vbmc-rs-sidecar:latest");
        assert!(patch[2]["value"].get("securityContext").is_none());

        assert_eq!(patch[3]["op"], "add");
        assert_eq!(patch[3]["value"]["name"], "libvirt-runtime");
    }

    #[tokio::test]
    async fn test_existing_libvirt_volume_reused() {
        let config = Arc::new(WebhookConfig {
            sidecar_image: "vbmc-rs-sidecar:latest".to_string(),
            bmc_network: "vbmc-bmc".to_string(),
            tls_secret: None,
            keylime_url: None,
            swtpm_socket: None,
        });

        let pod = make_pod(
            serde_json::json!({
                "kubevirt.io": "virt-launcher",
                "vbmc-rs/system-id": "my-vm"
            }),
            Some(serde_json::json!([{
                "name": "virt-run-libvirt",
                "emptyDir": {}
            }])),
        );

        pod["spec"]["containers"][0].as_object().unwrap();

        let mut pod_with_mounts = pod.clone();
        pod_with_mounts["spec"]["containers"][0]["volumeMounts"] = serde_json::json!([{
            "name": "virt-run-libvirt",
            "mountPath": "/var/run/libvirt"
        }]);

        let review = make_review(pod_with_mounts);

        let result = handle_mutate(State(config), Json(review)).await;
        let resp = result.0.response.unwrap();
        assert!(resp.allowed);

        let patch_bytes = BASE64.decode(resp.patch.unwrap()).unwrap();
        let patch: Vec<serde_json::Value> = serde_json::from_slice(&patch_bytes).unwrap();

        // annotations (add empty + set network) + container = 3 ops (no volume add)
        assert_eq!(patch.len(), 3);

        let container_patch = patch
            .iter()
            .find(|p| p["path"] == "/spec/containers/-")
            .unwrap();
        let sidecar_mounts = container_patch["value"]["volumeMounts"].as_array().unwrap();
        let libvirt_mount = sidecar_mounts
            .iter()
            .find(|m| m["mountPath"] == "/var/run/libvirt")
            .unwrap();
        assert_eq!(libvirt_mount["name"], "virt-run-libvirt");
    }
}

#[cfg(test)]
mod coverage_tests {
    use super::*;

    fn make_pod(
        labels: serde_json::Value,
        volumes: Option<serde_json::Value>,
    ) -> serde_json::Value {
        let mut pod = serde_json::json!({
            "metadata": {
                "name": "virt-launcher-test-vm-abc123",
                "namespace": "default",
                "labels": labels
            },
            "spec": {
                "containers": [{
                    "name": "compute",
                    "image": "registry.kubevirt.io/virt-launcher:latest"
                }],
                "volumes": []
            }
        });

        if let Some(vols) = volumes {
            pod["spec"]["volumes"] = vols;
        }

        pod
    }

    fn make_review(pod: serde_json::Value) -> AdmissionReview {
        AdmissionReview {
            api_version: "admission.k8s.io/v1".to_string(),
            kind: "AdmissionReview".to_string(),
            request: Some(AdmissionRequest {
                uid: "test-uid-123".to_string(),
                object: pod,
            }),
            response: None,
        }
    }

    #[tokio::test]
    async fn test_tls_secret_volume_mounted() {
        let config = Arc::new(WebhookConfig {
            sidecar_image: "vbmc-rs:test".to_string(),
            bmc_network: "vbmc-bmc".to_string(),
            tls_secret: Some("vbmc-tls-cert".to_string()),
            keylime_url: None,
            swtpm_socket: None,
        });

        let pod = make_pod(
            serde_json::json!({
                "kubevirt.io": "virt-launcher",
                "vbmc-rs/system-id": "test-vm"
            }),
            None,
        );
        let review = make_review(pod);

        let result = handle_mutate(State(config), Json(review)).await;
        let resp = result.0.response.unwrap();
        assert!(resp.allowed);

        let patch_bytes = BASE64.decode(resp.patch.unwrap()).unwrap();
        let patch: Vec<serde_json::Value> = serde_json::from_slice(&patch_bytes).unwrap();

        let tls_volume = patch
            .iter()
            .find(|p| p["value"]["name"] == "vbmc-tls")
            .unwrap();
        assert_eq!(tls_volume["value"]["secret"]["secretName"], "vbmc-tls-cert");

        let container_patch = patch
            .iter()
            .find(|p| p["path"] == "/spec/containers/-")
            .unwrap();
        let volume_mounts = container_patch["value"]["volumeMounts"].as_array().unwrap();
        let tls_mount = volume_mounts
            .iter()
            .find(|m| m["name"] == "vbmc-tls")
            .unwrap();
        assert_eq!(tls_mount["mountPath"], "/etc/vbmc-tls");
        assert_eq!(tls_mount["readOnly"], true);
    }

    #[tokio::test]
    async fn test_keylime_url_configures_attestation() {
        let config = Arc::new(WebhookConfig {
            sidecar_image: "vbmc-rs:test".to_string(),
            bmc_network: "vbmc-bmc".to_string(),
            tls_secret: None,
            keylime_url: Some("http://keylime:8080".to_string()),
            swtpm_socket: None,
        });

        let pod = make_pod(
            serde_json::json!({
                "kubevirt.io": "virt-launcher",
                "vbmc-rs/system-id": "test-vm"
            }),
            None,
        );
        let review = make_review(pod);

        let result = handle_mutate(State(config), Json(review)).await;
        let resp = result.0.response.unwrap();
        assert!(resp.allowed);

        let patch_bytes = BASE64.decode(resp.patch.unwrap()).unwrap();
        let patch: Vec<serde_json::Value> = serde_json::from_slice(&patch_bytes).unwrap();

        let container_patch = patch
            .iter()
            .find(|p| p["path"] == "/spec/containers/-")
            .unwrap();
        let command = container_patch["value"]["command"].as_array().unwrap();
        let script = command[2].as_str().unwrap();

        assert!(script.contains("spdm_enabled = true"));
        assert!(script.contains("provider = \"keylime\""));
        assert!(script.contains("provider_url = \"http://keylime:8080\""));
    }

    #[tokio::test]
    async fn test_swtpm_socket_configures_attestation() {
        let config = Arc::new(WebhookConfig {
            sidecar_image: "vbmc-rs:test".to_string(),
            bmc_network: "vbmc-bmc".to_string(),
            tls_secret: None,
            keylime_url: None,
            swtpm_socket: Some("/var/run/swtpm/swtpm.sock".to_string()),
        });

        let pod = make_pod(
            serde_json::json!({
                "kubevirt.io": "virt-launcher",
                "vbmc-rs/system-id": "test-vm"
            }),
            None,
        );
        let review = make_review(pod);

        let result = handle_mutate(State(config), Json(review)).await;
        let resp = result.0.response.unwrap();
        assert!(resp.allowed);

        let patch_bytes = BASE64.decode(resp.patch.unwrap()).unwrap();
        let patch: Vec<serde_json::Value> = serde_json::from_slice(&patch_bytes).unwrap();

        let container_patch = patch
            .iter()
            .find(|p| p["path"] == "/spec/containers/-")
            .unwrap();
        let command = container_patch["value"]["command"].as_array().unwrap();
        let script = command[2].as_str().unwrap();

        assert!(script.contains("spdm_enabled = true"));
        assert!(script.contains("provider = \"swtpm\""));
        assert!(script.contains("swtpm_socket = \"/var/run/swtpm/swtpm.sock\""));

        let swtpm_volume = patch
            .iter()
            .find(|p| p["value"]["name"] == "swtpm-sock")
            .unwrap();
        assert_eq!(swtpm_volume["value"]["hostPath"]["path"], "/var/run/swtpm");

        let volume_mounts = container_patch["value"]["volumeMounts"].as_array().unwrap();
        let swtpm_mount = volume_mounts
            .iter()
            .find(|m| m["name"] == "swtpm-sock")
            .unwrap();
        assert_eq!(swtpm_mount["mountPath"], "/var/run/swtpm");
        assert_eq!(swtpm_mount["readOnly"], true);
    }

    #[tokio::test]
    async fn test_existing_network_annotation_merged() {
        let config = Arc::new(WebhookConfig {
            sidecar_image: "vbmc-rs:test".to_string(),
            bmc_network: "vbmc-bmc".to_string(),
            tls_secret: None,
            keylime_url: None,
            swtpm_socket: None,
        });

        let mut pod = make_pod(
            serde_json::json!({
                "kubevirt.io": "virt-launcher",
                "vbmc-rs/system-id": "test-vm"
            }),
            None,
        );
        pod["metadata"]["annotations"] = serde_json::json!({
            "k8s.v1.cni.cncf.io/networks": "existing-network"
        });

        let review = make_review(pod);

        let result = handle_mutate(State(config), Json(review)).await;
        let resp = result.0.response.unwrap();

        let patch_bytes = BASE64.decode(resp.patch.unwrap()).unwrap();
        let patch: Vec<serde_json::Value> = serde_json::from_slice(&patch_bytes).unwrap();

        let network_patch = patch
            .iter()
            .find(|p| p["path"] == "/metadata/annotations/k8s.v1.cni.cncf.io~1networks")
            .unwrap();
        assert_eq!(network_patch["value"], "existing-network,vbmc-bmc");
    }

    #[tokio::test]
    async fn test_admission_review_missing_request_allowed() {
        let config = Arc::new(WebhookConfig {
            sidecar_image: "vbmc-rs:test".to_string(),
            bmc_network: "vbmc-bmc".to_string(),
            tls_secret: None,
            keylime_url: None,
            swtpm_socket: None,
        });

        let review = AdmissionReview {
            api_version: "admission.k8s.io/v1".to_string(),
            kind: "AdmissionReview".to_string(),
            request: None,
            response: None,
        };

        let result = handle_mutate(State(config), Json(review)).await;
        let resp = result.0.response.unwrap();
        assert!(resp.allowed);
        assert!(resp.patch.is_none());
    }

    #[tokio::test]
    async fn test_has_libvirt_volume_hostpath() {
        let pod = serde_json::json!({
            "spec": {
                "volumes": [{
                    "name": "libvirt-host",
                    "hostPath": {
                        "path": "/var/run/libvirt"
                    }
                }],
                "containers": [{
                    "volumeMounts": [{
                        "name": "libvirt-host",
                        "mountPath": "/var/run/libvirt"
                    }]
                }]
            }
        });

        let result = has_libvirt_volume(&pod);
        assert_eq!(result, Some("libvirt-host".to_string()));
    }

    #[tokio::test]
    async fn test_has_libvirt_volume_no_match() {
        let pod = serde_json::json!({
            "spec": {
                "volumes": [{
                    "name": "other-volume",
                    "emptyDir": {}
                }],
                "containers": [{
                    "volumeMounts": [{
                        "name": "other-volume",
                        "mountPath": "/other/path"
                    }]
                }]
            }
        });

        let result = has_libvirt_volume(&pod);
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_namespace_extracted_from_pod() {
        let config = Arc::new(WebhookConfig {
            sidecar_image: "vbmc-rs:test".to_string(),
            bmc_network: "vbmc-bmc".to_string(),
            tls_secret: None,
            keylime_url: None,
            swtpm_socket: None,
        });

        let mut pod = make_pod(
            serde_json::json!({
                "kubevirt.io": "virt-launcher",
                "vbmc-rs/system-id": "test-vm"
            }),
            None,
        );
        pod["metadata"]["namespace"] = serde_json::json!("test-namespace");

        let review = make_review(pod);

        let result = handle_mutate(State(config), Json(review)).await;
        let resp = result.0.response.unwrap();

        let patch_bytes = BASE64.decode(resp.patch.unwrap()).unwrap();
        let patch: Vec<serde_json::Value> = serde_json::from_slice(&patch_bytes).unwrap();

        let container_patch = patch
            .iter()
            .find(|p| p["path"] == "/spec/containers/-")
            .unwrap();
        let command = container_patch["value"]["command"].as_array().unwrap();
        let script = command[2].as_str().unwrap();

        assert!(script.contains("chassis_id = \"test-namespace\""));
    }
}
