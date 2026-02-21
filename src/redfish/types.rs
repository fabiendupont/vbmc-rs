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
        Self {
            odata_id: odata_id.into(),
            odata_type: odata_type.into(),
            odata_context: Some("/redfish/v1/$metadata".to_string()),
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
            state: Some("Unavailable".to_string()),
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
