//! TLS configuration helpers.
//!
//! The default configuration verifies server certificates against the
//! operating system's trust store via `rustls-platform-verifier`, and also
//! accepts certificates the user pinned (see [`PinningVerifier`]). Use
//! [`config_with_roots`] to trust only specific CA certificates (e.g. a
//! self-signed test server), or build your own [`rustls::ClientConfig`].

use std::net::IpAddr;
use std::sync::{Arc, Mutex};

use rustls_pki_types::pem::PemObject as _;
use rustls_pki_types::{CertificateDer, ServerName, UnixTime};
use rustls_platform_verifier::BuilderVerifierExt as _;
use serde::{Deserialize, Serialize};
use tokio_rustls::rustls::client::danger::{
    HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier,
};
use tokio_rustls::rustls::{
    self, ClientConfig, DigitallySignedStruct, RootCertStore, SignatureScheme,
};
use x509_parser::extensions::GeneralName;

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

/// Verifies server certificates with the platform verifier, but accepts
/// certificates whose SHA-256 fingerprint is pinned.
///
/// A pinned certificate is accepted without checking its chain, validity
/// period or names: the fingerprint identifies exactly the certificate the
/// user chose to trust, e.g. a NAS' self-signed one. Any other certificate
/// must pass the platform verifier. When it doesn't, the verifier keeps the
/// certificate, so the caller can show it and offer to pin it.
#[derive(Debug)]
pub struct PinningVerifier {
    pins: Vec<String>,
    platform: rustls_platform_verifier::Verifier,
    rejected: Mutex<Option<CertificateDer<'static>>>,
}

impl PinningVerifier {
    /// A verifier accepting the given fingerprints (hex, with or without
    /// colons, any case) in addition to what the platform trusts.
    pub fn new<S: AsRef<str>>(pins: impl IntoIterator<Item = S>) -> Result<Arc<Self>> {
        Ok(Arc::new(Self {
            pins: pins
                .into_iter()
                .map(|p| normalize_fingerprint(p.as_ref()))
                .collect(),
            platform: rustls_platform_verifier::Verifier::new(provider())?,
            rejected: Mutex::new(None),
        }))
    }

    /// A client configuration using this verifier.
    pub fn client_config(self: &Arc<Self>) -> Result<Arc<ClientConfig>> {
        let config = ClientConfig::builder_with_provider(provider())
            .with_safe_default_protocol_versions()?
            .dangerous()
            .with_custom_certificate_verifier(self.clone())
            .with_no_client_auth();
        Ok(Arc::new(config))
    }

    /// The server certificate of the last handshake that failed
    /// verification, if any.
    pub fn rejected_certificate(&self) -> Option<CertificateDer<'static>> {
        self.rejected
            .lock()
            .expect("verifier lock poisoned")
            .clone()
    }
}

impl ServerCertVerifier for PinningVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        ocsp_response: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        if self.pins.contains(&fingerprint(end_entity)) {
            return Ok(ServerCertVerified::assertion());
        }
        self.platform
            .verify_server_cert(end_entity, intermediates, server_name, ocsp_response, now)
            .inspect_err(|_| {
                *self.rejected.lock().expect("verifier lock poisoned") =
                    Some(end_entity.clone().into_owned());
            })
    }

    // The handshake signatures are checked against the certificate either
    // way, pinned or not.
    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        self.platform.verify_tls12_signature(message, cert, dss)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        self.platform.verify_tls13_signature(message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.platform.supported_verify_schemes()
    }
}

/// The SHA-256 fingerprint of a certificate as lowercase hex without
/// separators.
pub fn fingerprint(cert: &CertificateDer<'_>) -> String {
    hex(ring::digest::digest(&ring::digest::SHA256, cert.as_ref()).as_ref())
}

fn normalize_fingerprint(pin: &str) -> String {
    pin.chars()
        .filter(|c| c.is_ascii_hexdigit())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// What a user needs to decide whether to trust a certificate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CertificateInfo {
    /// SHA-256 fingerprint, see [`fingerprint`].
    pub sha256: String,
    /// Distinguished name, e.g. `CN=mail.example.com`. Empty if the
    /// certificate cannot be parsed.
    pub subject: String,
    /// Distinguished name of the issuer.
    pub issuer: String,
    /// Start of the validity period, in seconds since the Unix epoch.
    pub not_before: Option<i64>,
    /// End of the validity period, in seconds since the Unix epoch.
    pub not_after: Option<i64>,
    /// Host names and IP addresses the certificate is valid for
    /// (subject alternative names).
    pub names: Vec<String>,
}

impl CertificateInfo {
    /// Reads a DER certificate. Fields that can't be read stay empty; the
    /// fingerprint is always there.
    pub fn from_der(cert: &CertificateDer<'_>) -> Self {
        let mut info = Self {
            sha256: fingerprint(cert),
            subject: String::new(),
            issuer: String::new(),
            not_before: None,
            not_after: None,
            names: Vec::new(),
        };
        let Ok((_, parsed)) = x509_parser::parse_x509_certificate(cert.as_ref()) else {
            return info;
        };
        info.subject = parsed.subject().to_string();
        info.issuer = parsed.issuer().to_string();
        info.not_before = Some(parsed.validity().not_before.timestamp());
        info.not_after = Some(parsed.validity().not_after.timestamp());
        if let Ok(Some(san)) = parsed.subject_alternative_name() {
            info.names = san
                .value
                .general_names
                .iter()
                .filter_map(general_name)
                .collect();
        }
        info
    }
}

fn general_name(name: &GeneralName<'_>) -> Option<String> {
    match name {
        GeneralName::DNSName(dns) => Some((*dns).to_owned()),
        GeneralName::IPAddress(bytes) => match bytes.len() {
            4 => Some(IpAddr::from(<[u8; 4]>::try_from(*bytes).ok()?).to_string()),
            16 => Some(IpAddr::from(<[u8; 16]>::try_from(*bytes).ok()?).to_string()),
            _ => None,
        },
        _ => None,
    }
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
    fn sample_ca_parses() {
        let pem = include_bytes!("../tests/data/sample-ca.crt");
        let certs = certs_from_pem(pem).unwrap();
        assert_eq!(certs.len(), 1);
        config_with_roots(certs).unwrap();
    }

    #[test]
    fn certificate_info() {
        let pem = include_bytes!("../tests/data/sample-ca.crt");
        let cert = certs_from_pem(pem).unwrap().remove(0);
        let info = CertificateInfo::from_der(&cert);
        assert_eq!(info.sha256.len(), 64);
        assert_eq!(info.sha256, fingerprint(&cert));
        assert!(!info.subject.is_empty());
        assert!(info.not_before.unwrap() < info.not_after.unwrap());
    }

    #[test]
    fn unparsable_certificate_keeps_fingerprint() {
        let cert = CertificateDer::from(vec![1, 2, 3]);
        let info = CertificateInfo::from_der(&cert);
        assert_eq!(info.sha256, fingerprint(&cert));
        assert!(info.subject.is_empty() && info.not_after.is_none());
    }

    #[test]
    fn fingerprints_are_normalized() {
        assert_eq!(normalize_fingerprint("AB:cd:0F"), "abcd0f");
        let verifier = PinningVerifier::new(["AB:CD"]).unwrap();
        assert_eq!(verifier.pins, ["abcd"]);
        verifier.client_config().unwrap();
    }

    #[test]
    fn bad_pem() {
        assert!(certs_from_pem(b"nope").is_err());
        assert!(config_with_roots(Vec::new()).is_err());
    }
}
