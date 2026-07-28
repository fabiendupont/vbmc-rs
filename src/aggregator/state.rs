use std::sync::Arc;

use vbmc_rs::auth::accounts::AccountStore;
use vbmc_rs::auth::sessions::SessionStore;

use super::config::AggregatorConfig;
use super::discovery::SidecarRegistry;
use super::proxy::ProxyClient;

#[allow(dead_code)]
pub struct AggregatorState {
    pub config: AggregatorConfig,
    pub registry: Arc<SidecarRegistry>,
    pub proxy: ProxyClient,
    pub session_store: SessionStore,
    pub account_store: std::sync::Mutex<AccountStore>,
    pub instance_uuid: String,
}
