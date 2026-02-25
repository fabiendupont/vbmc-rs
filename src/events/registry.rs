#![allow(dead_code)]

pub const MSG_SYSTEM_POWER_ON: &str = "ResourceEvent.1.0.ResourcePowerStateChanged";
pub const MSG_SYSTEM_POWER_OFF: &str = "ResourceEvent.1.0.ResourcePowerStateChanged";
pub const MSG_SYSTEM_RESET: &str = "ResourceEvent.1.0.ResourcePowerStateChanged";
pub const MSG_VIRTUAL_MEDIA_INSERTED: &str = "ResourceEvent.1.0.ResourceChanged";
pub const MSG_VIRTUAL_MEDIA_EJECTED: &str = "ResourceEvent.1.0.ResourceChanged";
pub const MSG_SESSION_CREATED: &str = "Security.1.0.SessionCreated";
pub const MSG_SESSION_TERMINATED: &str = "Security.1.0.SessionTerminated";
pub const MSG_AUTH_FAILURE: &str = "Security.1.0.AuthenticationFailure";
pub const MSG_ACCOUNT_LOCKED: &str = "Security.1.0.AccountLocked";
pub const MSG_ATTESTATION_CHANGED: &str = "ComponentIntegrity.1.0.SPDMVerificationStatusChanged";
pub const MSG_BOOT_OVERRIDE_SET: &str = "ResourceEvent.1.0.ResourceChanged";
pub const MSG_CERTIFICATE_REPLACED: &str = "Security.1.0.CertificateReplaced";

pub const SEVERITY_OK: &str = "OK";
pub const SEVERITY_WARNING: &str = "Warning";
pub const SEVERITY_CRITICAL: &str = "Critical";

pub const EVENT_TYPE_STATUS_CHANGE: &str = "StatusChange";
pub const EVENT_TYPE_RESOURCE_UPDATED: &str = "ResourceUpdated";
pub const EVENT_TYPE_RESOURCE_ADDED: &str = "ResourceAdded";
pub const EVENT_TYPE_RESOURCE_REMOVED: &str = "ResourceRemoved";
pub const EVENT_TYPE_ALERT: &str = "Alert";
