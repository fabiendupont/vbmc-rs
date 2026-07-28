use axum::http::{HeaderMap, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use tracing::warn;

use super::discovery::SidecarEndpoint;

pub struct ProxyClient {
    client: reqwest::Client,
}

impl ProxyClient {
    pub fn new(sidecar_config: &super::config::SidecarConnectionConfig) -> anyhow::Result<Self> {
        let mut builder = reqwest::Client::builder();

        if let (Some(ca), Some(cert), Some(key)) = (
            &sidecar_config.tls_ca,
            &sidecar_config.tls_cert,
            &sidecar_config.tls_key,
        ) {
            let ca_pem = std::fs::read(ca)?;
            builder = builder.add_root_certificate(reqwest::Certificate::from_pem(&ca_pem)?);

            let cert_pem = std::fs::read(cert)?;
            let key_pem = std::fs::read(key)?;
            let identity = reqwest::Identity::from_pem(&[cert_pem, key_pem].concat())?;
            builder = builder.identity(identity);
        }

        Ok(Self {
            client: builder.build()?,
        })
    }

    pub async fn forward(
        &self,
        endpoint: &SidecarEndpoint,
        method: Method,
        path: &str,
        headers: HeaderMap,
        body: Option<bytes::Bytes>,
    ) -> Result<Response, StatusCode> {
        let url = format!("{}{}", endpoint.url, path);

        let mut req = self.client.request(method.clone(), &url);
        for (name, value) in &headers {
            if name == "host" || name == "connection" {
                continue;
            }
            req = req.header(name, value);
        }
        if let Some(body) = body {
            req = req.body(body);
        }

        match req.send().await {
            Ok(resp) => {
                let status =
                    StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
                let resp_headers = resp.headers().clone();
                let body_bytes = resp.bytes().await.unwrap_or_default();

                let mut response = (status, body_bytes).into_response();
                for (name, value) in &resp_headers {
                    response.headers_mut().insert(name, value.clone());
                }
                Ok(response)
            }
            Err(e) => {
                warn!(url = %url, method = %method, error = %e, "Failed to proxy request to sidecar");
                Err(StatusCode::BAD_GATEWAY)
            }
        }
    }
}
