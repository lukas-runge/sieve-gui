//! Integration tests against the Dovecot container of the managesieve crate.
//!
//! Skipped unless `MAILSTORE_TEST_ADDR` (IMAP with STARTTLS) is set:
//!
//! ```sh
//! crates/managesieve/tests/dovecot/gen-certs.sh
//! docker compose -f crates/managesieve/tests/dovecot/compose.yaml up -d --build
//! MAILSTORE_TEST_ADDR=localhost:14143 MAILSTORE_TEST_TLS_ADDR=localhost:14993 \
//!     cargo test -p mailstore --test dovecot
//! ```
//!
//! Any user name works with password `secret`, so each test uses its own
//! user and therefore its own, initially empty mail store.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use mailstore::{ConnectOptions, Error, Folder, Security, SpecialUse, list_folders};
use managesieve::rustls::ClientConfig;
use managesieve::tls;

const PASSWORD: &str = "secret";

fn split_addr(addr: &str) -> (String, u16) {
    let (host, port) = addr.rsplit_once(':').expect("address must be host:port");
    (host.to_owned(), port.parse().expect("invalid port"))
}

fn env(var: &str) -> Option<(String, u16)> {
    match std::env::var(var) {
        Ok(addr) => Some(split_addr(&addr)),
        Err(_) => {
            eprintln!("{var} not set; skipping Dovecot integration test");
            None
        }
    }
}

fn test_ca() -> Arc<ClientConfig> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../managesieve/tests/dovecot/certs/ca.crt"
    );
    let pem = std::fs::read(path).expect("cannot read test CA; run gen-certs.sh");
    tls::config_with_roots(tls::certs_from_pem(&pem).unwrap()).unwrap()
}

fn unique_user(test: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("{test}-{nanos}")
}

/// Creates folders over a plaintext connection (the test server allows it).
async fn create_folders(host: &str, port: u16, user: &str, names: &[&str]) {
    let tcp = tokio::net::TcpStream::connect((host, port)).await.unwrap();
    let mut client = async_imap::Client::new(tcp);
    client.read_response().await.unwrap();
    let mut session = client
        .login(user, PASSWORD)
        .await
        .map_err(|(e, _)| e)
        .unwrap();
    for name in names {
        session.create(name).await.unwrap();
    }
    session.logout().await.unwrap();
}

fn names(folders: &[Folder]) -> Vec<&str> {
    folders.iter().map(|f| f.name.as_str()).collect()
}

#[tokio::test]
async fn lists_folders_over_starttls() {
    let Some((host, port)) = env("MAILSTORE_TEST_ADDR") else {
        return;
    };
    let user = unique_user("starttls");
    // "Bücher" in modified UTF-7, as IMAP sends it.
    create_folders(&host, port, &user, &["Lists/Rust", "B&APw-cher"]).await;

    let opts = ConnectOptions::new(&host)
        .port(port)
        .security(Security::StartTls)
        .tls_config(test_ca());
    let folders = list_folders(&opts, &user, PASSWORD).await.unwrap();

    // Drafts, Junk, Sent and Trash are auto-created by the image's namespace.
    assert_eq!(
        names(&folders),
        [
            "INBOX",
            "B&APw-cher",
            "Drafts",
            "Junk",
            "Lists",
            "Lists/Rust",
            "Sent",
            "Trash"
        ]
    );
    assert!(folders.iter().all(|f| f.delimiter.as_deref() == Some("/")));
}

#[tokio::test]
async fn lists_folders_over_implicit_tls() {
    let Some((host, port)) = env("MAILSTORE_TEST_TLS_ADDR") else {
        return;
    };
    let opts = ConnectOptions::new(&host).port(port).tls_config(test_ca());
    let folders = list_folders(&opts, &unique_user("tls"), PASSWORD)
        .await
        .unwrap();
    assert!(names(&folders).contains(&"INBOX"));
}

#[tokio::test]
async fn reports_special_use() {
    let Some((host, port)) = env("MAILSTORE_TEST_ADDR") else {
        return;
    };
    let opts = ConnectOptions::new(&host)
        .port(port)
        .security(Security::StartTls)
        .tls_config(test_ca());
    let folders = list_folders(&opts, &unique_user("special"), PASSWORD)
        .await
        .unwrap();
    // The image's default namespace auto-creates the usual special-use folders.
    let trash = folders
        .iter()
        .find(|f| f.special_use == Some(SpecialUse::Trash));
    assert!(trash.is_some_and(|f| f.selectable));
}

#[tokio::test]
async fn wrong_password_is_auth_error() {
    let Some((host, port)) = env("MAILSTORE_TEST_ADDR") else {
        return;
    };
    let opts = ConnectOptions::new(&host)
        .port(port)
        .security(Security::StartTls)
        .tls_config(test_ca());
    let err = list_folders(&opts, &unique_user("auth"), "wrong")
        .await
        .unwrap_err();
    assert!(matches!(err, Error::Auth(_)), "{err:?}");
}

#[tokio::test]
async fn untrusted_certificate_rejected() {
    let Some((host, port)) = env("MAILSTORE_TEST_TLS_ADDR") else {
        return;
    };
    // Platform trust store: the test CA is not in it.
    let opts = ConnectOptions::new(&host).port(port);
    let err = list_folders(&opts, &unique_user("untrusted"), PASSWORD)
        .await
        .unwrap_err();
    assert!(matches!(err, Error::Tls(_)), "{err:?}");
}
