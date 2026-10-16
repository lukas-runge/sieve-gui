//! Client tests against a scripted in-memory server. Server output is
//! delivered in tiny chunks to exercise partial reads.

use std::time::Duration;

use managesieve::{
    Client, ConnectOptions, Credentials, Error, Mechanism, ParseError, ResponseCode, Security,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt, DuplexStream};

const GREETING: &[u8] = b"\"IMPLEMENTATION\" \"Mock\"\r\n\
\"SASL\" \"PLAIN LOGIN\"\r\n\
\"SIEVE\" \"fileinto vacation\"\r\n\
\"VERSION\" \"1.0\"\r\n\
OK \"ready\"\r\n";

const POST_AUTH_CAPS: &[u8] = b"\"IMPLEMENTATION\" \"Mock\"\r\n\
\"SIEVE\" \"fileinto vacation\"\r\n\
\"OWNER\" \"user\"\r\n\
\"VERSION\" \"1.0\"\r\n\
OK\r\n";

enum Step {
    /// Read exactly these bytes from the client.
    Expect(&'static [u8]),
    /// Send these bytes, a few octets at a time.
    Send(&'static [u8]),
}

use Step::{Expect, Send};

async fn run_server(mut io: DuplexStream, steps: Vec<Step>) {
    for step in steps {
        match step {
            Expect(expected) => {
                let mut got = vec![0; expected.len()];
                io.read_exact(&mut got).await.expect("client closed early");
                assert_eq!(
                    String::from_utf8_lossy(&got),
                    String::from_utf8_lossy(expected)
                );
            }
            Send(bytes) => {
                for chunk in bytes.chunks(3) {
                    io.write_all(chunk).await.unwrap();
                    io.flush().await.unwrap();
                    tokio::task::yield_now().await;
                }
            }
        }
    }
}

fn options() -> ConnectOptions {
    ConnectOptions::new("localhost")
        .security(Security::Insecure)
        .timeout(Some(Duration::from_secs(5)))
}

async fn client_with(steps: Vec<Step>) -> (Result<Client, Error>, tokio::task::JoinHandle<()>) {
    let (client_io, server_io) = tokio::io::duplex(64);
    let server = tokio::spawn(run_server(server_io, steps));
    (
        Client::connect_with_stream(client_io, &options()).await,
        server,
    )
}

#[tokio::test]
async fn full_session() {
    let script = "# Grüße\r\nkeep;\r\n";
    let (client, server) = client_with(vec![
        Send(GREETING),
        Expect(b"AUTHENTICATE \"PLAIN\" \"AHVzZXIAc2VjcmV0\"\r\n"),
        Send(b"OK \"Logged in.\"\r\n"),
        Expect(b"CAPABILITY\r\n"),
        Send(POST_AUTH_CAPS),
        Expect(b"LISTSCRIPTS\r\n"),
        Send(b"\"a\"\r\n{13}\r\nclever\"script ACTIVE\r\n\r\nOK\r\n"),
        Expect(b"GETSCRIPT \"clever\\\"script\"\r\n"),
        Send("{18}\r\n# Grüße\r\nkeep;\r\n\r\nOK \"Getscript completed.\"\r\n".as_bytes()),
        Expect("PUTSCRIPT \"n\" {18+}\r\n# Grüße\r\nkeep;\r\n\r\n".as_bytes()),
        Send(b"OK (WARNINGS) \"line 2: be careful\"\r\n"),
        Expect(b"CHECKSCRIPT {4+}\r\nfoo;\r\n"),
        Send(b"NO {34}\r\nline 1: error: unknown command foo\r\n"),
        Expect(b"HAVESPACE \"n\" 999999\r\n"),
        Send(b"NO (QUOTA/MAXSIZE) \"Quota exceeded\"\r\n"),
        Expect(b"SETACTIVE \"\"\r\n"),
        Send(b"OK\r\n"),
        Expect(b"NOOP \"t1\"\r\n"),
        Send(b"OK (TAG {2}\r\nt1) \"Done\"\r\n"),
        Expect(b"LOGOUT\r\n"),
        Send(b"OK \"Logout completed.\"\r\n"),
    ])
    .await;
    let mut client = client.unwrap();
    assert!(!client.is_tls());
    assert_eq!(client.capabilities().sasl, ["PLAIN", "LOGIN"]);

    client
        .authenticate(&Credentials::new("user", "secret"))
        .await
        .unwrap();
    assert!(client.is_authenticated());
    assert_eq!(client.capabilities().owner.as_deref(), Some("user"));

    let scripts = client.list_scripts().await.unwrap();
    assert_eq!(scripts.len(), 2);
    assert_eq!(scripts[1].name, "clever\"script");
    assert!(scripts[1].active && !scripts[0].active);

    assert_eq!(client.get_script("clever\"script").await.unwrap(), script);

    let outcome = client.put_script("n", script).await.unwrap();
    assert_eq!(outcome.warnings.as_deref(), Some("line 2: be careful"));
    assert_eq!(outcome.diagnostics()[0].line, Some(2));

    match client.check_script("foo;").await {
        Err(Error::No(r)) => {
            let d = r.diagnostics();
            assert_eq!(d.len(), 1);
            assert_eq!(d[0].line, Some(1));
            assert_eq!(d[0].message, "unknown command foo");
        }
        other => panic!("{other:?}"),
    }

    match client.have_space("n", 999_999).await {
        Err(Error::No(r)) => assert_eq!(r.code, Some(ResponseCode::QuotaMaxSize)),
        other => panic!("{other:?}"),
    }

    client.deactivate().await.unwrap();
    assert_eq!(
        client.noop_with_tag("t1").await.unwrap().as_deref(),
        Some("t1")
    );
    client.logout().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn login_mechanism_and_authzid() {
    let (client, server) = client_with(vec![
        Send(GREETING),
        Expect(b"AUTHENTICATE \"LOGIN\"\r\n"),
        Send(b"\"VXNlcm5hbWU6\"\r\n"),
        Expect(b"\"dXNlcg==\"\r\n"),
        Send(b"{12}\r\nUGFzc3dvcmQ6\r\n"),
        Expect(b"\"c2VjcmV0\"\r\n"),
        Send(b"OK\r\n"),
        Expect(b"CAPABILITY\r\n"),
        Send(POST_AUTH_CAPS),
    ])
    .await;
    let mut client = client.unwrap();
    client
        .authenticate_with(Mechanism::Login, &Credentials::new("user", "secret"))
        .await
        .unwrap();
    assert!(matches!(
        client
            .authenticate(&Credentials::new("user", "secret"))
            .await,
        Err(Error::AlreadyAuthenticated)
    ));
    server.await.unwrap();
}

#[tokio::test]
async fn failed_auth_is_recoverable() {
    let (client, server) = client_with(vec![
        Send(GREETING),
        Expect(b"AUTHENTICATE \"PLAIN\" \"YWRtaW4AdXNlcgB3cm9uZw==\"\r\n"),
        Send(b"NO \"Authentication failed.\"\r\n"),
        Expect(b"AUTHENTICATE \"LOGIN\"\r\n"),
        Send(b"\"VXNlcm5hbWU6\"\r\n"),
        Expect(b"\"dXNlcg==\"\r\n"),
        Send(b"\"UGFzc3dvcmQ6\"\r\n"),
        Expect(b"\"d3Jvbmc=\"\r\n"),
        Send(b"\"\"\r\n"),
        // Unexpected third challenge: client cancels.
        Expect(b"\"*\"\r\n"),
        Send(b"NO \"Authentication cancelled.\"\r\n"),
        Expect(b"NOOP\r\n"),
        Send(b"OK\r\n"),
    ])
    .await;
    let mut client = client.unwrap();
    let creds = Credentials::new("user", "wrong").with_authzid("admin");
    match client.authenticate(&creds).await {
        Err(Error::No(r)) => assert_eq!(r.text.as_deref(), Some("Authentication failed.")),
        other => panic!("{other:?}"),
    }
    assert!(matches!(
        client
            .authenticate_with(Mechanism::Login, &Credentials::new("user", "wrong"))
            .await,
        Err(Error::Sasl(_))
    ));
    client.noop().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn no_supported_mechanism() {
    let (client, server) = client_with(vec![Send(
        b"\"IMPLEMENTATION\" \"Mock\"\r\n\"SASL\" \"GSSAPI\"\r\n\"SIEVE\" \"\"\r\nOK\r\n",
    )])
    .await;
    let mut client = client.unwrap();
    match client.authenticate(&Credentials::new("u", "p")).await {
        Err(Error::NoSupportedMechanism { offered }) => assert_eq!(offered, ["GSSAPI"]),
        other => panic!("{other:?}"),
    }
    server.await.unwrap();
}

#[tokio::test]
async fn bye_makes_client_unusable() {
    let (client, server) = client_with(vec![
        Send(GREETING),
        Expect(b"NOOP\r\n"),
        Send(b"BYE (REFERRAL sieve://other.example.com) \"Go away\"\r\n"),
    ])
    .await;
    let mut client = client.unwrap();
    match client.noop().await {
        Err(Error::Bye(r)) => {
            assert_eq!(
                r.code,
                Some(ResponseCode::Referral("sieve://other.example.com".into()))
            );
        }
        other => panic!("{other:?}"),
    }
    assert!(matches!(
        client.noop().await,
        Err(Error::ConnectionUnusable)
    ));
    server.await.unwrap();
}

#[tokio::test]
async fn greeting_bye_is_error() {
    let (client, server) = client_with(vec![Send(b"BYE \"Server shutting down\"\r\n")]).await;
    assert!(matches!(client, Err(Error::Bye(_))));
    server.await.unwrap();
}

#[tokio::test]
async fn starttls_required_but_unavailable() {
    let (client_io, server_io) = tokio::io::duplex(64);
    let server = tokio::spawn(run_server(server_io, vec![Send(GREETING)]));
    let opts = options().security(Security::StartTls);
    let result = Client::connect_with_stream(client_io, &opts).await;
    assert!(matches!(result, Err(Error::StartTlsUnavailable)));
    server.await.unwrap();
}

#[tokio::test]
async fn data_injected_before_tls_is_rejected() {
    let (client_io, server_io) = tokio::io::duplex(1024);
    let server = tokio::spawn(run_server(
        server_io,
        vec![
            Send(b"\"IMPLEMENTATION\" \"Mock\"\r\n\"STARTTLS\"\r\n\"SASL\" \"\"\r\nOK\r\n"),
            Expect(b"STARTTLS\r\n"),
            // Send the OK and injected data in one write so both are buffered.
            Send(b"OK\r\n\"SASL\" \"PLAIN\"\r\nOK\r\n"),
        ],
    ));
    let opts = options().security(Security::StartTls);
    let result = Client::connect_with_stream(client_io, &opts).await;
    // Chunked delivery means the injected bytes may or may not be buffered
    // when the OK is parsed; either we detect them, or the TLS handshake
    // fails on the garbage. Both must be errors.
    match result {
        Err(Error::Protocol(ParseError::DataBeforeTls)) | Err(Error::Tls(_)) => {}
        other => panic!("{other:?}"),
    }
    server.abort();
}

#[tokio::test]
async fn malformed_server_data() {
    let (client, server) = client_with(vec![
        Send(GREETING),
        Expect(b"LISTSCRIPTS\r\n"),
        Send(b"\"a\" BOGUS\r\nOK\r\n"),
    ])
    .await;
    let mut client = client.unwrap();
    assert!(matches!(
        client.list_scripts().await,
        Err(Error::Protocol(_))
    ));
    assert!(matches!(
        client.list_scripts().await,
        Err(Error::ConnectionUnusable)
    ));
    server.await.unwrap();
}

#[tokio::test]
async fn oversized_literal_rejected() {
    let (client_io, server_io) = tokio::io::duplex(64);
    let server = tokio::spawn(run_server(
        server_io,
        vec![
            Send(GREETING),
            Expect(b"GETSCRIPT \"big\"\r\n"),
            Send(b"{1000}\r\n"),
        ],
    ));
    let opts = options().max_literal_size(100);
    let mut client = Client::connect_with_stream(client_io, &opts).await.unwrap();
    assert!(matches!(
        client.get_script("big").await,
        Err(Error::Protocol(ParseError::LiteralTooLarge { limit: 100 }))
    ));
    server.await.unwrap();
}

#[tokio::test]
async fn invalid_script_names_rejected_locally() {
    let (client, server) = client_with(vec![Send(GREETING)]).await;
    let mut client = client.unwrap();
    assert!(matches!(
        client.get_script("").await,
        Err(Error::InvalidScriptName(_))
    ));
    assert!(matches!(
        client.put_script("a\nb", "keep;").await,
        Err(Error::InvalidScriptName(_))
    ));
    server.await.unwrap();
}

#[tokio::test]
async fn timeout_fires() {
    let (client_io, _server_io) = tokio::io::duplex(64);
    let opts = options().timeout(Some(Duration::from_millis(50)));
    let result = Client::connect_with_stream(client_io, &opts).await;
    assert!(matches!(result, Err(Error::Timeout)));
}

#[tokio::test]
async fn connection_closed() {
    let (client, server) = client_with(vec![Send(b"\"IMPLEMENTATION\" \"Mock\"\r\n")]).await;
    assert!(matches!(client, Err(Error::ConnectionClosed)));
    server.await.unwrap();
}
