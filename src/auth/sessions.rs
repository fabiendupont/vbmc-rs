use chrono::{DateTime, Duration, Utc};
use dashmap::DashMap;
use rand::Rng;
use serde::Serialize;
use tokio_util::sync::CancellationToken;
use tracing::info;

#[derive(Debug, Clone, Serialize)]
pub struct Session {
    pub id: String,
    pub username: String,
    pub role: String,
    #[serde(skip)]
    pub token: String,
    pub created: DateTime<Utc>,
    pub expires: DateTime<Utc>,
}

pub struct SessionStore {
    sessions: DashMap<String, Session>,
    timeout_seconds: u64,
    max_sessions: usize,
}

impl SessionStore {
    pub fn new(timeout_seconds: u64, max_sessions: usize) -> Self {
        Self {
            sessions: DashMap::new(),
            timeout_seconds,
            max_sessions,
        }
    }

    pub fn create_session(&self, username: &str, role: &str) -> Option<Session> {
        if self.sessions.len() >= self.max_sessions {
            return None;
        }

        let id = uuid::Uuid::new_v4().to_string();
        let token = generate_token();
        let now = Utc::now();
        let expires = now + Duration::seconds(self.timeout_seconds as i64);

        let session = Session {
            id: id.clone(),
            username: username.to_string(),
            role: role.to_string(),
            token: token.clone(),
            created: now,
            expires,
        };

        self.sessions.insert(token, session.clone());
        Some(session)
    }

    pub fn validate_token(&self, token: &str) -> Option<Session> {
        let session = self.sessions.get(token)?;
        if session.expires < Utc::now() {
            drop(session);
            self.sessions.remove(token);
            return None;
        }
        Some(session.clone())
    }

    pub fn delete_session_by_id(&self, session_id: &str) -> bool {
        let token = self
            .sessions
            .iter()
            .find(|s| s.id == session_id)
            .map(|s| s.token.clone());

        if let Some(token) = token {
            self.sessions.remove(&token);
            true
        } else {
            false
        }
    }

    pub fn list_sessions(&self) -> Vec<Session> {
        self.sessions.iter().map(|s| s.value().clone()).collect()
    }

    pub fn start_sweeper(&self, cancel: CancellationToken)
    where
        Self: 'static,
    {
        let sessions = self.sessions.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => break,
                    _ = tokio::time::sleep(std::time::Duration::from_secs(60)) => {
                        let now = Utc::now();
                        let expired: Vec<String> = sessions
                            .iter()
                            .filter(|s| s.expires < now)
                            .map(|s| s.token.clone())
                            .collect();
                        for token in &expired {
                            sessions.remove(token);
                        }
                        if !expired.is_empty() {
                            info!("Swept {} expired sessions", expired.len());
                        }
                    }
                }
            }
        });
    }
}

fn generate_token() -> String {
    let mut rng = rand::thread_rng();
    let bytes: Vec<u8> = (0..32).map(|_| rng.r#gen()).collect();
    base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, &bytes)
}
