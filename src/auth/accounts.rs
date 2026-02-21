use std::path::Path;

use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use argon2::password_hash::SaltString;
use chrono::{DateTime, Utc};
use rand::rngs::OsRng;
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

    pub fn add_account(
        &mut self,
        username: &str,
        password: &str,
        role: &str,
    ) -> anyhow::Result<()> {
        let salt = SaltString::generate(&mut OsRng);
        let password_hash = Argon2::default()
            .hash_password(password.as_bytes(), &salt)
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
