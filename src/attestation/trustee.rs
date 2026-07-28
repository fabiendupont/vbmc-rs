use base64::Engine;
use chrono::Utc;

use super::trust_chain::{AttestationEvidence, MeasurementEntry, VerificationStatus};

pub struct TrusteeClient {
    base_url: String,
}

impl TrusteeClient {
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.to_string(),
        }
    }

    pub async fn attest_with_evidence(
        &self,
        _evidence: &[u8],
    ) -> anyhow::Result<AttestationEvidence> {
        let url = format!("{}/kbs/v0/attest", self.base_url);
        let resp = reqwest::Client::new()
            .post(&url)
            .header("Content-Type", "application/json")
            .body("{}")
            .send()
            .await?;

        let verification = if resp.status().is_success() {
            VerificationStatus::Success
        } else {
            return Ok(AttestationEvidence {
                responder_verification: Some(VerificationStatus::Failed),
                provider: Some("trustee".to_string()),
                ..Default::default()
            });
        };

        let body = resp.text().await.unwrap_or_default();
        let measurements = parse_jwt_claims(&body);

        Ok(AttestationEvidence {
            measurements,
            measurement_summary: None,
            measurement_summary_algorithm: None,
            measurement_summary_type: None,
            responder_verification: Some(verification),
            provider: Some("trustee".to_string()),
        })
    }
}

fn parse_jwt_claims(token: &str) -> Vec<MeasurementEntry> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return Vec::new();
    }

    let payload = match base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(parts[1]) {
        Ok(bytes) => bytes,
        Err(_) => return Vec::new(),
    };

    let claims: serde_json::Value = match serde_json::from_slice(&payload) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    let now = Utc::now().to_rfc3339();
    let mut measurements = Vec::new();
    let mut index = 0u32;

    // Extract tcb_status / launch measurement as ImmutableROM
    if let Some(tcb) = claims.get("tcb_status").and_then(|v| v.as_str()) {
        measurements.push(MeasurementEntry {
            index,
            measurement_type: "ImmutableROM".to_string(),
            measurement: base64::engine::general_purpose::STANDARD.encode(tcb.as_bytes()),
            hash_algorithm: "SHA-256".to_string(),
            part_of_summary: false,
            last_updated: Some(now.clone()),
        });
        index += 1;
    }

    if let Some(launch) = claims.get("launch_measurement").and_then(|v| v.as_str()) {
        measurements.push(MeasurementEntry {
            index,
            measurement_type: "ImmutableROM".to_string(),
            measurement: base64::engine::general_purpose::STANDARD.encode(launch.as_bytes()),
            hash_algorithm: "SHA-256".to_string(),
            part_of_summary: false,
            last_updated: Some(now.clone()),
        });
        index += 1;
    }

    // Extract configuration claims
    for key in &[
        "configuration",
        "platform_config",
        "guest_config",
        "fw_config",
    ] {
        if let Some(val) = claims.get(*key) {
            let val_str = match val.as_str() {
                Some(s) => s.to_string(),
                None => val.to_string(),
            };
            let mtype = if *key == "fw_config" {
                "FirmwareConfiguration"
            } else {
                "HardwareConfiguration"
            };
            measurements.push(MeasurementEntry {
                index,
                measurement_type: mtype.to_string(),
                measurement: base64::engine::general_purpose::STANDARD.encode(val_str.as_bytes()),
                hash_algorithm: "SHA-256".to_string(),
                part_of_summary: false,
                last_updated: Some(now.clone()),
            });
            index += 1;
        }
    }

    measurements
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_jwt(claims: &serde_json::Value) -> String {
        let header = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(b"{\"alg\":\"RS256\",\"typ\":\"JWT\"}");
        let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(serde_json::to_vec(claims).unwrap());
        let signature = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b"fake-signature");
        format!("{header}.{payload}.{signature}")
    }

    #[test]
    fn test_parse_jwt_claims_with_tcb_and_launch() {
        let claims = serde_json::json!({
            "tcb_status": "UpToDate",
            "launch_measurement": "abcdef0123456789"
        });
        let token = make_test_jwt(&claims);
        let measurements = parse_jwt_claims(&token);

        assert_eq!(measurements.len(), 2);
        assert_eq!(measurements[0].measurement_type, "ImmutableROM");
        assert_eq!(measurements[0].index, 0);
        assert_eq!(measurements[1].measurement_type, "ImmutableROM");
        assert_eq!(measurements[1].index, 1);
    }

    #[test]
    fn test_parse_jwt_claims_with_config() {
        let claims = serde_json::json!({
            "tcb_status": "UpToDate",
            "configuration": "sev-snp",
            "fw_config": "edk2-stable"
        });
        let token = make_test_jwt(&claims);
        let measurements = parse_jwt_claims(&token);

        assert_eq!(measurements.len(), 3);
        assert_eq!(measurements[0].measurement_type, "ImmutableROM");
        assert_eq!(measurements[1].measurement_type, "HardwareConfiguration");
        assert_eq!(measurements[2].measurement_type, "FirmwareConfiguration");
    }

    #[test]
    fn test_parse_jwt_claims_invalid_token() {
        assert!(parse_jwt_claims("not-a-jwt").is_empty());
        assert!(parse_jwt_claims("a.b").is_empty());
        assert!(parse_jwt_claims("a.!!!invalid-base64.c").is_empty());
    }

    #[test]
    fn test_parse_jwt_claims_empty_claims() {
        let token = make_test_jwt(&serde_json::json!({}));
        let measurements = parse_jwt_claims(&token);
        assert!(measurements.is_empty());
    }
}
