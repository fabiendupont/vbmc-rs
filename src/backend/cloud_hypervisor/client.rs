use std::path::{Path, PathBuf};

use axum::http::{Method, StatusCode};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

use crate::backend::BackendError;

pub struct UnixClient {
    socket_path: PathBuf,
}

impl UnixClient {
    pub fn new(socket_path: &Path) -> Self {
        Self {
            socket_path: socket_path.to_path_buf(),
        }
    }

    pub async fn request(
        &self,
        method: Method,
        path: &str,
        body: Option<&[u8]>,
    ) -> Result<(StatusCode, Vec<u8>), BackendError> {
        let mut stream = UnixStream::connect(&self.socket_path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound
                || e.kind() == std::io::ErrorKind::ConnectionRefused
            {
                BackendError::VmmNotRunning
            } else {
                BackendError::ConnectionFailed(e.to_string())
            }
        })?;

        // Build raw HTTP/1.1 request
        let mut request = format!("{method} {path} HTTP/1.1\r\nHost: localhost\r\n");

        if let Some(b) = body {
            request.push_str("Content-Type: application/json\r\n");
            request.push_str(&format!("Content-Length: {}\r\n", b.len()));
        } else {
            request.push_str("Content-Length: 0\r\n");
        }

        request.push_str("Connection: close\r\n\r\n");

        stream
            .write_all(request.as_bytes())
            .await
            .map_err(|e| BackendError::ConnectionFailed(e.to_string()))?;

        if let Some(b) = body {
            stream
                .write_all(b)
                .await
                .map_err(|e| BackendError::ConnectionFailed(e.to_string()))?;
        }

        stream
            .flush()
            .await
            .map_err(|e| BackendError::ConnectionFailed(e.to_string()))?;

        // Read response
        let mut response_buf = Vec::new();
        stream
            .read_to_end(&mut response_buf)
            .await
            .map_err(|e| BackendError::ApiError(e.to_string()))?;

        let response_str = String::from_utf8_lossy(&response_buf);

        // Parse status line
        let status_line = response_str
            .lines()
            .next()
            .ok_or_else(|| BackendError::ApiError("Empty response".to_string()))?;

        let status_code = status_line
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse::<u16>().ok())
            .ok_or_else(|| BackendError::ApiError(format!("Invalid status line: {status_line}")))?;

        let status =
            StatusCode::from_u16(status_code).map_err(|e| BackendError::ApiError(e.to_string()))?;

        // Find body (after \r\n\r\n)
        let body_bytes = if let Some(pos) = find_header_end(&response_buf) {
            response_buf[pos..].to_vec()
        } else {
            Vec::new()
        };

        Ok((status, body_bytes))
    }

    pub async fn get(&self, path: &str) -> Result<(StatusCode, Vec<u8>), BackendError> {
        self.request(Method::GET, path, None).await
    }

    pub async fn put(
        &self,
        path: &str,
        body: &[u8],
    ) -> Result<(StatusCode, Vec<u8>), BackendError> {
        self.request(Method::PUT, path, Some(body)).await
    }
}

fn find_header_end(buf: &[u8]) -> Option<usize> {
    for i in 0..buf.len().saturating_sub(3) {
        if buf[i] == b'\r' && buf[i + 1] == b'\n' && buf[i + 2] == b'\r' && buf[i + 3] == b'\n' {
            return Some(i + 4);
        }
    }
    None
}
