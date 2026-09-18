use std::path::PathBuf;

use tokio::io::AsyncWriteExt;
use tokio::sync::broadcast;
use tracing::{error, info};

use super::RedfishEvent;
use crate::config::AuditLogTarget;

pub async fn audit_log_writer(
    mut rx: broadcast::Receiver<RedfishEvent>,
    target: AuditLogTarget,
    path: PathBuf,
) {
    let mut file = match target {
        AuditLogTarget::Stdout => {
            info!("Audit log writer started: stdout");
            None
        }
        AuditLogTarget::File | AuditLogTarget::Both => {
            let label = match target {
                AuditLogTarget::Both => "file + stdout",
                _ => "file",
            };
            info!("Audit log writer started: {label} ({})", path.display());

            if let Some(parent) = path.parent()
                && let Err(e) = tokio::fs::create_dir_all(parent).await
            {
                error!("Failed to create audit log directory: {e}");
                return;
            }

            match tokio::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .await
            {
                Ok(f) => Some(f),
                Err(e) => {
                    error!("Failed to open audit log file: {e}");
                    return;
                }
            }
        }
    };

    loop {
        match rx.recv().await {
            Ok(event) => {
                let mut line = match serde_json::to_string(&event) {
                    Ok(s) => s,
                    Err(e) => {
                        error!("Failed to serialize audit event: {e}");
                        continue;
                    }
                };
                line.push('\n');

                if matches!(target, AuditLogTarget::Stdout | AuditLogTarget::Both) {
                    print!("{line}");
                }

                if let Some(f) = file.as_mut() {
                    if let Err(e) = f.write_all(line.as_bytes()).await {
                        error!("Failed to write audit log: {e}");
                    }
                    if let Err(e) = f.flush().await {
                        error!("Failed to flush audit log: {e}");
                    }
                }
            }
            Err(broadcast::error::RecvError::Lagged(n)) => {
                tracing::warn!("Audit log missed {n} events");
            }
            Err(broadcast::error::RecvError::Closed) => {
                info!("Event bus closed, stopping audit log writer");
                break;
            }
        }
    }
}

#[cfg(test)]
mod coverage_tests {
    use super::*;
    use chrono::Utc;

    fn make_event(msg: &str) -> RedfishEvent {
        RedfishEvent {
            event_type: "StatusChange".to_string(),
            event_id: "test-1".to_string(),
            event_timestamp: Utc::now(),
            message_id: "Test.1.0.Message".to_string(),
            message: msg.to_string(),
            origin_of_condition: Some("/redfish/v1/Systems/test".to_string()),
            severity: "OK".to_string(),
            actor: Some("admin".to_string()),
            payload: None,
        }
    }

    #[tokio::test]
    async fn test_audit_log_file_target() {
        let (tx, rx) = broadcast::channel::<RedfishEvent>(16);
        let temp_dir = std::env::temp_dir();
        let log_path = temp_dir.join(format!("vbmc-audit-test-{}.jsonl", uuid::Uuid::new_v4()));

        let log_path_clone = log_path.clone();
        tokio::spawn(async move {
            audit_log_writer(rx, AuditLogTarget::File, log_path_clone).await;
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        tx.send(make_event("test message 1")).unwrap();
        tx.send(make_event("test message 2")).unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let content = tokio::fs::read_to_string(&log_path).await.unwrap();
        let lines: Vec<&str> = content.lines().collect();

        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("test message 1"));
        assert!(lines[1].contains("test message 2"));
        assert!(lines[0].contains("\"EventType\":\"StatusChange\""));

        let _ = tokio::fs::remove_file(&log_path).await;
    }

    #[tokio::test]
    async fn test_audit_log_creates_parent_directory() {
        let (tx, rx) = broadcast::channel::<RedfishEvent>(16);
        let temp_dir = std::env::temp_dir();
        let nested_dir = temp_dir.join(format!("vbmc-test-{}", uuid::Uuid::new_v4()));
        let log_path = nested_dir.join("audit.jsonl");

        let log_path_clone = log_path.clone();
        tokio::spawn(async move {
            audit_log_writer(rx, AuditLogTarget::File, log_path_clone).await;
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        tx.send(make_event("test")).unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        assert!(log_path.exists());

        let _ = tokio::fs::remove_dir_all(&nested_dir).await;
    }

    #[tokio::test]
    async fn test_audit_log_appends_to_existing_file() {
        let temp_dir = std::env::temp_dir();
        let log_path = temp_dir.join(format!("vbmc-audit-append-{}.jsonl", uuid::Uuid::new_v4()));

        tokio::fs::write(&log_path, "existing line\n")
            .await
            .unwrap();

        let (tx, rx) = broadcast::channel::<RedfishEvent>(16);

        let log_path_clone = log_path.clone();
        tokio::spawn(async move {
            audit_log_writer(rx, AuditLogTarget::File, log_path_clone).await;
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        tx.send(make_event("new event")).unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let content = tokio::fs::read_to_string(&log_path).await.unwrap();
        let lines: Vec<&str> = content.lines().collect();

        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "existing line");
        assert!(lines[1].contains("new event"));

        let _ = tokio::fs::remove_file(&log_path).await;
    }

    #[tokio::test]
    async fn test_audit_log_closed_channel_exits() {
        let (tx, rx) = broadcast::channel::<RedfishEvent>(16);
        let temp_dir = std::env::temp_dir();
        let log_path = temp_dir.join(format!("vbmc-audit-close-{}.jsonl", uuid::Uuid::new_v4()));

        let log_path_clone = log_path.clone();
        let handle = tokio::spawn(async move {
            audit_log_writer(rx, AuditLogTarget::File, log_path_clone).await;
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        drop(tx);

        let result = tokio::time::timeout(tokio::time::Duration::from_secs(1), handle).await;
        assert!(
            result.is_ok(),
            "audit_log_writer should exit when channel closes"
        );

        let _ = tokio::fs::remove_file(&log_path).await;
    }

    #[tokio::test]
    async fn test_audit_log_stdout_target() {
        let (tx, rx) = broadcast::channel::<RedfishEvent>(16);
        let temp_dir = std::env::temp_dir();
        let log_path = temp_dir.join("unused.jsonl");

        let log_path_clone = log_path.clone();
        tokio::spawn(async move {
            audit_log_writer(rx, AuditLogTarget::Stdout, log_path_clone).await;
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        tx.send(make_event("stdout test")).unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        assert!(!log_path.exists(), "Stdout target should not create file");
    }

    #[tokio::test]
    async fn test_audit_log_both_target() {
        let (tx, rx) = broadcast::channel::<RedfishEvent>(16);
        let temp_dir = std::env::temp_dir();
        let log_path = temp_dir.join(format!("vbmc-audit-both-{}.jsonl", uuid::Uuid::new_v4()));

        let log_path_clone = log_path.clone();
        tokio::spawn(async move {
            audit_log_writer(rx, AuditLogTarget::Both, log_path_clone).await;
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        tx.send(make_event("both test")).unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        assert!(log_path.exists());
        let content = tokio::fs::read_to_string(&log_path).await.unwrap();
        assert!(content.contains("both test"));

        let _ = tokio::fs::remove_file(&log_path).await;
    }

    #[tokio::test]
    async fn test_audit_log_event_serialization_format() {
        let (tx, rx) = broadcast::channel::<RedfishEvent>(16);
        let temp_dir = std::env::temp_dir();
        let log_path = temp_dir.join(format!("vbmc-audit-fmt-{}.jsonl", uuid::Uuid::new_v4()));

        let log_path_clone = log_path.clone();
        tokio::spawn(async move {
            audit_log_writer(rx, AuditLogTarget::File, log_path_clone).await;
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let event = RedfishEvent {
            event_type: "Alert".to_string(),
            event_id: "evt-123".to_string(),
            event_timestamp: Utc::now(),
            message_id: "Test.1.0.Critical".to_string(),
            message: "Critical failure".to_string(),
            origin_of_condition: Some("/redfish/v1/Systems/vm1".to_string()),
            severity: "Critical".to_string(),
            actor: Some("operator".to_string()),
            payload: Some(serde_json::json!({"key": "value"})),
        };

        tx.send(event).unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let content = tokio::fs::read_to_string(&log_path).await.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(content.trim()).unwrap();

        assert_eq!(parsed["EventType"], "Alert");
        assert_eq!(parsed["EventId"], "evt-123");
        assert_eq!(parsed["MessageId"], "Test.1.0.Critical");
        assert_eq!(parsed["Message"], "Critical failure");
        assert_eq!(parsed["OriginOfCondition"], "/redfish/v1/Systems/vm1");
        assert_eq!(parsed["Severity"], "Critical");
        assert_eq!(parsed["actor"], "operator");
        assert_eq!(parsed["payload"]["key"], "value");

        let _ = tokio::fs::remove_file(&log_path).await;
    }
}
