use std::sync::Arc;

use dashmap::DashMap;
use tokio::sync::{Mutex, OwnedMutexGuard};

use crate::auth::accounts::AccountStore;
use crate::auth::sessions::SessionStore;
use crate::backend::Backend;
use crate::config::AppConfig;
use crate::events::subscriptions::SubscriptionStore;
use crate::events::EventBus;
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
    pub instance_uuid: String,
    system_locks: DashMap<String, Arc<Mutex<()>>>,
}

impl AppState {
    pub fn new(
        config: AppConfig,
        backend: Backend,
        account_store: AccountStore,
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

        Self {
            config,
            backend,
            vm_states,
            event_bus: EventBus::default(),
            task_manager: TaskManager::new(),
            session_store,
            account_store: std::sync::Mutex::new(account_store),
            subscription_store: SubscriptionStore::new(),
            instance_uuid: uuid::Uuid::new_v4().to_string(),
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
