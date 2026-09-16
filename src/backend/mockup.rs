use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use dashmap::DashMap;
use tracing::info;

use super::types as bt;
use super::{BackendError, VmmBackend};
use crate::twin::TwinConfig;

/// Filename of the optional twin binding sidecar in a mockup directory.
const TWIN_SIDECAR: &str = "twin.toml";

pub struct MockupStore {
    resources: DashMap<String, serde_json::Value>,
    /// Monotonic source of store-wide-unique Task IDs.
    next_task_id: AtomicU64,
    /// Digital-twin binding table. Empty unless a `twin.toml` sidecar is loaded,
    /// in which case `get()` resolves dynamic fields over the base resource.
    twin: TwinConfig,
}

impl MockupStore {
    pub fn generate(count: usize, port: u16, tls_enabled: bool) -> Self {
        let store = Self {
            resources: DashMap::new(),
            next_task_id: AtomicU64::new(1),
            twin: TwinConfig::empty(),
        };

        let mut members = Vec::new();
        for i in 1..=count {
            let id = format!("Server{i}");
            let uuid = uuid::Uuid::new_v5(
                &uuid::Uuid::NAMESPACE_DNS,
                format!("vbmc-rs-simulate-{i}").as_bytes(),
            );
            let serial = format!("VBMC{i:06}");
            let mac1 = format!("52:54:00:00:{:02x}:{:02x}", i / 256, i % 256);
            let mac2 = format!("52:54:00:01:{:02x}:{:02x}", i / 256, i % 256);

            store.resources.insert(
                format!("/redfish/v1/Systems/{id}"),
                serde_json::json!({
                    "@odata.id": format!("/redfish/v1/Systems/{id}"),
                    "@odata.type": "#ComputerSystem.v1_20_0.ComputerSystem",
                    "Id": id,
                    "Name": format!("Simulated Server {i}"),
                    "SystemType": "Physical",
                    "Manufacturer": "vbmc-rs",
                    "Model": "Virtual Server 1U",
                    "SerialNumber": serial,
                    "UUID": uuid.to_string(),
                    "PowerState": "Off",
                    "BiosVersion": "vbmc-rs 0.1.0",
                    "ProcessorSummary": {
                        "Count": 2,
                        "Model": "Virtual CPU",
                        "LogicalProcessorCount": 4,
                        "Status": {"State": "Enabled", "Health": "OK"}
                    },
                    "MemorySummary": {
                        "TotalSystemMemoryGiB": 64,
                        "Status": {"State": "Enabled", "Health": "OK"}
                    },
                    "Boot": {
                        "BootSourceOverrideTarget": "None",
                        "BootSourceOverrideEnabled": "Disabled",
                        "BootSourceOverrideMode": "UEFI",
                        "BootSourceOverrideTarget@Redfish.AllowableValues": ["None", "Pxe", "Hdd", "Cd"]
                    },
                    "Status": {"State": "Enabled", "Health": "OK"},
                    "Actions": {
                        "#ComputerSystem.Reset": {
                            "target": format!("/redfish/v1/Systems/{id}/Actions/ComputerSystem.Reset"),
                            "ResetType@Redfish.AllowableValues": ["On", "ForceOff", "GracefulShutdown", "GracefulRestart", "ForceRestart", "PushPowerButton"]
                        }
                    },
                    "Processors": {"@odata.id": format!("/redfish/v1/Systems/{id}/Processors")},
                    "Memory": {"@odata.id": format!("/redfish/v1/Systems/{id}/Memory")},
                    "EthernetInterfaces": {"@odata.id": format!("/redfish/v1/Systems/{id}/EthernetInterfaces")},
                    "Storage": {"@odata.id": format!("/redfish/v1/Systems/{id}/Storage")},
                    "Bios": {"@odata.id": format!("/redfish/v1/Systems/{id}/Bios")},
                    "SecureBoot": {"@odata.id": format!("/redfish/v1/Systems/{id}/SecureBoot")}
                }),
            );

            store.resources.insert(
                format!("/redfish/v1/Systems/{id}/Processors"),
                serde_json::json!({
                    "@odata.id": format!("/redfish/v1/Systems/{id}/Processors"),
                    "@odata.type": "#ProcessorCollection.ProcessorCollection",
                    "Name": "Processor Collection",
                    "Members": [{"@odata.id": format!("/redfish/v1/Systems/{id}/Processors/CPU0")}],
                    "Members@odata.count": 1
                }),
            );

            store.resources.insert(
                format!("/redfish/v1/Systems/{id}/Processors/CPU0"),
                serde_json::json!({
                    "@odata.id": format!("/redfish/v1/Systems/{id}/Processors/CPU0"),
                    "@odata.type": "#Processor.v1_18_0.Processor",
                    "Id": "CPU0",
                    "Name": "CPU 0",
                    "ProcessorType": "CPU",
                    "TotalCores": 2,
                    "TotalThreads": 4,
                    "MaxSpeedMHz": 3600,
                    "Manufacturer": "vbmc-rs",
                    "Model": "Virtual CPU",
                    "InstructionSet": "x86-64",
                    "Status": {"State": "Enabled", "Health": "OK"}
                }),
            );

            store.resources.insert(
                format!("/redfish/v1/Systems/{id}/Memory"),
                serde_json::json!({
                    "@odata.id": format!("/redfish/v1/Systems/{id}/Memory"),
                    "@odata.type": "#MemoryCollection.MemoryCollection",
                    "Name": "Memory Collection",
                    "Members": [{"@odata.id": format!("/redfish/v1/Systems/{id}/Memory/DIMM0")}],
                    "Members@odata.count": 1
                }),
            );

            store.resources.insert(
                format!("/redfish/v1/Systems/{id}/Memory/DIMM0"),
                serde_json::json!({
                    "@odata.id": format!("/redfish/v1/Systems/{id}/Memory/DIMM0"),
                    "@odata.type": "#Memory.v1_16_0.Memory",
                    "Id": "DIMM0",
                    "Name": "DIMM 0",
                    "CapacityMiB": 65536,
                    "MemoryDeviceType": "DDR5",
                    "DataWidthBits": 64,
                    "OperatingSpeedMhz": 4800,
                    "Manufacturer": "Virtual",
                    "Status": {"State": "Enabled", "Health": "OK"}
                }),
            );

            store.resources.insert(
                format!("/redfish/v1/Systems/{id}/EthernetInterfaces"),
                serde_json::json!({
                    "@odata.id": format!("/redfish/v1/Systems/{id}/EthernetInterfaces"),
                    "@odata.type": "#EthernetInterfaceCollection.EthernetInterfaceCollection",
                    "Name": "Ethernet Interface Collection",
                    "Members": [
                        {"@odata.id": format!("/redfish/v1/Systems/{id}/EthernetInterfaces/NIC0")},
                        {"@odata.id": format!("/redfish/v1/Systems/{id}/EthernetInterfaces/NIC1")}
                    ],
                    "Members@odata.count": 2
                }),
            );

            store.resources.insert(
                format!("/redfish/v1/Systems/{id}/EthernetInterfaces/NIC0"),
                serde_json::json!({
                    "@odata.id": format!("/redfish/v1/Systems/{id}/EthernetInterfaces/NIC0"),
                    "@odata.type": "#EthernetInterface.v1_9_0.EthernetInterface",
                    "Id": "NIC0",
                    "Name": "Ethernet Interface 0",
                    "MACAddress": mac1,
                    "SpeedMbps": 25000,
                    "Status": {"State": "Enabled", "Health": "OK"}
                }),
            );

            store.resources.insert(
                format!("/redfish/v1/Systems/{id}/EthernetInterfaces/NIC1"),
                serde_json::json!({
                    "@odata.id": format!("/redfish/v1/Systems/{id}/EthernetInterfaces/NIC1"),
                    "@odata.type": "#EthernetInterface.v1_9_0.EthernetInterface",
                    "Id": "NIC1",
                    "Name": "Ethernet Interface 1",
                    "MACAddress": mac2,
                    "SpeedMbps": 25000,
                    "Status": {"State": "Enabled", "Health": "OK"}
                }),
            );

            store.resources.insert(
                format!("/redfish/v1/Systems/{id}/Storage"),
                serde_json::json!({
                    "@odata.id": format!("/redfish/v1/Systems/{id}/Storage"),
                    "@odata.type": "#StorageCollection.StorageCollection",
                    "Name": "Storage Collection",
                    "Members": [{"@odata.id": format!("/redfish/v1/Systems/{id}/Storage/NVMe")}],
                    "Members@odata.count": 1
                }),
            );

            store.resources.insert(
                format!("/redfish/v1/Systems/{id}/Storage/NVMe"),
                serde_json::json!({
                    "@odata.id": format!("/redfish/v1/Systems/{id}/Storage/NVMe"),
                    "@odata.type": "#Storage.v1_15_0.Storage",
                    "Id": "NVMe",
                    "Name": "NVMe Storage",
                    "Status": {"State": "Enabled", "Health": "OK"},
                    "Drives": [
                        {"@odata.id": format!("/redfish/v1/Systems/{id}/Storage/NVMe/Drives/0")}
                    ],
                    "Drives@odata.count": 1,
                    "StorageControllers": [
                        {
                            "@odata.id": format!("/redfish/v1/Systems/{id}/Storage/NVMe#/StorageControllers/0"),
                            "MemberId": "0",
                            "Name": "NVMe Controller",
                            "Manufacturer": "vbmc-rs",
                            "Model": "Virtual NVMe Controller",
                            "SupportedDeviceProtocols": ["NVMe"],
                            "Status": {"State": "Enabled", "Health": "OK"}
                        }
                    ]
                }),
            );

            store.resources.insert(
                format!("/redfish/v1/Systems/{id}/Storage/NVMe/Drives/0"),
                serde_json::json!({
                    "@odata.id": format!("/redfish/v1/Systems/{id}/Storage/NVMe/Drives/0"),
                    "@odata.type": "#Drive.v1_18_0.Drive",
                    "Id": "0",
                    "Name": "NVMe Drive 0",
                    "MediaType": "SSD",
                    "Protocol": "NVMe",
                    "CapacityBytes": 512_110_190_592_i64,
                    "Manufacturer": "vbmc-rs",
                    "Model": "Virtual NVMe SSD",
                    "SerialNumber": "VBMC-NVME-0000",
                    "Status": {"State": "Enabled", "Health": "OK"}
                }),
            );

            store.resources.insert(
                format!("/redfish/v1/Systems/{id}/SecureBoot"),
                serde_json::json!({
                    "@odata.id": format!("/redfish/v1/Systems/{id}/SecureBoot"),
                    "@odata.type": "#SecureBoot.v1_1_0.SecureBoot",
                    "Id": "SecureBoot",
                    "Name": "UEFI Secure Boot",
                    "SecureBootEnable": false,
                    "SecureBootCurrentBoot": "Disabled",
                    "SecureBootMode": "UserMode"
                }),
            );

            let bios_attrs = serde_json::json!({
                "BootMode": "UEFI",
                "NumCores": 4,
                "HyperThreadingEnabled": true,
                "VirtualizationEnabled": true,
                "SecureBootState": "Disabled"
            });
            store.resources.insert(
                format!("/redfish/v1/Systems/{id}/Bios"),
                serde_json::json!({
                    "@odata.id": format!("/redfish/v1/Systems/{id}/Bios"),
                    "@odata.type": "#Bios.v1_2_0.Bios",
                    "Id": "Bios",
                    "Name": "BIOS Configuration",
                    "Attributes": bios_attrs,
                    "@Redfish.Settings": {
                        "SettingsObject": {"@odata.id": format!("/redfish/v1/Systems/{id}/Bios/Settings")}
                    },
                    "Actions": {
                        "#Bios.ResetBios": {
                            "target": format!("/redfish/v1/Systems/{id}/Bios/Actions/Bios.ResetBios")
                        }
                    }
                }),
            );
            store.resources.insert(
                format!("/redfish/v1/Systems/{id}/Bios/Settings"),
                serde_json::json!({
                    "@odata.id": format!("/redfish/v1/Systems/{id}/Bios/Settings"),
                    "@odata.type": "#Bios.v1_2_0.Bios",
                    "Id": "Settings",
                    "Name": "BIOS Pending Settings",
                    "Attributes": {}
                }),
            );

            // GPU chassis for systems with a GPU index
            let gpu_id = format!("GPU{}", i - 1);
            let gpu_sensor_path = format!("/redfish/v1/Chassis/{gpu_id}/Sensors");
            store.resources.insert(
                format!("/redfish/v1/Chassis/{gpu_id}"),
                serde_json::json!({
                    "@odata.id": format!("/redfish/v1/Chassis/{gpu_id}"),
                    "@odata.type": "#Chassis.v1_24_0.Chassis",
                    "Id": gpu_id,
                    "Name": format!("Simulated GPU {}", i - 1),
                    "ChassisType": "Card",
                    "Manufacturer": "vbmc-rs",
                    "Model": "Virtual GPU",
                    "Status": {"State": "Enabled", "Health": "OK"},
                    "Sensors": {"@odata.id": gpu_sensor_path}
                }),
            );
            store.resources.insert(
                format!("/redfish/v1/Chassis/{gpu_id}/Sensors"),
                serde_json::json!({
                    "@odata.id": format!("/redfish/v1/Chassis/{gpu_id}/Sensors"),
                    "@odata.type": "#SensorCollection.SensorCollection",
                    "Name": "GPU Sensor Collection",
                    "Members@odata.count": 2,
                    "Members": [
                        {
                            "@odata.id": format!("/redfish/v1/Chassis/{gpu_id}/Sensors/Temp0"),
                            "@odata.type": "#Sensor.v1_6_0.Sensor",
                            "Id": "Temp0",
                            "Name": "GPU Temperature",
                            "PhysicalContext": "GPU",
                            "Reading": 65.0_f64,
                            "ReadingType": "Temperature",
                            "ReadingUnits": "Cel",
                            "Status": {"State": "Enabled", "Health": "OK"}
                        },
                        {
                            "@odata.id": format!("/redfish/v1/Chassis/{gpu_id}/Sensors/Power0"),
                            "@odata.type": "#Sensor.v1_6_0.Sensor",
                            "Id": "Power0",
                            "Name": "GPU Power",
                            "PhysicalContext": "GPUSubsystem",
                            "Reading": 150.0_f64,
                            "ReadingType": "Power",
                            "ReadingUnits": "W",
                            "Status": {"State": "Enabled", "Health": "OK"}
                        }
                    ]
                }),
            );
            // Serve the individual sensor members advertised above.
            store.resources.insert(
                format!("/redfish/v1/Chassis/{gpu_id}/Sensors/Temp0"),
                serde_json::json!({
                    "@odata.id": format!("/redfish/v1/Chassis/{gpu_id}/Sensors/Temp0"),
                    "@odata.type": "#Sensor.v1_6_0.Sensor",
                    "Id": "Temp0",
                    "Name": "GPU Temperature",
                    "PhysicalContext": "GPU",
                    "Reading": 65.0_f64,
                    "ReadingType": "Temperature",
                    "ReadingUnits": "Cel",
                    "Status": {"State": "Enabled", "Health": "OK"}
                }),
            );
            store.resources.insert(
                format!("/redfish/v1/Chassis/{gpu_id}/Sensors/Power0"),
                serde_json::json!({
                    "@odata.id": format!("/redfish/v1/Chassis/{gpu_id}/Sensors/Power0"),
                    "@odata.type": "#Sensor.v1_6_0.Sensor",
                    "Id": "Power0",
                    "Name": "GPU Power",
                    "PhysicalContext": "GPUSubsystem",
                    "Reading": 150.0_f64,
                    "ReadingType": "Power",
                    "ReadingUnits": "W",
                    "Status": {"State": "Enabled", "Health": "OK"}
                }),
            );

            // SecureBoot database collections
            for db in &["db", "kek"] {
                store.resources.insert(
                    format!("/redfish/v1/Systems/{id}/SecureBoot/SecureBootDatabases/{db}"),
                    serde_json::json!({
                        "@odata.id": format!("/redfish/v1/Systems/{id}/SecureBoot/SecureBootDatabases/{db}"),
                        "@odata.type": "#SecureBootDatabase.v1_0_0.SecureBootDatabase",
                        "Id": db,
                        "Name": format!("Secure Boot {}", db.to_uppercase()),
                        "Certificates": {
                            "@odata.id": format!("/redfish/v1/Systems/{id}/SecureBoot/SecureBootDatabases/{db}/Certificates")
                        }
                    }),
                );
                store.resources.insert(
                    format!("/redfish/v1/Systems/{id}/SecureBoot/SecureBootDatabases/{db}/Certificates"),
                    serde_json::json!({
                        "@odata.id": format!("/redfish/v1/Systems/{id}/SecureBoot/SecureBootDatabases/{db}/Certificates"),
                        "@odata.type": "#CertificateCollection.CertificateCollection",
                        "Name": "Certificate Collection",
                        "Members": [],
                        "Members@odata.count": 0
                    }),
                );
            }
            store.resources.insert(
                format!("/redfish/v1/Systems/{id}/SecureBoot/SecureBootDatabases"),
                serde_json::json!({
                    "@odata.id": format!("/redfish/v1/Systems/{id}/SecureBoot/SecureBootDatabases"),
                    "@odata.type": "#SecureBootDatabaseCollection.SecureBootDatabaseCollection",
                    "Name": "SecureBoot Database Collection",
                    "Members": [
                        {"@odata.id": format!("/redfish/v1/Systems/{id}/SecureBoot/SecureBootDatabases/db")},
                        {"@odata.id": format!("/redfish/v1/Systems/{id}/SecureBoot/SecureBootDatabases/kek")}
                    ],
                    "Members@odata.count": 2
                }),
            );

            members.push(serde_json::json!({"@odata.id": format!("/redfish/v1/Systems/{id}")}));
        }

        store.resources.insert(
            "/redfish/v1/Systems".to_string(),
            serde_json::json!({
                "@odata.id": "/redfish/v1/Systems",
                "@odata.type": "#ComputerSystemCollection.ComputerSystemCollection",
                "Name": "Computer System Collection",
                "Members": members,
                "Members@odata.count": count
            }),
        );

        let bmc_mac = "52:54:00:ff:00:01";

        store.resources.insert(
            "/redfish/v1/Managers".to_string(),
            serde_json::json!({
                "@odata.id": "/redfish/v1/Managers",
                "@odata.type": "#ManagerCollection.ManagerCollection",
                "Name": "Manager Collection",
                "Members": [{"@odata.id": "/redfish/v1/Managers/vbmc"}],
                "Members@odata.count": 1
            }),
        );

        store.resources.insert(
            "/redfish/v1/Managers/vbmc".to_string(),
            serde_json::json!({
                "@odata.id": "/redfish/v1/Managers/vbmc",
                "@odata.type": "#Manager.v1_18_0.Manager",
                "Id": "vbmc",
                "Name": "vbmc-rs Manager",
                "ManagerType": "BMC",
                "FirmwareVersion": "0.1.0",
                "Status": {"State": "Enabled", "Health": "OK"},
                "EthernetInterfaces": {"@odata.id": "/redfish/v1/Managers/vbmc/EthernetInterfaces"},
                "NetworkProtocol": {"@odata.id": "/redfish/v1/Managers/vbmc/NetworkProtocol"},
                "Actions": {
                    "#Manager.Reset": {
                        "target": "/redfish/v1/Managers/vbmc/Actions/Manager.Reset",
                        "ResetType@Redfish.AllowableValues": ["GracefulRestart", "ForceRestart"]
                    }
                }
            }),
        );

        store.resources.insert(
            "/redfish/v1/Managers/vbmc/NetworkProtocol".to_string(),
            serde_json::json!({
                "@odata.id": "/redfish/v1/Managers/vbmc/NetworkProtocol",
                "@odata.type": "#ManagerNetworkProtocol.v1_9_0.ManagerNetworkProtocol",
                "Id": "NetworkProtocol",
                "Name": "Manager Network Protocol",
                "Status": {"State": "Enabled", "Health": "OK"},
                // Reflect the transport the fleet actually listens on: HTTPS when a
                // TLS cert/key was supplied, plain HTTP otherwise. Advertising HTTPS
                // unconditionally would mislead clients that follow NetworkProtocol.
                "HTTP": {"ProtocolEnabled": !tls_enabled, "Port": if tls_enabled { 0 } else { port }},
                "HTTPS": {"ProtocolEnabled": tls_enabled, "Port": if tls_enabled { port } else { 0 }},
                "SSDP": {"ProtocolEnabled": false}
            }),
        );

        store.resources.insert(
            "/redfish/v1/Managers/vbmc/EthernetInterfaces".to_string(),
            serde_json::json!({
                "@odata.id": "/redfish/v1/Managers/vbmc/EthernetInterfaces",
                "@odata.type": "#EthernetInterfaceCollection.EthernetInterfaceCollection",
                "Name": "Ethernet Interface Collection",
                "Members": [{"@odata.id": "/redfish/v1/Managers/vbmc/EthernetInterfaces/eth0"}],
                "Members@odata.count": 1
            }),
        );

        store.resources.insert(
            "/redfish/v1/Managers/vbmc/EthernetInterfaces/eth0".to_string(),
            serde_json::json!({
                "@odata.id": "/redfish/v1/Managers/vbmc/EthernetInterfaces/eth0",
                "@odata.type": "#EthernetInterface.v1_9_0.EthernetInterface",
                "Id": "eth0",
                "Name": "BMC Ethernet Interface",
                "MACAddress": bmc_mac,
                "SpeedMbps": 1000,
                "Status": {"State": "Enabled", "Health": "OK"}
            }),
        );

        // Simulated TPM/SPDM attestation resources
        let fake_cert = "-----BEGIN CERTIFICATE-----\n\
            MIIBpTCCAUygAwIBAgIUVbmcrSEGsUeK7jLvl6GFAAAAAA0wCgYIKoZIzj0EAwIw\n\
            AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA==\n\
            -----END CERTIFICATE-----";
        let evidence_action_path = "/redfish/v1/ComponentIntegrity/TPM0/Actions/ComponentIntegrity.SPDMGetSignedMeasurements";
        let ca_cert_path = "/redfish/v1/ComponentIntegrity/TPM0/Certificates/0";

        store.resources.insert(
            "/redfish/v1/ComponentIntegrity".to_string(),
            serde_json::json!({
                "@odata.id": "/redfish/v1/ComponentIntegrity",
                "@odata.type": "#ComponentIntegrityCollection.ComponentIntegrityCollection",
                "Name": "Component Integrity Collection",
                "Members@odata.count": 1,
                "Members": [{"@odata.id": "/redfish/v1/ComponentIntegrity/TPM0"}]
            }),
        );

        store.resources.insert(
            "/redfish/v1/ComponentIntegrity/TPM0".to_string(),
            serde_json::json!({
                "@odata.id": "/redfish/v1/ComponentIntegrity/TPM0",
                "@odata.type": "#ComponentIntegrity.v1_2_0.ComponentIntegrity",
                "ComponentIntegrityEnabled": true,
                "ComponentIntegrityType": "SPDM",
                "ComponentIntegrityTypeVersion": "1.1",
                "Id": "TPM0",
                "Name": "Simulated TPM",
                "SPDM": {
                    "IdentityAuthentication": {
                        "ResponderAuthentication": {
                            "ComponentCertificate": {"@odata.id": ca_cert_path}
                        }
                    },
                    "Requester": {"@odata.id": "/redfish/v1/Managers/vbmc"}
                },
                "Actions": {
                    "#ComponentIntegrity.SPDMGetSignedMeasurements": {
                        "@Redfish.ActionInfo": "/redfish/v1/ComponentIntegrity/TPM0/SPDMGetSignedMeasurementsActionInfo",
                        "target": evidence_action_path
                    }
                }
            }),
        );

        // The component advertises an @Redfish.ActionInfo link for the SPDM
        // action; store it so clients that follow the link get a resource
        // instead of a 404 (mockup GETs only resolve stored paths).
        store.resources.insert(
            "/redfish/v1/ComponentIntegrity/TPM0/SPDMGetSignedMeasurementsActionInfo".to_string(),
            serde_json::json!({
                "@odata.id": "/redfish/v1/ComponentIntegrity/TPM0/SPDMGetSignedMeasurementsActionInfo",
                "@odata.type": "#ActionInfo.v1_4_2.ActionInfo",
                "Id": "SPDMGetSignedMeasurementsActionInfo",
                "Name": "SPDMGetSignedMeasurements Action Info",
                "Parameters": [
                    {
                        "Name": "MeasurementIndices",
                        "Required": false,
                        "DataType": "NumberArray"
                    },
                    {
                        "Name": "Nonce",
                        "Required": false,
                        "DataType": "String"
                    },
                    {
                        "Name": "SlotId",
                        "Required": false,
                        "DataType": "Number"
                    }
                ]
            }),
        );

        store.resources.insert(
            ca_cert_path.to_string(),
            serde_json::json!({
                "CertificateString": fake_cert,
                "CertificateType": "PEM",
                "CertificateUsageTypes": ["Platform"],
                "Id": "0",
                "Name": "TPM Certificate",
                "SPDM": {"SlotId": 0}
            }),
        );

        // GET evidence at action_path + /data
        store.resources.insert(
            format!("{evidence_action_path}/data"),
            serde_json::json!({
                "HashingAlgorithm": "SHA256",
                "SignedMeasurements": "AABBCCDDEEFF00112233445566778899AABBCCDDEEFF00112233445566778899",
                "SigningAlgorithm": "ECDSA_ECC_NIST_P256",
                "Version": "1.1"
            }),
        );

        let mut chassis_members = vec![serde_json::json!({"@odata.id": "/redfish/v1/Chassis/1"})];
        for i in 1..=count {
            chassis_members.push(serde_json::json!({
                "@odata.id": format!("/redfish/v1/Chassis/GPU{}", i - 1)
            }));
        }
        let chassis_count = chassis_members.len();
        store.resources.insert(
            "/redfish/v1/Chassis".to_string(),
            serde_json::json!({
                "@odata.id": "/redfish/v1/Chassis",
                "@odata.type": "#ChassisCollection.ChassisCollection",
                "Name": "Chassis Collection",
                "Members": chassis_members,
                "Members@odata.count": chassis_count
            }),
        );

        store.resources.insert(
            "/redfish/v1/Chassis/1".to_string(),
            serde_json::json!({
                "@odata.id": "/redfish/v1/Chassis/1",
                "@odata.type": "#Chassis.v1_24_0.Chassis",
                "Id": "1",
                "Name": "vbmc-rs Chassis",
                "ChassisType": "RackMount",
                "Manufacturer": "vbmc-rs",
                "Model": "Virtual Server 1U",
                "Status": {"State": "Enabled", "Health": "OK"},
                "Assembly": {"@odata.id": "/redfish/v1/Chassis/1/Assembly"}
            }),
        );

        store.resources.insert(
            "/redfish/v1/Chassis/1/Assembly".to_string(),
            serde_json::json!({
                "@odata.id": "/redfish/v1/Chassis/1/Assembly",
                "@odata.type": "#Assembly.v1_5_0.Assembly",
                "Id": "Assembly",
                "Name": "Chassis Assembly",
                "Assemblies": [
                    {
                        "@odata.id": "/redfish/v1/Chassis/1/Assembly#/Assemblies/0",
                        "MemberId": "0",
                        "Name": "Motherboard",
                        "Model": "Virtual Motherboard 1U",
                        "Manufacturer": "vbmc-rs",
                        "Status": {"State": "Enabled", "Health": "OK"}
                    }
                ]
            }),
        );

        store.resources.insert(
            "/redfish/v1".to_string(),
            serde_json::json!({
                "@odata.id": "/redfish/v1",
                "@odata.type": "#ServiceRoot.v1_16_0.ServiceRoot",
                "Id": "RootService",
                "Name": "vbmc-rs Simulated BMC",
                "RedfishVersion": "1.21.0",
                "Vendor": "vbmc-rs",
                "Product": "Virtual BMC",
                "UUID": uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_DNS, b"vbmc-rs-simulate").to_string(),
                "Systems": {"@odata.id": "/redfish/v1/Systems"},
                "Chassis": {"@odata.id": "/redfish/v1/Chassis"},
                "Managers": {"@odata.id": "/redfish/v1/Managers"},
                "AccountService": {"@odata.id": "/redfish/v1/AccountService"},
                "SessionService": {"@odata.id": "/redfish/v1/SessionService"},
                "ComponentIntegrity": {"@odata.id": "/redfish/v1/ComponentIntegrity"},
                "Links": {
                    "Sessions": {"@odata.id": "/redfish/v1/SessionService/Sessions"},
                    "ManagerProvidingService": {"@odata.id": "/redfish/v1/Managers/vbmc"}
                }
            }),
        );

        store.resources.insert(
            "/redfish".to_string(),
            serde_json::json!({"v1": "/redfish/v1"}),
        );

        store.resources.insert(
            "/redfish/v1/AccountService".to_string(),
            serde_json::json!({
                "@odata.id": "/redfish/v1/AccountService",
                "@odata.type": "#AccountService.v1_13_0.AccountService",
                "Id": "AccountService",
                "Name": "Account Service",
                "Accounts": {"@odata.id": "/redfish/v1/AccountService/Accounts"}
            }),
        );

        store.resources.insert(
            "/redfish/v1/AccountService/Accounts".to_string(),
            serde_json::json!({
                "@odata.id": "/redfish/v1/AccountService/Accounts",
                "@odata.type": "#ManagerAccountCollection.ManagerAccountCollection",
                "Name": "Accounts Collection",
                "Members": [],
                "Members@odata.count": 0
            }),
        );

        store.resources.insert(
            "/redfish/v1/SessionService".to_string(),
            serde_json::json!({
                "@odata.id": "/redfish/v1/SessionService",
                "@odata.type": "#SessionService.v1_1_9.SessionService",
                "Id": "SessionService",
                "Name": "Session Service",
                "ServiceEnabled": true,
                "SessionTimeout": 30,
                "Sessions": {"@odata.id": "/redfish/v1/SessionService/Sessions"}
            }),
        );

        store.resources.insert(
            "/redfish/v1/SessionService/Sessions".to_string(),
            serde_json::json!({
                "@odata.id": "/redfish/v1/SessionService/Sessions",
                "@odata.type": "#SessionCollection.SessionCollection",
                "Name": "Session Collection",
                "Members": [],
                "Members@odata.count": 0
            }),
        );

        info!(systems = count, "Generated simulated BMC fleet");
        store
    }

    pub fn load(dir: &Path) -> anyhow::Result<Self> {
        let mut store = Self {
            resources: DashMap::new(),
            next_task_id: AtomicU64::new(1),
            twin: TwinConfig::empty(),
        };
        let dir_str = dir
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("mockup directory path is not valid UTF-8"))?;

        let mut count = 0;
        for entry in walkdir(dir)? {
            let rel = entry
                .strip_prefix(dir_str)
                .unwrap_or(&entry)
                .trim_start_matches('/');

            let path = if let Some(stripped) = rel.strip_suffix("/index.json") {
                format!("/{stripped}")
            } else if rel == "index.json" {
                "/".to_string()
            } else {
                continue;
            };

            let content = std::fs::read_to_string(&entry)?;
            let json: serde_json::Value = serde_json::from_str(&content)?;
            store.resources.insert(path, json);
            count += 1;
        }

        // A loaded mockup may already contain TaskService tasks. Start the
        // counter past the highest existing id so freshly minted tasks never
        // overwrite loaded ones.
        let max_task_id = store
            .resources
            .iter()
            .filter_map(|e| {
                e.key()
                    .strip_prefix("/redfish/v1/TaskService/Tasks/")
                    .and_then(|s| s.parse::<u64>().ok())
            })
            .max();
        if let Some(max) = max_task_id {
            store.next_task_id.store(max + 1, Ordering::Relaxed);
        }

        // Optional digital-twin sidecar: binds dynamic fields (formulas now,
        // external feed later) over the loaded base resources. Absent by
        // default, in which case `get()` stays byte-identical to the fixture.
        let twin_path = dir.join(TWIN_SIDECAR);
        if twin_path.is_file() {
            let text = std::fs::read_to_string(&twin_path)?;
            store.twin = TwinConfig::from_toml(&text)?;
            // Bake any scenario `level` targets against the loaded sensors'
            // static Thresholds, so scenario evaluation stays pure of the base.
            let resources = &store.resources;
            store
                .twin
                .bind_scenarios(|path| resources.get(path).map(|v| v.clone()))?;
            info!(sidecar = %twin_path.display(), "Loaded twin bindings");
        }

        info!(directory = %dir.display(), resources = count, "Loaded mockup data");
        Ok(store)
    }

    /// Empty store for unit tests. Production code builds a store via `generate`
    /// or `load`; tests seed only the resources they exercise.
    #[cfg(test)]
    pub(crate) fn for_test() -> Self {
        Self {
            resources: DashMap::new(),
            next_task_id: AtomicU64::new(1),
            twin: TwinConfig::empty(),
        }
    }

    /// Empty store carrying a specific twin config, for tests that exercise the
    /// twin seam (bindings, ingest routing, fleet identity).
    #[cfg(test)]
    pub(crate) fn for_test_with_twin(twin: TwinConfig) -> Self {
        Self {
            resources: DashMap::new(),
            next_task_id: AtomicU64::new(1),
            twin,
        }
    }

    pub fn get(&self, path: &str) -> Option<serde_json::Value> {
        let mut value = self.resources.get(path).map(|v| v.clone())?;
        // With no bindings this is a no-op, so the returned clone is identical
        // to the stored resource (regression-safe seam).
        if !self.twin.is_empty() {
            self.twin.resolve(path, &mut value, Instant::now());
        }
        Some(value)
    }

    pub fn patch(&self, path: &str, patch: &serde_json::Value) {
        if let Some(mut entry) = self.resources.get_mut(path) {
            merge_json(entry.value_mut(), patch);
        }
    }

    /// Insert or overwrite a resource wholesale (unlike `patch`, which deep-merges).
    pub fn set(&self, path: &str, value: serde_json::Value) {
        self.resources.insert(path.to_string(), value);
    }

    /// Whether a resource exists at `path`.
    pub fn contains(&self, path: &str) -> bool {
        self.resources.contains_key(path)
    }

    /// Atomically append a member to a collection: under a single entry lock,
    /// reserve the next 1-based id, build the member with `build`, push it, and
    /// bump `Members@odata.count`. Returns the reserved id, or `None` if the
    /// collection is missing or malformed. Avoids the read-then-write race of
    /// computing the id from a separate `get`.
    pub fn append_member<F>(&self, collection_path: &str, build: F) -> Option<u64>
    where
        F: FnOnce(u64) -> serde_json::Value,
    {
        let mut entry = self.resources.get_mut(collection_path)?;
        let obj = entry.value_mut().as_object_mut()?;
        let id = obj
            .get("Members@odata.count")
            .and_then(|v| v.as_u64())
            .unwrap_or(0)
            + 1;
        let member = build(id);
        obj.get_mut("Members")
            .and_then(|m| m.as_array_mut())?
            .push(member);
        obj.insert("Members@odata.count".to_string(), serde_json::json!(id));
        Some(id)
    }

    /// Stream-out tick cadence, or `None` when there are no twin bindings (in
    /// which case the stream loop is not spawned and behaviour is unchanged).
    pub fn twin_stream_interval(&self) -> Option<std::time::Duration> {
        if self.twin.is_empty() {
            None
        } else {
            Some(self.twin.tick_interval())
        }
    }

    /// Resolve every dynamic twin binding at `now` for the stream-out loop.
    pub fn twin_stream_snapshot(&self, now: Instant) -> Vec<crate::twin::StreamSample> {
        self.twin.stream_snapshot(now)
    }

    /// Record an external-twin reading for `key` (from `POST /twin/v1/state`).
    pub fn twin_ingest(&self, key: &str, value: serde_json::Value) {
        self.twin.ingest(key, value);
    }

    /// Whether any twin binding consumes external readings for `key`.
    pub fn twin_has_external_key(&self, key: &str) -> bool {
        self.twin.has_external_key(key)
    }

    /// The twin's control-intent webhook URL, if actuation is configured.
    pub fn twin_control_webhook(&self) -> Option<String> {
        self.twin.control_webhook().map(str::to_string)
    }

    /// This node's fleet identity (`[twin] system_id`), if configured (P5).
    pub fn twin_system_id(&self) -> Option<String> {
        self.twin.system_id().map(str::to_string)
    }

    /// Whether a twin sample addressed to `target` should be ingested here (P5).
    pub fn twin_accepts_system_id(&self, target: Option<&str>) -> bool {
        self.twin.accepts_system_id(target)
    }

    /// Store-wide-unique, monotonically increasing Task id.
    pub fn next_task_id(&self) -> u64 {
        self.next_task_id.fetch_add(1, Ordering::Relaxed)
    }

    pub fn system_ids(&self) -> Vec<String> {
        let mut ids = Vec::new();
        for entry in self.resources.iter() {
            let path = entry.key();
            if let Some(rest) = path.strip_prefix("/redfish/v1/Systems/")
                && !rest.contains('/')
                && !rest.is_empty()
            {
                ids.push(rest.to_string());
            }
        }
        ids
    }

    fn system_path(&self, system_id: &str) -> String {
        format!("/redfish/v1/Systems/{system_id}")
    }

    fn get_system(&self, system_id: &str) -> Option<serde_json::Value> {
        self.get(&self.system_path(system_id))
    }

    fn set_power_state(&self, system_id: &str, state: &str) {
        let path = self.system_path(system_id);
        if let Some(mut entry) = self.resources.get_mut(&path)
            && let Some(obj) = entry.value_mut().as_object_mut()
        {
            obj.insert("PowerState".to_string(), serde_json::json!(state));
        }
    }
}

fn merge_json(target: &mut serde_json::Value, patch: &serde_json::Value) {
    if let (Some(target_obj), Some(patch_obj)) = (target.as_object_mut(), patch.as_object()) {
        for (key, value) in patch_obj {
            if value.is_null() {
                target_obj.remove(key);
            } else if value.is_object() && target_obj.get(key).is_some_and(|v| v.is_object()) {
                merge_json(target_obj.get_mut(key).expect("checked above"), value);
            } else {
                target_obj.insert(key.clone(), value.clone());
            }
        }
    }
}

fn walkdir(dir: &Path) -> anyhow::Result<Vec<String>> {
    let mut files = Vec::new();
    walkdir_inner(dir, &mut files)?;
    Ok(files)
}

fn walkdir_inner(dir: &Path, files: &mut Vec<String>) -> anyhow::Result<()> {
    if !dir.is_dir() {
        anyhow::bail!("not a directory: {}", dir.display());
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            walkdir_inner(&path, files)?;
        } else if path.file_name().is_some_and(|n| n == "index.json")
            && let Some(s) = path.to_str()
        {
            files.push(s.to_string());
        }
    }
    Ok(())
}

pub struct MockupBackend {
    store: Arc<MockupStore>,
}

impl MockupBackend {
    pub fn new(store: Arc<MockupStore>) -> Self {
        Self { store }
    }

    pub fn store(&self) -> &Arc<MockupStore> {
        &self.store
    }
}

impl VmmBackend for MockupBackend {
    async fn vm_info(&self, system_id: &str) -> Result<bt::VmInfo, BackendError> {
        let sys = self
            .store
            .get_system(system_id)
            .ok_or(BackendError::VmNotFound)?;

        let power_state = match sys.get("PowerState").and_then(|v| v.as_str()) {
            Some("On") => bt::VmPowerState::On,
            Some("Off") | Some("GracefulShutdown") => bt::VmPowerState::Off,
            Some("Paused") => bt::VmPowerState::Paused,
            _ => bt::VmPowerState::Unknown,
        };

        let cpu_count = sys
            .pointer("/ProcessorSummary/Count")
            .and_then(|v| v.as_u64())
            .unwrap_or(1) as u32;

        let memory_gib = sys
            .pointer("/MemorySummary/TotalSystemMemoryGiB")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let memory_bytes = (memory_gib * 1024.0 * 1024.0 * 1024.0) as u64;

        let secure_boot = sys
            .pointer("/SecureBoot/SecureBootEnable")
            .and_then(|v| v.as_bool());

        let disks = self.extract_disks(system_id);
        let nics = self.extract_nics(system_id);

        Ok(bt::VmInfo {
            power_state,
            cpu_count,
            max_cpu_count: cpu_count,
            cpu_topology: None,
            memory_bytes,
            memory_actual_bytes: Some(memory_bytes),
            secure_boot,
            disks,
            nics,
            pci_devices: vec![],
            uuid: sys.get("UUID").and_then(|v| v.as_str()).map(String::from),
            raw: Some(sys),
        })
    }

    async fn vm_create(
        &self,
        _system_id: &str,
        _config: bt::VmCreateConfig,
    ) -> Result<(), BackendError> {
        Err(BackendError::NotSupported(
            "vm_create not supported in mockup backend".to_string(),
        ))
    }

    async fn vm_boot(&self, system_id: &str) -> Result<(), BackendError> {
        if self.store.get_system(system_id).is_none() {
            return Err(BackendError::VmNotFound);
        }
        self.store.set_power_state(system_id, "On");
        Ok(())
    }

    async fn vm_shutdown(&self, system_id: &str) -> Result<(), BackendError> {
        if self.store.get_system(system_id).is_none() {
            return Err(BackendError::VmNotFound);
        }
        self.store.set_power_state(system_id, "Off");
        Ok(())
    }

    async fn vm_delete(&self, _system_id: &str) -> Result<(), BackendError> {
        Err(BackendError::NotSupported(
            "vm_delete not supported in mockup backend".to_string(),
        ))
    }

    async fn vm_power_button(&self, system_id: &str) -> Result<(), BackendError> {
        if self.store.get_system(system_id).is_none() {
            return Err(BackendError::VmNotFound);
        }
        self.store.set_power_state(system_id, "Off");
        Ok(())
    }

    async fn vm_reboot(&self, system_id: &str) -> Result<(), BackendError> {
        if self.store.get_system(system_id).is_none() {
            return Err(BackendError::VmNotFound);
        }
        self.store.set_power_state(system_id, "On");
        Ok(())
    }

    async fn vm_add_disk(
        &self,
        _system_id: &str,
        _disk: bt::DiskCreateConfig,
    ) -> Result<(), BackendError> {
        Err(BackendError::NotSupported(
            "vm_add_disk not supported in mockup backend".to_string(),
        ))
    }

    async fn vm_remove_device(
        &self,
        _system_id: &str,
        _device_id: &str,
    ) -> Result<(), BackendError> {
        Err(BackendError::NotSupported(
            "vm_remove_device not supported in mockup backend".to_string(),
        ))
    }

    async fn vmm_ping(&self, _system_id: &str) -> Result<bt::VmmPingResponse, BackendError> {
        let version = self.store.get("/redfish/v1").and_then(|v| {
            v.get("RedfishVersion")
                .and_then(|v| v.as_str())
                .map(String::from)
        });
        Ok(bt::VmmPingResponse { version, pid: None })
    }

    async fn vm_counters(&self, _system_id: &str) -> Result<bt::VmCounters, BackendError> {
        Ok(bt::VmCounters::default())
    }

    async fn vm_set_secure_boot(&self, system_id: &str, enabled: bool) -> Result<(), BackendError> {
        let sb_path = format!("/redfish/v1/Systems/{system_id}/SecureBoot");
        self.store
            .patch(&sb_path, &serde_json::json!({"SecureBootEnable": enabled}));
        Ok(())
    }

    async fn vm_serial_console(
        &self,
        _system_id: &str,
    ) -> Result<bt::SerialConsoleInfo, BackendError> {
        Err(BackendError::NotSupported(
            "serial console not available in mockup backend".to_string(),
        ))
    }

    async fn vm_insert_iso(
        &self,
        _system_id: &str,
        _image_url: &str,
        _device_id: &str,
    ) -> Result<(), BackendError> {
        Err(BackendError::NotSupported("use download path".to_string()))
    }

    async fn vm_eject_iso(&self, _system_id: &str, _device_id: &str) -> Result<(), BackendError> {
        Err(BackendError::NotSupported(
            "use vm_remove_device".to_string(),
        ))
    }
}

impl MockupBackend {
    fn extract_disks(&self, system_id: &str) -> Vec<bt::DiskInfo> {
        let mut disks = Vec::new();
        let storage_path = format!("/redfish/v1/Systems/{system_id}/Storage");
        if let Some(collection) = self.store.get(&storage_path)
            && let Some(members) = collection.get("Members").and_then(|m| m.as_array())
        {
            for (i, member) in members.iter().enumerate() {
                let id = member
                    .get("@odata.id")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.rsplit('/').next())
                    .unwrap_or("disk")
                    .to_string();
                disks.push(bt::DiskInfo {
                    id: format!("{id}-{i}"),
                    path: None,
                    capacity_bytes: None,
                    readonly: false,
                    protocol: bt::DiskProtocol::Virtio,
                    media_type: bt::DiskMediaType::Virtual,
                });
            }
        }
        if disks.is_empty() {
            disks.push(bt::DiskInfo {
                id: "disk-0".to_string(),
                path: None,
                capacity_bytes: None,
                readonly: false,
                protocol: bt::DiskProtocol::Virtio,
                media_type: bt::DiskMediaType::Virtual,
            });
        }
        disks
    }

    fn extract_nics(&self, system_id: &str) -> Vec<bt::NicInfo> {
        let mut nics = Vec::new();
        let nic_path = format!("/redfish/v1/Systems/{system_id}/EthernetInterfaces");
        if let Some(collection) = self.store.get(&nic_path)
            && let Some(members) = collection.get("Members").and_then(|m| m.as_array())
        {
            for member in members {
                let id = member
                    .get("@odata.id")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.rsplit('/').next())
                    .unwrap_or("NIC0")
                    .to_string();
                nics.push(bt::NicInfo {
                    id,
                    mac_address: None,
                    tap: None,
                    speed_mbps: 0,
                });
            }
        }
        nics
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_mockup_dir() -> TempDir {
        let dir = TempDir::new().unwrap();

        let service_root = dir.path().join("redfish/v1");
        std::fs::create_dir_all(&service_root).unwrap();
        std::fs::write(
            service_root.join("index.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "@odata.id": "/redfish/v1",
                "@odata.type": "#ServiceRoot.v1_10_0.ServiceRoot",
                "RedfishVersion": "1.21.0",
                "Systems": {"@odata.id": "/redfish/v1/Systems"},
            }))
            .unwrap(),
        )
        .unwrap();

        let systems = dir.path().join("redfish/v1/Systems");
        std::fs::create_dir_all(&systems).unwrap();
        std::fs::write(
            systems.join("index.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "@odata.id": "/redfish/v1/Systems",
                "Members": [{"@odata.id": "/redfish/v1/Systems/Server1"}],
                "Members@odata.count": 1,
            }))
            .unwrap(),
        )
        .unwrap();

        let system1 = dir.path().join("redfish/v1/Systems/Server1");
        std::fs::create_dir_all(&system1).unwrap();
        std::fs::write(
            system1.join("index.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "@odata.id": "/redfish/v1/Systems/Server1",
                "@odata.type": "#ComputerSystem.v1_20_0.ComputerSystem",
                "Id": "Server1",
                "Name": "Test Server",
                "PowerState": "On",
                "SystemType": "Physical",
                "ProcessorSummary": {"Count": 2},
                "MemorySummary": {"TotalSystemMemoryGiB": 64.0},
                "UUID": "12345678-1234-1234-1234-123456789012",
            }))
            .unwrap(),
        )
        .unwrap();

        dir
    }

    #[test]
    fn test_load_mockup() {
        let dir = create_mockup_dir();
        let store = MockupStore::load(dir.path()).unwrap();

        assert!(store.get("/redfish/v1").is_some());
        assert!(store.get("/redfish/v1/Systems").is_some());
        assert!(store.get("/redfish/v1/Systems/Server1").is_some());
        assert!(store.get("/nonexistent").is_none());
    }

    #[test]
    fn test_twin_sidecar_resolves_formula_binding() {
        let dir = create_mockup_dir();

        // A sensor resource with a static base Reading.
        let sensor = dir.path().join("redfish/v1/Chassis/GPU_0/Sensors/Temp0");
        std::fs::create_dir_all(&sensor).unwrap();
        std::fs::write(
            sensor.join("index.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "@odata.id": "/redfish/v1/Chassis/GPU_0/Sensors/Temp0",
                "@odata.type": "#Sensor.v1_2_0.Sensor",
                "Name": "Temp0",
                "Reading": 0.0,
            }))
            .unwrap(),
        )
        .unwrap();

        // A twin.toml binding the sensor's Reading to a bounded formula.
        std::fs::write(
            dir.path().join(super::TWIN_SIDECAR),
            r#"
[twin]
[[twin.binding]]
path = "/redfish/v1/Chassis/GPU_0/Sensors/Temp0"
pointer = "/Reading"
source = "formula"
formula = { kind = "sine", min = 30.0, max = 90.0, period_s = 60 }
"#,
        )
        .unwrap();

        let store = MockupStore::load(dir.path()).unwrap();

        // The bound field is resolved into [min, max]; the base 0.0 is replaced.
        let sensor = store
            .get("/redfish/v1/Chassis/GPU_0/Sensors/Temp0")
            .unwrap();
        let reading = sensor["Reading"].as_f64().unwrap();
        assert!(
            (30.0..=90.0).contains(&reading),
            "resolved reading {reading} out of bounds"
        );
        // Unbound fields are untouched.
        assert_eq!(sensor["Name"], "Temp0");
    }

    #[test]
    fn test_no_twin_sidecar_is_byte_identical() {
        // Without a twin.toml, get() returns the stored resource verbatim.
        let dir = create_mockup_dir();
        let store = MockupStore::load(dir.path()).unwrap();
        let sys = store.get("/redfish/v1/Systems/Server1").unwrap();
        assert_eq!(sys["Name"], "Test Server");
        assert_eq!(sys["PowerState"], "On");
    }

    #[test]
    fn test_system_ids() {
        let dir = create_mockup_dir();
        let store = MockupStore::load(dir.path()).unwrap();
        let ids = store.system_ids();
        assert_eq!(ids, vec!["Server1"]);
    }

    #[test]
    fn test_set_power_state() {
        let dir = create_mockup_dir();
        let store = MockupStore::load(dir.path()).unwrap();

        let sys = store.get_system("Server1").unwrap();
        assert_eq!(sys["PowerState"], "On");

        store.set_power_state("Server1", "Off");
        let sys = store.get_system("Server1").unwrap();
        assert_eq!(sys["PowerState"], "Off");
    }

    #[test]
    fn test_patch() {
        let dir = create_mockup_dir();
        let store = MockupStore::load(dir.path()).unwrap();

        store.patch(
            "/redfish/v1/Systems/Server1",
            &serde_json::json!({"AssetTag": "MyTag"}),
        );
        let sys = store.get("/redfish/v1/Systems/Server1").unwrap();
        assert_eq!(sys["AssetTag"], "MyTag");
        assert_eq!(sys["PowerState"], "On");
    }

    #[test]
    fn test_merge_json_nested() {
        let mut target = serde_json::json!({"a": {"b": 1, "c": 2}, "d": 3});
        let patch = serde_json::json!({"a": {"b": 10}, "e": 5});
        merge_json(&mut target, &patch);
        assert_eq!(target["a"]["b"], 10);
        assert_eq!(target["a"]["c"], 2);
        assert_eq!(target["d"], 3);
        assert_eq!(target["e"], 5);
    }

    #[test]
    fn test_merge_json_delete() {
        let mut target = serde_json::json!({"a": 1, "b": 2});
        let patch = serde_json::json!({"b": null});
        merge_json(&mut target, &patch);
        assert_eq!(target["a"], 1);
        assert!(target.get("b").is_none());
    }

    #[tokio::test]
    async fn test_mockup_backend_vm_info() {
        let dir = create_mockup_dir();
        let store = Arc::new(MockupStore::load(dir.path()).unwrap());
        let backend = MockupBackend::new(store);

        let info = backend.vm_info("Server1").await.unwrap();
        assert_eq!(info.power_state, bt::VmPowerState::On);
        assert_eq!(info.cpu_count, 2);
        assert_eq!(info.memory_bytes, 64 * 1024 * 1024 * 1024);
        assert_eq!(
            info.uuid.as_deref(),
            Some("12345678-1234-1234-1234-123456789012")
        );
    }

    #[tokio::test]
    async fn test_mockup_backend_vm_not_found() {
        let dir = create_mockup_dir();
        let store = Arc::new(MockupStore::load(dir.path()).unwrap());
        let backend = MockupBackend::new(store);

        let result = backend.vm_info("Nonexistent").await;
        assert!(matches!(result, Err(BackendError::VmNotFound)));
    }

    #[tokio::test]
    async fn test_mockup_backend_boot_shutdown() {
        let dir = create_mockup_dir();
        let store = Arc::new(MockupStore::load(dir.path()).unwrap());
        let backend = MockupBackend::new(store);

        let info = backend.vm_info("Server1").await.unwrap();
        assert_eq!(info.power_state, bt::VmPowerState::On);

        backend.vm_shutdown("Server1").await.unwrap();
        let info = backend.vm_info("Server1").await.unwrap();
        assert_eq!(info.power_state, bt::VmPowerState::Off);

        backend.vm_boot("Server1").await.unwrap();
        let info = backend.vm_info("Server1").await.unwrap();
        assert_eq!(info.power_state, bt::VmPowerState::On);
    }

    #[tokio::test]
    async fn test_mockup_backend_vmm_ping() {
        let dir = create_mockup_dir();
        let store = Arc::new(MockupStore::load(dir.path()).unwrap());
        let backend = MockupBackend::new(store);

        let ping = backend.vmm_ping("Server1").await.unwrap();
        assert_eq!(ping.version.as_deref(), Some("1.21.0"));
    }
}
