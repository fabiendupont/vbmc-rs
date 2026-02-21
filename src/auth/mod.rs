pub mod accounts;
pub mod rbac;
pub mod sessions;

use std::sync::Arc;

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::StatusCode;
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
            [(
                "WWW-Authenticate",
                "Basic realm=\"vbmc-rs\", X-Auth-Token",
            )],
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

        // Check X-Auth-Token header
        if let Some(token) = parts.headers.get("X-Auth-Token") {
            if let Ok(token_str) = token.to_str() {
                if let Some(session) = state.session_store.validate_token(token_str) {
                    return Ok(AuthenticatedUser {
                        username: session.username,
                        role: session.role,
                    });
                }
            }
        }

        // Check Basic auth
        if let Some(auth) = parts.headers.get("Authorization") {
            if let Ok(auth_str) = auth.to_str() {
                if let Some(credentials) = auth_str.strip_prefix("Basic ") {
                    if let Ok(decoded) = base64::Engine::decode(
                        &base64::engine::general_purpose::STANDARD,
                        credentials,
                    ) {
                        if let Ok(decoded_str) = String::from_utf8(decoded) {
                            if let Some((username, password)) = decoded_str.split_once(':') {
                                let account_store = state.account_store.lock().unwrap();
                                if account_store.verify_password(username, password) {
                                    if let Some(account) =
                                        account_store.find_account(username)
                                    {
                                        return Ok(AuthenticatedUser {
                                            username: username.to_string(),
                                            role: account.role.clone(),
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Err(AuthError)
    }
}

/// Optional auth — returns None instead of 401 when auth is disabled or no credentials
pub struct OptionalAuth(pub Option<AuthenticatedUser>);

impl FromRequestParts<Arc<AppState>> for OptionalAuth {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        Ok(OptionalAuth(
            AuthenticatedUser::from_request_parts(parts, state)
                .await
                .ok(),
        ))
    }
}
