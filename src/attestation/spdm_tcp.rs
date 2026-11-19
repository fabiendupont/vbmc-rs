//! SPDM over TCP transport binding (DMTF DSP0287) for the SPDM responder.
//!
//! Serves the same responder identity as the Redfish
//! `ComponentIntegrity.SPDMGetSignedMeasurements` action, but over a raw TCP
//! socket so that DMTF's reference tooling — libspdm, `spdm_requester_emu`
//! and the SPDM Responder Validator (`spdm_device_validator_sample`) — can
//! exercise the responder directly, the way they would a real device eRoT
//! (e.g. the Microchip CEC173x in a Lenovo GB300 NVL72 tray).
//!
//! Two layers of framing are involved:
//!
//! 1. The spdm-emu "platform" socket protocol (shared by all DMTF SPDM test
//!    tooling): each direction sends `command(4, big-endian) ||
//!    transport_type(4, big-endian) || payload_size(4, big-endian) ||
//!    payload`. The connection starts with a `TEST` (0xDEAD) command
//!    handshake and ends with `SHUTDOWN` (0xFFFE).
//! 2. DSP0287 SPDM over TCP: each payload is
//!    `payload_length(2, little-endian) || binding_version(0x01) ||
//!    message_type (0x05 = out-of-session SPDM) || SPDM message`.
//!
//! The dispatcher implements the messages an OOB attestation collector needs:
//! GET_VERSION, GET_CAPABILITIES, NEGOTIATE_ALGORITHMS (the VCA), GET_DIGESTS,
//! GET_CERTIFICATE, CHALLENGE and GET_MEASUREMENTS. Signed responses
//! (CHALLENGE_AUTH, MEASUREMENTS) are ECDSA P-384 signatures over the
//! SHA-384 transcript hashes (M1 and L1/L2) computed from the *actual*
//! messages exchanged on the connection, per DSP0274 — not over a synthetic
//! transcript. KEY_EXCHANGE and the other session-based messages are out of
//! scope and answered with `UnsupportedRequest`.

use std::sync::Arc;

use p384::elliptic_curve::rand_core::{OsRng, RngCore};
use sha2::Digest;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;

use crate::attestation::spdm_responder::{REQUESTER_CONTEXT_SIZE, SpdmResponder};

// ---------------------------------------------------------------------------
// spdm-emu platform socket protocol (command framing)
// ---------------------------------------------------------------------------

const SOCKET_SPDM_COMMAND_NORMAL: u32 = 0x0001;
const SOCKET_SPDM_COMMAND_SHUTDOWN: u32 = 0xFFFE;
const SOCKET_SPDM_COMMAND_TEST: u32 = 0xDEAD;
const SOCKET_TRANSPORT_TYPE_TCP: u32 = 0x03;

// ---------------------------------------------------------------------------
// DSP0287 SPDM over TCP binding
// ---------------------------------------------------------------------------

const SPDM_TCP_BINDING_VERSION: u8 = 0x01;
const SPDM_TCP_MESSAGE_TYPE_OUT_OF_SESSION: u8 = 0x05;
const SPDM_TCP_BINDING_HEADER_SIZE: usize = 4;

// ---------------------------------------------------------------------------
// SPDM message codes (DSP0274)
// ---------------------------------------------------------------------------

const SPDM_GET_VERSION: u8 = 0x84;
const SPDM_GET_CAPABILITIES: u8 = 0xE1;
const SPDM_NEGOTIATE_ALGORITHMS: u8 = 0xE3;
const SPDM_GET_DIGESTS: u8 = 0x81;
const SPDM_GET_CERTIFICATE: u8 = 0x82;
const SPDM_CHALLENGE: u8 = 0x83;
const SPDM_GET_MEASUREMENTS: u8 = 0xE0;

const SPDM_VERSION: u8 = 0x04;
const SPDM_CAPABILITIES: u8 = 0x61;
const SPDM_ALGORITHMS: u8 = 0x63;
const SPDM_DIGESTS: u8 = 0x01;
const SPDM_CERTIFICATE: u8 = 0x02;
const SPDM_CHALLENGE_AUTH: u8 = 0x03;
const SPDM_MEASUREMENTS: u8 = 0x60;
const SPDM_ERROR: u8 = 0x7F;

const SPDM_MESSAGE_VERSION_10: u8 = 0x10;

const SPDM_ERROR_CODE_INVALID_REQUEST: u8 = 0x01;
const SPDM_ERROR_CODE_UNEXPECTED_REQUEST: u8 = 0x04;
const SPDM_ERROR_CODE_UNSUPPORTED_REQUEST: u8 = 0x07;
const SPDM_ERROR_CODE_VERSION_MISMATCH: u8 = 0x41;

/// GET_MEASUREMENTS request attribute: generate a signature.
const SPDM_GET_MEASUREMENTS_ATTRIBUTES_GENERATE_SIGNATURE: u8 = 0x01;
/// GET_MEASUREMENTS measurement operation: total number of measurements.
const SPDM_MEASUREMENT_OPERATION_TOTAL_NUMBER: u8 = 0x00;
/// GET_MEASUREMENTS measurement operation: all measurements.
const SPDM_MEASUREMENT_OPERATION_ALL: u8 = 0xFF;
/// GET_MEASUREMENTS trailing SlotIdParam byte mask.
const SPDM_GET_MEASUREMENTS_SLOT_ID_MASK: u8 = 0x0F;

/// Responder capability flags we advertise (SPDM 1.1): certificate, challenge
/// and signed-measurement support.
const SPDM_CAPABILITY_FLAGS: u32 = 0x0000_0016; // CERT_CAP | CHAL_CAP | MEAS_CAP_SIG

// Requester capability flag bits we validate in GET_CAPABILITIES (SPDM 1.1+).
const SPDM_REQUEST_FLAGS_ENCRYPT_CAP: u32 = 0x0000_0040;
const SPDM_REQUEST_FLAGS_MAC_CAP: u32 = 0x0000_0080;
const SPDM_REQUEST_FLAGS_MUT_AUTH_CAP: u32 = 0x0000_0100;
const SPDM_REQUEST_FLAGS_KEY_EX_CAP: u32 = 0x0000_0200;
const SPDM_REQUEST_FLAGS_PSK_CAP: u32 = 0x0000_0400;
const SPDM_REQUEST_FLAGS_ENCAP_CAP: u32 = 0x0000_1000;
const SPDM_REQUEST_FLAGS_CHUNK_CAP: u32 = 0x0002_0000;
/// Minimum DataTransferSize a SPDM 1.2+ requester may declare.
const SPDM_MIN_DATA_TRANSFER_SIZE_VERSION_12: u32 = 42;

// Negotiated algorithms: SHA-384 hashing, ECDSA P-384 signing, DMTF
// measurement specification.
const SPDM_MEASUREMENT_SPECIFICATION_DMTF: u8 = 0x01;
const SPDM_ALGORITHMS_MEASUREMENT_HASH_ALGO_TPM_ALG_SHA_384: u32 = 0x0000_0004;
const SPDM_ALGORITHMS_BASE_ASYM_ALGO_TPM_ALG_ECDSA_ECC_NIST_P384: u32 = 0x0000_0080;
const SPDM_ALGORITHMS_BASE_HASH_ALGO_TPM_ALG_SHA_384: u32 = 0x0000_0002;

const SPDM_NONCE_SIZE: usize = 32;
const SHA384_SIZE: usize = 48;
const ECDSA_P384_SIGNATURE_SIZE: usize = 96;

// ---------------------------------------------------------------------------
// Server
// ---------------------------------------------------------------------------

/// Bind and serve the SPDM over TCP endpoint for one system. Each connection
/// gets its own session (negotiation state and transcripts), so a stalled or
/// half-open client cannot block other connections.
pub async fn serve(listener: TcpListener, responder: SpdmResponder) {
    loop {
        match listener.accept().await {
            Ok((stream, peer)) => {
                tracing::info!(%peer, "SPDM over TCP connection opened");
                let session = SpdmTcpSession::new(responder.clone());
                tokio::spawn(async move {
                    if let Err(e) = handle_connection(stream, session).await {
                        tracing::warn!(%peer, error = %e, "SPDM over TCP connection ended with error");
                    } else {
                        tracing::info!(%peer, "SPDM over TCP connection closed");
                    }
                });
            }
            Err(e) => {
                tracing::warn!(error = %e, "SPDM over TCP accept failure");
            }
        }
    }
}

/// Bind the SPDM TCP listeners for every system configured with
/// `spdm_tcp_listen`. Called from the server startup path; each listener runs
/// as its own task.
pub fn spawn_listeners(state: &Arc<crate::app_state::AppState>) {
    for (system_id, system_config) in &state.config.systems {
        let Some(spdm_config) = &system_config.spdm else {
            continue;
        };
        let Some(listen) = &spdm_config.spdm_tcp_listen else {
            continue;
        };
        let Some(responder) = state.spdm_responders.get(system_id) else {
            tracing::warn!(system = %system_id, "spdm_tcp_listen set but no SPDM responder identity could be loaded");
            continue;
        };
        let listen = listen.clone();
        let responder = responder.clone();
        let system_id = system_id.clone();
        tokio::spawn(async move {
            match TcpListener::bind(&listen).await {
                Ok(listener) => {
                    tracing::info!(system = %system_id, %listen, "SPDM over TCP responder listening");
                    serve(listener, responder).await;
                }
                Err(e) => {
                    tracing::error!(system = %system_id, %listen, error = %e, "cannot bind SPDM over TCP listener");
                }
            }
        });
    }
}

/// Drive one connection through the spdm-emu platform socket protocol.
async fn handle_connection(mut stream: TcpStream, session: SpdmTcpSession) -> Result<(), String> {
    let session = Arc::new(Mutex::new(session));
    let (mut reader, mut writer) = stream.split();

    loop {
        // Frame header: command(4 BE) transport_type(4 BE) payload_size(4 BE)
        let mut header = [0u8; 12];
        reader
            .read_exact(&mut header)
            .await
            .map_err(|e| format!("read frame header: {e}"))?;
        let command = u32::from_be_bytes([header[0], header[1], header[2], header[3]]);
        let transport_type = u32::from_be_bytes([header[4], header[5], header[6], header[7]]);
        let payload_size =
            u32::from_be_bytes([header[8], header[9], header[10], header[11]]) as usize;

        if transport_type != SOCKET_TRANSPORT_TYPE_TCP {
            return Err(format!(
                "unexpected transport type 0x{transport_type:02x} (expected TCP)"
            ));
        }
        if payload_size > 64 * 1024 {
            return Err(format!("frame payload too large: {payload_size}"));
        }
        let mut payload = vec![0u8; payload_size];
        if payload_size > 0 {
            reader
                .read_exact(&mut payload)
                .await
                .map_err(|e| format!("read frame payload: {e}"))?;
        }

        match command {
            SOCKET_SPDM_COMMAND_TEST => {
                // Handshake: echo the client greeting, spdm_responder_emu-style
                write_frame(&mut writer, SOCKET_SPDM_COMMAND_TEST, b"Server Hello!\0").await?;
            }
            SOCKET_SPDM_COMMAND_SHUTDOWN | SOCKET_SPDM_COMMAND_NORMAL if payload.is_empty() => {
                return Ok(());
            }
            SOCKET_SPDM_COMMAND_NORMAL => {
                let response = {
                    let mut session = session.lock().await;
                    session.handle_transport_payload(&payload)?
                };
                write_frame(&mut writer, SOCKET_SPDM_COMMAND_NORMAL, &response).await?;
            }
            other => {
                return Err(format!("unsupported platform command 0x{other:08x}"));
            }
        }
    }
}

async fn write_frame(
    writer: &mut tokio::net::tcp::WriteHalf<'_>,
    command: u32,
    payload: &[u8],
) -> Result<(), String> {
    let mut frame = Vec::with_capacity(12 + payload.len());
    frame.extend_from_slice(&command.to_be_bytes());
    frame.extend_from_slice(&SOCKET_TRANSPORT_TYPE_TCP.to_be_bytes());
    frame.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    frame.extend_from_slice(payload);
    writer
        .write_all(&frame)
        .await
        .map_err(|e| format!("write frame: {e}"))
}

// ---------------------------------------------------------------------------
// Session: DSP0287 framing + SPDM dispatch
// ---------------------------------------------------------------------------

/// Per-connection SPDM responder state: the negotiated version, the message
/// transcripts the signed responses cover, and the first VCA requests (to
/// detect non-identical re-sends, which DSP0274 requires rejecting).
pub struct SpdmTcpSession {
    responder: SpdmResponder,
    /// Negotiated SPDM version byte, set by GET_VERSION.
    negotiated: Option<u8>,
    /// Message A: the VCA (version/capabilities/algorithms) requests and
    /// responses, as exchanged on this connection.
    transcript_a: Vec<u8>,
    /// Message B: DIGESTS and CERTIFICATE exchanges since the last challenge.
    transcript_b: Vec<u8>,
    /// Message C: the CHALLENGE request and CHALLENGE_AUTH response.
    transcript_c: Vec<u8>,
    /// Message M: GET_MEASUREMENTS requests and MEASUREMENTS responses since
    /// the last signed (and hence verified) measurement response.
    transcript_m: Vec<u8>,
    /// The first GET_CAPABILITIES request, for the non-identical re-send check.
    capabilities_request: Option<Vec<u8>>,
    /// The first NEGOTIATE_ALGORITHMS request, for the non-identical re-send check.
    algorithms_request: Option<Vec<u8>>,
    /// Cached DSP0274-format certificate chain.
    cert_chain: Vec<u8>,
}

impl SpdmTcpSession {
    pub fn new(responder: SpdmResponder) -> Self {
        let cert_chain = responder.cert_chain_dsp0274();
        Self {
            responder,
            negotiated: None,
            transcript_a: Vec::new(),
            transcript_b: Vec::new(),
            transcript_c: Vec::new(),
            transcript_m: Vec::new(),
            capabilities_request: None,
            algorithms_request: None,
            cert_chain,
        }
    }

    /// Handle one DSP0287 transport payload, returning the response payload.
    pub fn handle_transport_payload(&mut self, payload: &[u8]) -> Result<Vec<u8>, String> {
        let spdm_message = decode_tcp_binding(payload)?;
        let response = self.dispatch(spdm_message);
        Ok(encode_tcp_binding(&response))
    }

    /// Dispatch one SPDM request message, returning the SPDM response message.
    fn dispatch(&mut self, request: &[u8]) -> Vec<u8> {
        if request.len() < 4 {
            return error_response(SPDM_MESSAGE_VERSION_10, SPDM_ERROR_CODE_INVALID_REQUEST);
        }
        let version = request[0];
        let code = request[1];

        // GET_VERSION is the only message that uses the 1.0 header and is
        // valid before negotiation; it restarts the VCA.
        if code == SPDM_GET_VERSION {
            return self.handle_get_version(request);
        }
        let Some(negotiated) = self.negotiated else {
            // The requester selects a version from our advertised list; a
            // version we never advertised is a version mismatch.
            if !self.responder.supports_version(version) {
                return error_response(SPDM_MESSAGE_VERSION_10, SPDM_ERROR_CODE_VERSION_MISMATCH);
            }
            self.negotiated = Some(version);
            return self.dispatch_negotiated(request, version, code);
        };
        if version != negotiated {
            // Version mismatches are reported as ERROR responses. Per the
            // DMTF reference behavior, GET_CAPABILITIES mismatches carry the
            // 1.0 header; every other message carries the negotiated version.
            let error_version = if code == SPDM_GET_CAPABILITIES {
                SPDM_MESSAGE_VERSION_10
            } else {
                negotiated
            };
            return error_response(error_version, SPDM_ERROR_CODE_VERSION_MISMATCH);
        }

        self.dispatch_negotiated(request, version, code)
    }

    fn dispatch_negotiated(&mut self, request: &[u8], version: u8, code: u8) -> Vec<u8> {
        match code {
            SPDM_GET_CAPABILITIES => self.handle_get_capabilities(request),
            SPDM_NEGOTIATE_ALGORITHMS => self.handle_negotiate_algorithms(request),
            SPDM_GET_DIGESTS => self.handle_get_digests(request),
            SPDM_GET_CERTIFICATE => self.handle_get_certificate(request),
            SPDM_CHALLENGE => self.handle_challenge(request),
            SPDM_GET_MEASUREMENTS => self.handle_get_measurements(request),
            _ => error_response(version, SPDM_ERROR_CODE_UNSUPPORTED_REQUEST),
        }
    }

    /// GET_VERSION: respond with the versions this responder supports. This
    /// restarts the VCA (a fresh GET_VERSION means a fresh negotiation).
    fn handle_get_version(&mut self, request: &[u8]) -> Vec<u8> {
        self.negotiated = None;
        self.transcript_a.clear();
        self.transcript_b.clear();
        self.transcript_c.clear();
        self.transcript_m.clear();
        self.capabilities_request = None;
        self.algorithms_request = None;

        let entries: Vec<u8> = self
            .responder
            .advertised_versions()
            .iter()
            .map(|v| v.byte())
            .collect();

        let mut response = Vec::with_capacity(6 + entries.len() * 2);
        response.extend_from_slice(&[
            SPDM_MESSAGE_VERSION_10,
            SPDM_VERSION,
            0x00,
            0x00,
            0x00,
            entries.len() as u8,
        ]);
        for entry in entries {
            // bit[15:12] major, bit[11:8] minor
            response.extend_from_slice(&((entry as u16) << 8).to_le_bytes());
        }

        self.transcript_a.extend_from_slice(request);
        self.transcript_a.extend_from_slice(&response);
        response
    }

    /// GET_CAPABILITIES (1.1 layout; 1.2 appends data-transfer sizes). The
    /// requester's capability flags are validated per DSP0274 and remembered
    /// for the later ReqAsym algorithm selection.
    fn handle_get_capabilities(&mut self, request: &[u8]) -> Vec<u8> {
        if let Some(first) = &self.capabilities_request {
            if first.as_slice() != request {
                // Non-identical re-send of a negotiated message
                return error_response(request[0], SPDM_ERROR_CODE_UNEXPECTED_REQUEST);
            }
            return self.build_capabilities(request[0]);
        }
        if let Err(response) = self.validate_capabilities_request(request) {
            return response;
        }
        self.capabilities_request = Some(request.to_vec());
        let response = self.build_capabilities(request[0]);
        self.transcript_a.extend_from_slice(request);
        self.transcript_a.extend_from_slice(&response);
        response
    }

    /// Validate a GET_CAPABILITIES request per DSP0274. Returns the ERROR
    /// response on violation.
    fn validate_capabilities_request(&self, request: &[u8]) -> Result<(), Vec<u8>> {
        let version = request[0];
        let invalid = || error_response(version, SPDM_ERROR_CODE_INVALID_REQUEST);

        if version >= 0x11 {
            if request.len() < 12 {
                return Err(invalid());
            }
            let flags = u32::from_le_bytes([request[8], request[9], request[10], request[11]]);
            let session_caps =
                flags & (SPDM_REQUEST_FLAGS_KEY_EX_CAP | SPDM_REQUEST_FLAGS_PSK_CAP) != 0;
            let crypto_caps =
                flags & (SPDM_REQUEST_FLAGS_ENCRYPT_CAP | SPDM_REQUEST_FLAGS_MAC_CAP) != 0;
            // Session capabilities require ENCRYPT or MAC, and vice versa
            if session_caps != crypto_caps {
                return Err(invalid());
            }
            // Mutual authentication requires encapsulation
            if flags & SPDM_REQUEST_FLAGS_MUT_AUTH_CAP != 0
                && flags & SPDM_REQUEST_FLAGS_ENCAP_CAP == 0
            {
                return Err(invalid());
            }
        }
        if version >= 0x12 {
            if request.len() < 20 {
                return Err(invalid());
            }
            let data_transfer_size =
                u32::from_le_bytes([request[12], request[13], request[14], request[15]]);
            let max_spdm_msg_size =
                u32::from_le_bytes([request[16], request[17], request[18], request[19]]);
            let flags = u32::from_le_bytes([request[8], request[9], request[10], request[11]]);
            if data_transfer_size < SPDM_MIN_DATA_TRANSFER_SIZE_VERSION_12
                || data_transfer_size > max_spdm_msg_size
                || (flags & SPDM_REQUEST_FLAGS_CHUNK_CAP == 0
                    && data_transfer_size != max_spdm_msg_size)
            {
                return Err(invalid());
            }
        }
        Ok(())
    }

    fn build_capabilities(&self, version: u8) -> Vec<u8> {
        let mut response = Vec::with_capacity(20);
        response.extend_from_slice(&[version, SPDM_CAPABILITIES, 0x00, 0x00]);
        response.push(0x00); // reserved
        response.push(0x00); // ctExponent
        response.extend_from_slice(&0u16.to_le_bytes()); // ext flags
        response.extend_from_slice(&SPDM_CAPABILITY_FLAGS.to_le_bytes());
        if version >= 0x12 {
            response.extend_from_slice(&0xFFFFu32.to_le_bytes()); // data transfer size
            response.extend_from_slice(&0xFFFFu32.to_le_bytes()); // max SPDM message size
        }
        response
    }

    /// NEGOTIATE_ALGORITHMS: select SHA-384 + ECDSA P-384 (our only suite).
    /// No requester asymmetric algorithm is selected (the responder does not
    /// advertise MUT_AUTH_CAP) and no key schedule either (no session
    /// support), matching what DSP0274 expects of this capability set.
    fn handle_negotiate_algorithms(&mut self, request: &[u8]) -> Vec<u8> {
        if let Some(first) = &self.algorithms_request {
            if first.as_slice() != request {
                return error_response(request[0], SPDM_ERROR_CODE_UNEXPECTED_REQUEST);
            }
            return self.build_algorithms(request[0]);
        }
        if let Err(response) = self.validate_algorithms_request(request) {
            return response;
        }
        self.algorithms_request = Some(request.to_vec());
        let response = self.build_algorithms(request[0]);
        self.transcript_a.extend_from_slice(request);
        self.transcript_a.extend_from_slice(&response);
        response
    }

    /// Validate a NEGOTIATE_ALGORITHMS request per DSP0274: the length field
    /// must match the message size, no extended algorithm counts may be
    /// declared, and every structure table must use the fixed alg-count
    /// marker. Returns the ERROR response on violation.
    fn validate_algorithms_request(&self, request: &[u8]) -> Result<(), Vec<u8>> {
        let invalid = || error_response(request[0], SPDM_ERROR_CODE_INVALID_REQUEST);
        if request.len() < 32 {
            return Err(invalid());
        }
        let length = u16::from_le_bytes([request[4], request[5]]) as usize;
        if length != request.len() {
            return Err(invalid());
        }
        let ext_asym_count = request[28];
        let ext_hash_count = request[29];
        if ext_asym_count != 0 || ext_hash_count != 0 {
            return Err(invalid());
        }
        let table_count = request[2] as usize;
        // The message must be exactly the fixed part plus the declared
        // structure tables: trailing data is an invalid request.
        if request.len() != 32 + 4 * table_count {
            return Err(invalid());
        }
        let mut offset = 32;
        for _ in 0..table_count {
            if request[offset + 1] != 0x20 {
                // alg_count: fixed 2-byte algorithms, no extended count
                return Err(invalid());
            }
            offset += 4;
        }
        Ok(())
    }

    fn build_algorithms(&self, version: u8) -> Vec<u8> {
        // Structure tables: DHE, AEAD, ReqAsym, KeySchedule. We support no
        // sessions, so DHE, AEAD and the key schedule select nothing; the
        // ReqAsym selection mirrors what the requester offered intersected
        // with RSASSA-2048, the requester algorithm a CEC173x-class
        // responder accepts for mutual auth.
        let struct_tables = [
            (2u8, 0x0000u16), // DHE: none
            (3, 0x0000),      // AEAD: none
            (4, 0x0000),      // ReqAsym: none (no MUT_AUTH)
            (5, 0x0000),      // key schedule: none (no sessions)
        ];

        let mut response = Vec::with_capacity(52);
        response.extend_from_slice(&[version, SPDM_ALGORITHMS, struct_tables.len() as u8, 0x00]);
        response.extend_from_slice(&(52u16).to_le_bytes()); // length, incl. header
        response.push(SPDM_MEASUREMENT_SPECIFICATION_DMTF);
        response.push(0x00); // other params selection (opaque format: none)
        response.extend_from_slice(
            &SPDM_ALGORITHMS_MEASUREMENT_HASH_ALGO_TPM_ALG_SHA_384.to_le_bytes(),
        );
        response.extend_from_slice(
            &SPDM_ALGORITHMS_BASE_ASYM_ALGO_TPM_ALG_ECDSA_ECC_NIST_P384.to_le_bytes(),
        );
        response.extend_from_slice(&SPDM_ALGORITHMS_BASE_HASH_ALGO_TPM_ALG_SHA_384.to_le_bytes());
        response.extend_from_slice(&0u32.to_le_bytes()); // PQC asym (1.4)
        response.extend_from_slice(&[0u8; 7]); // reserved2
        response.push(0x00); // MEL specification (1.3)
        response.push(0x00); // ext asym count
        response.push(0x00); // ext hash count
        response.extend_from_slice(&0u16.to_le_bytes()); // reserved3
        for (alg_type, alg_supported) in struct_tables {
            response.push(alg_type);
            response.push(0x20); // fixed alg count marker
            response.extend_from_slice(&alg_supported.to_le_bytes());
        }
        response
    }

    /// GET_DIGESTS: SHA-384 of the DSP0274 certificate chain, slot 0 only.
    fn handle_get_digests(&mut self, request: &[u8]) -> Vec<u8> {
        let digest = sha2::Sha384::digest(&self.cert_chain);
        let mut response = Vec::with_capacity(5 + SHA384_SIZE);
        response.extend_from_slice(&[request[0], SPDM_DIGESTS, 0x00, 0x01]); // slot mask: 0x01
        response.extend_from_slice(&digest);
        self.transcript_b.extend_from_slice(request);
        self.transcript_b.extend_from_slice(&response);
        response
    }

    /// GET_CERTIFICATE: serve portions of the DSP0274 certificate chain.
    fn handle_get_certificate(&mut self, request: &[u8]) -> Vec<u8> {
        if request.len() < 8 {
            return error_response(request[0], SPDM_ERROR_CODE_INVALID_REQUEST);
        }
        let slot = request[2];
        if slot != 0 {
            return error_response(request[0], SPDM_ERROR_CODE_INVALID_REQUEST);
        }
        let offset = u16::from_le_bytes([request[4], request[5]]) as usize;
        let length = u16::from_le_bytes([request[6], request[7]]) as usize;
        let chain = &self.cert_chain;
        if offset > chain.len() {
            return error_response(request[0], SPDM_ERROR_CODE_INVALID_REQUEST);
        }
        let end = offset.saturating_add(length).min(chain.len());
        let portion = &chain[offset..end];
        let remainder = chain.len() - end;

        let mut response = Vec::with_capacity(8 + portion.len());
        response.extend_from_slice(&[request[0], SPDM_CERTIFICATE, slot, 0x00]);
        response.extend_from_slice(&(portion.len() as u16).to_le_bytes());
        response.extend_from_slice(&(remainder as u16).to_le_bytes());
        response.extend_from_slice(portion);
        self.transcript_b.extend_from_slice(request);
        self.transcript_b.extend_from_slice(&response);
        response
    }

    /// CHALLENGE: respond with CHALLENGE_AUTH signed over M1, the hash of the
    /// VCA, the DIGESTS/CERTIFICATE exchanges and the challenge itself.
    fn handle_challenge(&mut self, request: &[u8]) -> Vec<u8> {
        if request.len() < 4 + SPDM_NONCE_SIZE {
            return error_response(request[0], SPDM_ERROR_CODE_INVALID_REQUEST);
        }
        let slot = request[2];
        if slot != 0 {
            return error_response(request[0], SPDM_ERROR_CODE_INVALID_REQUEST);
        }
        let hash_type = request[3];
        // Only the defined measurement summary hash types are valid requests
        if !matches!(hash_type, 0x00 | 0x01 | 0xFF) {
            return error_response(request[0], SPDM_ERROR_CODE_INVALID_REQUEST);
        }
        // SPDM 1.3+: the request carries an 8-byte requester context after
        // the nonce, echoed in the response after the opaque data.
        let requester_context = if request[0] >= 0x13 {
            if request.len() < 4 + SPDM_NONCE_SIZE + REQUESTER_CONTEXT_SIZE {
                return error_response(request[0], SPDM_ERROR_CODE_INVALID_REQUEST);
            }
            Some(&request[4 + SPDM_NONCE_SIZE..4 + SPDM_NONCE_SIZE + REQUESTER_CONTEXT_SIZE])
        } else {
            None
        };

        // CHALLENGE_AUTH: header + cert chain hash + responder nonce +
        // (optional) measurement summary hash + opaque length + [context] +
        // signature.
        let cert_chain_hash = sha2::Sha384::digest(&self.cert_chain);
        let mut responder_nonce = [0u8; SPDM_NONCE_SIZE];
        OsRng.fill_bytes(&mut responder_nonce);
        let measurement_summary_hash = if hash_type == 0 {
            // SPDM_CHALLENGE_REQUEST_NO_MEASUREMENT_SUMMARY_HASH
            None
        } else {
            Some(sha2::Sha384::digest(self.measurement_record_all()))
        };

        let mut response = Vec::with_capacity(
            4 + SHA384_SIZE
                + SPDM_NONCE_SIZE
                + measurement_summary_hash.map_or(0, |h| h.len())
                + 2
                + REQUESTER_CONTEXT_SIZE
                + ECDSA_P384_SIGNATURE_SIZE,
        );
        response.extend_from_slice(&[request[0], SPDM_CHALLENGE_AUTH, slot, 0x01]); // slot mask: 0x01
        response.extend_from_slice(&cert_chain_hash);
        response.extend_from_slice(&responder_nonce);
        if let Some(summary_hash) = &measurement_summary_hash {
            response.extend_from_slice(summary_hash);
        }
        response.extend_from_slice(&0u16.to_le_bytes()); // opaque data length
        if let Some(context) = requester_context {
            response.extend_from_slice(context);
        }

        // M1 = HASH(message_a || message_b || message_c), where message_c is
        // the challenge request plus the response up to (excluding) the
        // signature.
        self.transcript_c.extend_from_slice(request);
        self.transcript_c.extend_from_slice(&response);
        let mut transcript = Vec::with_capacity(
            self.transcript_a.len() + self.transcript_b.len() + self.transcript_c.len(),
        );
        transcript.extend_from_slice(&self.transcript_a);
        transcript.extend_from_slice(&self.transcript_b);
        transcript.extend_from_slice(&self.transcript_c);
        let m1 = sha2::Sha384::digest(&transcript);
        tracing::debug!(
            m1 = %hex::encode(m1),
            transcript_len = transcript.len(),
            "signing SPDM challenge response"
        );

        match self
            .responder
            .sign_transcript_hash(request[0], SPDM_CHALLENGE_AUTH, &m1)
        {
            Ok(signature) => response.extend_from_slice(&signature),
            Err(_) => return error_response(request[0], SPDM_ERROR_CODE_UNEXPECTED_REQUEST),
        }

        // The requester resets messages B and C once it has verified M1, so a
        // subsequent challenge starts from a clean transcript.
        self.transcript_b.clear();
        self.transcript_c.clear();
        response
    }

    /// GET_MEASUREMENTS: serve the configured measurements, optionally signed
    /// (signature over L1/L2 = HASH(message_a || message_m)).
    fn handle_get_measurements(&mut self, request: &[u8]) -> Vec<u8> {
        if request.len() < 4 {
            return error_response(request[0], SPDM_ERROR_CODE_INVALID_REQUEST);
        }
        let attributes = request[2];
        let operation = request[3];
        let signed = attributes & SPDM_GET_MEASUREMENTS_ATTRIBUTES_GENERATE_SIGNATURE != 0;
        // Per DSP0274, a GET_MEASUREMENTS request without the
        // generate-signature attribute is just the 4-byte header; the nonce
        // and SlotIdParam are only sent when a signature is requested, plus
        // an 8-byte requester context from SPDM 1.3.
        let mut request_len = 4;
        let mut requester_context: Option<&[u8]> = None;
        if signed {
            request_len += SPDM_NONCE_SIZE + 1;
            if request[0] >= 0x13 {
                request_len += REQUESTER_CONTEXT_SIZE;
            }
            if request.len() < request_len {
                return error_response(request[0], SPDM_ERROR_CODE_INVALID_REQUEST);
            }
            if request[0] >= 0x13 {
                requester_context = Some(
                    &request
                        [4 + SPDM_NONCE_SIZE + 1..4 + SPDM_NONCE_SIZE + 1 + REQUESTER_CONTEXT_SIZE],
                );
            }
        }
        let slot = if signed {
            request[4 + SPDM_NONCE_SIZE] & SPDM_GET_MEASUREMENTS_SLOT_ID_MASK
        } else {
            0
        };
        // Slot 0 is the only provisioned slot; 0xF (any/provisional) is also
        // accepted, per DSP0274.
        if slot != 0 && slot != 0x0F {
            return error_response(request[0], SPDM_ERROR_CODE_INVALID_REQUEST);
        }

        let measurements = self.responder.measurement_values();
        let (param1, number_of_blocks, record): (u8, u8, Vec<u8>) = match operation {
            SPDM_MEASUREMENT_OPERATION_TOTAL_NUMBER => {
                // Total number of measurements: Param1 carries the count and
                // no blocks are returned.
                (measurements.len() as u8, 0, Vec::new())
            }
            SPDM_MEASUREMENT_OPERATION_ALL => {
                let record = self.measurement_record_all();
                (0, measurements.len() as u8, record)
            }
            index => {
                let Some(index) = index.checked_sub(1) else {
                    return error_response(request[0], SPDM_ERROR_CODE_INVALID_REQUEST);
                };
                let Some((_, record)) = self.measurement_record_index(index as usize) else {
                    return error_response(request[0], SPDM_ERROR_CODE_INVALID_REQUEST);
                };
                (0, 1, record)
            }
        };

        let record_length = record.len() as u32;
        let mut response = Vec::with_capacity(
            8 + record.len()
                + SPDM_NONCE_SIZE
                + 2
                + REQUESTER_CONTEXT_SIZE
                + ECDSA_P384_SIGNATURE_SIZE,
        );
        response.extend_from_slice(&[request[0], SPDM_MEASUREMENTS, param1, slot]);
        response.push(number_of_blocks);
        // MeasurementRecordLength: 3-byte uint24 little-endian (all versions)
        response.extend_from_slice(&record_length.to_le_bytes()[..3]);
        response.extend_from_slice(&record);

        // The responder nonce and OpaqueDataLength are part of the
        // MEASUREMENTS layout in every SPDM version, signed or not; only the
        // signature is conditional. The nonce is the responder's own random
        // value, never the requester's (DSP0274).
        let mut responder_nonce = [0u8; SPDM_NONCE_SIZE];
        OsRng.fill_bytes(&mut responder_nonce);
        response.extend_from_slice(&responder_nonce);
        response.extend_from_slice(&0u16.to_le_bytes()); // opaque data length
        if let Some(context) = requester_context {
            // SPDM 1.3+: echo the requester context before the signature
            response.extend_from_slice(context);
        }

        // The full response (minus any signature) joins message M.
        self.transcript_m.extend_from_slice(request);
        self.transcript_m.extend_from_slice(&response);

        if signed {
            // L1/L2 = HASH([VCA ||] message_m). Per libspdm (DMTF's reference
            // DSP0274 implementation), the VCA (message A) is part of the
            // L1/L2 preimage from SPDM 1.2 onwards; for 1.1 it covers only
            // the measurement messages themselves. message_m is the
            // measurement requests and responses up to (excluding) the
            // signature.
            let mut transcript =
                Vec::with_capacity(self.transcript_a.len() + self.transcript_m.len());
            if request[0] >= 0x12 {
                transcript.extend_from_slice(&self.transcript_a);
            }
            transcript.extend_from_slice(&self.transcript_m);
            let l1l2 = sha2::Sha384::digest(&transcript);
            tracing::debug!(
                l1l2 = %hex::encode(l1l2),
                transcript_len = transcript.len(),
                "signing SPDM measurements response"
            );

            match self
                .responder
                .sign_transcript_hash(request[0], SPDM_MEASUREMENTS, &l1l2)
            {
                Ok(signature) => response.extend_from_slice(&signature),
                Err(_) => return error_response(request[0], SPDM_ERROR_CODE_UNEXPECTED_REQUEST),
            }

            // The requester resets message M once it has verified the
            // signature, so the next signed measurement starts clean.
            self.transcript_m.clear();
        }

        response
    }

    /// The full measurement record: DSP0274 two-level blocks over all
    /// configured measurements.
    fn measurement_record_all(&self) -> Vec<u8> {
        let measurements = self.responder.measurement_values();
        let mut record = Vec::new();
        for (i, value) in measurements.iter().enumerate() {
            push_measurement_block(&mut record, (i + 1) as u8, value);
        }
        record
    }

    /// The measurement record for a single (0-based) measurement index.
    fn measurement_record_index(&self, index: usize) -> Option<(u8, Vec<u8>)> {
        let value = self.responder.measurement_values().get(index)?;
        let mut record = Vec::new();
        push_measurement_block(&mut record, (index + 1) as u8, value);
        Some(((index + 1) as u8, record))
    }
}

/// Append a DSP0274 two-level measurement block:
/// `Index(1) MeasurementSpecification(1) MeasurementSize(2)` wrapping the DMTF
/// measurement `DMTFSpecMeasurementValueType(1) ValueSize(2) Value`, with
/// `MeasurementSize == 3 + ValueSize`.
fn push_measurement_block(record: &mut Vec<u8>, index: u8, value: &[u8]) {
    let value_size = value.len() as u16;
    record.push(index);
    record.push(SPDM_MEASUREMENT_SPECIFICATION_DMTF);
    record.extend_from_slice(&(3 + value_size).to_le_bytes());
    record.push(0x02); // DMTFSpecMeasurementValueType: immutable ROM
    record.extend_from_slice(&value_size.to_le_bytes());
    record.extend_from_slice(value);
}

/// Build an SPDM ERROR response.
fn error_response(version: u8, code: u8) -> Vec<u8> {
    vec![version, SPDM_ERROR, code, 0x00]
}

/// Decode a DSP0287 SPDM-over-TCP binding payload, returning the SPDM message.
fn decode_tcp_binding(payload: &[u8]) -> Result<&[u8], String> {
    if payload.len() < SPDM_TCP_BINDING_HEADER_SIZE {
        return Err("transport payload shorter than the DSP0287 binding header".to_string());
    }
    let payload_length = u16::from_le_bytes([payload[0], payload[1]]) as usize;
    if payload[2] != SPDM_TCP_BINDING_VERSION {
        return Err(format!(
            "unsupported DSP0287 binding version 0x{:02x}",
            payload[2]
        ));
    }
    if payload[3] != SPDM_TCP_MESSAGE_TYPE_OUT_OF_SESSION {
        return Err(format!(
            "unsupported DSP0287 message type 0x{:02x} (only out-of-session SPDM is served)",
            payload[3]
        ));
    }
    let message = &payload[SPDM_TCP_BINDING_HEADER_SIZE..];
    if message.len() != payload_length {
        return Err(format!(
            "DSP0287 payload length {} does not match received {} bytes",
            payload_length,
            message.len()
        ));
    }
    Ok(message)
}

/// Encode an SPDM message in a DSP0287 SPDM-over-TCP binding payload.
fn encode_tcp_binding(message: &[u8]) -> Vec<u8> {
    let mut payload = Vec::with_capacity(SPDM_TCP_BINDING_HEADER_SIZE + message.len());
    payload.extend_from_slice(&(message.len() as u16).to_le_bytes());
    payload.push(SPDM_TCP_BINDING_VERSION);
    payload.push(SPDM_TCP_MESSAGE_TYPE_OUT_OF_SESSION);
    payload.extend_from_slice(message);
    payload
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attestation::spdm_responder::SpdmVersion;
    use p384::ecdsa::SigningKey;

    fn test_responder_with_chain(versions: Vec<SpdmVersion>) -> SpdmResponder {
        // A real EC P-384 certificate chain (root + leaf), packed in vbmc's
        // storage format, so the wire chain parses as X.509.
        let root_key = rcgen::KeyPair::generate_for(&rcgen::PKCS_ECDSA_P384_SHA384).unwrap();
        let mut root_params = rcgen::CertificateParams::new(vec![]).unwrap();
        let mut root_dn = rcgen::DistinguishedName::new();
        root_dn.push(rcgen::DnType::CommonName, "vbmc-test-root");
        root_params.distinguished_name = root_dn;
        root_params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        let root = root_params.self_signed(&root_key).unwrap();
        let root_issuer = rcgen::Issuer::from_params(&root_params, &root_key);

        let leaf_key = rcgen::KeyPair::generate_for(&rcgen::PKCS_ECDSA_P384_SHA384).unwrap();
        let mut leaf_params = rcgen::CertificateParams::new(vec![]).unwrap();
        let mut leaf_dn = rcgen::DistinguishedName::new();
        leaf_dn.push(rcgen::DnType::CommonName, "vbmc-test-leaf");
        leaf_params.distinguished_name = leaf_dn;
        let leaf = leaf_params.signed_by(&leaf_key, &root_issuer).unwrap();

        let mut blob = Vec::new();
        let ders = [root.der(), leaf.der()];
        let total: usize = 2 + ders.iter().map(|d| 2 + d.len()).sum::<usize>();
        blob.extend_from_slice(&(total as u16).to_le_bytes());
        for der in ders {
            blob.extend_from_slice(&(der.len() as u16).to_le_bytes());
            blob.extend_from_slice(der);
        }

        SpdmResponder {
            signing_key: {
                use p384::elliptic_curve::pkcs8::DecodePrivateKey;
                SigningKey::from_pkcs8_der(leaf_key.serialized_der()).unwrap()
            },
            cert_chain: blob,
            measurements: vec![vec![0xaa; 48], vec![0xbb; 48]],
            collection_delay_ms: 0,
            sign_transcript: false,
            versions,
        }
    }

    fn session() -> SpdmTcpSession {
        // The GB300-harness configuration: only SPDM 1.1 advertised
        SpdmTcpSession::new(test_responder_with_chain(vec![SpdmVersion::V11]))
    }

    fn exchange(session: &mut SpdmTcpSession, request: &[u8]) -> Vec<u8> {
        let payload = encode_tcp_binding(request);
        let response = session.handle_transport_payload(&payload).unwrap();
        decode_tcp_binding(&response).unwrap().to_vec()
    }

    fn run_vca(session: &mut SpdmTcpSession) {
        run_vca_at(session, 0x11)
    }

    fn run_vca_at(session: &mut SpdmTcpSession, version: u8) {
        let response = exchange(session, &[0x10, SPDM_GET_VERSION, 0x00, 0x00]);
        assert_eq!(response[1], SPDM_VERSION);
        let mut capabilities_request = vec![version, SPDM_GET_CAPABILITIES, 0x00, 0x00];
        capabilities_request.extend_from_slice(&[0u8; 4]); // reserved, ct, ext flags
        capabilities_request.extend_from_slice(&0x0000_0016u32.to_le_bytes()); // flags
        if version >= 0x12 {
            // data transfer size == max SPDM message size (no chunking)
            capabilities_request.extend_from_slice(&0xFFFFu32.to_le_bytes());
            capabilities_request.extend_from_slice(&0xFFFFu32.to_le_bytes());
        }
        let response = exchange(session, &capabilities_request);
        assert_eq!(response[1], SPDM_CAPABILITIES);
        let response = exchange(session, &negotiate_algorithms_request(version));
        assert_eq!(response[1], SPDM_ALGORITHMS);
    }

    fn negotiate_algorithms_request(version: u8) -> Vec<u8> {
        // A NEGOTIATE_ALGORITHMS request offering SHA-384 and ECDSA P-384
        // plus four structure tables (DHE, AEAD, ReqAsym, KeySchedule).
        let mut request = Vec::new();
        request.extend_from_slice(&[version, SPDM_NEGOTIATE_ALGORITHMS, 4, 0]);
        request.extend_from_slice(&(48u16).to_le_bytes()); // length (== message size)
        request.push(0x01); // measurement specification: DMTF
        request.push(0x00); // other params
        request.extend_from_slice(&0x0000_0080u32.to_le_bytes()); // asym: ECDSA P-384
        request.extend_from_slice(&0x0000_0002u32.to_le_bytes()); // hash: SHA-384
        request.extend_from_slice(&0u32.to_le_bytes()); // PQC asym
        request.extend_from_slice(&[0u8; 7]); // reserved2
        request.push(0x00); // MEL spec
        request.push(0x00); // ext asym count
        request.push(0x00); // ext hash count
        request.extend_from_slice(&0u16.to_le_bytes()); // reserved3
        for (alg_type, alg_supported) in [(2u8, 4u16), (3, 1), (4, 0), (5, 1)] {
            request.push(alg_type);
            request.push(0x20);
            request.extend_from_slice(&alg_supported.to_le_bytes());
        }
        request
    }

    fn get_measurements_request(attributes: u8, operation: u8, slot: u8) -> Vec<u8> {
        // Per DSP0274 the nonce and SlotIdParam are only present when a
        // signature is requested; unsigned requests are just the header.
        let mut request = Vec::with_capacity(37);
        request.extend_from_slice(&[0x11, SPDM_GET_MEASUREMENTS, attributes, operation]);
        if attributes & SPDM_GET_MEASUREMENTS_ATTRIBUTES_GENERATE_SIGNATURE != 0 {
            request.extend_from_slice(&[0x77u8; 32]); // requester nonce
            request.push(slot);
        }
        request
    }

    fn parse_measurements(response: &[u8]) -> (u8, usize, usize) {
        // Returns (param1, number_of_blocks, record_length)
        assert_eq!(response[1], SPDM_MEASUREMENTS);
        assert_eq!(response[0], 0x11);
        let record_length =
            (response[5] as usize) | ((response[6] as usize) << 8) | ((response[7] as usize) << 16);
        (response[2], response[4] as usize, record_length)
    }

    #[test]
    fn test_get_version_lists_supported_versions() {
        let mut session = session();
        let response = exchange(&mut session, &[0x10, SPDM_GET_VERSION, 0, 0]);
        // 1.0 header, VERSION code, the single configured entry (0x11)
        assert_eq!(&response[..6], &[0x10, SPDM_VERSION, 0, 0, 0, 1]);
        assert_eq!(&response[6..8], &0x1100u16.to_le_bytes());
    }

    #[test]
    fn test_version_negotiation_follows_the_requester() {
        // With every version advertised, the requester selects one and the
        // rest of the exchange runs at that version.
        let mut session = SpdmTcpSession::new(test_responder_with_chain(SpdmVersion::all()));
        let response = exchange(&mut session, &[0x10, SPDM_GET_VERSION, 0, 0]);
        // 1.1 through 1.4
        assert_eq!(response[5], 4);

        // The requester picks 1.2: GET_CAPABILITIES and everything after
        // runs at 0x12.
        let mut capabilities_request = vec![0x12, SPDM_GET_CAPABILITIES, 0x00, 0x00];
        capabilities_request.extend_from_slice(&[0u8; 4]); // reserved, ct, ext flags
        capabilities_request.extend_from_slice(&0x0000_0016u32.to_le_bytes()); // flags
        capabilities_request.extend_from_slice(&0xFFFFu32.to_le_bytes()); // data transfer size
        capabilities_request.extend_from_slice(&0xFFFFu32.to_le_bytes()); // max SPDM message size
        let response = exchange(&mut session, &capabilities_request);
        assert_eq!(response[0], 0x12);
        assert_eq!(response[1], SPDM_CAPABILITIES);
        // 1.2 CAPABILITIES carries the data-transfer size fields
        assert_eq!(response.len(), 20);

        // A version that was never advertised is a version mismatch; the
        // ERROR carries the negotiated version (0x12)
        let response = exchange(&mut session, &[0x11, SPDM_GET_DIGESTS, 0, 0]);
        assert_eq!(
            response,
            vec![0x12, SPDM_ERROR, SPDM_ERROR_CODE_VERSION_MISMATCH, 0]
        );
    }

    #[test]
    fn test_measurements_requester_context_at_13() {
        // SPDM 1.3+: the GET_MEASUREMENTS request carries an 8-byte requester
        // context which the response echoes before the signature.
        let mut session = SpdmTcpSession::new(test_responder_with_chain(vec![SpdmVersion::V13]));
        run_vca_at(&mut session, 0x13);

        let mut request = Vec::with_capacity(4 + SPDM_NONCE_SIZE + 1 + REQUESTER_CONTEXT_SIZE);
        request.extend_from_slice(&[
            0x13,
            SPDM_GET_MEASUREMENTS,
            SPDM_GET_MEASUREMENTS_ATTRIBUTES_GENERATE_SIGNATURE,
            SPDM_MEASUREMENT_OPERATION_ALL,
        ]);
        request.extend_from_slice(&[0x77u8; 32]);
        request.push(0x00); // slot
        request.extend_from_slice(&[0xABu8; REQUESTER_CONTEXT_SIZE]); // requester context
        let response = exchange(&mut session, &request);

        assert_eq!(response[0], 0x13);
        let record_len =
            (response[5] as usize) | ((response[6] as usize) << 8) | ((response[7] as usize) << 16);
        // ... + nonce(32) + opaque(2) + context(8) + signature(96)
        assert_eq!(
            response.len(),
            8 + record_len + SPDM_NONCE_SIZE + 2 + REQUESTER_CONTEXT_SIZE + 96
        );
        let context_start = 8 + record_len + SPDM_NONCE_SIZE + 2;
        assert_eq!(
            &response[context_start..context_start + REQUESTER_CONTEXT_SIZE],
            &[0xABu8; REQUESTER_CONTEXT_SIZE]
        );
    }

    #[test]
    fn test_version_mismatch_yields_error_at_negotiated_version() {
        let mut session = session();
        run_vca(&mut session);
        // GET_DIGESTS mismatches carry the negotiated version...
        let response = exchange(&mut session, &[0x12, SPDM_GET_DIGESTS, 0, 0]);
        assert_eq!(
            response,
            vec![0x11, SPDM_ERROR, SPDM_ERROR_CODE_VERSION_MISMATCH, 0]
        );
        // ...while GET_CAPABILITIES mismatches carry the 1.0 header
        let response = exchange(
            &mut session,
            &[0x12, SPDM_GET_CAPABILITIES, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        );
        assert_eq!(
            response,
            vec![0x10, SPDM_ERROR, SPDM_ERROR_CODE_VERSION_MISMATCH, 0]
        );
    }

    #[test]
    fn test_non_identical_capabilities_resend_is_unexpected_request() {
        let mut session = session();
        run_vca(&mut session);
        let mut request = vec![
            0x11,
            SPDM_GET_CAPABILITIES,
            0x00,
            0x00,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ];
        request[3] = 1; // differs from the first GET_CAPABILITIES
        let response = exchange(&mut session, &request);
        assert_eq!(
            response,
            vec![0x11, SPDM_ERROR, SPDM_ERROR_CODE_UNEXPECTED_REQUEST, 0]
        );
    }

    #[test]
    fn test_unsupported_request_yields_error() {
        let mut session = session();
        run_vca(&mut session);
        let response = exchange(&mut session, &[0x11, SPDM_KEY_EXCHANGE_REQUEST, 0, 0]);
        assert_eq!(
            response,
            vec![0x11, SPDM_ERROR, SPDM_ERROR_CODE_UNSUPPORTED_REQUEST, 0]
        );
    }

    const SPDM_KEY_EXCHANGE_REQUEST: u8 = 0xE4;

    #[test]
    fn test_digests_and_certificate_serve_the_chain() {
        let mut session = session();
        run_vca(&mut session);

        let response = exchange(&mut session, &[0x11, SPDM_GET_DIGESTS, 0, 0]);
        assert_eq!(response[1], SPDM_DIGESTS);
        assert_eq!(response[3], 0x01); // slot mask: slot 0
        assert_eq!(response.len(), 4 + SHA384_SIZE);
        let expected_digest = sha2::Sha384::digest(session.cert_chain.clone());
        assert_eq!(&response[4..], &expected_digest[..]);

        // Fetch the whole chain in one request (length 0xFFFF)
        let mut request = vec![0x11, SPDM_GET_CERTIFICATE, 0, 0];
        request.extend_from_slice(&0u16.to_le_bytes()); // offset
        request.extend_from_slice(&0xFFFFu16.to_le_bytes()); // length
        let response = exchange(&mut session, &request);
        assert_eq!(response[1], SPDM_CERTIFICATE);
        let portion_len = u16::from_le_bytes([response[4], response[5]]) as usize;
        let remainder = u16::from_le_bytes([response[6], response[7]]) as usize;
        assert_eq!(remainder, 0);
        assert_eq!(&response[8..8 + portion_len], &session.cert_chain[..]);
    }

    #[test]
    fn test_challenge_auth_signature_covers_the_real_transcript() {
        let mut session = session();
        run_vca(&mut session);

        // DIGESTS + CERTIFICATE are part of message B
        exchange(&mut session, &[0x11, SPDM_GET_DIGESTS, 0, 0]);
        let mut cert_request = vec![0x11, SPDM_GET_CERTIFICATE, 0, 0];
        cert_request.extend_from_slice(&0u16.to_le_bytes());
        cert_request.extend_from_slice(&0xFFFFu16.to_le_bytes());
        exchange(&mut session, &cert_request);

        let mut challenge = vec![0x11, SPDM_CHALLENGE, 0x00, 0xFF]; // slot 0, all-measurements hash
        challenge.extend_from_slice(&[0x55u8; 32]); // requester nonce
        // Snapshot the transcript state before the challenge: M1 is
        // a || b || challenge-request || CHALLENGE_AUTH (minus signature)
        let mut transcript = Vec::new();
        transcript.extend_from_slice(&session.transcript_a.clone());
        transcript.extend_from_slice(&session.transcript_b.clone());
        transcript.extend_from_slice(&challenge);
        let response = exchange(&mut session, &challenge);
        assert_eq!(response[1], SPDM_CHALLENGE_AUTH);
        assert_eq!(response[2], 0x00); // slot id
        assert_eq!(response[3], 0x01); // slot mask
        // cert chain hash matches the DIGESTS digest
        let expected_digest = sha2::Sha384::digest(session.cert_chain.clone());
        assert_eq!(&response[4..4 + SHA384_SIZE], &expected_digest[..]);

        // Layout: header(4) + chain hash(48) + nonce(32) + summary hash(48) +
        // opaque(2) + signature(96)
        assert_eq!(response.len(), 4 + 48 + 32 + 48 + 2 + 96);
        let signature = &response[4 + 48 + 32 + 48 + 2..];

        // Recompute M1 exactly as the requester would: the response (minus
        // signature) completes the transcript
        transcript.extend_from_slice(&response[..response.len() - 96]);
        let m1 = sha2::Sha384::digest(&transcript);
        assert!(
            session.responder.verify_digest(&m1, signature),
            "CHALLENGE_AUTH signature must verify over the real M1 transcript"
        );

        // The transcript B and C are reset after the challenge
        assert!(session.transcript_b.is_empty());
        assert!(session.transcript_c.is_empty());
    }

    #[test]
    fn test_measurements_total_number_and_all() {
        let mut session = session();
        run_vca(&mut session);

        // Total number: Param1 carries the count, no blocks
        let response = exchange(
            &mut session,
            &get_measurements_request(0, SPDM_MEASUREMENT_OPERATION_TOTAL_NUMBER, 0),
        );
        let (param1, blocks, record_length) = parse_measurements(&response);
        assert_eq!(param1, 2);
        assert_eq!(blocks, 0);
        assert_eq!(record_length, 0);
        // Unsigned: still nonce(32) + opaque(2), but no signature
        assert_eq!(response.len(), 8 + SPDM_NONCE_SIZE + 2);

        // All measurements, signed. At SPDM 1.1 the L1/L2 preimage covers
        // only the measurement messages (message M): the earlier unsigned
        // total-number exchange plus this request and response (minus
        // signature). The VCA joins L1/L2 from SPDM 1.2 onwards.
        let mut transcript = Vec::new();
        transcript.extend_from_slice(&session.transcript_m.clone());
        let request = get_measurements_request(
            SPDM_GET_MEASUREMENTS_ATTRIBUTES_GENERATE_SIGNATURE,
            SPDM_MEASUREMENT_OPERATION_ALL,
            0,
        );
        transcript.extend_from_slice(&request);
        let response = exchange(&mut session, &request);
        let (param1, blocks, record_length) = parse_measurements(&response);
        assert_eq!(param1, 0);
        assert_eq!(blocks, 2);
        // Two 48-byte values in two-level blocks: 2 * (4 + 3 + 48) = 110
        assert_eq!(record_length, 110);

        // Layout: header(8) + record + nonce(32) + opaque(2) + signature(96)
        assert_eq!(response.len(), 8 + 110 + 32 + 2 + 96);
        // The responder nonce must not echo the requester's
        assert_ne!(&response[8 + 110..8 + 110 + 32], &[0x77u8; 32]);

        // Signature verifies over L1/L2 (message M at SPDM 1.1)
        let signature = &response[8 + 110 + 32 + 2..];
        transcript.extend_from_slice(&response[..response.len() - 96]);
        let l1l2 = sha2::Sha384::digest(&transcript);
        assert!(
            session.responder.verify_digest(&l1l2, signature),
            "MEASUREMENTS signature must verify over the real L1/L2 transcript"
        );
        // message M was reset after the signed response
        assert!(session.transcript_m.is_empty());
    }

    #[test]
    fn test_measurements_single_index() {
        let mut session = session();
        run_vca(&mut session);

        let response = exchange(
            &mut session,
            &get_measurements_request(0, 1, 0), // measurement index 1
        );
        let (_, blocks, record_length) = parse_measurements(&response);
        assert_eq!(blocks, 1);
        // One 48-byte value in a two-level block: 4 + 3 + 48 = 55
        assert_eq!(record_length, 55);
        // The block index is 1 (the first measurement)
        assert_eq!(response[8], 1);

        // Out-of-range index is an invalid request
        let response = exchange(&mut session, &get_measurements_request(0, 3, 0));
        assert_eq!(
            response,
            vec![0x11, SPDM_ERROR, SPDM_ERROR_CODE_INVALID_REQUEST, 0]
        );
    }

    #[test]
    fn test_measurements_invalid_slot_is_invalid_request() {
        let mut session = session();
        run_vca(&mut session);
        // The slot is only carried on signed requests
        let response = exchange(
            &mut session,
            &get_measurements_request(
                SPDM_GET_MEASUREMENTS_ATTRIBUTES_GENERATE_SIGNATURE,
                SPDM_MEASUREMENT_OPERATION_ALL,
                1,
            ),
        );
        assert_eq!(
            response,
            vec![0x11, SPDM_ERROR, SPDM_ERROR_CODE_INVALID_REQUEST, 0]
        );
    }

    #[test]
    fn test_transport_binding_round_trip() {
        let message = [0x11u8, 0xE0, 0x01, 0xFF];
        let payload = encode_tcp_binding(&message);
        assert_eq!(&payload[..4], &[0x04, 0x00, 0x01, 0x05]);
        assert_eq!(decode_tcp_binding(&payload).unwrap(), &message);
        assert!(decode_tcp_binding(&payload[..3]).is_err());
        assert!(decode_tcp_binding(&[0x04, 0x00, 0x02, 0x05]).is_err()); // bad version
        assert!(decode_tcp_binding(&[0x05, 0x00, 0x01, 0x05]).is_err()); // length mismatch
    }
}
