#[cfg(feature = "keylime")]
pub mod keylime;
pub mod swtpm;
pub mod trust_chain;
#[cfg(feature = "trustee")]
pub mod trustee;

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

use crate::app_state::AppState;
use crate::events::RedfishEvent;
use crate::events::registry::*;
use trust_chain::AttestationEvidence;

pub struct AttestationCoordinator;

impl AttestationCoordinator {
    pub fn start_polling(state: Arc<AppState>, interval: Duration, cancel: CancellationToken) {
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
        if let Ok(policy) = state.security_policy.read()
            && !policy.spdm_enabled
        {
            return;
        }

        for (system_id, sys_config) in &state.config.systems {
            if sys_config.attestation.is_none() {
                continue;
            }

            let result = Self::check_system(state, system_id).await;
            let mut vm_state = state.get_vm_state(system_id);
            let old_status = vm_state.attestation.verification_status.clone();

            let (new_status, evidence) = match result {
                Ok(evidence) => {
                    let status = evidence
                        .responder_verification
                        .as_ref()
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "Unknown".to_string());
                    (status, Some(evidence))
                }
                Err(e) => {
                    error!("Attestation check failed for {system_id}: {e}");
                    ("Unknown".to_string(), None)
                }
            };

            if old_status.as_deref() != Some(&new_status) || evidence.is_some() {
                vm_state.attestation.verification_status = Some(new_status.clone());
                vm_state.attestation.last_checked = Some(Utc::now().to_rfc3339());
                if evidence.is_some() {
                    vm_state.attestation.evidence = evidence;
                }
                state.save_vm_state(system_id, &vm_state);

                if old_status.as_deref() != Some(&new_status) {
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
    }

    async fn check_system(
        state: &Arc<AppState>,
        system_id: &str,
    ) -> anyhow::Result<AttestationEvidence> {
        let sys_config = state
            .config
            .systems
            .get(system_id)
            .ok_or_else(|| anyhow::anyhow!("System '{system_id}' not found"))?;

        let att_config = sys_config
            .attestation
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No attestation config for '{system_id}'"))?;

        let agent_id = att_config.agent_id.as_deref().unwrap_or(system_id);

        match att_config.provider.as_str() {
            "swtpm" => {
                let socket = att_config
                    .swtpm_socket
                    .as_deref()
                    .unwrap_or("/var/run/swtpm/swtpm-sock");
                let pcr_indices: Vec<u32> = (0..=7).collect();
                let client = swtpm::SwtpmClient::new(socket);
                let mut evidence = client.read_pcrs(&pcr_indices).await?;

                if let Some(ref policy) = att_config.pcr_policy {
                    let status = swtpm::validate_pcrs_against_policy(&evidence, policy);
                    evidence.responder_verification = Some(status);
                }

                Ok(evidence)
            }
            #[cfg(feature = "keylime")]
            "keylime" => {
                let client = keylime::KeylimeClient::new(&att_config.provider_url);
                client.get_agent_attestation(agent_id).await
            }
            #[cfg(feature = "trustee")]
            "trustee" => {
                let client = trustee::TrusteeClient::new(&att_config.provider_url);
                client.attest_with_evidence(&[]).await
            }
            other => Err(anyhow::anyhow!(
                "Unknown attestation provider '{other}' for system '{system_id}'"
            )),
        }
    }
}
