use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use axum::routing::post;
use clap::Parser;
use tracing::info;

mod mutate;

use mutate::WebhookConfig;

#[derive(Parser, Debug)]
#[command(
    name = "vbmc-rs-webhook",
    version,
    about = "Mutating admission webhook for injecting vbmc-rs sidecar into virt-launcher pods"
)]
struct Cli {
    /// Path to TLS certificate file
    #[arg(long)]
    cert: PathBuf,

    /// Path to TLS private key file
    #[arg(long)]
    key: PathBuf,

    /// Listen port
    #[arg(long, default_value_t = 8443)]
    port: u16,

    /// Sidecar container image to inject
    #[arg(long)]
    sidecar_image: String,

    /// Name of the BMC network-attachment-definition for UDN
    #[arg(long, default_value = "vbmc-bmc")]
    bmc_network: String,

    /// Secret name containing TLS certs for sidecar mTLS (optional)
    #[arg(long)]
    tls_secret: Option<String>,

    /// Keylime verifier URL for attestation (optional)
    #[arg(long)]
    keylime_url: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "vbmc_rs_webhook=info".into()),
        )
        .init();

    let cli = Cli::parse();

    let config = Arc::new(WebhookConfig {
        sidecar_image: cli.sidecar_image,
        bmc_network: cli.bmc_network,
        tls_secret: cli.tls_secret,
        keylime_url: cli.keylime_url,
    });

    let app = Router::new()
        .route("/mutate", post(mutate::handle_mutate))
        .with_state(config);

    let addr = SocketAddr::from(([0, 0, 0, 0], cli.port));

    let rustls_config = axum_server::tls_rustls::RustlsConfig::from_pem_file(&cli.cert, &cli.key)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to load TLS certificate: {e}"))?;

    info!("Webhook server listening on {addr} (TLS)");

    let handle = axum_server::Handle::new();
    let handle_clone = handle.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        info!("Received shutdown signal");
        handle_clone.graceful_shutdown(None);
    });

    axum_server::bind_rustls(addr, rustls_config)
        .handle(handle)
        .serve(app.into_make_service())
        .await?;

    info!("Webhook server shut down");
    Ok(())
}
