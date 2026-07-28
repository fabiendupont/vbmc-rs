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
