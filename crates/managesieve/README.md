# managesieve

Async [ManageSieve (RFC 5804)](https://www.rfc-editor.org/rfc/rfc5804) client on `tokio` and `rustls`.

- STARTTLS (default, port 4190), implicit TLS, or explicit `Security::Insecure` plaintext
- Certificates verified against the OS trust store (`rustls-platform-verifier`) by default. A custom `rustls::ClientConfig` can be injected, see `tls::config_with_roots`
- SASL `PLAIN` (with optional authorization identity) and `LOGIN`, picked automatically (PLAIN first) or forced
- All RFC 5804 commands except `UNAUTHENTICATE`; typed capabilities, response codes and best-effort line-numbered diagnostics from `PUTSCRIPT`/`CHECKSCRIPT` errors and warnings

Implemented from RFC 5804, RFC 4616, RFC 4422 and draft-murchison-sasl-login. MIT licensed.

```rust
use managesieve::{Client, ConnectOptions, Credentials, Error};

let mut client = Client::connect(&ConnectOptions::new("mail.example.com")).await?;
client.authenticate(&Credentials::new("user", "password")).await?;
match client.put_script("main", "require \"fileinto\";\nfileinto \"Archive\";\n").await {
    Ok(outcome) => println!("stored; warnings: {:?}", outcome.diagnostics()),
    Err(Error::No(response)) => println!("rejected: {:?}", response.diagnostics()),
    Err(e) => return Err(e),
}
client.set_active("main").await?;
client.logout().await?;
```

## Tests

`cargo test -p managesieve` runs the unit tests and the mock-server tests. They don't need a network connection.

The integration tests in `tests/dovecot.rs` run against Dovecot 2.4 with Pigeonhole in Docker (image `dovecot/dovecot:2.4.5`). They are skipped unless `MANAGESIEVE_TEST_ADDR` is set:

```sh
crates/managesieve/tests/dovecot/gen-certs.sh
docker compose -f crates/managesieve/tests/dovecot/compose.yaml up -d --build
MANAGESIEVE_TEST_ADDR=localhost:14190 MANAGESIEVE_TEST_TLS_ADDR=localhost:14191 \
    cargo test -p managesieve --test dovecot
docker compose -f crates/managesieve/tests/dovecot/compose.yaml down
```

- `14190`: ManageSieve with STARTTLS. Plaintext authentication is also allowed here, for testing `Security::Insecure`
- `14191`: ManageSieve over implicit TLS
- Any user name works with password `secret`, so each test logs in as its own user
- `gen-certs.sh` creates a throwaway test CA and a TLS certificate for `localhost`/`127.0.0.1` in `tests/dovecot/certs/` (git-ignored). The tests trust `certs/ca.crt`
- Optional overrides: `MANAGESIEVE_TEST_CA` (PEM path), `MANAGESIEVE_TEST_PASSWORD`
