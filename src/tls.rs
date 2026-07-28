use std::fs::File;
use std::io::BufReader;
use std::sync::Arc;

use rustls::ServerConfig;
use rustls::pki_types::CertificateDer;
use rustls::server::WebPkiClientVerifier;

use crate::config;

fn parse_tls_min_version(version: &str) -> anyhow::Result<&'static rustls::SupportedProtocolVersion> {
    match version {
        "1.2" => Ok(&rustls::version::TLS12),
        "1.3" => Ok(&rustls::version::TLS13),
        other => anyhow::bail!("unsupported TLS minimum version '{other}', expected '1.2' or '1.3'"),
    }
}

pub fn build_tls_config(
    server_config: &config::ServerConfig,
    tls_minimum_version: Option<&str>,
) -> anyhow::Result<Option<ServerConfig>> {
    let (cert_path, key_path) = match (&server_config.tls_cert, &server_config.tls_key) {
        (Some(cert), Some(key)) => (cert, key),
        _ => return Ok(None),
    };

    let cert_file = File::open(cert_path)?;
    let certs: Vec<CertificateDer<'static>> =
        rustls_pemfile::certs(&mut BufReader::new(cert_file)).collect::<Result<_, _>>()?;

    let key_file = File::open(key_path)?;
    let key = rustls_pemfile::private_key(&mut BufReader::new(key_file))?
        .ok_or_else(|| anyhow::anyhow!("no private key found in {}", key_path.display()))?;

    let builder = if let Some(min_ver) = tls_minimum_version {
        let version = parse_tls_min_version(min_ver)?;
        ServerConfig::builder_with_protocol_versions(&[version])
    } else {
        ServerConfig::builder()
    };

    let mut tls_config = if let Some(ca_path) = &server_config.tls_client_ca {
        let ca_file = File::open(ca_path)?;
        let ca_certs: Vec<CertificateDer<'static>> =
            rustls_pemfile::certs(&mut BufReader::new(ca_file)).collect::<Result<_, _>>()?;

        let mut root_store = rustls::RootCertStore::empty();
        for cert in ca_certs {
            root_store.add(cert)?;
        }

        let verifier = WebPkiClientVerifier::builder(Arc::new(root_store)).build()?;

        builder
            .with_client_cert_verifier(verifier)
            .with_single_cert(certs, key)?
    } else {
        builder
            .with_no_client_auth()
            .with_single_cert(certs, key)?
    };

    tls_config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

    Ok(Some(tls_config))
}
