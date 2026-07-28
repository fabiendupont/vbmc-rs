use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use super::error::RedfishApiError;
use super::types::{Collection, ODataId};
use crate::app_state::AppState;
use crate::auth::AuthenticatedUser;
use crate::auth::rbac::{Privilege, has_privilege};
use crate::events::RedfishEvent;
use crate::events::registry::*;

#[derive(Debug, Serialize)]
pub struct SessionServiceResource {
    #[serde(rename = "@odata.id")]
    pub odata_id: &'static str,
    #[serde(rename = "@odata.type")]
    pub odata_type: &'static str,
    #[serde(rename = "Id")]
    pub id: &'static str,
    #[serde(rename = "Name")]
    pub name: &'static str,
    #[serde(rename = "Description")]
    pub description: &'static str,
    #[serde(rename = "ServiceEnabled")]
    pub service_enabled: bool,
    #[serde(rename = "SessionTimeout")]
    pub session_timeout: u64,
    #[serde(rename = "Sessions")]
    pub sessions: ODataId,
    #[serde(rename = "Status")]
    pub status: super::types::Status,
}

pub async fn get_session_service(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
) -> Json<SessionServiceResource> {
    Json(SessionServiceResource {
        odata_id: "/redfish/v1/SessionService",
        odata_type: "#SessionService.v1_1_9.SessionService",
        id: "SessionService",
        name: "Session Service",
        description: "Session management service",
        service_enabled: true,
        session_timeout: state.config.auth.session_timeout_seconds,
        sessions: ODataId::new("/redfish/v1/SessionService/Sessions"),
        status: super::types::Status::enabled_ok(),
    })
}

pub async fn get_sessions(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
) -> Json<Collection<ODataId>> {
    let sessions = state.session_store.list_sessions();
    let members: Vec<ODataId> = sessions
        .iter()
        .map(|s| ODataId::new(format!("/redfish/v1/SessionService/Sessions/{}", s.id)))
        .collect();

    Json(Collection::new(
        "/redfish/v1/SessionService/Sessions",
        "#SessionCollection.SessionCollection",
        "Session Collection",
        members,
    ))
}

#[derive(Debug, Deserialize)]
pub struct CreateSessionRequest {
    #[serde(rename = "UserName")]
    pub user_name: String,
    #[serde(rename = "Password")]
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct SessionResource {
    #[serde(rename = "@odata.id")]
    pub odata_id: String,
    #[serde(rename = "@odata.type")]
    pub odata_type: &'static str,
    #[serde(rename = "Id")]
    pub id: String,
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Description")]
    pub description: &'static str,
    #[serde(rename = "UserName")]
    pub user_name: String,
}

pub async fn create_session(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateSessionRequest>,
) -> Result<Response, RedfishApiError> {
    let mut account_store = state
        .account_store
        .lock()
        .map_err(|_| RedfishApiError::InternalError("Account store lock poisoned".to_string()))?;
    account_store.check_and_unlock(&body.user_name);
    if !account_store.verify_password(&body.user_name, &body.password) {
        let locked = account_store.record_failed_login(
            &body.user_name,
            state.config.auth.lockout_threshold,
            state.config.auth.lockout_duration_seconds,
        );
        if let Some(path) = &state.config.auth.accounts_file {
            let _ = account_store.save(path);
        }
        drop(account_store);
        if locked {
            state.event_bus.emit(RedfishEvent {
                event_type: EVENT_TYPE_ALERT.to_string(),
                event_id: uuid::Uuid::new_v4().to_string(),
                event_timestamp: Utc::now(),
                message_id: MSG_ACCOUNT_LOCKED.to_string(),
                message: format!("Account '{}' locked after too many failed attempts", body.user_name),
                origin_of_condition: Some(format!("/redfish/v1/AccountService/Accounts/{}", body.user_name)),
                severity: SEVERITY_WARNING.to_string(),
                actor: Some(body.user_name.clone()),
                payload: None,
            });
        }
        state.event_bus.emit(RedfishEvent {
            event_type: EVENT_TYPE_ALERT.to_string(),
            event_id: uuid::Uuid::new_v4().to_string(),
            event_timestamp: Utc::now(),
            message_id: MSG_AUTH_FAILURE.to_string(),
            message: format!("Authentication failure for user '{}'", body.user_name),
            origin_of_condition: Some("/redfish/v1/SessionService".to_string()),
            severity: SEVERITY_WARNING.to_string(),
            actor: Some(body.user_name.clone()),
            payload: None,
        });
        crate::telemetry::record_auth_attempt(false);
        return Err(RedfishApiError::Unauthorized(
            "Invalid credentials".to_string(),
        ));
    }

    let role = account_store
        .find_account(&body.user_name)
        .map(|a| a.role.clone())
        .unwrap_or_else(|| "ReadOnly".to_string());
    account_store.record_successful_login(&body.user_name);
    if let Some(path) = &state.config.auth.accounts_file {
        let _ = account_store.save(path);
    }
    drop(account_store);
    crate::telemetry::record_auth_attempt(true);

    let session = state
        .session_store
        .create_session(&body.user_name, &role)
        .ok_or_else(|| RedfishApiError::Conflict("Maximum sessions reached".to_string()))?;

    state.event_bus.emit(RedfishEvent {
        event_type: EVENT_TYPE_RESOURCE_ADDED.to_string(),
        event_id: uuid::Uuid::new_v4().to_string(),
        event_timestamp: Utc::now(),
        message_id: MSG_SESSION_CREATED.to_string(),
        message: format!("Session created for user '{}'", body.user_name),
        origin_of_condition: Some(format!(
            "/redfish/v1/SessionService/Sessions/{}",
            session.id
        )),
        severity: SEVERITY_OK.to_string(),
        actor: Some(body.user_name.clone()),
        payload: None,
    });

    let resource = SessionResource {
        odata_id: format!("/redfish/v1/SessionService/Sessions/{}", session.id),
        odata_type: "#Session.v1_7_0.Session",
        id: session.id.clone(),
        name: format!("Session for {}", body.user_name),
        description: "User session",
        user_name: body.user_name,
    };

    Ok((
        StatusCode::CREATED,
        [
            ("X-Auth-Token", session.token),
            (
                "Location",
                format!("/redfish/v1/SessionService/Sessions/{}", session.id),
            ),
        ],
        Json(resource),
    )
        .into_response())
}

pub async fn delete_session(
    State(state): State<Arc<AppState>>,
    user: AuthenticatedUser,
    Path(session_id): Path<String>,
) -> Result<StatusCode, RedfishApiError> {
    if let Some(session) = state.session_store.get_session_by_id(&session_id)
        && session.username != user.username
        && !has_privilege(&user.role, Privilege::ConfigureManager)
    {
        return Err(RedfishApiError::Forbidden(
            "Insufficient privileges".to_string(),
        ));
    }

    if state.session_store.delete_session_by_id(&session_id) {
        state.event_bus.emit(RedfishEvent {
            event_type: EVENT_TYPE_RESOURCE_REMOVED.to_string(),
            event_id: uuid::Uuid::new_v4().to_string(),
            event_timestamp: Utc::now(),
            message_id: MSG_SESSION_TERMINATED.to_string(),
            message: format!("Session '{session_id}' terminated"),
            origin_of_condition: Some(format!("/redfish/v1/SessionService/Sessions/{session_id}")),
            severity: SEVERITY_OK.to_string(),
            actor: None,
            payload: None,
        });
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(RedfishApiError::NotFound(format!(
            "Session '{session_id}' not found"
        )))
    }
}
