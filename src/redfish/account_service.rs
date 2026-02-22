use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};

use super::error::RedfishApiError;
use super::types::{Collection, ODataId};
use crate::app_state::AppState;
use crate::auth::rbac::{has_privilege, Privilege};
use crate::auth::OptionalAuth;

#[derive(Debug, Serialize)]
pub struct AccountServiceResource {
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
    #[serde(rename = "Accounts")]
    pub accounts: ODataId,
    #[serde(rename = "Roles")]
    pub roles: ODataId,
    #[serde(rename = "AccountLockoutThreshold")]
    pub lockout_threshold: u32,
    #[serde(rename = "AccountLockoutDuration")]
    pub lockout_duration: u64,
    #[serde(rename = "MinPasswordLength")]
    pub min_password_length: u32,
    #[serde(rename = "MaxPasswordLength")]
    pub max_password_length: u32,
    #[serde(rename = "AccountLockoutCounterResetAfter")]
    pub lockout_counter_reset_after: u64,
    #[serde(rename = "AccountLockoutCounterResetEnabled")]
    pub lockout_counter_reset_enabled: bool,
    #[serde(rename = "Status")]
    pub status: super::types::Status,
}

pub async fn get_account_service(
    State(state): State<Arc<AppState>>,
) -> Json<AccountServiceResource> {
    Json(AccountServiceResource {
        odata_id: "/redfish/v1/AccountService",
        odata_type: "#AccountService.v1_15_0.AccountService",
        id: "AccountService",
        name: "Account Service",
        description: "Account management service",
        service_enabled: state.config.auth.enabled,
        accounts: ODataId::new("/redfish/v1/AccountService/Accounts"),
        roles: ODataId::new("/redfish/v1/AccountService/Roles"),
        lockout_threshold: state.config.auth.lockout_threshold,
        lockout_duration: state.config.auth.lockout_duration_seconds,
        min_password_length: 1,
        max_password_length: 128,
        lockout_counter_reset_after: state.config.auth.lockout_duration_seconds,
        lockout_counter_reset_enabled: true,
        status: super::types::Status::enabled_ok(),
    })
}

pub async fn get_accounts(
    State(state): State<Arc<AppState>>,
) -> Json<Collection<ODataId>> {
    let store = state.account_store.lock().unwrap();
    let members: Vec<ODataId> = store
        .accounts
        .iter()
        .map(|a| {
            ODataId::new(format!(
                "/redfish/v1/AccountService/Accounts/{}",
                a.username
            ))
        })
        .collect();

    Json(Collection::new(
        "/redfish/v1/AccountService/Accounts",
        "#ManagerAccountCollection.ManagerAccountCollection",
        "Account Collection",
        members,
    ))
}

#[derive(Debug, Serialize)]
pub struct AccountResource {
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
    #[serde(rename = "RoleId")]
    pub role_id: String,
    #[serde(rename = "Enabled")]
    pub enabled: bool,
    #[serde(rename = "Locked")]
    pub locked: bool,
}

pub async fn get_account(
    State(state): State<Arc<AppState>>,
    Path(account_id): Path<String>,
) -> Result<Json<AccountResource>, RedfishApiError> {
    let store = state.account_store.lock().unwrap();
    let account = store
        .find_account(&account_id)
        .ok_or_else(|| {
            RedfishApiError::NotFound(format!("Account '{account_id}' not found"))
        })?;

    Ok(Json(AccountResource {
        odata_id: format!("/redfish/v1/AccountService/Accounts/{}", account.username),
        odata_type: "#ManagerAccount.v1_12_0.ManagerAccount",
        id: account.username.clone(),
        name: format!("Account: {}", account.username),
        description: "User account",
        user_name: account.username.clone(),
        role_id: account.role.clone(),
        enabled: account.enabled,
        locked: account.locked,
    }))
}

#[derive(Debug, Deserialize)]
pub struct CreateAccountRequest {
    #[serde(rename = "UserName")]
    pub user_name: String,
    #[serde(rename = "Password")]
    pub password: String,
    #[serde(rename = "RoleId")]
    pub role_id: String,
}

pub async fn create_account(
    State(state): State<Arc<AppState>>,
    OptionalAuth(user): OptionalAuth,
    Json(body): Json<CreateAccountRequest>,
) -> Result<impl IntoResponse, RedfishApiError> {
    if state.config.auth.enabled {
        let u = user
            .ok_or_else(|| RedfishApiError::Unauthorized("Authentication required".to_string()))?;
        if !has_privilege(&u.role, Privilege::ConfigureUsers) {
            return Err(RedfishApiError::Forbidden(
                "Insufficient privileges".to_string(),
            ));
        }
    }

    let mut store = state.account_store.lock().unwrap();
    if store.find_account(&body.user_name).is_some() {
        return Err(RedfishApiError::Conflict(format!(
            "Account '{}' already exists",
            body.user_name
        )));
    }

    store
        .add_account(&body.user_name, &body.password, &body.role_id)
        .map_err(|e| RedfishApiError::InternalError(e.to_string()))?;

    if let Some(path) = &state.config.auth.accounts_file {
        store
            .save(path)
            .map_err(|e| RedfishApiError::InternalError(e.to_string()))?;
    }

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "UserName": body.user_name,
            "RoleId": body.role_id,
        })),
    ))
}

pub async fn get_roles() -> Json<Collection<ODataId>> {
    let members = vec![
        ODataId::new("/redfish/v1/AccountService/Roles/Administrator"),
        ODataId::new("/redfish/v1/AccountService/Roles/Operator"),
        ODataId::new("/redfish/v1/AccountService/Roles/ReadOnly"),
    ];

    Json(Collection::new(
        "/redfish/v1/AccountService/Roles",
        "#RoleCollection.RoleCollection",
        "Role Collection",
        members,
    ))
}

#[derive(Debug, Serialize)]
pub struct RoleResource {
    #[serde(rename = "@odata.id")]
    pub odata_id: String,
    #[serde(rename = "@odata.type")]
    pub odata_type: &'static str,
    #[serde(rename = "Id")]
    pub id: String,
    #[serde(rename = "RoleId")]
    pub role_id: String,
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Description")]
    pub description: &'static str,
    #[serde(rename = "IsPredefined")]
    pub is_predefined: bool,
    #[serde(rename = "AssignedPrivileges")]
    pub assigned_privileges: Vec<String>,
}

pub async fn get_role(
    Path(role_id): Path<String>,
) -> Result<Json<RoleResource>, RedfishApiError> {
    let privileges = match role_id.as_str() {
        "Administrator" => vec![
            "Login", "ConfigureManager", "ConfigureUsers",
            "ConfigureComponents", "ConfigureSelf",
        ],
        "Operator" => vec!["Login", "ConfigureComponents", "ConfigureSelf"],
        "ReadOnly" => vec!["Login", "ConfigureSelf"],
        _ => {
            return Err(RedfishApiError::NotFound(format!(
                "Role '{role_id}' not found"
            )));
        }
    };

    Ok(Json(RoleResource {
        odata_id: format!("/redfish/v1/AccountService/Roles/{role_id}"),
        odata_type: "#Role.v1_3_1.Role",
        id: role_id.clone(),
        role_id: role_id.clone(),
        name: format!("{role_id} Role"),
        description: "User role",
        is_predefined: true,
        assigned_privileges: privileges.into_iter().map(String::from).collect(),
    }))
}
