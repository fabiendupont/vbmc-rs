pub mod account_service;
pub mod certificate_service;
pub mod component_integrity;
pub mod error;
pub mod ethernet;
pub mod event_service;
pub mod managers;
pub mod power;
pub mod processors;
pub mod secure_boot;
pub mod security_policy;
pub mod service_root;
pub mod session_service;
pub mod storage;
pub mod systems;
pub mod task_service;
pub mod telemetry;
pub mod trusted_component;
pub mod types;
pub mod virtual_media;

use std::sync::Arc;

use axum::routing::{delete, get, post};
use axum::Router;

use crate::app_state::AppState;

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        // Redfish root
        .route("/redfish", get(service_root::get_redfish_root))
        .route("/redfish/v1", get(service_root::get_service_root))
        .route("/redfish/v1/", get(service_root::get_service_root))
        // Systems
        .route("/redfish/v1/Systems", get(systems::get_systems))
        .route(
            "/redfish/v1/Systems/{system_id}",
            get(systems::get_system).patch(systems::patch_system),
        )
        // Power actions
        .route(
            "/redfish/v1/Systems/{system_id}/Actions/ComputerSystem.Reset",
            post(power::reset_system),
        )
        // Virtual Media
        .route(
            "/redfish/v1/Systems/{system_id}/VirtualMedia",
            get(virtual_media::get_virtual_media_collection),
        )
        .route(
            "/redfish/v1/Systems/{system_id}/VirtualMedia/{media_id}",
            get(virtual_media::get_virtual_media),
        )
        .route(
            "/redfish/v1/Systems/{system_id}/VirtualMedia/{media_id}/Actions/VirtualMedia.InsertMedia",
            post(virtual_media::insert_media),
        )
        .route(
            "/redfish/v1/Systems/{system_id}/VirtualMedia/{media_id}/Actions/VirtualMedia.EjectMedia",
            post(virtual_media::eject_media),
        )
        // Ethernet Interfaces
        .route(
            "/redfish/v1/Systems/{system_id}/EthernetInterfaces",
            get(ethernet::get_ethernet_interfaces),
        )
        .route(
            "/redfish/v1/Systems/{system_id}/EthernetInterfaces/{nic_id}",
            get(ethernet::get_ethernet_interface),
        )
        // Processors
        .route(
            "/redfish/v1/Systems/{system_id}/Processors",
            get(processors::get_processors),
        )
        .route(
            "/redfish/v1/Systems/{system_id}/Processors/{processor_id}",
            get(processors::get_processor),
        )
        // Storage
        .route(
            "/redfish/v1/Systems/{system_id}/SimpleStorage",
            get(storage::get_simple_storage_collection),
        )
        .route(
            "/redfish/v1/Systems/{system_id}/SimpleStorage/{storage_id}",
            get(storage::get_simple_storage),
        )
        // Secure Boot
        .route(
            "/redfish/v1/Systems/{system_id}/SecureBoot",
            get(secure_boot::get_secure_boot).patch(secure_boot::patch_secure_boot),
        )
        // Managers
        .route("/redfish/v1/Managers", get(managers::get_managers))
        .route(
            "/redfish/v1/Managers/{manager_id}",
            get(managers::get_manager),
        )
        // Task Service
        .route(
            "/redfish/v1/TaskService",
            get(task_service::get_task_service),
        )
        .route(
            "/redfish/v1/TaskService/Tasks",
            get(task_service::get_tasks),
        )
        .route(
            "/redfish/v1/TaskService/Tasks/{task_id}",
            get(task_service::get_task),
        )
        .route(
            "/redfish/v1/TaskService/TaskMonitors/{task_id}",
            get(task_service::get_task_monitor),
        )
        // Session Service
        .route(
            "/redfish/v1/SessionService",
            get(session_service::get_session_service),
        )
        .route(
            "/redfish/v1/SessionService/Sessions",
            get(session_service::get_sessions).post(session_service::create_session),
        )
        .route(
            "/redfish/v1/SessionService/Sessions/{session_id}",
            delete(session_service::delete_session),
        )
        // Account Service
        .route(
            "/redfish/v1/AccountService",
            get(account_service::get_account_service),
        )
        .route(
            "/redfish/v1/AccountService/Accounts",
            get(account_service::get_accounts).post(account_service::create_account),
        )
        .route(
            "/redfish/v1/AccountService/Accounts/{account_id}",
            get(account_service::get_account),
        )
        .route(
            "/redfish/v1/AccountService/Roles",
            get(account_service::get_roles),
        )
        .route(
            "/redfish/v1/AccountService/Roles/{role_id}",
            get(account_service::get_role),
        )
        // Event Service
        .route(
            "/redfish/v1/EventService",
            get(event_service::get_event_service),
        )
        .route(
            "/redfish/v1/EventService/Subscriptions",
            get(event_service::get_subscriptions).post(event_service::create_subscription),
        )
        .route(
            "/redfish/v1/EventService/Subscriptions/{sub_id}",
            get(event_service::get_subscription).delete(event_service::delete_subscription),
        )
        .route(
            "/redfish/v1/EventService/SSE",
            get(event_service::sse_stream),
        )
        // Certificate Service
        .route(
            "/redfish/v1/CertificateService",
            get(certificate_service::get_certificate_service),
        )
        .route(
            "/redfish/v1/CertificateService/CertificateLocations",
            get(certificate_service::get_certificate_locations),
        )
        // Security Policy
        .route(
            "/redfish/v1/SecurityPolicy",
            get(security_policy::get_security_policy).patch(security_policy::patch_security_policy),
        )
        // Chassis
        .route(
            "/redfish/v1/Chassis",
            get(trusted_component::get_chassis_collection),
        )
        .route(
            "/redfish/v1/Chassis/1",
            get(trusted_component::get_chassis),
        )
        .route(
            "/redfish/v1/Chassis/1/TrustedComponents",
            get(trusted_component::get_trusted_components),
        )
        .route(
            "/redfish/v1/Chassis/1/TrustedComponents/{component_id}",
            get(trusted_component::get_trusted_component),
        )
        // Component Integrity
        .route(
            "/redfish/v1/ComponentIntegrity",
            get(component_integrity::get_component_integrity_collection),
        )
        .route(
            "/redfish/v1/ComponentIntegrity/{system_id}",
            get(component_integrity::get_component_integrity),
        )
        // Telemetry Service
        .route(
            "/redfish/v1/TelemetryService",
            get(telemetry::get_telemetry_service),
        )
        .route(
            "/redfish/v1/TelemetryService/MetricDefinitions",
            get(telemetry::get_metric_definitions),
        )
        .route(
            "/redfish/v1/TelemetryService/MetricReports",
            get(telemetry::get_metric_reports),
        )
        .with_state(state)
}
