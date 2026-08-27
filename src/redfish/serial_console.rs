use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket};
use axum::extract::{Path, State, WebSocketUpgrade};
use axum::response::{IntoResponse, Response};
use futures_util::{SinkExt, StreamExt};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{info, warn};

use super::error::RedfishApiError;
use crate::app_state::AppState;
use crate::auth::AuthenticatedUser;
use crate::backend::VmmBackend;

pub async fn serial_console_ws(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
    Path(system_id): Path<String>,
    ws: WebSocketUpgrade,
) -> Result<Response, RedfishApiError> {
    if !state.config.systems.contains_key(&system_id) {
        return Err(RedfishApiError::NotFound(format!(
            "System '{system_id}' not found"
        )));
    }

    let console_info = state
        .backend
        .vm_serial_console(&system_id)
        .await
        .map_err(|e| RedfishApiError::ServiceUnavailable(e.to_string()))?;

    let pty_path = console_info.pty_path.ok_or_else(|| {
        RedfishApiError::ServiceUnavailable("no serial console available".to_string())
    })?;

    info!(system_id = %system_id, pty = %pty_path, "WebSocket serial console requested");

    Ok(ws
        .on_upgrade(move |socket| handle_console(socket, pty_path, system_id))
        .into_response())
}

async fn handle_console(socket: WebSocket, pty_path: String, system_id: String) {
    let pty = match tokio::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&pty_path)
        .await
    {
        Ok(f) => f,
        Err(e) => {
            warn!(system_id = %system_id, error = %e, "Failed to open PTY");
            return;
        }
    };

    let std_file = pty.into_std().await;
    let (pty_read, pty_write) = match (std_file.try_clone(), std_file) {
        (Ok(r), w) => (tokio::fs::File::from_std(r), tokio::fs::File::from_std(w)),
        (Err(e), _) => {
            warn!(system_id = %system_id, error = %e, "Failed to clone PTY fd");
            return;
        }
    };

    let (mut ws_sender, mut ws_receiver) = socket.split();

    let sys_id = system_id.clone();
    let pty_to_ws = tokio::spawn(async move {
        let mut reader = pty_read;
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => {
                    if ws_sender
                        .send(Message::Binary(buf[..n].to_vec().into()))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Err(e) => {
                    warn!(system_id = %sys_id, error = %e, "PTY read error");
                    break;
                }
            }
        }
    });

    let sys_id = system_id.clone();
    let ws_to_pty = tokio::spawn(async move {
        let mut writer = pty_write;
        while let Some(Ok(msg)) = ws_receiver.next().await {
            match msg {
                Message::Binary(data) => {
                    if writer.write_all(&data).await.is_err() {
                        break;
                    }
                }
                Message::Text(text) => {
                    if writer.write_all(text.as_bytes()).await.is_err() {
                        break;
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
        drop(writer);
        info!(system_id = %sys_id, "WebSocket serial console closed");
    });

    tokio::select! {
        _ = pty_to_ws => {}
        _ = ws_to_pty => {}
    }
}
