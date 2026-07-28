use axum::Json;
use serde::Serialize;

use super::types::ODataId;
use crate::auth::AuthenticatedUser;

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
