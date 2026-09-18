use axum::Json;
use axum::http::header;
use axum::response::{IntoResponse, Response};

static METADATA_XML: &str = include_str!("../../data/metadata.xml");

pub async fn get_metadata() -> Response {
    ([(header::CONTENT_TYPE, "application/xml")], METADATA_XML).into_response()
}

pub async fn get_odata_service_document() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "@odata.context": "/redfish/v1/$metadata",
        "value": [
            { "name": "Systems", "kind": "Singleton", "url": "/redfish/v1/Systems" },
            { "name": "Chassis", "kind": "Singleton", "url": "/redfish/v1/Chassis" },
            { "name": "Managers", "kind": "Singleton", "url": "/redfish/v1/Managers" },
            { "name": "SessionService", "kind": "Singleton", "url": "/redfish/v1/SessionService" },
            { "name": "AccountService", "kind": "Singleton", "url": "/redfish/v1/AccountService" },
            { "name": "EventService", "kind": "Singleton", "url": "/redfish/v1/EventService" },
            { "name": "TaskService", "kind": "Singleton", "url": "/redfish/v1/TaskService" },
            { "name": "TelemetryService", "kind": "Singleton", "url": "/redfish/v1/TelemetryService" },
            { "name": "CertificateService", "kind": "Singleton", "url": "/redfish/v1/CertificateService" },
            { "name": "UpdateService", "kind": "Singleton", "url": "/redfish/v1/UpdateService" },
            { "name": "LicenseService", "kind": "Singleton", "url": "/redfish/v1/LicenseService" },
        ]
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_odata_service_document_structure() {
        let response = get_odata_service_document().await;
        let json = response.0;

        assert_eq!(json["@odata.context"], "/redfish/v1/$metadata");
        assert!(json["value"].is_array());
        let services = json["value"].as_array().unwrap();
        assert!(!services.is_empty());

        // Check first service has required fields
        let first = &services[0];
        assert!(first["name"].is_string());
        assert_eq!(first["kind"], "Singleton");
        assert!(first["url"].is_string());
    }

    #[tokio::test]
    async fn test_odata_service_document_contains_expected_services() {
        let response = get_odata_service_document().await;
        let json = response.0;
        let services = json["value"].as_array().unwrap();

        let names: Vec<&str> = services
            .iter()
            .map(|s| s["name"].as_str().unwrap())
            .collect();

        assert!(names.contains(&"Systems"));
        assert!(names.contains(&"SessionService"));
        assert!(names.contains(&"AccountService"));
    }
}
