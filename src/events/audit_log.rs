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
