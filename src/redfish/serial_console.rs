use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket};
use axum::extract::{Path, State, WebSocketUpgrade};
use axum::response::{IntoResponse, Response};
use futures_util::{SinkExt, StreamExt};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
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

    if console_info.pty_path.is_none() && console_info.websocket_url.is_none() {
        return Err(RedfishApiError::ServiceUnavailable(
            "no serial console available".to_string(),
        ));
    }

    info!(system_id = %system_id, "WebSocket serial console requested");

    Ok(ws
        .on_upgrade(move |socket| handle_console(socket, console_info, system_id))
        .into_response())
}

async fn handle_console(
    socket: WebSocket,
    console_info: crate::backend::types::SerialConsoleInfo,
    system_id: String,
) {
    if let Some(pty_path) = console_info.pty_path {
        handle_pty_console(socket, pty_path, system_id).await;
    } else if let Some(ws_url) = console_info.websocket_url {
        handle_ws_proxy(socket, ws_url, system_id).await;
    }
}

async fn handle_pty_console(socket: WebSocket, path: String, system_id: String) {
    // Try Unix socket first (KubeVirt), then PTY file (standalone libvirt)
    if let Ok(stream) = tokio::net::UnixStream::connect(&path).await {
        info!(system_id = %system_id, path = %path, "Connected to serial console (unix socket)");
        let (reader, writer) = stream.into_split();
        bridge_ws_to_io(socket, reader, writer, system_id).await;
    } else if let Ok(file) = tokio::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .await
    {
        info!(system_id = %system_id, path = %path, "Connected to serial console (pty)");
        let std_file = file.into_std().await;
        match std_file.try_clone() {
            Ok(clone) => {
                let reader = tokio::fs::File::from_std(clone);
                let writer = tokio::fs::File::from_std(std_file);
                bridge_ws_to_io(socket, reader, writer, system_id).await;
            }
            Err(e) => {
                warn!(system_id = %system_id, error = %e, "Failed to clone PTY fd");
            }
        }
    } else {
        warn!(system_id = %system_id, path = %path, "Failed to connect to serial console");
    }
}

async fn bridge_ws_to_io<R, W>(socket: WebSocket, reader: R, writer: W, system_id: String)
where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    let (mut ws_sender, mut ws_receiver) = socket.split();

    let sys_id = system_id.clone();
    let io_to_ws = tokio::spawn(async move {
        let mut reader = tokio::io::BufReader::new(reader);
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
                    warn!(system_id = %sys_id, error = %e, "Console read error");
                    break;
                }
            }
        }
    });

    let sys_id = system_id.clone();
    let ws_to_io = tokio::spawn(async move {
        let mut writer = writer;
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
        info!(system_id = %sys_id, "WebSocket serial console closed");
    });

    tokio::select! {
        _ = io_to_ws => {}
        _ = ws_to_io => {}
    }
}

async fn handle_ws_proxy(socket: WebSocket, upstream_url: String, system_id: String) {
    let upstream = match tokio_tungstenite::connect_async(&upstream_url).await {
        Ok((ws, _)) => ws,
        Err(e) => {
            warn!(system_id = %system_id, url = %upstream_url, error = %e, "Failed to connect to upstream console");
            return;
        }
    };

    let (mut client_tx, mut client_rx) = socket.split();
    let (mut upstream_tx, mut upstream_rx) = upstream.split();

    let sys_id = system_id.clone();
    let upstream_to_client = tokio::spawn(async move {
        while let Some(Ok(msg)) = upstream_rx.next().await {
            let axum_msg = match msg {
                tokio_tungstenite::tungstenite::Message::Binary(data) => Message::Binary(data),
                tokio_tungstenite::tungstenite::Message::Text(text) => {
                    Message::Text(text.to_string().into())
                }
                tokio_tungstenite::tungstenite::Message::Close(_) => break,
                _ => continue,
            };
            if client_tx.send(axum_msg).await.is_err() {
                break;
            }
        }
    });

    let sys_id2 = sys_id.clone();
    let client_to_upstream = tokio::spawn(async move {
        while let Some(Ok(msg)) = client_rx.next().await {
            let tung_msg = match msg {
                Message::Binary(data) => tokio_tungstenite::tungstenite::Message::Binary(data),
                Message::Text(text) => {
                    tokio_tungstenite::tungstenite::Message::Text(text.to_string().into())
                }
                Message::Close(_) => break,
                _ => continue,
            };
            if upstream_tx.send(tung_msg).await.is_err() {
                break;
            }
        }
        info!(system_id = %sys_id2, "WebSocket console proxy closed");
    });

    tokio::select! {
        _ = upstream_to_client => {}
        _ = client_to_upstream => {}
    }
}
