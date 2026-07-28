use base64::Engine;
use chrono::Utc;

use super::trust_chain::{AttestationEvidence, MeasurementEntry, VerificationStatus};

pub struct KeylimeClient {
    base_url: String,
}

impl KeylimeClient {
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.to_string(),
        }
    }

    pub async fn get_agent_attestation(
        &self,
        agent_id: &str,
    ) -> anyhow::Result<AttestationEvidence> {
        let url = format!("{}/v2/agents/{agent_id}", self.base_url);
        let resp = reqwest::get(&url).await?;

        if !resp.status().is_success() {
            return Ok(AttestationEvidence {
                responder_verification: Some(VerificationStatus::Unknown),
                provider: Some("keylime".to_string()),
                ..Default::default()
            });
        }

        let body: serde_json::Value = resp.json().await?;
        let results = body.get("results");

        let operational_state = results
            .and_then(|r| r.get("operational_state"))
            .and_then(|s| s.as_u64())
            .unwrap_or(0);

        let verification = match operational_state {
            7 => VerificationStatus::Success,
            0 => VerificationStatus::Unknown,
            _ => VerificationStatus::Failed,
        };

        let now = Utc::now().to_rfc3339();
        let mut measurements = Vec::new();

        // Parse PCR quote data
        if let Some(quote) = results.and_then(|r| r.get("quote")) {
            parse_pcr_quote(quote, &now, &mut measurements);
        }

        // Determine which PCRs are part of summary from tpm_policy
        if let Some(policy) = results.and_then(|r| r.get("tpm_policy")) {
            apply_tpm_policy(policy, &mut measurements);
        }

        // Parse IMA measurement list
        if let Some(ima) = results
            .and_then(|r| r.get("ima_measurement_list"))
            .and_then(|v| v.as_str())
        {
            parse_ima_measurements(ima, &now, &mut measurements);
        }

        Ok(AttestationEvidence {
            measurements,
            measurement_summary: None,
            measurement_summary_algorithm: None,
            measurement_summary_type: None,
            responder_verification: Some(verification),
            provider: Some("keylime".to_string()),
        })
    }
}

fn pcr_measurement_type(pcr_index: u32) -> &'static str {
    match pcr_index {
        0..=7 => "ImmutableROM",
        8..=13 => "MutableFirmware",
        14 => "FirmwareConfiguration",
        _ => "MutableFirmware",
    }
}

fn parse_pcr_quote(
    quote: &serde_json::Value,
    timestamp: &str,
    measurements: &mut Vec<MeasurementEntry>,
) {
    // Keylime quote can be a string (base64-encoded) or an object with PCR values.
    // Try as object with PCR bank keys like "sha256" containing PCR index->value maps.
    if let Some(obj) = quote.as_object() {
        for (_bank, pcrs) in obj {
            if let Some(pcr_map) = pcrs.as_object() {
                for (index_str, value) in pcr_map {
                    let index = match index_str.parse::<u32>() {
                        Ok(i) => i,
                        Err(_) => continue,
                    };
                    let digest = match value.as_str() {
                        Some(s) => s,
                        None => continue,
                    };
                    let encoded =
                        base64::engine::general_purpose::STANDARD.encode(hex_to_bytes(digest));
                    measurements.push(MeasurementEntry {
                        index,
                        measurement_type: pcr_measurement_type(index).to_string(),
                        measurement: encoded,
                        hash_algorithm: "SHA-256".to_string(),
                        part_of_summary: false,
                        last_updated: Some(timestamp.to_string()),
                    });
                }
            }
        }
    }
}

fn apply_tpm_policy(policy: &serde_json::Value, measurements: &mut [MeasurementEntry]) {
    if let Some(obj) = policy.as_object() {
        for (index_str, _) in obj {
            if let Ok(index) = index_str.parse::<u32>() {
                for m in measurements.iter_mut() {
                    if m.index == index {
                        m.part_of_summary = true;
                    }
                }
            }
        }
    }
}

fn parse_ima_measurements(
    ima_list: &str,
    timestamp: &str,
    measurements: &mut Vec<MeasurementEntry>,
) {
    // IMA log format: "pcr_index template_hash template_name filedata_hash filename"
    // We cap at 256 entries to avoid unbounded growth.
    let base_index = measurements.len() as u32 + 100;
    let mut count = 0u32;

    for line in ima_list.lines() {
        if count >= 256 {
            break;
        }
        let parts: Vec<&str> = line.splitn(5, ' ').collect();
        if parts.len() < 4 {
            continue;
        }
        let digest = parts[1];
        let encoded = base64::engine::general_purpose::STANDARD.encode(hex_to_bytes(digest));
        measurements.push(MeasurementEntry {
            index: base_index + count,
            measurement_type: "MutableFirmware".to_string(),
            measurement: encoded,
            hash_algorithm: "SHA-256".to_string(),
            part_of_summary: false,
            last_updated: Some(timestamp.to_string()),
        });
        count += 1;
    }
}

fn hex_to_bytes(hex: &str) -> Vec<u8> {
    // Best-effort hex decode; return raw bytes on success, empty on failure
    let hex = hex.trim();
    if !hex.len().is_multiple_of(2) {
        return hex.as_bytes().to_vec();
    }
    let mut bytes = Vec::with_capacity(hex.len() / 2);
    for i in (0..hex.len()).step_by(2) {
        match u8::from_str_radix(&hex[i..i + 2], 16) {
            Ok(b) => bytes.push(b),
            Err(_) => return hex.as_bytes().to_vec(),
        }
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pcr_measurement_type_mapping() {
        assert_eq!(pcr_measurement_type(0), "ImmutableROM");
        assert_eq!(pcr_measurement_type(7), "ImmutableROM");
        assert_eq!(pcr_measurement_type(8), "MutableFirmware");
        assert_eq!(pcr_measurement_type(13), "MutableFirmware");
        assert_eq!(pcr_measurement_type(14), "FirmwareConfiguration");
        assert_eq!(pcr_measurement_type(15), "MutableFirmware");
    }

    #[test]
    fn test_parse_pcr_quote() {
        let quote = serde_json::json!({
            "sha256": {
                "0": "abc123",
                "8": "def456",
                "14": "789fed"
            }
        });
        let mut measurements = Vec::new();
        parse_pcr_quote(&quote, "2026-01-01T00:00:00Z", &mut measurements);

        assert_eq!(measurements.len(), 3);
        let m0 = measurements.iter().find(|m| m.index == 0).unwrap();
        assert_eq!(m0.measurement_type, "ImmutableROM");
        assert_eq!(m0.hash_algorithm, "SHA-256");

        let m8 = measurements.iter().find(|m| m.index == 8).unwrap();
        assert_eq!(m8.measurement_type, "MutableFirmware");

        let m14 = measurements.iter().find(|m| m.index == 14).unwrap();
        assert_eq!(m14.measurement_type, "FirmwareConfiguration");
    }

    #[test]
    fn test_apply_tpm_policy() {
        let policy = serde_json::json!({ "0": ["abc123"], "8": ["def456"] });
        let mut measurements = vec![
            MeasurementEntry {
                index: 0,
                measurement_type: "ImmutableROM".to_string(),
                measurement: "dGVzdA==".to_string(),
                hash_algorithm: "SHA-256".to_string(),
                part_of_summary: false,
                last_updated: None,
            },
            MeasurementEntry {
                index: 8,
                measurement_type: "MutableFirmware".to_string(),
                measurement: "dGVzdA==".to_string(),
                hash_algorithm: "SHA-256".to_string(),
                part_of_summary: false,
                last_updated: None,
            },
            MeasurementEntry {
                index: 14,
                measurement_type: "FirmwareConfiguration".to_string(),
                measurement: "dGVzdA==".to_string(),
                hash_algorithm: "SHA-256".to_string(),
                part_of_summary: false,
                last_updated: None,
            },
        ];
        apply_tpm_policy(&policy, &mut measurements);

        assert!(measurements[0].part_of_summary);
        assert!(measurements[1].part_of_summary);
        assert!(!measurements[2].part_of_summary);
    }

    #[test]
    fn test_parse_ima_measurements() {
        let ima = "10 abc123def456abc123def456abc123def456abc1 ima-ng sha256:abcdef0123456789 /usr/bin/test\n\
                   10 123456789abcdef0123456789abcdef012345678a ima-ng sha256:fedcba9876543210 /usr/lib/test.so";
        let mut measurements = Vec::new();
        parse_ima_measurements(ima, "2026-01-01T00:00:00Z", &mut measurements);

        assert_eq!(measurements.len(), 2);
        assert_eq!(measurements[0].measurement_type, "MutableFirmware");
        assert_eq!(measurements[0].index, 100);
        assert_eq!(measurements[1].index, 101);
    }

    #[test]
    fn test_parse_ima_measurements_capped_at_256() {
        let mut ima = String::new();
        for i in 0..300 {
            ima.push_str(&format!(
                "10 abcdef0123456789abcdef0123456789abcdef01 ima-ng sha256:aabb /file{i}\n"
            ));
        }
        let mut measurements = Vec::new();
        parse_ima_measurements(&ima, "2026-01-01T00:00:00Z", &mut measurements);

        assert_eq!(measurements.len(), 256);
    }

    #[test]
    fn test_hex_to_bytes() {
        assert_eq!(hex_to_bytes("abcd"), vec![0xab, 0xcd]);
        assert_eq!(hex_to_bytes("0011ff"), vec![0x00, 0x11, 0xff]);
        // Odd length falls back to raw bytes
        assert_eq!(hex_to_bytes("abc"), b"abc".to_vec());
    }
}
