//! Chassis-scoped Certificate resources holding SPDM device certificate chains.
//!
//! This is the Redfish-conformant way for SPDM-capable BMCs to publish the
//! certificate chain of a device's root of trust: the chain is served as a
//! Certificate resource under the protecting Chassis, and referenced by link
//! from the ComponentIntegrity resource (`SPDM.IdentityAuthentication.
//! ResponderAuthentication.ComponentCertificate`) and from the
//! SPDMGetSignedMeasurements action response (`Certificate` member).

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use serde::Serialize;

use super::error::RedfishApiError;
use super::types::{Collection, ODataId};
use crate::app_state::AppState;
use crate::auth::AuthenticatedUser;

#[derive(Debug, Serialize)]
pub struct CertificateResource {
    #[serde(rename = "@odata.id")]
    pub odata_id: String,
    #[serde(rename = "@odata.type")]
    pub odata_type: &'static str,
    #[serde(rename = "Id")]
    pub id: String,
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Description")]
    pub description: String,
    /// The certificate chain in PEM form (leaf first), as obtained from the device
    /// via SPDM GET_CERTIFICATE.
    #[serde(rename = "CertificateString")]
    pub certificate_string: String,
    #[serde(rename = "CertificateType")]
    pub certificate_type: &'static str,
}

/// GET /redfish/v1/Chassis/{chassis_id}/Certificates
pub async fn get_certificates_collection(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
    Path(chassis_id): Path<String>,
) -> Result<Json<Collection<ODataId>>, RedfishApiError> {
    if chassis_id != state.chassis_id {
        return Err(RedfishApiError::NotFound(format!(
            "Chassis '{chassis_id}' not found"
        )));
    }

    let members: Vec<ODataId> = state
        .spdm_responders
        .iter()
        .map(|entry| {
            ODataId::new(format!(
                "/redfish/v1/Chassis/{chassis_id}/Certificates/{}",
                entry.key()
            ))
        })
        .collect();

    Ok(Json(Collection::new(
        format!("/redfish/v1/Chassis/{chassis_id}/Certificates"),
        "#CertificateCollection.CertificateCollection",
        "Certificate Collection",
        members,
    )))
}

/// GET /redfish/v1/Chassis/{chassis_id}/Certificates/{cert_id}
///
/// The `cert_id` is the ID of the system whose SPDM responder identity the
/// chain belongs to (i.e. the ComponentIntegrity ID).
pub async fn get_certificate(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
    Path((chassis_id, cert_id)): Path<(String, String)>,
) -> Result<Json<CertificateResource>, RedfishApiError> {
    if chassis_id != state.chassis_id {
        return Err(RedfishApiError::NotFound(format!(
            "Chassis '{chassis_id}' not found"
        )));
    }

    let responder = state
        .spdm_responders
        .get(&cert_id)
        .ok_or_else(|| RedfishApiError::NotFound(format!("Certificate '{cert_id}' not found")))?;

    Ok(Json(CertificateResource {
        odata_id: format!("/redfish/v1/Chassis/{chassis_id}/Certificates/{cert_id}"),
        odata_type: "#Certificate.v1_9_0.Certificate",
        id: cert_id.clone(),
        name: format!("SPDM Certificate Chain: {cert_id}"),
        description: format!(
            "SPDM certificate chain of the root of trust protecting {cert_id}, obtained via SPDM GET_CERTIFICATE"
        ),
        certificate_string: responder.cert_chain_pem(),
        certificate_type: "PEMchain",
    }))
}
