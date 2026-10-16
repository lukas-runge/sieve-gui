//! Read-only IMAP access to the user's mail store: the folder list for the
//! folder picker (and, later, messages for the dry run).
//!
//! Connection security and TLS verification behave exactly like the
//! ManageSieve client: STARTTLS never downgrades, certificates are verified
//! with the platform verifier unless a [`ClientConfig`] is given.

use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use async_imap::types::NameAttribute;
use futures_util::TryStreamExt as _;
use managesieve::rustls::ClientConfig;
use rustls_pki_types::ServerName;
use serde::Serialize;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;
use tokio_rustls::client::TlsStream;

pub use managesieve::Security;

/// The IANA-assigned port for IMAP over implicit TLS.
pub const IMAPS_PORT: u16 = 993;
/// The IANA-assigned port for IMAP (with STARTTLS).
pub const IMAP_PORT: u16 = 143;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Io(#[from] io::Error),
    #[error("timed out")]
    Timeout,
    #[error("TLS: {0}")]
    Tls(String),
    #[error("invalid server name: {0}")]
    InvalidServerName(String),
    /// The server rejected the credentials.
    #[error("authentication failed: {0}")]
    Auth(String),
    #[error("{0}")]
    Imap(#[from] async_imap::error::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Options for [`list_folders`].
#[derive(Clone)]
pub struct ConnectOptions {
    pub host: String,
    pub port: u16,
    pub security: Security,
    /// `None` uses [`managesieve::tls::platform_config`].
    pub tls_config: Option<Arc<ClientConfig>>,
    /// Timeout for the whole operation.
    pub timeout: Duration,
}

impl ConnectOptions {
    /// Options for `host` with implicit TLS on port 993 and a 60 s timeout.
    pub fn new(host: impl Into<String>) -> Self {
        Self {
            host: host.into(),
            port: IMAPS_PORT,
            security: Security::ImplicitTls,
            tls_config: None,
            timeout: Duration::from_secs(60),
        }
    }

    pub fn port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    pub fn security(mut self, security: Security) -> Self {
        self.security = security;
        self
    }

    pub fn tls_config(mut self, config: Arc<ClientConfig>) -> Self {
        self.tls_config = Some(config);
        self
    }
}

/// A mailbox as listed by the server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Folder {
    /// The name as the server sends it, usually modified UTF-7.
    pub name: String,
    /// Hierarchy delimiter, e.g. `/` or `.`.
    pub delimiter: Option<String>,
    /// `false` for `\Noselect` / `\NonExistent` (pure hierarchy nodes).
    pub selectable: bool,
    /// Special use (RFC 6154), if the server announces one.
    pub special_use: Option<SpecialUse>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SpecialUse {
    All,
    Archive,
    Drafts,
    Flagged,
    Junk,
    Sent,
    Trash,
}

/// Logs in and returns all folders (`LIST "" "*"`), sorted with INBOX first.
pub async fn list_folders(
    opts: &ConnectOptions,
    username: &str,
    password: &str,
) -> Result<Vec<Folder>> {
    match tokio::time::timeout(opts.timeout, list_folders_inner(opts, username, password)).await {
        Ok(result) => result,
        Err(_) => Err(Error::Timeout),
    }
}

async fn list_folders_inner(
    opts: &ConnectOptions,
    username: &str,
    password: &str,
) -> Result<Vec<Folder>> {
    let client = connect(opts).await?;
    let mut session = client
        .login(username, password)
        .await
        .map_err(|(e, _)| match e {
            async_imap::error::Error::No(text) => Error::Auth(text),
            e => Error::Imap(e),
        })?;
    let mut folders: Vec<Folder> = session
        .list(Some(""), Some("*"))
        .await?
        .map_ok(|name| folder(&name))
        .try_collect()
        .await?;
    let _ = session.logout().await;
    sort_folders(&mut folders);
    Ok(folders)
}

fn folder(name: &async_imap::types::Name) -> Folder {
    let attrs = name.attributes();
    let selectable = !attrs.iter().any(|a| {
        matches!(a, NameAttribute::NoSelect)
            || matches!(a, NameAttribute::Extension(ext) if ext.eq_ignore_ascii_case("\\NonExistent"))
    });
    let special_use = attrs.iter().find_map(|a| match a {
        NameAttribute::All => Some(SpecialUse::All),
        NameAttribute::Archive => Some(SpecialUse::Archive),
        NameAttribute::Drafts => Some(SpecialUse::Drafts),
        NameAttribute::Flagged => Some(SpecialUse::Flagged),
        NameAttribute::Junk => Some(SpecialUse::Junk),
        NameAttribute::Sent => Some(SpecialUse::Sent),
        NameAttribute::Trash => Some(SpecialUse::Trash),
        _ => None,
    });
    Folder {
        name: name.name().to_owned(),
        delimiter: name.delimiter().map(str::to_owned),
        selectable,
        special_use,
    }
}

/// INBOX first, then by name (byte order keeps children after parents).
fn sort_folders(folders: &mut [Folder]) {
    folders.sort_by(|a, b| {
        let a_inbox = a.name.eq_ignore_ascii_case("INBOX");
        let b_inbox = b.name.eq_ignore_ascii_case("INBOX");
        b_inbox.cmp(&a_inbox).then_with(|| a.name.cmp(&b.name))
    });
}

// ── Connection ────────────────────────────────────────────────────────────────

type Client = async_imap::Client<Stream>;

async fn connect(opts: &ConnectOptions) -> Result<Client> {
    let tcp = TcpStream::connect((opts.host.as_str(), opts.port)).await?;
    match opts.security {
        Security::ImplicitTls => {
            let mut client = async_imap::Client::new(Stream::Tls(Box::new(tls(opts, tcp).await?)));
            read_greeting(&mut client).await?;
            Ok(client)
        }
        Security::StartTls => {
            let mut client = async_imap::Client::new(Stream::Plain(tcp));
            read_greeting(&mut client).await?;
            // A server without STARTTLS answers BAD/NO here; never continue in plaintext.
            client.run_command_and_check_ok("STARTTLS", None).await?;
            let Stream::Plain(tcp) = client.into_inner() else {
                unreachable!("plain stream above")
            };
            // No greeting follows the TLS handshake.
            Ok(async_imap::Client::new(Stream::Tls(Box::new(
                tls(opts, tcp).await?,
            ))))
        }
        Security::Insecure => {
            let mut client = async_imap::Client::new(Stream::Plain(tcp));
            read_greeting(&mut client).await?;
            Ok(client)
        }
    }
}

async fn read_greeting(client: &mut Client) -> Result<()> {
    match client.read_response().await? {
        Some(_) => Ok(()),
        None => Err(Error::Imap(async_imap::error::Error::ConnectionLost)),
    }
}

async fn tls(opts: &ConnectOptions, tcp: TcpStream) -> Result<TlsStream<TcpStream>> {
    let config = match &opts.tls_config {
        Some(config) => config.clone(),
        None => managesieve::tls::platform_config().map_err(|e| Error::Tls(e.to_string()))?,
    };
    let name = ServerName::try_from(opts.host.clone())
        .map_err(|_| Error::InvalidServerName(opts.host.clone()))?;
    TlsConnector::from(config)
        .connect(name, tcp)
        .await
        .map_err(|e| Error::Tls(e.to_string()))
}

#[derive(Debug)]
enum Stream {
    Plain(TcpStream),
    Tls(Box<TlsStream<TcpStream>>),
}

impl AsyncRead for Stream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        match self.get_mut() {
            Stream::Plain(s) => Pin::new(s).poll_read(cx, buf),
            Stream::Tls(s) => Pin::new(s).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for Stream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        match self.get_mut() {
            Stream::Plain(s) => Pin::new(s).poll_write(cx, buf),
            Stream::Tls(s) => Pin::new(s).poll_write(cx, buf),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            Stream::Plain(s) => Pin::new(s).poll_flush(cx),
            Stream::Tls(s) => Pin::new(s).poll_flush(cx),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            Stream::Plain(s) => Pin::new(s).poll_shutdown(cx),
            Stream::Tls(s) => Pin::new(s).poll_shutdown(cx),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(name: &str) -> Folder {
        Folder {
            name: name.into(),
            delimiter: Some("/".into()),
            selectable: true,
            special_use: None,
        }
    }

    #[test]
    fn inbox_first_then_by_name() {
        let mut folders = vec![
            f("Archive"),
            f("B&APw-cher"),
            f("INBOX/Sub"),
            f("Inbox"),
            f("Archive/2025"),
        ];
        sort_folders(&mut folders);
        let names: Vec<_> = folders.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "Inbox",
                "Archive",
                "Archive/2025",
                "B&APw-cher",
                "INBOX/Sub"
            ]
        );
    }
}
