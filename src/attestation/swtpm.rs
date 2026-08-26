use std::path::Path;

use base64::Engine;
use chrono::Utc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

use super::trust_chain::{AttestationEvidence, MeasurementEntry, VerificationStatus};

const TPM2_ST_NO_SESSIONS: u16 = 0x8001;
const TPM2_CC_PCR_READ: u32 = 0x0000017E;
const TPM2_ALG_SHA256: u16 = 0x000B;

pub struct SwtpmClient {
    socket_path: String,
}

impl SwtpmClient {
    pub fn new(socket_path: &str) -> Self {
        Self {
            socket_path: socket_path.to_string(),
        }
    }

    pub async fn read_pcrs(&self, pcr_indices: &[u32]) -> anyhow::Result<AttestationEvidence> {
        let now = Utc::now().to_rfc3339();
        let mut measurements = Vec::new();

        for &pcr_index in pcr_indices {
            match self.read_single_pcr(pcr_index).await {
                Ok(digest) => {
                    let encoded = base64::engine::general_purpose::STANDARD.encode(&digest);
                    measurements.push(MeasurementEntry {
                        index: pcr_index,
                        measurement_type: pcr_measurement_type(pcr_index).to_string(),
                        measurement: encoded,
                        hash_algorithm: "SHA-256".to_string(),
                        part_of_summary: pcr_index <= 7,
                        last_updated: Some(now.clone()),
                    });
                }
                Err(e) => {
                    tracing::warn!("Failed to read PCR {pcr_index}: {e}");
                }
            }
        }

        let verification = if measurements.is_empty() {
            VerificationStatus::Unknown
        } else {
            VerificationStatus::Success
        };

        Ok(AttestationEvidence {
            measurements,
            measurement_summary: None,
            measurement_summary_algorithm: None,
            measurement_summary_type: None,
            responder_verification: Some(verification),
            provider: Some("swtpm".to_string()),
        })
    }

    async fn read_single_pcr(&self, pcr_index: u32) -> anyhow::Result<Vec<u8>> {
        let cmd = build_pcr_read_command(pcr_index);

        let mut stream = UnixStream::connect(Path::new(&self.socket_path)).await?;
        stream.write_all(&cmd).await?;

        let mut header = [0u8; 10];
        stream.read_exact(&mut header).await?;

        let response_size = u32::from_be_bytes([header[2], header[3], header[4], header[5]]);
        let response_code = u32::from_be_bytes([header[6], header[7], header[8], header[9]]);

        if response_code != 0 {
            anyhow::bail!("TPM2_PCR_Read returned error 0x{response_code:08X}");
        }

        let remaining = response_size as usize - 10;
        let mut body = vec![0u8; remaining];
        stream.read_exact(&mut body).await?;

        parse_pcr_read_response(&body)
    }
}

fn build_pcr_read_command(pcr_index: u32) -> Vec<u8> {
    // TPML_PCR_SELECTION: count=1, then TPMS_PCR_SELECTION
    let mut pcr_select = [0u8; 3];
    let byte_index = (pcr_index / 8) as usize;
    let bit_index = pcr_index % 8;
    if byte_index < 3 {
        pcr_select[byte_index] = 1 << bit_index;
    }

    let mut payload = Vec::new();
    // TPML_PCR_SELECTION.count = 1
    payload.extend_from_slice(&1u32.to_be_bytes());
    // TPMS_PCR_SELECTION.hash = SHA-256
    payload.extend_from_slice(&TPM2_ALG_SHA256.to_be_bytes());
    // TPMS_PCR_SELECTION.sizeofSelect = 3
    payload.push(3);
    // TPMS_PCR_SELECTION.pcrSelect
    payload.extend_from_slice(&pcr_select);

    let command_size = 10 + payload.len() as u32;
    let mut cmd = Vec::with_capacity(command_size as usize);
    cmd.extend_from_slice(&TPM2_ST_NO_SESSIONS.to_be_bytes());
    cmd.extend_from_slice(&command_size.to_be_bytes());
    cmd.extend_from_slice(&TPM2_CC_PCR_READ.to_be_bytes());
    cmd.extend_from_slice(&payload);

    cmd
}

fn parse_pcr_read_response(body: &[u8]) -> anyhow::Result<Vec<u8>> {
    // Body layout after 10-byte header:
    //   4 bytes: pcrUpdateCounter
    //   4 bytes: TPML_PCR_SELECTION.count
    //   variable: TPMS_PCR_SELECTION entries (hash(2) + sizeofSelect(1) + pcrSelect(sizeofSelect))
    //   4 bytes: TPML_DIGEST.count
    //   For each digest: 2 bytes size + digest bytes
    if body.len() < 8 {
        anyhow::bail!("PCR read response too short");
    }

    let selection_count = u32::from_be_bytes([body[4], body[5], body[6], body[7]]) as usize;

    let mut offset = 8;
    for _ in 0..selection_count {
        if offset + 3 > body.len() {
            anyhow::bail!("PCR selection truncated");
        }
        // skip hash alg (2 bytes)
        offset += 2;
        let select_size = body[offset] as usize;
        offset += 1 + select_size;
    }

    if offset + 4 > body.len() {
        anyhow::bail!("Digest list truncated");
    }
    let digest_count = u32::from_be_bytes([
        body[offset],
        body[offset + 1],
        body[offset + 2],
        body[offset + 3],
    ]);
    offset += 4;

    if digest_count == 0 {
        anyhow::bail!("No digest returned");
    }

    if offset + 2 > body.len() {
        anyhow::bail!("Digest size truncated");
    }
    let digest_size = u16::from_be_bytes([body[offset], body[offset + 1]]) as usize;
    offset += 2;

    if offset + digest_size > body.len() {
        anyhow::bail!("Digest data truncated");
    }

    Ok(body[offset..offset + digest_size].to_vec())
}

fn pcr_measurement_type(pcr_index: u32) -> &'static str {
    match pcr_index {
        0..=7 => "ImmutableROM",
        8..=13 => "MutableFirmware",
        14 => "FirmwareConfiguration",
        _ => "MutableFirmware",
    }
}

pub fn validate_pcrs_against_policy(
    evidence: &AttestationEvidence,
    expected: &std::collections::HashMap<u32, String>,
) -> VerificationStatus {
    if evidence.measurements.is_empty() {
        return VerificationStatus::Unknown;
    }

    for (pcr_index, expected_value) in expected {
        match evidence.measurements.iter().find(|m| m.index == *pcr_index) {
            Some(m) if m.measurement == *expected_value => {}
            Some(_) => return VerificationStatus::Failed,
            None => return VerificationStatus::Failed,
        }
    }

    VerificationStatus::Success
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_pcr_read_command_pcr0() {
        let cmd = build_pcr_read_command(0);
        assert_eq!(&cmd[0..2], &TPM2_ST_NO_SESSIONS.to_be_bytes());
        assert_eq!(&cmd[6..10], &TPM2_CC_PCR_READ.to_be_bytes());
        // TPML_PCR_SELECTION.count = 1
        assert_eq!(&cmd[10..14], &1u32.to_be_bytes());
        // hash = SHA-256
        assert_eq!(&cmd[14..16], &TPM2_ALG_SHA256.to_be_bytes());
        // sizeofSelect = 3
        assert_eq!(cmd[16], 3);
        // pcr_select[0] bit 0 set
        assert_eq!(cmd[17], 0x01);
        assert_eq!(cmd[18], 0x00);
        assert_eq!(cmd[19], 0x00);
    }

    #[test]
    fn test_build_pcr_read_command_pcr7() {
        let cmd = build_pcr_read_command(7);
        assert_eq!(cmd[17], 0x80); // bit 7
        assert_eq!(cmd[18], 0x00);
        assert_eq!(cmd[19], 0x00);
    }

    #[test]
    fn test_build_pcr_read_command_pcr8() {
        let cmd = build_pcr_read_command(8);
        assert_eq!(cmd[17], 0x00);
        assert_eq!(cmd[18], 0x01); // bit 0 of second byte
        assert_eq!(cmd[19], 0x00);
    }

    #[test]
    fn test_parse_pcr_read_response_valid() {
        // Construct a valid response body:
        // pcrUpdateCounter(4) + selection_count(4) + 1 selection(2+1+3) + digest_count(4) + digest(2+32)
        let mut body = Vec::new();
        body.extend_from_slice(&42u32.to_be_bytes()); // pcrUpdateCounter
        body.extend_from_slice(&1u32.to_be_bytes()); // selection count
        body.extend_from_slice(&TPM2_ALG_SHA256.to_be_bytes()); // hash
        body.push(3); // sizeofSelect
        body.extend_from_slice(&[0x01, 0x00, 0x00]); // pcrSelect
        body.extend_from_slice(&1u32.to_be_bytes()); // digest count
        body.extend_from_slice(&32u16.to_be_bytes()); // digest size
        let digest = vec![0xAB; 32];
        body.extend_from_slice(&digest);

        let result = parse_pcr_read_response(&body).unwrap();
        assert_eq!(result, digest);
    }

    #[test]
    fn test_parse_pcr_read_response_empty_digest() {
        let mut body = Vec::new();
        body.extend_from_slice(&0u32.to_be_bytes()); // pcrUpdateCounter
        body.extend_from_slice(&0u32.to_be_bytes()); // selection count = 0
        body.extend_from_slice(&0u32.to_be_bytes()); // digest count = 0

        assert!(parse_pcr_read_response(&body).is_err());
    }

    #[test]
    fn test_validate_pcrs_success() {
        let evidence = AttestationEvidence {
            measurements: vec![
                MeasurementEntry {
                    index: 0,
                    measurement_type: "ImmutableROM".to_string(),
                    measurement: "abc123".to_string(),
                    hash_algorithm: "SHA-256".to_string(),
                    part_of_summary: true,
                    last_updated: None,
                },
                MeasurementEntry {
                    index: 7,
                    measurement_type: "ImmutableROM".to_string(),
                    measurement: "def456".to_string(),
                    hash_algorithm: "SHA-256".to_string(),
                    part_of_summary: true,
                    last_updated: None,
                },
            ],
            ..Default::default()
        };

        let mut expected = std::collections::HashMap::new();
        expected.insert(0, "abc123".to_string());
        expected.insert(7, "def456".to_string());

        assert_eq!(
            validate_pcrs_against_policy(&evidence, &expected),
            VerificationStatus::Success
        );
    }

    #[test]
    fn test_validate_pcrs_mismatch() {
        let evidence = AttestationEvidence {
            measurements: vec![MeasurementEntry {
                index: 0,
                measurement_type: "ImmutableROM".to_string(),
                measurement: "abc123".to_string(),
                hash_algorithm: "SHA-256".to_string(),
                part_of_summary: true,
                last_updated: None,
            }],
            ..Default::default()
        };

        let mut expected = std::collections::HashMap::new();
        expected.insert(0, "wrong_value".to_string());

        assert_eq!(
            validate_pcrs_against_policy(&evidence, &expected),
            VerificationStatus::Failed
        );
    }

    #[test]
    fn test_validate_pcrs_missing_pcr() {
        let evidence = AttestationEvidence {
            measurements: vec![MeasurementEntry {
                index: 0,
                measurement_type: "ImmutableROM".to_string(),
                measurement: "abc123".to_string(),
                hash_algorithm: "SHA-256".to_string(),
                part_of_summary: true,
                last_updated: None,
            }],
            ..Default::default()
        };

        let mut expected = std::collections::HashMap::new();
        expected.insert(7, "def456".to_string());

        assert_eq!(
            validate_pcrs_against_policy(&evidence, &expected),
            VerificationStatus::Failed
        );
    }

    #[test]
    fn test_validate_pcrs_empty_evidence() {
        let evidence = AttestationEvidence::default();
        let expected = std::collections::HashMap::new();
        assert_eq!(
            validate_pcrs_against_policy(&evidence, &expected),
            VerificationStatus::Unknown
        );
    }

    #[test]
    fn test_pcr_measurement_type() {
        assert_eq!(pcr_measurement_type(0), "ImmutableROM");
        assert_eq!(pcr_measurement_type(7), "ImmutableROM");
        assert_eq!(pcr_measurement_type(8), "MutableFirmware");
        assert_eq!(pcr_measurement_type(14), "FirmwareConfiguration");
        assert_eq!(pcr_measurement_type(15), "MutableFirmware");
    }
}
