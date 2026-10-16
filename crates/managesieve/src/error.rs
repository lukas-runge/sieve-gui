use tokio_rustls::rustls;

use crate::response::ServerResponse;

/// Convenience alias for results returned by this crate.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Errors returned by the ManageSieve client.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// Network I/O failed.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// TLS configuration or handshake failed (including certificate
    /// verification errors).
    #[error("TLS error: {0}")]
    Tls(#[source] std::io::Error),

    /// Building the TLS configuration failed.
    #[error("TLS configuration error: {0}")]
    TlsConfig(#[from] rustls::Error),

    /// The host name cannot be used for TLS server name verification.
    #[error("invalid TLS server name: {0}")]
    InvalidServerName(String),

    /// An operation did not complete within the configured timeout.
    #[error("operation timed out")]
    Timeout,

    /// The server closed the connection unexpectedly.
    #[error("connection closed by server")]
    ConnectionClosed,

    /// A previous error left the connection in an unknown state; reconnect.
    #[error("connection is no longer usable after a previous error")]
    ConnectionUnusable,

    /// The server sent data that does not conform to RFC 5804.
    #[error("protocol error: {0}")]
    Protocol(#[from] ParseError),

    /// The server rejected a command with a `NO` response.
    #[error("server responded NO{}", .0.describe())]
    No(ServerResponse),

    /// The server sent `BYE` and is closing the connection.
    #[error("server closed the session (BYE){}", .0.describe())]
    Bye(ServerResponse),

    /// STARTTLS was requested but the server does not advertise it.
    #[error("server does not support STARTTLS")]
    StartTlsUnavailable,

    /// None of the client's SASL mechanisms is offered by the server.
    #[error("no supported SASL mechanism offered by server (offered: {offered:?})")]
    NoSupportedMechanism {
        /// The mechanisms the server advertised.
        offered: Vec<String>,
    },

    /// The client is already authenticated.
    #[error("already authenticated")]
    AlreadyAuthenticated,

    /// The SASL exchange did not go as the mechanism expects.
    #[error("SASL error: {0}")]
    Sasl(String),

    /// A script name is not valid according to RFC 5804 §1.6.
    #[error("invalid script name: {0}")]
    InvalidScriptName(String),

    /// An argument cannot be sent to the server (e.g. too large).
    #[error("invalid argument: {0}")]
    InvalidArgument(String),
}

impl Error {
    /// The server response for [`Error::No`] and [`Error::Bye`].
    pub fn server_response(&self) -> Option<&ServerResponse> {
        match self {
            Error::No(r) | Error::Bye(r) => Some(r),
            _ => None,
        }
    }

    /// `true` if the connection can no longer be used after this error.
    pub fn is_fatal(&self) -> bool {
        !matches!(
            self,
            Error::No(_)
                | Error::InvalidScriptName(_)
                | Error::InvalidArgument(_)
                | Error::NoSupportedMechanism { .. }
                | Error::AlreadyAuthenticated
                | Error::Sasl(_)
        )
    }
}

/// Malformed or oversized data received from the server.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ParseError {
    /// An unexpected character was found.
    #[error("unexpected character {found:?} at offset {offset}")]
    Unexpected {
        /// The offending character.
        found: char,
        /// Byte offset within the line.
        offset: usize,
    },
    /// A quoted string was not terminated before the end of the line.
    #[error("unterminated quoted string at offset {offset}")]
    UnterminatedQuoted {
        /// Byte offset within the line.
        offset: usize,
    },
    /// A parenthesised list was not closed before the end of the line.
    #[error("unterminated list at offset {offset}")]
    UnterminatedList {
        /// Byte offset within the line.
        offset: usize,
    },
    /// A literal header (`{n}`) is malformed.
    #[error("malformed literal at offset {offset}")]
    InvalidLiteral {
        /// Byte offset within the line.
        offset: usize,
    },
    /// A literal exceeds the configured maximum size.
    #[error("literal exceeds maximum size of {limit} bytes")]
    LiteralTooLarge {
        /// The configured limit.
        limit: usize,
    },
    /// A line exceeds the configured maximum length.
    #[error("line exceeds maximum length of {limit} bytes")]
    LineTooLong {
        /// The configured limit.
        limit: usize,
    },
    /// Lists are nested too deeply.
    #[error("lists nested too deeply")]
    TooDeeplyNested,
    /// A string that must be UTF-8 (e.g. a script) is not.
    #[error("invalid UTF-8 in {0}")]
    InvalidUtf8(&'static str),
    /// A line does not have the shape the current command expects.
    #[error("unexpected response: {0}")]
    UnexpectedResponse(String),
    /// A single response exceeds the configured maximum size.
    #[error("response exceeds maximum size of {limit} bytes")]
    ResponseTooLarge {
        /// The configured limit.
        limit: usize,
    },
    /// Data was received after `OK` to STARTTLS, before the TLS handshake.
    #[error("unexpected data received before TLS handshake")]
    DataBeforeTls,
}
