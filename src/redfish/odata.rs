use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum::Json;

static METADATA_XML: &str = include_str!("../../data/metadata.xml");

pub async fn get_metadata() -> Response {
    (
        [(header::CONTENT_TYPE, "application/xml")],
        METADATA_XML,
    )
        .into_response()
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
