use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum VerificationStatus {
    Success,
    Failed,
    Unknown,
}

impl fmt::Display for VerificationStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Success => write!(f, "Success"),
            Self::Failed => write!(f, "Failed"),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MeasurementEntry {
    pub index: u32,
    pub measurement_type: String,
    pub measurement: String,
    pub hash_algorithm: String,
    #[serde(default)]
    pub part_of_summary: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_updated: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AttestationEvidence {
    #[serde(default)]
    pub measurements: Vec<MeasurementEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub measurement_summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub measurement_summary_algorithm: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub measurement_summary_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub responder_verification: Option<VerificationStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_measurement_entry_serde_roundtrip() {
        let entry = MeasurementEntry {
            index: 0,
            measurement_type: "ImmutableROM".to_string(),
            measurement: "AQID".to_string(),
            hash_algorithm: "SHA-256".to_string(),
            part_of_summary: true,
            last_updated: Some("2026-01-15T10:00:00Z".to_string()),
        };
        let json = serde_json::to_string(&entry).unwrap();
        let decoded: MeasurementEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.index, 0);
        assert_eq!(decoded.measurement_type, "ImmutableROM");
        assert_eq!(decoded.measurement, "AQID");
        assert!(decoded.part_of_summary);
        assert_eq!(decoded.last_updated.as_deref(), Some("2026-01-15T10:00:00Z"));
    }

    #[test]
    fn test_attestation_evidence_serde_roundtrip() {
        let evidence = AttestationEvidence {
            measurements: vec![MeasurementEntry {
                index: 7,
                measurement_type: "ImmutableROM".to_string(),
                measurement: "dGVzdA==".to_string(),
                hash_algorithm: "SHA-256".to_string(),
                part_of_summary: false,
                last_updated: None,
            }],
            measurement_summary: Some("abc123".to_string()),
            measurement_summary_algorithm: Some("SHA-256".to_string()),
            measurement_summary_type: Some("TCB".to_string()),
            responder_verification: Some(VerificationStatus::Success),
            provider: Some("keylime".to_string()),
        };
        let json = serde_json::to_string(&evidence).unwrap();
        let decoded: AttestationEvidence = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.measurements.len(), 1);
        assert_eq!(decoded.measurements[0].index, 7);
        assert_eq!(decoded.measurement_summary.as_deref(), Some("abc123"));
        assert_eq!(decoded.responder_verification, Some(VerificationStatus::Success));
        assert_eq!(decoded.provider.as_deref(), Some("keylime"));
    }

    #[test]
    fn test_attestation_evidence_default_is_empty() {
        let evidence = AttestationEvidence::default();
        assert!(evidence.measurements.is_empty());
        assert!(evidence.measurement_summary.is_none());
        assert!(evidence.responder_verification.is_none());
        assert!(evidence.provider.is_none());
    }

    #[test]
    fn test_attestation_evidence_skips_none_fields() {
        let evidence = AttestationEvidence::default();
        let json = serde_json::to_value(&evidence).unwrap();
        assert!(json.get("measurement_summary").is_none());
        assert!(json.get("measurement_summary_algorithm").is_none());
        assert!(json.get("responder_verification").is_none());
        assert!(json.get("provider").is_none());
    }
}
