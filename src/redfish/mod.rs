pub mod account_service;
pub mod bios;
pub mod boot_options;
pub mod certificate_service;
pub mod chassis_power;
pub mod chassis_power_subsystem;
pub mod chassis_thermal;
pub mod chassis_thermal_subsystem;
pub mod compliance;
pub mod component_integrity;
pub mod error;
pub mod ethernet;
pub mod event_service;
pub mod license_service;
pub mod log_service;
pub mod manager_network;
pub mod managers;
pub mod memory;
pub mod memory_metrics;
pub mod network_adapter;
pub mod network_interfaces;
pub mod odata;
pub mod pcie;
pub mod power;
pub mod processor_metrics;
pub mod processors;
pub mod registries;
pub mod secure_boot;
pub mod security_policy;
pub mod sensors;
pub mod service_root;
pub mod session_service;
pub mod storage;
pub mod storage_controllers;
pub mod systems;
pub mod task_service;
pub mod telemetry;
pub mod trusted_component;
pub mod types;
pub mod update_service;
pub mod virtual_media;

use std::sync::Arc;

use axum::Router;
use axum::routing::{delete, get, post};

use crate::app_state::AppState;
use compliance::ODataComplianceLayer;

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        // Redfish root
        .route("/redfish", get(service_root::get_redfish_root))
        .route("/redfish/v1", get(service_root::get_service_root))
        .route("/redfish/v1/", get(service_root::get_service_root))
        // OData metadata and service document
        .route("/redfish/v1/$metadata", get(odata::get_metadata))
        .route("/redfish/v1/odata", get(odata::get_odata_service_document))
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
        // SimpleStorage
        .route(
            "/redfish/v1/Systems/{system_id}/SimpleStorage",
            get(storage::get_simple_storage_collection),
        )
        .route(
            "/redfish/v1/Systems/{system_id}/SimpleStorage/{storage_id}",
            get(storage::get_simple_storage),
        )
        // Memory
        .route(
            "/redfish/v1/Systems/{system_id}/Memory",
            get(memory::get_memory_collection),
        )
        .route(
            "/redfish/v1/Systems/{system_id}/Memory/{dimm_id}",
            get(memory::get_memory),
        )
        // Storage (full)
        .route(
            "/redfish/v1/Systems/{system_id}/Storage",
            get(storage_controllers::get_storage_collection),
        )
        .route(
            "/redfish/v1/Systems/{system_id}/Storage/{ctrl_id}",
            get(storage_controllers::get_storage),
        )
        .route(
            "/redfish/v1/Systems/{system_id}/Storage/{ctrl_id}/Drives/{drive_id}",
            get(storage_controllers::get_drive),
        )
        .route(
            "/redfish/v1/Systems/{system_id}/Storage/{ctrl_id}/Volumes",
            get(storage_controllers::get_volumes),
        )
        .route(
            "/redfish/v1/Systems/{system_id}/Storage/{ctrl_id}/Volumes/{vol_id}",
            get(storage_controllers::get_volume),
        )
        .route(
            "/redfish/v1/Systems/{system_id}/Storage/{ctrl_id}/Controllers",
            get(storage_controllers::get_controllers),
        )
        .route(
            "/redfish/v1/Systems/{system_id}/Storage/{ctrl_id}/Controllers/{controller_id}",
            get(storage_controllers::get_controller),
        )
        // PCIe Devices
        .route(
            "/redfish/v1/Systems/{system_id}/PCIeDevices",
            get(pcie::get_pcie_devices),
        )
        .route(
            "/redfish/v1/Systems/{system_id}/PCIeDevices/{dev_id}",
            get(pcie::get_pcie_device),
        )
        .route(
            "/redfish/v1/Systems/{system_id}/PCIeDevices/{dev_id}/PCIeFunctions",
            get(pcie::get_pcie_functions),
        )
        .route(
            "/redfish/v1/Systems/{system_id}/PCIeDevices/{dev_id}/PCIeFunctions/{func_id}",
            get(pcie::get_pcie_function),
        )
        // BIOS
        .route(
            "/redfish/v1/Systems/{system_id}/Bios",
            get(bios::get_bios),
        )
        .route(
            "/redfish/v1/Systems/{system_id}/Bios/Settings",
            get(bios::get_bios_settings).patch(bios::patch_bios_settings),
        )
        // LogServices (System)
        .route(
            "/redfish/v1/Systems/{system_id}/LogServices",
            get(log_service::get_system_log_services),
        )
        .route(
            "/redfish/v1/Systems/{system_id}/LogServices/{log_id}",
            get(log_service::get_system_log_service),
        )
        .route(
            "/redfish/v1/Systems/{system_id}/LogServices/{log_id}/Entries",
            get(log_service::get_system_log_entries),
        )
        // Network Interfaces
        .route(
            "/redfish/v1/Systems/{system_id}/NetworkInterfaces",
            get(network_interfaces::get_network_interfaces),
        )
        .route(
            "/redfish/v1/Systems/{system_id}/NetworkInterfaces/{nic_id}",
            get(network_interfaces::get_network_interface),
        )
        // Boot Options
        .route(
            "/redfish/v1/Systems/{system_id}/BootOptions",
            get(boot_options::get_boot_options),
        )
        .route(
            "/redfish/v1/Systems/{system_id}/BootOptions/{option_id}",
            get(boot_options::get_boot_option),
        )
        // Processor Metrics
        .route(
            "/redfish/v1/Systems/{system_id}/Processors/{processor_id}/ProcessorMetrics",
            get(processor_metrics::get_processor_metrics),
        )
        // Memory Metrics
        .route(
            "/redfish/v1/Systems/{system_id}/Memory/{dimm_id}/MemoryMetrics",
            get(memory_metrics::get_memory_metrics),
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
        // Manager Network Protocol
        .route(
            "/redfish/v1/Managers/vbmc/NetworkProtocol",
            get(manager_network::get_network_protocol),
        )
        // Manager Ethernet Interfaces
        .route(
            "/redfish/v1/Managers/vbmc/EthernetInterfaces",
            get(manager_network::get_manager_ethernet_interfaces),
        )
        .route(
            "/redfish/v1/Managers/vbmc/EthernetInterfaces/{nic_id}",
            get(manager_network::get_manager_ethernet_interface),
        )
        // Manager LogServices
        .route(
            "/redfish/v1/Managers/vbmc/LogServices",
            get(log_service::get_manager_log_services),
        )
        .route(
            "/redfish/v1/Managers/vbmc/LogServices/{log_id}",
            get(log_service::get_manager_log_service),
        )
        .route(
            "/redfish/v1/Managers/vbmc/LogServices/{log_id}/Entries",
            get(log_service::get_manager_log_entries),
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
            get(account_service::get_account).patch(account_service::patch_account),
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
        // Update Service
        .route(
            "/redfish/v1/UpdateService",
            get(update_service::get_update_service),
        )
        .route(
            "/redfish/v1/UpdateService/FirmwareInventory",
            get(update_service::get_firmware_inventory),
        )
        .route(
            "/redfish/v1/UpdateService/FirmwareInventory/{item_id}",
            get(update_service::get_firmware_inventory_item),
        )
        // License Service
        .route(
            "/redfish/v1/LicenseService",
            get(license_service::get_license_service),
        )
        .route(
            "/redfish/v1/LicenseService/Licenses",
            get(license_service::get_licenses).post(license_service::create_license),
        )
        .route(
            "/redfish/v1/LicenseService/Licenses/{license_id}",
            get(license_service::get_license),
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
            "/redfish/v1/Chassis/1/Power",
            get(chassis_power::get_power),
        )
        .route(
            "/redfish/v1/Chassis/1/Thermal",
            get(chassis_thermal::get_thermal),
        )
        // PowerSubsystem (modern)
        .route(
            "/redfish/v1/Chassis/1/PowerSubsystem",
            get(chassis_power_subsystem::get_power_subsystem),
        )
        .route(
            "/redfish/v1/Chassis/1/PowerSubsystem/PowerSupplies",
            get(chassis_power_subsystem::get_power_supplies),
        )
        .route(
            "/redfish/v1/Chassis/1/PowerSubsystem/PowerSupplies/0",
            get(chassis_power_subsystem::get_power_supply),
        )
        // ThermalSubsystem (modern)
        .route(
            "/redfish/v1/Chassis/1/ThermalSubsystem",
            get(chassis_thermal_subsystem::get_thermal_subsystem),
        )
        .route(
            "/redfish/v1/Chassis/1/ThermalSubsystem/ThermalMetrics",
            get(chassis_thermal_subsystem::get_thermal_metrics),
        )
        .route(
            "/redfish/v1/Chassis/1/ThermalSubsystem/Fans",
            get(chassis_thermal_subsystem::get_fans),
        )
        .route(
            "/redfish/v1/Chassis/1/ThermalSubsystem/Fans/0",
            get(chassis_thermal_subsystem::get_fan),
        )
        // Sensors
        .route(
            "/redfish/v1/Chassis/1/Sensors",
            get(sensors::get_sensors),
        )
        .route(
            "/redfish/v1/Chassis/1/Sensors/{sensor_id}",
            get(sensors::get_sensor),
        )
        .route(
            "/redfish/v1/Chassis/1/NetworkAdapters",
            get(network_adapter::get_network_adapters),
        )
        .route(
            "/redfish/v1/Chassis/1/NetworkAdapters/{adapter_id}",
            get(network_adapter::get_network_adapter),
        )
        .route(
            "/redfish/v1/Chassis/1/NetworkAdapters/{adapter_id}/NetworkDeviceFunctions",
            get(network_adapter::get_network_device_functions),
        )
        .route(
            "/redfish/v1/Chassis/1/NetworkAdapters/{adapter_id}/NetworkDeviceFunctions/{func_id}",
            get(network_adapter::get_network_device_function),
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
        // Registries
        .route(
            "/redfish/v1/Registries",
            get(registries::get_registries),
        )
        .route(
            "/redfish/v1/Registries/{registry_id}",
            get(registries::get_registry),
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
        .route(
            "/redfish/v1/TelemetryService/MetricDefinitions/{def_id}",
            get(telemetry::get_metric_definition),
        )
        .layer(ODataComplianceLayer)
        .layer(axum::middleware::from_fn(crate::telemetry::metrics_middleware))
        .with_state(state)
}
