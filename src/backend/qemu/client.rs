use std::path::{Path, PathBuf};

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

use crate::backend::BackendError;

pub struct QmpClient {
    socket_path: PathBuf,
}

impl QmpClient {
    pub fn new(socket_path: &Path) -> Self {
        Self {
            socket_path: socket_path.to_path_buf(),
        }
    }

    /// Execute a QMP command and return the raw JSON response.
    /// Each call opens a new connection, performs the greeting/capabilities
    /// handshake, sends the command, and returns the result.
    pub async fn execute(
        &self,
        command: &str,
        arguments: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, BackendError> {
        let stream = UnixStream::connect(&self.socket_path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound
                || e.kind() == std::io::ErrorKind::ConnectionRefused
            {
                BackendError::VmmNotRunning
            } else {
                BackendError::ConnectionFailed(e.to_string())
            }
        })?;

        let (reader, mut writer) = stream.into_split();
        let mut reader = BufReader::new(reader);
        let mut line = String::new();

        // 1. Read QMP greeting
        reader
            .read_line(&mut line)
            .await
            .map_err(|e| BackendError::ConnectionFailed(e.to_string()))?;

        // Validate it's a QMP greeting
        if !line.contains("\"QMP\"") {
            return Err(BackendError::ApiError(format!(
                "Expected QMP greeting, got: {line}"
            )));
        }

        // 2. Send qmp_capabilities to enter command mode
        let caps = serde_json::json!({"execute": "qmp_capabilities"});
        let mut caps_bytes = serde_json::to_vec(&caps)
            .map_err(|e| BackendError::ApiError(e.to_string()))?;
        caps_bytes.push(b'\n');
        writer
            .write_all(&caps_bytes)
            .await
            .map_err(|e| BackendError::ConnectionFailed(e.to_string()))?;

        // Read capabilities response
        line.clear();
        reader
            .read_line(&mut line)
            .await
            .map_err(|e| BackendError::ConnectionFailed(e.to_string()))?;

        // Check for error in capabilities response
        let caps_resp: serde_json::Value = serde_json::from_str(&line)
            .map_err(|e| BackendError::ApiError(format!("Invalid QMP response: {e}")))?;
        if caps_resp.get("error").is_some() {
            return Err(BackendError::ApiError(format!(
                "qmp_capabilities failed: {}",
                caps_resp
            )));
        }

        // 3. Send the actual command
        let mut cmd = serde_json::json!({"execute": command});
        if let Some(args) = arguments {
            cmd["arguments"] = args;
        }
        let mut cmd_bytes = serde_json::to_vec(&cmd)
            .map_err(|e| BackendError::ApiError(e.to_string()))?;
        cmd_bytes.push(b'\n');
        writer
            .write_all(&cmd_bytes)
            .await
            .map_err(|e| BackendError::ConnectionFailed(e.to_string()))?;

        // 4. Read response, skipping event messages
        loop {
            line.clear();
            let bytes_read = reader
                .read_line(&mut line)
                .await
                .map_err(|e| BackendError::ConnectionFailed(e.to_string()))?;

            if bytes_read == 0 {
                return Err(BackendError::ApiError(
                    "Connection closed before response".to_string(),
                ));
            }

            let resp: serde_json::Value = serde_json::from_str(&line)
                .map_err(|e| BackendError::ApiError(format!("Invalid JSON: {e}")))?;

            // Skip event notifications
            if resp.get("event").is_some() {
                continue;
            }

            // Check for error
            if let Some(err) = resp.get("error") {
                let desc = err
                    .get("desc")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown error");
                return Err(BackendError::ApiError(format!("QMP error: {desc}")));
            }

            // Return the "return" field
            if let Some(result) = resp.get("return") {
                return Ok(result.clone());
            }

            return Ok(resp);
        }
    }
}
