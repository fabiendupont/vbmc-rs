use tokio::net::UdpSocket;
use tokio::sync::broadcast;
use tracing::{error, info, warn};

use super::RedfishEvent;
use crate::config::SnmpTrapConfig;

const SNMP_TRAP_OID: &[u32] = &[1, 3, 6, 1, 6, 3, 1, 1, 4, 1, 0];
const VBMC_ENTERPRISE_OID: &[u32] = &[1, 3, 6, 1, 4, 1, 99999];
const SYS_UPTIME_OID: &[u32] = &[1, 3, 6, 1, 2, 1, 1, 3, 0];

pub async fn snmp_trap_sender(mut rx: broadcast::Receiver<RedfishEvent>, config: SnmpTrapConfig) {
    info!(receiver = %config.receiver, "SNMP trap sender started");

    let socket = match UdpSocket::bind("0.0.0.0:0").await {
        Ok(s) => s,
        Err(e) => {
            error!("Failed to bind SNMP trap socket: {e}");
            return;
        }
    };

    loop {
        match rx.recv().await {
            Ok(event) => {
                let trap_oid = build_event_oid(&event.message_id);
                let packet = build_snmpv2c_trap(
                    config.community.as_bytes(),
                    &trap_oid,
                    &event.message_id,
                    &event.message,
                    &event.severity,
                );
                if let Err(e) = socket.send_to(&packet, &config.receiver).await {
                    warn!(receiver = %config.receiver, error = %e, "Failed to send SNMP trap");
                }
            }
            Err(broadcast::error::RecvError::Lagged(n)) => {
                warn!("SNMP trap sender missed {n} events");
            }
            Err(broadcast::error::RecvError::Closed) => {
                info!("Event bus closed, stopping SNMP trap sender");
                break;
            }
        }
    }
}

fn build_event_oid(message_id: &str) -> Vec<u32> {
    let hash = message_id.bytes().fold(0u32, |acc, b| {
        acc.wrapping_mul(31).wrapping_add(u32::from(b))
    });
    let mut oid = VBMC_ENTERPRISE_OID.to_vec();
    oid.push(hash % 10000);
    oid
}

fn build_snmpv2c_trap(
    community: &[u8],
    trap_oid: &[u32],
    message_id: &str,
    message: &str,
    severity: &str,
) -> Vec<u8> {
    let mut varbinds = Vec::new();

    // sysUpTime.0 = TimeTicks(0)
    append_varbind(&mut varbinds, SYS_UPTIME_OID, &encode_timeticks(0));
    // snmpTrapOID.0 = trap OID
    append_varbind(&mut varbinds, SNMP_TRAP_OID, &encode_oid_value(trap_oid));
    // messageId
    let msg_id_oid = &[1, 3, 6, 1, 4, 1, 99999, 1];
    append_varbind(
        &mut varbinds,
        msg_id_oid,
        &encode_octet_string(message_id.as_bytes()),
    );
    // message
    let msg_oid = &[1, 3, 6, 1, 4, 1, 99999, 2];
    append_varbind(
        &mut varbinds,
        msg_oid,
        &encode_octet_string(message.as_bytes()),
    );
    // severity
    let sev_oid = &[1, 3, 6, 1, 4, 1, 99999, 3];
    append_varbind(
        &mut varbinds,
        sev_oid,
        &encode_octet_string(severity.as_bytes()),
    );

    let varbind_seq = encode_sequence(&varbinds);

    // SNMPv2c trap PDU (implicit tag 0xA7)
    let mut pdu_content = Vec::new();
    pdu_content.extend_from_slice(&encode_integer(0)); // request-id
    pdu_content.extend_from_slice(&encode_integer(0)); // error-status
    pdu_content.extend_from_slice(&encode_integer(0)); // error-index
    pdu_content.extend_from_slice(&varbind_seq);
    let trap_pdu = encode_tagged(0xA7, &pdu_content);

    // SNMP message: version(1=v2c) + community + pdu
    let mut msg_content = Vec::new();
    msg_content.extend_from_slice(&encode_integer(1)); // version = SNMPv2c
    msg_content.extend_from_slice(&encode_octet_string(community));
    msg_content.extend_from_slice(&trap_pdu);

    encode_sequence(&msg_content)
}

fn append_varbind(buf: &mut Vec<u8>, oid: &[u32], encoded_value: &[u8]) {
    let mut vb = encode_oid(oid);
    vb.extend_from_slice(encoded_value);
    buf.extend_from_slice(&encode_sequence(&vb));
}

fn encode_sequence(content: &[u8]) -> Vec<u8> {
    encode_tagged(0x30, content)
}

fn encode_tagged(tag: u8, content: &[u8]) -> Vec<u8> {
    let mut buf = vec![tag];
    encode_length(&mut buf, content.len());
    buf.extend_from_slice(content);
    buf
}

fn encode_length(buf: &mut Vec<u8>, len: usize) {
    if len < 128 {
        buf.push(len as u8);
    } else if len <= 0xFF {
        buf.push(0x81);
        buf.push(len as u8);
    } else {
        buf.push(0x82);
        buf.push((len >> 8) as u8);
        buf.push(len as u8);
    }
}

fn encode_integer(value: i32) -> Vec<u8> {
    let bytes = value.to_be_bytes();
    let start = bytes.iter().position(|&b| b != 0).unwrap_or(3);
    let significant = &bytes[start..];
    let content = if significant.is_empty() {
        &[0u8] as &[u8]
    } else if significant[0] & 0x80 != 0 {
        let mut padded = vec![0u8];
        padded.extend_from_slice(significant);
        return encode_tagged(0x02, &padded);
    } else {
        significant
    };
    encode_tagged(0x02, content)
}

fn encode_octet_string(data: &[u8]) -> Vec<u8> {
    encode_tagged(0x04, data)
}

fn encode_timeticks(value: u32) -> Vec<u8> {
    let bytes = value.to_be_bytes();
    let start = bytes.iter().position(|&b| b != 0).unwrap_or(3);
    let significant = &bytes[start..];
    let content = if significant.is_empty() {
        &[0u8] as &[u8]
    } else {
        significant
    };
    encode_tagged(0x43, content)
}

fn encode_oid(oid: &[u32]) -> Vec<u8> {
    let mut content = Vec::new();
    if oid.len() >= 2 {
        content.push((oid[0] * 40 + oid[1]) as u8);
        for &component in &oid[2..] {
            encode_oid_component(&mut content, component);
        }
    }
    encode_tagged(0x06, &content)
}

fn encode_oid_value(oid: &[u32]) -> Vec<u8> {
    encode_oid(oid)
}

fn encode_oid_component(buf: &mut Vec<u8>, value: u32) {
    if value < 128 {
        buf.push(value as u8);
    } else {
        let mut parts = Vec::new();
        let mut v = value;
        parts.push((v & 0x7F) as u8);
        v >>= 7;
        while v > 0 {
            parts.push((v & 0x7F) as u8 | 0x80);
            v >>= 7;
        }
        parts.reverse();
        buf.extend_from_slice(&parts);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_snmp_trap_end_to_end() {
        let listener = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let (tx, rx) = broadcast::channel::<RedfishEvent>(16);

        let config = SnmpTrapConfig {
            enabled: true,
            receiver: addr.to_string(),
            community: "public".to_string(),
        };

        tokio::spawn(snmp_trap_sender(rx, config));

        let event = RedfishEvent {
            event_type: "StatusChange".to_string(),
            event_id: "test-1".to_string(),
            event_timestamp: chrono::Utc::now(),
            message_id: "PowerStateChanged".to_string(),
            message: "System powered on".to_string(),
            origin_of_condition: Some("/redfish/v1/Systems/vm1".to_string()),
            severity: "OK".to_string(),
            actor: None,
            payload: None,
        };

        tx.send(event).unwrap();

        let mut buf = [0u8; 4096];
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            listener.recv_from(&mut buf),
        )
        .await;

        let (len, _from) = result.expect("timeout receiving trap").unwrap();
        let packet = &buf[..len];

        // Verify SNMP SEQUENCE
        assert_eq!(packet[0], 0x30, "outer tag should be SEQUENCE");

        // Verify community string "public" is in the packet
        let packet_str = String::from_utf8_lossy(packet);
        assert!(
            packet_str.contains("public"),
            "packet should contain community string"
        );
        assert!(
            packet_str.contains("PowerStateChanged"),
            "packet should contain message_id"
        );
        assert!(
            packet_str.contains("System powered on"),
            "packet should contain message"
        );
    }

    #[test]
    fn test_encode_oid_simple() {
        let encoded = encode_oid(&[1, 3, 6, 1, 2, 1, 1, 3, 0]);
        assert_eq!(encoded[0], 0x06); // OID tag
        assert_eq!(encoded[2], 0x2B); // 1*40+3 = 43 = 0x2B
    }

    #[test]
    fn test_encode_oid_large_component() {
        let encoded = encode_oid(&[1, 3, 6, 1, 4, 1, 99999]);
        assert_eq!(encoded[0], 0x06);
        // 99999 requires multi-byte encoding
        assert!(encoded.len() > 8);
    }

    #[test]
    fn test_encode_integer_zero() {
        let encoded = encode_integer(0);
        assert_eq!(encoded, vec![0x02, 0x01, 0x00]);
    }

    #[test]
    fn test_encode_octet_string() {
        let encoded = encode_octet_string(b"test");
        assert_eq!(encoded[0], 0x04);
        assert_eq!(encoded[1], 4);
        assert_eq!(&encoded[2..], b"test");
    }

    #[test]
    fn test_build_snmpv2c_trap_parses() {
        let packet = build_snmpv2c_trap(
            b"public",
            &[1, 3, 6, 1, 4, 1, 99999, 1],
            "TestEvent",
            "Test message",
            "OK",
        );
        // Outer tag is SEQUENCE (0x30)
        assert_eq!(packet[0], 0x30);
        // Should be valid BER - at least 30 bytes
        assert!(packet.len() > 30);
    }

    #[test]
    fn test_build_event_oid_deterministic() {
        let oid1 = build_event_oid("PowerStateChanged");
        let oid2 = build_event_oid("PowerStateChanged");
        assert_eq!(oid1, oid2);
    }

    #[test]
    fn test_build_event_oid_different_messages() {
        let oid1 = build_event_oid("PowerStateChanged");
        let oid2 = build_event_oid("AttestationChanged");
        assert_ne!(oid1, oid2);
    }
}
