use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};

use super::error::RedfishApiError;
use super::types::{Collection, ODataId};
use crate::app_state::AppState;
use crate::auth::AuthenticatedUser;
use crate::auth::rbac::{Privilege, has_privilege};

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
    #[serde(rename = "LocalAccountAuth")]
    pub local_account_auth: &'static str,
    #[serde(rename = "AuthFailureLoggingThreshold")]
    pub auth_failure_logging_threshold: u32,
    #[serde(rename = "SupportedAccountTypes")]
    pub supported_account_types: Vec<&'static str>,
    #[serde(rename = "HTTPBasicAuth")]
    pub http_basic_auth: &'static str,
    #[serde(rename = "PasswordExpirationDays")]
    pub password_expiration_days: u32,
    #[serde(rename = "RequireChangePasswordAction")]
    pub require_change_password_action: bool,
    #[serde(rename = "RestrictedPrivileges")]
    pub restricted_privileges: Vec<&'static str>,
    #[serde(rename = "Status")]
    pub status: super::types::Status,
}

pub async fn get_account_service(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
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
        local_account_auth: "Enabled",
        auth_failure_logging_threshold: 3,
        supported_account_types: vec!["Redfish"],
        http_basic_auth: "Enabled",
        password_expiration_days: 0,
        require_change_password_action: false,
        restricted_privileges: Vec::new(),
        status: super::types::Status::enabled_ok(),
    })
}

pub async fn get_accounts(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
) -> Result<Json<Collection<ODataId>>, RedfishApiError> {
    let store = state
        .account_store
        .lock()
        .map_err(|_| RedfishApiError::InternalError("Account store lock poisoned".to_string()))?;
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

    Ok(Json(Collection::new(
        "/redfish/v1/AccountService/Accounts",
        "#ManagerAccountCollection.ManagerAccountCollection",
        "Account Collection",
        members,
    )))
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
    _user: AuthenticatedUser,
    Path(account_id): Path<String>,
) -> Result<Json<AccountResource>, RedfishApiError> {
    let store = state
        .account_store
        .lock()
        .map_err(|_| RedfishApiError::InternalError("Account store lock poisoned".to_string()))?;
    let account = store
        .find_account(&account_id)
        .ok_or_else(|| RedfishApiError::NotFound(format!("Account '{account_id}' not found")))?;

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
    user: AuthenticatedUser,
    Json(body): Json<CreateAccountRequest>,
) -> Result<impl IntoResponse, RedfishApiError> {
    if !has_privilege(&user.role, Privilege::ConfigureUsers) {
        return Err(RedfishApiError::Forbidden(
            "Insufficient privileges".to_string(),
        ));
    }

    let mut store = state
        .account_store
        .lock()
        .map_err(|_| RedfishApiError::InternalError("Account store lock poisoned".to_string()))?;
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

#[derive(Debug, Deserialize)]
pub struct PatchAccountRequest {
    #[serde(rename = "Password")]
    pub password: Option<String>,
    #[serde(rename = "RoleId")]
    pub role_id: Option<String>,
    #[serde(rename = "Enabled")]
    pub enabled: Option<bool>,
}

pub async fn patch_account(
    State(state): State<Arc<AppState>>,
    user: AuthenticatedUser,
    Path(account_id): Path<String>,
    Json(body): Json<PatchAccountRequest>,
) -> Result<Json<AccountResource>, RedfishApiError> {
    if user.username == account_id {
        if !has_privilege(&user.role, Privilege::ConfigureSelf) {
            return Err(RedfishApiError::Forbidden(
                "Insufficient privileges".to_string(),
            ));
        }
        if (body.role_id.is_some() || body.enabled.is_some())
            && !has_privilege(&user.role, Privilege::ConfigureUsers)
        {
            return Err(RedfishApiError::Forbidden(
                "Insufficient privileges to modify role or status".to_string(),
            ));
        }
    } else if !has_privilege(&user.role, Privilege::ConfigureUsers) {
        return Err(RedfishApiError::Forbidden(
            "Insufficient privileges".to_string(),
        ));
    }

    let mut store = state
        .account_store
        .lock()
        .map_err(|_| RedfishApiError::InternalError("Account store lock poisoned".to_string()))?;
    let account = store
        .find_account_mut(&account_id)
        .ok_or_else(|| RedfishApiError::NotFound(format!("Account '{account_id}' not found")))?;

    if let Some(role) = body.role_id {
        account.role = role;
    }
    if let Some(enabled) = body.enabled {
        account.enabled = enabled;
    }
    if let Some(password) = body.password {
        account
            .set_password(&password)
            .map_err(|e| RedfishApiError::InternalError(e.to_string()))?;
    }

    let result = AccountResource {
        odata_id: format!("/redfish/v1/AccountService/Accounts/{}", account.username),
        odata_type: "#ManagerAccount.v1_12_0.ManagerAccount",
        id: account.username.clone(),
        name: format!("Account: {}", account.username),
        description: "User account",
        user_name: account.username.clone(),
        role_id: account.role.clone(),
        enabled: account.enabled,
        locked: account.locked,
    };

    if let Some(path) = &state.config.auth.accounts_file {
        store
            .save(path)
            .map_err(|e| RedfishApiError::InternalError(e.to_string()))?;
    }

    Ok(Json(result))
}

pub async fn get_roles(_user: AuthenticatedUser) -> Json<Collection<ODataId>> {
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
    #[serde(rename = "AlternateRoleId")]
    pub alternate_role_id: String,
    #[serde(rename = "OemPrivileges")]
    pub oem_privileges: Vec<String>,
    #[serde(rename = "Restricted")]
    pub restricted: bool,
}

pub async fn get_role(
    _user: AuthenticatedUser,
    Path(role_id): Path<String>,
) -> Result<Json<RoleResource>, RedfishApiError> {
    let privileges = match role_id.as_str() {
        "Administrator" => vec![
            "Login",
            "ConfigureManager",
            "ConfigureUsers",
            "ConfigureComponents",
            "ConfigureSelf",
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
        alternate_role_id: role_id,
        oem_privileges: Vec::new(),
        restricted: false,
    }))
}

#[cfg(test)]
mod tests {
    use super::super::types::Status;
    use super::*;

    #[test]
    fn test_account_service_serialization() {
        let service = AccountServiceResource {
            odata_id: "/redfish/v1/AccountService",
            odata_type: "#AccountService.v1_15_0.AccountService",
            id: "AccountService",
            name: "Account Service",
            description: "Account management service",
            service_enabled: true,
            accounts: ODataId::new("/redfish/v1/AccountService/Accounts"),
            roles: ODataId::new("/redfish/v1/AccountService/Roles"),
            lockout_threshold: 5,
            lockout_duration: 300,
            min_password_length: 1,
            max_password_length: 128,
            lockout_counter_reset_after: 300,
            lockout_counter_reset_enabled: true,
            local_account_auth: "Enabled",
            auth_failure_logging_threshold: 3,
            supported_account_types: vec!["Redfish"],
            http_basic_auth: "Enabled",
            password_expiration_days: 0,
            require_change_password_action: false,
            restricted_privileges: Vec::new(),
            status: Status::enabled_ok(),
        };

        let value = serde_json::to_value(&service).unwrap();

        assert_eq!(value["@odata.id"], "/redfish/v1/AccountService");
        assert_eq!(
            value["@odata.type"],
            "#AccountService.v1_15_0.AccountService"
        );
        assert_eq!(value["Id"], "AccountService");
        assert_eq!(value["Name"], "Account Service");
        assert_eq!(value["ServiceEnabled"], true);
        assert_eq!(
            value["Accounts"]["@odata.id"],
            "/redfish/v1/AccountService/Accounts"
        );
        assert_eq!(
            value["Roles"]["@odata.id"],
            "/redfish/v1/AccountService/Roles"
        );
        assert_eq!(value["AccountLockoutThreshold"], 5);
        assert_eq!(value["AccountLockoutDuration"], 300);
        assert_eq!(value["MinPasswordLength"], 1);
        assert_eq!(value["MaxPasswordLength"], 128);
        assert_eq!(value["AccountLockoutCounterResetAfter"], 300);
        assert_eq!(value["AccountLockoutCounterResetEnabled"], true);
        assert_eq!(value["LocalAccountAuth"], "Enabled");
        assert_eq!(value["AuthFailureLoggingThreshold"], 3);
        assert_eq!(
            value["SupportedAccountTypes"],
            serde_json::json!(["Redfish"])
        );
        assert_eq!(value["HTTPBasicAuth"], "Enabled");
        assert_eq!(value["PasswordExpirationDays"], 0);
        assert_eq!(value["RequireChangePasswordAction"], false);
        assert_eq!(value["RestrictedPrivileges"], serde_json::json!([]));
    }

    #[test]
    fn test_account_resource_serialization() {
        let account = AccountResource {
            odata_id: "/redfish/v1/AccountService/Accounts/admin".to_string(),
            odata_type: "#ManagerAccount.v1_12_0.ManagerAccount",
            id: "admin".to_string(),
            name: "Account: admin".to_string(),
            description: "User account",
            user_name: "admin".to_string(),
            role_id: "Administrator".to_string(),
            enabled: true,
            locked: false,
        };

        let value = serde_json::to_value(&account).unwrap();

        assert_eq!(
            value["@odata.id"],
            "/redfish/v1/AccountService/Accounts/admin"
        );
        assert_eq!(
            value["@odata.type"],
            "#ManagerAccount.v1_12_0.ManagerAccount"
        );
        assert_eq!(value["Id"], "admin");
        assert_eq!(value["Name"], "Account: admin");
        assert_eq!(value["UserName"], "admin");
        assert_eq!(value["RoleId"], "Administrator");
        assert_eq!(value["Enabled"], true);
        assert_eq!(value["Locked"], false);
    }

    #[test]
    fn test_role_resource_serialization() {
        let role = RoleResource {
            odata_id: "/redfish/v1/AccountService/Roles/Administrator".to_string(),
            odata_type: "#Role.v1_3_1.Role",
            id: "Administrator".to_string(),
            role_id: "Administrator".to_string(),
            name: "Administrator Role".to_string(),
            description: "User role",
            is_predefined: true,
            assigned_privileges: vec![
                "Login".to_string(),
                "ConfigureManager".to_string(),
                "ConfigureUsers".to_string(),
            ],
            alternate_role_id: "Administrator".to_string(),
            oem_privileges: Vec::new(),
            restricted: false,
        };

        let value = serde_json::to_value(&role).unwrap();

        assert_eq!(
            value["@odata.id"],
            "/redfish/v1/AccountService/Roles/Administrator"
        );
        assert_eq!(value["@odata.type"], "#Role.v1_3_1.Role");
        assert_eq!(value["Id"], "Administrator");
        assert_eq!(value["RoleId"], "Administrator");
        assert_eq!(value["Name"], "Administrator Role");
        assert_eq!(value["IsPredefined"], true);
        assert_eq!(
            value["AssignedPrivileges"],
            serde_json::json!(["Login", "ConfigureManager", "ConfigureUsers"])
        );
        assert_eq!(value["AlternateRoleId"], "Administrator");
        assert_eq!(value["OemPrivileges"], serde_json::json!([]));
        assert_eq!(value["Restricted"], false);
    }

    #[test]
    fn test_create_account_request_deserialization() {
        let json = serde_json::json!({
            "UserName": "newuser",
            "Password": "password123",
            "RoleId": "Operator"
        });

        let request: CreateAccountRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.user_name, "newuser");
        assert_eq!(request.password, "password123");
        assert_eq!(request.role_id, "Operator");
    }

    #[test]
    fn test_patch_account_request_deserialization() {
        let json = serde_json::json!({
            "Password": "newpassword",
            "RoleId": "Administrator",
            "Enabled": false
        });

        let request: PatchAccountRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.password, Some("newpassword".to_string()));
        assert_eq!(request.role_id, Some("Administrator".to_string()));
        assert_eq!(request.enabled, Some(false));
    }

    #[test]
    fn test_patch_account_request_partial() {
        let json = serde_json::json!({
            "Password": "newpassword"
        });

        let request: PatchAccountRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.password, Some("newpassword".to_string()));
        assert_eq!(request.role_id, None);
        assert_eq!(request.enabled, None);
    }

    #[test]
    fn test_patch_account_request_empty() {
        let json = serde_json::json!({});

        let request: PatchAccountRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.password, None);
        assert_eq!(request.role_id, None);
        assert_eq!(request.enabled, None);
    }

    // Integration tests using the test harness
    use crate::backend::mock::MockBackend;
    use crate::redfish::test_harness::{app_state, get, router, systems_with};

    #[tokio::test]
    async fn test_get_account_service_handler() {
        let state = app_state(MockBackend::new(), systems_with("test-sys"));
        let app = router(state);

        let (status, json, _headers) = get(&app, "/redfish/v1/AccountService").await;

        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(json["@odata.id"], "/redfish/v1/AccountService");
        assert_eq!(
            json["@odata.type"],
            "#AccountService.v1_15_0.AccountService"
        );
        assert_eq!(json["Id"], "AccountService");
        assert_eq!(
            json["Accounts"]["@odata.id"],
            "/redfish/v1/AccountService/Accounts"
        );
        assert_eq!(
            json["Roles"]["@odata.id"],
            "/redfish/v1/AccountService/Roles"
        );
    }

    #[tokio::test]
    async fn test_get_accounts_collection_empty() {
        let state = app_state(MockBackend::new(), systems_with("test-sys"));
        let app = router(state);

        let (status, json, _headers) = get(&app, "/redfish/v1/AccountService/Accounts").await;

        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(json["@odata.id"], "/redfish/v1/AccountService/Accounts");
        assert_eq!(json["Members@odata.count"], 0);
        assert!(json["Members"].is_array());
    }

    #[tokio::test]
    async fn test_get_accounts_collection_with_accounts() {
        use crate::auth::accounts::AccountStore;
        use std::sync::Arc;

        let mut account_store = AccountStore::default();
        account_store
            .add_account("admin", "password", "Administrator")
            .unwrap();
        account_store
            .add_account("user1", "password", "ReadOnly")
            .unwrap();

        let mock = MockBackend::new();
        let config = crate::redfish::test_harness::test_config(systems_with("test-sys"));
        let state = Arc::new(crate::app_state::AppState::new(
            config,
            crate::backend::Backend::Mock(mock),
            account_store,
            None,
            None,
        ));
        let app = router(state);

        let (status, json, _headers) = get(&app, "/redfish/v1/AccountService/Accounts").await;

        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(json["Members@odata.count"], 2);
        assert!(json["Members"].is_array());
    }

    #[tokio::test]
    async fn test_get_account_not_found() {
        let state = app_state(MockBackend::new(), systems_with("test-sys"));
        let app = router(state);

        let (status, json, _headers) =
            get(&app, "/redfish/v1/AccountService/Accounts/nonexistent").await;

        assert_eq!(status, axum::http::StatusCode::NOT_FOUND);
        assert!(json["error"].is_object());
    }

    #[tokio::test]
    async fn test_get_account_found() {
        use crate::auth::accounts::AccountStore;
        use std::sync::Arc;

        let mut account_store = AccountStore::default();
        account_store
            .add_account("testuser", "password", "Operator")
            .unwrap();

        let mock = MockBackend::new();
        let config = crate::redfish::test_harness::test_config(systems_with("test-sys"));
        let state = Arc::new(crate::app_state::AppState::new(
            config,
            crate::backend::Backend::Mock(mock),
            account_store,
            None,
            None,
        ));
        let app = router(state);

        let (status, json, _headers) =
            get(&app, "/redfish/v1/AccountService/Accounts/testuser").await;

        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(
            json["@odata.id"],
            "/redfish/v1/AccountService/Accounts/testuser"
        );
        assert_eq!(json["UserName"], "testuser");
        assert_eq!(json["RoleId"], "Operator");
        assert_eq!(json["Enabled"], true);
        assert_eq!(json["Locked"], false);
    }

    #[tokio::test]
    async fn test_get_roles_collection() {
        let state = app_state(MockBackend::new(), systems_with("test-sys"));
        let app = router(state);

        let (status, json, _headers) = get(&app, "/redfish/v1/AccountService/Roles").await;

        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(json["@odata.id"], "/redfish/v1/AccountService/Roles");
        assert_eq!(json["Members@odata.count"], 3);
        assert!(json["Members"].is_array());
        let members = json["Members"].as_array().unwrap();
        assert!(
            members
                .iter()
                .any(|m| m["@odata.id"] == "/redfish/v1/AccountService/Roles/Administrator")
        );
        assert!(
            members
                .iter()
                .any(|m| m["@odata.id"] == "/redfish/v1/AccountService/Roles/Operator")
        );
        assert!(
            members
                .iter()
                .any(|m| m["@odata.id"] == "/redfish/v1/AccountService/Roles/ReadOnly")
        );
    }

    #[tokio::test]
    async fn test_get_role_administrator() {
        let state = app_state(MockBackend::new(), systems_with("test-sys"));
        let app = router(state);

        let (status, json, _headers) =
            get(&app, "/redfish/v1/AccountService/Roles/Administrator").await;

        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(
            json["@odata.id"],
            "/redfish/v1/AccountService/Roles/Administrator"
        );
        assert_eq!(json["RoleId"], "Administrator");
        assert_eq!(json["IsPredefined"], true);
        assert!(json["AssignedPrivileges"].is_array());
        let privs = json["AssignedPrivileges"].as_array().unwrap();
        assert!(privs.contains(&serde_json::Value::String("Login".to_string())));
        assert!(privs.contains(&serde_json::Value::String("ConfigureManager".to_string())));
    }

    #[tokio::test]
    async fn test_get_role_operator() {
        let state = app_state(MockBackend::new(), systems_with("test-sys"));
        let app = router(state);

        let (status, json, _headers) = get(&app, "/redfish/v1/AccountService/Roles/Operator").await;

        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(json["RoleId"], "Operator");
        assert!(json["AssignedPrivileges"].is_array());
    }

    #[tokio::test]
    async fn test_get_role_readonly() {
        let state = app_state(MockBackend::new(), systems_with("test-sys"));
        let app = router(state);

        let (status, json, _headers) = get(&app, "/redfish/v1/AccountService/Roles/ReadOnly").await;

        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(json["RoleId"], "ReadOnly");
    }

    #[tokio::test]
    async fn test_get_role_not_found() {
        let state = app_state(MockBackend::new(), systems_with("test-sys"));
        let app = router(state);

        let (status, json, _headers) =
            get(&app, "/redfish/v1/AccountService/Roles/InvalidRole").await;

        assert_eq!(status, axum::http::StatusCode::NOT_FOUND);
        assert!(json["error"].is_object());
    }
}

#[cfg(test)]
mod harness_tests {
    use crate::backend::mock::MockBackend;
    use crate::redfish::test_harness as h;
    use axum::http::{Method, StatusCode};
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_create_account_success() {
        let app = h::router(h::app_state(MockBackend::new(), HashMap::new()));
        let body = serde_json::json!({
            "UserName": "newuser",
            "Password": "hunter2pass",
            "RoleId": "Operator"
        });
        let (status, json, _) = h::request_json(
            &app,
            Method::POST,
            "/redfish/v1/AccountService/Accounts",
            body,
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(json["UserName"], "newuser");
        assert_eq!(json["RoleId"], "Operator");
    }

    #[tokio::test]
    async fn test_create_account_duplicate() {
        let state = h::app_state_with_accounts(
            MockBackend::new(),
            HashMap::new(),
            &[("bob", "bobpass12", "ReadOnly")],
        );
        let app = h::router(state);
        let body = serde_json::json!({
            "UserName": "bob",
            "Password": "another-pass",
            "RoleId": "Operator"
        });
        let (status, _, _) = h::request_json(
            &app,
            Method::POST,
            "/redfish/v1/AccountService/Accounts",
            body,
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn test_patch_account_success() {
        let state = h::app_state_with_accounts(
            MockBackend::new(),
            HashMap::new(),
            &[("bob", "bobpass12", "ReadOnly")],
        );
        let app = h::router(state);
        let body = serde_json::json!({ "RoleId": "Operator", "Enabled": false });
        let (status, json, _) = h::request_json(
            &app,
            Method::PATCH,
            "/redfish/v1/AccountService/Accounts/bob",
            body,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["RoleId"], "Operator");
        assert_eq!(json["Enabled"], false);
    }

    #[tokio::test]
    async fn test_patch_account_not_found() {
        let app = h::router(h::app_state(MockBackend::new(), HashMap::new()));
        let body = serde_json::json!({ "RoleId": "Operator" });
        let (status, _, _) = h::request_json(
            &app,
            Method::PATCH,
            "/redfish/v1/AccountService/Accounts/ghost",
            body,
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
}
