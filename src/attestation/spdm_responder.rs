//! SPDM responder role for `ComponentIntegrity.SPDMGetSignedMeasurements`.
//!
//! Builds DMTF DSP0274 MEASUREMENTS responses over a caller-supplied nonce and
//! signs them with a configured EC P-384 device key, so that out-of-band
//! attestation services (e.g., Keylime's OOB collector) can verify freshness,
//! device identity and measurement content without an in-band agent.
//!
//! Wire format produced (SPDM 1.2, signature appended):
//!
//! ```text
//! SPDMVersion(1) SPDMMessageCode(1) Param1(1) Param2(1)
//! NumberOfBlocks(1) MeasurementRecordLength(4)
//! MeasurementRecord(var)
//! Nonce(32)
//! OpaqueDataLength(2)
//! Signature(var)
//! ```

use base64::Engine;
use p384::ecdsa::{Signature, SigningKey, signature::Signer};
use p384::elliptic_curve::pkcs8::DecodePrivateKey;

use crate::config::SpdmConfig;

pub const SPDM_VERSION: u8 = 0x12; // SPDM 1.2
pub const SPDM_MEASUREMENTS_CODE: u8 = 0x60;
pub const NONCE_SIZE: usize = 32;
pub const SIGNATURE_SIZE: usize = 96; // raw r||s for ECDSA P-384
pub const SIGNING_ALGORITHM: &str = "TPM_ALG_ECDSA_ECC_NIST_P384";
pub const HASHING_ALGORITHM: &str = "SHA384";
pub const SPDM_VERSION_STRING: &str = "1.2";

/// A loaded SPDM responder identity: signing key plus the certificate chain
/// blob served to attestation clients. `collection_delay_ms` models the time a
/// real BMC takes to traverse MCTP to the device's root of trust and run the
/// SPDM exchange — the reason SPDM-capable BMCs return a Task from
/// SPDMGetSignedMeasurements rather than responding synchronously.
#[derive(Clone)]
pub struct SpdmResponder {
    signing_key: SigningKey,
    cert_chain: Vec<u8>,
    measurements: Vec<Vec<u8>>,
    /// Simulated MCTP/SPDM collection latency in milliseconds (see struct docs)
    pub collection_delay_ms: u64,
}

impl SpdmResponder {
    /// Load a responder from configuration. The private key must be an EC
    /// P-384 key in SEC1 or PKCS#8 PEM form; the certificate chain must be a
    /// DMTF DSP0274 chain blob (length-prefixed DER certificates, leaf first).
    pub fn load(config: &SpdmConfig) -> Result<Self, String> {
        let key_pem = std::fs::read_to_string(&config.private_key)
            .map_err(|e| format!("cannot read SPDM private key: {e}"))?;
        let signing_key = SigningKey::from_pkcs8_pem(&key_pem)
            .or_else(|_| p384::SecretKey::from_sec1_pem(&key_pem).map(SigningKey::from))
            .map_err(|e| format!("cannot parse SPDM private key as EC P-384 PEM: {e}"))?;

        let cert_chain = std::fs::read(&config.cert_chain)
            .map_err(|e| format!("cannot read SPDM certificate chain: {e}"))?;

        let mut measurements = Vec::new();
        for (i, hexval) in config.measurements.iter().enumerate() {
            let bytes = hex::decode(hexval.trim_start_matches("0x"))
                .map_err(|e| format!("invalid hex in spdm.measurements[{i}]: {e}"))?;
            measurements.push(bytes);
        }

        Ok(Self {
            signing_key,
            cert_chain,
            measurements,
            collection_delay_ms: config.collection_delay_ms,
        })
    }

    /// The DMTF DSP0274 certificate chain blob (leaf first).
    pub fn cert_chain(&self) -> &[u8] {
        &self.cert_chain
    }

    /// The DMTF DSP0274 certificate chain blob, re-armored as concatenated PEM certificates
    /// (leaf first). This is the form served by Redfish Certificate resources, matching what
    /// SPDM-capable BMCs publish at `/redfish/v1/Chassis/{id}/Certificates/{slot}`.
    pub fn cert_chain_pem(&self) -> String {
        let mut pem = String::new();
        let blob = &self.cert_chain;

        if blob.len() < 2 {
            return pem;
        }

        let total = u16::from_le_bytes([blob[0], blob[1]]) as usize;
        let mut offset = 2;

        while offset + 2 <= total.min(blob.len()) {
            let len = u16::from_le_bytes([blob[offset], blob[offset + 1]]) as usize;
            offset += 2;
            if len == 0 || offset + len > blob.len() {
                break;
            }
            let der = &blob[offset..offset + len];
            offset += len;

            pem.push_str("-----BEGIN CERTIFICATE-----\n");
            let b64 = base64::engine::general_purpose::STANDARD.encode(der);
            for chunk in b64.as_bytes().chunks(64) {
                pem.push_str(std::str::from_utf8(chunk).unwrap_or(""));
                pem.push('\n');
            }
            pem.push_str("-----END CERTIFICATE-----\n");
        }

        pem
    }

    /// The leaf certificate public key in PEM form.
    pub fn public_key_pem(&self) -> String {
        use p384::elliptic_curve::pkcs8::EncodePublicKey;
        self.signing_key
            .verifying_key()
            .to_public_key_pem(p384::elliptic_curve::pkcs8::LineEnding::LF)
            .unwrap_or_default()
    }

    /// Build and sign an SPDM MEASUREMENTS response over the given nonce.
    ///
    /// `indices` selects which measurement blocks to include (empty or [255]
    /// means all). Returns the full signed statement (message plus trailing
    /// raw ECDSA signature) exactly as conveyed in the Redfish action's
    /// `SignedMeasurements` property.
    pub fn signed_measurements(&self, nonce_hex: &str, indices: &[u8]) -> Result<Vec<u8>, String> {
        if nonce_hex.len() != NONCE_SIZE * 2 {
            return Err(format!(
                "nonce must be a {}-character hex string, got {} characters",
                NONCE_SIZE * 2,
                nonce_hex.len()
            ));
        }
        let nonce = hex::decode(nonce_hex).map_err(|e| format!("nonce is not valid hex: {e}"))?;

        let selected: Vec<(usize, &[u8])> = if indices.is_empty() || indices.contains(&255) {
            self.measurements
                .iter()
                .enumerate()
                .map(|(i, m)| (i, m.as_slice()))
                .collect()
        } else {
            indices
                .iter()
                .filter_map(|&i| {
                    self.measurements
                        .get(i as usize)
                        .map(|m| (i as usize, m.as_slice()))
                })
                .collect()
        };

        if selected.is_empty() {
            return Err("no measurement blocks available for the requested indices".to_string());
        }

        // Measurement record: DMTF measurement blocks (index, spec, dmtf spec, type, size, value)
        let mut record = Vec::new();
        for (i, value) in &selected {
            record.extend_from_slice(&[(i + 1) as u8, 0x01, 0x01, 0x02]);
            record.extend_from_slice(&(value.len() as u16).to_le_bytes());
            record.extend_from_slice(value);
        }

        // Message (the signed portion)
        let mut message = Vec::with_capacity(9 + record.len() + NONCE_SIZE + 2 + SIGNATURE_SIZE);
        message.extend_from_slice(&[SPDM_VERSION, SPDM_MEASUREMENTS_CODE, 0x00, 0x00]);
        message.push(selected.len() as u8);
        message.extend_from_slice(&(record.len() as u32).to_le_bytes());
        message.extend_from_slice(&record);
        message.extend_from_slice(&nonce);
        message.extend_from_slice(&0u16.to_le_bytes()); // opaque data length

        // Signature: ECDSA P-384 (the Signer impl hashes the message with
        // SHA-384), raw r||s encoding (SPDM style, not DER)
        let signature: Signature = self.signing_key.sign(&message);
        let mut signed = message;
        signed.extend_from_slice(&signature.to_bytes());

        Ok(signed)
    }
}

/// Parse the Redfish action request parameters for SPDMGetSignedMeasurements.
#[derive(Debug, serde::Deserialize)]
pub struct SpdmGetSignedMeasurementsRequest {
    #[serde(default, rename = "Nonce")]
    pub nonce: Option<String>,
    #[serde(default, rename = "SlotId")]
    pub slot_id: Option<u8>,
    #[serde(default, rename = "MeasurementIndices")]
    pub measurement_indices: Option<Vec<u8>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_responder() -> SpdmResponder {
        use p384::elliptic_curve::rand_core::OsRng;
        let signing_key = SigningKey::random(&mut OsRng);
        SpdmResponder {
            signing_key,
            cert_chain: b"\x04\x00\x02\x00\xca\xfe".to_vec(),
            measurements: vec![vec![0xaa; 48], vec![0xbb; 48]],
            collection_delay_ms: 0,
        }
    }

    #[test]
    fn test_signed_measurements_layout() {
        let responder = test_responder();
        let nonce_hex = "11".repeat(32);
        let signed = responder.signed_measurements(&nonce_hex, &[]).unwrap();

        assert_eq!(signed[0], SPDM_VERSION);
        assert_eq!(signed[1], SPDM_MEASUREMENTS_CODE);
        assert_eq!(signed[4], 2); // two blocks
        let record_len = u32::from_le_bytes([signed[5], signed[6], signed[7], signed[8]]) as usize;
        // message = header(9) + record + nonce(32) + opaque len(2); signature appended
        assert_eq!(
            signed.len(),
            9 + record_len + NONCE_SIZE + 2 + SIGNATURE_SIZE
        );
        // nonce is embedded right after the record
        assert_eq!(
            &signed[9 + record_len..9 + record_len + NONCE_SIZE],
            &[0x11u8; 32]
        );
    }

    #[test]
    fn test_signed_measurements_selects_indices() {
        let responder = test_responder();
        let nonce_hex = "22".repeat(32);
        let signed = responder.signed_measurements(&nonce_hex, &[1]).unwrap();
        assert_eq!(signed[4], 1); // one block
    }

    #[test]
    fn test_signed_measurements_rejects_bad_nonce() {
        let responder = test_responder();
        assert!(responder.signed_measurements("aabb", &[]).is_err());
        assert!(
            responder
                .signed_measurements(&"zz".repeat(32), &[])
                .is_err()
        );
    }

    #[test]
    fn test_signed_measurements_signature_verifies() {
        use p384::ecdsa::signature::Verifier;

        let responder = test_responder();
        let nonce_hex = "33".repeat(32);
        let signed = responder.signed_measurements(&nonce_hex, &[]).unwrap();

        let message = &signed[..signed.len() - SIGNATURE_SIZE];
        let sig_bytes = &signed[signed.len() - SIGNATURE_SIZE..];
        let signature = Signature::from_slice(sig_bytes).unwrap();

        responder
            .signing_key
            .verifying_key()
            .verify(message, &signature)
            .expect("signature must verify");
    }

    #[test]
    fn test_nonce_changes_signature() {
        let responder = test_responder();
        let a = responder
            .signed_measurements(&"11".repeat(32), &[])
            .unwrap();
        let b = responder
            .signed_measurements(&"44".repeat(32), &[])
            .unwrap();
        assert_ne!(a, b);
    }
}
