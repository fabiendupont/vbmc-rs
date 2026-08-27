#[cfg(test)]
mod integration_tests;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use tracing::info;
use vbmc_rs::app_state::AppState;
use vbmc_rs::auth::accounts::AccountStore;
#[cfg(any(feature = "qemu", feature = "libvirt", feature = "kubevirt"))]
use vbmc_rs::backend;
use vbmc_rs::backend::Backend;
use vbmc_rs::backend::cloud_hypervisor::CloudHypervisorBackend;
use vbmc_rs::{attestation, config, events, prometheus, redfish, tls};

#[derive(Parser, Debug)]
#[command(name = "vbmc-rs", version, about = "Redfish-compliant virtual BMC")]
struct Cli {
    /// Path to configuration file
    #[arg(short, long, default_value = "/etc/vbmc-rs/config.toml")]
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
                .unwrap_or_else(|_| "vbmc_rs=info".into()),
        )
        .init();

    let cli = Cli::parse();

    let config = config::AppConfig::load(&cli.config)?;
    info!("Loaded configuration from {}", cli.config.display());

    config.server.validate_tls()?;

    let addr = SocketAddr::new(config.server.bind_address.parse()?, config.server.port);

    // Build backend based on config
    let backend = match config.backend {
        config::BackendType::CloudHypervisor => {
            let sockets = config
                .systems
                .iter()
                .filter_map(|(id, sys)| sys.socket_path.clone().map(|p| (id.clone(), p)))
                .collect();
            Backend::CloudHypervisor(CloudHypervisorBackend::new(sockets))
        }
        #[cfg(feature = "qemu")]
        config::BackendType::Qemu => backend::qemu::build_backend(&config),
        #[cfg(feature = "libvirt")]
        config::BackendType::Libvirt => backend::libvirt::build_backend(&config)?,
        #[cfg(feature = "kubevirt")]
        config::BackendType::KubeVirt => backend::kubevirt::build_backend(&config)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?,
    };

    // Load accounts
    let account_store = config
        .auth
        .accounts_file
        .as_ref()
        .map(|p| AccountStore::load(p))
        .transpose()?
        .unwrap_or_default();

    let tls_server_config = tls::build_tls_config(
        &config.server,
        config.security_policy.tls_minimum_version.as_deref(),
    )?;
    let rustls_config =
        tls_server_config.map(|c| axum_server::tls_rustls::RustlsConfig::from_config(Arc::new(c)));

    let app_state = Arc::new(AppState::new(
        config.clone(),
        backend,
        account_store,
        rustls_config.clone(),
    ));

    // Start audit log writer
    let audit_rx = app_state.event_bus.subscribe();
    let audit_path = if config.audit_log.as_os_str().is_empty() {
        config.state_directory.join("audit.jsonl")
    } else {
        config.audit_log.clone()
    };
    tokio::spawn(events::audit_log::audit_log_writer(
        audit_rx,
        config.audit_log_target,
        audit_path,
    ));

    if config.snmp_trap.enabled {
        let trap_rx = app_state.event_bus.subscribe();
        tokio::spawn(events::snmp_trap::snmp_trap_sender(
            trap_rx,
            config.snmp_trap.clone(),
        ));
    }

    let cancel = CancellationToken::new();

    // Start session sweeper
    app_state.session_store.start_sweeper(cancel.clone());

    let attestation_intervals: Vec<u64> = config
        .systems
        .values()
        .filter_map(|sys| sys.attestation.as_ref())
        .map(|att| att.poll_interval_seconds)
        .collect();
    if config.security_policy.spdm_enabled && !attestation_intervals.is_empty() {
        let interval_secs = attestation_intervals.into_iter().min().unwrap_or(30);
        info!(
            "Starting attestation coordinator (poll interval: {}s)",
            interval_secs
        );
        attestation::AttestationCoordinator::start_polling(
            app_state.clone(),
            std::time::Duration::from_secs(interval_secs),
            cancel.clone(),
        );
    }

    // Start metrics server
    if config.metrics.enabled {
        tokio::spawn(prometheus::start_metrics_server(
            config.metrics.port,
            cancel.clone(),
        ));
    }

    let app = redfish::router(app_state.clone());

    if let Some(rustls_config) = rustls_config {
        if config.server.tls_client_ca.is_some() {
            info!("Listening on {} (TLS with mutual authentication)", addr);
        } else {
            info!("Listening on {} (TLS)", addr);
        }
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
        info!("Listening on {}", addr);
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

    info!("Server shut down");
    Ok(())
}
