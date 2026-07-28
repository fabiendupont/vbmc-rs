use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use serde::{Deserialize, Serialize};

use super::error::RedfishApiError;
use super::types::ODataId;
use crate::app_state::AppState;
use crate::auth::AuthenticatedUser;
use crate::auth::rbac::{Privilege, has_privilege};

#[derive(Debug, Serialize)]
pub struct CertificateServiceResource {
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
    #[serde(rename = "CertificateLocations")]
    pub certificate_locations: ODataId,
    #[serde(rename = "Actions")]
    pub actions: CertificateActions,
}

#[derive(Debug, Serialize)]
pub struct CertificateActions {
    #[serde(rename = "#CertificateService.GenerateCSR")]
    pub generate_csr: ActionTarget,
    #[serde(rename = "#CertificateService.ReplaceCertificate")]
    pub replace_certificate: ActionTarget,
}

#[derive(Debug, Serialize)]
pub struct ActionTarget {
    pub target: String,
}

pub async fn get_certificate_service(_user: AuthenticatedUser) -> Json<CertificateServiceResource> {
    Json(CertificateServiceResource {
        odata_id: "/redfish/v1/CertificateService",
        odata_type: "#CertificateService.v1_0_5.CertificateService",
        id: "CertificateService",
        name: "Certificate Service",
        description: "Certificate management service",
        certificate_locations: ODataId::new("/redfish/v1/CertificateService/CertificateLocations"),
        actions: CertificateActions {
            generate_csr: ActionTarget {
                target: "/redfish/v1/CertificateService/Actions/CertificateService.GenerateCSR"
                    .to_string(),
            },
            replace_certificate: ActionTarget {
                target:
                    "/redfish/v1/CertificateService/Actions/CertificateService.ReplaceCertificate"
                        .to_string(),
            },
        },
    })
}

pub async fn get_certificate_locations(_user: AuthenticatedUser) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "@odata.id": "/redfish/v1/CertificateService/CertificateLocations",
        "@odata.type": "#CertificateLocations.v1_0_3.CertificateLocations",
        "Id": "CertificateLocations",
        "Name": "Certificate Locations",
        "Links": {
            "Certificates": []
        }
    }))
}

#[derive(Debug, Deserialize)]
pub struct GenerateCSRRequest {
    #[serde(rename = "CommonName")]
    pub common_name: String,
    #[serde(rename = "Organization", default)]
    pub organization: Option<String>,
    #[serde(rename = "OrganizationalUnit", default)]
    pub organizational_unit: Option<String>,
    #[serde(rename = "Country", default)]
    pub country: Option<String>,
    #[serde(rename = "State", default)]
    pub state: Option<String>,
    #[serde(rename = "City", default)]
    pub city: Option<String>,
    #[serde(rename = "AlternativeNames", default)]
    pub alternative_names: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct GenerateCSRResponse {
    #[serde(rename = "CSRString")]
    pub csr_string: String,
}

pub async fn generate_csr(
    user: AuthenticatedUser,
    Json(body): Json<GenerateCSRRequest>,
) -> Result<Json<GenerateCSRResponse>, RedfishApiError> {
    if !has_privilege(&user.role, Privilege::ConfigureManager) {
        return Err(RedfishApiError::Forbidden(
            "Insufficient privileges".to_string(),
        ));
    }

    let mut params = rcgen::CertificateParams::new(body.alternative_names.clone())
        .map_err(|e| RedfishApiError::BadRequest(format!("Invalid alternative names: {e}")))?;

    let mut dn = rcgen::DistinguishedName::new();
    dn.push(rcgen::DnType::CommonName, &body.common_name);
    if let Some(org) = &body.organization {
        dn.push(rcgen::DnType::OrganizationName, org);
    }
    if let Some(ou) = &body.organizational_unit {
        dn.push(rcgen::DnType::OrganizationalUnitName, ou);
    }
    if let Some(country) = &body.country {
        dn.push(rcgen::DnType::CountryName, country);
    }
    if let Some(state) = &body.state {
        dn.push(rcgen::DnType::StateOrProvinceName, state);
    }
    if let Some(city) = &body.city {
        dn.push(rcgen::DnType::LocalityName, city);
    }
    params.distinguished_name = dn;

    let key_pair = rcgen::KeyPair::generate()
        .map_err(|e| RedfishApiError::InternalError(format!("Failed to generate key pair: {e}")))?;
    let csr = params
        .serialize_request(&key_pair)
        .map_err(|e| RedfishApiError::InternalError(format!("Failed to generate CSR: {e}")))?;

    Ok(Json(GenerateCSRResponse {
        csr_string: csr.pem().map_err(|e| {
            RedfishApiError::InternalError(format!("Failed to encode CSR as PEM: {e}"))
        })?,
    }))
}

#[derive(Debug, Deserialize)]
pub struct ReplaceCertificateRequest {
    #[serde(rename = "CertificateString")]
    pub certificate_string: String,
    #[serde(rename = "CertificateType")]
    pub certificate_type: String,
}

pub async fn replace_certificate(
    State(state): State<Arc<AppState>>,
    user: AuthenticatedUser,
    Json(body): Json<ReplaceCertificateRequest>,
) -> Result<Json<serde_json::Value>, RedfishApiError> {
    if !has_privilege(&user.role, Privilege::ConfigureManager) {
        return Err(RedfishApiError::Forbidden(
            "Insufficient privileges".to_string(),
        ));
    }

    if body.certificate_type != "PEM" {
        return Err(RedfishApiError::BadRequest(
            "Only PEM certificate type is supported".to_string(),
        ));
    }

    let cert_path = state
        .config
        .server
        .tls_cert
        .as_ref()
        .ok_or_else(|| RedfishApiError::BadRequest("TLS is not configured".to_string()))?;
    let key_path = state
        .config
        .server
        .tls_key
        .as_ref()
        .ok_or_else(|| RedfishApiError::BadRequest("TLS is not configured".to_string()))?;

    std::fs::write(cert_path, &body.certificate_string)
        .map_err(|e| RedfishApiError::InternalError(format!("Failed to write certificate: {e}")))?;

    if let Some(tls_config) = &state.tls_config {
        tls_config
            .reload_from_pem_file(cert_path, key_path)
            .await
            .map_err(|e| {
                RedfishApiError::InternalError(format!("Failed to reload TLS config: {e}"))
            })?;
    }

    Ok(Json(serde_json::json!({
        "message": "Certificate replaced successfully"
    })))
}
