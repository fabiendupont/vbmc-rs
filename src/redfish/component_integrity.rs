use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use serde::Serialize;

use super::error::RedfishApiError;
use super::types::{Collection, ODataId, Status};
use crate::app_state::AppState;
use crate::attestation::trust_chain::AttestationEvidence;

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
            measurement_set: None,
            identity_authentication: None,
            component_communication: None,
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
        target_component_uri: format!("/redfish/v1/Chassis/1/TrustedComponents/{system_id}"),
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
