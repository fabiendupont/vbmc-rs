use std::net::SocketAddr;

use axum::Router;
use axum::routing::get;
use metrics_exporter_prometheus::PrometheusBuilder;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

pub async fn start_metrics_server(port: u16, cancel: CancellationToken) {
    let builder = PrometheusBuilder::new();
    let handle = match builder.install_recorder() {
        Ok(handle) => handle,
        Err(e) => {
            error!("Failed to install prometheus recorder: {e}");
            return;
        }
    };

    let app = Router::new().route(
        "/metrics",
        get(move || {
            let h = handle.clone();
            async move { h.render() }
        }),
    );

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = match TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            error!("Failed to bind metrics server on port {port}: {e}");
            return;
        }
    };

    info!("Metrics server listening on {addr}");

    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            cancel.cancelled().await;
        })
        .await
        .ok();
}
