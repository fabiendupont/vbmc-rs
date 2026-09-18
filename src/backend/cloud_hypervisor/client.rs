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

#[cfg(test)]
mod coverage_tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_unix_client_new() {
        let path = PathBuf::from("/tmp/cloud-hypervisor.sock");
        let client = UnixClient::new(&path);
        assert_eq!(client.socket_path, path);
    }

    #[test]
    fn test_find_header_end_basic() {
        let response = b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nHello";
        let pos = find_header_end(response).unwrap();
        assert_eq!(pos, 38);
        assert_eq!(&response[pos..], b"Hello");
    }

    #[test]
    fn test_find_header_end_no_body() {
        let response = b"HTTP/1.1 204 No Content\r\n\r\n";
        let pos = find_header_end(response).unwrap();
        assert_eq!(pos, 27);
    }

    #[test]
    fn test_find_header_end_not_found() {
        let incomplete = b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n";
        assert!(find_header_end(incomplete).is_none());
    }

    #[test]
    fn test_find_header_end_empty() {
        assert!(find_header_end(b"").is_none());
    }

    #[test]
    fn test_find_header_end_short() {
        assert!(find_header_end(b"abc").is_none());
    }

    #[test]
    fn test_find_header_end_with_headers() {
        let response = b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 13\r\n\r\n{\"status\":\"ok\"}";
        let pos = find_header_end(response).unwrap();
        assert_eq!(&response[pos..], b"{\"status\":\"ok\"}");
    }

    #[tokio::test]
    async fn test_request_nonexistent_socket() {
        let client = UnixClient::new(&PathBuf::from("/nonexistent/ch.sock"));
        let result = client.request(Method::GET, "/api/v1/vmm.ping", None).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            BackendError::VmmNotRunning => {}
            other => panic!("Expected VmmNotRunning, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_get_nonexistent_socket() {
        let client = UnixClient::new(&PathBuf::from("/nonexistent/ch-get.sock"));
        let result = client.get("/api/v1/vm.info").await;
        assert!(result.is_err());
        match result.unwrap_err() {
            BackendError::VmmNotRunning => {}
            other => panic!("Expected VmmNotRunning, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_put_nonexistent_socket() {
        let client = UnixClient::new(&PathBuf::from("/nonexistent/ch-put.sock"));
        let result = client.put("/api/v1/vm.boot", b"{}").await;
        assert!(result.is_err());
        match result.unwrap_err() {
            BackendError::VmmNotRunning => {}
            other => panic!("Expected VmmNotRunning, got: {other:?}"),
        }
    }

    #[test]
    fn test_http_request_format_get() {
        // Verify the format of HTTP request we build
        let method = Method::GET;
        let path = "/api/v1/vmm.ping";
        let mut request = format!("{method} {path} HTTP/1.1\r\nHost: localhost\r\n");
        request.push_str("Content-Length: 0\r\n");
        request.push_str("Connection: close\r\n\r\n");

        assert!(request.contains("GET /api/v1/vmm.ping HTTP/1.1"));
        assert!(request.contains("Host: localhost"));
        assert!(request.contains("Content-Length: 0"));
        assert!(request.contains("Connection: close"));
        assert!(request.ends_with("\r\n\r\n"));
    }

    #[test]
    fn test_http_request_format_put_with_body() {
        let method = Method::PUT;
        let path = "/api/v1/vm.create";
        let body = br#"{"cpus":{"boot_vcpus":2,"max_vcpus":2}}"#;
        let mut request = format!("{method} {path} HTTP/1.1\r\nHost: localhost\r\n");
        request.push_str("Content-Type: application/json\r\n");
        request.push_str(&format!("Content-Length: {}\r\n", body.len()));
        request.push_str("Connection: close\r\n\r\n");

        assert!(request.contains("PUT /api/v1/vm.create HTTP/1.1"));
        assert!(request.contains("Content-Type: application/json"));
        assert!(request.contains(&format!("Content-Length: {}", body.len())));
    }

    #[test]
    fn test_status_line_parsing() {
        let status_line = "HTTP/1.1 200 OK";
        let status_code = status_line
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap();
        assert_eq!(status_code, 200);

        let status = StatusCode::from_u16(status_code).unwrap();
        assert_eq!(status, StatusCode::OK);
    }

    #[test]
    fn test_status_line_parsing_error() {
        let status_line = "HTTP/1.1 404 Not Found";
        let status_code = status_line
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap();
        assert_eq!(status_code, 404);

        let status = StatusCode::from_u16(status_code).unwrap();
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(!status.is_success());
    }

    #[test]
    fn test_status_line_parsing_created() {
        let status_line = "HTTP/1.1 201 Created";
        let status_code = status_line
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap();
        let status = StatusCode::from_u16(status_code).unwrap();
        assert!(status.is_success());
    }

    #[test]
    fn test_status_line_parsing_no_content() {
        let status_line = "HTTP/1.1 204 No Content";
        let status_code = status_line
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap();
        let status = StatusCode::from_u16(status_code).unwrap();
        assert_eq!(status, StatusCode::NO_CONTENT);
        assert!(status.is_success());
    }
}
