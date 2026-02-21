use axum::Json;
use serde::Serialize;

use super::types::Status;

#[derive(Debug, Serialize)]
pub struct PowerResource {
    #[serde(rename = "@odata.id")]
    pub odata_id: &'static str,
    #[serde(rename = "@odata.type")]
    pub odata_type: &'static str,
    #[serde(rename = "Id")]
    pub id: &'static str,
    #[serde(rename = "Name")]
    pub name: &'static str,
    #[serde(rename = "PowerControl")]
    pub power_control: Vec<PowerControl>,
    #[serde(rename = "PowerSupplies")]
    pub power_supplies: Vec<PowerSupply>,
}

#[derive(Debug, Serialize)]
pub struct PowerControl {
    #[serde(rename = "MemberId")]
    pub member_id: &'static str,
    #[serde(rename = "Name")]
    pub name: &'static str,
    #[serde(rename = "PowerConsumedWatts")]
    pub power_consumed_watts: u32,
    #[serde(rename = "PowerCapacityWatts")]
    pub power_capacity_watts: u32,
    #[serde(rename = "Status")]
    pub status: Status,
}

#[derive(Debug, Serialize)]
pub struct PowerSupply {
    #[serde(rename = "MemberId")]
    pub member_id: &'static str,
    #[serde(rename = "Name")]
    pub name: &'static str,
    #[serde(rename = "PowerCapacityWatts")]
    pub power_capacity_watts: u32,
    #[serde(rename = "PowerSupplyType")]
    pub power_supply_type: &'static str,
    #[serde(rename = "Status")]
    pub status: Status,
}

pub async fn get_power() -> Json<PowerResource> {
    Json(PowerResource {
        odata_id: "/redfish/v1/Chassis/1/Power",
        odata_type: "#Power.v1_7_2.Power",
        id: "Power",
        name: "Power",
        power_control: vec![PowerControl {
            member_id: "0",
            name: "System Power Control",
            power_consumed_watts: 50,
            power_capacity_watts: 500,
            status: Status::enabled_ok(),
        }],
        power_supplies: vec![PowerSupply {
            member_id: "0",
            name: "Virtual PSU",
            power_capacity_watts: 500,
            power_supply_type: "AC",
            status: Status::enabled_ok(),
        }],
    })
}
