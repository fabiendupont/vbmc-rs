use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use serde::{Deserialize, Serialize};

use super::error::RedfishApiError;
use super::types::{Collection, ODataId, Status};
use crate::app_state::AppState;
use crate::auth::AuthenticatedUser;
use crate::auth::rbac::{Privilege, has_privilege};

#[derive(Debug, Serialize)]
pub struct LicenseServiceResource {
    #[serde(rename = "@odata.id")]
    pub odata_id: &'static str,
    #[serde(rename = "@odata.type")]
    pub odata_type: &'static str,
    #[serde(rename = "Id")]
    pub id: &'static str,
    #[serde(rename = "Name")]
    pub name: &'static str,
    #[serde(rename = "Description")]
    pub description: &'static str,
    #[serde(rename = "ServiceEnabled")]
    pub service_enabled: bool,
    #[serde(rename = "LicenseExpirationWarningDays")]
    pub license_expiration_warning_days: u32,
    #[serde(rename = "Licenses")]
    pub licenses: ODataId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseResource {
    #[serde(rename = "@odata.id")]
    pub odata_id: String,
    #[serde(rename = "@odata.type")]
    pub odata_type: &'static str,
    #[serde(rename = "Id")]
    pub id: String,
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Description")]
    pub description: &'static str,
    #[serde(rename = "LicenseType")]
    pub license_type: String,
    #[serde(rename = "Status")]
    pub status: Status,
}

pub async fn get_license_service(_user: AuthenticatedUser) -> Json<LicenseServiceResource> {
    Json(LicenseServiceResource {
        odata_id: "/redfish/v1/LicenseService",
        odata_type: "#LicenseService.v1_1_0.LicenseService",
        id: "LicenseService",
        name: "License Service",
        description: "License management service",
        service_enabled: true,
        license_expiration_warning_days: 30,
        licenses: ODataId::new("/redfish/v1/LicenseService/Licenses"),
    })
}

pub async fn get_licenses(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
) -> Json<Collection<ODataId>> {
    // Collect licenses from all system states
    let mut members = Vec::new();
    for entry in state.vm_states.iter() {
        let vm_state = entry.value();
        for lic in &vm_state.licenses {
            members.push(ODataId::new(format!(
                "/redfish/v1/LicenseService/Licenses/{}",
                lic.id
            )));
        }
    }

    Json(Collection::new(
        "/redfish/v1/LicenseService/Licenses",
        "#LicenseCollection.LicenseCollection",
        "License Collection",
        members,
    ))
}

#[derive(Debug, Deserialize)]
pub struct CreateLicenseRequest {
    #[serde(rename = "LicenseString")]
    pub license_string: String,
    #[serde(rename = "Name", default)]
    pub name: Option<String>,
}

pub async fn create_license(
    State(state): State<Arc<AppState>>,
    user: AuthenticatedUser,
    Json(body): Json<CreateLicenseRequest>,
) -> Result<Json<serde_json::Value>, RedfishApiError> {
    if !has_privilege(&user.role, Privilege::ConfigureManager) {
        return Err(RedfishApiError::Forbidden(
            "Insufficient privileges".to_string(),
        ));
    }

    let id = uuid::Uuid::new_v4().to_string();
    let name = body.name.unwrap_or_else(|| format!("License {}", &id[..8]));

    let license = crate::state::LicenseInfo {
        id: id.clone(),
        name: name.clone(),
        license_type: "Production".to_string(),
        license_string: body.license_string,
    };

    // Store in the first system's state (licenses are global)
    if let Some(first_system) = state.config.systems.keys().next() {
        let system_id = first_system.clone();
        let mut vm_state = state.get_vm_state(&system_id);
        vm_state.licenses.push(license);
        state.save_vm_state(&system_id, &vm_state);
    }

    Ok(Json(serde_json::json!({
        "@odata.id": format!("/redfish/v1/LicenseService/Licenses/{id}"),
        "Id": id,
        "Name": name,
        "message": "License created"
    })))
}

pub async fn get_license(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
    Path(license_id): Path<String>,
) -> Result<Json<LicenseResource>, RedfishApiError> {
    for entry in state.vm_states.iter() {
        let vm_state = entry.value();
        if let Some(lic) = vm_state.licenses.iter().find(|l| l.id == license_id) {
            return Ok(Json(LicenseResource {
                odata_id: format!("/redfish/v1/LicenseService/Licenses/{license_id}"),
                odata_type: "#License.v1_1_1.License",
                id: license_id,
                name: lic.name.clone(),
                description: "License entry",
                license_type: lic.license_type.clone(),
                status: Status::enabled_ok(),
            }));
        }
    }

    Err(RedfishApiError::NotFound(format!(
        "License '{license_id}' not found"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_license_service_serialization() {
        let service = LicenseServiceResource {
            odata_id: "/redfish/v1/LicenseService",
            odata_type: "#LicenseService.v1_1_0.LicenseService",
            id: "LicenseService",
            name: "License Service",
            description: "License management service",
            service_enabled: true,
            license_expiration_warning_days: 30,
            licenses: ODataId::new("/redfish/v1/LicenseService/Licenses"),
        };

        let value = serde_json::to_value(&service).unwrap();

        assert_eq!(value["@odata.id"], "/redfish/v1/LicenseService");
        assert_eq!(
            value["@odata.type"],
            "#LicenseService.v1_1_0.LicenseService"
        );
        assert_eq!(value["Id"], "LicenseService");
        assert_eq!(value["Name"], "License Service");
        assert_eq!(value["ServiceEnabled"], true);
        assert_eq!(value["LicenseExpirationWarningDays"], 30);
        assert_eq!(
            value["Licenses"]["@odata.id"],
            "/redfish/v1/LicenseService/Licenses"
        );
    }

    #[test]
    fn test_license_resource_serialization() {
        let license = LicenseResource {
            odata_id: "/redfish/v1/LicenseService/Licenses/test-123".to_string(),
            odata_type: "#License.v1_1_1.License",
            id: "test-123".to_string(),
            name: "Test License".to_string(),
            description: "License entry",
            license_type: "Production".to_string(),
            status: Status::enabled_ok(),
        };

        let value = serde_json::to_value(&license).unwrap();

        assert_eq!(
            value["@odata.id"],
            "/redfish/v1/LicenseService/Licenses/test-123"
        );
        assert_eq!(value["@odata.type"], "#License.v1_1_1.License");
        assert_eq!(value["Id"], "test-123");
        assert_eq!(value["Name"], "Test License");
        assert_eq!(value["LicenseType"], "Production");
        assert_eq!(value["Status"]["State"], "Enabled");
        assert_eq!(value["Status"]["Health"], "OK");
    }

    #[test]
    fn test_create_license_request_deserialization() {
        let json = serde_json::json!({
            "LicenseString": "LICENSE-KEY-12345",
            "Name": "My License"
        });

        let request: CreateLicenseRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.license_string, "LICENSE-KEY-12345");
        assert_eq!(request.name, Some("My License".to_string()));
    }

    #[test]
    fn test_create_license_request_without_name() {
        let json = serde_json::json!({
            "LicenseString": "LICENSE-KEY-12345"
        });

        let request: CreateLicenseRequest = serde_json::from_value(json).unwrap();
        assert_eq!(request.license_string, "LICENSE-KEY-12345");
        assert_eq!(request.name, None);
    }

    // Integration tests exercising async handler bodies
    #[tokio::test]
    async fn test_get_license_service_handler() {
        use crate::redfish::test_harness::*;
        let router = router_with_systems(systems_with("system1"));

        let (status, json, _headers) = get(&router, "/redfish/v1/LicenseService").await;

        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(json["@odata.id"], "/redfish/v1/LicenseService");
        assert_eq!(json["@odata.type"], "#LicenseService.v1_1_0.LicenseService");
        assert_eq!(json["Id"], "LicenseService");
        assert_eq!(json["Name"], "License Service");
        assert_eq!(json["ServiceEnabled"], true);
        assert_eq!(json["LicenseExpirationWarningDays"], 30);
        assert_eq!(
            json["Licenses"]["@odata.id"],
            "/redfish/v1/LicenseService/Licenses"
        );
    }

    #[tokio::test]
    async fn test_get_licenses_collection() {
        use crate::redfish::test_harness::*;
        let router = router_with_systems(systems_with("system1"));

        let (status, json, _headers) = get(&router, "/redfish/v1/LicenseService/Licenses").await;

        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(json["@odata.id"], "/redfish/v1/LicenseService/Licenses");
        assert_eq!(json["@odata.type"], "#LicenseCollection.LicenseCollection");
        assert_eq!(json["Name"], "License Collection");
        // Members count may vary due to persistent state, just verify structure
        assert!(json["Members"].is_array());
        assert!(json["Members@odata.count"].is_number());
    }

    #[tokio::test]
    async fn test_create_license_valid() {
        use crate::redfish::test_harness::*;
        use axum::http::Method;

        let router = router_with_systems(systems_with("system1"));

        let body = serde_json::json!({
            "LicenseString": "TEST-LICENSE-KEY-12345",
            "Name": "Test Production License"
        });

        let (status, json, _headers) = request_json(
            &router,
            Method::POST,
            "/redfish/v1/LicenseService/Licenses",
            body,
        )
        .await;

        assert!(
            status.is_success(),
            "Expected 2xx for valid license creation, got {status}"
        );
        assert!(
            json["@odata.id"]
                .as_str()
                .unwrap()
                .starts_with("/redfish/v1/LicenseService/Licenses/")
        );
        assert_eq!(json["Name"], "Test Production License");
    }

    #[tokio::test]
    async fn test_create_license_minimal() {
        use crate::redfish::test_harness::*;
        use axum::http::Method;

        let router = router_with_systems(systems_with("system1"));

        // Only required field
        let body = serde_json::json!({
            "LicenseString": "TEST-LICENSE-KEY-67890"
        });

        let (status, json, _headers) = request_json(
            &router,
            Method::POST,
            "/redfish/v1/LicenseService/Licenses",
            body,
        )
        .await;

        assert!(
            status.is_success(),
            "Expected 2xx for minimal valid license, got {status}"
        );
        assert!(
            json["@odata.id"]
                .as_str()
                .unwrap()
                .starts_with("/redfish/v1/LicenseService/Licenses/")
        );
        // Name should be auto-generated
        assert!(json["Name"].as_str().unwrap().starts_with("License "));
    }

    #[tokio::test]
    async fn test_get_license_after_create() {
        use crate::redfish::test_harness::*;
        use axum::http::Method;

        let router = router_with_systems(systems_with("system1"));

        // Create a license
        let body = serde_json::json!({
            "LicenseString": "TEST-LICENSE-FOR-GET",
            "Name": "Get Test License"
        });

        let (_status, create_response, _headers) = request_json(
            &router,
            Method::POST,
            "/redfish/v1/LicenseService/Licenses",
            body,
        )
        .await;

        let license_id = create_response["Id"].as_str().unwrap();
        let license_uri = format!("/redfish/v1/LicenseService/Licenses/{license_id}");

        // GET the created license
        let (status, json, _headers) = get(&router, &license_uri).await;

        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(json["@odata.id"], license_uri);
        assert_eq!(json["@odata.type"], "#License.v1_1_1.License");
        assert_eq!(json["Id"], license_id);
        assert_eq!(json["Name"], "Get Test License");
        assert_eq!(json["LicenseType"], "Production");
        assert_eq!(json["Status"]["State"], "Enabled");
        assert_eq!(json["Status"]["Health"], "OK");
    }

    #[tokio::test]
    async fn test_create_license_bad_body() {
        use crate::redfish::test_harness::*;
        use axum::http::Method;

        let router = router_with_systems(systems_with("system1"));

        // Missing required "LicenseString" field
        let bad_body = serde_json::json!({
            "Name": "License without key"
        });

        let (status, _json, _headers) = request_json(
            &router,
            Method::POST,
            "/redfish/v1/LicenseService/Licenses",
            bad_body,
        )
        .await;

        assert!(
            status.is_client_error(),
            "Expected 4xx for bad body, got {status}"
        );
    }

    #[tokio::test]
    async fn test_get_license_not_found() {
        use crate::redfish::test_harness::*;

        let router = router_with_systems(systems_with("system1"));

        let (status, _json, _headers) = get(
            &router,
            "/redfish/v1/LicenseService/Licenses/nonexistent-id",
        )
        .await;

        assert_eq!(status, axum::http::StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_get_licenses_after_create() {
        use crate::redfish::test_harness::*;
        use axum::http::Method;

        let router = router_with_systems(systems_with("system1"));

        // Get initial count
        let (_, initial_json, _) = get(&router, "/redfish/v1/LicenseService/Licenses").await;
        let initial_count = initial_json["Members@odata.count"].as_u64().unwrap();

        // Create a license
        let body = serde_json::json!({
            "LicenseString": "TEST-LICENSE-COLLECTION",
            "Name": "Collection Test License"
        });

        let (_status, create_response, _headers) = request_json(
            &router,
            Method::POST,
            "/redfish/v1/LicenseService/Licenses",
            body,
        )
        .await;

        let license_uri = create_response["@odata.id"].as_str().unwrap();

        // GET the collection - should now contain one more license
        let (status, json, _headers) = get(&router, "/redfish/v1/LicenseService/Licenses").await;

        assert_eq!(status, axum::http::StatusCode::OK);
        assert_eq!(json["Members@odata.count"], initial_count + 1);
        let members = json["Members"].as_array().unwrap();
        assert_eq!(members.len() as u64, initial_count + 1);
        // Verify our newly created license is in the collection
        assert!(
            members
                .iter()
                .any(|m| m["@odata.id"].as_str() == Some(license_uri))
        );
    }
}
