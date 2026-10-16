//! Integration tests against a real Dovecot + Pigeonhole server.
//!
//! Skipped unless `MANAGESIEVE_TEST_ADDR` is set. See the crate README:
//!
//! ```sh
//! docker compose -f crates/managesieve/tests/dovecot/compose.yaml up -d --build
//! MANAGESIEVE_TEST_ADDR=localhost:14190 MANAGESIEVE_TEST_TLS_ADDR=localhost:14191 \
//!     cargo test -p managesieve --test dovecot
//! ```
//!
//! Optional: `MANAGESIEVE_TEST_CA` (PEM file, defaults to the bundled test
//! CA) and `MANAGESIEVE_TEST_PASSWORD` (defaults to `secret`). The test
//! server accepts any user name with that password, so every test uses its
//! own user and therefore its own script storage.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use managesieve::rustls::ClientConfig;
use managesieve::{
    Client, ConnectOptions, Credentials, Error, Mechanism, ResponseCode, Security, Severity, tls,
};

struct Env {
    host: String,
    port: u16,
    tls_addr: Option<(String, u16)>,
    tls: Arc<ClientConfig>,
    password: String,
}

fn split_addr(addr: &str) -> (String, u16) {
    let (host, port) = addr.rsplit_once(':').expect("address must be host:port");
    (host.to_owned(), port.parse().expect("invalid port"))
}

fn env() -> Option<Env> {
    let Ok(addr) = std::env::var("MANAGESIEVE_TEST_ADDR") else {
        eprintln!("MANAGESIEVE_TEST_ADDR not set; skipping Dovecot integration test");
        return None;
    };
    let (host, port) = split_addr(&addr);
    let ca_path = std::env::var("MANAGESIEVE_TEST_CA").unwrap_or_else(|_| {
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/dovecot/certs/ca.crt").to_owned()
    });
    let pem = std::fs::read(&ca_path).expect("cannot read test CA");
    let tls = tls::config_with_roots(tls::certs_from_pem(&pem).unwrap()).unwrap();
    Some(Env {
        host,
        port,
        tls_addr: std::env::var("MANAGESIEVE_TEST_TLS_ADDR")
            .ok()
            .map(|a| split_addr(&a)),
        tls,
        password: std::env::var("MANAGESIEVE_TEST_PASSWORD").unwrap_or_else(|_| "secret".into()),
    })
}

/// A user name unique to this test run, so tests don't share scripts.
fn user(test: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("{test}-{nanos}")
}

impl Env {
    fn options(&self, security: Security) -> ConnectOptions {
        ConnectOptions::new(&self.host)
            .port(self.port)
            .security(security)
            .tls_config(self.tls.clone())
    }

    async fn login(&self, test: &str, mechanism: Option<Mechanism>) -> Client {
        let mut client = Client::connect(&self.options(Security::StartTls))
            .await
            .expect("connect with STARTTLS");
        let creds = Credentials::new(user(test), &self.password);
        match mechanism {
            Some(m) => client.authenticate_with(m, &creds).await,
            None => client.authenticate(&creds).await,
        }
        .expect("authenticate");
        client
    }
}

const SCRIPT: &str = "# Filter für Grüße – ✓ 日本語\r\n\
require [\"fileinto\"];\r\n\
if header :contains \"subject\" \"Grüße\" {\r\n\
  fileinto \"Archiv/Grüße\";\r\n\
}\r\n";

const BROKEN: &str = "require [\"fileinto\"];\r\n\
if header :contains \"subject\" \"x\" {\r\n\
  fileinto \"a\"\r\n\
}\r\n";

#[tokio::test]
async fn starttls_capabilities() {
    let Some(env) = env() else { return };
    let client = Client::connect(&env.options(Security::StartTls))
        .await
        .unwrap();
    assert!(client.is_tls());
    let caps = client.capabilities();
    assert_eq!(caps.implementation.as_deref(), Some("Dovecot Pigeonhole"));
    assert!(caps.is_rfc5804());
    assert!(caps.has_extension("fileinto"));
    assert!(caps.has_sasl("PLAIN") && caps.has_sasl("LOGIN"));
    // RFC 5804 §1.7: STARTTLS is not advertised after STARTTLS.
    assert!(!caps.starttls);
    client.logout().await.unwrap();
}

#[tokio::test]
async fn full_lifecycle_plain() {
    let Some(env) = env() else { return };
    let mut c = env.login("plain", Some(Mechanism::Plain)).await;
    assert!(c.is_authenticated());
    assert_eq!(c.capabilities().max_redirects, Some(4));

    // HAVESPACE: small OK, oversized refused with a quota code.
    c.have_space("main", SCRIPT.len()).await.unwrap();
    match c.have_space("main", 10 * 1024 * 1024).await {
        Err(Error::No(r)) => {
            assert!(r.code.as_ref().is_some_and(ResponseCode::is_quota), "{r:?}");
        }
        other => panic!("expected quota refusal, got {other:?}"),
    }

    // CHECKSCRIPT: valid and broken.
    let outcome = c.check_script(SCRIPT).await.unwrap();
    assert_eq!(outcome.warnings, None);
    match c.check_script(BROKEN).await {
        Err(Error::No(r)) => {
            let text = r.text.clone().unwrap_or_default();
            assert!(text.contains("line 4"), "{text}");
            let diagnostics = r.diagnostics();
            assert!(
                diagnostics
                    .iter()
                    .any(|d| d.line == Some(4) && d.severity == Severity::Error),
                "{diagnostics:?}"
            );
        }
        other => panic!("expected NO, got {other:?}"),
    }

    // PUTSCRIPT: broken is rejected with errors and not stored.
    match c.put_script("broken", BROKEN).await {
        Err(Error::No(r)) => assert!(!r.diagnostics().is_empty()),
        other => panic!("expected NO, got {other:?}"),
    }
    assert!(c.list_scripts().await.unwrap().is_empty());

    // PUTSCRIPT + GETSCRIPT round trip with non-ASCII content and name.
    let name = "Grüße ✓";
    c.put_script(name, SCRIPT).await.unwrap();
    c.put_script("second", "keep;\r\n").await.unwrap();
    assert_eq!(c.get_script(name).await.unwrap(), SCRIPT);

    // Replacing keeps the name.
    c.put_script("second", "discard;\r\n").await.unwrap();
    assert_eq!(c.get_script("second").await.unwrap(), "discard;\r\n");

    let mut scripts = c.list_scripts().await.unwrap();
    scripts.sort_by(|a, b| a.name.cmp(&b.name));
    assert_eq!(
        scripts.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
        [name, "second"]
    );
    assert!(scripts.iter().all(|s| !s.active));

    // SETACTIVE and refusal to delete the active script.
    c.set_active(name).await.unwrap();
    let active: Vec<_> = c
        .list_scripts()
        .await
        .unwrap()
        .into_iter()
        .filter(|s| s.active)
        .collect();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].name, name);
    match c.delete_script(name).await {
        Err(Error::No(r)) => assert_eq!(r.code, Some(ResponseCode::Active)),
        other => panic!("expected NO (ACTIVE), got {other:?}"),
    }
    match c.set_active("missing").await {
        Err(Error::No(r)) => assert_eq!(r.code, Some(ResponseCode::Nonexistent)),
        other => panic!("expected NO (NONEXISTENT), got {other:?}"),
    }

    // RENAMESCRIPT: collisions, missing scripts, active script stays active.
    match c.rename_script(name, "second").await {
        Err(Error::No(r)) => assert_eq!(r.code, Some(ResponseCode::AlreadyExists)),
        other => panic!("expected NO (ALREADYEXISTS), got {other:?}"),
    }
    match c.rename_script("missing", "x").await {
        Err(Error::No(r)) => assert_eq!(r.code, Some(ResponseCode::Nonexistent)),
        other => panic!("expected NO (NONEXISTENT), got {other:?}"),
    }
    c.rename_script(name, "renamed").await.unwrap();
    let scripts = c.list_scripts().await.unwrap();
    assert!(scripts.iter().any(|s| s.name == "renamed" && s.active));
    assert_eq!(c.get_script("renamed").await.unwrap(), SCRIPT);
    match c.get_script(name).await {
        Err(Error::No(r)) => assert_eq!(r.code, Some(ResponseCode::Nonexistent)),
        other => panic!("expected NO (NONEXISTENT), got {other:?}"),
    }

    // Deactivate, then delete everything.
    c.deactivate().await.unwrap();
    c.deactivate().await.unwrap(); // idempotent per RFC 5804 §2.8
    assert!(c.list_scripts().await.unwrap().iter().all(|s| !s.active));
    c.delete_script("renamed").await.unwrap();
    c.delete_script("second").await.unwrap();
    assert!(c.list_scripts().await.unwrap().is_empty());

    c.noop().await.unwrap();
    assert_eq!(
        c.noop_with_tag("sync-42").await.unwrap().as_deref(),
        Some("sync-42")
    );
    c.refresh_capabilities().await.unwrap();
    c.logout().await.unwrap();
}

#[tokio::test]
async fn login_mechanism() {
    let Some(env) = env() else { return };
    let mut c = env.login("login", Some(Mechanism::Login)).await;
    c.put_script("s", "keep;\r\n").await.unwrap();
    assert_eq!(c.list_scripts().await.unwrap().len(), 1);
    c.logout().await.unwrap();
}

#[tokio::test]
async fn automatic_mechanism_and_warnings() {
    let Some(env) = env() else { return };
    let mut c = env.login("auto", None).await;
    // Pigeonhole accepts an over-long vacation period with a warning.
    let script = "require \"vacation\";\r\nvacation :days 1000 \"away\";\r\n";
    for outcome in [
        c.check_script(script).await.unwrap(),
        c.put_script("vacation", script).await.unwrap(),
    ] {
        let warnings = outcome.warnings.clone().expect("WARNINGS response");
        assert!(warnings.contains(":days"), "{warnings}");
        let d = outcome.diagnostics();
        assert_eq!(d.len(), 1, "{d:?}");
        assert_eq!(d[0].line, Some(2));
        assert_eq!(d[0].severity, Severity::Warning);
    }
    // A clean script has no warnings.
    assert_eq!(c.check_script("keep;\r\n").await.unwrap().warnings, None);
    c.logout().await.unwrap();
}

#[tokio::test]
async fn wrong_password_then_retry() {
    let Some(env) = env() else { return };
    let mut c = Client::connect(&env.options(Security::StartTls))
        .await
        .unwrap();
    let user = user("retry");
    match c.authenticate(&Credentials::new(&user, "wrong")).await {
        Err(Error::No(_)) => {}
        other => panic!("expected NO, got {other:?}"),
    }
    assert!(!c.is_authenticated());
    c.authenticate(&Credentials::new(&user, &env.password))
        .await
        .unwrap();
    assert!(c.is_authenticated());
    c.logout().await.unwrap();
}

#[tokio::test]
async fn proxy_authorization_identity() {
    let Some(env) = env() else { return };
    let mut c = Client::connect(&env.options(Security::StartTls))
        .await
        .unwrap();
    // Dovecot rejects authzid != authcid unless master users are configured;
    // what matters is that the request is well-formed and answered with NO.
    let creds = Credentials::new(user("proxy"), &env.password).with_authzid("someone-else");
    match c.authenticate_with(Mechanism::Plain, &creds).await {
        Ok(()) | Err(Error::No(_)) => {}
        Err(e) => panic!("unexpected error {e:?}"),
    }
}

#[tokio::test]
async fn insecure_plaintext() {
    let Some(env) = env() else { return };
    let mut c = Client::connect(&env.options(Security::Insecure))
        .await
        .unwrap();
    assert!(!c.is_tls());
    assert!(c.capabilities().starttls);
    c.authenticate(&Credentials::new(user("insecure"), &env.password))
        .await
        .unwrap();
    assert!(c.list_scripts().await.unwrap().is_empty());
    c.logout().await.unwrap();
}

#[tokio::test]
async fn implicit_tls() {
    let Some(env) = env() else { return };
    let Some((host, port)) = &env.tls_addr else {
        eprintln!("MANAGESIEVE_TEST_TLS_ADDR not set; skipping implicit TLS test");
        return;
    };
    let opts = ConnectOptions::new(host)
        .port(*port)
        .security(Security::ImplicitTls)
        .tls_config(env.tls.clone());
    let mut c = Client::connect(&opts).await.unwrap();
    assert!(c.is_tls());
    c.authenticate(&Credentials::new(user("implicit"), &env.password))
        .await
        .unwrap();
    c.noop().await.unwrap();
    c.logout().await.unwrap();
}

#[tokio::test]
async fn untrusted_certificate_rejected() {
    let Some(env) = env() else { return };
    // The OS trust store does not contain the test CA.
    let opts = ConnectOptions::new(&env.host)
        .port(env.port)
        .security(Security::StartTls);
    match Client::connect(&opts).await {
        Err(Error::Tls(_)) => {}
        other => panic!("expected TLS error, got {other:?}"),
    }
}

#[tokio::test]
async fn large_script_round_trip() {
    let Some(env) = env() else { return };
    let mut c = env.login("large", None).await;
    let mut script = String::new();
    let mut i = 0;
    while script.len() < 200 * 1024 {
        script.push_str(&format!(
            "# Zeile {i}: Grüße aus Köln – ✓\r\nif header :is \"x-n\" \"{i}\" {{ keep; }}\r\n"
        ));
        i += 1;
    }
    c.have_space("large", script.len()).await.unwrap();
    c.put_script("large", &script).await.unwrap();
    assert_eq!(c.get_script("large").await.unwrap(), script);
    c.delete_script("large").await.unwrap();
    c.logout().await.unwrap();
}
