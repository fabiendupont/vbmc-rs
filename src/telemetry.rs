use std::time::Instant;

use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;
use metrics::{counter, histogram};

pub async fn metrics_middleware(request: Request, next: Next) -> Response {
    let method = request.method().to_string();
    let path = request.uri().path().to_string();
    let start = Instant::now();

    let response = next.run(request).await;

    let duration = start.elapsed().as_secs_f64();
    let status = response.status().as_u16().to_string();

    counter!("vbmc_rs_http_requests_total", "method" => method.clone(), "path" => path.clone(), "status" => status).increment(1);
    histogram!("vbmc_rs_http_request_duration_seconds", "method" => method, "path" => path)
        .record(duration);

    response
}

pub fn record_vm_power_state(system_id: &str, state: &str) {
    counter!("vbmc_rs_vm_power_state", "system" => system_id.to_string(), "state" => state.to_string()).absolute(1);
}

pub fn record_auth_attempt(success: bool) {
    let result = if success { "success" } else { "failure" };
    counter!("vbmc_rs_auth_attempts_total", "result" => result).increment(1);
}
