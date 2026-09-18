use std::fs::File;
use std::io::BufReader;
use std::sync::Arc;

use rustls::ServerConfig;
use rustls::pki_types::CertificateDer;
use rustls::server::WebPkiClientVerifier;

use crate::config;

fn parse_tls_min_version(
    version: &str,
) -> anyhow::Result<&'static rustls::SupportedProtocolVersion> {
    match version {
        "1.2" => Ok(&rustls::version::TLS12),
        "1.3" => Ok(&rustls::version::TLS13),
        other => {
            anyhow::bail!("unsupported TLS minimum version '{other}', expected '1.2' or '1.3'")
        }
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
        builder.with_no_client_auth().with_single_cert(certs, key)?
    };

    tls_config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

    Ok(Some(tls_config))
}

#[cfg(test)]
mod coverage_tests {
    use super::*;
    use std::io::Write;
    use std::path::PathBuf;

    // rustls 0.23 requires a process-level CryptoProvider before a ServerConfig
    // can be built. Production installs the ring provider in each binary's main();
    // mirror that here, guarded so it runs exactly once per test process.
    fn ensure_crypto_provider() {
        use std::sync::Once;
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            let _ = rustls::crypto::ring::default_provider().install_default();
        });
    }

    fn generate_test_cert_and_key() -> (String, String) {
        ensure_crypto_provider();
        let mut params = rcgen::CertificateParams::new(vec!["localhost".to_string()]).unwrap();
        params
            .distinguished_name
            .push(rcgen::DnType::CommonName, "Test Server");
        let key_pair = rcgen::KeyPair::generate().unwrap();
        let cert = params.self_signed(&key_pair).unwrap();

        let cert_pem = cert.pem();
        let key_pem = key_pair.serialize_pem();

        (cert_pem, key_pem)
    }

    fn write_temp_file(content: &str) -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let mut path = std::env::temp_dir();
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        path.push(format!("vbmc-test-{}-{}.pem", std::process::id(), id));
        let mut file = File::create(&path).unwrap();
        file.write_all(content.as_bytes()).unwrap();
        path
    }

    #[test]
    fn test_parse_tls_min_version_1_2() {
        let version = parse_tls_min_version("1.2").unwrap();
        assert_eq!(version, &rustls::version::TLS12);
    }

    #[test]
    fn test_parse_tls_min_version_1_3() {
        let version = parse_tls_min_version("1.3").unwrap();
        assert_eq!(version, &rustls::version::TLS13);
    }

    #[test]
    fn test_parse_tls_min_version_invalid() {
        let result = parse_tls_min_version("1.1");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("unsupported TLS minimum version"));
    }

    #[test]
    fn test_parse_tls_min_version_garbage() {
        let result = parse_tls_min_version("foobar");
        assert!(result.is_err());
    }

    #[test]
    fn test_build_tls_config_no_cert() {
        let server_config = config::ServerConfig {
            bind_address: "127.0.0.1".to_string(),
            port: 8000,
            tls_cert: None,
            tls_key: None,
            tls_client_ca: None,
        };
        let result = build_tls_config(&server_config, None).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_build_tls_config_cert_without_key() {
        let (cert_pem, _) = generate_test_cert_and_key();
        let cert_path = write_temp_file(&cert_pem);

        let server_config = config::ServerConfig {
            bind_address: "127.0.0.1".to_string(),
            port: 8000,
            tls_cert: Some(cert_path.clone()),
            tls_key: None,
            tls_client_ca: None,
        };
        let result = build_tls_config(&server_config, None).unwrap();
        assert!(result.is_none());

        let _ = std::fs::remove_file(cert_path);
    }

    #[test]
    fn test_build_tls_config_key_without_cert() {
        let (_, key_pem) = generate_test_cert_and_key();
        let key_path = write_temp_file(&key_pem);

        let server_config = config::ServerConfig {
            bind_address: "127.0.0.1".to_string(),
            port: 8000,
            tls_cert: None,
            tls_key: Some(key_path.clone()),
            tls_client_ca: None,
        };
        let result = build_tls_config(&server_config, None).unwrap();
        assert!(result.is_none());

        let _ = std::fs::remove_file(key_path);
    }

    #[test]
    fn test_build_tls_config_success() {
        let (cert_pem, key_pem) = generate_test_cert_and_key();
        let cert_path = write_temp_file(&cert_pem);
        let key_path = write_temp_file(&key_pem);

        let server_config = config::ServerConfig {
            bind_address: "127.0.0.1".to_string(),
            port: 8000,
            tls_cert: Some(cert_path.clone()),
            tls_key: Some(key_path.clone()),
            tls_client_ca: None,
        };
        let result = build_tls_config(&server_config, None).unwrap();
        assert!(result.is_some());
        let tls_config = result.unwrap();
        assert_eq!(
            tls_config.alpn_protocols,
            vec![b"h2".to_vec(), b"http/1.1".to_vec()]
        );

        let _ = std::fs::remove_file(cert_path);
        let _ = std::fs::remove_file(key_path);
    }

    #[test]
    fn test_build_tls_config_with_min_version() {
        let (cert_pem, key_pem) = generate_test_cert_and_key();
        let cert_path = write_temp_file(&cert_pem);
        let key_path = write_temp_file(&key_pem);

        let server_config = config::ServerConfig {
            bind_address: "127.0.0.1".to_string(),
            port: 8000,
            tls_cert: Some(cert_path.clone()),
            tls_key: Some(key_path.clone()),
            tls_client_ca: None,
        };
        let result = build_tls_config(&server_config, Some("1.3")).unwrap();
        assert!(result.is_some());

        let _ = std::fs::remove_file(cert_path);
        let _ = std::fs::remove_file(key_path);
    }

    #[test]
    fn test_build_tls_config_invalid_min_version() {
        let (cert_pem, key_pem) = generate_test_cert_and_key();
        let cert_path = write_temp_file(&cert_pem);
        let key_path = write_temp_file(&key_pem);

        let server_config = config::ServerConfig {
            bind_address: "127.0.0.1".to_string(),
            port: 8000,
            tls_cert: Some(cert_path.clone()),
            tls_key: Some(key_path.clone()),
            tls_client_ca: None,
        };
        let result = build_tls_config(&server_config, Some("1.1"));
        assert!(result.is_err());

        let _ = std::fs::remove_file(cert_path);
        let _ = std::fs::remove_file(key_path);
    }

    #[test]
    fn test_build_tls_config_missing_cert_file() {
        let key_path = write_temp_file("dummy key");

        let server_config = config::ServerConfig {
            bind_address: "127.0.0.1".to_string(),
            port: 8000,
            tls_cert: Some(PathBuf::from("/nonexistent/cert.pem")),
            tls_key: Some(key_path.clone()),
            tls_client_ca: None,
        };
        let result = build_tls_config(&server_config, None);
        assert!(result.is_err());

        let _ = std::fs::remove_file(key_path);
    }

    #[test]
    fn test_build_tls_config_missing_key_file() {
        let cert_path = write_temp_file("dummy cert");

        let server_config = config::ServerConfig {
            bind_address: "127.0.0.1".to_string(),
            port: 8000,
            tls_cert: Some(cert_path.clone()),
            tls_key: Some(PathBuf::from("/nonexistent/key.pem")),
            tls_client_ca: None,
        };
        let result = build_tls_config(&server_config, None);
        assert!(result.is_err());

        let _ = std::fs::remove_file(cert_path);
    }

    #[test]
    fn test_build_tls_config_malformed_cert() {
        let cert_path = write_temp_file("not a valid PEM certificate");
        let key_path = write_temp_file("not a valid PEM key");

        let server_config = config::ServerConfig {
            bind_address: "127.0.0.1".to_string(),
            port: 8000,
            tls_cert: Some(cert_path.clone()),
            tls_key: Some(key_path.clone()),
            tls_client_ca: None,
        };
        let result = build_tls_config(&server_config, None);
        assert!(result.is_err());

        let _ = std::fs::remove_file(cert_path);
        let _ = std::fs::remove_file(key_path);
    }

    #[test]
    fn test_build_tls_config_with_client_ca() {
        let (cert_pem, key_pem) = generate_test_cert_and_key();
        let cert_path = write_temp_file(&cert_pem);
        let key_path = write_temp_file(&key_pem);
        let ca_path = write_temp_file(&cert_pem);

        let server_config = config::ServerConfig {
            bind_address: "127.0.0.1".to_string(),
            port: 8000,
            tls_cert: Some(cert_path.clone()),
            tls_key: Some(key_path.clone()),
            tls_client_ca: Some(ca_path.clone()),
        };
        let result = build_tls_config(&server_config, None).unwrap();
        assert!(result.is_some());

        let _ = std::fs::remove_file(cert_path);
        let _ = std::fs::remove_file(key_path);
        let _ = std::fs::remove_file(ca_path);
    }

    #[test]
    fn test_build_tls_config_missing_ca_file() {
        let (cert_pem, key_pem) = generate_test_cert_and_key();
        let cert_path = write_temp_file(&cert_pem);
        let key_path = write_temp_file(&key_pem);

        let server_config = config::ServerConfig {
            bind_address: "127.0.0.1".to_string(),
            port: 8000,
            tls_cert: Some(cert_path.clone()),
            tls_key: Some(key_path.clone()),
            tls_client_ca: Some(PathBuf::from("/nonexistent/ca.pem")),
        };
        let result = build_tls_config(&server_config, None);
        assert!(result.is_err());

        let _ = std::fs::remove_file(cert_path);
        let _ = std::fs::remove_file(key_path);
    }

    #[test]
    fn test_build_tls_config_malformed_ca() {
        let (cert_pem, key_pem) = generate_test_cert_and_key();
        let cert_path = write_temp_file(&cert_pem);
        let key_path = write_temp_file(&key_pem);
        let ca_path = write_temp_file("not a valid CA certificate");

        let server_config = config::ServerConfig {
            bind_address: "127.0.0.1".to_string(),
            port: 8000,
            tls_cert: Some(cert_path.clone()),
            tls_key: Some(key_path.clone()),
            tls_client_ca: Some(ca_path.clone()),
        };
        let result = build_tls_config(&server_config, None);
        assert!(result.is_err());

        let _ = std::fs::remove_file(cert_path);
        let _ = std::fs::remove_file(key_path);
        let _ = std::fs::remove_file(ca_path);
    }
}
