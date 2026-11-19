//! SPDM responder role for `ComponentIntegrity.SPDMGetSignedMeasurements`.
//!
//! Builds DMTF DSP0274 MEASUREMENTS responses over a caller-supplied nonce and
//! signs them with a configured EC P-384 device key, so that out-of-band
//! attestation services (e.g., Keylime's OOB collector) can verify freshness,
//! device identity and measurement content without an in-band agent.
//!
//! Wire format produced (signature appended):
//!
//! ```text
//! SPDMVersion(1) SPDMMessageCode(1) Param1(1) Param2(1)
//! NumberOfBlocks(1) MeasurementRecordLength(3, uint24 LE)
//! MeasurementRecord(var)
//! Nonce(32)
//! OpaqueDataLength(2)
//! [RequesterContext(8), SPDM 1.3+]
//! Signature(var)
//! ```
//!
//! The MeasurementRecordLength is a 3-byte uint24 little-endian field in every
//! SPDM version (1.0 through 1.4), as is the OpaqueDataLength field after the
//! nonce — matching libspdm, DMTF's reference DSP0274 implementation. The
//! RequesterContext echo (and the corresponding field in GET_MEASUREMENTS
//! requests) is added from SPDM 1.3 onwards.
//!
//! A real SPDM responder does not have "a version": it answers GET_VERSION
//! with the list of versions it supports and runs subsequent messages at the
//! version the requester selects. The responder here advertises the versions
//! configured via `spdm_versions` (default: every version implemented) and,
//! where there is no live negotiation (the Redfish action path), emits at the
//! highest configured version.

use base64::Engine;
use p384::ecdsa::{Signature, SigningKey, signature::Signer};
use p384::elliptic_curve::pkcs8::DecodePrivateKey;
use p384::elliptic_curve::rand_core::{OsRng, RngCore};

use crate::config::SpdmConfig;

pub const SPDM_MEASUREMENTS_CODE: u8 = 0x60;
pub const NONCE_SIZE: usize = 32;
pub const SIGNATURE_SIZE: usize = 96; // raw r||s for ECDSA P-384
pub const SIGNING_ALGORITHM: &str = "TPM_ALG_ECDSA_ECC_NIST_P384";
pub const HASHING_ALGORITHM: &str = "SHA384";
/// Size of the RequesterContext echo carried in SPDM 1.3+ requests/responses.
pub const REQUESTER_CONTEXT_SIZE: usize = 8;

/// An SPDM version this responder can speak, configured via
/// `spdm_versions = ["1.1", ...]`. Ordered lowest to highest so the highest
/// configured version (the default emission version) is the maximum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SpdmVersion {
    V11,
    V12,
    V13,
    V14,
}

impl SpdmVersion {
    /// The SPDM version byte used in message headers (e.g. 0x11 for 1.1).
    pub fn byte(self) -> u8 {
        match self {
            Self::V11 => 0x11,
            Self::V12 => 0x12,
            Self::V13 => 0x13,
            Self::V14 => 0x14,
        }
    }

    /// The SPDM version as a `"major.minor"` string.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::V11 => "1.1",
            Self::V12 => "1.2",
            Self::V13 => "1.3",
            Self::V14 => "1.4",
        }
    }

    /// Parse a `"major.minor"` version string.
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "1.1" => Ok(Self::V11),
            "1.2" => Ok(Self::V12),
            "1.3" => Ok(Self::V13),
            "1.4" => Ok(Self::V14),
            other => Err(format!(
                "unknown SPDM version '{other}' (expected one of 1.1, 1.2, 1.3, 1.4)"
            )),
        }
    }

    /// Every version this responder implements.
    pub fn all() -> Vec<Self> {
        vec![Self::V11, Self::V12, Self::V13, Self::V14]
    }
}

impl std::fmt::Display for SpdmVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl<'de> serde::Deserialize<'de> for SpdmVersion {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Self::parse(&s).map_err(serde::de::Error::custom)
    }
}

/// Build the 100-byte SPDM 1.2+ signing context for a responder message:
/// four copies of the version-tagged `dmtf-spdm-vX.Y.*` prefix (64 bytes)
/// followed by 36 bytes of zero padding plus the operation's context string.
fn spdm_signing_context(version_byte: u8, op_code: u8) -> Vec<u8> {
    let mut context = Vec::with_capacity(100);
    let major = b'0' + (version_byte >> 4);
    let minor = b'0' + (version_byte & 0x0F);
    for _ in 0..4 {
        let mut prefix = *b"dmtf-spdm-v1.2.*";
        prefix[11] = major;
        prefix[13] = minor;
        context.extend_from_slice(&prefix);
    }
    // 36 bytes: zero padding then the operation's context string
    let op_context: &[u8] = match op_code {
        0x03 => b"responder-challenge_auth signing",
        0x60 => b"responder-measurements signing",
        _ => b"",
    };
    let padding = 36usize.saturating_sub(op_context.len());
    context.extend(std::iter::repeat_n(0u8, padding));
    context.extend_from_slice(op_context);
    context
}

/// A minimal static VCA (version/capabilities/algorithms negotiation transcript): the
/// GET_VERSION/VERSION, GET_CAPABILITIES/CAPABILITIES and NEGOTIATE_ALGORITHMS/ALGORITHMS
/// exchanges that precede a GET_MEASUREMENTS. The content is opaque to the verifier — it is
/// only included in the signed transcript — so a compact plausible exchange suffices. The
/// VERSION response advertises the responder's configured version list.
fn minimal_vca(advertised: &[SpdmVersion], negotiated: u8) -> Vec<u8> {
    let mut vca = Vec::new();
    // GET_VERSION / VERSION: the request always uses the 1.0 header
    vca.extend_from_slice(&[0x10, 0x84, 0x00, 0x00]); // request
    vca.extend_from_slice(&[0x10, 0x04, 0x00, 0x00, 0x00, advertised.len() as u8]); // response
    for version in advertised {
        // bit[15:12] major, bit[11:8] minor
        vca.extend_from_slice(&((version.byte() as u16) << 8).to_le_bytes());
    }
    // GET_CAPABILITIES / CAPABILITIES: MEAS_CAP (signed measurements) + CERT_CAP
    vca.extend_from_slice(&[negotiated, 0xE1, 0x00, 0x00]); // request
    vca.extend_from_slice(&[negotiated, 0x61, 0x00, 0x00]); // response header
    vca.extend_from_slice(&0x0000_0012u32.to_le_bytes()); // flags: MEAS_CAP_SIG + CERT_CAP
    vca.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]); // ctExponent + reserved
    // NEGOTIATE_ALGORITHMS / ALGORITHMS: ECDSA-P384 signing, SHA-384 hashing
    vca.extend_from_slice(&[negotiated, 0xE3, 0x00, 0x00]); // request
    vca.extend_from_slice(&[negotiated, 0x63, 0x00, 0x00, 0x00, 0x00]); // response
    vca
}

/// The GET_MEASUREMENTS request carrying the challenge nonce: Param1 bit 0
/// requests a signature, Param2 is the measurement operation; the body is the
/// nonce followed by the slot, plus the requester context from SPDM 1.3.
fn get_measurements_request(version: u8, nonce: &[u8]) -> Vec<u8> {
    let mut request = Vec::with_capacity(4 + nonce.len() + 1 + REQUESTER_CONTEXT_SIZE);
    request.extend_from_slice(&[version, 0xE0, 0x01, 0x00]);
    request.extend_from_slice(nonce);
    request.push(0x00); // SlotId
    if version >= 0x13 {
        request.extend_from_slice(&[0x00; REQUESTER_CONTEXT_SIZE]);
    }
    request
}

/// A loaded SPDM responder identity: signing key plus the certificate chain
/// blob served to attestation clients. `collection_delay_ms` models the time a
/// real BMC takes to traverse MCTP to the device's root of trust and run the
/// SPDM exchange — the reason SPDM-capable BMCs return a Task from
/// SPDMGetSignedMeasurements rather than responding synchronously.
#[derive(Clone)]
pub struct SpdmResponder {
    pub(crate) signing_key: SigningKey,
    pub(crate) cert_chain: Vec<u8>,
    pub(crate) measurements: Vec<Vec<u8>>,
    /// Simulated MCTP/SPDM collection latency in milliseconds (see struct docs)
    pub collection_delay_ms: u64,
    /// Sign the full L2 measurement transcript rather than just the response (see struct docs)
    pub sign_transcript: bool,
    /// The SPDM versions this responder advertises in GET_VERSION, ordered
    /// lowest to highest. Where there is no live negotiation (the Redfish
    /// action path), messages are emitted at the highest configured version.
    pub versions: Vec<SpdmVersion>,
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
        if measurements.is_empty() {
            // Documented default: deterministic pseudo-measurements. Derive
            // SHA-384 digests from the device's own public key so every
            // identity gets stable, distinct values (reproducible across
            // restarts, and different per system).
            use sha2::Digest;
            let verifying_key = signing_key.verifying_key();
            let public_point = verifying_key.to_encoded_point(false);
            let public_bytes = public_point.as_bytes();
            for i in 0u8..2 {
                let mut hasher = sha2::Sha384::new();
                hasher.update(public_bytes);
                hasher.update([i]);
                measurements.push(hasher.finalize().to_vec());
            }
        }

        let mut versions = config.spdm_versions.clone();
        if versions.is_empty() {
            versions = SpdmVersion::all();
        }
        versions.sort_unstable();
        versions.dedup();

        Ok(Self {
            signing_key,
            cert_chain,
            measurements,
            collection_delay_ms: config.collection_delay_ms,
            sign_transcript: config.sign_transcript,
            versions,
        })
    }

    /// The SPDM versions this responder advertises, lowest to highest.
    pub fn advertised_versions(&self) -> &[SpdmVersion] {
        &self.versions
    }

    /// Whether a raw SPDM version byte (e.g. 0x11) is one this responder
    /// advertises.
    pub fn supports_version(&self, version_byte: u8) -> bool {
        self.versions.iter().any(|v| v.byte() == version_byte)
    }

    /// The version used for messages when there is no live negotiation: the
    /// highest advertised version.
    pub fn highest_version(&self) -> SpdmVersion {
        self.versions
            .iter()
            .copied()
            .max()
            .unwrap_or(SpdmVersion::V11)
    }

    /// The emission version as a `"major.minor"` string.
    pub fn version_string(&self) -> &'static str {
        self.highest_version().as_str()
    }

    /// Sign a pre-computed digest with the device key: ECDSA P-384 over the
    /// raw digest (no re-hashing), returned in the SPDM raw `r||s` encoding.
    /// This is the signature format DSP0274 mandates for CHALLENGE_AUTH and
    /// signed MEASUREMENTS responses, which sign transcript hashes.
    pub fn sign_digest(&self, digest: &[u8]) -> Result<Vec<u8>, String> {
        use p384::ecdsa::signature::hazmat::PrehashSigner;
        let signature: Signature = self
            .signing_key
            .sign_prehash(digest)
            .map_err(|e| format!("cannot sign digest: {e}"))?;
        Ok(signature.to_bytes().to_vec())
    }

    /// Sign a transcript hash the way DSP0274 requires for the given message
    /// version: SPDM 1.1 signs the raw transcript hash, while SPDM 1.2+
    /// prepends the 100-byte SPDM signing context (four version-tagged
    /// `dmtf-spdm-vX.Y.*` prefixes plus the operation's context string) and
    /// signs the SHA-384 of the combination.
    pub fn sign_transcript_hash(
        &self,
        version_byte: u8,
        op_code: u8,
        transcript_hash: &[u8],
    ) -> Result<Vec<u8>, String> {
        if version_byte <= 0x11 {
            return self.sign_digest(transcript_hash);
        }
        let mut message = spdm_signing_context(version_byte, op_code);
        message.extend_from_slice(transcript_hash);
        let digest = {
            use sha2::Digest;
            sha2::Sha384::digest(&message)
        };
        self.sign_digest(&digest)
    }

    /// Verify a raw `r||s` ECDSA P-384 signature over a pre-computed digest
    /// (the mirror of [`Self::sign_digest`], used by tests).
    #[cfg(test)]
    pub fn verify_digest(&self, digest: &[u8], signature: &[u8]) -> bool {
        use p384::ecdsa::signature::hazmat::PrehashVerifier;
        match Signature::from_slice(signature) {
            Ok(sig) => self
                .signing_key
                .verifying_key()
                .verify_prehash(digest, &sig)
                .is_ok(),
            Err(_) => false,
        }
    }

    /// The configured measurement values, in measurement-index order.
    pub fn measurement_values(&self) -> &[Vec<u8>] {
        &self.measurements
    }

    /// The certificate chain in DMTF DSP0274 `spdm_cert_chain_t` wire format:
    /// `Length(4, little-endian, including all fields) || RootHash(SHA-384 of
    /// the root certificate DER) || concatenated DER certificates`, leaf last.
    ///
    /// The configured chain blob uses vbmc's internal storage format (2-byte
    /// total length followed by 2-byte length-prefixed DER certificates);
    /// this converts it to the format GET_CERTIFICATE serves on the wire.
    pub fn cert_chain_dsp0274(&self) -> Vec<u8> {
        use sha2::Digest;

        let mut ders = Vec::new();
        let blob = &self.cert_chain;
        if blob.len() >= 2 {
            let total = u16::from_le_bytes([blob[0], blob[1]]) as usize;
            let mut offset = 2;
            while offset + 2 <= total.min(blob.len()) {
                let len = u16::from_le_bytes([blob[offset], blob[offset + 1]]) as usize;
                offset += 2;
                if len == 0 || offset + len > blob.len() {
                    break;
                }
                ders.push(&blob[offset..offset + len]);
                offset += len;
            }
        }

        let mut chain = Vec::with_capacity(4 + 48 + ders.iter().map(|d| d.len()).sum::<usize>());
        chain.extend_from_slice(&[0, 0, 0, 0]); // Length, patched below
        if let Some(root) = ders.first() {
            let root_hash = sha2::Sha384::digest(root);
            chain.extend_from_slice(&root_hash);
        } else {
            chain.extend_from_slice(&[0u8; 48]);
        }
        for der in &ders {
            chain.extend_from_slice(der);
        }
        let length = chain.len() as u32;
        chain[..4].copy_from_slice(&length.to_le_bytes());
        chain
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

        // Measurement record: DSP0274 measurement blocks, which are two-level in
        // ALL SPDM versions: the outer block is Index(1) MeasurementSpecification(1)
        // MeasurementSize(2), and MeasurementSize bytes of DMTF measurement follow as
        // DMTFSpecMeasurementValueType(1) ValueSize(2) Value, with
        // MeasurementSize == 3 + ValueSize.
        let mut record = Vec::new();
        for (i, value) in &selected {
            let value_size = value.len() as u16;
            record.push((i + 1) as u8); // Index
            record.push(0x01); // MeasurementSpecification: DMTF
            record.extend_from_slice(&(3 + value_size).to_le_bytes()); // MeasurementSize
            record.push(0x02); // DMTFSpecMeasurementValueType: immutable ROM
            record.extend_from_slice(&value_size.to_le_bytes()); // ValueSize
            record.extend_from_slice(value);
        }

        // The responder generates its OWN random nonce for the MEASUREMENTS
        // response (DSP0274): the requester's nonce is never echoed. Freshness
        // comes from the signed transcript, which includes the GET_MEASUREMENTS
        // request carrying the requester's nonce.
        let mut responder_nonce = [0u8; NONCE_SIZE];
        OsRng.fill_bytes(&mut responder_nonce);

        // MEASUREMENTS response message. MeasurementRecordLength is a 3-byte
        // uint24 little-endian in every SPDM version (DSP0274 / libspdm).
        // Without a live negotiation (this is the Redfish action path), the
        // highest configured version is used.
        let version = self.highest_version().byte();
        let record_len_bytes = (record.len() as u32).to_le_bytes();
        let mut response =
            Vec::with_capacity(8 + record.len() + NONCE_SIZE + 2 + REQUESTER_CONTEXT_SIZE);
        response.extend_from_slice(&[version, SPDM_MEASUREMENTS_CODE, 0x00, 0x00]);
        response.push(selected.len() as u8);
        response.extend_from_slice(&record_len_bytes[..3]);
        response.extend_from_slice(&record);
        response.extend_from_slice(&responder_nonce);
        response.extend_from_slice(&0u16.to_le_bytes()); // opaque data length
        if version >= 0x13 {
            // SPDM 1.3+: echo the requester context (zeros in this synthetic
            // transcript) after the opaque data, before the signature.
            response.extend_from_slice(&[0x00; REQUESTER_CONTEXT_SIZE]);
        }

        // The signed portion: real SPDM responders sign the full L2 measurement transcript —
        // VCA (version/capabilities/algorithms negotiation) followed by the GET_MEASUREMENTS
        // request and the MEASUREMENTS response. The response is always the last message,
        // immediately preceding the signature.
        let signed_portion = if self.sign_transcript {
            let mut transcript = minimal_vca(&self.versions, version);
            transcript.extend_from_slice(&get_measurements_request(version, &nonce));
            transcript.extend_from_slice(&response);
            transcript
        } else {
            response
        };

        // Signature: ECDSA P-384 (the Signer impl hashes the message with
        // SHA-384), raw r||s encoding (SPDM style, not DER)
        let signature: Signature = self.signing_key.sign(&signed_portion);
        let mut signed = signed_portion;
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

    fn test_responder(versions: Vec<SpdmVersion>) -> SpdmResponder {
        use p384::elliptic_curve::rand_core::OsRng;
        let signing_key = SigningKey::random(&mut OsRng);
        SpdmResponder {
            signing_key,
            cert_chain: b"\x04\x00\x02\x00\xca\xfe".to_vec(),
            measurements: vec![vec![0xaa; 48], vec![0xbb; 48]],
            collection_delay_ms: 0,
            sign_transcript: false,
            versions,
        }
    }

    /// Length of the response header, including the 3-byte uint24
    /// MeasurementRecordLength field (present in every SPDM version).
    fn header_len() -> usize {
        8
    }

    /// Read the 3-byte uint24 little-endian MeasurementRecordLength at `offset`.
    fn record_len_at(bytes: &[u8], offset: usize) -> usize {
        (bytes[offset] as usize)
            | ((bytes[offset + 1] as usize) << 8)
            | ((bytes[offset + 2] as usize) << 16)
    }

    /// Parse a MEASUREMENTS response (message only, no signature) the way a
    /// strict DSP0274 verifier would, asserting the two-level measurement
    /// block encoding. Returns (number_of_blocks, record, responder_nonce).
    fn parse_response(response: &[u8], version_byte: u8) -> (u8, Vec<u8>, [u8; NONCE_SIZE]) {
        assert_eq!(response[0], version_byte);
        assert_eq!(response[1], SPDM_MEASUREMENTS_CODE);
        let number_of_blocks = response[4];
        let record_len = record_len_at(response, 5);
        let record = &response[8..8 + record_len];
        let nonce_start = 8 + record_len;
        let mut nonce = [0u8; NONCE_SIZE];
        nonce.copy_from_slice(&response[nonce_start..nonce_start + NONCE_SIZE]);
        // OpaqueDataLength is part of the layout in every SPDM version, always
        // zero here
        let opaque = u16::from_le_bytes([
            response[nonce_start + NONCE_SIZE],
            response[nonce_start + NONCE_SIZE + 1],
        ]);
        assert_eq!(opaque, 0);
        let expected_len = nonce_start
            + NONCE_SIZE
            + 2
            + if version_byte >= 0x13 {
                REQUESTER_CONTEXT_SIZE
            } else {
                0
            };
        assert_eq!(response.len(), expected_len);

        // Two-level measurement blocks: Index(1) MeasurementSpecification(1)
        // MeasurementSize(2) { DMTFSpecMeasurementValueType(1) ValueSize(2) Value }
        let mut offset = 0usize;
        let mut prev_index = 0u8;
        let mut block_count = 0u8;
        while offset < record.len() {
            let index = record[offset];
            let spec = record[offset + 1];
            let measurement_size =
                u16::from_le_bytes([record[offset + 2], record[offset + 3]]) as usize;
            let value_type = record[offset + 4];
            let value_size = u16::from_le_bytes([record[offset + 5], record[offset + 6]]) as usize;
            assert!(index > prev_index, "block indices must be increasing");
            prev_index = index;
            assert_eq!(spec, 0x01); // MeasurementSpecification: DMTF
            assert_eq!(measurement_size, 3 + value_size);
            assert_eq!(value_type, 0x02); // immutable ROM
            offset += 4 + measurement_size;
            block_count += 1;
        }
        assert_eq!(offset, record.len(), "record must be fully consumed");
        assert_eq!(block_count, number_of_blocks);

        (number_of_blocks, record.to_vec(), nonce)
    }

    /// Extract the response message (without signature) from a signed blob.
    /// When the full transcript is signed, the response is the last message:
    /// locate it by scanning for a MEASUREMENTS message whose parsed length
    /// lands exactly at the signature boundary.
    fn response_of(signed: &[u8], sign_transcript: bool, version_byte: u8) -> &[u8] {
        let sig_start = signed.len() - SIGNATURE_SIZE;
        if !sign_transcript {
            return &signed[..sig_start];
        }
        let mut offset = None;
        for i in 0..sig_start.saturating_sub(8) {
            if signed[i] != version_byte || signed[i + 1] != SPDM_MEASUREMENTS_CODE {
                continue;
            }
            let record_len = record_len_at(signed, i + 5);
            let context = if version_byte >= 0x13 {
                REQUESTER_CONTEXT_SIZE
            } else {
                0
            };
            let end = i + 8 + record_len + NONCE_SIZE + 2 + context;
            if end == sig_start {
                offset = Some(i); // keep the last (rightmost) match
            }
        }
        let offset = offset.expect("MEASUREMENTS response not found in transcript");
        &signed[offset..sig_start]
    }

    #[test]
    fn test_spdm_version_parsing() {
        assert_eq!(SpdmVersion::parse("1.1").unwrap(), SpdmVersion::V11);
        assert_eq!(SpdmVersion::parse("1.2").unwrap(), SpdmVersion::V12);
        assert_eq!(SpdmVersion::parse("1.3").unwrap(), SpdmVersion::V13);
        assert_eq!(SpdmVersion::parse("1.4").unwrap(), SpdmVersion::V14);
        assert!(SpdmVersion::parse("1.0").is_err());
        assert!(SpdmVersion::parse("cec173x").is_err());
        assert!(SpdmVersion::parse("2.0").is_err());
        // Version bytes and ordering: the highest configured version wins
        assert!(SpdmVersion::V11 < SpdmVersion::V14);
        assert_eq!(SpdmVersion::V13.byte(), 0x13);
        assert_eq!(SpdmVersion::V12.as_str(), "1.2");
    }

    #[test]
    fn test_configured_versions_parse_from_toml() {
        #[derive(serde::Deserialize)]
        struct Cfg {
            spdm_versions: Vec<SpdmVersion>,
        }
        let cfg: Cfg = toml::from_str(r#"spdm_versions = ["1.1", "1.2"]"#).unwrap();
        assert_eq!(cfg.spdm_versions, vec![SpdmVersion::V11, SpdmVersion::V12]);
        assert!(toml::from_str::<Cfg>(r#"spdm_versions = ["1.0"]"#).is_err());
    }

    #[test]
    fn test_highest_version_wins_by_default() {
        // Default (all versions): emits at the highest implemented version
        let responder = test_responder(SpdmVersion::all());
        assert_eq!(responder.highest_version(), SpdmVersion::V14);
        assert_eq!(responder.version_string(), "1.4");
        assert!(responder.supports_version(0x11));
        assert!(responder.supports_version(0x14));
        assert!(!responder.supports_version(0x10));
        // A pinned single-version list (the GB300 harness configuration)
        let responder = test_responder(vec![SpdmVersion::V11]);
        assert_eq!(responder.highest_version(), SpdmVersion::V11);
        assert_eq!(responder.version_string(), "1.1");
        assert!(!responder.supports_version(0x12));
    }

    #[test]
    fn test_signed_measurements_layout_11() {
        let responder = test_responder(vec![SpdmVersion::V11]);
        let nonce_hex = "11".repeat(32);
        let signed = responder.signed_measurements(&nonce_hex, &[]).unwrap();

        assert_eq!(signed[0], 0x11);
        assert_eq!(signed[1], SPDM_MEASUREMENTS_CODE);
        assert_eq!(signed[4], 2); // two blocks
        // 3-byte (uint24, little-endian) record length
        let record_len = record_len_at(&signed, 5);
        // message = header(8) + record + nonce(32) + opaque len(2), NO
        // requester context before 1.3; signature appended
        assert_eq!(
            signed.len(),
            header_len() + record_len + NONCE_SIZE + 2 + SIGNATURE_SIZE
        );
    }

    #[test]
    fn test_signed_measurements_layout_12() {
        let responder = test_responder(vec![SpdmVersion::V12]);
        let nonce_hex = "22".repeat(32);
        let signed = responder.signed_measurements(&nonce_hex, &[]).unwrap();

        assert_eq!(signed[0], 0x12);
        assert_eq!(signed[1], SPDM_MEASUREMENTS_CODE);
        assert_eq!(signed[4], 2); // two blocks
        // MeasurementRecordLength is a 3-byte uint24 in every SPDM version,
        // including 1.2 (matches libspdm / DMTF spdm-dump)
        let record_len = record_len_at(&signed, 5);
        assert_eq!(
            record_len,
            (signed[5] as usize) | ((signed[6] as usize) << 8) | ((signed[7] as usize) << 16)
        );
        // ...and no fourth length byte and no requester context before 1.3
        assert_eq!(
            signed.len(),
            header_len() + record_len + NONCE_SIZE + 2 + SIGNATURE_SIZE
        );
    }

    #[test]
    fn test_signed_measurements_layout_13_requester_context() {
        let responder = test_responder(vec![SpdmVersion::V13]);
        let nonce_hex = "33".repeat(32);
        let signed = responder.signed_measurements(&nonce_hex, &[]).unwrap();

        assert_eq!(signed[0], 0x13);
        let record_len = record_len_at(&signed, 5);
        // SPDM 1.3+: 8-byte requester context after the opaque data, before
        // the signature
        assert_eq!(
            signed.len(),
            header_len() + record_len + NONCE_SIZE + 2 + REQUESTER_CONTEXT_SIZE + SIGNATURE_SIZE
        );
        // ...and the context bytes are the zero echo of the synthetic request
        let context_start = header_len() + record_len + NONCE_SIZE + 2;
        assert_eq!(
            &signed[context_start..context_start + REQUESTER_CONTEXT_SIZE],
            &[0u8; REQUESTER_CONTEXT_SIZE]
        );

        // Default (all versions) emits at the highest configured version
        let responder = test_responder(SpdmVersion::all());
        let signed = responder.signed_measurements(&nonce_hex, &[]).unwrap();
        assert_eq!(signed[0], 0x14);
    }

    #[test]
    fn test_signed_measurements_two_level_blocks() {
        // Per DSP0274 the block is two-level in ALL versions: every version
        // must encode Index(1) Spec(1) MeasurementSize(2) wrapping ValueType(1)
        // ValueSize(2) Value with MeasurementSize == 3 + ValueSize.
        for version in SpdmVersion::all() {
            let responder = test_responder(vec![version]);
            let signed = responder
                .signed_measurements(&"11".repeat(32), &[])
                .unwrap();
            let response = response_of(&signed, false, version.byte());
            let (blocks, _record, _nonce) = parse_response(response, version.byte());
            assert_eq!(blocks, 2);
        }
    }

    #[test]
    fn test_responder_nonce_differs_from_request_nonce() {
        // DSP0274: the responder generates its OWN random nonce and never echoes
        // the requester's.
        let request_nonce = [0x33u8; 32];
        let nonce_hex = hex::encode(request_nonce);

        for version in SpdmVersion::all() {
            let responder = test_responder(vec![version]);
            let signed = responder.signed_measurements(&nonce_hex, &[]).unwrap();
            let response = response_of(&signed, false, version.byte());
            let (_, _, responder_nonce) = parse_response(response, version.byte());
            assert_ne!(
                responder_nonce, request_nonce,
                "responder nonce must not echo the request nonce ({version})"
            );
        }
    }

    #[test]
    fn test_signed_measurements_round_trip_parser() {
        // Round-trip responses of every version through the strict parser,
        // with and without the transcript signing mode.
        for version in SpdmVersion::all() {
            for sign_transcript in [false, true] {
                let mut responder = test_responder(vec![version]);
                responder.sign_transcript = sign_transcript;

                let signed = responder
                    .signed_measurements(&"77".repeat(32), &[])
                    .unwrap();
                let response = response_of(&signed, sign_transcript, version.byte());
                let (blocks, record, _nonce) = parse_response(response, version.byte());
                assert_eq!(blocks, 2);

                // Index selection round-trips: block 2 carries the second value
                let signed_one = responder
                    .signed_measurements(&"77".repeat(32), &[1])
                    .unwrap();
                let response_one = response_of(&signed_one, sign_transcript, version.byte());
                let (blocks_one, record_one, _nonce_one) =
                    parse_response(response_one, version.byte());
                assert_eq!(blocks_one, 1);
                // ...and its record is a strict suffix of the full record
                assert!(record.ends_with(&record_one));
            }
        }
    }

    #[test]
    fn test_load_synthesizes_pseudo_measurements() {
        // With no spdm.measurements configured, load() must synthesize
        // deterministic pseudo-measurements (documented default) so that
        // SPDMGetSignedMeasurements works out of the box.
        use p384::elliptic_curve::rand_core::OsRng;

        let dir = std::env::temp_dir().join(format!("vbmc-spdm-load-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let key = SigningKey::random(&mut OsRng);
        let key_pem = p384::SecretKey::from(&key)
            .to_sec1_pem(Default::default())
            .unwrap();
        std::fs::write(dir.join("key.pem"), key_pem.as_str()).unwrap();
        std::fs::write(dir.join("chain.blob"), b"\x04\x00\x02\x00\xca\xfe").unwrap();

        let config = crate::config::SpdmConfig {
            private_key: dir.join("key.pem"),
            cert_chain: dir.join("chain.blob"),
            measurements: vec![],
            collection_delay_ms: 0,
            sign_transcript: true,
            spdm_versions: vec![SpdmVersion::V11],
            spdm_tcp_listen: None,
        };

        let responder =
            SpdmResponder::load(&config).expect("load must succeed without measurements");
        let nonce_hex = "11".repeat(32);

        let signed = responder
            .signed_measurements(&nonce_hex, &[255])
            .expect("all-indices request must succeed with synthesized measurements");
        let response = response_of(&signed, true, 0x11);
        let (blocks, _record, _nonce) = parse_response(response, 0x11);
        assert_eq!(blocks, 2, "two pseudo-measurement blocks are synthesized");

        // Deterministic: the same identity yields the same measurements again
        let responder2 = SpdmResponder::load(&config).unwrap();
        let signed2 = responder2.signed_measurements(&nonce_hex, &[255]).unwrap();
        let (blocks2, record2, _) = parse_response(response_of(&signed2, true, 0x11), 0x11);
        assert_eq!(blocks2, 2);
        let (_, record, _) = parse_response(response, 0x11);
        assert_eq!(
            record, record2,
            "pseudo-measurements must be stable across loads"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_signed_measurements_selects_indices() {
        let responder = test_responder(vec![SpdmVersion::V11]);
        let nonce_hex = "22".repeat(32);
        let signed = responder.signed_measurements(&nonce_hex, &[1]).unwrap();
        assert_eq!(signed[4], 1); // one block
    }

    #[test]
    fn test_signed_measurements_rejects_bad_nonce() {
        let responder = test_responder(vec![SpdmVersion::V11]);
        assert!(responder.signed_measurements("aabb", &[]).is_err());
        assert!(
            responder
                .signed_measurements(&"zz".repeat(32), &[])
                .is_err()
        );
    }

    #[test]
    fn test_signed_measurements_transcript_layout() {
        let mut responder = test_responder(vec![SpdmVersion::V11]);
        responder.sign_transcript = true;

        let nonce_hex = "55".repeat(32);
        let signed = responder.signed_measurements(&nonce_hex, &[]).unwrap();

        // The transcript starts with the VCA (GET_VERSION request, 1.0 header),
        // not the MEASUREMENTS response
        assert_eq!(signed[0], 0x10);
        assert_eq!(signed[1], 0x84);

        // VERSION response: [0x10, 0x04, 0, 0, reserved, count, entries...]
        assert_eq!(signed[5], 0x04);
        let count = signed[9] as usize;
        assert_eq!(count, 1);
        assert_eq!(&signed[10..12], &0x1100u16.to_le_bytes());

        // The response is the last message and round-trips through the parser
        let response = response_of(&signed, true, 0x11);
        assert!(response.as_ptr() as usize > signed.as_ptr() as usize);
        let (blocks, _record, _nonce) = parse_response(response, 0x11);
        assert_eq!(blocks, 2);
    }

    #[test]
    fn test_transcript_vca_advertises_all_configured_versions() {
        let mut responder = test_responder(vec![SpdmVersion::V11, SpdmVersion::V13]);
        responder.sign_transcript = true;

        let signed = responder
            .signed_measurements(&"55".repeat(32), &[])
            .unwrap();
        // VERSION response: [0x10, 0x04, 0, 0, reserved, count=2, entries...]
        assert_eq!(signed[9], 2);
        assert_eq!(&signed[10..12], &0x1100u16.to_le_bytes());
        assert_eq!(&signed[12..14], &0x1300u16.to_le_bytes());
        // ...and the rest of the VCA runs at the negotiated (highest) version
        assert_eq!(signed[14], 0x13);
    }

    #[test]
    fn test_signed_measurements_transcript_signature_covers_vca() {
        use p384::ecdsa::signature::Verifier;

        let mut responder = test_responder(vec![SpdmVersion::V11]);
        responder.sign_transcript = true;

        let nonce_hex = "66".repeat(32);
        let signed = responder.signed_measurements(&nonce_hex, &[]).unwrap();

        let sig_start = signed.len() - SIGNATURE_SIZE;
        let transcript = &signed[..sig_start];
        let signature = Signature::from_slice(&signed[sig_start..]).unwrap();

        // The signature verifies over the WHOLE transcript (VCA included), not just the response
        responder
            .signing_key
            .verifying_key()
            .verify(transcript, &signature)
            .expect("signature must cover the full transcript");

        // ...and does NOT verify over just the trailing response: locate the response and prove
        // that signing only that part would fail (guards against regressing to response-only signing)
        let response = response_of(&signed, true, 0x11);
        if !std::ptr::eq(response.as_ptr(), signed.as_ptr()) {
            assert!(
                responder
                    .signing_key
                    .verifying_key()
                    .verify(response, &signature)
                    .is_err(),
                "signature must not verify over the response alone"
            );
        }
    }

    #[test]
    fn test_signed_measurements_signature_verifies() {
        use p384::ecdsa::signature::Verifier;

        let responder = test_responder(vec![SpdmVersion::V12]);
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
        let responder = test_responder(vec![SpdmVersion::V11]);
        let a = responder
            .signed_measurements(&"11".repeat(32), &[])
            .unwrap();
        let b = responder
            .signed_measurements(&"44".repeat(32), &[])
            .unwrap();
        assert_ne!(a, b);
    }
}
