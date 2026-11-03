//! Certificate pinning against an in-process TLS server with a self-signed
//! certificate, generated at test time.

use std::sync::Arc;
use std::time::Duration;

use managesieve::rustls::ServerConfig;
use managesieve::tls::fingerprint;
use managesieve::{Client, ConnectOptions, Error, Security};
use rustls_pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;

const GREETING: &[u8] = b"\"IMPLEMENTATION\" \"Pinning test\"\r\n\
\"SASL\" \"PLAIN\"\r\n\
\"SIEVE\" \"fileinto\"\r\n\
\"VERSION\" \"1.0\"\r\n\
OK \"ready\"\r\n";

/// Starts a ManageSieve server speaking implicit TLS with a fresh
/// self-signed certificate for `names`. Returns its port and certificate.
async fn server_for(names: &[&str]) -> (u16, CertificateDer<'static>) {
    let key =
        rcgen::generate_simple_self_signed(names.iter().map(|n| n.to_string()).collect::<Vec<_>>())
            .unwrap();
    let cert = key.cert.der().clone();
    let private = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key.signing_key.serialize_der()));
    let provider = Arc::new(managesieve::rustls::crypto::ring::default_provider());
    let config = ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(vec![cert.clone()], private)
        .unwrap();
    let acceptor = TlsAcceptor::from(Arc::new(config));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            let Ok((tcp, _)) = listener.accept().await else {
                return;
            };
            let acceptor = acceptor.clone();
            tokio::spawn(async move {
                // Clients that reject the certificate abort the handshake.
                let Ok(mut tls) = acceptor.accept(tcp).await else {
                    return;
                };
                let _ = tls.write_all(GREETING).await;
                let _ = tls.flush().await;
                let mut rest = Vec::new();
                let _ = tls.read_to_end(&mut rest).await;
            });
        }
    });
    (port, cert)
}

async fn server() -> (u16, CertificateDer<'static>) {
    server_for(&["localhost", "127.0.0.1"]).await
}

fn options(port: u16) -> ConnectOptions {
    ConnectOptions::new("localhost")
        .port(port)
        .security(Security::ImplicitTls)
        .timeout(Some(Duration::from_secs(10)))
}

#[tokio::test]
async fn untrusted_certificate_is_reported_with_details() {
    let (port, cert) = server().await;
    match Client::connect(&options(port)).await {
        Err(Error::UntrustedCertificate {
            certificate,
            reason,
        }) => {
            assert_eq!(certificate.sha256, fingerprint(&cert));
            assert!(
                certificate.names.contains(&"localhost".to_owned()),
                "{:?}",
                certificate.names
            );
            assert!(
                certificate.names.contains(&"127.0.0.1".to_owned()),
                "{:?}",
                certificate.names
            );
            assert!(certificate.not_before.is_some() && certificate.not_after.is_some());
            assert!(!reason.is_empty());
        }
        other => panic!("expected an untrusted certificate, got {other:?}"),
    }
}

#[tokio::test]
async fn pinned_certificate_is_accepted() {
    let (port, cert) = server().await;
    let client = Client::connect(&options(port).pinned_certificates([fingerprint(&cert)]))
        .await
        .unwrap();
    assert!(client.is_tls());
    assert_eq!(
        client.capabilities().implementation.as_deref(),
        Some("Pinning test")
    );
}

#[tokio::test]
async fn pinned_certificate_ignores_the_host_name() {
    // The certificate is for another name than the one connected to (like
    // a NAS' internal certificate); the pin identifies the certificate, so
    // the name doesn't matter.
    let (port, cert) = server_for(&["nas.internal.example"]).await;
    assert!(matches!(
        Client::connect(&options(port)).await,
        Err(Error::UntrustedCertificate { .. })
    ));
    Client::connect(&options(port).pinned_certificates([fingerprint(&cert)]))
        .await
        .unwrap();
}

#[tokio::test]
async fn other_pin_does_not_help() {
    let (port, _) = server().await;
    let opts = options(port).pinned_certificates(["00".repeat(32)]);
    assert!(matches!(
        Client::connect(&opts).await,
        Err(Error::UntrustedCertificate { .. })
    ));
}
