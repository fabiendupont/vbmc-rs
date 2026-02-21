pub mod cloud_hypervisor;
#[cfg(feature = "qemu")]
pub mod qemu;
#[cfg(feature = "libvirt")]
pub mod libvirt;
pub mod types;

use std::fmt;

use types::{DiskCreateConfig, VmCreateConfig, VmInfo, VmmPingResponse};

#[derive(Debug)]
pub enum BackendError {
    VmmNotRunning,
    VmNotFound,
    InvalidState(String),
    ConnectionFailed(String),
    ApiError(String),
    NotSupported(String),
}

impl fmt::Display for BackendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::VmmNotRunning => write!(f, "VMM process is not running"),
            Self::VmNotFound => write!(f, "VM not found"),
            Self::InvalidState(s) => write!(f, "Invalid VM state: {s}"),
            Self::ConnectionFailed(s) => write!(f, "Connection failed: {s}"),
            Self::ApiError(s) => write!(f, "API error: {s}"),
            Self::NotSupported(s) => write!(f, "Operation not supported: {s}"),
        }
    }
}

impl std::error::Error for BackendError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backend_error_display() {
        assert_eq!(BackendError::VmmNotRunning.to_string(), "VMM process is not running");
        assert_eq!(BackendError::VmNotFound.to_string(), "VM not found");
        assert_eq!(
            BackendError::InvalidState("bad".into()).to_string(),
            "Invalid VM state: bad"
        );
        assert_eq!(
            BackendError::ConnectionFailed("refused".into()).to_string(),
            "Connection failed: refused"
        );
        assert_eq!(
            BackendError::ApiError("500".into()).to_string(),
            "API error: 500"
        );
        assert_eq!(
            BackendError::NotSupported("vm_create".into()).to_string(),
            "Operation not supported: vm_create"
        );
    }
}

pub trait VmmBackend: Send + Sync {
    fn vm_info(
        &self,
        system_id: &str,
    ) -> impl std::future::Future<Output = Result<VmInfo, BackendError>> + Send;

    fn vm_create(
        &self,
        system_id: &str,
        config: VmCreateConfig,
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
        disk: DiskCreateConfig,
    ) -> impl std::future::Future<Output = Result<(), BackendError>> + Send;

    fn vm_remove_device(
        &self,
        system_id: &str,
        device_id: &str,
    ) -> impl std::future::Future<Output = Result<(), BackendError>> + Send;

    fn vmm_ping(
        &self,
        system_id: &str,
    ) -> impl std::future::Future<Output = Result<VmmPingResponse, BackendError>> + Send;

    fn vm_counters(
        &self,
        system_id: &str,
    ) -> impl std::future::Future<Output = Result<serde_json::Value, BackendError>> + Send;
}

pub enum Backend {
    CloudHypervisor(cloud_hypervisor::CloudHypervisorBackend),
    #[cfg(feature = "qemu")]
    Qemu(qemu::QemuBackend),
    #[cfg(feature = "libvirt")]
    Libvirt(libvirt::LibvirtBackend),
}

impl VmmBackend for Backend {
    async fn vm_info(&self, system_id: &str) -> Result<VmInfo, BackendError> {
        match self {
            Self::CloudHypervisor(b) => b.vm_info(system_id).await,
            #[cfg(feature = "qemu")]
            Self::Qemu(b) => b.vm_info(system_id).await,
            #[cfg(feature = "libvirt")]
            Self::Libvirt(b) => b.vm_info(system_id).await,
        }
    }

    async fn vm_create(
        &self,
        system_id: &str,
        config: VmCreateConfig,
    ) -> Result<(), BackendError> {
        match self {
            Self::CloudHypervisor(b) => b.vm_create(system_id, config).await,
            #[cfg(feature = "qemu")]
            Self::Qemu(b) => b.vm_create(system_id, config).await,
            #[cfg(feature = "libvirt")]
            Self::Libvirt(b) => b.vm_create(system_id, config).await,
        }
    }

    async fn vm_boot(&self, system_id: &str) -> Result<(), BackendError> {
        match self {
            Self::CloudHypervisor(b) => b.vm_boot(system_id).await,
            #[cfg(feature = "qemu")]
            Self::Qemu(b) => b.vm_boot(system_id).await,
            #[cfg(feature = "libvirt")]
            Self::Libvirt(b) => b.vm_boot(system_id).await,
        }
    }

    async fn vm_shutdown(&self, system_id: &str) -> Result<(), BackendError> {
        match self {
            Self::CloudHypervisor(b) => b.vm_shutdown(system_id).await,
            #[cfg(feature = "qemu")]
            Self::Qemu(b) => b.vm_shutdown(system_id).await,
            #[cfg(feature = "libvirt")]
            Self::Libvirt(b) => b.vm_shutdown(system_id).await,
        }
    }

    async fn vm_delete(&self, system_id: &str) -> Result<(), BackendError> {
        match self {
            Self::CloudHypervisor(b) => b.vm_delete(system_id).await,
            #[cfg(feature = "qemu")]
            Self::Qemu(b) => b.vm_delete(system_id).await,
            #[cfg(feature = "libvirt")]
            Self::Libvirt(b) => b.vm_delete(system_id).await,
        }
    }

    async fn vm_power_button(&self, system_id: &str) -> Result<(), BackendError> {
        match self {
            Self::CloudHypervisor(b) => b.vm_power_button(system_id).await,
            #[cfg(feature = "qemu")]
            Self::Qemu(b) => b.vm_power_button(system_id).await,
            #[cfg(feature = "libvirt")]
            Self::Libvirt(b) => b.vm_power_button(system_id).await,
        }
    }

    async fn vm_reboot(&self, system_id: &str) -> Result<(), BackendError> {
        match self {
            Self::CloudHypervisor(b) => b.vm_reboot(system_id).await,
            #[cfg(feature = "qemu")]
            Self::Qemu(b) => b.vm_reboot(system_id).await,
            #[cfg(feature = "libvirt")]
            Self::Libvirt(b) => b.vm_reboot(system_id).await,
        }
    }

    async fn vm_add_disk(
        &self,
        system_id: &str,
        disk: DiskCreateConfig,
    ) -> Result<(), BackendError> {
        match self {
            Self::CloudHypervisor(b) => b.vm_add_disk(system_id, disk).await,
            #[cfg(feature = "qemu")]
            Self::Qemu(b) => b.vm_add_disk(system_id, disk).await,
            #[cfg(feature = "libvirt")]
            Self::Libvirt(b) => b.vm_add_disk(system_id, disk).await,
        }
    }

    async fn vm_remove_device(
        &self,
        system_id: &str,
        device_id: &str,
    ) -> Result<(), BackendError> {
        match self {
            Self::CloudHypervisor(b) => b.vm_remove_device(system_id, device_id).await,
            #[cfg(feature = "qemu")]
            Self::Qemu(b) => b.vm_remove_device(system_id, device_id).await,
            #[cfg(feature = "libvirt")]
            Self::Libvirt(b) => b.vm_remove_device(system_id, device_id).await,
        }
    }

    async fn vmm_ping(&self, system_id: &str) -> Result<VmmPingResponse, BackendError> {
        match self {
            Self::CloudHypervisor(b) => b.vmm_ping(system_id).await,
            #[cfg(feature = "qemu")]
            Self::Qemu(b) => b.vmm_ping(system_id).await,
            #[cfg(feature = "libvirt")]
            Self::Libvirt(b) => b.vmm_ping(system_id).await,
        }
    }

    async fn vm_counters(&self, system_id: &str) -> Result<serde_json::Value, BackendError> {
        match self {
            Self::CloudHypervisor(b) => b.vm_counters(system_id).await,
            #[cfg(feature = "qemu")]
            Self::Qemu(b) => b.vm_counters(system_id).await,
            #[cfg(feature = "libvirt")]
            Self::Libvirt(b) => b.vm_counters(system_id).await,
        }
    }
}
