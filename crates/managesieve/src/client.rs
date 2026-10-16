//! The async ManageSieve client.

use std::fmt;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use rustls_pki_types::ServerName;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _};
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;
use tokio_rustls::rustls::ClientConfig;

use crate::capabilities::Capabilities;
use crate::command::{Command, validate_script_name};
use crate::diagnostics::{Diagnostic, Severity, parse_diagnostics};
use crate::error::{Error, ParseError, Result};
use crate::parser::{Limits, Parsed, Token, parse_line};
use crate::response::{Line, ResponseCode, ServerResponse, Status, StatusLine};
use crate::sasl::{Credentials, Mechanism, Session};

/// The IANA-assigned ManageSieve port.
pub const DEFAULT_PORT: u16 = 4190;

/// How the connection is secured.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Security {
    /// Connect in plaintext and upgrade with `STARTTLS` (RFC 5804 default).
    /// Fails if the server does not offer `STARTTLS`; never downgrades.
    #[default]
    StartTls,
    /// TLS from the first byte (non-standard, offered by some providers).
    ImplicitTls,
    /// No encryption at all. Credentials are sent in cleartext; only use
    /// this for tests or connections to localhost.
    Insecure,
}

/// Options for [`Client::connect`].
#[derive(Clone)]
pub struct ConnectOptions {
    /// Host name or IP address. Used for TLS server name verification.
    pub host: String,
    /// TCP port, [`DEFAULT_PORT`] by default.
    pub port: u16,
    /// Transport security.
    pub security: Security,
    /// TLS configuration. `None` uses [`crate::tls::platform_config`].
    pub tls_config: Option<Arc<ClientConfig>>,
    /// Timeout for connecting, the TLS handshake, and each read or write.
    /// `None` disables timeouts.
    pub timeout: Option<Duration>,
    /// Maximum size of a literal accepted from the server (e.g. a script).
    pub max_literal_size: usize,
}

impl ConnectOptions {
    /// Options for `host` with STARTTLS on port 4190 and a 60 s timeout.
    pub fn new(host: impl Into<String>) -> Self {
        Self {
            host: host.into(),
            port: DEFAULT_PORT,
            security: Security::StartTls,
            tls_config: None,
            timeout: Some(Duration::from_secs(60)),
            max_literal_size: crate::DEFAULT_MAX_LITERAL_SIZE,
        }
    }

    /// Sets the port.
    pub fn port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    /// Sets the transport security.
    pub fn security(mut self, security: Security) -> Self {
        self.security = security;
        self
    }

    /// Uses a custom TLS configuration (e.g. custom roots for testing).
    pub fn tls_config(mut self, config: Arc<ClientConfig>) -> Self {
        self.tls_config = Some(config);
        self
    }

    /// Sets the I/O timeout (`None` disables it).
    pub fn timeout(mut self, timeout: Option<Duration>) -> Self {
        self.timeout = timeout;
        self
    }

    /// Sets the maximum accepted literal size.
    pub fn max_literal_size(mut self, size: usize) -> Self {
        self.max_literal_size = size;
        self
    }

    fn bare_host(&self) -> &str {
        self.host
            .strip_prefix('[')
            .and_then(|h| h.strip_suffix(']'))
            .unwrap_or(&self.host)
    }
}

impl fmt::Debug for ConnectOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConnectOptions")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("security", &self.security)
            .field("tls_config", &self.tls_config.as_ref().map(|_| "custom"))
            .field("timeout", &self.timeout)
            .field("max_literal_size", &self.max_literal_size)
            .finish()
    }
}

/// A script as listed by `LISTSCRIPTS`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptInfo {
    /// Script name.
    pub name: String,
    /// Whether this is the active script.
    pub active: bool,
}

/// Successful result of `PUTSCRIPT` or `CHECKSCRIPT`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptOutcome {
    /// Warning text if the server answered `OK (WARNINGS)`.
    pub warnings: Option<String>,
}

impl ScriptOutcome {
    /// Best-effort line-numbered warnings.
    pub fn diagnostics(&self) -> Vec<Diagnostic> {
        self.warnings
            .as_deref()
            .map(|w| parse_diagnostics(w, Severity::Warning))
            .unwrap_or_default()
    }
}

trait Io: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Io for T {}

/// A complete server response: data lines plus the completion status.
struct Response {
    data: Vec<Vec<Token>>,
    status: StatusLine,
}

impl Response {
    /// Converts `NO`/`BYE` into errors.
    fn ok(self) -> Result<(Vec<Vec<Token>>, ServerResponse)> {
        match self.status.status {
            Status::Ok => Ok((self.data, self.status.response)),
            Status::No => Err(Error::No(self.status.response)),
            Status::Bye => Err(Error::Bye(self.status.response)),
        }
    }

    /// Like [`Response::ok`] but rejects unexpected data lines.
    fn ok_without_data(self) -> Result<ServerResponse> {
        let (data, response) = self.ok()?;
        if !data.is_empty() {
            return Err(unexpected("unexpected data lines"));
        }
        Ok(response)
    }
}

fn unexpected(msg: &str) -> Error {
    Error::Protocol(ParseError::UnexpectedResponse(msg.into()))
}

async fn with_timeout<T>(
    timeout: Option<Duration>,
    fut: impl Future<Output = std::io::Result<T>>,
) -> Result<T> {
    match timeout {
        Some(t) => match tokio::time::timeout(t, fut).await {
            Ok(r) => Ok(r?),
            Err(_) => Err(Error::Timeout),
        },
        None => Ok(fut.await?),
    }
}

/// The byte stream plus a read buffer and the sans-IO parser.
struct Connection {
    io: Box<dyn Io>,
    buf: Vec<u8>,
    limits: Limits,
    max_response: usize,
    timeout: Option<Duration>,
}

impl Connection {
    fn new(io: Box<dyn Io>, opts: &ConnectOptions) -> Self {
        let limits = Limits {
            max_literal: opts.max_literal_size,
            ..Limits::default()
        };
        Self {
            io,
            buf: Vec::with_capacity(8 * 1024),
            limits,
            max_response: opts
                .max_literal_size
                .saturating_add(limits.max_line)
                .saturating_add(1024 * 1024),
            timeout: opts.timeout,
        }
    }

    async fn send(&mut self, bytes: &[u8]) -> Result<()> {
        let io = &mut self.io;
        with_timeout(self.timeout, async move {
            io.write_all(bytes).await?;
            io.flush().await
        })
        .await
    }

    /// Reads one logical line. Returns the tokens and the bytes consumed.
    async fn read_tokens(&mut self) -> Result<(Vec<Token>, usize)> {
        loop {
            match parse_line(&self.buf, &self.limits)? {
                Parsed::Complete { tokens, consumed } => {
                    self.buf.drain(..consumed);
                    return Ok((tokens, consumed));
                }
                Parsed::Incomplete { needed } => {
                    // When waiting for a literal payload, read until it is
                    // complete instead of re-parsing after every chunk.
                    let target = needed.unwrap_or(0).max(self.buf.len() + 1);
                    while self.buf.len() < target {
                        let want = (target - self.buf.len()).clamp(4 * 1024, 1024 * 1024);
                        self.buf.reserve(want);
                        let io = &mut self.io;
                        let buf = &mut self.buf;
                        let n = with_timeout(self.timeout, io.read_buf(buf)).await?;
                        if n == 0 {
                            return Err(Error::ConnectionClosed);
                        }
                    }
                }
            }
        }
    }

    async fn read_line(&mut self) -> Result<(Line, usize)> {
        loop {
            let (tokens, consumed) = self.read_tokens().await?;
            // Tolerate stray empty lines.
            if tokens.is_empty() {
                continue;
            }
            return Ok((Line::classify(tokens)?, consumed));
        }
    }

    async fn read_response(&mut self) -> Result<Response> {
        let mut data = Vec::new();
        let mut total = 0usize;
        loop {
            let (line, consumed) = self.read_line().await?;
            total = total.saturating_add(consumed);
            if total > self.max_response {
                return Err(ParseError::ResponseTooLarge {
                    limit: self.max_response,
                }
                .into());
            }
            match line {
                Line::Data(tokens) => data.push(tokens),
                Line::Status(status) => return Ok(Response { data, status }),
            }
        }
    }
}

/// An open ManageSieve session.
///
/// Created with [`Client::connect`]; most commands require
/// [`Client::authenticate`] first. After an error for which
/// [`Error::is_fatal`] is `true` the client is unusable and further calls
/// return [`Error::ConnectionUnusable`].
pub struct Client {
    conn: Connection,
    capabilities: Capabilities,
    authenticated: bool,
    tls: bool,
    broken: bool,
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("capabilities", &self.capabilities)
            .field("authenticated", &self.authenticated)
            .field("tls", &self.tls)
            .field("broken", &self.broken)
            .finish_non_exhaustive()
    }
}

impl Client {
    /// Connects to the server, reads the greeting and, depending on
    /// [`ConnectOptions::security`], negotiates TLS.
    pub async fn connect(opts: &ConnectOptions) -> Result<Self> {
        let host = opts.bare_host();
        let tcp = with_timeout(opts.timeout, TcpStream::connect((host, opts.port))).await?;
        tcp.set_nodelay(true)?;
        Self::connect_with_stream(tcp, opts).await
    }

    /// Like [`Client::connect`], but over an already established byte
    /// stream (e.g. through a proxy). `opts.host` is still used for TLS
    /// server name verification; `opts.port` is ignored.
    pub async fn connect_with_stream<S>(stream: S, opts: &ConnectOptions) -> Result<Self>
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let mut io: Box<dyn Io> = Box::new(stream);
        let mut tls = false;
        if opts.security == Security::ImplicitTls {
            io = tls_handshake(io, opts).await?;
            tls = true;
        }
        let mut conn = Connection::new(io, opts);
        let (data, _) = conn.read_response().await?.ok()?;
        let mut capabilities = Capabilities::from_lines(&data)?;

        if opts.security == Security::StartTls {
            if !capabilities.starttls {
                return Err(Error::StartTlsUnavailable);
            }
            conn.send(&Command::new("STARTTLS").finish()).await?;
            conn.read_response().await?.ok_without_data()?;
            // Anything buffered now was sent before the handshake and could
            // have been injected by an attacker.
            if !conn.buf.is_empty() {
                return Err(ParseError::DataBeforeTls.into());
            }
            let io = tls_handshake(conn.io, opts).await?;
            tls = true;
            conn = Connection::new(io, opts);
            // RFC 5804 §2.2: the server re-issues capabilities after TLS.
            let (data, _) = conn.read_response().await?.ok()?;
            capabilities = Capabilities::from_lines(&data)?;
        }

        Ok(Self {
            conn,
            capabilities,
            authenticated: false,
            tls,
            broken: false,
        })
    }

    /// The most recently received capabilities.
    pub fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }

    /// Whether authentication has succeeded.
    pub fn is_authenticated(&self) -> bool {
        self.authenticated
    }

    /// Whether the connection is protected by TLS.
    pub fn is_tls(&self) -> bool {
        self.tls
    }

    /// Runs `f`, marking the client unusable on fatal errors.
    async fn guarded<T>(&mut self, f: impl AsyncFnOnce(&mut Self) -> Result<T>) -> Result<T> {
        if self.broken {
            return Err(Error::ConnectionUnusable);
        }
        let result = f(self).await;
        if let Err(e) = &result
            && e.is_fatal()
        {
            self.broken = true;
        }
        result
    }

    async fn simple(&mut self, command: Vec<u8>) -> Result<ServerResponse> {
        self.guarded(async move |c| {
            c.conn.send(&command).await?;
            c.conn.read_response().await?.ok_without_data()
        })
        .await
    }

    /// `CAPABILITY`: re-reads and stores the server capabilities.
    pub async fn refresh_capabilities(&mut self) -> Result<&Capabilities> {
        let caps = self
            .guarded(async |c| {
                c.conn.send(&Command::new("CAPABILITY").finish()).await?;
                let (data, _) = c.conn.read_response().await?.ok()?;
                Ok(Capabilities::from_lines(&data)?)
            })
            .await?;
        self.capabilities = caps;
        Ok(&self.capabilities)
    }

    /// `AUTHENTICATE` with the preferred mechanism the server offers
    /// (`PLAIN`, then `LOGIN`). Capabilities are re-read afterwards.
    pub async fn authenticate(&mut self, credentials: &Credentials) -> Result<()> {
        if self.authenticated {
            return Err(Error::AlreadyAuthenticated);
        }
        let mechanism = Mechanism::PREFERENCE
            .into_iter()
            .find(|m| self.capabilities.has_sasl(m.name()))
            .ok_or_else(|| Error::NoSupportedMechanism {
                offered: self.capabilities.sasl.clone(),
            })?;
        self.authenticate_with(mechanism, credentials).await
    }

    /// `AUTHENTICATE` with a specific mechanism, even if the server doesn't
    /// advertise it. Capabilities are re-read afterwards.
    pub async fn authenticate_with(
        &mut self,
        mechanism: Mechanism,
        credentials: &Credentials,
    ) -> Result<()> {
        if self.authenticated {
            return Err(Error::AlreadyAuthenticated);
        }
        let mut session = Session::new(mechanism, credentials)?;
        let mut command = Command::new("AUTHENTICATE").string(mechanism.name().as_bytes());
        if let Some(initial) = session.initial_response()? {
            command = command.string(initial.as_bytes());
        }
        let command = command.finish();

        self.guarded(async move |c| {
            c.conn.send(&command).await?;
            loop {
                match c.conn.read_line().await?.0 {
                    Line::Data(tokens) => {
                        let challenge = match tokens.as_slice() {
                            [Token::String(s)] => s,
                            _ => return Err(unexpected("malformed SASL challenge")),
                        };
                        match session.respond(challenge) {
                            Ok(response) => {
                                let line = Command::bare().string(response.as_bytes()).finish();
                                c.conn.send(&line).await?;
                            }
                            Err(e) => {
                                // Cancel the exchange; the server answers NO.
                                c.conn.send(b"\"*\"\r\n").await?;
                                let response = c.conn.read_response().await?;
                                if response.status.status == Status::Bye {
                                    return Err(Error::Bye(response.status.response));
                                }
                                return Err(e);
                            }
                        }
                    }
                    Line::Status(status) => {
                        return Response {
                            data: Vec::new(),
                            status,
                        }
                        .ok()
                        .map(drop);
                    }
                }
            }
        })
        .await?;

        self.authenticated = true;
        // PLAIN and LOGIN never negotiate a SASL security layer, so the
        // server does not re-issue capabilities; request them explicitly to
        // pick up post-login values such as OWNER.
        self.refresh_capabilities().await?;
        Ok(())
    }

    /// `LISTSCRIPTS`.
    pub async fn list_scripts(&mut self) -> Result<Vec<ScriptInfo>> {
        self.guarded(async |c| {
            c.conn.send(&Command::new("LISTSCRIPTS").finish()).await?;
            let (data, _) = c.conn.read_response().await?.ok()?;
            data.iter()
                .map(|line| {
                    let (name, active) = match line.as_slice() {
                        [Token::String(name)] => (name, false),
                        [Token::String(name), Token::Atom(a)]
                            if a.eq_ignore_ascii_case("ACTIVE") =>
                        {
                            (name, true)
                        }
                        _ => return Err(unexpected("malformed LISTSCRIPTS line")),
                    };
                    let name = String::from_utf8(name.clone())
                        .map_err(|_| ParseError::InvalidUtf8("script name"))?;
                    Ok(ScriptInfo { name, active })
                })
                .collect()
        })
        .await
    }

    /// `GETSCRIPT`: downloads a script.
    pub async fn get_script(&mut self, name: &str) -> Result<String> {
        validate_script_name(name)?;
        let command = Command::new("GETSCRIPT").string(name.as_bytes()).finish();
        self.guarded(async move |c| {
            c.conn.send(&command).await?;
            let (data, _) = c.conn.read_response().await?.ok()?;
            match data.as_slice() {
                [line] => match line.as_slice() {
                    [token] => {
                        let bytes = token
                            .as_bytes()
                            .ok_or_else(|| unexpected("GETSCRIPT data is not a string"))?;
                        Ok(String::from_utf8(bytes.to_vec())
                            .map_err(|_| ParseError::InvalidUtf8("script"))?)
                    }
                    _ => Err(unexpected("malformed GETSCRIPT data")),
                },
                _ => Err(unexpected("GETSCRIPT must return exactly one string")),
            }
        })
        .await
    }

    /// `PUTSCRIPT`: uploads (and replaces) a script. On a validation failure
    /// the server's compile errors are in the [`Error::No`] text (see
    /// [`ServerResponse::diagnostics`]).
    pub async fn put_script(&mut self, name: &str, content: &str) -> Result<ScriptOutcome> {
        validate_script_name(name)?;
        check_size(content.len())?;
        let command = Command::new("PUTSCRIPT")
            .string(name.as_bytes())
            .literal(content.as_bytes())
            .finish();
        let response = self.simple(command).await?;
        Ok(outcome(response))
    }

    /// `CHECKSCRIPT`: validates a script without storing it (RFC 5804
    /// servers only). Errors are reported like [`Client::put_script`].
    pub async fn check_script(&mut self, content: &str) -> Result<ScriptOutcome> {
        check_size(content.len())?;
        let command = Command::new("CHECKSCRIPT")
            .literal(content.as_bytes())
            .finish();
        let response = self.simple(command).await?;
        Ok(outcome(response))
    }

    /// `SETACTIVE`: makes a script the active one.
    pub async fn set_active(&mut self, name: &str) -> Result<()> {
        validate_script_name(name)?;
        let command = Command::new("SETACTIVE").string(name.as_bytes()).finish();
        self.simple(command).await.map(drop)
    }

    /// `SETACTIVE ""`: deactivates the active script, if any.
    pub async fn deactivate(&mut self) -> Result<()> {
        let command = Command::new("SETACTIVE").string(b"").finish();
        self.simple(command).await.map(drop)
    }

    /// `DELETESCRIPT`. The active script cannot be deleted
    /// ([`ResponseCode::Active`]).
    pub async fn delete_script(&mut self, name: &str) -> Result<()> {
        validate_script_name(name)?;
        let command = Command::new("DELETESCRIPT")
            .string(name.as_bytes())
            .finish();
        self.simple(command).await.map(drop)
    }

    /// `RENAMESCRIPT` (RFC 5804 servers only).
    pub async fn rename_script(&mut self, old_name: &str, new_name: &str) -> Result<()> {
        validate_script_name(old_name)?;
        validate_script_name(new_name)?;
        let command = Command::new("RENAMESCRIPT")
            .string(old_name.as_bytes())
            .string(new_name.as_bytes())
            .finish();
        self.simple(command).await.map(drop)
    }

    /// `HAVESPACE`: asks whether a script of `size` octets could be stored
    /// as `name`. A refusal is an [`Error::No`], typically with a quota
    /// response code ([`ResponseCode::is_quota`]).
    pub async fn have_space(&mut self, name: &str, size: usize) -> Result<()> {
        validate_script_name(name)?;
        let size = check_size(size)?;
        let command = Command::new("HAVESPACE")
            .string(name.as_bytes())
            .number(size)
            .finish();
        self.simple(command).await.map(drop)
    }

    /// `NOOP` (RFC 5804 servers only), e.g. to keep the session alive.
    pub async fn noop(&mut self) -> Result<()> {
        self.simple(Command::new("NOOP").finish()).await.map(drop)
    }

    /// `NOOP <tag>`: returns the tag echoed in the `TAG` response code.
    pub async fn noop_with_tag(&mut self, tag: &str) -> Result<Option<String>> {
        let command = Command::new("NOOP").string(tag.as_bytes()).finish();
        let response = self.simple(command).await?;
        Ok(match response.code {
            Some(ResponseCode::Tag(t)) => Some(t),
            _ => None,
        })
    }

    /// `LOGOUT` and close the connection.
    pub async fn logout(mut self) -> Result<()> {
        let result = self
            .guarded(async |c| {
                c.conn.send(&Command::new("LOGOUT").finish()).await?;
                let response = c.conn.read_response().await?;
                match response.status.status {
                    // BYE is an acceptable answer to LOGOUT.
                    Status::Ok | Status::Bye => Ok(()),
                    Status::No => Err(Error::No(response.status.response)),
                }
            })
            .await;
        let _ = with_timeout(self.conn.timeout, self.conn.io.shutdown()).await;
        result
    }
}

fn outcome(response: ServerResponse) -> ScriptOutcome {
    ScriptOutcome {
        warnings: if response.code == Some(ResponseCode::Warnings) {
            Some(response.text.unwrap_or_default())
        } else {
            None
        },
    }
}

/// Sizes in ManageSieve are 32-bit unsigned numbers.
fn check_size(size: usize) -> Result<u32> {
    u32::try_from(size).map_err(|_| Error::InvalidArgument("script too large".into()))
}

async fn tls_handshake(io: Box<dyn Io>, opts: &ConnectOptions) -> Result<Box<dyn Io>> {
    let host = opts.bare_host();
    let name = ServerName::try_from(host.to_owned())
        .map_err(|_| Error::InvalidServerName(host.to_owned()))?;
    let config = match &opts.tls_config {
        Some(c) => c.clone(),
        None => crate::tls::platform_config()?,
    };
    let connector = TlsConnector::from(config);
    let stream = match opts.timeout {
        Some(t) => tokio::time::timeout(t, connector.connect(name, io))
            .await
            .map_err(|_| Error::Timeout)?,
        None => connector.connect(name, io).await,
    }
    .map_err(Error::Tls)?;
    Ok(Box::new(stream))
}
