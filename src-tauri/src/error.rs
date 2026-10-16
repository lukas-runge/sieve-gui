//! Errors as seen by the frontend.

use managesieve::Diagnostic;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ErrorKind {
    /// Network problems: unreachable host, timeout, dropped connection.
    Connection,
    /// TLS handshake or certificate verification failed.
    Tls,
    /// The server rejected the credentials.
    Auth,
    /// The server rejected a command (`NO`), e.g. a script with errors.
    Server,
    /// The script on the server changed since it was loaded.
    Conflict,
    /// The script could not be parsed.
    Parse,
    /// Reading or writing the password in the OS keychain failed.
    Keychain,
    /// Reading or writing the app configuration failed.
    Config,
    NotFound,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub kind: ErrorKind,
    pub message: String,
    /// Line-based messages from the server, e.g. for `CHECKSCRIPT`.
    pub diagnostics: Vec<Diagnostic>,
}

impl AppError {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self { kind, message: message.into(), diagnostics: Vec::new() }
    }

    /// Maps a ManageSieve error. `during_auth` turns `NO` into [`ErrorKind::Auth`].
    pub fn from_sieve(err: managesieve::Error, during_auth: bool) -> Self {
        use managesieve::Error as E;
        let kind = match &err {
            E::Tls(_) | E::TlsConfig(_) | E::InvalidServerName(_) | E::StartTlsUnavailable => ErrorKind::Tls,
            E::No(_) | E::NoSupportedMechanism { .. } | E::Sasl(_) if during_auth => ErrorKind::Auth,
            E::No(_) | E::InvalidScriptName(_) | E::InvalidArgument(_) => ErrorKind::Server,
            _ => ErrorKind::Connection,
        };
        let diagnostics = match &err {
            E::No(resp) => resp.diagnostics(),
            _ => Vec::new(),
        };
        let message = match err.server_response() {
            Some(resp) if !resp.text.as_deref().unwrap_or("").is_empty() => {
                resp.text.clone().unwrap_or_default()
            }
            _ => err.to_string(),
        };
        Self { kind, message, diagnostics }
    }
}

impl From<sieve_core::ParseError> for AppError {
    fn from(err: sieve_core::ParseError) -> Self {
        Self::new(ErrorKind::Parse, err.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
