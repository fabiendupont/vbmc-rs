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

    let cancel = CancellationToken::new();

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
            let token = cancel.clone();
            let bmc_net = config.discovery.bmc_network.clone();
            tokio::spawn(async move {
                discovery::start_kubernetes_watcher(reg, ns, selector, port, bmc_net, token).await;
            });
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

    let kube_client = if config.auth_mode == "kubernetes" {
        match kube::Client::try_default().await {
            Ok(c) => {
                info!("Kubernetes auth mode: created kube client");
                Some(c)
            }
            Err(e) => {
                anyhow::bail!("Failed to create Kubernetes client for auth: {e}");
            }
        }
    } else {
        None
    };

    let app_state = Arc::new(state::AggregatorState {
        config: config.clone(),
        registry,
        proxy: proxy_client,
        session_store,
        account_store: std::sync::Mutex::new(account_store),
        instance_uuid: uuid::Uuid::new_v4().to_string(),
        kube_client,
        token_cache: dashmap::DashMap::new(),
        authz_cache: dashmap::DashMap::new(),
    });

    let app = router::aggregator_router(app_state);

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
