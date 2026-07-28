use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use serde::{Deserialize, Serialize};

use super::error::RedfishApiError;
use crate::app_state::AppState;

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
    Json(body): Json<PatchSecurityPolicyRequest>,
) -> Result<Json<SecurityPolicyResource>, RedfishApiError> {
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
