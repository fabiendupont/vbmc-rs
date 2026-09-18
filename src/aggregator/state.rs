use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};

use vbmc_rs::auth::accounts::AccountStore;
use vbmc_rs::auth::sessions::SessionStore;

use super::config::{AggregatorConfig, ChassisConfig};
use super::discovery::{KubeVirtVmRegistry, SidecarRegistry};
use super::k8s_auth::TokenCache;
use super::k8s_authz::AuthzCache;
use super::proxy::ProxyClient;

#[allow(dead_code)]
pub struct AggregatorState {
    pub config: AggregatorConfig,
    /// Hot-reloadable chassis list (updated on SIGHUP or POST /api/v1/config/reload).
    pub chassis_config: Arc<RwLock<Vec<ChassisConfig>>>,
    /// Per-chassis CancellationTokens for watcher lifecycle management.
    pub watcher_handles: Arc<Mutex<HashMap<String, tokio_util::sync::CancellationToken>>>,
    /// Path to the config file, used when reloading.
    pub config_path: PathBuf,
    pub registry: Arc<SidecarRegistry>,
    pub vm_registry: Option<Arc<KubeVirtVmRegistry>>,
    pub proxy: ProxyClient,
    pub session_store: SessionStore,
    pub account_store: std::sync::Mutex<AccountStore>,
    pub instance_uuid: String,
    pub kube_client: Option<kube::Client>,
    pub token_cache: TokenCache,
    pub authz_cache: AuthzCache,
}
