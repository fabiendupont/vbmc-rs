use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use super::error::RedfishApiError;
use super::types::{Collection, ODataId, Status};
use crate::app_state::AppState;
use crate::auth::AuthenticatedUser;
use crate::auth::rbac::{Privilege, has_privilege};
use crate::backend::VmmBackend;
use crate::backend::types::{DiskCreateConfig, VmPowerState};
use crate::events::RedfishEvent;
use crate::events::registry::*;

#[derive(Debug, Serialize)]
pub struct VirtualMediaResource {
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
    #[serde(rename = "MediaTypes")]
    pub media_types: Vec<&'static str>,
    #[serde(rename = "Inserted")]
    pub inserted: bool,
    #[serde(rename = "Image")]
    pub image: Option<String>,
    #[serde(rename = "ImageName")]
    pub image_name: Option<String>,
    #[serde(rename = "UserName")]
    pub user_name: Option<String>,
    #[serde(rename = "Password")]
    pub password: Option<String>,
    #[serde(rename = "EjectTimeout")]
    pub eject_timeout: &'static str,
    #[serde(rename = "WriteProtected")]
    pub write_protected: bool,
    #[serde(rename = "ConnectedVia")]
    pub connected_via: &'static str,
    #[serde(rename = "TransferMethod")]
    pub transfer_method: &'static str,
    #[serde(rename = "TransferProtocolType")]
    pub transfer_protocol_type: &'static str,
    #[serde(rename = "VerifyCertificate")]
    pub verify_certificate: bool,
    #[serde(rename = "EjectPolicy")]
    pub eject_policy: &'static str,
    #[serde(rename = "Status")]
    pub status: Status,
    #[serde(rename = "Actions")]
    pub actions: VirtualMediaActions,
}

#[derive(Debug, Serialize)]
pub struct VirtualMediaActions {
    #[serde(rename = "#VirtualMedia.InsertMedia")]
    pub insert_media: ActionTarget,
    #[serde(rename = "#VirtualMedia.EjectMedia")]
    pub eject_media: ActionTarget,
}

#[derive(Debug, Serialize)]
pub struct ActionTarget {
    pub target: String,
}

pub async fn get_virtual_media_collection(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
    Path(system_id): Path<String>,
) -> Result<Json<Collection<ODataId>>, RedfishApiError> {
    if !state.config.systems.contains_key(&system_id) {
        return Err(RedfishApiError::NotFound(format!(
            "System '{system_id}' not found"
        )));
    }

    let members = vec![ODataId::new(format!(
        "/redfish/v1/Systems/{system_id}/VirtualMedia/Cd"
    ))];

    Ok(Json(Collection::new(
        format!("/redfish/v1/Systems/{system_id}/VirtualMedia"),
        "#VirtualMediaCollection.VirtualMediaCollection",
        "Virtual Media Collection",
        members,
    )))
}

pub async fn get_virtual_media(
    State(state): State<Arc<AppState>>,
    _user: AuthenticatedUser,
    Path((system_id, media_id)): Path<(String, String)>,
) -> Result<Json<VirtualMediaResource>, RedfishApiError> {
    if !state.config.systems.contains_key(&system_id) {
        return Err(RedfishApiError::NotFound(format!(
            "System '{system_id}' not found"
        )));
    }
    if media_id != "Cd" {
        return Err(RedfishApiError::NotFound(format!(
            "VirtualMedia '{media_id}' not found"
        )));
    }

    let vm_state = state.get_vm_state(&system_id);

    let image_name = vm_state
        .virtual_media
        .image_url
        .as_ref()
        .and_then(|url| url.rsplit('/').next().map(|s| s.to_string()));

    Ok(Json(VirtualMediaResource {
        odata_id: format!("/redfish/v1/Systems/{system_id}/VirtualMedia/Cd"),
        odata_type: "#VirtualMedia.v1_6_0.VirtualMedia",
        id: "Cd",
        name: "Virtual CD",
        description: "Virtual media device",
        media_types: vec!["CD", "DVD"],
        inserted: vm_state.virtual_media.inserted,
        image: vm_state.virtual_media.image_url.clone(),
        image_name,
        user_name: None,
        password: None,
        eject_timeout: "PT0S",
        write_protected: true,
        connected_via: if vm_state.virtual_media.inserted {
            "URI"
        } else {
            "NotConnected"
        },
        transfer_method: "Stream",
        transfer_protocol_type: "HTTP",
        verify_certificate: false,
        eject_policy: "OnPowerOff",
        status: Status::enabled_ok(),
        actions: VirtualMediaActions {
            insert_media: ActionTarget {
                target: format!(
                    "/redfish/v1/Systems/{system_id}/VirtualMedia/Cd/Actions/VirtualMedia.InsertMedia"
                ),
            },
            eject_media: ActionTarget {
                target: format!(
                    "/redfish/v1/Systems/{system_id}/VirtualMedia/Cd/Actions/VirtualMedia.EjectMedia"
                ),
            },
        },
    }))
}

#[derive(Debug, Deserialize)]
pub struct InsertMediaRequest {
    #[serde(rename = "Image")]
    pub image: String,
    #[serde(rename = "Inserted", default = "default_true")]
    pub inserted: bool,
    #[serde(rename = "WriteProtected", default = "default_true")]
    pub write_protected: bool,
}

fn default_true() -> bool {
    true
}

async fn do_insert_media(
    state: &AppState,
    system_id: &str,
    body: &InsertMediaRequest,
) -> Result<Json<serde_json::Value>, RedfishApiError> {
    // Try backend-native ISO insertion (KubeVirt CDI path).
    // Falls back to download-and-hotplug for all other backends.
    if state
        .backend
        .vm_insert_iso(system_id, &body.image, "cd")
        .await
        .is_ok()
    {
        let mut vm_state = state.get_vm_state(system_id);
        vm_state.virtual_media.inserted = body.inserted;
        vm_state.virtual_media.image_url = Some(body.image.clone());
        vm_state.virtual_media.write_protected = body.write_protected;
        vm_state.virtual_media.media_type = Some("CD".to_string());
        vm_state.virtual_media.device_id = Some("cd".to_string());
        state.save_vm_state(system_id, &vm_state);

        state.event_bus.emit(RedfishEvent {
            event_type: EVENT_TYPE_RESOURCE_UPDATED.to_string(),
            event_id: uuid::Uuid::new_v4().to_string(),
            event_timestamp: Utc::now(),
            message_id: MSG_VIRTUAL_MEDIA_INSERTED.to_string(),
            message: format!("Virtual media inserted on system '{system_id}'"),
            origin_of_condition: Some(format!("/redfish/v1/Systems/{system_id}/VirtualMedia/Cd")),
            severity: SEVERITY_OK.to_string(),
            actor: None,
            payload: None,
        });

        return Ok(Json(serde_json::json!({"message": "Media inserted"})));
    }

    // Download-and-hotplug path for CH, QEMU, libvirt.
    let download_dir = state
        .config
        .systems
        .get(system_id)
        .and_then(|s| s.virtual_media_directory.clone())
        .unwrap_or_else(|| state.config.state_directory.join("media"));

    let image_path = crate::media::download_image(&body.image, &download_dir)
        .await
        .map_err(|e| RedfishApiError::InternalError(format!("Failed to download image: {e}")))?;

    if let Ok(info) = state.backend.vm_info(system_id).await
        && info.power_state == VmPowerState::On
    {
        let disk = DiskCreateConfig {
            path: Some(image_path.to_string_lossy().to_string()),
            id: Some("_vbmc_cdrom".to_string()),
            readonly: true,
            vhost_user: None,
            vhost_socket: None,
        };
        let _ = state.backend.vm_add_disk(system_id, disk).await;
    }

    let mut vm_state = state.get_vm_state(system_id);
    vm_state.virtual_media.inserted = body.inserted;
    vm_state.virtual_media.image_url = Some(body.image.clone());
    vm_state.virtual_media.image_path = Some(image_path);
    vm_state.virtual_media.write_protected = body.write_protected;
    vm_state.virtual_media.media_type = Some("CD".to_string());
    vm_state.virtual_media.device_id = Some("_vbmc_cdrom".to_string());
    state.save_vm_state(system_id, &vm_state);

    state.event_bus.emit(RedfishEvent {
        event_type: EVENT_TYPE_RESOURCE_UPDATED.to_string(),
        event_id: uuid::Uuid::new_v4().to_string(),
        event_timestamp: Utc::now(),
        message_id: MSG_VIRTUAL_MEDIA_INSERTED.to_string(),
        message: format!("Virtual media inserted on system '{system_id}'"),
        origin_of_condition: Some(format!("/redfish/v1/Systems/{system_id}/VirtualMedia/Cd")),
        severity: SEVERITY_OK.to_string(),
        actor: None,
        payload: None,
    });

    Ok(Json(serde_json::json!({"message": "Media inserted"})))
}

pub async fn insert_media(
    State(state): State<Arc<AppState>>,
    user: AuthenticatedUser,
    Path((system_id, media_id)): Path<(String, String)>,
    Json(body): Json<InsertMediaRequest>,
) -> Result<Json<serde_json::Value>, RedfishApiError> {
    if !has_privilege(&user.role, Privilege::ConfigureComponents) {
        return Err(RedfishApiError::Forbidden(
            "Insufficient privileges".to_string(),
        ));
    }

    if !state.config.systems.contains_key(&system_id) {
        return Err(RedfishApiError::NotFound(format!(
            "System '{system_id}' not found"
        )));
    }
    if media_id != "Cd" {
        return Err(RedfishApiError::NotFound(format!(
            "VirtualMedia '{media_id}' not found"
        )));
    }

    let task_id = state.task_manager.create_task("InsertMedia");
    let _lock = state.system_lock(&system_id).await;

    let result = do_insert_media(&state, &system_id, &body).await;
    match &result {
        Ok(_) => state.task_manager.complete_task(&task_id, None),
        Err(e) => state.task_manager.fail_task(&task_id, e.message()),
    }
    result
}

pub async fn eject_media(
    State(state): State<Arc<AppState>>,
    user: AuthenticatedUser,
    Path((system_id, media_id)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, RedfishApiError> {
    if !has_privilege(&user.role, Privilege::ConfigureComponents) {
        return Err(RedfishApiError::Forbidden(
            "Insufficient privileges".to_string(),
        ));
    }

    if !state.config.systems.contains_key(&system_id) {
        return Err(RedfishApiError::NotFound(format!(
            "System '{system_id}' not found"
        )));
    }
    if media_id != "Cd" {
        return Err(RedfishApiError::NotFound(format!(
            "VirtualMedia '{media_id}' not found"
        )));
    }

    let _lock = state.system_lock(&system_id).await;

    // Try backend-native eject (KubeVirt CDI: hotunplug + PVC + VIS cleanup).
    let device_id = state
        .get_vm_state(&system_id)
        .virtual_media
        .device_id
        .clone()
        .unwrap_or_else(|| "_vbmc_cdrom".to_string());

    let _ = state.backend.vm_eject_iso(&system_id, &device_id).await;

    // Also try plain hotunplug for backends that use vm_add_disk (no-op if already handled).
    if let Ok(info) = state.backend.vm_info(&system_id).await
        && info.power_state == VmPowerState::On
    {
        let _ = state.backend.vm_remove_device(&system_id, &device_id).await;
    }

    // Update state
    let mut vm_state = state.get_vm_state(&system_id);
    vm_state.virtual_media = crate::state::VirtualMediaState::default();
    state.save_vm_state(&system_id, &vm_state);

    state.event_bus.emit(RedfishEvent {
        event_type: EVENT_TYPE_RESOURCE_UPDATED.to_string(),
        event_id: uuid::Uuid::new_v4().to_string(),
        event_timestamp: Utc::now(),
        message_id: MSG_VIRTUAL_MEDIA_EJECTED.to_string(),
        message: format!("Virtual media ejected from system '{system_id}'"),
        origin_of_condition: Some(format!("/redfish/v1/Systems/{system_id}/VirtualMedia/Cd")),
        severity: SEVERITY_OK.to_string(),
        actor: None,
        payload: None,
    });

    Ok(Json(serde_json::json!({"message": "Media ejected"})))
}

#[cfg(test)]
mod tests {
    use crate::backend::mock::MockBackend;
    use crate::redfish::test_harness::*;
    use axum::http::{Method, StatusCode};

    #[tokio::test]
    async fn test_get_virtual_media_collection_success() {
        let mock = MockBackend::new().with_vm("test-system", running_vm());
        let router = router(app_state(mock, systems_with("test-system")));

        let (status, body, _) = get(&router, "/redfish/v1/Systems/test-system/VirtualMedia").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            body["@odata.type"],
            "#VirtualMediaCollection.VirtualMediaCollection"
        );
        assert_eq!(body["Name"], "Virtual Media Collection");
        assert_eq!(body["Members@odata.count"], 1);
        assert_eq!(
            body["Members"][0]["@odata.id"],
            "/redfish/v1/Systems/test-system/VirtualMedia/Cd"
        );
    }

    #[tokio::test]
    async fn test_get_virtual_media_collection_system_not_found() {
        let mock = MockBackend::new();
        let router = router(app_state(mock, systems_with("test-system")));

        let (status, body, _) =
            get(&router, "/redfish/v1/Systems/unknown-system/VirtualMedia").await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap()
                .to_lowercase()
                .contains("not found")
        );
    }

    #[tokio::test]
    async fn test_get_virtual_media_success() {
        let mock = MockBackend::new().with_vm("test-system", running_vm());
        let router = router(app_state(mock, systems_with("test-system")));

        let (status, body, _) =
            get(&router, "/redfish/v1/Systems/test-system/VirtualMedia/Cd").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["@odata.type"], "#VirtualMedia.v1_6_0.VirtualMedia");
        assert_eq!(body["Id"], "Cd");
        assert_eq!(body["Name"], "Virtual CD");
        assert_eq!(body["Inserted"], false);
        assert_eq!(body["WriteProtected"], true);
        assert_eq!(body["ConnectedVia"], "NotConnected");
        assert!(
            body["MediaTypes"]
                .as_array()
                .unwrap()
                .contains(&serde_json::json!("CD"))
        );
        assert!(
            body["MediaTypes"]
                .as_array()
                .unwrap()
                .contains(&serde_json::json!("DVD"))
        );
    }

    #[tokio::test]
    async fn test_get_virtual_media_system_not_found() {
        let mock = MockBackend::new();
        let router = router(app_state(mock, systems_with("test-system")));

        let (status, body, _) = get(
            &router,
            "/redfish/v1/Systems/unknown-system/VirtualMedia/Cd",
        )
        .await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap()
                .to_lowercase()
                .contains("not found")
        );
    }

    #[tokio::test]
    async fn test_get_virtual_media_invalid_media_id() {
        let mock = MockBackend::new().with_vm("test-system", running_vm());
        let router = router(app_state(mock, systems_with("test-system")));

        let (status, body, _) = get(
            &router,
            "/redfish/v1/Systems/test-system/VirtualMedia/Invalid",
        )
        .await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap()
                .to_lowercase()
                .contains("not found")
        );
    }

    #[tokio::test]
    async fn test_insert_media_download_failure() {
        // MockBackend returns NotSupported for vm_insert_iso, so handler falls back
        // to download path, which fails for non-existent URL. This exercises the
        // privilege check, validation, and download-and-hotplug path.
        let mock = MockBackend::new().with_vm("test-system", running_vm());
        let router = router(app_state(mock, systems_with("test-system")));

        let (status, body, _) = request_json(
            &router,
            Method::POST,
            "/redfish/v1/Systems/test-system/VirtualMedia/Cd/Actions/VirtualMedia.InsertMedia",
            serde_json::json!({
                "Image": "http://example.com/test.iso",
                "Inserted": true,
                "WriteProtected": true
            }),
        )
        .await;

        // Download fails for non-existent URL
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap()
                .to_lowercase()
                .contains("download")
        );
    }

    #[tokio::test]
    async fn test_insert_media_system_not_found() {
        let mock = MockBackend::new();
        let router = router(app_state(mock, systems_with("test-system")));

        let (status, body, _) = request_json(
            &router,
            Method::POST,
            "/redfish/v1/Systems/unknown-system/VirtualMedia/Cd/Actions/VirtualMedia.InsertMedia",
            serde_json::json!({
                "Image": "http://example.com/test.iso"
            }),
        )
        .await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap()
                .to_lowercase()
                .contains("not found")
        );
    }

    #[tokio::test]
    async fn test_insert_media_invalid_media_id() {
        let mock = MockBackend::new().with_vm("test-system", running_vm());
        let router = router(app_state(mock, systems_with("test-system")));

        let (status, body, _) = request_json(
            &router,
            Method::POST,
            "/redfish/v1/Systems/test-system/VirtualMedia/Invalid/Actions/VirtualMedia.InsertMedia",
            serde_json::json!({
                "Image": "http://example.com/test.iso"
            }),
        )
        .await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap()
                .to_lowercase()
                .contains("not found")
        );
    }

    #[tokio::test]
    async fn test_eject_media_success() {
        let mock = MockBackend::new().with_vm("test-system", running_vm());
        let router = router(app_state(mock, systems_with("test-system")));

        let (status, body, _) = request(
            &router,
            Method::POST,
            "/redfish/v1/Systems/test-system/VirtualMedia/Cd/Actions/VirtualMedia.EjectMedia",
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["message"], "Media ejected");
    }

    #[tokio::test]
    async fn test_eject_media_system_not_found() {
        let mock = MockBackend::new();
        let router = router(app_state(mock, systems_with("test-system")));

        let (status, body, _) = request(
            &router,
            Method::POST,
            "/redfish/v1/Systems/unknown-system/VirtualMedia/Cd/Actions/VirtualMedia.EjectMedia",
        )
        .await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap()
                .to_lowercase()
                .contains("not found")
        );
    }

    #[tokio::test]
    async fn test_eject_media_invalid_media_id() {
        let mock = MockBackend::new().with_vm("test-system", running_vm());
        let router = router(app_state(mock, systems_with("test-system")));

        let (status, body, _) = request(
            &router,
            Method::POST,
            "/redfish/v1/Systems/test-system/VirtualMedia/Invalid/Actions/VirtualMedia.EjectMedia",
        )
        .await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap()
                .to_lowercase()
                .contains("not found")
        );
    }
}
