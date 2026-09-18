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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_certificate_service_serialization() {
        let service = CertificateServiceResource {
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
        };

        let value = serde_json::to_value(&service).unwrap();

        assert_eq!(value["@odata.id"], "/redfish/v1/CertificateService");
        assert_eq!(
            value["@odata.type"],
            "#CertificateService.v1_0_5.CertificateService"
        );
        assert_eq!(value["Id"], "CertificateService");
        assert_eq!(value["Name"], "Certificate Service");
        assert_eq!(
            value["CertificateLocations"]["@odata.id"],
            "/redfish/v1/CertificateService/CertificateLocations"
        );
        assert_eq!(
            value["Actions"]["#CertificateService.GenerateCSR"]["target"],
            "/redfish/v1/CertificateService/Actions/CertificateService.GenerateCSR"
        );
        assert_eq!(
            value["Actions"]["#CertificateService.ReplaceCertificate"]["target"],
            "/redfish/v1/CertificateService/Actions/CertificateService.ReplaceCertificate"
        );
    }

    #[test]
    fn test_action_target_serialization() {
        let action = ActionTarget {
            target: "/redfish/v1/test/action".to_string(),
        };

        let value = serde_json::to_value(&action).unwrap();
        assert_eq!(value["target"], "/redfish/v1/test/action");
    }

    #[test]
    fn test_certificate_actions_serialization() {
        let actions = CertificateActions {
            generate_csr: ActionTarget {
                target: "/redfish/v1/test/csr".to_string(),
            },
            replace_certificate: ActionTarget {
                target: "/redfish/v1/test/replace".to_string(),
            },
        };

        let value = serde_json::to_value(&actions).unwrap();
        assert_eq!(
            value["#CertificateService.GenerateCSR"]["target"],
            "/redfish/v1/test/csr"
        );
        assert_eq!(
            value["#CertificateService.ReplaceCertificate"]["target"],
            "/redfish/v1/test/replace"
        );
    }

    #[test]
    fn test_generate_csr_response_serialization() {
        let response = GenerateCSRResponse {
            csr_string:
                "-----BEGIN CERTIFICATE REQUEST-----\ntest\n-----END CERTIFICATE REQUEST-----"
                    .to_string(),
        };

        let value = serde_json::to_value(&response).unwrap();
        assert_eq!(
            value["CSRString"],
            "-----BEGIN CERTIFICATE REQUEST-----\ntest\n-----END CERTIFICATE REQUEST-----"
        );
    }

    #[test]
    fn test_generate_csr_request_deserialization() {
        let json = serde_json::json!({
            "CommonName": "test.example.com",
            "Organization": "Test Org",
            "Country": "US"
        });

        let request: GenerateCSRRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.common_name, "test.example.com");
        assert_eq!(request.organization, Some("Test Org".to_string()));
        assert_eq!(request.country, Some("US".to_string()));
        assert_eq!(request.organizational_unit, None);
    }

    #[test]
    fn test_generate_csr_request_with_optional_fields() {
        let json = serde_json::json!({
            "CommonName": "test.example.com"
        });

        let request: GenerateCSRRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.common_name, "test.example.com");
        assert_eq!(request.organization, None);
        assert_eq!(request.alternative_names, Vec::<String>::new());
    }

    #[test]
    fn test_replace_certificate_request_deserialization() {
        let json = serde_json::json!({
            "CertificateString": "-----BEGIN CERTIFICATE-----\ntest\n-----END CERTIFICATE-----",
            "CertificateType": "PEM"
        });

        let request: ReplaceCertificateRequest = serde_json::from_value(json).unwrap();
        assert_eq!(
            request.certificate_string,
            "-----BEGIN CERTIFICATE-----\ntest\n-----END CERTIFICATE-----"
        );
        assert_eq!(request.certificate_type, "PEM");
    }
}

#[cfg(test)]
mod harness_tests {
    use crate::backend::mock::MockBackend;
    use crate::redfish::test_harness as h;
    use axum::http::{Method, StatusCode};
    use std::collections::HashMap;

    // Auth is disabled by default, so the AuthenticatedUser extractor yields an
    // anonymous Administrator — which holds every privilege, letting these
    // ConfigureManager-guarded handlers reach their bodies.

    #[tokio::test]
    async fn test_get_certificate_service() {
        let app = h::router(h::app_state(MockBackend::new(), HashMap::new()));
        let (status, json, _) = h::get(&app, "/redfish/v1/CertificateService").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["Id"], "CertificateService");
        assert_eq!(
            json["Actions"]["#CertificateService.GenerateCSR"]["target"],
            "/redfish/v1/CertificateService/Actions/CertificateService.GenerateCSR"
        );
    }

    #[tokio::test]
    async fn test_get_certificate_locations() {
        let app = h::router(h::app_state(MockBackend::new(), HashMap::new()));
        let (status, json, _) =
            h::get(&app, "/redfish/v1/CertificateService/CertificateLocations").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["Id"], "CertificateLocations");
    }

    #[tokio::test]
    async fn test_generate_csr_success() {
        let app = h::router(h::app_state(MockBackend::new(), HashMap::new()));
        let body = serde_json::json!({
            "CommonName": "bmc.example.com",
            "Organization": "Example",
            "Country": "US"
        });
        let (status, json, _) = h::request_json(
            &app,
            Method::POST,
            "/redfish/v1/CertificateService/Actions/CertificateService.GenerateCSR",
            body,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(
            json["CSRString"]
                .as_str()
                .unwrap()
                .contains("CERTIFICATE REQUEST")
        );
    }

    #[tokio::test]
    async fn test_replace_certificate_no_tls_configured() {
        let app = h::router(h::app_state(MockBackend::new(), HashMap::new()));
        let body = serde_json::json!({
            "CertificateString": "-----BEGIN CERTIFICATE-----\nx\n-----END CERTIFICATE-----",
            "CertificateType": "PEM"
        });
        let (status, _, _) = h::request_json(
            &app,
            Method::POST,
            "/redfish/v1/CertificateService/Actions/CertificateService.ReplaceCertificate",
            body,
        )
        .await;
        // TLS is not configured in the test harness -> BadRequest.
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_replace_certificate_bad_type() {
        let app = h::router(h::app_state(MockBackend::new(), HashMap::new()));
        let body = serde_json::json!({
            "CertificateString": "x",
            "CertificateType": "DER"
        });
        let (status, _, _) = h::request_json(
            &app,
            Method::POST,
            "/redfish/v1/CertificateService/Actions/CertificateService.ReplaceCertificate",
            body,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }
}
