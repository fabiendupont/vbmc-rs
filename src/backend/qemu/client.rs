use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

use super::types::{QmpGreeting, QmpResponse};
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

    async fn connect(
        &self,
    ) -> Result<
        (
            BufReader<tokio::net::unix::OwnedReadHalf>,
            tokio::net::unix::OwnedWriteHalf,
        ),
        BackendError,
    > {
        let stream = UnixStream::connect(&self.socket_path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound
                || e.kind() == std::io::ErrorKind::ConnectionRefused
            {
                BackendError::VmmNotRunning
            } else {
                BackendError::ConnectionFailed(e.to_string())
            }
        })?;

        let (reader, writer) = stream.into_split();
        Ok((BufReader::new(reader), writer))
    }

    async fn handshake(
        reader: &mut BufReader<tokio::net::unix::OwnedReadHalf>,
        writer: &mut tokio::net::unix::OwnedWriteHalf,
    ) -> Result<QmpGreeting, BackendError> {
        let mut line = String::new();

        // Read and parse QMP greeting
        reader
            .read_line(&mut line)
            .await
            .map_err(|e| BackendError::ConnectionFailed(e.to_string()))?;

        let greeting: QmpGreeting = serde_json::from_str(&line)
            .map_err(|e| BackendError::ApiError(format!("Invalid QMP greeting: {e}")))?;

        // Send qmp_capabilities to enter command mode
        let caps = serde_json::json!({"execute": "qmp_capabilities"});
        let mut caps_bytes =
            serde_json::to_vec(&caps).map_err(|e| BackendError::ApiError(e.to_string()))?;
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

        let caps_resp: QmpResponse<serde_json::Value> = serde_json::from_str(&line)
            .map_err(|e| BackendError::ApiError(format!("Invalid QMP response: {e}")))?;
        if let Some(err) = caps_resp.error {
            return Err(BackendError::ApiError(format!(
                "qmp_capabilities failed: {}",
                err.desc
            )));
        }

        Ok(greeting)
    }

    /// Connect, perform the QMP handshake, and return the QEMU version string.
    pub async fn query_version(&self) -> Result<String, BackendError> {
        let (mut reader, mut writer) = self.connect().await?;
        let greeting = Self::handshake(&mut reader, &mut writer).await?;
        let v = &greeting.qmp.version.qemu;
        Ok(format!("{}.{}.{}", v.major, v.minor, v.micro))
    }

    /// Execute a QMP command and deserialize the response into `T`.
    pub async fn execute<T: DeserializeOwned>(
        &self,
        command: &str,
        arguments: Option<serde_json::Value>,
    ) -> Result<T, BackendError> {
        let (mut reader, mut writer) = self.connect().await?;
        Self::handshake(&mut reader, &mut writer).await?;

        // Send the command
        let mut cmd = serde_json::json!({"execute": command});
        if let Some(args) = arguments {
            cmd["arguments"] = args;
        }
        let mut cmd_bytes =
            serde_json::to_vec(&cmd).map_err(|e| BackendError::ApiError(e.to_string()))?;
        cmd_bytes.push(b'\n');
        writer
            .write_all(&cmd_bytes)
            .await
            .map_err(|e| BackendError::ConnectionFailed(e.to_string()))?;

        // Read response, skipping event messages
        let mut line = String::new();
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

            // Skip event notifications
            let peek: serde_json::Value = serde_json::from_str(&line)
                .map_err(|e| BackendError::ApiError(format!("Invalid JSON: {e}")))?;
            if peek.get("event").is_some() {
                continue;
            }

            // Parse as typed response
            let resp: QmpResponse<T> = serde_json::from_str(&line).map_err(|e| {
                BackendError::ApiError(format!("Failed to parse QMP response: {e}"))
            })?;

            if let Some(err) = resp.error {
                return Err(BackendError::ApiError(format!("QMP error: {}", err.desc)));
            }

            if let Some(result) = resp.result {
                return Ok(result);
            }

            return Err(BackendError::ApiError(
                "QMP response contained neither 'return' nor 'error'".to_string(),
            ));
        }
    }

    /// Execute a QMP command that returns `{"return": {}}`.
    pub async fn execute_void(
        &self,
        command: &str,
        arguments: Option<serde_json::Value>,
    ) -> Result<(), BackendError> {
        let _: serde_json::Value = self.execute(command, arguments).await?;
        Ok(())
    }
}

#[cfg(test)]
mod coverage_tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_qmp_client_new() {
        let path = PathBuf::from("/tmp/qmp.sock");
        let client = QmpClient::new(&path);
        assert_eq!(client.socket_path, path);
    }

    #[tokio::test]
    async fn test_connect_to_nonexistent_socket() {
        let client = QmpClient::new(&PathBuf::from("/nonexistent/qmp.sock"));
        let result = client.connect().await;
        assert!(result.is_err());
        match result.unwrap_err() {
            BackendError::VmmNotRunning => {}
            other => panic!("Expected VmmNotRunning, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_query_version_nonexistent_socket() {
        let client = QmpClient::new(&PathBuf::from("/nonexistent/qmp-test.sock"));
        let result = client.query_version().await;
        assert!(result.is_err());
        match result.unwrap_err() {
            BackendError::VmmNotRunning => {}
            other => panic!("Expected VmmNotRunning, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_execute_nonexistent_socket() {
        let client = QmpClient::new(&PathBuf::from("/nonexistent/qmp-exec.sock"));
        let result: Result<serde_json::Value, _> = client.execute("query-status", None).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            BackendError::VmmNotRunning => {}
            other => panic!("Expected VmmNotRunning, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_execute_void_nonexistent_socket() {
        let client = QmpClient::new(&PathBuf::from("/nonexistent/qmp-void.sock"));
        let result = client.execute_void("cont", None).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            BackendError::VmmNotRunning => {}
            other => panic!("Expected VmmNotRunning, got: {other:?}"),
        }
    }

    #[test]
    fn test_qmp_command_serialization() {
        // Test that command JSON is built correctly
        let cmd = serde_json::json!({"execute": "query-status"});
        assert_eq!(cmd["execute"], "query-status");

        let cmd_with_args = serde_json::json!({
            "execute": "blockdev-add",
            "arguments": {"driver": "file", "filename": "/tmp/test.img"}
        });
        assert_eq!(cmd_with_args["execute"], "blockdev-add");
        assert_eq!(cmd_with_args["arguments"]["driver"], "file");
    }

    #[test]
    fn test_greeting_parse_from_real_qemu_format() {
        // Real QEMU greeting format
        let greeting_json = r#"{"QMP": {"version": {"qemu": {"micro": 1, "minor": 2, "major": 8}, "package": "Fedora-8.2.1-1.fc40"}, "capabilities": ["oob"]}}"#;
        let greeting: QmpGreeting = serde_json::from_str(greeting_json).unwrap();
        assert_eq!(greeting.qmp.version.qemu.major, 8);
        assert_eq!(greeting.qmp.version.qemu.minor, 2);
        assert_eq!(greeting.qmp.version.qemu.micro, 1);
    }

    #[test]
    fn test_qmp_response_parsing_empty_return() {
        let json = r#"{"return": {}}"#;
        let resp: QmpResponse<serde_json::Value> = serde_json::from_str(json).unwrap();
        assert!(resp.result.is_some());
        assert!(resp.error.is_none());
    }

    #[test]
    fn test_qmp_error_response() {
        let json = r#"{"error": {"class": "GenericError", "desc": "Device 'virtio0' not found"}}"#;
        let resp: QmpResponse<serde_json::Value> = serde_json::from_str(json).unwrap();
        assert!(resp.result.is_none());
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap().desc, "Device 'virtio0' not found");
    }

    #[test]
    fn test_event_detection() {
        // Events have an "event" field and should be skipped during execute()
        let event_json = r#"{"event": "SHUTDOWN", "timestamp": {"seconds": 1234567890, "microseconds": 123456}}"#;
        let val: serde_json::Value = serde_json::from_str(event_json).unwrap();
        assert!(val.get("event").is_some());
        assert_eq!(val["event"], "SHUTDOWN");
    }
}
