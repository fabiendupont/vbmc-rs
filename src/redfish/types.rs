use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ODataId {
    #[serde(rename = "@odata.id")]
    pub odata_id: String,
}

impl ODataId {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            odata_id: id.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Collection<T: Serialize> {
    #[serde(rename = "@odata.id")]
    pub odata_id: String,
    #[serde(rename = "@odata.type")]
    pub odata_type: String,
    #[serde(rename = "@odata.context", skip_serializing_if = "Option::is_none")]
    pub odata_context: Option<String>,
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Description")]
    pub description: String,
    #[serde(rename = "Members")]
    pub members: Vec<T>,
    #[serde(rename = "Members@odata.count")]
    pub members_count: usize,
}

impl<T: Serialize> Collection<T> {
    pub fn new(
        odata_id: impl Into<String>,
        odata_type: impl Into<String>,
        name: impl Into<String>,
        members: Vec<T>,
    ) -> Self {
        let count = members.len();
        let odata_type = odata_type.into();
        let name = name.into();
        // Derive @odata.context from @odata.type: "#Foo.Foo" → "/redfish/v1/$metadata#Foo.Foo"
        let odata_context = odata_type
            .strip_prefix('#')
            .map(|t| format!("/redfish/v1/$metadata#{t}"));
        let description = name.clone();
        Self {
            odata_id: odata_id.into(),
            odata_type,
            odata_context,
            name,
            description,
            members,
            members_count: count,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Status {
    #[serde(rename = "State", skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(rename = "Health", skip_serializing_if = "Option::is_none")]
    pub health: Option<String>,
    #[serde(rename = "HealthRollup", skip_serializing_if = "Option::is_none")]
    pub health_rollup: Option<String>,
}

impl Status {
    pub fn enabled_ok() -> Self {
        Self {
            state: Some("Enabled".to_string()),
            health: Some("OK".to_string()),
            health_rollup: Some("OK".to_string()),
        }
    }

    pub fn unavailable_critical() -> Self {
        Self {
            state: Some("UnavailableOffline".to_string()),
            health: Some("Critical".to_string()),
            health_rollup: Some("Critical".to_string()),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RedfishLocation {
    #[serde(rename = "Placement")]
    pub placement: Placement,
    #[serde(rename = "PostalAddress")]
    pub postal_address: PostalAddress,
    #[serde(rename = "PhysicalAddress")]
    pub physical_address: PhysicalAddress,
    #[serde(rename = "PartLocation")]
    pub part_location: PartLocation,
    #[serde(rename = "Contacts")]
    pub contacts: Vec<serde_json::Value>,
    #[serde(rename = "AltitudeMeters")]
    pub altitude_meters: f64,
    #[serde(rename = "Latitude")]
    pub latitude: f64,
    #[serde(rename = "Longitude")]
    pub longitude: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Placement {
    #[serde(rename = "Row")]
    pub row: &'static str,
    #[serde(rename = "Rack")]
    pub rack: &'static str,
    #[serde(rename = "RackOffset")]
    pub rack_offset: u32,
    #[serde(rename = "RackOffsetUnits")]
    pub rack_offset_units: &'static str,
    #[serde(rename = "Room")]
    pub room: &'static str,
    #[serde(rename = "FacilityName")]
    pub facility_name: &'static str,
    #[serde(rename = "AdditionalInfo")]
    pub additional_info: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct PostalAddress {
    #[serde(rename = "Country")]
    pub country: &'static str,
    #[serde(rename = "Territory")]
    pub territory: &'static str,
    #[serde(rename = "City")]
    pub city: &'static str,
    #[serde(rename = "Street")]
    pub street: &'static str,
    #[serde(rename = "HouseNumber")]
    pub house_number: u32,
    #[serde(rename = "Name")]
    pub name: &'static str,
    #[serde(rename = "PostalCode")]
    pub postal_code: &'static str,
    #[serde(rename = "Building")]
    pub building: &'static str,
    #[serde(rename = "Floor")]
    pub floor: &'static str,
    #[serde(rename = "Room")]
    pub room: &'static str,
    #[serde(rename = "Unit")]
    pub unit: &'static str,
    #[serde(rename = "Seat")]
    pub seat: &'static str,
    #[serde(rename = "PlaceType")]
    pub place_type: &'static str,
    #[serde(rename = "Community")]
    pub community: &'static str,
    #[serde(rename = "District")]
    pub district: &'static str,
    #[serde(rename = "Division")]
    pub division: &'static str,
    #[serde(rename = "Neighborhood")]
    pub neighborhood: &'static str,
    #[serde(rename = "LeadingStreetDirection")]
    pub leading_street_direction: &'static str,
    #[serde(rename = "TrailingStreetSuffix")]
    pub trailing_street_suffix: &'static str,
    #[serde(rename = "StreetSuffix")]
    pub street_suffix: &'static str,
    #[serde(rename = "HouseNumberSuffix")]
    pub house_number_suffix: &'static str,
    #[serde(rename = "Landmark")]
    pub landmark: &'static str,
    #[serde(rename = "POBox")]
    pub po_box: &'static str,
    #[serde(rename = "AdditionalCode")]
    pub additional_code: &'static str,
    #[serde(rename = "AdditionalInfo")]
    pub additional_info: &'static str,
    #[serde(rename = "Road")]
    pub road: &'static str,
    #[serde(rename = "RoadSection")]
    pub road_section: &'static str,
    #[serde(rename = "RoadBranch")]
    pub road_branch: &'static str,
    #[serde(rename = "RoadSubBranch")]
    pub road_sub_branch: &'static str,
    #[serde(rename = "RoadPreModifier")]
    pub road_pre_modifier: &'static str,
    #[serde(rename = "RoadPostModifier")]
    pub road_post_modifier: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct PhysicalAddress {
    #[serde(rename = "StreetAddress")]
    pub street_address: &'static str,
    #[serde(rename = "City")]
    pub city: &'static str,
    #[serde(rename = "StateOrProvince")]
    pub state_or_province: &'static str,
    #[serde(rename = "Country")]
    pub country: &'static str,
    #[serde(rename = "PostalCode")]
    pub postal_code: &'static str,
    #[serde(rename = "ISOCountryCode")]
    pub iso_country_code: &'static str,
    #[serde(rename = "ISOSubdivisionCode")]
    pub iso_subdivision_code: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct PartLocation {
    #[serde(rename = "ServiceLabel")]
    pub service_label: String,
    #[serde(rename = "LocationType")]
    pub location_type: &'static str,
    #[serde(rename = "LocationOrdinalValue")]
    pub location_ordinal_value: u32,
    #[serde(rename = "Reference")]
    pub reference: &'static str,
    #[serde(rename = "Orientation")]
    pub orientation: &'static str,
}

impl RedfishLocation {
    pub fn new(
        service_label: impl Into<String>,
        location_type: &'static str,
        ordinal: u32,
    ) -> Self {
        Self {
            placement: Placement {
                row: "1",
                rack: "1",
                rack_offset: 1,
                rack_offset_units: "EIA_310",
                room: "",
                facility_name: "",
                additional_info: "",
            },
            postal_address: PostalAddress {
                country: "",
                territory: "",
                city: "",
                street: "",
                house_number: 0,
                name: "",
                postal_code: "",
                building: "",
                floor: "",
                room: "",
                unit: "",
                seat: "",
                place_type: "",
                community: "",
                district: "",
                division: "",
                neighborhood: "",
                leading_street_direction: "",
                trailing_street_suffix: "",
                street_suffix: "",
                house_number_suffix: "",
                landmark: "",
                po_box: "",
                additional_code: "",
                additional_info: "",
                road: "",
                road_section: "",
                road_branch: "",
                road_sub_branch: "",
                road_pre_modifier: "",
                road_post_modifier: "",
            },
            physical_address: PhysicalAddress {
                street_address: "",
                city: "",
                state_or_province: "",
                country: "",
                postal_code: "",
                iso_country_code: "XX",
                iso_subdivision_code: "XX",
            },
            part_location: PartLocation {
                service_label: service_label.into(),
                location_type,
                location_ordinal_value: ordinal,
                reference: "Top",
                orientation: "FrontToBack",
            },
            contacts: Vec::new(),
            altitude_meters: 0.0,
            latitude: 0.0,
            longitude: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusRollup {
    #[serde(rename = "Health")]
    pub health: String,
}

impl StatusRollup {
    pub fn from_statuses(statuses: &[&Status]) -> Self {
        let health = if statuses
            .iter()
            .any(|s| s.health.as_deref() == Some("Critical"))
        {
            "Critical"
        } else if statuses
            .iter()
            .any(|s| s.health.as_deref() == Some("Warning"))
        {
            "Warning"
        } else {
            "OK"
        };
        Self {
            health: health.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_odata_id_new() {
        let id = ODataId::new("/redfish/v1/Systems/vm1");
        assert_eq!(id.odata_id, "/redfish/v1/Systems/vm1");
    }

    #[test]
    fn test_odata_id_serialization() {
        let id = ODataId::new("/redfish/v1/Chassis/1");
        let json = serde_json::to_value(&id).unwrap();
        assert_eq!(json["@odata.id"], "/redfish/v1/Chassis/1");
    }

    #[test]
    fn test_collection_new() {
        let members = vec![
            ODataId::new("/redfish/v1/Systems/vm1"),
            ODataId::new("/redfish/v1/Systems/vm2"),
        ];
        let coll = Collection::new(
            "/redfish/v1/Systems",
            "#ComputerSystemCollection.ComputerSystemCollection",
            "Computer System Collection",
            members,
        );

        assert_eq!(coll.odata_id, "/redfish/v1/Systems");
        assert_eq!(coll.members_count, 2);
        assert_eq!(coll.members.len(), 2);
        assert_eq!(
            coll.odata_context.as_deref(),
            Some("/redfish/v1/$metadata#ComputerSystemCollection.ComputerSystemCollection")
        );
    }

    #[test]
    fn test_collection_empty() {
        let coll: Collection<ODataId> =
            Collection::new("/redfish/v1/Empty", "#EmptyCollection", "Empty", vec![]);
        assert_eq!(coll.members_count, 0);
        assert!(coll.members.is_empty());
    }

    #[test]
    fn test_collection_serialization() {
        let coll = Collection::new(
            "/redfish/v1/Systems",
            "#ComputerSystemCollection.ComputerSystemCollection",
            "Systems",
            vec![ODataId::new("/redfish/v1/Systems/vm1")],
        );
        let json = serde_json::to_value(&coll).unwrap();
        assert_eq!(json["@odata.id"], "/redfish/v1/Systems");
        assert_eq!(
            json["@odata.type"],
            "#ComputerSystemCollection.ComputerSystemCollection"
        );
        assert_eq!(
            json["@odata.context"],
            "/redfish/v1/$metadata#ComputerSystemCollection.ComputerSystemCollection"
        );
        assert_eq!(json["Name"], "Systems");
        assert_eq!(json["Members@odata.count"], 1);
        assert_eq!(json["Members"][0]["@odata.id"], "/redfish/v1/Systems/vm1");
    }

    #[test]
    fn test_status_enabled_ok() {
        let s = Status::enabled_ok();
        assert_eq!(s.state.as_deref(), Some("Enabled"));
        assert_eq!(s.health.as_deref(), Some("OK"));
        assert_eq!(s.health_rollup.as_deref(), Some("OK"));
    }

    #[test]
    fn test_status_unavailable_critical() {
        let s = Status::unavailable_critical();
        assert_eq!(s.state.as_deref(), Some("UnavailableOffline"));
        assert_eq!(s.health.as_deref(), Some("Critical"));
    }

    #[test]
    fn test_status_default_is_empty() {
        let s = Status::default();
        assert!(s.state.is_none());
        assert!(s.health.is_none());
        assert!(s.health_rollup.is_none());
    }

    #[test]
    fn test_status_rollup_all_ok() {
        let s1 = Status::enabled_ok();
        let s2 = Status::enabled_ok();
        let rollup = StatusRollup::from_statuses(&[&s1, &s2]);
        assert_eq!(rollup.health, "OK");
    }

    #[test]
    fn test_status_rollup_one_warning() {
        let ok = Status::enabled_ok();
        let warn = Status {
            state: Some("Enabled".to_string()),
            health: Some("Warning".to_string()),
            health_rollup: None,
        };
        let rollup = StatusRollup::from_statuses(&[&ok, &warn]);
        assert_eq!(rollup.health, "Warning");
    }

    #[test]
    fn test_status_rollup_critical_wins() {
        let warn = Status {
            state: Some("Enabled".to_string()),
            health: Some("Warning".to_string()),
            health_rollup: None,
        };
        let crit = Status::unavailable_critical();
        let rollup = StatusRollup::from_statuses(&[&warn, &crit]);
        assert_eq!(rollup.health, "Critical");
    }

    #[test]
    fn test_status_rollup_empty_is_ok() {
        let rollup = StatusRollup::from_statuses(&[]);
        assert_eq!(rollup.health, "OK");
    }
}
