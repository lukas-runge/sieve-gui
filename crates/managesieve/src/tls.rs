//! TLS configuration helpers.
//!
//! The default configuration verifies server certificates against the
//! operating system's trust store via `rustls-platform-verifier`. Use
//! [`config_with_roots`] to trust only specific CA certificates (e.g. a
//! self-signed test server), or build your own [`rustls::ClientConfig`].

use std::sync::Arc;

use rustls_pki_types::CertificateDer;
use rustls_pki_types::pem::PemObject as _;
use rustls_platform_verifier::BuilderVerifierExt as _;
use tokio_rustls::rustls::{self, ClientConfig, RootCertStore};

use crate::error::{Error, Result};

fn provider() -> Arc<rustls::crypto::CryptoProvider> {
    Arc::new(rustls::crypto::ring::default_provider())
}

/// A client configuration that verifies certificates with the platform
/// (OS) verifier and trust store.
pub fn platform_config() -> Result<Arc<ClientConfig>> {
    let config = ClientConfig::builder_with_provider(provider())
        .with_safe_default_protocol_versions()?
        .with_platform_verifier()?
        .with_no_client_auth();
    Ok(Arc::new(config))
}

/// A client configuration that trusts only the given root certificates.
pub fn config_with_roots(
    roots: impl IntoIterator<Item = CertificateDer<'static>>,
) -> Result<Arc<ClientConfig>> {
    let mut store = RootCertStore::empty();
    for cert in roots {
        store.add(cert)?;
    }
    if store.is_empty() {
        return Err(Error::InvalidArgument("no root certificates given".into()));
    }
    let config = ClientConfig::builder_with_provider(provider())
        .with_safe_default_protocol_versions()?
        .with_root_certificates(store)
        .with_no_client_auth();
    Ok(Arc::new(config))
}

/// Parses all certificates from PEM data.
pub fn certs_from_pem(pem: &[u8]) -> Result<Vec<CertificateDer<'static>>> {
    let certs = CertificateDer::pem_slice_iter(pem)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| Error::InvalidArgument(format!("invalid PEM: {e}")))?;
    if certs.is_empty() {
        return Err(Error::InvalidArgument("no certificates in PEM data".into()));
    }
    Ok(certs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_config_builds() {
        platform_config().unwrap();
    }

    #[test]
    fn test_ca_parses() {
        let pem = include_bytes!("../tests/dovecot/certs/ca.crt");
        let certs = certs_from_pem(pem).unwrap();
        assert_eq!(certs.len(), 1);
        config_with_roots(certs).unwrap();
    }

    #[test]
    fn bad_pem() {
        assert!(certs_from_pem(b"nope").is_err());
        assert!(config_with_roots(Vec::new()).is_err());
    }
}
