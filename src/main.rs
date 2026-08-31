#[cfg(test)]
mod integration_tests;

use std::io;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use clap_complete::Shell;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use tracing::info;
use vbmc_rs::app_state::AppState;
use vbmc_rs::auth::accounts::AccountStore;
#[cfg(any(feature = "qemu", feature = "libvirt", feature = "kubevirt"))]
use vbmc_rs::backend;
use vbmc_rs::backend::Backend;
use vbmc_rs::backend::cloud_hypervisor::CloudHypervisorBackend;
use vbmc_rs::backend::mockup::{MockupBackend, MockupStore};
use vbmc_rs::{attestation, config, events, prometheus, redfish, tls};

#[derive(Parser, Debug)]
#[command(name = "vbmc-rs", version, about = "Redfish-compliant virtual BMC")]
struct Cli {
    /// Path to configuration file
    #[arg(short, long, default_value = "/etc/vbmc-rs/config.toml", global = true)]
    config: PathBuf,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(clap::Subcommand, Debug)]
enum Command {
    /// Generate a starter configuration file
    Init {
        /// Backend type (cloud_hypervisor, qemu, libvirt, kube_virt, mockup)
        #[arg(short, long, default_value = "cloud_hypervisor")]
        backend: String,

        /// Output file path
        #[arg(short, long, default_value = "config.toml")]
        output: PathBuf,
    },
    /// Validate configuration without starting the server
    Validate,
    /// Generate shell completions
    Completions {
        /// Shell type
        shell: Shell,
    },
    /// Start a simulated BMC fleet with no config file or hypervisor needed
    Simulate {
        /// Number of simulated servers
        #[arg(short, long, default_value_t = 1)]
        systems: usize,

        /// Listen port
        #[arg(short, long, default_value_t = 8000)]
        port: u16,
    },
}

fn generate_init_config(backend: &str) -> anyhow::Result<String> {
    let config = match backend {
        "cloud_hypervisor" => {
            r#"backend = "cloud_hypervisor"

[server]
bind_address = "127.0.0.1"
port = 8000

[systems.vm1]
name = "My VM"
socket_path = "/tmp/cloud-hypervisor-vm1.sock"

[systems.vm1.hardware]
cpu_count = 2
memory_mib = 1024

[[systems.vm1.hardware.disks]]
path = "/var/lib/images/vm1.qcow2"
id = "rootdisk"
"#
        }
        "qemu" => {
            r#"backend = "qemu"

[server]
bind_address = "127.0.0.1"
port = 8000

[systems.vm1]
name = "My QEMU VM"
socket_path = "/tmp/qmp-vm1.sock"
"#
        }
        "libvirt" => {
            r#"backend = "libvirt"

[server]
bind_address = "127.0.0.1"
port = 8000

[systems.vm1]
name = "My Libvirt VM"
connection_uri = "qemu:///system"
domain_name = "my-domain"
"#
        }
        "kube_virt" => {
            r#"backend = "kube_virt"

[server]
bind_address = "0.0.0.0"
port = 8000

[systems.vm1]
name = "KubeVirt VM 1"
namespace = "default"
vm_name = "my-test-vm"

[systems.vm1.hardware]
cpu_count = 2
memory_mib = 2048
"#
        }
        "mockup" => {
            r#"backend = "mockup"
mockup_directory = "./mockup"

[server]
bind_address = "127.0.0.1"
port = 8000
"#
        }
        other => anyhow::bail!(
            "Unknown backend: {other}. Valid options: cloud_hypervisor, qemu, libvirt, kube_virt, mockup"
        ),
    };
    Ok(config.to_string())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Command::Init { backend, output }) => {
            let config = generate_init_config(&backend)?;
            if output.exists() {
                anyhow::bail!("{} already exists, refusing to overwrite", output.display());
            }
            std::fs::write(&output, &config)?;
            println!("Generated {} config at {}", backend, output.display());
            return Ok(());
        }
        Some(Command::Validate) => {
            let config = config::AppConfig::load(&cli.config)?;
            config.server.validate_tls()?;
            println!(
                "Configuration valid: {} backend, {} system(s)",
                format!("{:?}", config.backend).to_lowercase(),
                config.systems.len()
            );
            return Ok(());
        }
        Some(Command::Completions { shell }) => {
            clap_complete::generate(
                shell,
                &mut <Cli as clap::CommandFactory>::command(),
                "vbmc-rs",
                &mut io::stdout(),
            );
            return Ok(());
        }
        Some(Command::Simulate { systems, port }) => {
            tracing_subscriber::fmt()
                .with_env_filter(
                    tracing_subscriber::EnvFilter::try_from_default_env()
                        .unwrap_or_else(|_| "vbmc_rs=info".into()),
                )
                .init();

            let store = Arc::new(MockupStore::generate(systems));
            let config = config::AppConfig::simulate(port);
            let app_state = Arc::new(AppState::new(
                config,
                Backend::Mockup(MockupBackend::new(store.clone())),
                AccountStore::default(),
                None,
                Some(store),
            ));
            let addr = SocketAddr::new("127.0.0.1".parse()?, port);
            let app = redfish::router(app_state);
            let listener = TcpListener::bind(addr).await?;
            info!("Simulating {} server(s) at http://{}", systems, addr);
            info!("Try: curl -s http://{}/redfish/v1/Systems | jq .", addr);
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    tokio::signal::ctrl_c().await.ok();
                })
                .await?;
            return Ok(());
        }
        None => {}
    }

    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "vbmc_rs=info".into()),
        )
        .init();

    let config = config::AppConfig::load(&cli.config)?;
    info!("Loaded configuration from {}", cli.config.display());

    config.server.validate_tls()?;

    let addr = SocketAddr::new(config.server.bind_address.parse()?, config.server.port);

    // Build backend based on config
    let mut mockup_store: Option<Arc<MockupStore>> = None;
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
        config::BackendType::Mockup => {
            let dir = config.mockup_directory.as_ref().ok_or_else(|| {
                anyhow::anyhow!("mockup_directory is required for mockup backend")
            })?;
            let store = Arc::new(MockupStore::load(dir)?);
            mockup_store = Some(store.clone());
            Backend::Mockup(MockupBackend::new(store))
        }
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
        mockup_store,
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
