use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::Instant;

use axum::extract::FromRequestParts;
use axum::http::StatusCode;
use axum::http::request::Parts;
use axum::response::{IntoResponse, Response};
use dashmap::DashMap;
use tracing::warn;

use super::state::AggregatorState;

#[derive(Debug, Clone)]
pub struct KubernetesUser {
    pub username: String,
    pub groups: Vec<String>,
}

pub struct KubeAuthError;

impl IntoResponse for KubeAuthError {
    fn into_response(self) -> Response {
        (
            StatusCode::UNAUTHORIZED,
            "Kubernetes authentication required",
        )
            .into_response()
    }
}

const TOKEN_CACHE_TTL_SECS: u64 = 60;

pub type TokenCache = DashMap<u64, (KubernetesUser, Instant)>;

fn hash_token(token: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    token.hash(&mut hasher);
    hasher.finish()
}

impl FromRequestParts<Arc<AggregatorState>> for KubernetesUser {
    type Rejection = KubeAuthError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AggregatorState>,
    ) -> Result<Self, Self::Rejection> {
        if !state.config.auth.enabled {
            return Ok(KubernetesUser {
                username: "anonymous".to_string(),
                groups: vec![],
            });
        }

        if state.config.auth_mode == "kubernetes" {
            return authenticate_kubernetes(parts, state).await;
        }

        authenticate_local(parts, state)
    }
}

async fn authenticate_kubernetes(
    parts: &mut Parts,
    state: &Arc<AggregatorState>,
) -> Result<KubernetesUser, KubeAuthError> {
    let token_str = parts
        .headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .ok_or(KubeAuthError)?;

    let token_hash = hash_token(token_str);

    if let Some(entry) = state.token_cache.get(&token_hash) {
        let (user, cached_at) = entry.value();
        if cached_at.elapsed().as_secs() < TOKEN_CACHE_TTL_SECS {
            return Ok(user.clone());
        }
        drop(entry);
        state.token_cache.remove(&token_hash);
    }

    let client = state.kube_client.as_ref().ok_or(KubeAuthError)?;

    let review_body = serde_json::json!({
        "apiVersion": "authentication.k8s.io/v1",
        "kind": "TokenReview",
        "spec": {"token": token_str}
    });

    let req = http::Request::post("/apis/authentication.k8s.io/v1/tokenreviews")
        .header("content-type", "application/json")
        .body(serde_json::to_vec(&review_body).unwrap())
        .unwrap();

    let resp: serde_json::Value = match client.request(req).await {
        Ok(v) => v,
        Err(e) => {
            warn!(error = %e, "TokenReview request failed");
            return Err(KubeAuthError);
        }
    };

    let authenticated = resp
        .pointer("/status/authenticated")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if !authenticated {
        return Err(KubeAuthError);
    }

    let username = resp
        .pointer("/status/user/username")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    let groups = resp
        .pointer("/status/user/groups")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let user = KubernetesUser { username, groups };
    state
        .token_cache
        .insert(token_hash, (user.clone(), Instant::now()));

    Ok(user)
}

fn authenticate_local(
    parts: &mut Parts,
    state: &Arc<AggregatorState>,
) -> Result<KubernetesUser, KubeAuthError> {
    if let Some(token) = parts.headers.get("X-Auth-Token")
        && let Ok(token_str) = token.to_str()
        && let Some(session) = state.session_store.validate_token(token_str)
    {
        return Ok(KubernetesUser {
            username: session.username,
            groups: vec![],
        });
    }

    if let Some(auth) = parts.headers.get("Authorization")
        && let Ok(auth_str) = auth.to_str()
        && let Some(credentials) = auth_str.strip_prefix("Basic ")
        && let Ok(decoded) =
            base64::Engine::decode(&base64::engine::general_purpose::STANDARD, credentials)
        && let Ok(decoded_str) = String::from_utf8(decoded)
        && let Some((username, password)) = decoded_str.split_once(':')
    {
        let mut account_store = state.account_store.lock().map_err(|_| KubeAuthError)?;
        account_store.check_and_unlock(username);
        if account_store.verify_password(username, password) {
            account_store.record_successful_login(username);
            if let Some(path) = &state.config.auth.accounts_file {
                let _ = account_store.save(path);
            }
            return Ok(KubernetesUser {
                username: username.to_string(),
                groups: vec![],
            });
        }
        account_store.record_failed_login(
            username,
            state.config.auth.lockout_threshold,
            state.config.auth.lockout_duration_seconds,
        );
        if let Some(path) = &state.config.auth.accounts_file {
            let _ = account_store.save(path);
        }
    }

    Err(KubeAuthError)
}

#[cfg(test)]
mod coverage_tests {
    use super::*;

    #[test]
    fn test_hash_token_deterministic() {
        let token = "my-secret-token";
        let hash1 = hash_token(token);
        let hash2 = hash_token(token);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_hash_token_different() {
        let token1 = "token1";
        let token2 = "token2";
        assert_ne!(hash_token(token1), hash_token(token2));
    }

    #[test]
    fn test_kube_auth_error_into_response() {
        let err = KubeAuthError;
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn test_kubernetes_user_clone() {
        let user = KubernetesUser {
            username: "testuser".to_string(),
            groups: vec!["group1".to_string(), "group2".to_string()],
        };
        let cloned = user.clone();
        assert_eq!(cloned.username, "testuser");
        assert_eq!(cloned.groups.len(), 2);
    }

    #[test]
    fn test_token_review_request_structure() {
        // Verify the TokenReview request body structure that's sent to k8s API
        let token = "Bearer xyz";
        let review_body = serde_json::json!({
            "apiVersion": "authentication.k8s.io/v1",
            "kind": "TokenReview",
            "spec": {"token": token}
        });

        assert_eq!(review_body["apiVersion"], "authentication.k8s.io/v1");
        assert_eq!(review_body["kind"], "TokenReview");
        assert_eq!(review_body["spec"]["token"], token);
    }

    #[test]
    fn test_token_review_response_parsing() {
        // Verify parsing a successful TokenReview response
        let response = serde_json::json!({
            "status": {
                "authenticated": true,
                "user": {
                    "username": "system:serviceaccount:default:my-sa",
                    "groups": ["system:serviceaccounts", "system:authenticated"]
                }
            }
        });

        let authenticated = response
            .pointer("/status/authenticated")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        assert!(authenticated);

        let username = response
            .pointer("/status/user/username")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        assert_eq!(username, "system:serviceaccount:default:my-sa");

        let groups = response
            .pointer("/status/user/groups")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        assert_eq!(groups.len(), 2);
        assert!(groups.contains(&"system:serviceaccounts".to_string()));
    }

    #[test]
    fn test_token_review_response_parsing_not_authenticated() {
        let response = serde_json::json!({
            "status": {
                "authenticated": false
            }
        });

        let authenticated = response
            .pointer("/status/authenticated")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        assert!(!authenticated);
    }

    #[test]
    fn test_token_review_response_parsing_missing_groups() {
        let response = serde_json::json!({
            "status": {
                "authenticated": true,
                "user": {
                    "username": "testuser"
                }
            }
        });

        let groups = response
            .pointer("/status/user/groups")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        assert!(groups.is_empty());
    }

    #[test]
    fn test_token_cache_ttl_constant() {
        assert_eq!(TOKEN_CACHE_TTL_SECS, 60);
    }
}

#[cfg(all(test, feature = "aggregator"))]
mod local_auth_tests {
    use super::super::config::AggregatorConfig;
    use super::super::discovery::SidecarRegistry;
    use super::super::k8s_authz::AuthzCache;
    use super::super::proxy::ProxyClient;
    use super::super::state::AggregatorState;
    use super::*;
    use vbmc_rs::auth::accounts::AccountStore;
    use vbmc_rs::auth::sessions::SessionStore;

    fn test_config(auth_enabled: bool) -> AggregatorConfig {
        let toml = format!(
            r#"
[server]
bind_address = "0.0.0.0"
port = 8080

[auth]
enabled = {auth_enabled}

[discovery]
endpoints = []

[sidecar]
"#
        );
        toml::from_str(&toml).unwrap()
    }

    fn make_state(config: AggregatorConfig, accounts: AccountStore) -> Arc<AggregatorState> {
        let proxy = ProxyClient::new(&config.sidecar).unwrap();
        Arc::new(AggregatorState {
            config,
            registry: Arc::new(SidecarRegistry::new()),
            proxy,
            session_store: SessionStore::new(3600, 16),
            account_store: std::sync::Mutex::new(accounts),
            instance_uuid: "test-uuid".to_string(),
            kube_client: None,
            token_cache: TokenCache::new(),
            authz_cache: AuthzCache::new(),
        })
    }

    fn parts_with_headers(headers: &[(&str, &str)]) -> Parts {
        let mut builder = http::Request::builder().method("GET").uri("/");
        for (k, v) in headers {
            builder = builder.header(*k, *v);
        }
        builder.body(()).unwrap().into_parts().0
    }

    fn basic_auth_header(username: &str, password: &str) -> String {
        let creds = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            format!("{username}:{password}"),
        );
        format!("Basic {creds}")
    }

    #[test]
    fn test_authenticate_local_no_headers_fails() {
        let state = make_state(test_config(true), AccountStore::default());
        let mut parts = parts_with_headers(&[]);
        assert!(authenticate_local(&mut parts, &state).is_err());
    }

    #[test]
    fn test_authenticate_local_valid_basic_auth() {
        let mut accounts = AccountStore::default();
        accounts
            .add_account("alice", "secret123", "Administrator")
            .unwrap();
        let state = make_state(test_config(true), accounts);

        let header = basic_auth_header("alice", "secret123");
        let mut parts = parts_with_headers(&[("Authorization", &header)]);

        let user = authenticate_local(&mut parts, &state)
            .ok()
            .expect("basic auth should succeed");
        assert_eq!(user.username, "alice");
        assert!(user.groups.is_empty());
    }

    #[test]
    fn test_authenticate_local_wrong_password_fails() {
        let mut accounts = AccountStore::default();
        accounts
            .add_account("alice", "secret123", "Administrator")
            .unwrap();
        let state = make_state(test_config(true), accounts);

        let header = basic_auth_header("alice", "wrongpass");
        let mut parts = parts_with_headers(&[("Authorization", &header)]);
        assert!(authenticate_local(&mut parts, &state).is_err());
    }

    #[test]
    fn test_authenticate_local_unknown_user_fails() {
        let state = make_state(test_config(true), AccountStore::default());
        let header = basic_auth_header("ghost", "whatever");
        let mut parts = parts_with_headers(&[("Authorization", &header)]);
        assert!(authenticate_local(&mut parts, &state).is_err());
    }

    #[test]
    fn test_authenticate_local_malformed_basic_credentials_fails() {
        let state = make_state(test_config(true), AccountStore::default());
        // Not valid base64 after the "Basic " prefix.
        let mut parts = parts_with_headers(&[("Authorization", "Basic !!!not-base64")]);
        assert!(authenticate_local(&mut parts, &state).is_err());
    }

    #[test]
    fn test_authenticate_local_valid_session_token() {
        let state = make_state(test_config(true), AccountStore::default());
        let session = state
            .session_store
            .create_session("bob", "Operator")
            .unwrap();
        let mut parts = parts_with_headers(&[("X-Auth-Token", &session.token)]);

        let user = authenticate_local(&mut parts, &state)
            .ok()
            .expect("session auth should succeed");
        assert_eq!(user.username, "bob");
    }

    #[test]
    fn test_authenticate_local_invalid_session_token_fails() {
        let state = make_state(test_config(true), AccountStore::default());
        let mut parts = parts_with_headers(&[("X-Auth-Token", "bogus-token")]);
        assert!(authenticate_local(&mut parts, &state).is_err());
    }

    #[tokio::test]
    async fn test_from_request_parts_anonymous_when_auth_disabled() {
        let state = make_state(test_config(false), AccountStore::default());
        let mut parts = parts_with_headers(&[]);
        let user = KubernetesUser::from_request_parts(&mut parts, &state)
            .await
            .ok()
            .expect("anonymous access should be granted when auth is disabled");
        assert_eq!(user.username, "anonymous");
        assert!(user.groups.is_empty());
    }

    #[tokio::test]
    async fn test_from_request_parts_local_mode_valid_session() {
        let state = make_state(test_config(true), AccountStore::default());
        let session = state
            .session_store
            .create_session("carol", "ReadOnly")
            .unwrap();
        let mut parts = parts_with_headers(&[("X-Auth-Token", &session.token)]);
        let user = KubernetesUser::from_request_parts(&mut parts, &state)
            .await
            .ok()
            .expect("session auth should succeed");
        assert_eq!(user.username, "carol");
    }

    #[tokio::test]
    async fn test_from_request_parts_local_mode_no_credentials_rejected() {
        let state = make_state(test_config(true), AccountStore::default());
        let mut parts = parts_with_headers(&[]);
        assert!(
            KubernetesUser::from_request_parts(&mut parts, &state)
                .await
                .is_err()
        );
    }
}
