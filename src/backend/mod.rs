pub mod cloud_hypervisor;
#[cfg(feature = "kubevirt")]
pub mod kubevirt;
#[cfg(feature = "libvirt")]
pub mod libvirt;
#[cfg(feature = "qemu")]
pub mod qemu;
pub mod types;

use std::fmt;

use types::{
    DiskCreateConfig, SerialConsoleInfo, VmCounters, VmCreateConfig, VmInfo, VmmPingResponse,
};

#[derive(Debug)]
#[allow(dead_code)]
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
    use super::BackendError;

    #[test]
    fn test_backend_error_display() {
        assert_eq!(
            BackendError::VmmNotRunning.to_string(),
            "VMM process is not running"
        );
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

    #[allow(dead_code)]
    fn vmm_ping(
        &self,
        system_id: &str,
    ) -> impl std::future::Future<Output = Result<VmmPingResponse, BackendError>> + Send;

    fn vm_counters(
        &self,
        system_id: &str,
    ) -> impl std::future::Future<Output = Result<VmCounters, BackendError>> + Send;

    fn vm_set_secure_boot(
        &self,
        system_id: &str,
        enabled: bool,
    ) -> impl std::future::Future<Output = Result<(), BackendError>> + Send;

    fn vm_serial_console(
        &self,
        system_id: &str,
    ) -> impl std::future::Future<Output = Result<SerialConsoleInfo, BackendError>> + Send;
}

pub enum Backend {
    CloudHypervisor(cloud_hypervisor::CloudHypervisorBackend),
    #[cfg(feature = "kubevirt")]
    KubeVirt(kubevirt::KubeVirtBackend),
    #[cfg(feature = "qemu")]
    Qemu(qemu::QemuBackend),
    #[cfg(feature = "libvirt")]
    Libvirt(libvirt::LibvirtBackend),
    #[cfg(any(test, feature = "test-support"))]
    Mock(mock::MockBackend),
}

#[cfg(any(test, feature = "test-support"))]
pub mod mock {
    use std::collections::HashMap;
    use std::sync::Mutex;

    use super::types as bt;
    use super::{BackendError, VmmBackend};

    pub struct MockBackend {
        vms: Mutex<HashMap<String, bt::VmInfo>>,
    }

    impl Default for MockBackend {
        fn default() -> Self {
            Self {
                vms: Mutex::new(HashMap::new()),
            }
        }
    }

    impl MockBackend {
        pub fn new() -> Self {
            Self::default()
        }

        pub fn with_vm(self, system_id: &str, info: bt::VmInfo) -> Self {
            self.vms.lock().unwrap().insert(system_id.to_string(), info);
            self
        }
    }

    impl VmmBackend for MockBackend {
        async fn vm_info(&self, system_id: &str) -> Result<bt::VmInfo, BackendError> {
            self.vms
                .lock()
                .unwrap()
                .get(system_id)
                .cloned()
                .ok_or(BackendError::VmmNotRunning)
        }

        async fn vm_create(
            &self,
            _system_id: &str,
            _config: bt::VmCreateConfig,
        ) -> Result<(), BackendError> {
            Ok(())
        }

        async fn vm_boot(&self, _system_id: &str) -> Result<(), BackendError> {
            Ok(())
        }

        async fn vm_shutdown(&self, _system_id: &str) -> Result<(), BackendError> {
            Ok(())
        }

        async fn vm_delete(&self, _system_id: &str) -> Result<(), BackendError> {
            Ok(())
        }

        async fn vm_power_button(&self, _system_id: &str) -> Result<(), BackendError> {
            Ok(())
        }

        async fn vm_reboot(&self, _system_id: &str) -> Result<(), BackendError> {
            Ok(())
        }

        async fn vm_add_disk(
            &self,
            _system_id: &str,
            _disk: bt::DiskCreateConfig,
        ) -> Result<(), BackendError> {
            Ok(())
        }

        async fn vm_remove_device(
            &self,
            _system_id: &str,
            _device_id: &str,
        ) -> Result<(), BackendError> {
            Ok(())
        }

        async fn vmm_ping(&self, _system_id: &str) -> Result<bt::VmmPingResponse, BackendError> {
            Ok(bt::VmmPingResponse {
                version: Some("mock-1.0".to_string()),
                pid: Some(12345),
            })
        }

        async fn vm_counters(&self, _system_id: &str) -> Result<bt::VmCounters, BackendError> {
            Ok(bt::VmCounters::default())
        }

        async fn vm_set_secure_boot(
            &self,
            _system_id: &str,
            _enabled: bool,
        ) -> Result<(), BackendError> {
            Ok(())
        }

        async fn vm_serial_console(
            &self,
            _system_id: &str,
        ) -> Result<bt::SerialConsoleInfo, BackendError> {
            Err(BackendError::NotSupported(
                "serial console not available in mock backend".to_string(),
            ))
        }
    }
}

impl VmmBackend for Backend {
    async fn vm_info(&self, system_id: &str) -> Result<VmInfo, BackendError> {
        match self {
            Self::CloudHypervisor(b) => b.vm_info(system_id).await,
            #[cfg(feature = "kubevirt")]
            Self::KubeVirt(b) => b.vm_info(system_id).await,
            #[cfg(feature = "qemu")]
            Self::Qemu(b) => b.vm_info(system_id).await,
            #[cfg(feature = "libvirt")]
            Self::Libvirt(b) => b.vm_info(system_id).await,
            #[cfg(any(test, feature = "test-support"))]
            Self::Mock(b) => b.vm_info(system_id).await,
        }
    }

    async fn vm_create(&self, system_id: &str, config: VmCreateConfig) -> Result<(), BackendError> {
        match self {
            Self::CloudHypervisor(b) => b.vm_create(system_id, config).await,
            #[cfg(feature = "kubevirt")]
            Self::KubeVirt(b) => b.vm_create(system_id, config).await,
            #[cfg(feature = "qemu")]
            Self::Qemu(b) => b.vm_create(system_id, config).await,
            #[cfg(feature = "libvirt")]
            Self::Libvirt(b) => b.vm_create(system_id, config).await,
            #[cfg(any(test, feature = "test-support"))]
            Self::Mock(b) => b.vm_create(system_id, config).await,
        }
    }

    async fn vm_boot(&self, system_id: &str) -> Result<(), BackendError> {
        match self {
            Self::CloudHypervisor(b) => b.vm_boot(system_id).await,
            #[cfg(feature = "kubevirt")]
            Self::KubeVirt(b) => b.vm_boot(system_id).await,
            #[cfg(feature = "qemu")]
            Self::Qemu(b) => b.vm_boot(system_id).await,
            #[cfg(feature = "libvirt")]
            Self::Libvirt(b) => b.vm_boot(system_id).await,
            #[cfg(any(test, feature = "test-support"))]
            Self::Mock(b) => b.vm_boot(system_id).await,
        }
    }

    async fn vm_shutdown(&self, system_id: &str) -> Result<(), BackendError> {
        match self {
            Self::CloudHypervisor(b) => b.vm_shutdown(system_id).await,
            #[cfg(feature = "kubevirt")]
            Self::KubeVirt(b) => b.vm_shutdown(system_id).await,
            #[cfg(feature = "qemu")]
            Self::Qemu(b) => b.vm_shutdown(system_id).await,
            #[cfg(feature = "libvirt")]
            Self::Libvirt(b) => b.vm_shutdown(system_id).await,
            #[cfg(any(test, feature = "test-support"))]
            Self::Mock(b) => b.vm_shutdown(system_id).await,
        }
    }

    async fn vm_delete(&self, system_id: &str) -> Result<(), BackendError> {
        match self {
            Self::CloudHypervisor(b) => b.vm_delete(system_id).await,
            #[cfg(feature = "kubevirt")]
            Self::KubeVirt(b) => b.vm_delete(system_id).await,
            #[cfg(feature = "qemu")]
            Self::Qemu(b) => b.vm_delete(system_id).await,
            #[cfg(feature = "libvirt")]
            Self::Libvirt(b) => b.vm_delete(system_id).await,
            #[cfg(any(test, feature = "test-support"))]
            Self::Mock(b) => b.vm_delete(system_id).await,
        }
    }

    async fn vm_power_button(&self, system_id: &str) -> Result<(), BackendError> {
        match self {
            Self::CloudHypervisor(b) => b.vm_power_button(system_id).await,
            #[cfg(feature = "kubevirt")]
            Self::KubeVirt(b) => b.vm_power_button(system_id).await,
            #[cfg(feature = "qemu")]
            Self::Qemu(b) => b.vm_power_button(system_id).await,
            #[cfg(feature = "libvirt")]
            Self::Libvirt(b) => b.vm_power_button(system_id).await,
            #[cfg(any(test, feature = "test-support"))]
            Self::Mock(b) => b.vm_power_button(system_id).await,
        }
    }

    async fn vm_reboot(&self, system_id: &str) -> Result<(), BackendError> {
        match self {
            Self::CloudHypervisor(b) => b.vm_reboot(system_id).await,
            #[cfg(feature = "kubevirt")]
            Self::KubeVirt(b) => b.vm_reboot(system_id).await,
            #[cfg(feature = "qemu")]
            Self::Qemu(b) => b.vm_reboot(system_id).await,
            #[cfg(feature = "libvirt")]
            Self::Libvirt(b) => b.vm_reboot(system_id).await,
            #[cfg(any(test, feature = "test-support"))]
            Self::Mock(b) => b.vm_reboot(system_id).await,
        }
    }

    async fn vm_add_disk(
        &self,
        system_id: &str,
        disk: DiskCreateConfig,
    ) -> Result<(), BackendError> {
        match self {
            Self::CloudHypervisor(b) => b.vm_add_disk(system_id, disk).await,
            #[cfg(feature = "kubevirt")]
            Self::KubeVirt(b) => b.vm_add_disk(system_id, disk).await,
            #[cfg(feature = "qemu")]
            Self::Qemu(b) => b.vm_add_disk(system_id, disk).await,
            #[cfg(feature = "libvirt")]
            Self::Libvirt(b) => b.vm_add_disk(system_id, disk).await,
            #[cfg(any(test, feature = "test-support"))]
            Self::Mock(b) => b.vm_add_disk(system_id, disk).await,
        }
    }

    async fn vm_remove_device(&self, system_id: &str, device_id: &str) -> Result<(), BackendError> {
        match self {
            Self::CloudHypervisor(b) => b.vm_remove_device(system_id, device_id).await,
            #[cfg(feature = "kubevirt")]
            Self::KubeVirt(b) => b.vm_remove_device(system_id, device_id).await,
            #[cfg(feature = "qemu")]
            Self::Qemu(b) => b.vm_remove_device(system_id, device_id).await,
            #[cfg(feature = "libvirt")]
            Self::Libvirt(b) => b.vm_remove_device(system_id, device_id).await,
            #[cfg(any(test, feature = "test-support"))]
            Self::Mock(b) => b.vm_remove_device(system_id, device_id).await,
        }
    }

    async fn vmm_ping(&self, system_id: &str) -> Result<VmmPingResponse, BackendError> {
        match self {
            Self::CloudHypervisor(b) => b.vmm_ping(system_id).await,
            #[cfg(feature = "kubevirt")]
            Self::KubeVirt(b) => b.vmm_ping(system_id).await,
            #[cfg(feature = "qemu")]
            Self::Qemu(b) => b.vmm_ping(system_id).await,
            #[cfg(feature = "libvirt")]
            Self::Libvirt(b) => b.vmm_ping(system_id).await,
            #[cfg(any(test, feature = "test-support"))]
            Self::Mock(b) => b.vmm_ping(system_id).await,
        }
    }

    async fn vm_counters(&self, system_id: &str) -> Result<VmCounters, BackendError> {
        match self {
            Self::CloudHypervisor(b) => b.vm_counters(system_id).await,
            #[cfg(feature = "kubevirt")]
            Self::KubeVirt(b) => b.vm_counters(system_id).await,
            #[cfg(feature = "qemu")]
            Self::Qemu(b) => b.vm_counters(system_id).await,
            #[cfg(feature = "libvirt")]
            Self::Libvirt(b) => b.vm_counters(system_id).await,
            #[cfg(any(test, feature = "test-support"))]
            Self::Mock(b) => b.vm_counters(system_id).await,
        }
    }

    async fn vm_set_secure_boot(&self, system_id: &str, enabled: bool) -> Result<(), BackendError> {
        match self {
            Self::CloudHypervisor(b) => b.vm_set_secure_boot(system_id, enabled).await,
            #[cfg(feature = "kubevirt")]
            Self::KubeVirt(b) => b.vm_set_secure_boot(system_id, enabled).await,
            #[cfg(feature = "qemu")]
            Self::Qemu(b) => b.vm_set_secure_boot(system_id, enabled).await,
            #[cfg(feature = "libvirt")]
            Self::Libvirt(b) => b.vm_set_secure_boot(system_id, enabled).await,
            #[cfg(any(test, feature = "test-support"))]
            Self::Mock(b) => b.vm_set_secure_boot(system_id, enabled).await,
        }
    }

    async fn vm_serial_console(&self, system_id: &str) -> Result<SerialConsoleInfo, BackendError> {
        match self {
            Self::CloudHypervisor(b) => b.vm_serial_console(system_id).await,
            #[cfg(feature = "kubevirt")]
            Self::KubeVirt(b) => b.vm_serial_console(system_id).await,
            #[cfg(feature = "qemu")]
            Self::Qemu(b) => b.vm_serial_console(system_id).await,
            #[cfg(feature = "libvirt")]
            Self::Libvirt(b) => b.vm_serial_console(system_id).await,
            #[cfg(any(test, feature = "test-support"))]
            Self::Mock(b) => b.vm_serial_console(system_id).await,
        }
    }
}
