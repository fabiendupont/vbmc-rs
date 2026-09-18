use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use tracing::info;

mod config;
mod discovery;
mod k8s_auth;
mod k8s_authz;
mod proxy;
mod router;
mod state;

#[derive(Parser, Debug)]
#[command(
    name = "vbmc-rs-aggregator",
    version,
    about = "Redfish aggregator for vbmc-rs sidecars"
)]
struct Cli {
    #[arg(short, long, default_value = "/etc/vbmc-rs/aggregator.toml")]
    config: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "vbmc_rs_aggregator=info".into()),
        )
        .init();

    let cli = Cli::parse();

    let config = config::AggregatorConfig::load(&cli.config)?;
    info!("Loaded configuration from {}", cli.config.display());

    config.server.validate_tls()?;

    let addr = SocketAddr::new(config.server.bind_address.parse()?, config.server.port);

    let registry = Arc::new(discovery::SidecarRegistry::new());
    let vm_registry: Option<Arc<discovery::KubeVirtVmRegistry>> =
        if config.discovery.mode == "kubevirt-hybrid" {
            Some(Arc::new(discovery::KubeVirtVmRegistry::new()))
        } else {
            None
        };

    let cancel = CancellationToken::new();

    // Per-chassis watcher handles: chassis_name → child CancellationToken.
    let watcher_handles: Arc<
        std::sync::Mutex<std::collections::HashMap<String, CancellationToken>>,
    > = Arc::new(std::sync::Mutex::new(std::collections::HashMap::new()));

    match config.discovery.mode.as_str() {
        "static" => {
            discovery::register_static_endpoints(&registry, &config.discovery.endpoints);
        }
        #[cfg(feature = "aggregator")]
        "kubernetes" => {
            let reg = registry.clone();
            let ns = config.discovery.namespace.clone();
            let selector = config.discovery.label_selector.clone();
            let port = config.sidecar.port;
            let tls = config.sidecar.tls_enabled();
            let token = cancel.clone();
            let bmc_net = config.discovery.bmc_network.clone();
            tokio::spawn(async move {
                discovery::start_kubernetes_watcher(reg, ns, selector, port, tls, bmc_net, token)
                    .await;
            });
        }
        #[cfg(feature = "aggregator")]
        "kubevirt-hybrid" => {
            let vm_reg = vm_registry
                .clone()
                .expect("vm_registry always Some in kubevirt-hybrid");
            let port = config.sidecar.port;
            let tls = config.sidecar.tls_enabled();
            let bmc_net = config.discovery.bmc_network.clone();

            for chassis in config.effective_chassis() {
                let chassis_token = cancel.child_token();
                watcher_handles
                    .lock()
                    .unwrap()
                    .insert(chassis.name.clone(), chassis_token.clone());

                // Pod watcher per chassis.
                let reg = registry.clone();
                let ns = Some(chassis.namespace.clone());
                let selector = config.discovery.label_selector.clone();
                let bmc_net_c = bmc_net.clone();
                let token_pods = chassis_token.clone();
                tokio::spawn(async move {
                    discovery::start_kubernetes_watcher(
                        reg, ns, selector, port, tls, bmc_net_c, token_pods,
                    )
                    .await;
                });

                // VM watcher per chassis.
                let vm_reg_c = vm_reg.clone();
                let ns_vms = Some(chassis.namespace.clone());
                let chassis_name = chassis.name.clone();
                let label_sel = chassis.vm_selector.label_selector_string();
                let token_vms = chassis_token;
                tokio::spawn(async move {
                    discovery::start_kubevirt_vm_watcher(
                        vm_reg_c,
                        ns_vms,
                        chassis_name,
                        label_sel,
                        token_vms,
                    )
                    .await;
                });
            }
        }
        other => {
            anyhow::bail!("Unknown discovery mode: {other}");
        }
    }

    let proxy_client = proxy::ProxyClient::new(&config.sidecar)?;

    let account_store = config
        .auth
        .accounts_file
        .as_ref()
        .map(|p| vbmc_rs::auth::accounts::AccountStore::load(p))
        .transpose()?
        .unwrap_or_default();

    let session_store = vbmc_rs::auth::sessions::SessionStore::new(
        config.auth.session_timeout_seconds,
        config.auth.max_sessions,
    );
    session_store.start_sweeper(cancel.clone());

    let kube_client =
        if config.auth_mode == "kubernetes" || config.discovery.mode == "kubevirt-hybrid" {
            match kube::Client::try_default().await {
                Ok(c) => {
                    info!(
                        "Kubernetes client created (auth_mode={}, discovery={})",
                        config.auth_mode, config.discovery.mode
                    );
                    Some(c)
                }
                Err(e) => {
                    anyhow::bail!("Failed to create Kubernetes client: {e}");
                }
            }
        } else {
            None
        };

    let initial_chassis = config.effective_chassis();
    let app_state = Arc::new(state::AggregatorState {
        chassis_config: Arc::new(std::sync::RwLock::new(initial_chassis)),
        watcher_handles,
        config_path: cli.config.clone(),
        config: config.clone(),
        registry,
        vm_registry,
        proxy: proxy_client,
        session_store,
        account_store: std::sync::Mutex::new(account_store),
        instance_uuid: uuid::Uuid::new_v4().to_string(),
        kube_client,
        token_cache: dashmap::DashMap::new(),
        authz_cache: dashmap::DashMap::new(),
    });

    let app = router::aggregator_router(app_state.clone());

    // SIGHUP triggers a config reload on Unix.
    #[cfg(unix)]
    {
        let state_for_sighup = app_state.clone();
        tokio::spawn(async move {
            use tokio::signal::unix::{SignalKind, signal};
            let mut sighup = match signal(SignalKind::hangup()) {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!("Failed to register SIGHUP handler: {e}");
                    return;
                }
            };
            loop {
                sighup.recv().await;
                info!("SIGHUP received — reloading config");
                router::reload_chassis_config(&state_for_sighup).await;
            }
        });
    }

    let tls_server_config = vbmc_rs::tls::build_tls_config(&config.server, None)?;
    let rustls_config =
        tls_server_config.map(|c| axum_server::tls_rustls::RustlsConfig::from_config(Arc::new(c)));

    if let Some(rustls_config) = rustls_config {
        info!("Listening on {addr} (TLS)");
        let handle = axum_server::Handle::new();
        let handle_clone = handle.clone();
        tokio::spawn(async move {
            tokio::signal::ctrl_c().await.ok();
            info!("Received shutdown signal");
            handle_clone.graceful_shutdown(None);
            cancel.cancel();
        });
        axum_server::bind_rustls(addr, rustls_config)
            .handle(handle)
            .serve(app.into_make_service())
            .await?;
    } else {
        let listener = TcpListener::bind(addr).await?;
        info!("Listening on {addr}");
        let cancel_clone = cancel.clone();
        tokio::spawn(async move {
            tokio::signal::ctrl_c().await.ok();
            info!("Received shutdown signal");
            cancel_clone.cancel();
        });
        axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                cancel.cancelled().await;
            })
            .await?;
    }

    info!("Aggregator shut down");
    Ok(())
}
