use super::trust_chain::VerificationStatus;

pub struct TrusteeClient {
    base_url: String,
}

impl TrusteeClient {
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.to_string(),
        }
    }

    pub async fn attest(
        &self,
        _evidence: &[u8],
    ) -> anyhow::Result<VerificationStatus> {
        let url = format!("{}/kbs/v0/attest", self.base_url);
        let resp = reqwest::Client::new()
            .post(&url)
            .header("Content-Type", "application/json")
            .body("{}")
            .send()
            .await?;

        if resp.status().is_success() {
            Ok(VerificationStatus::Success)
        } else {
            Ok(VerificationStatus::Failed)
        }
    }
}
