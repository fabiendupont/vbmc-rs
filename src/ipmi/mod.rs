mod commands;
mod protocol;

use std::path::PathBuf;
use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixListener;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use crate::app_state::AppState;
use crate::backend::VmmBackend;
use crate::backend::types::VmPowerState;
use commands::{ChassisAction, HandleResult};

pub async fn start_ipmi_server(
    socket_path: PathBuf,
    system_id: String,
    app_state: Arc<AppState>,
    cancel: CancellationToken,
) {
    if socket_path.exists()
        && let Err(e) = std::fs::remove_file(&socket_path)
    {
        error!(path = %socket_path.display(), error = %e, "Failed to remove stale IPMI socket");
        return;
    }

    let listener = match UnixListener::bind(&socket_path) {
        Ok(l) => l,
        Err(e) => {
            error!(path = %socket_path.display(), error = %e, "Failed to bind IPMI socket");
            return;
        }
    };

    info!(
        path = %socket_path.display(),
        system = %system_id,
        "IPMI extern BMC listening"
    );

    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                info!(system = %system_id, "IPMI server shutting down");
                break;
            }
            result = listener.accept() => {
                match result {
                    Ok((stream, _)) => {
                        info!(system = %system_id, "QEMU connected to IPMI socket");
                        let state = app_state.clone();
                        let sid = system_id.clone();
                        let token = cancel.clone();
                        tokio::spawn(handle_connection(stream, sid, state, token));
                    }
                    Err(e) => {
                        error!(error = %e, "Failed to accept IPMI connection");
                    }
                }
            }
        }
    }

    let _ = std::fs::remove_file(&socket_path);
}

async fn handle_connection(
    stream: tokio::net::UnixStream,
    system_id: String,
    app_state: Arc<AppState>,
    cancel: CancellationToken,
) {
    let (mut reader, mut writer) = stream.into_split();

    let mut decoder = protocol::FrameDecoder::new();
    let mut buf = [0u8; 1];
    let mut boot_device: u8 = 0x00;

    loop {
        tokio::select! {
            _ = cancel.cancelled() => break,
            result = reader.read(&mut buf) => {
                match result {
                    Ok(0) => {
                        info!(system = %system_id, "QEMU disconnected from IPMI socket");
                        break;
                    }
                    Ok(_) => {
                        if let Some(frame) = decoder.feed(buf[0]) {
                            match frame {
                                protocol::Frame::IpmiMessage { msg_id, data } => {
                                    if let Some(response) = handle_ipmi_message(
                                        msg_id,
                                        &data,
                                        &system_id,
                                        &app_state,
                                        &mut boot_device,
                                    ).await
                                        && let Err(e) = writer.write_all(&response).await
                                    {
                                        error!(error = %e, "Failed to write IPMI response");
                                        break;
                                    }
                                }
                                protocol::Frame::Command { cmd, data } => {
                                    if let Some(response) =
                                        handle_qemu_command(cmd, &data, &system_id)
                                        && let Err(e) = writer.write_all(&response).await
                                    {
                                        error!(error = %e, "Failed to write command response");
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!(system = %system_id, error = %e, "IPMI read error");
                        break;
                    }
                }
            }
        }
    }
}

async fn handle_ipmi_message(
    msg_id: u8,
    data: &[u8],
    system_id: &str,
    app_state: &Arc<AppState>,
    boot_device: &mut u8,
) -> Option<Vec<u8>> {
    let req = commands::parse_request(data)?;
    debug!(
        system = %system_id,
        netfn = req.netfn,
        cmd = req.cmd,
        "IPMI request"
    );

    let power_state = app_state
        .backend
        .vm_info(system_id)
        .await
        .map(|info| info.power_state)
        .unwrap_or(VmPowerState::Unknown);

    let result = commands::handle_request(&req, power_state, *boot_device);

    match result {
        HandleResult::Response(resp) => {
            let bytes = resp.to_bytes();
            Some(protocol::encode_ipmi_response(msg_id, &bytes))
        }
        HandleResult::ChassisAction(action, resp) => {
            execute_chassis_action(action, system_id, app_state).await;
            let bytes = resp.to_bytes();
            Some(protocol::encode_ipmi_response(msg_id, &bytes))
        }
    }
}

async fn execute_chassis_action(action: ChassisAction, system_id: &str, app_state: &Arc<AppState>) {
    let result = match action {
        ChassisAction::PowerOff => {
            info!(system = %system_id, "IPMI: power off");
            app_state.backend.vm_shutdown(system_id).await
        }
        ChassisAction::PowerOn => {
            info!(system = %system_id, "IPMI: power on");
            app_state.backend.vm_boot(system_id).await
        }
        ChassisAction::PowerCycle => {
            info!(system = %system_id, "IPMI: power cycle");
            let _ = app_state.backend.vm_shutdown(system_id).await;
            app_state.backend.vm_boot(system_id).await
        }
        ChassisAction::HardReset => {
            info!(system = %system_id, "IPMI: hard reset");
            app_state.backend.vm_reboot(system_id).await
        }
        ChassisAction::Pulse | ChassisAction::SoftShutdown => {
            info!(system = %system_id, "IPMI: soft shutdown");
            app_state.backend.vm_power_button(system_id).await
        }
    };

    if let Err(e) = result {
        warn!(system = %system_id, error = %e, "IPMI chassis action failed");
    }
}

fn handle_qemu_command(cmd: u8, data: &[u8], system_id: &str) -> Option<Vec<u8>> {
    match cmd {
        0xFF => {
            let version = data.first().copied().unwrap_or(0);
            debug!(system = %system_id, version, "QEMU sent protocol version");
            None
        }
        0x08 => {
            let caps = data.first().copied().unwrap_or(0);
            debug!(system = %system_id, capabilities = caps, "QEMU sent capabilities");
            Some(protocol::encode_noattn())
        }
        other => {
            debug!(system = %system_id, cmd = other, "Unknown QEMU command");
            None
        }
    }
}
