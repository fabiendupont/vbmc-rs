use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use axum::http::Uri;
use serde::Serialize;

use super::error::RedfishApiError;
use crate::app_state::AppState;
use crate::auth::AuthenticatedUser;

#[derive(Debug, Serialize)]
pub struct AssemblyResource {
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
    #[serde(rename = "Assemblies")]
    pub assemblies: Vec<serde_json::Value>,
    #[serde(rename = "Assemblies@odata.count")]
    pub assemblies_count: usize,
}

pub async fn get_chassis_assembly(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
) -> Json<AssemblyResource> {
    Json(AssemblyResource {
        odata_id: format!("/redfish/v1/Chassis/{}/Assembly", state.chassis_id),
        odata_type: "#Assembly.v1_5_0.Assembly",
        id: "Assembly",
        name: "Chassis Assembly",
        description: "Virtual chassis assembly information",
        assemblies: Vec::new(),
        assemblies_count: 0,
    })
}

pub async fn get_chassis_sub_assembly(
    _user: AuthenticatedUser,
    uri: Uri,
) -> Result<Json<AssemblyResource>, RedfishApiError> {
    Ok(Json(AssemblyResource {
        odata_id: uri.path().to_string(),
        odata_type: "#Assembly.v1_5_0.Assembly",
        id: "Assembly",
        name: "Component Assembly",
        description: "Component assembly information",
        assemblies: Vec::new(),
        assemblies_count: 0,
    }))
}

pub async fn get_system_sub_assembly(
    _user: AuthenticatedUser,
    uri: Uri,
) -> Result<Json<AssemblyResource>, RedfishApiError> {
    Ok(Json(AssemblyResource {
        odata_id: uri.path().to_string(),
        odata_type: "#Assembly.v1_5_0.Assembly",
        id: "Assembly",
        name: "Component Assembly",
        description: "Component assembly information",
        assemblies: Vec::new(),
        assemblies_count: 0,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_assembly_resource_serialization() {
        let resource = AssemblyResource {
            odata_id: "/redfish/v1/Chassis/vbmc/Assembly".to_string(),
            odata_type: "#Assembly.v1_5_0.Assembly",
            id: "Assembly",
            name: "Chassis Assembly",
            description: "Virtual chassis assembly information",
            assemblies: Vec::new(),
            assemblies_count: 0,
        };

        let value = serde_json::to_value(&resource).unwrap();

        assert_eq!(value["@odata.id"], "/redfish/v1/Chassis/vbmc/Assembly");
        assert_eq!(value["@odata.type"], "#Assembly.v1_5_0.Assembly");
        assert_eq!(value["Id"], "Assembly");
        assert_eq!(value["Name"], "Chassis Assembly");
        assert_eq!(value["Description"], "Virtual chassis assembly information");
        assert!(value["Assemblies"].is_array());
        assert_eq!(value["Assemblies@odata.count"], 0);
    }

    #[test]
    fn test_assembly_resource_with_assemblies() {
        let assemblies = vec![
            serde_json::json!({
                "MemberId": "1",
                "Name": "CPU",
                "Description": "Processor"
            }),
            serde_json::json!({
                "MemberId": "2",
                "Name": "Memory",
                "Description": "RAM Module"
            }),
        ];

        let resource = AssemblyResource {
            odata_id: "/redfish/v1/Chassis/vbmc/Assembly".to_string(),
            odata_type: "#Assembly.v1_5_0.Assembly",
            id: "Assembly",
            name: "Chassis Assembly",
            description: "Virtual chassis assembly information",
            assemblies_count: assemblies.len(),
            assemblies,
        };

        let value = serde_json::to_value(&resource).unwrap();
        assert_eq!(value["Assemblies@odata.count"], 2);
        assert_eq!(value["Assemblies"].as_array().unwrap().len(), 2);
        assert_eq!(value["Assemblies"][0]["MemberId"], "1");
        assert_eq!(value["Assemblies"][1]["MemberId"], "2");
    }

    #[test]
    fn test_assembly_resource_empty() {
        let resource = AssemblyResource {
            odata_id: "/redfish/v1/Systems/vm1/Processors/CPU0/Assembly".to_string(),
            odata_type: "#Assembly.v1_5_0.Assembly",
            id: "Assembly",
            name: "Component Assembly",
            description: "Component assembly information",
            assemblies: Vec::new(),
            assemblies_count: 0,
        };

        let value = serde_json::to_value(&resource).unwrap();
        assert_eq!(value["Assemblies@odata.count"], 0);
        assert!(value["Assemblies"].as_array().unwrap().is_empty());
    }

    #[test]
    fn test_assembly_resource_count_consistency() {
        let assemblies = vec![
            serde_json::json!({"MemberId": "1"}),
            serde_json::json!({"MemberId": "2"}),
            serde_json::json!({"MemberId": "3"}),
        ];

        let resource = AssemblyResource {
            odata_id: "/redfish/v1/Chassis/vbmc/Assembly".to_string(),
            odata_type: "#Assembly.v1_5_0.Assembly",
            id: "Assembly",
            name: "Chassis Assembly",
            description: "Virtual chassis assembly information",
            assemblies_count: assemblies.len(),
            assemblies,
        };

        let value = serde_json::to_value(&resource).unwrap();
        assert_eq!(
            value["Assemblies@odata.count"],
            value["Assemblies"].as_array().unwrap().len()
        );
    }

    #[test]
    fn test_assembly_odata_count_annotation() {
        let resource = AssemblyResource {
            odata_id: "/test".to_string(),
            odata_type: "#Assembly.v1_5_0.Assembly",
            id: "Assembly",
            name: "Test",
            description: "Test assembly",
            assemblies: vec![serde_json::json!({"test": "value"})],
            assemblies_count: 1,
        };

        let value = serde_json::to_value(&resource).unwrap();
        // Check that the @odata.count annotation is properly named
        assert!(
            value
                .as_object()
                .unwrap()
                .contains_key("Assemblies@odata.count")
        );
    }
}
