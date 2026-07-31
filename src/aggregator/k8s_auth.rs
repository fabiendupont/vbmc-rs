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
