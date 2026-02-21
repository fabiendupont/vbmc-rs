pub mod cloud_hypervisor;

use std::fmt;

#[derive(Debug)]
pub enum BackendError {
    VmmNotRunning,
    VmNotFound,
    InvalidState(String),
    ConnectionFailed(String),
    ApiError(String),
}

impl fmt::Display for BackendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::VmmNotRunning => write!(f, "VMM process is not running"),
            Self::VmNotFound => write!(f, "VM not found"),
            Self::InvalidState(s) => write!(f, "Invalid VM state: {s}"),
            Self::ConnectionFailed(s) => write!(f, "Connection failed: {s}"),
            Self::ApiError(s) => write!(f, "API error: {s}"),
        }
    }
}

impl std::error::Error for BackendError {}

pub trait VmmBackend: Send + Sync {
    fn vm_info(
        &self,
        system_id: &str,
    ) -> impl std::future::Future<Output = Result<cloud_hypervisor::types::VmInfo, BackendError>> + Send;

    fn vm_create(
        &self,
        system_id: &str,
        config: cloud_hypervisor::types::VmConfig,
    ) -> impl std::future::Future<Output = Result<(), BackendError>> + Send;

    fn vm_boot(
        &self,
        system_id: &str,
    ) -> impl std::future::Future<Output = Result<(), BackendError>> + Send;

    fn vm_shutdown(
        &self,
        system_id: &str,
    ) -> impl std::future::Future<Output = Result<(), BackendError>> + Send;

    fn vm_delete(
        &self,
        system_id: &str,
    ) -> impl std::future::Future<Output = Result<(), BackendError>> + Send;

    fn vm_power_button(
        &self,
        system_id: &str,
    ) -> impl std::future::Future<Output = Result<(), BackendError>> + Send;

    fn vm_reboot(
        &self,
        system_id: &str,
    ) -> impl std::future::Future<Output = Result<(), BackendError>> + Send;

    fn vm_add_disk(
        &self,
        system_id: &str,
        disk: cloud_hypervisor::types::DiskConfig,
    ) -> impl std::future::Future<Output = Result<(), BackendError>> + Send;

    fn vm_remove_device(
        &self,
        system_id: &str,
        device_id: &str,
    ) -> impl std::future::Future<Output = Result<(), BackendError>> + Send;

    fn vmm_ping(
        &self,
        system_id: &str,
    ) -> impl std::future::Future<Output = Result<cloud_hypervisor::types::VmmPingResponse, BackendError>> + Send;

    fn vm_counters(
        &self,
        system_id: &str,
    ) -> impl std::future::Future<Output = Result<serde_json::Value, BackendError>> + Send;
}
