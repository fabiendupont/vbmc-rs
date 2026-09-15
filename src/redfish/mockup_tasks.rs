//! Timed Task lifecycle for mockup/simulate mode.
//!
//! A real BMC returns a Task reference from a long-running action (firmware
//! update, certificate install) and the client polls
//! `TaskService/Tasks/<id>` until `TaskState` reaches `Completed`, watching
//! `PercentComplete` climb. The static mockup store only knows how to serve a
//! task that is *already* `Completed` (see `complete_task` in `mod.rs`), which
//! is fine for fire-and-forget actions but not for wait-mode clients like
//! nvfwupd/RMS that expect to observe a real poll loop.
//!
//! This module adds a task that starts `Running` at 0%, steps `PercentComplete`
//! to 100 over a configurable duration, and only then flips to `Completed`. An
//! `on_complete` hook runs against the store just before completion so an update
//! handler can apply its effect (e.g. bump a `FirmwareInventory` version)
//! atomically with the task finishing.

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use serde_json::{Value, json};

use crate::backend::mockup::MockupStore;

const TASKS_COLLECTION: &str = "/redfish/v1/TaskService/Tasks";

/// Progression shape for a spawned task.
#[derive(Debug, Clone, Copy)]
pub struct TaskProgress {
    /// Total time from `Running` (0%) to `Completed` (100%).
    pub duration: Duration,
    /// Number of intermediate `PercentComplete` updates before completion.
    pub steps: u32,
}

impl Default for TaskProgress {
    fn default() -> Self {
        Self {
            duration: Duration::from_secs(3),
            steps: 6,
        }
    }
}

/// Terminal state a spawned task settles into once its progress finishes.
///
/// A wait-mode client (nvfwupd/RMS) decides success or failure from the
/// terminal `TaskState`/`TaskStatus` and any failure `Messages`: `Exception`
/// (or a `Critical` status, or a message id containing a failure marker) is
/// read as a failed update. This lets the update handler inject failures.
#[derive(Debug, Clone)]
pub struct TaskOutcome {
    /// Terminal `TaskState` (e.g. `Completed` or `Exception`).
    pub state: String,
    /// Terminal `TaskStatus` (e.g. `OK` or `Critical`).
    pub status: String,
    /// Messages appended to the lifecycle messages at the terminal snapshot,
    /// e.g. an `Update.1.0.ApplyFailedOnComponent` detail on failure.
    pub final_messages: Vec<Value>,
}

impl TaskOutcome {
    /// A successful completion: `Completed` / `OK`, no extra messages.
    pub fn success() -> Self {
        Self {
            state: "Completed".to_string(),
            status: "OK".to_string(),
            final_messages: Vec::new(),
        }
    }

    /// A terminal failure: `Exception` / `Critical`, carrying failure messages.
    pub fn failure(final_messages: Vec<Value>) -> Self {
        Self {
            state: "Exception".to_string(),
            status: "Critical".to_string(),
            final_messages,
        }
    }

    /// Whether this outcome represents a successful completion.
    pub fn is_success(&self) -> bool {
        self.state == "Completed"
    }
}

impl Default for TaskOutcome {
    fn default() -> Self {
        Self::success()
    }
}

/// A snapshot of a Task at one point in its lifecycle, rendered by [`task_json`].
struct TaskSnapshot<'a> {
    id: u64,
    name: &'a str,
    state: &'a str,
    status: &'a str,
    percent: u8,
    start: &'a str,
    end: Option<&'a str>,
    /// Redfish task `Messages` (e.g. `Update.1.0.InstallingOnComponent`). Wait-
    /// mode clients poll for these to detect that an update has started, so they
    /// are carried through the whole lifecycle, not just at completion.
    messages: &'a [Value],
}

/// Build a Task resource for a given lifecycle snapshot.
fn task_json(s: &TaskSnapshot) -> Value {
    let task_path = format!("{TASKS_COLLECTION}/{}", s.id);
    let mut task = json!({
        "@odata.id": task_path,
        "@odata.type": "#Task.v1_7_1.Task",
        "Id": s.id.to_string(),
        "Name": s.name,
        "TaskState": s.state,
        "TaskStatus": s.status,
        "StartTime": s.start,
        "PercentComplete": s.percent,
    });
    if let Some(end) = s.end {
        task["EndTime"] = json!(end);
    }
    if !s.messages.is_empty() {
        task["Messages"] = json!(s.messages);
    }
    task
}

/// Create a `Running` task, register it in the Tasks collection, and spawn a
/// background job that steps it to `Completed`.
///
/// `on_complete` runs against the store just before the task flips to
/// `Completed`, so a caller (e.g. a firmware-update handler) can apply its
/// effect atomically with completion. Returns the task's `@odata.id`, which is
/// what the caller hands back to the client so its follow-up polls resolve.
///
/// `messages` are Redfish task `Messages` carried through the whole lifecycle.
/// Wait-mode clients (nvfwupd/RMS) poll for an `Update.1.0.InstallingOnComponent`
/// entry to detect that the update has started; without it they block on their
/// start-detection loop for the full timeout even though the task completes.
///
/// `outcome` selects the terminal state: [`TaskOutcome::success`] flips the task
/// to `Completed`/`OK`; [`TaskOutcome::failure`] flips it to `Exception`/`Critical`
/// with the supplied failure messages, which a wait-mode client reads as a failed
/// update. `on_complete` still runs in both cases (so a handler can restore
/// side effects like background-copy status regardless of outcome).
pub fn spawn_task<F>(
    store: Arc<MockupStore>,
    name: &str,
    progress: TaskProgress,
    messages: Vec<Value>,
    outcome: TaskOutcome,
    on_complete: F,
) -> String
where
    F: FnOnce(&MockupStore) + Send + 'static,
{
    let id = store.next_task_id();
    let task_path = format!("{TASKS_COLLECTION}/{id}");
    let start = Utc::now().to_rfc3339();

    // Serve the task immediately as Running at 0% so a client that polls before
    // the first progress tick still sees a valid, in-progress task.
    store.set(
        &task_path,
        task_json(&TaskSnapshot {
            id,
            name,
            state: "Running",
            status: "OK",
            percent: 0,
            start: &start,
            end: None,
            messages: &messages,
        }),
    );

    // Register in the collection. `append_member` allocates its own member id,
    // but the member must reference the task path we already minted, so the
    // closure ignores that id.
    let member_path = task_path.clone();
    let _ = store.append_member(TASKS_COLLECTION, move |_| json!({ "@odata.id": member_path }));

    let steps = progress.steps.max(1);
    let step_delay = progress.duration / steps;
    let task_path_bg = task_path.clone();
    let name = name.to_string();

    tokio::spawn(async move {
        let mut last_percent = 0u8;
        for step in 1..=steps {
            tokio::time::sleep(step_delay).await;
            // Cap intermediate progress at 99 so 100% is only ever visible once
            // the task is actually Completed.
            last_percent = ((step * 100) / steps).min(99) as u8;
            store.patch(&task_path_bg, &json!({ "PercentComplete": last_percent }));
        }
        // Apply the caller's effect, then flip to the terminal state in one set.
        on_complete(&store);
        let end = Utc::now().to_rfc3339();
        // A successful task reports 100%; a failure reports where it stopped.
        let percent = if outcome.is_success() {
            100
        } else {
            last_percent
        };
        // Lifecycle messages plus any terminal (e.g. failure) messages.
        let mut final_messages = messages;
        final_messages.extend(outcome.final_messages);
        store.set(
            &task_path_bg,
            task_json(&TaskSnapshot {
                id,
                name: &name,
                state: &outcome.state,
                status: &outcome.status,
                percent,
                start: &start,
                end: Some(&end),
                messages: &final_messages,
            }),
        );
    });

    task_path
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::*;

    fn seed_tasks_collection(store: &MockupStore) {
        store.set(
            TASKS_COLLECTION,
            json!({
                "@odata.id": TASKS_COLLECTION,
                "@odata.type": "#TaskCollection.TaskCollection",
                "Members": [],
                "Members@odata.count": 0,
                "Name": "Task Collection"
            }),
        );
    }

    #[tokio::test]
    async fn task_starts_running_and_is_registered() {
        let store = Arc::new(MockupStore::for_test());
        seed_tasks_collection(&store);

        let progress = TaskProgress {
            duration: Duration::from_millis(60),
            steps: 3,
        };
        let task_path = spawn_task(
            store.clone(),
            "Test Update",
            progress,
            Vec::new(),
            TaskOutcome::success(),
            |_| {},
        );

        // Immediately Running at 0% before any tick elapses.
        let task = store.get(&task_path).unwrap();
        assert_eq!(task["TaskState"], "Running");
        assert_eq!(task["PercentComplete"], 0);
        assert!(task.get("EndTime").is_none());

        // Registered in the Tasks collection, pointing at the minted path.
        let coll = store.get(TASKS_COLLECTION).unwrap();
        assert_eq!(coll["Members@odata.count"], 1);
        assert_eq!(coll["Members"][0]["@odata.id"], task_path);
    }

    #[tokio::test]
    async fn task_progresses_to_completed_and_runs_callback() {
        let store = Arc::new(MockupStore::for_test());
        seed_tasks_collection(&store);

        let fired = Arc::new(AtomicBool::new(false));
        let fired_cb = fired.clone();
        let progress = TaskProgress {
            duration: Duration::from_millis(60),
            steps: 3,
        };
        let task_path = spawn_task(
            store.clone(),
            "Test Update",
            progress,
            Vec::new(),
            TaskOutcome::success(),
            move |_| {
                fired_cb.store(true, Ordering::SeqCst);
            },
        );

        // Wait comfortably past the total duration.
        tokio::time::sleep(Duration::from_millis(250)).await;

        let task = store.get(&task_path).unwrap();
        assert_eq!(task["TaskState"], "Completed");
        assert_eq!(task["PercentComplete"], 100);
        assert!(task.get("EndTime").is_some());
        assert!(
            fired.load(Ordering::SeqCst),
            "on_complete hook should have run before completion"
        );
    }

    #[tokio::test]
    async fn task_carries_messages_through_lifecycle() {
        let store = Arc::new(MockupStore::for_test());
        seed_tasks_collection(&store);

        let messages = vec![json!({
            "MessageId": "Update.1.0.InstallingOnComponent",
            "MessageArgs": ["firmware image", "BMC_Firmware"],
        })];
        let progress = TaskProgress {
            duration: Duration::from_millis(60),
            steps: 3,
        };
        let task_path = spawn_task(
            store.clone(),
            "Test Update",
            progress,
            messages,
            TaskOutcome::success(),
            |_| {},
        );

        // Present while Running.
        let running = store.get(&task_path).unwrap();
        assert_eq!(
            running["Messages"][0]["MessageId"],
            "Update.1.0.InstallingOnComponent"
        );

        // And still present once Completed.
        tokio::time::sleep(Duration::from_millis(250)).await;
        let done = store.get(&task_path).unwrap();
        assert_eq!(done["TaskState"], "Completed");
        assert_eq!(
            done["Messages"][0]["MessageId"],
            "Update.1.0.InstallingOnComponent"
        );
    }

    #[tokio::test]
    async fn failure_outcome_flips_task_to_exception_with_messages() {
        let store = Arc::new(MockupStore::for_test());
        seed_tasks_collection(&store);

        let progress = TaskProgress {
            duration: Duration::from_millis(60),
            steps: 3,
        };
        let failure = TaskOutcome::failure(vec![json!({
            "MessageId": "Update.1.0.ApplyFailedOnComponent",
            "Severity": "Critical",
            "MessageArgs": ["firmware image", "BMC_Firmware"],
        })]);
        let task_path =
            spawn_task(store.clone(), "Test Update", progress, Vec::new(), failure, |_| {});

        tokio::time::sleep(Duration::from_millis(250)).await;

        let task = store.get(&task_path).unwrap();
        assert_eq!(task["TaskState"], "Exception");
        assert_eq!(task["TaskStatus"], "Critical");
        // A failed task must not claim 100%.
        assert_ne!(task["PercentComplete"], 100);
        assert_eq!(
            task["Messages"][0]["MessageId"],
            "Update.1.0.ApplyFailedOnComponent"
        );
    }

    #[tokio::test]
    async fn intermediate_progress_never_reaches_100_before_completed() {
        let store = Arc::new(MockupStore::for_test());
        seed_tasks_collection(&store);

        let progress = TaskProgress {
            duration: Duration::from_millis(120),
            steps: 4,
        };
        let task_path = spawn_task(
            store.clone(),
            "Test Update",
            progress,
            Vec::new(),
            TaskOutcome::success(),
            |_| {},
        );

        // Sample mid-flight: state must still be Running and percent < 100.
        tokio::time::sleep(Duration::from_millis(70)).await;
        let task = store.get(&task_path).unwrap();
        if task["TaskState"] == "Running" {
            let pct = task["PercentComplete"].as_u64().unwrap();
            assert!(pct < 100, "running task must not report 100% (got {pct})");
        }
    }
}
