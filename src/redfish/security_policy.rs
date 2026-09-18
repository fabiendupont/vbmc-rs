use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use serde::{Deserialize, Serialize};

use super::error::RedfishApiError;
use crate::app_state::AppState;
use crate::auth::AuthenticatedUser;
use crate::auth::rbac::{Privilege, has_privilege};

#[derive(Debug, Serialize)]
pub struct SecurityPolicyResource {
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
    #[serde(rename = "SPDM")]
    pub spdm: SpdmPolicy,
    #[serde(rename = "TLS")]
    pub tls: TlsPolicy,
}

#[derive(Debug, Serialize)]
pub struct SpdmPolicy {
    #[serde(rename = "Enabled")]
    pub enabled: bool,
}

#[derive(Debug, Serialize)]
pub struct TlsPolicy {
    #[serde(rename = "MinimumVersion", skip_serializing_if = "Option::is_none")]
    pub minimum_version: Option<String>,
}

pub async fn get_security_policy(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
) -> Result<Json<SecurityPolicyResource>, RedfishApiError> {
    let policy = state
        .security_policy
        .read()
        .map_err(|_| RedfishApiError::InternalError("Security policy lock poisoned".to_string()))?;
    Ok(Json(SecurityPolicyResource {
        odata_id: "/redfish/v1/SecurityPolicy",
        odata_type: "#SecurityPolicy.v1_0_0.SecurityPolicy",
        id: "SecurityPolicy",
        name: "Security Policy",
        description: "Security policy configuration",
        spdm: SpdmPolicy {
            enabled: policy.spdm_enabled,
        },
        tls: TlsPolicy {
            minimum_version: policy.tls_minimum_version.clone(),
        },
    }))
}

#[derive(Debug, Deserialize)]
pub struct PatchSecurityPolicyRequest {
    #[serde(rename = "SPDM")]
    pub spdm: Option<PatchSpdmPolicy>,
    #[serde(rename = "TLS")]
    pub tls: Option<PatchTlsPolicy>,
}

#[derive(Debug, Deserialize)]
pub struct PatchSpdmPolicy {
    #[serde(rename = "Enabled")]
    pub enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct PatchTlsPolicy {
    #[serde(rename = "MinimumVersion")]
    pub minimum_version: Option<String>,
}

pub async fn patch_security_policy(
    State(state): State<Arc<AppState>>,
    user: AuthenticatedUser,
    Json(body): Json<PatchSecurityPolicyRequest>,
) -> Result<Json<SecurityPolicyResource>, RedfishApiError> {
    if !has_privilege(&user.role, Privilege::ConfigureManager) {
        return Err(RedfishApiError::Forbidden(
            "Insufficient privileges".to_string(),
        ));
    }

    let mut policy = state
        .security_policy
        .write()
        .map_err(|_| RedfishApiError::InternalError("Security policy lock poisoned".to_string()))?;

    if let Some(spdm) = &body.spdm
        && let Some(enabled) = spdm.enabled
    {
        policy.spdm_enabled = enabled;
    }

    if let Some(tls) = &body.tls
        && let Some(version) = &tls.minimum_version
    {
        policy.tls_minimum_version = Some(version.clone());
    }

    Ok(Json(SecurityPolicyResource {
        odata_id: "/redfish/v1/SecurityPolicy",
        odata_type: "#SecurityPolicy.v1_0_0.SecurityPolicy",
        id: "SecurityPolicy",
        name: "Security Policy",
        description: "Security policy configuration",
        spdm: SpdmPolicy {
            enabled: policy.spdm_enabled,
        },
        tls: TlsPolicy {
            minimum_version: policy.tls_minimum_version.clone(),
        },
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_security_policy_resource_serialization() {
        let policy = SecurityPolicyResource {
            odata_id: "/redfish/v1/SecurityPolicy",
            odata_type: "#SecurityPolicy.v1_0_0.SecurityPolicy",
            id: "SecurityPolicy",
            name: "Security Policy",
            description: "Security policy configuration",
            spdm: SpdmPolicy { enabled: true },
            tls: TlsPolicy {
                minimum_version: Some("TLS_1_2".to_string()),
            },
        };

        let json = serde_json::to_value(&policy).unwrap();
        assert_eq!(json["@odata.id"], "/redfish/v1/SecurityPolicy");
        assert_eq!(json["@odata.type"], "#SecurityPolicy.v1_0_0.SecurityPolicy");
        assert_eq!(json["Id"], "SecurityPolicy");
        assert_eq!(json["Name"], "Security Policy");
        assert_eq!(json["Description"], "Security policy configuration");
        assert_eq!(json["SPDM"]["Enabled"], true);
        assert_eq!(json["TLS"]["MinimumVersion"], "TLS_1_2");
    }

    #[test]
    fn test_spdm_policy_serialization() {
        let spdm_enabled = SpdmPolicy { enabled: true };
        let json_enabled = serde_json::to_value(&spdm_enabled).unwrap();
        assert_eq!(json_enabled["Enabled"], true);

        let spdm_disabled = SpdmPolicy { enabled: false };
        let json_disabled = serde_json::to_value(&spdm_disabled).unwrap();
        assert_eq!(json_disabled["Enabled"], false);
    }

    #[test]
    fn test_tls_policy_serialization_with_version() {
        let tls = TlsPolicy {
            minimum_version: Some("TLS_1_3".to_string()),
        };

        let json = serde_json::to_value(&tls).unwrap();
        assert_eq!(json["MinimumVersion"], "TLS_1_3");
    }

    #[test]
    fn test_tls_policy_serialization_without_version() {
        let tls = TlsPolicy {
            minimum_version: None,
        };

        let json = serde_json::to_value(&tls).unwrap();
        // MinimumVersion should be absent when None
        assert!(!json.as_object().unwrap().contains_key("MinimumVersion"));
    }

    #[test]
    fn test_patch_security_policy_request_deserialization_full() {
        let json_str = r#"{
            "SPDM": {
                "Enabled": true
            },
            "TLS": {
                "MinimumVersion": "TLS_1_3"
            }
        }"#;

        let request: PatchSecurityPolicyRequest = serde_json::from_str(json_str).unwrap();
        assert!(request.spdm.is_some());
        assert_eq!(request.spdm.unwrap().enabled, Some(true));
        assert!(request.tls.is_some());
        assert_eq!(
            request.tls.unwrap().minimum_version,
            Some("TLS_1_3".to_string())
        );
    }

    #[test]
    fn test_patch_security_policy_request_deserialization_spdm_only() {
        let json_str = r#"{
            "SPDM": {
                "Enabled": false
            }
        }"#;

        let request: PatchSecurityPolicyRequest = serde_json::from_str(json_str).unwrap();
        assert!(request.spdm.is_some());
        assert_eq!(request.spdm.unwrap().enabled, Some(false));
        assert!(request.tls.is_none());
    }

    #[test]
    fn test_patch_security_policy_request_deserialization_tls_only() {
        let json_str = r#"{
            "TLS": {
                "MinimumVersion": "TLS_1_2"
            }
        }"#;

        let request: PatchSecurityPolicyRequest = serde_json::from_str(json_str).unwrap();
        assert!(request.spdm.is_none());
        assert!(request.tls.is_some());
        assert_eq!(
            request.tls.unwrap().minimum_version,
            Some("TLS_1_2".to_string())
        );
    }

    #[test]
    fn test_patch_security_policy_request_deserialization_empty() {
        let json_str = r#"{}"#;

        let request: PatchSecurityPolicyRequest = serde_json::from_str(json_str).unwrap();
        assert!(request.spdm.is_none());
        assert!(request.tls.is_none());
    }

    #[test]
    fn test_patch_spdm_policy_deserialization() {
        let json_str = r#"{"Enabled": true}"#;
        let patch: PatchSpdmPolicy = serde_json::from_str(json_str).unwrap();
        assert_eq!(patch.enabled, Some(true));

        let json_str_none = r#"{}"#;
        let patch_none: PatchSpdmPolicy = serde_json::from_str(json_str_none).unwrap();
        assert!(patch_none.enabled.is_none());
    }

    #[test]
    fn test_patch_tls_policy_deserialization() {
        let json_str = r#"{"MinimumVersion": "TLS_1_3"}"#;
        let patch: PatchTlsPolicy = serde_json::from_str(json_str).unwrap();
        assert_eq!(patch.minimum_version, Some("TLS_1_3".to_string()));

        let json_str_none = r#"{}"#;
        let patch_none: PatchTlsPolicy = serde_json::from_str(json_str_none).unwrap();
        assert!(patch_none.minimum_version.is_none());
    }
}
