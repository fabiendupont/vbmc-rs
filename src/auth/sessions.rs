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

    pub fn get_session_by_id(&self, session_id: &str) -> Option<Session> {
        self.sessions
            .iter()
            .find(|s| s.id == session_id)
            .map(|s| s.value().clone())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_session() {
        let store = SessionStore::new(3600, 64);
        let session = store.create_session("admin", "Administrator").unwrap();

        assert_eq!(session.username, "admin");
        assert_eq!(session.role, "Administrator");
        assert!(!session.id.is_empty());
        assert!(!session.token.is_empty());
        assert!(session.expires > session.created);
    }

    #[test]
    fn test_validate_token() {
        let store = SessionStore::new(3600, 64);
        let session = store.create_session("user", "ReadOnly").unwrap();

        let validated = store.validate_token(&session.token).unwrap();
        assert_eq!(validated.username, "user");
        assert_eq!(validated.id, session.id);
    }

    #[test]
    fn test_validate_invalid_token() {
        let store = SessionStore::new(3600, 64);
        assert!(store.validate_token("bogus_token").is_none());
    }

    #[test]
    fn test_validate_expired_token() {
        let store = SessionStore::new(0, 64); // 0 second timeout
        let session = store.create_session("user", "ReadOnly").unwrap();

        // Session expires immediately (or already expired)
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(store.validate_token(&session.token).is_none());
    }

    #[test]
    fn test_delete_session_by_id() {
        let store = SessionStore::new(3600, 64);
        let session = store.create_session("user", "ReadOnly").unwrap();
        let session_id = session.id.clone();
        let token = session.token.clone();

        assert!(store.delete_session_by_id(&session_id));
        assert!(store.validate_token(&token).is_none());
    }

    #[test]
    fn test_delete_nonexistent_session() {
        let store = SessionStore::new(3600, 64);
        assert!(!store.delete_session_by_id("nonexistent"));
    }

    #[test]
    fn test_list_sessions() {
        let store = SessionStore::new(3600, 64);
        assert!(store.list_sessions().is_empty());

        store.create_session("a", "ReadOnly");
        store.create_session("b", "Operator");
        assert_eq!(store.list_sessions().len(), 2);
    }

    #[test]
    fn test_max_sessions_enforced() {
        let store = SessionStore::new(3600, 2);
        assert!(store.create_session("a", "ReadOnly").is_some());
        assert!(store.create_session("b", "ReadOnly").is_some());
        assert!(store.create_session("c", "ReadOnly").is_none()); // should fail
    }

    #[test]
    fn test_unique_tokens() {
        let store = SessionStore::new(3600, 64);
        let s1 = store.create_session("a", "ReadOnly").unwrap();
        let s2 = store.create_session("b", "ReadOnly").unwrap();
        assert_ne!(s1.token, s2.token);
        assert_ne!(s1.id, s2.id);
    }

    #[test]
    fn test_generate_token_length() {
        let token = generate_token();
        // 32 bytes base64url-encoded without padding = 43 chars
        assert_eq!(token.len(), 43);
    }
}
