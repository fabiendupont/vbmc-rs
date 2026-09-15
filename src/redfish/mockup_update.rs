//! Firmware-update handlers for mockup/simulate mode.
//!
//! A wait-mode client (nvfwupd/RMS) drives an update by POSTing to one of the
//! UpdateService push URIs, reading a Task reference from the response, then
//! polling `TaskService/Tasks/<id>` until it reaches `Completed` while watching
//! `PercentComplete` climb. This module wires the three entry points nvfwupd
//! uses — the multipart push URI, the raw push URI, and the `SimpleUpdate`
//! action — to the timed task engine in [`super::mockup_tasks`], with an
//! `on_complete` hook that bumps the targeted `FirmwareInventory` versions so a
//! post-update GET reflects the new firmware.
//!
//! The typed UpdateService handlers (`update_service.rs`) serve `&'static str`
//! JSON for the config-backed backend and cannot mutate the fixture, so these
//! routes are mounted only on `mockup_router` and operate on the `MockupStore`.

use std::sync::Arc;

use axum::extract::{Multipart, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};

use crate::app_state::AppState;
use crate::backend::mockup::MockupStore;
use crate::redfish::mockup_tasks::{TaskProgress, spawn_task};

const FW_INVENTORY: &str = "/redfish/v1/UpdateService/FirmwareInventory";

/// POST to the MultipartHttpPushUri (`/redfish/v1/UpdateService/update-multipart`).
///
/// nvfwupd sends a multipart form with an `UpdateParameters` JSON part carrying
/// `{"Targets": [...]}` and an `UpdateFile` binary part. We only need the
/// targets; the firmware bytes are opaque to the simulation.
pub async fn update_multipart(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Response {
    let store = match store_from(&state) {
        Some(s) => s,
        None => return StatusCode::NOT_FOUND.into_response(),
    };

    let mut targets: Vec<String> = Vec::new();
    // Iterate parts; only `UpdateParameters` carries the target list. Drain the
    // remaining parts (notably `UpdateFile`) so the request body is consumed.
    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().map(str::to_string).unwrap_or_default();
        let bytes = match field.bytes().await {
            Ok(b) => b,
            Err(_) => return StatusCode::BAD_REQUEST.into_response(),
        };
        if name != "UpdateParameters" {
            continue;
        }
        if let Ok(params) = serde_json::from_slice::<Value>(&bytes) {
            targets = targets_from_params(&params);
        }
    }

    accept_update(store, targets)
}

/// POST to the HttpPushUri (`/redfish/v1/UpdateService/update`).
///
/// This is a raw firmware push with no in-band target list; the client selects
/// targets out of band via `HttpPushUriTargets`. Without that context we treat
/// the push as updating every `Updateable` component, which is the documented
/// default when no targets are supplied.
pub async fn update_push(State(state): State<Arc<AppState>>, _body: axum::body::Bytes) -> Response {
    let store = match store_from(&state) {
        Some(s) => s,
        None => return StatusCode::NOT_FOUND.into_response(),
    };
    accept_update(store, Vec::new())
}

/// POST to the `#UpdateService.SimpleUpdate` action.
///
/// The body is a JSON object with an `ImageURI` and an optional `Targets` list.
pub async fn simple_update(
    State(state): State<Arc<AppState>>,
    body: axum::body::Bytes,
) -> Response {
    let store = match store_from(&state) {
        Some(s) => s,
        None => return StatusCode::NOT_FOUND.into_response(),
    };
    let params: Value = serde_json::from_slice(&body).unwrap_or_default();
    let targets = targets_from_params(&params);
    accept_update(store, targets)
}

fn store_from(state: &Arc<AppState>) -> Option<Arc<MockupStore>> {
    state.mockup_store.clone()
}

/// Extract the `Targets` array from an UpdateParameters/SimpleUpdate body.
fn targets_from_params(params: &Value) -> Vec<String> {
    params
        .get("Targets")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// Spawn a timed update task and return a 202 with the Task reference.
///
/// The response carries the Task JSON (so nvfwupd reads its `Id`/`@odata.id`)
/// and a `Location` header pointing at the task, matching real BMC behavior.
fn accept_update(store: Arc<MockupStore>, targets: Vec<String>) -> Response {
    let resolved = resolve_targets(&store, &targets);
    let messages = install_messages(&resolved);

    let bump = resolved.clone();
    let task_path = spawn_task(
        store.clone(),
        "Firmware Update",
        TaskProgress::default(),
        messages,
        move |s| {
            for path in &bump {
                bump_component_version(s, path);
            }
        },
    );

    let body = store
        .get(&task_path)
        .unwrap_or_else(|| json!({ "@odata.id": task_path }));

    let mut response = (StatusCode::ACCEPTED, axum::Json(body)).into_response();
    if let Ok(location) = HeaderValue::from_str(&task_path) {
        response.headers_mut().insert(header::LOCATION, location);
    }
    response
}

/// Build the Redfish task `Messages` a wait-mode client watches for.
///
/// nvfwupd/RMS block on a start-detection loop until they see a message whose
/// `MessageId` is `Update.1.0.InstallingOnComponent`, reading `MessageArgs[1]`
/// as the component being flashed. We emit one such message per resolved
/// component (and a generic one if nothing resolved) so the client detects the
/// start promptly instead of polling for its full timeout.
fn install_messages(resolved: &[String]) -> Vec<Value> {
    let components: Vec<&str> = if resolved.is_empty() {
        vec!["firmware"]
    } else {
        resolved
            .iter()
            .map(|p| p.rsplit('/').next().unwrap_or(p.as_str()))
            .collect()
    };
    components
        .into_iter()
        .map(|comp| {
            json!({
                "@odata.type": "#Message.v1_1_1.Message",
                "MessageId": "Update.1.0.InstallingOnComponent",
                "Message": format!("Installing image on component '{comp}'."),
                "MessageArgs": ["firmware image", comp],
                "Severity": "OK",
                "Resolution": "None"
            })
        })
        .collect()
}

/// Resolve the requested targets to concrete `FirmwareInventory` resource paths.
///
/// An empty target list means "all updateable components" (the push-URI
/// default). Explicit targets are kept only when they resolve to a stored
/// resource that carries a `Version` — non-firmware targets (e.g. Chassis, used
/// by wait-mode background-copy checks in a later phase) are ignored here.
fn resolve_targets(store: &MockupStore, targets: &[String]) -> Vec<String> {
    if targets.is_empty() {
        return updateable_components(store);
    }
    targets
        .iter()
        .map(|t| t.trim_end_matches('/').to_string())
        .filter(|p| {
            store
                .get(p)
                .map(|r| r.get("Version").is_some())
                .unwrap_or(false)
        })
        .collect()
}

/// Enumerate every `FirmwareInventory` member whose `Updateable` flag is true.
fn updateable_components(store: &MockupStore) -> Vec<String> {
    let Some(collection) = store.get(FW_INVENTORY) else {
        return Vec::new();
    };
    let Some(members) = collection.get("Members").and_then(Value::as_array) else {
        return Vec::new();
    };
    members
        .iter()
        .filter_map(|m| m.get("@odata.id").and_then(Value::as_str))
        .map(|p| p.trim_end_matches('/').to_string())
        .filter(|p| {
            store
                .get(p)
                .and_then(|r| r.get("Updateable").and_then(Value::as_bool))
                .unwrap_or(false)
        })
        .collect()
}

/// Bump the `Version` string of a stored `FirmwareInventory` component so a
/// post-update GET observes the change.
fn bump_component_version(store: &MockupStore, path: &str) {
    let Some(mut resource) = store.get(path) else {
        return;
    };
    let Some(current) = resource.get("Version").and_then(Value::as_str) else {
        return;
    };
    let bumped = bump_version(current);
    resource["Version"] = json!(bumped);
    store.set(path, resource);
}

/// Increment the trailing numeric run of a version string, preserving any
/// zero-padding width. Versions with no trailing digits get a `.1` suffix.
fn bump_version(version: &str) -> String {
    let split_at = version
        .char_indices()
        .rev()
        .take_while(|(_, c)| c.is_ascii_digit())
        .last()
        .map(|(i, _)| i);

    match split_at {
        Some(i) => {
            let (prefix, digits) = version.split_at(i);
            let width = digits.len();
            let next = digits.parse::<u64>().unwrap_or(0) + 1;
            format!("{prefix}{next:0width$}")
        }
        None => format!("{version}.1"),
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn seed_inventory(store: &MockupStore) {
        store.set(
            "/redfish/v1/TaskService/Tasks",
            json!({
                "@odata.id": "/redfish/v1/TaskService/Tasks",
                "@odata.type": "#TaskCollection.TaskCollection",
                "Members": [],
                "Members@odata.count": 0,
                "Name": "Task Collection"
            }),
        );
        store.set(
            FW_INVENTORY,
            json!({
                "@odata.id": FW_INVENTORY,
                "Members": [
                    {"@odata.id": "/redfish/v1/UpdateService/FirmwareInventory/BMC_Firmware"},
                    {"@odata.id": "/redfish/v1/UpdateService/FirmwareInventory/DPU_BSP"}
                ],
                "Members@odata.count": 2
            }),
        );
        store.set(
            "/redfish/v1/UpdateService/FirmwareInventory/BMC_Firmware",
            json!({
                "@odata.id": "/redfish/v1/UpdateService/FirmwareInventory/BMC_Firmware",
                "Version": "BF-24.07-14",
                "Updateable": true
            }),
        );
        store.set(
            "/redfish/v1/UpdateService/FirmwareInventory/DPU_BSP",
            json!({
                "@odata.id": "/redfish/v1/UpdateService/FirmwareInventory/DPU_BSP",
                "Version": "4.5.0.12984",
                "Updateable": false
            }),
        );
    }

    #[test]
    fn bump_version_increments_trailing_digits() {
        assert_eq!(bump_version("4.5.0.12984"), "4.5.0.12985");
        assert_eq!(bump_version("BF-24.07-14"), "BF-24.07-15");
    }

    #[test]
    fn bump_version_preserves_zero_padding() {
        assert_eq!(bump_version("1.0.09"), "1.0.10");
        assert_eq!(bump_version("v001"), "v002");
    }

    #[test]
    fn bump_version_appends_when_no_trailing_digits() {
        assert_eq!(bump_version("golden"), "golden.1");
    }

    #[test]
    fn empty_targets_resolve_to_updateable_only() {
        let store = MockupStore::for_test();
        seed_inventory(&store);
        let resolved = resolve_targets(&store, &[]);
        assert_eq!(
            resolved,
            vec!["/redfish/v1/UpdateService/FirmwareInventory/BMC_Firmware".to_string()]
        );
    }

    #[test]
    fn explicit_targets_keep_only_versioned_resources() {
        let store = MockupStore::for_test();
        seed_inventory(&store);
        let resolved = resolve_targets(
            &store,
            &[
                "/redfish/v1/UpdateService/FirmwareInventory/DPU_BSP".to_string(),
                "/redfish/v1/Chassis/Bluefield".to_string(), // no Version, absent -> dropped
            ],
        );
        assert_eq!(
            resolved,
            vec!["/redfish/v1/UpdateService/FirmwareInventory/DPU_BSP".to_string()]
        );
    }

    #[tokio::test]
    async fn update_completes_and_bumps_targeted_version() {
        let store = Arc::new(MockupStore::for_test());
        seed_inventory(&store);

        // Force a fast task so the test does not wait the default 3s.
        let resolved =
            vec!["/redfish/v1/UpdateService/FirmwareInventory/DPU_BSP".to_string()];
        let messages = install_messages(&resolved);
        let task_path = spawn_task(
            store.clone(),
            "Firmware Update",
            TaskProgress {
                duration: Duration::from_millis(60),
                steps: 3,
            },
            messages,
            move |s| {
                for path in &resolved {
                    bump_component_version(s, path);
                }
            },
        );

        tokio::time::sleep(Duration::from_millis(250)).await;

        let task = store.get(&task_path).unwrap();
        assert_eq!(task["TaskState"], "Completed");

        let inv = store
            .get("/redfish/v1/UpdateService/FirmwareInventory/DPU_BSP")
            .unwrap();
        assert_eq!(inv["Version"], "4.5.0.12985");
    }

    #[test]
    fn install_messages_name_each_resolved_component() {
        let msgs = install_messages(&[
            "/redfish/v1/UpdateService/FirmwareInventory/BMC_Firmware".to_string(),
            "/redfish/v1/UpdateService/FirmwareInventory/DPU_BSP".to_string(),
        ]);
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0]["MessageId"], "Update.1.0.InstallingOnComponent");
        assert_eq!(msgs[0]["MessageArgs"][1], "BMC_Firmware");
        assert_eq!(msgs[1]["MessageArgs"][1], "DPU_BSP");
    }

    #[test]
    fn install_messages_fall_back_to_generic_component() {
        let msgs = install_messages(&[]);
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0]["MessageId"], "Update.1.0.InstallingOnComponent");
        assert_eq!(msgs[0]["MessageArgs"][1], "firmware");
    }
}
