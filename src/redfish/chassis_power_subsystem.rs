use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use serde::Serialize;

use super::types::{Collection, ODataId, Status};
use crate::app_state::AppState;
use crate::auth::AuthenticatedUser;

#[derive(Debug, Serialize)]
pub struct PowerSubsystemResource {
    #[serde(rename = "@odata.id")]
    pub odata_id: String,
    #[serde(rename = "@odata.type")]
    pub odata_type: &'static str,
    #[serde(rename = "Id")]
    pub id: &'static str,
    #[serde(rename = "Name")]
    pub name: &'static str,
    #[serde(rename = "Description")]
    pub description: &'static str,
    #[serde(rename = "Status")]
    pub status: Status,
    #[serde(rename = "CapacityWatts")]
    pub capacity_watts: u32,
    #[serde(rename = "Allocation")]
    pub allocation: PowerAllocation,
    #[serde(rename = "PowerSupplyRedundancy")]
    pub power_supply_redundancy: Vec<serde_json::Value>,
    #[serde(rename = "PowerSupplies")]
    pub power_supplies: ODataId,
}

#[derive(Debug, Serialize)]
pub struct PowerAllocation {
    #[serde(rename = "RequestedWatts")]
    pub requested_watts: u32,
    #[serde(rename = "AllocatedWatts")]
    pub allocated_watts: u32,
}

#[derive(Debug, Serialize)]
pub struct PowerSupplyResource {
    #[serde(rename = "@odata.id")]
    pub odata_id: String,
    #[serde(rename = "@odata.type")]
    pub odata_type: &'static str,
    #[serde(rename = "Id")]
    pub id: &'static str,
    #[serde(rename = "Name")]
    pub name: &'static str,
    #[serde(rename = "Description")]
    pub description: &'static str,
    #[serde(rename = "PowerCapacityWatts")]
    pub power_capacity_watts: u32,
    #[serde(rename = "PowerSupplyType")]
    pub power_supply_type: &'static str,
    #[serde(rename = "Manufacturer")]
    pub manufacturer: &'static str,
    #[serde(rename = "Model")]
    pub model: &'static str,
    #[serde(rename = "SerialNumber")]
    pub serial_number: &'static str,
    #[serde(rename = "PartNumber")]
    pub part_number: &'static str,
    #[serde(rename = "FirmwareVersion")]
    pub firmware_version: &'static str,
    #[serde(rename = "InputNominalVoltageType")]
    pub input_nominal_voltage_type: &'static str,
    #[serde(rename = "HotPluggable")]
    pub hot_pluggable: bool,
    #[serde(rename = "Location")]
    pub location: super::types::RedfishLocation,
    #[serde(rename = "LineInputStatus")]
    pub line_input_status: &'static str,
    #[serde(rename = "OutputNominalVoltageType")]
    pub output_nominal_voltage_type: &'static str,
    #[serde(rename = "PhaseWiringType")]
    pub phase_wiring_type: &'static str,
    #[serde(rename = "Replaceable")]
    pub replaceable: bool,
    #[serde(rename = "ProductionDate")]
    pub production_date: &'static str,
    #[serde(rename = "LocationIndicatorActive")]
    pub location_indicator_active: bool,
    #[serde(rename = "SparePartNumber")]
    pub spare_part_number: &'static str,
    #[serde(rename = "Version")]
    pub version: &'static str,
    #[serde(rename = "InputRanges")]
    pub input_ranges: Vec<PsuInputRange>,
    #[serde(rename = "EfficiencyRatings")]
    pub efficiency_ratings: Vec<PsuEfficiencyRating>,
    #[serde(rename = "Assembly")]
    pub assembly: ODataId,
    #[serde(rename = "Links")]
    pub psu_links: PsuLinks,
    #[serde(rename = "Status")]
    pub status: Status,
}

#[derive(Debug, Serialize)]
pub struct PsuEfficiencyRating {
    #[serde(rename = "LoadPercent")]
    pub load_percent: u32,
    #[serde(rename = "EfficiencyPercent")]
    pub efficiency_percent: u32,
}

#[derive(Debug, Serialize)]
pub struct PsuInputRange {
    #[serde(rename = "NominalVoltageType")]
    pub nominal_voltage_type: &'static str,
    #[serde(rename = "CapacityWatts")]
    pub capacity_watts: u32,
}

#[derive(Debug, Serialize)]
pub struct PsuLinks {
    #[serde(rename = "PoweringChassis")]
    pub powering_chassis: Vec<ODataId>,
    #[serde(rename = "PowerOutlets")]
    pub power_outlets: Vec<ODataId>,
}

pub async fn get_power_subsystem(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
) -> Json<PowerSubsystemResource> {
    let cid = &state.chassis_id;
    Json(PowerSubsystemResource {
        odata_id: format!("/redfish/v1/Chassis/{cid}/PowerSubsystem"),
        odata_type: "#PowerSubsystem.v1_1_0.PowerSubsystem",
        id: "PowerSubsystem",
        name: "Power Subsystem",
        description: "Power subsystem for virtual chassis",
        status: Status::enabled_ok(),
        capacity_watts: 1000,
        allocation: PowerAllocation {
            requested_watts: 50,
            allocated_watts: 500,
        },
        power_supply_redundancy: Vec::new(),
        power_supplies: ODataId::new(format!(
            "/redfish/v1/Chassis/{cid}/PowerSubsystem/PowerSupplies"
        )),
    })
}

pub async fn get_power_supplies(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
) -> Json<Collection<ODataId>> {
    let cid = &state.chassis_id;
    let members = vec![ODataId::new(format!(
        "/redfish/v1/Chassis/{cid}/PowerSubsystem/PowerSupplies/0"
    ))];

    Json(Collection::new(
        format!("/redfish/v1/Chassis/{cid}/PowerSubsystem/PowerSupplies"),
        "#PowerSupplyCollection.PowerSupplyCollection",
        "Power Supply Collection",
        members,
    ))
}

pub async fn get_power_supply(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
) -> Json<PowerSupplyResource> {
    let cid = &state.chassis_id;
    Json(PowerSupplyResource {
        odata_id: format!("/redfish/v1/Chassis/{cid}/PowerSubsystem/PowerSupplies/0"),
        odata_type: "#PowerSupply.v1_5_0.PowerSupply",
        id: "0",
        name: "Virtual PSU",
        description: "Virtual power supply unit",
        power_capacity_watts: 500,
        power_supply_type: "AC",
        manufacturer: "vbmc-rs",
        model: "Virtual PSU",
        serial_number: "VBMC-PSU-001",
        part_number: "VBMC-PSU",
        firmware_version: "1.0",
        input_nominal_voltage_type: "AC240V",
        hot_pluggable: false,
        location: super::types::RedfishLocation::new("PSU 0", "Bay", 0),
        line_input_status: "Normal",
        output_nominal_voltage_type: "DC12V",
        phase_wiring_type: "OnePhase3Wire",
        replaceable: false,
        production_date: "2026-01-01T00:00:00Z",
        location_indicator_active: false,
        spare_part_number: "VBMC-PSU-SPARE",
        version: "1.0",
        input_ranges: vec![PsuInputRange {
            nominal_voltage_type: "AC240V",
            capacity_watts: 500,
        }],
        efficiency_ratings: vec![
            PsuEfficiencyRating {
                load_percent: 50,
                efficiency_percent: 90,
            },
            PsuEfficiencyRating {
                load_percent: 100,
                efficiency_percent: 85,
            },
        ],
        assembly: ODataId::new(format!(
            "/redfish/v1/Chassis/{cid}/PowerSubsystem/PowerSupplies/0/Assembly"
        )),
        psu_links: PsuLinks {
            powering_chassis: vec![ODataId::new(format!("/redfish/v1/Chassis/{cid}"))],
            power_outlets: Vec::new(),
        },
        status: Status::enabled_ok(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_power_subsystem_resource_serialization() {
        let resource = PowerSubsystemResource {
            odata_id: "/redfish/v1/Chassis/test/PowerSubsystem".to_string(),
            odata_type: "#PowerSubsystem.v1_1_0.PowerSubsystem",
            id: "PowerSubsystem",
            name: "Power Subsystem",
            description: "Test power subsystem",
            status: Status::enabled_ok(),
            capacity_watts: 1000,
            allocation: PowerAllocation {
                requested_watts: 50,
                allocated_watts: 500,
            },
            power_supply_redundancy: Vec::new(),
            power_supplies: ODataId::new(
                "/redfish/v1/Chassis/test/PowerSubsystem/PowerSupplies".to_string(),
            ),
        };

        let json = serde_json::to_value(&resource).unwrap();
        assert_eq!(json["@odata.id"], "/redfish/v1/Chassis/test/PowerSubsystem");
        assert_eq!(json["@odata.type"], "#PowerSubsystem.v1_1_0.PowerSubsystem");
        assert_eq!(json["Id"], "PowerSubsystem");
        assert_eq!(json["Name"], "Power Subsystem");
        assert_eq!(json["CapacityWatts"], 1000);
        assert_eq!(json["Allocation"]["RequestedWatts"], 50);
        assert_eq!(json["Allocation"]["AllocatedWatts"], 500);
    }

    #[test]
    fn test_power_allocation_serialization() {
        let allocation = PowerAllocation {
            requested_watts: 100,
            allocated_watts: 200,
        };

        let json = serde_json::to_value(&allocation).unwrap();
        assert_eq!(json["RequestedWatts"], 100);
        assert_eq!(json["AllocatedWatts"], 200);
    }

    #[test]
    fn test_power_supply_resource_serialization() {
        let psu = PowerSupplyResource {
            odata_id: "/redfish/v1/Chassis/test/PowerSubsystem/PowerSupplies/0".to_string(),
            odata_type: "#PowerSupply.v1_5_0.PowerSupply",
            id: "0",
            name: "Virtual PSU",
            description: "Test PSU",
            power_capacity_watts: 500,
            power_supply_type: "AC",
            manufacturer: "vbmc-rs",
            model: "Virtual PSU",
            serial_number: "VBMC-PSU-001",
            part_number: "VBMC-PSU",
            firmware_version: "1.0",
            input_nominal_voltage_type: "AC240V",
            hot_pluggable: false,
            location: crate::redfish::types::RedfishLocation::new("PSU 0", "Bay", 0),
            line_input_status: "Normal",
            output_nominal_voltage_type: "DC12V",
            phase_wiring_type: "OnePhase3Wire",
            replaceable: false,
            production_date: "2026-01-01T00:00:00Z",
            location_indicator_active: false,
            spare_part_number: "VBMC-PSU-SPARE",
            version: "1.0",
            input_ranges: Vec::new(),
            efficiency_ratings: Vec::new(),
            assembly: ODataId::new("/redfish/v1/Chassis/test/Assembly".to_string()),
            psu_links: PsuLinks {
                powering_chassis: Vec::new(),
                power_outlets: Vec::new(),
            },
            status: Status::enabled_ok(),
        };

        let json = serde_json::to_value(&psu).unwrap();
        assert_eq!(
            json["@odata.id"],
            "/redfish/v1/Chassis/test/PowerSubsystem/PowerSupplies/0"
        );
        assert_eq!(json["@odata.type"], "#PowerSupply.v1_5_0.PowerSupply");
        assert_eq!(json["PowerCapacityWatts"], 500);
        assert_eq!(json["PowerSupplyType"], "AC");
        assert_eq!(json["InputNominalVoltageType"], "AC240V");
        assert_eq!(json["HotPluggable"], false);
        assert_eq!(json["LineInputStatus"], "Normal");
        assert_eq!(json["PhaseWiringType"], "OnePhase3Wire");
    }

    #[test]
    fn test_psu_efficiency_rating_serialization() {
        let rating = PsuEfficiencyRating {
            load_percent: 50,
            efficiency_percent: 90,
        };

        let json = serde_json::to_value(&rating).unwrap();
        assert_eq!(json["LoadPercent"], 50);
        assert_eq!(json["EfficiencyPercent"], 90);
    }

    #[test]
    fn test_psu_input_range_serialization() {
        let range = PsuInputRange {
            nominal_voltage_type: "AC240V",
            capacity_watts: 500,
        };

        let json = serde_json::to_value(&range).unwrap();
        assert_eq!(json["NominalVoltageType"], "AC240V");
        assert_eq!(json["CapacityWatts"], 500);
    }

    #[test]
    fn test_psu_links_serialization() {
        let links = PsuLinks {
            powering_chassis: vec![ODataId::new("/redfish/v1/Chassis/test".to_string())],
            power_outlets: vec![ODataId::new("/redfish/v1/PowerOutlet/1".to_string())],
        };

        let json = serde_json::to_value(&links).unwrap();
        assert_eq!(json["PoweringChassis"].as_array().unwrap().len(), 1);
        assert_eq!(json["PowerOutlets"].as_array().unwrap().len(), 1);
    }
}

#[cfg(test)]
mod harness_tests {
    use crate::backend::mock::MockBackend;
    use crate::redfish::test_harness as h;
    use axum::http::StatusCode;

    #[tokio::test]
    async fn test_get_power_subsystem() {
        let state = h::app_state(MockBackend::new(), h::systems_with("sys"));
        let app = h::router(state);

        let (status, json, _) = h::get(&app, "/redfish/v1/Chassis/1/PowerSubsystem").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["@odata.id"], "/redfish/v1/Chassis/1/PowerSubsystem");
        assert_eq!(json["@odata.type"], "#PowerSubsystem.v1_1_0.PowerSubsystem");
        assert_eq!(json["Id"], "PowerSubsystem");
        assert_eq!(json["Name"], "Power Subsystem");
        assert_eq!(json["Status"]["State"], "Enabled");
        assert_eq!(json["CapacityWatts"], 1000);
        assert_eq!(json["Allocation"]["RequestedWatts"], 50);
        assert_eq!(json["Allocation"]["AllocatedWatts"], 500);
        assert_eq!(
            json["PowerSupplies"]["@odata.id"],
            "/redfish/v1/Chassis/1/PowerSubsystem/PowerSupplies"
        );
    }

    #[tokio::test]
    async fn test_get_power_supplies_collection() {
        let state = h::app_state(MockBackend::new(), h::systems_with("sys"));
        let app = h::router(state);

        let (status, json, _) =
            h::get(&app, "/redfish/v1/Chassis/1/PowerSubsystem/PowerSupplies").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            json["@odata.id"],
            "/redfish/v1/Chassis/1/PowerSubsystem/PowerSupplies"
        );
        assert_eq!(
            json["@odata.type"],
            "#PowerSupplyCollection.PowerSupplyCollection"
        );
        assert_eq!(json["Name"], "Power Supply Collection");
        assert!(json["Members"].is_array());
        assert_eq!(json["Members"].as_array().unwrap().len(), 1);
        assert_eq!(
            json["Members"][0]["@odata.id"],
            "/redfish/v1/Chassis/1/PowerSubsystem/PowerSupplies/0"
        );
        assert_eq!(json["Members@odata.count"], 1);
    }

    #[tokio::test]
    async fn test_get_power_supply() {
        let state = h::app_state(MockBackend::new(), h::systems_with("sys"));
        let app = h::router(state);

        let (status, json, _) =
            h::get(&app, "/redfish/v1/Chassis/1/PowerSubsystem/PowerSupplies/0").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            json["@odata.id"],
            "/redfish/v1/Chassis/1/PowerSubsystem/PowerSupplies/0"
        );
        assert_eq!(json["@odata.type"], "#PowerSupply.v1_5_0.PowerSupply");
        assert_eq!(json["Id"], "0");
        assert_eq!(json["Name"], "Virtual PSU");
        assert_eq!(json["Manufacturer"], "vbmc-rs");
        assert_eq!(json["Model"], "Virtual PSU");
        assert_eq!(json["SerialNumber"], "VBMC-PSU-001");
        assert_eq!(json["PowerCapacityWatts"], 500);
        assert_eq!(json["PowerSupplyType"], "AC");
        assert_eq!(json["InputNominalVoltageType"], "AC240V");
        assert_eq!(json["OutputNominalVoltageType"], "DC12V");
        assert_eq!(json["HotPluggable"], false);
        assert_eq!(json["LineInputStatus"], "Normal");
        assert_eq!(json["PhaseWiringType"], "OnePhase3Wire");
        assert_eq!(json["Status"]["State"], "Enabled");
        assert!(json["InputRanges"].is_array());
        assert_eq!(json["InputRanges"][0]["NominalVoltageType"], "AC240V");
        assert_eq!(json["InputRanges"][0]["CapacityWatts"], 500);
        assert!(json["EfficiencyRatings"].is_array());
        assert_eq!(json["EfficiencyRatings"].as_array().unwrap().len(), 2);
        assert_eq!(json["EfficiencyRatings"][0]["LoadPercent"], 50);
        assert_eq!(json["EfficiencyRatings"][0]["EfficiencyPercent"], 90);
        assert!(json["Links"]["PoweringChassis"].is_array());
        assert_eq!(
            json["Assembly"]["@odata.id"],
            "/redfish/v1/Chassis/1/PowerSubsystem/PowerSupplies/0/Assembly"
        );
    }
}
