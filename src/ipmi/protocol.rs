const VM_MSG_CHAR: u8 = 0xA0;
const VM_CMD_CHAR: u8 = 0xA1;
const VM_ESCAPE_CHAR: u8 = 0xAA;

const VM_CMD_VERSION: u8 = 0xFF;
const VM_CMD_CAPABILITIES: u8 = 0x08;

const CAP_POWER: u8 = 0x01;
const CAP_RESET: u8 = 0x02;
const CAP_GRACEFUL_SHUTDOWN: u8 = 0x20;

#[derive(Debug)]
pub enum Frame {
    IpmiMessage { msg_id: u8, data: Vec<u8> },
    Command { cmd: u8, data: Vec<u8> },
}

pub struct FrameDecoder {
    buf: Vec<u8>,
    in_escape: bool,
}

impl FrameDecoder {
    pub fn new() -> Self {
        Self {
            buf: Vec::with_capacity(256),
            in_escape: false,
        }
    }

    pub fn feed(&mut self, byte: u8) -> Option<Frame> {
        if self.in_escape {
            self.in_escape = false;
            self.buf.push(byte & !0x10);
            return None;
        }

        match byte {
            VM_ESCAPE_CHAR => {
                self.in_escape = true;
                None
            }
            VM_MSG_CHAR => {
                let frame = self.decode_message();
                self.buf.clear();
                frame
            }
            VM_CMD_CHAR => {
                let frame = self.decode_command();
                self.buf.clear();
                frame
            }
            _ => {
                self.buf.push(byte);
                None
            }
        }
    }

    fn decode_message(&self) -> Option<Frame> {
        if self.buf.len() < 3 {
            return None;
        }
        if ipmb_checksum(&self.buf) != 0 {
            return None;
        }
        let msg_id = self.buf[0];
        let data = self.buf[1..self.buf.len() - 1].to_vec();
        Some(Frame::IpmiMessage { msg_id, data })
    }

    fn decode_command(&self) -> Option<Frame> {
        if self.buf.is_empty() {
            return None;
        }
        let cmd = self.buf[0];
        let data = self.buf[1..].to_vec();
        Some(Frame::Command { cmd, data })
    }
}

fn ipmb_checksum(data: &[u8]) -> u8 {
    let mut sum: u8 = 0;
    for &b in data {
        sum = sum.wrapping_add(b);
    }
    sum
}

fn escape_byte(byte: u8, out: &mut Vec<u8>) {
    if byte == VM_MSG_CHAR || byte == VM_CMD_CHAR || byte == VM_ESCAPE_CHAR {
        out.push(VM_ESCAPE_CHAR);
        out.push(byte | 0x10);
    } else {
        out.push(byte);
    }
}

pub fn encode_ipmi_response(msg_id: u8, response: &[u8]) -> Vec<u8> {
    let mut payload = Vec::with_capacity(response.len() + 2);
    payload.push(msg_id);
    payload.extend_from_slice(response);
    let checksum = (-(ipmb_checksum(&payload) as i8)) as u8;
    payload.push(checksum);

    let mut out = Vec::with_capacity(payload.len() * 2);
    for &b in &payload {
        escape_byte(b, &mut out);
    }
    out.push(VM_MSG_CHAR);
    out
}

pub fn encode_handshake_response() -> Vec<u8> {
    let mut out = Vec::new();

    // Version response
    escape_byte(VM_CMD_VERSION, &mut out);
    escape_byte(0x01, &mut out);
    out.push(VM_CMD_CHAR);

    // Capabilities
    escape_byte(VM_CMD_CAPABILITIES, &mut out);
    escape_byte(CAP_POWER | CAP_RESET | CAP_GRACEFUL_SHUTDOWN, &mut out);
    out.push(VM_CMD_CHAR);

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ipmb_checksum() {
        assert_eq!(ipmb_checksum(&[]), 0);
        assert_eq!(ipmb_checksum(&[0x01, 0x02]), 0x03);
    }

    #[test]
    fn test_encode_decode_message() {
        let response = &[0x20, 0x01, 0x00, 0x01];
        let encoded = encode_ipmi_response(0x42, response);

        let mut decoder = FrameDecoder::new();
        let mut frame = None;
        for &b in &encoded {
            if let Some(f) = decoder.feed(b) {
                frame = Some(f);
            }
        }

        match frame.unwrap() {
            Frame::IpmiMessage { msg_id, data } => {
                assert_eq!(msg_id, 0x42);
                assert_eq!(data, response);
            }
            _ => panic!("expected IpmiMessage"),
        }
    }

    #[test]
    fn test_escape_special_bytes() {
        let response = &[VM_MSG_CHAR, VM_CMD_CHAR, VM_ESCAPE_CHAR];
        let encoded = encode_ipmi_response(0x01, response);

        let mut decoder = FrameDecoder::new();
        let mut frame = None;
        for &b in &encoded {
            if let Some(f) = decoder.feed(b) {
                frame = Some(f);
            }
        }

        match frame.unwrap() {
            Frame::IpmiMessage { msg_id, data } => {
                assert_eq!(msg_id, 0x01);
                assert_eq!(data, response);
            }
            _ => panic!("expected IpmiMessage"),
        }
    }

    #[test]
    fn test_decode_command() {
        let encoded = vec![0x03, VM_CMD_CHAR]; // POWEROFF command
        let mut decoder = FrameDecoder::new();
        let mut frame = None;
        for &b in &encoded {
            if let Some(f) = decoder.feed(b) {
                frame = Some(f);
            }
        }

        match frame.unwrap() {
            Frame::Command { cmd, .. } => assert_eq!(cmd, 0x03),
            _ => panic!("expected Command"),
        }
    }

    #[test]
    fn test_handshake_response() {
        let bytes = encode_handshake_response();
        let mut decoder = FrameDecoder::new();
        let mut frames = Vec::new();
        for &b in &bytes {
            if let Some(f) = decoder.feed(b) {
                frames.push(f);
            }
        }
        assert_eq!(frames.len(), 2);
        match &frames[0] {
            Frame::Command { cmd, data } => {
                assert_eq!(*cmd, VM_CMD_VERSION);
                assert_eq!(data, &[0x01]);
            }
            _ => panic!("expected version command"),
        }
        match &frames[1] {
            Frame::Command { cmd, data } => {
                assert_eq!(*cmd, VM_CMD_CAPABILITIES);
                assert_eq!(data[0] & CAP_POWER, CAP_POWER);
            }
            _ => panic!("expected capabilities command"),
        }
    }
}
