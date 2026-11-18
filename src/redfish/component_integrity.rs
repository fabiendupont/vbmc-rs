use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use serde::Serialize;

use super::error::RedfishApiError;
use super::types::{Collection, ODataId, Status};
use crate::app_state::AppState;
use crate::attestation::spdm_responder::{
    HASHING_ALGORITHM, SIGNING_ALGORITHM, SPDM_VERSION_STRING, SpdmGetSignedMeasurementsRequest,
};
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
    #[serde(rename = "Actions", skip_serializing_if = "Option::is_none")]
    pub actions: Option<ComponentIntegrityActions>,
}

#[derive(Debug, Serialize)]
pub struct ComponentIntegrityActions {
    #[serde(rename = "#ComponentIntegrity.SPDMGetSignedMeasurements")]
    pub spdm_get_signed_measurements: ActionTarget,
}

#[derive(Debug, Serialize)]
pub struct ActionTarget {
    pub target: String,
}

/// Response payload of the SPDMGetSignedMeasurements action (ComponentIntegrity
/// schema). `SignedMeasurements` is the Base64 of the signed DSP0274 statement
/// (message plus trailing raw ECDSA signature). `Certificate` is the
/// schema-conformant link to the Redfish Certificate resource holding the
/// device's SPDM certificate chain. `CertificateChain` is a non-standard
/// extension carrying the Base64 DMTF chain blob inline, kept for clients that
/// want everything in one round-trip.
#[derive(Debug, Serialize)]
pub struct SpdmGetSignedMeasurementsResponse {
    #[serde(rename = "@odata.type")]
    pub odata_type: &'static str,
    #[serde(rename = "Version")]
    pub version: &'static str,
    #[serde(rename = "HashingAlgorithm")]
    pub hashing_algorithm: &'static str,
    #[serde(rename = "SigningAlgorithm")]
    pub signing_algorithm: &'static str,
    #[serde(rename = "SignedMeasurements")]
    pub signed_measurements: String,
    #[serde(rename = "PublicKey")]
    pub public_key: String,
    #[serde(rename = "Certificate")]
    pub certificate: ODataId,
    #[serde(rename = "CertificateChain")]
    pub certificate_chain: String,
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

fn build_spdm_from_evidence(
    chassis_id: &str,
    system_id: &str,
    evidence: &AttestationEvidence,
) -> SpdmInfo {
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
                component_certificate: Some(ODataId::new(format!(
                    "/redfish/v1/Chassis/{chassis_id}/Certificates/{system_id}"
                ))),
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
        Some(evidence) => build_spdm_from_evidence(&state.chassis_id, &system_id, evidence),
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
                    component_certificate: Some(ODataId::new(format!(
                        "/redfish/v1/Chassis/{}/Certificates/{system_id}",
                        state.chassis_id
                    ))),
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
        actions: Some(ComponentIntegrityActions {
            spdm_get_signed_measurements: ActionTarget {
                target: format!(
                    "/redfish/v1/ComponentIntegrity/{system_id}/Actions/ComponentIntegrity.SPDMGetSignedMeasurements"
                ),
            },
        }),
    }))
}

/// POST /redfish/v1/ComponentIntegrity/{system_id}/Actions/ComponentIntegrity.SPDMGetSignedMeasurements
///
/// Signs fresh SPDM measurements over the caller-supplied nonce using the
/// system's configured SPDM responder identity. The nonce must be provided by
/// the caller (a 32-byte hex string) so that a replayed response cannot match
/// a later attestation challenge.
/// Build the measurement response payload for a completed SPDM measurement task.
#[allow(clippy::too_many_arguments)]
fn spdm_measurement_response(
    chassis_id: &str,
    system_id: &str,
    responder: &crate::attestation::spdm_responder::SpdmResponder,
    signed: Vec<u8>,
) -> serde_json::Value {
    serde_json::json!({
        "@odata.type": "#ComponentIntegrity.v1_2_0.SPDMGetSignedMeasurementsResponse",
        "Version": SPDM_VERSION_STRING,
        "HashingAlgorithm": HASHING_ALGORITHM,
        "SigningAlgorithm": SIGNING_ALGORITHM,
        "SignedMeasurements": BASE64.encode(&signed),
        "PublicKey": responder.public_key_pem(),
        "Certificate": { "@odata.id": format!("/redfish/v1/Chassis/{chassis_id}/Certificates/{system_id}") },
        "CertificateChain": BASE64.encode(responder.cert_chain()),
    })
}

/// Validate a challenge nonce without doing any signing work: SPDM nonces are
/// 32-byte values given as 64 hex characters.
fn validate_nonce(nonce: &str) -> Result<(), String> {
    if nonce.len() != 64 {
        return Err(format!(
            "nonce must be a 64-character hex string, got {} characters",
            nonce.len()
        ));
    }
    if !nonce.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("nonce is not valid hex".to_string());
    }
    Ok(())
}

/// POST /redfish/v1/ComponentIntegrity/{system_id}/Actions/ComponentIntegrity.SPDMGetSignedMeasurements
///
/// Signs fresh SPDM measurements over the caller-supplied nonce using the
/// system's configured SPDM responder identity. The nonce must be provided by
/// the caller (a 32-byte hex string) so that a replayed response cannot match
/// a later attestation challenge.
///
/// Mirroring SPDM-capable BMCs (e.g. NVIDIA GB300 NVL72 trays), the action is
/// asynchronous: gathering measurements means traversing MCTP to the device's
/// root of trust, which is genuinely slow. The handler therefore returns HTTP
/// 202 with a Task resource; clients poll the task until `TaskState` is
/// `Completed`, then retrieve the measurements from `GET {action target}/data`.
/// The simulated collection latency is configurable per system
/// (`[systems.<id>.spdm] collection_delay_ms`).
pub async fn spdm_get_signed_measurements(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
    Path(system_id): Path<String>,
    Json(request): Json<SpdmGetSignedMeasurementsRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), RedfishApiError> {
    let responder = state.spdm_responders.get(&system_id).ok_or_else(|| {
        RedfishApiError::ServiceUnavailable(format!(
            "ComponentIntegrity '{system_id}' has no SPDM responder identity configured"
        ))
    })?;

    let nonce = request
        .nonce
        .ok_or_else(|| RedfishApiError::BadRequest("Nonce is required".to_string()))?;

    validate_nonce(&nonce).map_err(RedfishApiError::BadRequest)?;

    let indices = request.measurement_indices.unwrap_or_default();

    let task_id = state
        .task_manager
        .create_task("SPDM measurement collection");
    let task_id_response = task_id.clone();

    let task_state = state.clone();
    let responder_clone = responder.clone();
    let system_id_clone = system_id.clone();
    let chassis_id = state.chassis_id.clone();
    let delay = responder.collection_delay_ms;
    tokio::spawn(async move {
        // Model the MCTP/SPDM round-trip to the device's root of trust
        tokio::time::sleep(std::time::Duration::from_millis(delay)).await;

        match responder_clone.signed_measurements(&nonce, &indices) {
            Ok(signed) => {
                let response = spdm_measurement_response(
                    &chassis_id,
                    &system_id_clone,
                    &responder_clone,
                    signed,
                );
                task_state
                    .spdm_measurement_results
                    .insert(system_id_clone.clone(), response.clone());
                task_state
                    .task_manager
                    .complete_task(&task_id, Some(response));
            }
            Err(e) => {
                task_state.task_manager.fail_task(
                    &task_id,
                    &format!("SPDM measurement collection failed: {e}"),
                );
            }
        }
    });

    Ok((
        StatusCode::ACCEPTED,
        Json(serde_json::json!({
            "@odata.id": format!("/redfish/v1/TaskService/Tasks/{task_id_response}"),
            "@odata.type": "#Task.v1_7_0.Task",
            "Id": task_id_response,
            "Name": "SPDM measurement collection",
            "TaskState": "Running",
            "TaskStatus": "OK",
            "TaskMonitor": format!("/redfish/v1/TaskService/TaskMonitors/{task_id_response}"),
        })),
    ))
}

/// GET /redfish/v1/ComponentIntegrity/{system_id}/Actions/ComponentIntegrity.SPDMGetSignedMeasurements/data
///
/// Retrieves the measurement payload of the most recently completed SPDM measurement task
/// for the component, following the convention used by SPDM-capable BMCs.
pub async fn spdm_get_signed_measurements_data(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
    Path(system_id): Path<String>,
) -> Result<Json<serde_json::Value>, RedfishApiError> {
    let result = state
        .spdm_measurement_results
        .get(&system_id)
        .ok_or_else(|| {
            RedfishApiError::NotFound(format!(
                "No SPDM measurements available for '{system_id}' yet"
            ))
        })?;

    Ok(Json(result.clone()))
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
            actions: None,
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

        let spdm = build_spdm_from_evidence("1", "vm1", &evidence);

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

        let spdm = build_spdm_from_evidence("1", "vm1", &evidence);

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

        let spdm = build_spdm_from_evidence("1", "vm1", &evidence);

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

    // ---- HTTP-level tests for the SPDMGetSignedMeasurements action ----

    use crate::redfish::test_harness::{request_json, router_with_systems};
    use axum::http::{Method, StatusCode};
    use p384::ecdsa::SigningKey;
    use p384::elliptic_curve::pkcs8::EncodePrivateKey;
    use p384::elliptic_curve::rand_core::OsRng;

    fn spdm_systems() -> std::collections::HashMap<String, crate::config::SystemConfig> {
        let dir = tempfile::tempdir().unwrap();

        let signing_key = SigningKey::random(&mut OsRng);
        let key_pem = signing_key
            .to_pkcs8_pem(p384::elliptic_curve::pkcs8::LineEnding::LF)
            .unwrap();
        let key_path = dir.path().join("key.pem");
        std::fs::write(&key_path, key_pem.as_bytes()).unwrap();

        let chain = b"\x06\x00\x04\x00\xde\xad\xbe\xef".to_vec();
        let chain_path = dir.path().join("chain.blob");
        std::fs::write(&chain_path, &chain).unwrap();

        // Keep the tempdir alive for the duration of the test by converting it
        // into a leaked path (test-only; the files are tiny and the OS reclaims
        // them at exit).
        let dir_path = dir.keep();

        let mut spdm = std::collections::HashMap::new();
        spdm.insert(
            "vm1".to_string(),
            crate::config::SystemConfig {
                name: Some("vm1".to_string()),
                spdm: Some(crate::config::SpdmConfig {
                    private_key: key_path,
                    cert_chain: chain_path,
                    measurements: vec!["aa".repeat(48), "bb".repeat(48)],
                    collection_delay_ms: 0,
                    sign_transcript: false,
                }),
                ..crate::redfish::test_harness::system_config("vm1")
            },
        );
        let _ = dir_path;
        spdm
    }

    #[tokio::test]
    async fn test_spdm_get_signed_measurements_action() {
        let app = router_with_systems(spdm_systems());
        let nonce_hex = "11".repeat(32);

        // 1. POST the action: must return 202 with a Task (async collection, like GB300 BMCs)
        let body = serde_json::json!({
            "Nonce": nonce_hex,
            "SlotId": 0,
            "MeasurementIndices": [255]
        });
        let (status, task, _) = request_json(
            &app,
            Method::POST,
            "/redfish/v1/ComponentIntegrity/vm1/Actions/ComponentIntegrity.SPDMGetSignedMeasurements",
            body,
        )
        .await;

        assert_eq!(status, StatusCode::ACCEPTED);
        assert_eq!(task["@odata.type"], "#Task.v1_7_0.Task");
        let task_uri = task["@odata.id"].as_str().unwrap().to_string();
        assert!(task_uri.starts_with("/redfish/v1/TaskService/Tasks/"));

        // 2. Poll the task until it completes
        let mut task_json = task.clone();
        for _ in 0..50 {
            let (status, polled, _) =
                request_json(&app, Method::GET, &task_uri, serde_json::json!({})).await;
            assert_eq!(status, StatusCode::OK);
            if polled["TaskState"] == "Completed" {
                task_json = polled;
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        assert_eq!(task_json["TaskState"], "Completed");

        // 3. Fetch the measurement data from the action's /data endpoint
        let (status, json, _) = request_json(
            &app,
            Method::GET,
            "/redfish/v1/ComponentIntegrity/vm1/Actions/ComponentIntegrity.SPDMGetSignedMeasurements/data",
            serde_json::json!({}),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["Version"], "1.2");
        assert_eq!(json["HashingAlgorithm"], "SHA384");
        assert_eq!(json["SigningAlgorithm"], "TPM_ALG_ECDSA_ECC_NIST_P384");

        // Spec-conformant certificate link to the Chassis-scoped Certificate resource
        assert_eq!(
            json["Certificate"]["@odata.id"],
            "/redfish/v1/Chassis/1/Certificates/vm1"
        );

        let signed = BASE64
            .decode(json["SignedMeasurements"].as_str().unwrap())
            .unwrap();
        // message layout: header(9) + record + nonce(32) + opaque len(2) + signature(96)
        assert_eq!(signed[0], 0x12);
        assert_eq!(signed[1], 0x60);
        assert_eq!(signed[4], 2); // two measurement blocks
        let record_len = u32::from_le_bytes([signed[5], signed[6], signed[7], signed[8]]) as usize;
        let nonce_offset = 9 + record_len;
        assert_eq!(
            hex::encode(&signed[nonce_offset..nonce_offset + 32]),
            nonce_hex.to_lowercase()
        );

        let chain = BASE64
            .decode(json["CertificateChain"].as_str().unwrap())
            .unwrap();
        assert_eq!(chain, b"\x06\x00\x04\x00\xde\xad\xbe\xef".to_vec());
        assert!(
            json["PublicKey"]
                .as_str()
                .unwrap()
                .starts_with("-----BEGIN PUBLIC KEY-----")
        );
    }

    #[tokio::test]
    async fn test_chassis_certificate_resource_serves_spdm_chain() {
        use crate::redfish::test_harness::router_with_systems;

        // The Chassis Certificates collection requires a configured SPDM identity
        let app = router_with_systems(spdm_systems());

        let (status, collection, _) = request_json(
            &app,
            Method::GET,
            "/redfish/v1/Chassis/1/Certificates",
            serde_json::json!({}),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            collection["Members"][0]["@odata.id"],
            "/redfish/v1/Chassis/1/Certificates/vm1"
        );

        let (status, cert, _) = request_json(
            &app,
            Method::GET,
            "/redfish/v1/Chassis/1/Certificates/vm1",
            serde_json::json!({}),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(cert["@odata.type"], "#Certificate.v1_9_0.Certificate");
        assert_eq!(cert["CertificateType"], "PEMchain");
        // The fake chain blob (  Þ­¾ï) re-armored as PEM:
        // a single 4-byte DER certificate
        let pem = cert["CertificateString"].as_str().unwrap();
        assert!(pem.starts_with("-----BEGIN CERTIFICATE-----"));
        assert!(pem.ends_with("-----END CERTIFICATE-----\n"));
        let b64_body: String = pem.lines().filter(|l| !l.starts_with("-----")).collect();
        assert_eq!(
            base64::engine::general_purpose::STANDARD
                .decode(b64_body)
                .unwrap(),
            vec![0xde, 0xad, 0xbe, 0xef]
        );
    }

    #[tokio::test]
    async fn test_component_integrity_advertises_component_certificate() {
        use crate::redfish::test_harness::router_with_systems;

        let app = router_with_systems(spdm_systems());
        let (status, json, _) = request_json(
            &app,
            Method::GET,
            "/redfish/v1/ComponentIntegrity/vm1",
            serde_json::json!({}),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            json["SPDM"]["IdentityAuthentication"]["ResponderAuthentication"]["ComponentCertificate"]
                ["@odata.id"],
            "/redfish/v1/Chassis/1/Certificates/vm1"
        );
    }

    #[tokio::test]
    async fn test_spdm_get_signed_measurements_requires_nonce() {
        let app = router_with_systems(spdm_systems());

        let (status, json, _) = request_json(
            &app,
            Method::POST,
            "/redfish/v1/ComponentIntegrity/vm1/Actions/ComponentIntegrity.SPDMGetSignedMeasurements",
            serde_json::json!({}),
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(json["error"]["message"].as_str().unwrap().contains("Nonce"));
    }

    #[tokio::test]
    async fn test_spdm_get_signed_measurements_without_config() {
        use crate::redfish::test_harness::systems_with;
        let app = router_with_systems(systems_with("vm1"));

        let (status, _, _) = request_json(
            &app,
            Method::POST,
            "/redfish/v1/ComponentIntegrity/vm1/Actions/ComponentIntegrity.SPDMGetSignedMeasurements",
            serde_json::json!({"Nonce": "11".repeat(32)}),
        )
        .await;

        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn test_component_integrity_advertises_action() {
        use crate::redfish::test_harness::{get, router_with_systems, systems_with};
        let app = router_with_systems(systems_with("vm1"));

        let (status, json, _) = get(&app, "/redfish/v1/ComponentIntegrity/vm1").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            json["Actions"]["#ComponentIntegrity.SPDMGetSignedMeasurements"]["target"],
            "/redfish/v1/ComponentIntegrity/vm1/Actions/ComponentIntegrity.SPDMGetSignedMeasurements"
        );
    }
}
