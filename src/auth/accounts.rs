use std::path::Path;

use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub username: String,
    pub password_hash: String,
    pub role: String,
    pub enabled: bool,
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    pub failed_login_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lockout_until: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_scope: Option<Vec<String>>,
}

impl Account {
    pub fn set_password(&mut self, password: &str) -> anyhow::Result<()> {
        self.password_hash = Argon2::default()
            .hash_password(password.as_bytes())
            .map_err(|e| anyhow::anyhow!("Failed to hash password: {e}"))?
            .to_string();
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AccountStore {
    pub accounts: Vec<Account>,
}

impl AccountStore {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        if path.exists() {
            let content = std::fs::read_to_string(path)?;
            let store: AccountStore = serde_json::from_str(&content)?;
            Ok(store)
        } else {
            Ok(Self::default())
        }
    }

    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension("json.tmp");
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&tmp, content)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }

    pub fn find_account(&self, username: &str) -> Option<&Account> {
        self.accounts.iter().find(|a| a.username == username)
    }

    pub fn find_account_mut(&mut self, username: &str) -> Option<&mut Account> {
        self.accounts.iter_mut().find(|a| a.username == username)
    }

    pub fn verify_password(&self, username: &str, password: &str) -> bool {
        let Some(account) = self.find_account(username) else {
            return false;
        };
        if !account.enabled || account.locked {
            return false;
        }
        let Ok(parsed_hash) = PasswordHash::new(&account.password_hash) else {
            return false;
        };
        Argon2::default()
            .verify_password(password.as_bytes(), &parsed_hash)
            .is_ok()
    }

    pub fn check_and_unlock(&mut self, username: &str) {
        let Some(account) = self.find_account_mut(username) else {
            return;
        };
        if account.locked
            && let Some(until) = account.lockout_until
            && until <= Utc::now()
        {
            account.locked = false;
            account.lockout_until = None;
            account.failed_login_count = 0;
        }
    }

    pub fn record_failed_login(
        &mut self,
        username: &str,
        threshold: u32,
        duration_secs: u64,
    ) -> bool {
        let Some(account) = self.find_account_mut(username) else {
            return false;
        };
        if account.locked {
            return false;
        }
        account.failed_login_count += 1;
        if threshold > 0 && account.failed_login_count >= threshold {
            account.locked = true;
            account.lockout_until =
                Some(Utc::now() + chrono::Duration::seconds(duration_secs as i64));
            return true;
        }
        false
    }

    pub fn record_successful_login(&mut self, username: &str) {
        if let Some(account) = self.find_account_mut(username) {
            account.failed_login_count = 0;
        }
    }

    pub fn add_account(
        &mut self,
        username: &str,
        password: &str,
        role: &str,
    ) -> anyhow::Result<()> {
        let password_hash = Argon2::default()
            .hash_password(password.as_bytes())
            .map_err(|e| anyhow::anyhow!("Failed to hash password: {e}"))?
            .to_string();

        self.accounts.push(Account {
            username: username.to_string(),
            password_hash,
            role: role.to_string(),
            enabled: true,
            locked: false,
            failed_login_count: 0,
            lockout_until: None,
            system_scope: None,
        });

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_store_is_empty() {
        let store = AccountStore::default();
        assert!(store.accounts.is_empty());
    }

    #[test]
    fn test_add_account() {
        let mut store = AccountStore::default();
        store
            .add_account("admin", "secret123", "Administrator")
            .unwrap();

        assert_eq!(store.accounts.len(), 1);
        assert_eq!(store.accounts[0].username, "admin");
        assert_eq!(store.accounts[0].role, "Administrator");
        assert!(store.accounts[0].enabled);
        assert!(!store.accounts[0].locked);
        assert_eq!(store.accounts[0].failed_login_count, 0);
        // Password hash should not be the plaintext password
        assert_ne!(store.accounts[0].password_hash, "secret123");
        assert!(store.accounts[0].password_hash.starts_with("$argon2"));
    }

    #[test]
    fn test_find_account() {
        let mut store = AccountStore::default();
        store.add_account("alice", "pass1", "ReadOnly").unwrap();
        store.add_account("bob", "pass2", "Operator").unwrap();

        assert!(store.find_account("alice").is_some());
        assert_eq!(store.find_account("alice").unwrap().role, "ReadOnly");
        assert!(store.find_account("bob").is_some());
        assert!(store.find_account("charlie").is_none());
    }

    #[test]
    fn test_find_account_mut() {
        let mut store = AccountStore::default();
        store.add_account("alice", "pass", "ReadOnly").unwrap();

        let account = store.find_account_mut("alice").unwrap();
        account.locked = true;

        assert!(store.find_account("alice").unwrap().locked);
    }

    #[test]
    fn test_verify_password_correct() {
        let mut store = AccountStore::default();
        store
            .add_account("user", "correct_password", "ReadOnly")
            .unwrap();

        assert!(store.verify_password("user", "correct_password"));
    }

    #[test]
    fn test_verify_password_wrong() {
        let mut store = AccountStore::default();
        store.add_account("user", "correct", "ReadOnly").unwrap();

        assert!(!store.verify_password("user", "wrong"));
    }

    #[test]
    fn test_verify_password_nonexistent_user() {
        let store = AccountStore::default();
        assert!(!store.verify_password("nobody", "password"));
    }

    #[test]
    fn test_verify_password_disabled_account() {
        let mut store = AccountStore::default();
        store.add_account("user", "password", "ReadOnly").unwrap();
        store.accounts[0].enabled = false;

        assert!(!store.verify_password("user", "password"));
    }

    #[test]
    fn test_verify_password_locked_account() {
        let mut store = AccountStore::default();
        store.add_account("user", "password", "ReadOnly").unwrap();
        store.accounts[0].locked = true;

        assert!(!store.verify_password("user", "password"));
    }

    #[test]
    fn test_save_and_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("accounts.json");

        let mut store = AccountStore::default();
        store
            .add_account("admin", "secret", "Administrator")
            .unwrap();
        store.add_account("viewer", "view", "ReadOnly").unwrap();
        store.save(&path).unwrap();

        let loaded = AccountStore::load(&path).unwrap();
        assert_eq!(loaded.accounts.len(), 2);
        assert_eq!(loaded.accounts[0].username, "admin");
        assert_eq!(loaded.accounts[1].username, "viewer");
        // Verify loaded password hash still works
        assert!(loaded.verify_password("admin", "secret"));
    }

    #[test]
    fn test_record_failed_login_increments_count() {
        let mut store = AccountStore::default();
        store.add_account("user", "pass", "ReadOnly").unwrap();

        let locked = store.record_failed_login("user", 5, 300);
        assert!(!locked);
        assert_eq!(store.find_account("user").unwrap().failed_login_count, 1);
    }

    #[test]
    fn test_record_failed_login_locks_at_threshold() {
        let mut store = AccountStore::default();
        store.add_account("user", "pass", "ReadOnly").unwrap();

        for _ in 0..4 {
            assert!(!store.record_failed_login("user", 5, 300));
        }
        assert!(store.record_failed_login("user", 5, 300));
        assert!(store.find_account("user").unwrap().locked);
        assert!(store.find_account("user").unwrap().lockout_until.is_some());
    }

    #[test]
    fn test_record_failed_login_unknown_user() {
        let mut store = AccountStore::default();
        assert!(!store.record_failed_login("nobody", 5, 300));
    }

    #[test]
    fn test_record_successful_login_resets_count() {
        let mut store = AccountStore::default();
        store.add_account("user", "pass", "ReadOnly").unwrap();

        store.record_failed_login("user", 5, 300);
        store.record_failed_login("user", 5, 300);
        assert_eq!(store.find_account("user").unwrap().failed_login_count, 2);

        store.record_successful_login("user");
        assert_eq!(store.find_account("user").unwrap().failed_login_count, 0);
    }

    #[test]
    fn test_check_and_unlock_after_duration() {
        let mut store = AccountStore::default();
        store.add_account("user", "pass", "ReadOnly").unwrap();

        // Lock with 0-second duration (already expired)
        let account = store.find_account_mut("user").unwrap();
        account.locked = true;
        account.failed_login_count = 5;
        account.lockout_until = Some(Utc::now() - chrono::Duration::seconds(1));

        store.check_and_unlock("user");
        let account = store.find_account("user").unwrap();
        assert!(!account.locked);
        assert_eq!(account.failed_login_count, 0);
        assert!(account.lockout_until.is_none());
    }

    #[test]
    fn test_check_and_unlock_not_yet_expired() {
        let mut store = AccountStore::default();
        store.add_account("user", "pass", "ReadOnly").unwrap();

        let account = store.find_account_mut("user").unwrap();
        account.locked = true;
        account.failed_login_count = 5;
        account.lockout_until = Some(Utc::now() + chrono::Duration::seconds(300));

        store.check_and_unlock("user");
        assert!(store.find_account("user").unwrap().locked);
    }

    #[test]
    fn test_check_and_unlock_unknown_user() {
        let mut store = AccountStore::default();
        store.check_and_unlock("nobody"); // should not panic
    }

    #[test]
    fn test_load_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nonexistent.json");

        let store = AccountStore::load(&path).unwrap();
        assert!(store.accounts.is_empty());
    }
}
