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
    let mut rng = rand::rng();
    let bytes: Vec<u8> = (0..32).map(|_| rng.random()).collect();
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

#[cfg(test)]
mod coverage_tests {
    use super::*;

    #[test]
    fn test_session_serialization() {
        let now = Utc::now();
        let session = Session {
            id: "sess-123".to_string(),
            username: "admin".to_string(),
            role: "Administrator".to_string(),
            token: "token-abc".to_string(),
            created: now,
            expires: now + Duration::seconds(3600),
        };

        let json = serde_json::to_string(&session).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["id"], "sess-123");
        assert_eq!(parsed["username"], "admin");
        assert_eq!(parsed["role"], "Administrator");
        assert!(parsed.get("token").is_none());
    }

    #[test]
    fn test_create_session_within_limit() {
        let store = SessionStore::new(3600, 5);
        for i in 1..=5 {
            let session = store.create_session(&format!("user{}", i), "ReadOnly");
            assert!(session.is_some());
        }
    }

    #[test]
    fn test_create_session_exceeds_max() {
        let store = SessionStore::new(3600, 2);
        assert!(store.create_session("user1", "ReadOnly").is_some());
        assert!(store.create_session("user2", "ReadOnly").is_some());
        assert!(store.create_session("user3", "ReadOnly").is_none());
    }

    #[test]
    fn test_validate_token_not_expired() {
        let store = SessionStore::new(3600, 64);
        let session = store.create_session("admin", "Administrator").unwrap();

        let validated = store.validate_token(&session.token);
        assert!(validated.is_some());
    }

    #[test]
    fn test_get_session_by_id_exists() {
        let store = SessionStore::new(3600, 64);
        let session = store.create_session("admin", "Administrator").unwrap();

        let fetched = store.get_session_by_id(&session.id);
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().username, "admin");
    }

    #[test]
    fn test_get_session_by_id_nonexistent() {
        let store = SessionStore::new(3600, 64);
        assert!(store.get_session_by_id("nonexistent-id").is_none());
    }

    #[test]
    fn test_delete_session_by_id_removes_from_validate() {
        let store = SessionStore::new(3600, 64);
        let session = store.create_session("admin", "Administrator").unwrap();
        let token = session.token.clone();

        assert!(store.delete_session_by_id(&session.id));
        assert!(store.validate_token(&token).is_none());
    }

    #[test]
    fn test_list_sessions_empty() {
        let store = SessionStore::new(3600, 64);
        assert_eq!(store.list_sessions().len(), 0);
    }

    #[test]
    fn test_list_sessions_multiple() {
        let store = SessionStore::new(3600, 64);
        store.create_session("user1", "ReadOnly");
        store.create_session("user2", "Operator");
        store.create_session("user3", "Administrator");

        let sessions = store.list_sessions();
        assert_eq!(sessions.len(), 3);

        let usernames: Vec<String> = sessions.iter().map(|s| s.username.clone()).collect();
        assert!(usernames.contains(&"user1".to_string()));
        assert!(usernames.contains(&"user2".to_string()));
        assert!(usernames.contains(&"user3".to_string()));
    }

    #[tokio::test]
    async fn test_expired_sessions_removed_on_validate() {
        // The background sweeper only ticks every 60s; the authoritative
        // expiry-removal path is validate_token, which drops a session once its
        // wall-clock `expires` has passed. Verify that deterministically.
        let store = SessionStore::new(1, 64);

        let session1 = store.create_session("user1", "ReadOnly").unwrap();
        let session2 = store.create_session("user2", "ReadOnly").unwrap();
        assert_eq!(store.list_sessions().len(), 2);

        // Sessions have a 1s timeout; wait past it, then access them.
        tokio::time::sleep(tokio::time::Duration::from_millis(1100)).await;

        assert!(store.validate_token(&session1.token).is_none());
        assert!(store.validate_token(&session2.token).is_none());
        assert_eq!(store.list_sessions().len(), 0);
    }

    #[tokio::test]
    async fn test_session_sweeper_cancellation() {
        let store = SessionStore::new(3600, 64);

        let cancel = CancellationToken::new();
        store.start_sweeper(cancel.clone());

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        cancel.cancel();

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    }

    #[test]
    fn test_session_expiry_calculation() {
        let store = SessionStore::new(7200, 64);
        let session = store.create_session("admin", "Administrator").unwrap();

        let duration = (session.expires - session.created).num_seconds();
        assert_eq!(duration, 7200);
    }

    #[test]
    fn test_session_clone() {
        let now = Utc::now();
        let session = Session {
            id: "sess-1".to_string(),
            username: "test".to_string(),
            role: "ReadOnly".to_string(),
            token: "token-xyz".to_string(),
            created: now,
            expires: now + Duration::seconds(3600),
        };

        let cloned = session.clone();
        assert_eq!(cloned.id, session.id);
        assert_eq!(cloned.username, session.username);
        assert_eq!(cloned.role, session.role);
        assert_eq!(cloned.token, session.token);
        assert_eq!(cloned.created, session.created);
        assert_eq!(cloned.expires, session.expires);
    }

    #[test]
    fn test_validate_token_removes_expired() {
        let store = SessionStore::new(0, 64);
        let session = store.create_session("user", "ReadOnly").unwrap();

        std::thread::sleep(std::time::Duration::from_millis(10));

        assert!(store.validate_token(&session.token).is_none());

        let refetch = store.get_session_by_id(&session.id);
        assert!(refetch.is_none(), "expired session should be removed");
    }

    #[test]
    fn test_generate_token_uniqueness() {
        let token1 = generate_token();
        let token2 = generate_token();
        assert_ne!(token1, token2);
    }
}
