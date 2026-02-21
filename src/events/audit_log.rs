use std::path::PathBuf;

use tokio::io::AsyncWriteExt;
use tokio::sync::broadcast;
use tracing::{error, info};

use super::RedfishEvent;

pub async fn audit_log_writer(mut rx: broadcast::Receiver<RedfishEvent>, path: PathBuf) {
    info!("Audit log writer started: {}", path.display());

    if let Some(parent) = path.parent() {
        if let Err(e) = tokio::fs::create_dir_all(parent).await {
            error!("Failed to create audit log directory: {e}");
            return;
        }
    }

    let mut file = match tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .await
    {
        Ok(f) => f,
        Err(e) => {
            error!("Failed to open audit log file: {e}");
            return;
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

                if let Err(e) = file.write_all(line.as_bytes()).await {
                    error!("Failed to write audit log: {e}");
                }
                if let Err(e) = file.flush().await {
                    error!("Failed to flush audit log: {e}");
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
