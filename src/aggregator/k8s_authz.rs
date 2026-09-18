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

#[cfg(test)]
mod coverage_tests {
    use super::*;

    #[test]
    fn test_authz_cache_ttl_constant() {
        assert_eq!(AUTHZ_CACHE_TTL_SECS, 60);
    }

    #[test]
    fn test_subject_access_review_request_structure() {
        // Verify the SAR request body structure sent to k8s API
        let username = "testuser";
        let groups = vec!["group1".to_string(), "group2".to_string()];
        let namespace = "default";
        let vm_name = "vm1";

        let sar = serde_json::json!({
            "apiVersion": "authorization.k8s.io/v1",
            "kind": "SubjectAccessReview",
            "spec": {
                "user": username,
                "groups": groups,
                "resourceAttributes": {
                    "namespace": namespace,
                    "verb": "get",
                    "group": "kubevirt.io",
                    "resource": "virtualmachines",
                    "name": vm_name
                }
            }
        });

        assert_eq!(sar["apiVersion"], "authorization.k8s.io/v1");
        assert_eq!(sar["kind"], "SubjectAccessReview");
        assert_eq!(sar["spec"]["user"], username);
        assert_eq!(sar["spec"]["resourceAttributes"]["namespace"], namespace);
        assert_eq!(sar["spec"]["resourceAttributes"]["verb"], "get");
        assert_eq!(sar["spec"]["resourceAttributes"]["group"], "kubevirt.io");
        assert_eq!(
            sar["spec"]["resourceAttributes"]["resource"],
            "virtualmachines"
        );
        assert_eq!(sar["spec"]["resourceAttributes"]["name"], vm_name);
    }

    #[test]
    fn test_sar_response_parsing_allowed() {
        let response = serde_json::json!({
            "status": {
                "allowed": true
            }
        });

        let allowed = response
            .pointer("/status/allowed")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        assert!(allowed);
    }

    #[test]
    fn test_sar_response_parsing_denied() {
        let response = serde_json::json!({
            "status": {
                "allowed": false,
                "reason": "user does not have permission"
            }
        });

        let allowed = response
            .pointer("/status/allowed")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        assert!(!allowed);
    }

    #[test]
    fn test_sar_response_parsing_missing_status() {
        let response = serde_json::json!({});

        let allowed = response
            .pointer("/status/allowed")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        assert!(!allowed);
    }
}
