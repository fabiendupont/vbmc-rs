use super::trust_chain::VerificationStatus;

pub struct KeylimeClient {
    base_url: String,
}

impl KeylimeClient {
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.to_string(),
        }
    }

    pub async fn get_agent_status(
        &self,
        agent_id: &str,
    ) -> anyhow::Result<VerificationStatus> {
        let url = format!("{}/v2/agents/{agent_id}", self.base_url);
        let resp = reqwest::get(&url).await?;

        if !resp.status().is_success() {
            return Ok(VerificationStatus::Unknown);
        }

        let body: serde_json::Value = resp.json().await?;
        let operational_state = body
            .get("results")
            .and_then(|r| r.get("operational_state"))
            .and_then(|s| s.as_u64())
            .unwrap_or(0);

        // Keylime operational states:
        // 7 = Get Quote (verified), others are various stages
        let status = match operational_state {
            7 => VerificationStatus::Success,
            0 => VerificationStatus::Unknown,
            _ => VerificationStatus::Failed,
        };

        Ok(status)
    }
}
