use std::sync::Arc;

use dashmap::DashMap;
use tokio::sync::{Mutex, OwnedMutexGuard};

use crate::auth::accounts::AccountStore;
use crate::auth::sessions::SessionStore;
use crate::backend::Backend;
use crate::backend::mockup::MockupStore;
use crate::config::{AppConfig, SecurityPolicyConfig};
use crate::events::EventBus;
use crate::events::subscriptions::SubscriptionStore;
use crate::state::VmState;
use crate::tasks::TaskManager;

pub struct AppState {
    pub config: AppConfig,
    pub backend: Backend,
    pub vm_states: DashMap<String, VmState>,
    pub event_bus: EventBus,
    pub task_manager: TaskManager,
    pub session_store: SessionStore,
    pub account_store: std::sync::Mutex<AccountStore>,
    pub subscription_store: SubscriptionStore,
    pub security_policy: std::sync::RwLock<SecurityPolicyConfig>,
    pub tls_config: Option<axum_server::tls_rustls::RustlsConfig>,
    pub instance_uuid: String,
    pub mockup_store: Option<Arc<MockupStore>>,
    /// Chassis ID derived from config: the unique chassis_id across all systems,
    /// defaulting to "1" when none is set.
    pub chassis_id: String,
    system_locks: DashMap<String, Arc<Mutex<()>>>,
}

impl AppState {
    pub fn new(
        config: AppConfig,
        backend: Backend,
        account_store: AccountStore,
        tls_config: Option<axum_server::tls_rustls::RustlsConfig>,
        mockup_store: Option<Arc<MockupStore>>,
    ) -> Self {
        let vm_states: DashMap<String, VmState> = DashMap::new();

        for system_id in config.systems.keys() {
            let state = VmState::load(&config.state_directory, system_id)
                .unwrap_or_else(|_| VmState::new(system_id));
            vm_states.insert(system_id.clone(), state);
        }

        let session_store = SessionStore::new(
            config.auth.session_timeout_seconds,
            config.auth.max_sessions,
        );

        let security_policy = std::sync::RwLock::new(config.security_policy.clone());

        // Derive chassis_id: collect unique chassis_id values from all systems.
        // A single sidecar always has one chassis (its namespace); fall back to "1".
        let chassis_id = {
            let mut ids: Vec<&str> = config
                .systems
                .values()
                .filter_map(|s| s.chassis_id.as_deref())
                .collect();
            ids.dedup();
            if ids.len() == 1 {
                ids[0].to_string()
            } else {
                "1".to_string()
            }
        };

        Self {
            config,
            backend,
            vm_states,
            event_bus: EventBus::default(),
            task_manager: TaskManager::new(),
            session_store,
            account_store: std::sync::Mutex::new(account_store),
            security_policy,
            tls_config,
            subscription_store: SubscriptionStore::new(),
            instance_uuid: uuid::Uuid::new_v4().to_string(),
            mockup_store,
            chassis_id,
            system_locks: DashMap::new(),
        }
    }

    pub fn get_vm_state(&self, system_id: &str) -> VmState {
        self.vm_states
            .get(system_id)
            .map(|v| v.clone())
            .unwrap_or_else(|| VmState::new(system_id))
    }

    pub fn save_vm_state(&self, system_id: &str, vm_state: &VmState) {
        self.vm_states
            .insert(system_id.to_string(), vm_state.clone());
        if let Err(e) = vm_state.save(&self.config.state_directory) {
            tracing::error!("Failed to persist state for {system_id}: {e}");
        }
    }

    pub async fn system_lock(&self, system_id: &str) -> OwnedMutexGuard<()> {
        let lock = self
            .system_locks
            .entry(system_id.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone();
        lock.lock_owned().await
    }
}
