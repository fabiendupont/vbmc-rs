use std::time::Instant;

use dashmap::DashMap;
use tracing::warn;

use super::k8s_auth::KubernetesUser;

const AUTHZ_CACHE_TTL_SECS: u64 = 60;

pub type AuthzCache = DashMap<(String, String), (bool, Instant)>;

pub async fn can_access_vm(
    client: &kube::Client,
    user: &KubernetesUser,
    namespace: &str,
    vm_name: &str,
    cache: &AuthzCache,
) -> bool {
    let _ = vm_name;
    let cache_key = (user.username.clone(), namespace.to_string());

    if let Some(entry) = cache.get(&cache_key) {
        let (allowed, cached_at) = entry.value();
        if cached_at.elapsed().as_secs() < AUTHZ_CACHE_TTL_SECS {
            return *allowed;
        }
        drop(entry);
        cache.remove(&cache_key);
    }

    let sar = serde_json::json!({
        "apiVersion": "authorization.k8s.io/v1",
        "kind": "SubjectAccessReview",
        "spec": {
            "user": user.username,
            "groups": user.groups,
            "resourceAttributes": {
                "namespace": namespace,
                "verb": "get",
                "group": "kubevirt.io",
                "resource": "virtualmachines",
                "name": vm_name
            }
        }
    });

    let req = http::Request::post("/apis/authorization.k8s.io/v1/subjectaccessreviews")
        .header("content-type", "application/json")
        .body(serde_json::to_vec(&sar).unwrap())
        .unwrap();

    let allowed = match client.request::<serde_json::Value>(req).await {
        Ok(resp) => resp
            .pointer("/status/allowed")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        Err(e) => {
            warn!(
                user = %user.username,
                namespace = %namespace,
                error = %e,
                "SubjectAccessReview request failed"
            );
            false
        }
    };

    cache.insert(cache_key, (allowed, Instant::now()));
    allowed
}
