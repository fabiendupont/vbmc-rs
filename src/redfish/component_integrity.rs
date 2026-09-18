use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use serde::Serialize;

use super::error::RedfishApiError;
use super::types::{Collection, ODataId, Status};
use crate::app_state::AppState;
use crate::attestation::trust_chain::AttestationEvidence;
use crate::auth::AuthenticatedUser;

#[derive(Debug, Serialize)]
pub struct ComponentIntegrityResource {
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
    #[serde(rename = "ComponentIntegrityType")]
    pub component_integrity_type: &'static str,
    #[serde(rename = "ComponentIntegrityTypeVersion")]
    pub component_integrity_type_version: &'static str,
    #[serde(rename = "ComponentIntegrityEnabled")]
    pub component_integrity_enabled: bool,
    #[serde(rename = "TargetComponentURI")]
    pub target_component_uri: String,
    #[serde(rename = "LastUpdated")]
    pub last_updated: String,
    #[serde(rename = "Status")]
    pub status: Status,
    #[serde(rename = "Links")]
    pub links: ComponentIntegrityLinks,
    #[serde(rename = "SPDM", skip_serializing_if = "Option::is_none")]
    pub spdm: Option<SpdmInfo>,
}

#[derive(Debug, Serialize)]
pub struct ComponentIntegrityLinks {
    #[serde(rename = "ComponentsProtected")]
    pub components_protected: Vec<ODataId>,
}

#[derive(Debug, Serialize)]
pub struct SpdmInfo {
    #[serde(rename = "Requester")]
    pub requester: ODataId,
    #[serde(rename = "MeasurementSet", skip_serializing_if = "Option::is_none")]
    pub measurement_set: Option<SpdmMeasurementSet>,
    #[serde(
        rename = "IdentityAuthentication",
        skip_serializing_if = "Option::is_none"
    )]
    pub identity_authentication: Option<SpdmIdentity>,
    #[serde(
        rename = "ComponentCommunication",
        skip_serializing_if = "Option::is_none"
    )]
    pub component_communication: Option<SpdmCommunication>,
}

#[derive(Debug, Serialize)]
pub struct SpdmMeasurementSet {
    #[serde(
        rename = "MeasurementSpecification",
        skip_serializing_if = "Option::is_none"
    )]
    pub measurement_specification: Option<String>,
    #[serde(rename = "Measurements", skip_serializing_if = "Option::is_none")]
    pub measurements: Option<Vec<SpdmSingleMeasurement>>,
    #[serde(rename = "MeasurementSummary", skip_serializing_if = "Option::is_none")]
    pub measurement_summary: Option<String>,
    #[serde(
        rename = "MeasurementSummaryHashAlgorithm",
        skip_serializing_if = "Option::is_none"
    )]
    pub measurement_summary_hash_algorithm: Option<String>,
    #[serde(
        rename = "MeasurementSummaryType",
        skip_serializing_if = "Option::is_none"
    )]
    pub measurement_summary_type: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SpdmSingleMeasurement {
    #[serde(rename = "MeasurementIndex")]
    pub measurement_index: u32,
    #[serde(rename = "MeasurementType", skip_serializing_if = "Option::is_none")]
    pub measurement_type: Option<String>,
    #[serde(rename = "Measurement", skip_serializing_if = "Option::is_none")]
    pub measurement: Option<String>,
    #[serde(
        rename = "MeasurementHashAlgorithm",
        skip_serializing_if = "Option::is_none"
    )]
    pub measurement_hash_algorithm: Option<String>,
    #[serde(rename = "PartofSummaryHash", skip_serializing_if = "Option::is_none")]
    pub part_of_summary_hash: Option<bool>,
    #[serde(rename = "LastUpdated", skip_serializing_if = "Option::is_none")]
    pub last_updated: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SpdmIdentity {
    #[serde(rename = "ResponderAuthentication")]
    pub responder_authentication: SpdmResponderAuth,
    #[serde(
        rename = "RequesterAuthentication",
        skip_serializing_if = "Option::is_none"
    )]
    pub requester_authentication: Option<SpdmRequesterAuth>,
}

#[derive(Debug, Serialize)]
pub struct SpdmResponderAuth {
    #[serde(rename = "VerificationStatus")]
    pub verification_status: String,
    #[serde(
        rename = "ComponentCertificate",
        skip_serializing_if = "Option::is_none"
    )]
    pub component_certificate: Option<ODataId>,
}

#[derive(Debug, Serialize)]
pub struct SpdmRequesterAuth {
    #[serde(rename = "VerificationStatus")]
    pub verification_status: String,
}

#[derive(Debug, Serialize)]
pub struct SpdmCommunication {
    #[serde(rename = "Sessions", skip_serializing_if = "Option::is_none")]
    pub sessions: Option<Vec<SpdmSession>>,
}

#[derive(Debug, Serialize)]
pub struct SpdmSession {
    #[serde(rename = "SessionId")]
    pub session_id: u32,
    #[serde(rename = "SessionType")]
    pub session_type: String,
}

fn build_spdm_from_evidence(system_id: &str, evidence: &AttestationEvidence) -> SpdmInfo {
    let measurements: Vec<SpdmSingleMeasurement> = evidence
        .measurements
        .iter()
        .map(|m| SpdmSingleMeasurement {
            measurement_index: m.index,
            measurement_type: Some(m.measurement_type.clone()),
            measurement: Some(m.measurement.clone()),
            measurement_hash_algorithm: Some(m.hash_algorithm.clone()),
            part_of_summary_hash: Some(m.part_of_summary),
            last_updated: m.last_updated.clone(),
        })
        .collect();

    let measurement_set = if !measurements.is_empty() || evidence.measurement_summary.is_some() {
        Some(SpdmMeasurementSet {
            measurement_specification: Some("DMTF".to_string()),
            measurements: if measurements.is_empty() {
                None
            } else {
                Some(measurements)
            },
            measurement_summary: evidence.measurement_summary.clone(),
            measurement_summary_hash_algorithm: evidence.measurement_summary_algorithm.clone(),
            measurement_summary_type: evidence.measurement_summary_type.clone(),
        })
    } else {
        None
    };

    let identity_authentication = evidence
        .responder_verification
        .as_ref()
        .map(|v| SpdmIdentity {
            responder_authentication: SpdmResponderAuth {
                verification_status: v.to_string(),
                component_certificate: None,
            },
            requester_authentication: None,
        });

    SpdmInfo {
        requester: ODataId::new(format!("/redfish/v1/ComponentIntegrity/{system_id}")),
        measurement_set,
        identity_authentication,
        component_communication: None,
    }
}

pub async fn get_component_integrity_collection(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
) -> Json<Collection<ODataId>> {
    let members: Vec<ODataId> = state
        .config
        .systems
        .keys()
        .map(|id| ODataId::new(format!("/redfish/v1/ComponentIntegrity/{id}")))
        .collect();

    Json(Collection::new(
        "/redfish/v1/ComponentIntegrity",
        "#ComponentIntegrityCollection.ComponentIntegrityCollection",
        "Component Integrity Collection",
        members,
    ))
}

pub async fn get_component_integrity(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
    Path(system_id): Path<String>,
) -> Result<Json<ComponentIntegrityResource>, RedfishApiError> {
    if !state.config.systems.contains_key(&system_id) {
        return Err(RedfishApiError::NotFound(format!(
            "ComponentIntegrity '{system_id}' not found"
        )));
    }

    let vm_state = state.get_vm_state(&system_id);
    let verification_status = vm_state
        .attestation
        .verification_status
        .unwrap_or_else(|| "Unknown".to_string());

    let health = match verification_status.as_str() {
        "Success" => "OK",
        "Failed" => "Critical",
        _ => "Warning",
    };

    let spdm = match &vm_state.attestation.evidence {
        Some(evidence) => build_spdm_from_evidence(&system_id, evidence),
        None => SpdmInfo {
            requester: ODataId::new(format!("/redfish/v1/ComponentIntegrity/{system_id}")),
            measurement_set: Some(SpdmMeasurementSet {
                measurement_specification: Some("DMTF".to_string()),
                measurements: Some(Vec::new()),
                measurement_summary: None,
                measurement_summary_hash_algorithm: None,
                measurement_summary_type: None,
            }),
            identity_authentication: Some(SpdmIdentity {
                responder_authentication: SpdmResponderAuth {
                    verification_status: "Success".to_string(),
                    component_certificate: None,
                },
                requester_authentication: None,
            }),
            component_communication: Some(SpdmCommunication {
                sessions: Some(Vec::new()),
            }),
        },
    };

    Ok(Json(ComponentIntegrityResource {
        odata_id: format!("/redfish/v1/ComponentIntegrity/{system_id}"),
        odata_type: "#ComponentIntegrity.v1_2_0.ComponentIntegrity",
        id: system_id.clone(),
        name: format!("Integrity: {system_id}"),
        description: format!("SPDM integrity status for {system_id}"),
        component_integrity_type: "SPDM",
        component_integrity_type_version: "1.0",
        component_integrity_enabled: true,
        target_component_uri: format!(
            "/redfish/v1/Chassis/{}/TrustedComponents/{system_id}",
            state.chassis_id
        ),
        last_updated: vm_state
            .attestation
            .last_checked
            .clone()
            .unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()),
        status: Status {
            state: Some("Enabled".to_string()),
            health: Some(health.to_string()),
            health_rollup: Some(health.to_string()),
        },
        links: ComponentIntegrityLinks {
            components_protected: vec![ODataId::new(format!("/redfish/v1/Systems/{system_id}"))],
        },
        spdm: Some(spdm),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attestation::trust_chain::{MeasurementEntry, VerificationStatus};

    #[test]
    fn test_component_integrity_resource_serialization() {
        let resource = ComponentIntegrityResource {
            odata_id: "/redfish/v1/ComponentIntegrity/vm1".to_string(),
            odata_type: "#ComponentIntegrity.v1_2_0.ComponentIntegrity",
            id: "vm1".to_string(),
            name: "Integrity: vm1".to_string(),
            description: "SPDM integrity status for vm1".to_string(),
            component_integrity_type: "SPDM",
            component_integrity_type_version: "1.0",
            component_integrity_enabled: true,
            target_component_uri: "/redfish/v1/Chassis/host/TrustedComponents/vm1".to_string(),
            last_updated: "2024-01-01T00:00:00Z".to_string(),
            status: Status {
                state: Some("Enabled".to_string()),
                health: Some("OK".to_string()),
                health_rollup: Some("OK".to_string()),
            },
            links: ComponentIntegrityLinks {
                components_protected: vec![ODataId::new("/redfish/v1/Systems/vm1".to_string())],
            },
            spdm: None,
        };

        let json = serde_json::to_value(&resource).unwrap();
        assert_eq!(json["@odata.id"], "/redfish/v1/ComponentIntegrity/vm1");
        assert_eq!(
            json["@odata.type"],
            "#ComponentIntegrity.v1_2_0.ComponentIntegrity"
        );
        assert_eq!(json["Id"], "vm1");
        assert_eq!(json["Name"], "Integrity: vm1");
        assert_eq!(json["Description"], "SPDM integrity status for vm1");
        assert_eq!(json["ComponentIntegrityType"], "SPDM");
        assert_eq!(json["ComponentIntegrityTypeVersion"], "1.0");
        assert_eq!(json["ComponentIntegrityEnabled"], true);
        assert_eq!(
            json["TargetComponentURI"],
            "/redfish/v1/Chassis/host/TrustedComponents/vm1"
        );
        assert_eq!(json["LastUpdated"], "2024-01-01T00:00:00Z");
        assert_eq!(json["Status"]["State"], "Enabled");
        assert_eq!(json["Status"]["Health"], "OK");
        assert_eq!(json["Status"]["HealthRollup"], "OK");
        assert!(json["Links"]["ComponentsProtected"].is_array());
        // SPDM should be absent when None
        assert!(!json.as_object().unwrap().contains_key("SPDM"));
    }

    #[test]
    fn test_spdm_info_serialization_with_all_fields() {
        let spdm = SpdmInfo {
            requester: ODataId::new("/redfish/v1/ComponentIntegrity/vm1".to_string()),
            measurement_set: Some(SpdmMeasurementSet {
                measurement_specification: Some("DMTF".to_string()),
                measurements: Some(vec![SpdmSingleMeasurement {
                    measurement_index: 1,
                    measurement_type: Some("ImmutableROM".to_string()),
                    measurement: Some("aabbccdd".to_string()),
                    measurement_hash_algorithm: Some("TPM_ALG_SHA_384".to_string()),
                    part_of_summary_hash: Some(true),
                    last_updated: Some("2024-01-01T00:00:00Z".to_string()),
                }]),
                measurement_summary: Some("summary_hash".to_string()),
                measurement_summary_hash_algorithm: Some("TPM_ALG_SHA_384".to_string()),
                measurement_summary_type: Some("All".to_string()),
            }),
            identity_authentication: Some(SpdmIdentity {
                responder_authentication: SpdmResponderAuth {
                    verification_status: "Success".to_string(),
                    component_certificate: None,
                },
                requester_authentication: Some(SpdmRequesterAuth {
                    verification_status: "Success".to_string(),
                }),
            }),
            component_communication: Some(SpdmCommunication {
                sessions: Some(vec![SpdmSession {
                    session_id: 1,
                    session_type: "Encrypted".to_string(),
                }]),
            }),
        };

        let json = serde_json::to_value(&spdm).unwrap();
        assert_eq!(
            json["Requester"]["@odata.id"],
            "/redfish/v1/ComponentIntegrity/vm1"
        );
        assert_eq!(json["MeasurementSet"]["MeasurementSpecification"], "DMTF");
        assert_eq!(
            json["MeasurementSet"]["Measurements"][0]["MeasurementIndex"],
            1
        );
        assert_eq!(
            json["MeasurementSet"]["Measurements"][0]["MeasurementType"],
            "ImmutableROM"
        );
        assert_eq!(
            json["MeasurementSet"]["Measurements"][0]["Measurement"],
            "aabbccdd"
        );
        assert_eq!(json["MeasurementSet"]["MeasurementSummary"], "summary_hash");
        assert_eq!(
            json["IdentityAuthentication"]["ResponderAuthentication"]["VerificationStatus"],
            "Success"
        );
        assert_eq!(
            json["IdentityAuthentication"]["RequesterAuthentication"]["VerificationStatus"],
            "Success"
        );
        assert_eq!(
            json["ComponentCommunication"]["Sessions"][0]["SessionId"],
            1
        );
    }

    #[test]
    fn test_spdm_info_serialization_minimal_fields() {
        let spdm = SpdmInfo {
            requester: ODataId::new("/redfish/v1/ComponentIntegrity/vm1".to_string()),
            measurement_set: None,
            identity_authentication: None,
            component_communication: None,
        };

        let json = serde_json::to_value(&spdm).unwrap();
        assert_eq!(
            json["Requester"]["@odata.id"],
            "/redfish/v1/ComponentIntegrity/vm1"
        );
        // Optional fields should be absent
        assert!(!json.as_object().unwrap().contains_key("MeasurementSet"));
        assert!(
            !json
                .as_object()
                .unwrap()
                .contains_key("IdentityAuthentication")
        );
        assert!(
            !json
                .as_object()
                .unwrap()
                .contains_key("ComponentCommunication")
        );
    }

    #[test]
    fn test_spdm_measurement_set_empty_measurements() {
        let measurement_set = SpdmMeasurementSet {
            measurement_specification: Some("DMTF".to_string()),
            measurements: None,
            measurement_summary: Some("summary".to_string()),
            measurement_summary_hash_algorithm: Some("TPM_ALG_SHA_384".to_string()),
            measurement_summary_type: Some("All".to_string()),
        };

        let json = serde_json::to_value(&measurement_set).unwrap();
        assert_eq!(json["MeasurementSpecification"], "DMTF");
        assert!(!json.as_object().unwrap().contains_key("Measurements"));
        assert_eq!(json["MeasurementSummary"], "summary");
    }

    #[test]
    fn test_build_spdm_from_evidence_with_measurements() {
        let evidence = AttestationEvidence {
            measurements: vec![
                MeasurementEntry {
                    index: 0,
                    measurement_type: "ImmutableROM".to_string(),
                    measurement: "aabbccdd".to_string(),
                    hash_algorithm: "TPM_ALG_SHA_384".to_string(),
                    part_of_summary: true,
                    last_updated: Some("2024-01-01T00:00:00Z".to_string()),
                },
                MeasurementEntry {
                    index: 1,
                    measurement_type: "MutableFirmware".to_string(),
                    measurement: "eeff0011".to_string(),
                    hash_algorithm: "TPM_ALG_SHA_384".to_string(),
                    part_of_summary: true,
                    last_updated: Some("2024-01-01T00:00:00Z".to_string()),
                },
            ],
            measurement_summary: Some("summary_hash".to_string()),
            measurement_summary_algorithm: Some("TPM_ALG_SHA_384".to_string()),
            measurement_summary_type: Some("All".to_string()),
            responder_verification: Some(VerificationStatus::Success),
            provider: Some("test".to_string()),
        };

        let spdm = build_spdm_from_evidence("vm1", &evidence);

        assert_eq!(
            spdm.requester.odata_id,
            "/redfish/v1/ComponentIntegrity/vm1"
        );
        assert!(spdm.measurement_set.is_some());

        let ms = spdm.measurement_set.unwrap();
        assert_eq!(ms.measurement_specification, Some("DMTF".to_string()));
        assert_eq!(ms.measurements.as_ref().unwrap().len(), 2);
        assert_eq!(ms.measurements.as_ref().unwrap()[0].measurement_index, 0);
        assert_eq!(ms.measurements.as_ref().unwrap()[1].measurement_index, 1);
        assert_eq!(ms.measurement_summary, Some("summary_hash".to_string()));

        assert!(spdm.identity_authentication.is_some());
        let ia = spdm.identity_authentication.unwrap();
        assert_eq!(ia.responder_authentication.verification_status, "Success");
    }

    #[test]
    fn test_build_spdm_from_evidence_empty_measurements() {
        let evidence = AttestationEvidence {
            measurements: vec![],
            measurement_summary: Some("summary_hash".to_string()),
            measurement_summary_algorithm: Some("TPM_ALG_SHA_384".to_string()),
            measurement_summary_type: Some("All".to_string()),
            responder_verification: None,
            provider: None,
        };

        let spdm = build_spdm_from_evidence("vm1", &evidence);

        assert!(spdm.measurement_set.is_some());
        let ms = spdm.measurement_set.unwrap();
        assert!(ms.measurements.is_none());
        assert_eq!(ms.measurement_summary, Some("summary_hash".to_string()));
        assert!(spdm.identity_authentication.is_none());
    }

    #[test]
    fn test_build_spdm_from_evidence_no_summary_no_measurements() {
        let evidence = AttestationEvidence {
            measurements: vec![],
            measurement_summary: None,
            measurement_summary_algorithm: None,
            measurement_summary_type: None,
            responder_verification: Some(VerificationStatus::Failed),
            provider: None,
        };

        let spdm = build_spdm_from_evidence("vm1", &evidence);

        assert!(spdm.measurement_set.is_none());
        assert!(spdm.identity_authentication.is_some());
        let ia = spdm.identity_authentication.unwrap();
        assert_eq!(ia.responder_authentication.verification_status, "Failed");
    }

    #[test]
    fn test_spdm_single_measurement_optional_fields() {
        let measurement = SpdmSingleMeasurement {
            measurement_index: 5,
            measurement_type: None,
            measurement: None,
            measurement_hash_algorithm: None,
            part_of_summary_hash: None,
            last_updated: None,
        };

        let json = serde_json::to_value(&measurement).unwrap();
        assert_eq!(json["MeasurementIndex"], 5);
        assert!(!json.as_object().unwrap().contains_key("MeasurementType"));
        assert!(!json.as_object().unwrap().contains_key("Measurement"));
        assert!(
            !json
                .as_object()
                .unwrap()
                .contains_key("MeasurementHashAlgorithm")
        );
        assert!(!json.as_object().unwrap().contains_key("PartofSummaryHash"));
        assert!(!json.as_object().unwrap().contains_key("LastUpdated"));
    }

    #[test]
    fn test_component_integrity_links_serialization() {
        let links = ComponentIntegrityLinks {
            components_protected: vec![
                ODataId::new("/redfish/v1/Systems/vm1".to_string()),
                ODataId::new("/redfish/v1/Systems/vm2".to_string()),
            ],
        };

        let json = serde_json::to_value(&links).unwrap();
        assert!(json["ComponentsProtected"].is_array());
        assert_eq!(
            json["ComponentsProtected"][0]["@odata.id"],
            "/redfish/v1/Systems/vm1"
        );
        assert_eq!(
            json["ComponentsProtected"][1]["@odata.id"],
            "/redfish/v1/Systems/vm2"
        );
    }
}
