pub mod client;
pub mod types;

use std::collections::HashMap;
use std::path::PathBuf;

use axum::http::StatusCode;

use crate::backend::{BackendError, VmmBackend};
use client::UnixClient;
use types::{DiskConfig, VmConfig, VmInfo, VmRemoveDevice, VmmPingResponse};

pub struct CloudHypervisorBackend {
    sockets: HashMap<String, PathBuf>,
}

impl CloudHypervisorBackend {
    pub fn new(sockets: HashMap<String, PathBuf>) -> Self {
        Self { sockets }
    }

    fn client_for(&self, system_id: &str) -> Result<UnixClient, BackendError> {
        let path = self
            .sockets
            .get(system_id)
            .ok_or(BackendError::VmNotFound)?;
        Ok(UnixClient::new(path))
    }

    fn parse_response<T: serde::de::DeserializeOwned>(
        status: StatusCode,
        body: &[u8],
    ) -> Result<T, BackendError> {
        if status.is_success() {
            serde_json::from_slice(body)
                .map_err(|e| BackendError::ApiError(format!("Failed to parse response: {e}")))
        } else {
            let msg = String::from_utf8_lossy(body);
            Err(BackendError::ApiError(format!(
                "HTTP {status}: {msg}"
            )))
        }
    }

    fn check_success(status: StatusCode, body: &[u8]) -> Result<(), BackendError> {
        if status.is_success() {
            Ok(())
        } else {
            let msg = String::from_utf8_lossy(body);
            Err(BackendError::ApiError(format!(
                "HTTP {status}: {msg}"
            )))
        }
    }
}

impl VmmBackend for CloudHypervisorBackend {
    async fn vm_info(&self, system_id: &str) -> Result<VmInfo, BackendError> {
        let client = self.client_for(system_id)?;
        let (status, body) = client.get("/api/v1/vm.info").await?;
        Self::parse_response(status, &body)
    }

    async fn vm_create(&self, system_id: &str, config: VmConfig) -> Result<(), BackendError> {
        let client = self.client_for(system_id)?;
        let payload = serde_json::to_vec(&config)
            .map_err(|e| BackendError::ApiError(e.to_string()))?;
        let (status, body) = client.put("/api/v1/vm.create", &payload).await?;
        Self::check_success(status, &body)
    }

    async fn vm_boot(&self, system_id: &str) -> Result<(), BackendError> {
        let client = self.client_for(system_id)?;
        let (status, body) = client.put("/api/v1/vm.boot", b"").await?;
        Self::check_success(status, &body)
    }

    async fn vm_shutdown(&self, system_id: &str) -> Result<(), BackendError> {
        let client = self.client_for(system_id)?;
        let (status, body) = client.put("/api/v1/vm.shutdown", b"").await?;
        Self::check_success(status, &body)
    }

    async fn vm_delete(&self, system_id: &str) -> Result<(), BackendError> {
        let client = self.client_for(system_id)?;
        let (status, body) = client.put("/api/v1/vm.delete", b"").await?;
        Self::check_success(status, &body)
    }

    async fn vm_power_button(&self, system_id: &str) -> Result<(), BackendError> {
        let client = self.client_for(system_id)?;
        let (status, body) = client.put("/api/v1/vm.power-button", b"").await?;
        Self::check_success(status, &body)
    }

    async fn vm_reboot(&self, system_id: &str) -> Result<(), BackendError> {
        let client = self.client_for(system_id)?;
        let (status, body) = client.put("/api/v1/vm.reboot", b"").await?;
        Self::check_success(status, &body)
    }

    async fn vm_add_disk(&self, system_id: &str, disk: DiskConfig) -> Result<(), BackendError> {
        let client = self.client_for(system_id)?;
        let payload = serde_json::to_vec(&disk)
            .map_err(|e| BackendError::ApiError(e.to_string()))?;
        let (status, body) = client.put("/api/v1/vm.add-disk", &payload).await?;
        Self::check_success(status, &body)
    }

    async fn vm_remove_device(
        &self,
        system_id: &str,
        device_id: &str,
    ) -> Result<(), BackendError> {
        let client = self.client_for(system_id)?;
        let payload = serde_json::to_vec(&VmRemoveDevice {
            id: device_id.to_string(),
        })
        .map_err(|e| BackendError::ApiError(e.to_string()))?;
        let (status, body) = client.put("/api/v1/vm.remove-device", &payload).await?;
        Self::check_success(status, &body)
    }

    async fn vmm_ping(&self, system_id: &str) -> Result<VmmPingResponse, BackendError> {
        let client = self.client_for(system_id)?;
        let (status, body) = client.get("/api/v1/vmm.ping").await?;
        Self::parse_response(status, &body)
    }

    async fn vm_counters(&self, system_id: &str) -> Result<serde_json::Value, BackendError> {
        let client = self.client_for(system_id)?;
        let (status, body) = client.get("/api/v1/vm.counters").await?;
        Self::parse_response(status, &body)
    }
}
