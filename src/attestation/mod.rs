#[cfg(feature = "keylime")]
pub mod keylime;
#[cfg(feature = "trustee")]
pub mod trustee;
pub mod trust_chain;

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

use crate::app_state::AppState;
use crate::events::registry::*;
use crate::events::RedfishEvent;
use trust_chain::VerificationStatus;

pub struct AttestationCoordinator;

impl AttestationCoordinator {
    pub fn start_polling(
        state: Arc<AppState>,
        interval: Duration,
        cancel: CancellationToken,
    ) {
        tokio::spawn(async move {
            info!("Attestation coordinator started (interval: {:?})", interval);
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => {
                        info!("Attestation coordinator stopped");
                        break;
                    }
                    _ = tokio::time::sleep(interval) => {
                        Self::poll_all(&state).await;
                    }
                }
            }
        });
    }

    async fn poll_all(state: &Arc<AppState>) {
        for system_id in state.config.systems.keys() {
            let result = Self::check_system(state, system_id).await;
            let mut vm_state = state.get_vm_state(system_id);
            let old_status = vm_state.attestation.verification_status.clone();
            let new_status = match result {
                Ok(status) => status.to_string(),
                Err(e) => {
                    error!("Attestation check failed for {system_id}: {e}");
                    "Unknown".to_string()
                }
            };

            if old_status.as_deref() != Some(&new_status) {
                vm_state.attestation.verification_status = Some(new_status.clone());
                vm_state.attestation.last_checked = Some(Utc::now().to_rfc3339());
                state.save_vm_state(system_id, &vm_state);

                state.event_bus.emit(RedfishEvent {
                    event_type: EVENT_TYPE_STATUS_CHANGE.to_string(),
                    event_id: uuid::Uuid::new_v4().to_string(),
                    event_timestamp: Utc::now(),
                    message_id: MSG_ATTESTATION_CHANGED.to_string(),
                    message: format!(
                        "Attestation status changed for '{system_id}': {new_status}"
                    ),
                    origin_of_condition: Some(format!(
                        "/redfish/v1/ComponentIntegrity/{system_id}"
                    )),
                    severity: if new_status == "Success" {
                        SEVERITY_OK
                    } else {
                        SEVERITY_WARNING
                    }
                    .to_string(),
                    actor: None,
                    payload: None,
                });
            }
        }
    }

    async fn check_system(
        _state: &Arc<AppState>,
        _system_id: &str,
    ) -> anyhow::Result<VerificationStatus> {
        #[cfg(feature = "keylime")]
        {
            // Would call keylime verifier here
        }
        #[cfg(feature = "trustee")]
        {
            // Would call trustee/KBS here
        }
        Ok(VerificationStatus::Unknown)
    }
}
