pub mod accounts;
pub mod rbac;
pub mod sessions;

use std::sync::Arc;

use axum::extract::FromRequestParts;
use axum::http::StatusCode;
use axum::http::request::Parts;
use axum::response::{IntoResponse, Response};

use crate::app_state::AppState;

#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub username: String,
    pub role: String,
}

#[derive(Debug)]
pub struct AuthError;

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        (
            StatusCode::UNAUTHORIZED,
            [("WWW-Authenticate", "Basic realm=\"vbmc-rs\", X-Auth-Token")],
            "Authentication required",
        )
            .into_response()
    }
}

impl FromRequestParts<Arc<AppState>> for AuthenticatedUser {
    type Rejection = AuthError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        if !state.config.auth.enabled {
            return Ok(AuthenticatedUser {
                username: "anonymous".to_string(),
                role: "Administrator".to_string(),
            });
        }

        let mut attempted_user: Option<String> = None;

        // Check X-Auth-Token header
        if let Some(token) = parts.headers.get("X-Auth-Token")
            && let Ok(token_str) = token.to_str()
            && let Some(session) = state.session_store.validate_token(token_str)
        {
            return Ok(AuthenticatedUser {
                username: session.username,
                role: session.role,
            });
        }

        // Check Basic auth
        if let Some(auth) = parts.headers.get("Authorization")
            && let Ok(auth_str) = auth.to_str()
            && let Some(credentials) = auth_str.strip_prefix("Basic ")
            && let Ok(decoded) =
                base64::Engine::decode(&base64::engine::general_purpose::STANDARD, credentials)
            && let Ok(decoded_str) = String::from_utf8(decoded)
            && let Some((username, password)) = decoded_str.split_once(':')
        {
            attempted_user = Some(username.to_string());
            let mut account_store = state.account_store.lock().map_err(|_| AuthError)?;
            account_store.check_and_unlock(username);
            if account_store.verify_password(username, password) {
                let role = account_store
                    .find_account(username)
                    .map(|a| a.role.clone())
                    .unwrap_or_else(|| "ReadOnly".to_string());
                account_store.record_successful_login(username);
                if let Some(path) = &state.config.auth.accounts_file {
                    let _ = account_store.save(path);
                }
                return Ok(AuthenticatedUser {
                    username: username.to_string(),
                    role,
                });
            }
            let locked = account_store.record_failed_login(
                username,
                state.config.auth.lockout_threshold,
                state.config.auth.lockout_duration_seconds,
            );
            if let Some(path) = &state.config.auth.accounts_file {
                let _ = account_store.save(path);
            }
            if locked {
                state.event_bus.emit(crate::events::RedfishEvent {
                    event_type: crate::events::registry::EVENT_TYPE_ALERT.to_string(),
                    event_id: uuid::Uuid::new_v4().to_string(),
                    event_timestamp: chrono::Utc::now(),
                    message_id: crate::events::registry::MSG_ACCOUNT_LOCKED.to_string(),
                    message: format!("Account '{username}' locked after too many failed attempts"),
                    origin_of_condition: Some(format!(
                        "/redfish/v1/AccountService/Accounts/{username}"
                    )),
                    severity: crate::events::registry::SEVERITY_WARNING.to_string(),
                    actor: Some(username.to_string()),
                    payload: None,
                });
            }
        }

        state.event_bus.emit(crate::events::RedfishEvent {
            event_type: crate::events::registry::EVENT_TYPE_ALERT.to_string(),
            event_id: uuid::Uuid::new_v4().to_string(),
            event_timestamp: chrono::Utc::now(),
            message_id: crate::events::registry::MSG_AUTH_FAILURE.to_string(),
            message: format!(
                "Authentication failure{}",
                attempted_user
                    .as_ref()
                    .map(|u| format!(" for user '{u}'"))
                    .unwrap_or_default()
            ),
            origin_of_condition: Some(parts.uri.path().to_string()),
            severity: crate::events::registry::SEVERITY_WARNING.to_string(),
            actor: attempted_user,
            payload: None,
        });
        crate::telemetry::record_auth_attempt(false);

        Err(AuthError)
    }
}

#[cfg(test)]
mod coverage_tests {
    use super::*;
    use crate::app_state::AppState;
    use crate::auth::accounts::AccountStore;
    use crate::backend::Backend;
    use crate::backend::mock::MockBackend;
    use crate::redfish::test_harness::{systems_with, test_config};
    use axum::Router;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::response::Json;
    use axum::routing::get;
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD as BASE64;
    use serde_json::json;
    use tower::ServiceExt;

    async fn test_handler(user: AuthenticatedUser) -> Json<serde_json::Value> {
        Json(json!({
            "username": user.username,
            "role": user.role
        }))
    }

    fn make_auth_app_state(auth_enabled: bool) -> Arc<AppState> {
        let mut config = test_config(systems_with("vm1"));
        config.auth.enabled = auth_enabled;

        let mut account_store = AccountStore::default();
        if auth_enabled {
            account_store
                .add_account("admin", "password123", "Administrator")
                .unwrap();
            account_store
                .add_account("readonly", "readonly123", "ReadOnly")
                .unwrap();
        }

        Arc::new(AppState::new(
            config,
            Backend::Mock(MockBackend::new()),
            account_store,
            None,
            None,
        ))
    }

    #[tokio::test]
    async fn test_auth_disabled_returns_anonymous_administrator() {
        let state = make_auth_app_state(false);
        let app = Router::new()
            .route("/test", get(test_handler))
            .with_state(state);

        let req = Request::builder().uri("/test").body(Body::empty()).unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

        assert_eq!(body["username"], "anonymous");
        assert_eq!(body["role"], "Administrator");
    }

    #[tokio::test]
    async fn test_auth_enabled_valid_basic_auth() {
        let state = make_auth_app_state(true);
        let app = Router::new()
            .route("/test", get(test_handler))
            .with_state(state);

        let credentials = "admin:password123";
        let encoded = BASE64.encode(credentials.as_bytes());

        let req = Request::builder()
            .uri("/test")
            .header("Authorization", format!("Basic {}", encoded))
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

        assert_eq!(body["username"], "admin");
        assert_eq!(body["role"], "Administrator");
    }

    #[tokio::test]
    async fn test_auth_enabled_invalid_basic_auth() {
        let state = make_auth_app_state(true);
        let app = Router::new()
            .route("/test", get(test_handler))
            .with_state(state);

        let credentials = "admin:wrongpassword";
        let encoded = BASE64.encode(credentials.as_bytes());

        let req = Request::builder()
            .uri("/test")
            .header("Authorization", format!("Basic {}", encoded))
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_auth_enabled_no_credentials() {
        let state = make_auth_app_state(true);
        let app = Router::new()
            .route("/test", get(test_handler))
            .with_state(state);

        let req = Request::builder().uri("/test").body(Body::empty()).unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_auth_enabled_valid_token() {
        let state = make_auth_app_state(true);

        let session = state
            .session_store
            .create_session("admin", "Administrator")
            .unwrap();

        let app = Router::new()
            .route("/test", get(test_handler))
            .with_state(state);

        let req = Request::builder()
            .uri("/test")
            .header("X-Auth-Token", &session.token)
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

        assert_eq!(body["username"], "admin");
        assert_eq!(body["role"], "Administrator");
    }

    #[tokio::test]
    async fn test_auth_enabled_invalid_token() {
        let state = make_auth_app_state(true);
        let app = Router::new()
            .route("/test", get(test_handler))
            .with_state(state);

        let req = Request::builder()
            .uri("/test")
            .header("X-Auth-Token", "invalid-token-12345")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_auth_error_into_response() {
        let error = AuthError;
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_readonly_user_role() {
        let state = make_auth_app_state(true);
        let app = Router::new()
            .route("/test", get(test_handler))
            .with_state(state);

        let credentials = "readonly:readonly123";
        let encoded = BASE64.encode(credentials.as_bytes());

        let req = Request::builder()
            .uri("/test")
            .header("Authorization", format!("Basic {}", encoded))
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

        assert_eq!(body["username"], "readonly");
        assert_eq!(body["role"], "ReadOnly");
    }

    #[tokio::test]
    async fn test_malformed_basic_auth_header() {
        let state = make_auth_app_state(true);
        let app = Router::new()
            .route("/test", get(test_handler))
            .with_state(state);

        let req = Request::builder()
            .uri("/test")
            .header("Authorization", "Basic not-base64-!!!!")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_basic_auth_without_colon() {
        let state = make_auth_app_state(true);
        let app = Router::new()
            .route("/test", get(test_handler))
            .with_state(state);

        let credentials = "usernameonly";
        let encoded = BASE64.encode(credentials.as_bytes());

        let req = Request::builder()
            .uri("/test")
            .header("Authorization", format!("Basic {}", encoded))
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_nonexistent_user() {
        let state = make_auth_app_state(true);
        let app = Router::new()
            .route("/test", get(test_handler))
            .with_state(state);

        let credentials = "nonexistent:password";
        let encoded = BASE64.encode(credentials.as_bytes());

        let req = Request::builder()
            .uri("/test")
            .header("Authorization", format!("Basic {}", encoded))
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}
