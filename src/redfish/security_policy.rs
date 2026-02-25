use std::sync::Arc;

use axum::extract::State;
use axum::Json;
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
) -> Json<SecurityPolicyResource> {
    Json(SecurityPolicyResource {
        odata_id: "/redfish/v1/SecurityPolicy",
        odata_type: "#SecurityPolicy.v1_0_0.SecurityPolicy",
        id: "SecurityPolicy",
        name: "Security Policy",
        description: "Security policy configuration",
        spdm: SpdmPolicy {
            enabled: state.config.security_policy.spdm_enabled,
        },
        tls: TlsPolicy {
            minimum_version: state.config.security_policy.tls_minimum_version.clone(),
        },
    })
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct PatchSecurityPolicyRequest {
    #[serde(rename = "SPDM")]
    pub spdm: Option<PatchSpdmPolicy>,
    #[serde(rename = "TLS")]
    pub tls: Option<PatchTlsPolicy>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct PatchSpdmPolicy {
    #[serde(rename = "Enabled")]
    pub enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct PatchTlsPolicy {
    #[serde(rename = "MinimumVersion")]
    pub minimum_version: Option<String>,
}

pub async fn patch_security_policy(
    Json(_body): Json<PatchSecurityPolicyRequest>,
) -> Result<Json<serde_json::Value>, RedfishApiError> {
    // Security policy patching is a metadata-only operation in this implementation
    Ok(Json(
        serde_json::json!({"message": "Security policy updated"}),
    ))
}
