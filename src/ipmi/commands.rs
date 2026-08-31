use crate::backend::types::VmPowerState;

const NETFN_CHASSIS: u8 = 0x00;
const NETFN_APP: u8 = 0x06;

const CMD_GET_CHASSIS_STATUS: u8 = 0x01;
const CMD_CHASSIS_CONTROL: u8 = 0x02;
const CMD_GET_SYSTEM_BOOT_OPTIONS: u8 = 0x09;
const CMD_SET_SYSTEM_BOOT_OPTIONS: u8 = 0x08;
const CMD_GET_DEVICE_ID: u8 = 0x01;
const CMD_GET_CHANNEL_AUTH_CAP: u8 = 0x38;
const CMD_GET_SESSION_INFO: u8 = 0x3D;
const CMD_SET_GLOBAL_ENABLES: u8 = 0x2E;
const CMD_GET_GLOBAL_ENABLES: u8 = 0x2F;

const CC_OK: u8 = 0x00;
const CC_INVALID_CMD: u8 = 0xC1;
const CC_INVALID_DATA: u8 = 0xCC;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChassisAction {
    PowerOff,
    PowerOn,
    PowerCycle,
    HardReset,
    Pulse,
    SoftShutdown,
}

pub struct IpmiRequest<'a> {
    pub netfn: u8,
    pub cmd: u8,
    pub data: &'a [u8],
}

#[derive(Debug)]
pub struct IpmiResponse {
    pub netfn: u8,
    pub cmd: u8,
    pub completion_code: u8,
    pub data: Vec<u8>,
}

impl IpmiResponse {
    fn ok(netfn: u8, cmd: u8, data: Vec<u8>) -> Self {
        Self {
            netfn: netfn | 0x01,
            cmd,
            completion_code: CC_OK,
            data,
        }
    }

    fn error(netfn: u8, cmd: u8, cc: u8) -> Self {
        Self {
            netfn: netfn | 0x01,
            cmd,
            completion_code: cc,
            data: vec![],
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(3 + self.data.len());
        out.push(self.netfn << 2);
        out.push(self.cmd);
        out.push(self.completion_code);
        out.extend_from_slice(&self.data);
        out
    }
}

pub fn parse_request(raw: &[u8]) -> Option<IpmiRequest<'_>> {
    if raw.len() < 2 {
        return None;
    }
    let netfn = raw[0] >> 2;
    let cmd = raw[1];
    let data = if raw.len() > 2 { &raw[2..] } else { &[] };
    Some(IpmiRequest { netfn, cmd, data })
}

#[derive(Debug)]
pub enum HandleResult {
    Response(IpmiResponse),
    ChassisAction(ChassisAction, IpmiResponse),
}

pub fn handle_request(
    req: &IpmiRequest<'_>,
    power_state: VmPowerState,
    boot_device: u8,
) -> HandleResult {
    let resp = match (req.netfn, req.cmd) {
        (NETFN_APP, CMD_GET_DEVICE_ID) => handle_get_device_id(),
        (NETFN_APP, CMD_GET_CHANNEL_AUTH_CAP) => handle_get_channel_auth_cap(),
        (NETFN_APP, CMD_GET_SESSION_INFO) => handle_get_session_info(),
        (NETFN_APP, CMD_SET_GLOBAL_ENABLES) => IpmiResponse::ok(req.netfn, req.cmd, vec![]),
        (NETFN_APP, CMD_GET_GLOBAL_ENABLES) => IpmiResponse::ok(req.netfn, req.cmd, vec![0x00]),
        (NETFN_CHASSIS, CMD_GET_CHASSIS_STATUS) => handle_get_chassis_status(power_state),
        (NETFN_CHASSIS, CMD_CHASSIS_CONTROL) => {
            return handle_chassis_control(req);
        }
        (NETFN_CHASSIS, CMD_GET_SYSTEM_BOOT_OPTIONS) => handle_get_boot_options(req, boot_device),
        (NETFN_CHASSIS, CMD_SET_SYSTEM_BOOT_OPTIONS) => handle_set_boot_options(req),
        _ => IpmiResponse::error(req.netfn, req.cmd, CC_INVALID_CMD),
    };
    HandleResult::Response(resp)
}

fn handle_get_device_id() -> IpmiResponse {
    IpmiResponse::ok(
        NETFN_APP,
        CMD_GET_DEVICE_ID,
        vec![
            0x20, // Device ID
            0x01, // Device revision
            0x02, // Firmware major
            0x00, // Firmware minor
            0x20, // IPMI version 2.0
            0xBF, // Additional device support
            0x00, 0x00, 0x00, // Manufacturer ID (generic)
            0x00, 0x00, // Product ID
        ],
    )
}

fn handle_get_channel_auth_cap() -> IpmiResponse {
    IpmiResponse::ok(
        NETFN_APP,
        CMD_GET_CHANNEL_AUTH_CAP,
        vec![
            0x00, // Channel number
            0x04, // Auth type: password
            0x00, // Authentication status
            0x00, // Extended capabilities
            0x00, 0x00, 0x00, 0x00, // OEM data
        ],
    )
}

fn handle_get_session_info() -> IpmiResponse {
    IpmiResponse::ok(NETFN_APP, CMD_GET_SESSION_INFO, vec![0x00, 0x00, 0x00])
}

fn handle_get_chassis_status(power_state: VmPowerState) -> IpmiResponse {
    let power_on = matches!(power_state, VmPowerState::On);
    let current_power_state = if power_on { 0x01 } else { 0x00 };
    IpmiResponse::ok(
        NETFN_CHASSIS,
        CMD_GET_CHASSIS_STATUS,
        vec![
            current_power_state, // Current power state
            0x00,                // Last power event
            0x40,                // Misc chassis state (chassis identified)
            0x00,                // Front panel button capabilities
        ],
    )
}

fn handle_chassis_control(req: &IpmiRequest<'_>) -> HandleResult {
    if req.data.is_empty() {
        return HandleResult::Response(IpmiResponse::error(req.netfn, req.cmd, CC_INVALID_DATA));
    }

    let action = match req.data[0] & 0x0F {
        0x00 => ChassisAction::PowerOff,
        0x01 => ChassisAction::PowerOn,
        0x02 => ChassisAction::PowerCycle,
        0x03 => ChassisAction::HardReset,
        0x04 => ChassisAction::Pulse,
        0x05 => ChassisAction::SoftShutdown,
        _ => {
            return HandleResult::Response(IpmiResponse::error(
                req.netfn,
                req.cmd,
                CC_INVALID_DATA,
            ));
        }
    };

    HandleResult::ChassisAction(action, IpmiResponse::ok(req.netfn, req.cmd, vec![]))
}

fn handle_get_boot_options(req: &IpmiRequest<'_>, boot_device: u8) -> IpmiResponse {
    let param = if !req.data.is_empty() {
        req.data[0] & 0x7F
    } else {
        0
    };

    match param {
        5 => IpmiResponse::ok(
            req.netfn,
            req.cmd,
            vec![
                0x01,        // Parameter version
                0x05,        // Parameter selector
                0x80,        // Parameter valid, boot flags valid
                boot_device, // Boot device (bits 5:2)
                0x00,        // Boot info ack
                0x00,        // Reserved
                0x00,        // Reserved
            ],
        ),
        _ => IpmiResponse::ok(req.netfn, req.cmd, vec![0x01, param, 0x00]),
    }
}

fn handle_set_boot_options(_req: &IpmiRequest<'_>) -> IpmiResponse {
    IpmiResponse::ok(NETFN_CHASSIS, CMD_SET_SYSTEM_BOOT_OPTIONS, vec![])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_request() {
        let raw = [0x18, 0x01]; // NetFn=App(6), Cmd=GetDeviceId(1)
        let req = parse_request(&raw).unwrap();
        assert_eq!(req.netfn, NETFN_APP);
        assert_eq!(req.cmd, CMD_GET_DEVICE_ID);
    }

    #[test]
    fn test_get_device_id() {
        let req = IpmiRequest {
            netfn: NETFN_APP,
            cmd: CMD_GET_DEVICE_ID,
            data: &[],
        };
        match handle_request(&req, VmPowerState::On, 0x00) {
            HandleResult::Response(resp) => {
                assert_eq!(resp.completion_code, CC_OK);
                assert!(!resp.data.is_empty());
            }
            _ => panic!("expected Response"),
        }
    }

    #[test]
    fn test_chassis_status_on() {
        let req = IpmiRequest {
            netfn: NETFN_CHASSIS,
            cmd: CMD_GET_CHASSIS_STATUS,
            data: &[],
        };
        match handle_request(&req, VmPowerState::On, 0x00) {
            HandleResult::Response(resp) => {
                assert_eq!(resp.completion_code, CC_OK);
                assert_eq!(resp.data[0] & 0x01, 0x01);
            }
            _ => panic!("expected Response"),
        }
    }

    #[test]
    fn test_chassis_status_off() {
        let req = IpmiRequest {
            netfn: NETFN_CHASSIS,
            cmd: CMD_GET_CHASSIS_STATUS,
            data: &[],
        };
        match handle_request(&req, VmPowerState::Off, 0x00) {
            HandleResult::Response(resp) => {
                assert_eq!(resp.data[0] & 0x01, 0x00);
            }
            _ => panic!("expected Response"),
        }
    }

    #[test]
    fn test_chassis_control_power_off() {
        let req = IpmiRequest {
            netfn: NETFN_CHASSIS,
            cmd: CMD_CHASSIS_CONTROL,
            data: &[0x00],
        };
        match handle_request(&req, VmPowerState::On, 0x00) {
            HandleResult::ChassisAction(action, resp) => {
                assert_eq!(action, ChassisAction::PowerOff);
                assert_eq!(resp.completion_code, CC_OK);
            }
            _ => panic!("expected ChassisAction"),
        }
    }

    #[test]
    fn test_chassis_control_power_on() {
        let req = IpmiRequest {
            netfn: NETFN_CHASSIS,
            cmd: CMD_CHASSIS_CONTROL,
            data: &[0x01],
        };
        match handle_request(&req, VmPowerState::Off, 0x00) {
            HandleResult::ChassisAction(action, _) => {
                assert_eq!(action, ChassisAction::PowerOn);
            }
            _ => panic!("expected ChassisAction"),
        }
    }

    #[test]
    fn test_unknown_command() {
        let req = IpmiRequest {
            netfn: 0x3F,
            cmd: 0xFF,
            data: &[],
        };
        match handle_request(&req, VmPowerState::On, 0x00) {
            HandleResult::Response(resp) => {
                assert_eq!(resp.completion_code, CC_INVALID_CMD);
            }
            _ => panic!("expected Response"),
        }
    }

    #[test]
    fn test_response_to_bytes() {
        let resp = IpmiResponse::ok(NETFN_APP, CMD_GET_DEVICE_ID, vec![0x20]);
        let bytes = resp.to_bytes();
        assert_eq!(bytes[0] >> 2, NETFN_APP | 0x01);
        assert_eq!(bytes[1], CMD_GET_DEVICE_ID);
        assert_eq!(bytes[2], CC_OK);
        assert_eq!(bytes[3], 0x20);
    }
}
