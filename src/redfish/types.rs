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
    #[serde(rename = "Members")]
    pub members: Vec<T>,
    #[serde(rename = "Members@odata.count")]
    pub members_count: usize,
}

impl<T: Serialize> Collection<T> {
    pub fn new(odata_id: impl Into<String>, odata_type: impl Into<String>, name: impl Into<String>, members: Vec<T>) -> Self {
        let count = members.len();
        let odata_type = odata_type.into();
        // Derive @odata.context from @odata.type: "#Foo.Foo" → "/redfish/v1/$metadata#Foo.Foo"
        let odata_context = odata_type
            .strip_prefix('#')
            .map(|t| format!("/redfish/v1/$metadata#{t}"));
        Self {
            odata_id: odata_id.into(),
            odata_type,
            odata_context,
            name: name.into(),
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusRollup {
    #[serde(rename = "Health")]
    pub health: String,
}

impl StatusRollup {
    pub fn from_statuses(statuses: &[&Status]) -> Self {
        let health = if statuses.iter().any(|s| s.health.as_deref() == Some("Critical")) {
            "Critical"
        } else if statuses.iter().any(|s| s.health.as_deref() == Some("Warning")) {
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
        let coll: Collection<ODataId> = Collection::new(
            "/redfish/v1/Empty",
            "#EmptyCollection",
            "Empty",
            vec![],
        );
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
        assert_eq!(json["@odata.type"], "#ComputerSystemCollection.ComputerSystemCollection");
        assert_eq!(json["@odata.context"], "/redfish/v1/$metadata#ComputerSystemCollection.ComputerSystemCollection");
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
