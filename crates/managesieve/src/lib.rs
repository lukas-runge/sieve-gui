//! Async ManageSieve (RFC 5804) client.
//!
//! Implemented from RFC 5804 (ManageSieve), RFC 4616 (SASL `PLAIN`),
//! RFC 4422 (SASL) and draft-murchison-sasl-login (SASL `LOGIN`).
//!
//! ```no_run
//! # async fn demo() -> managesieve::Result<()> {
//! use managesieve::{Client, ConnectOptions, Credentials};
//!
//! let mut client = Client::connect(&ConnectOptions::new("mail.example.com")).await?;
//! client.authenticate(&Credentials::new("user", "password")).await?;
//! for script in client.list_scripts().await? {
//!     println!("{} {}", script.name, if script.active { "(active)" } else { "" });
//! }
//! match client.check_script("require \"fileinto\";\nfileinto \"x\";\n").await {
//!     Ok(outcome) => println!("ok, warnings: {:?}", outcome.warnings),
//!     Err(managesieve::Error::No(response)) => {
//!         for d in response.diagnostics() {
//!             println!("line {:?}: {}", d.line, d.message);
//!         }
//!     }
//!     Err(e) => return Err(e),
//! }
//! client.logout().await?;
//! # Ok(())
//! # }
//! ```
//!
//! Server responses are parsed by a sans-IO tokenizer that never panics on
//! malformed input; literal, line and response sizes are bounded (see
//! [`ConnectOptions::max_literal_size`]).
//!
//! Not implemented: DNS SRV lookup (RFC 5804 §1.8), SASL mechanisms other
//! than `PLAIN`/`LOGIN`, SASLprep of credentials, `UNAUTHENTICATE`.

mod capabilities;
mod client;
mod command;
mod diagnostics;
mod error;
mod parser;
mod response;
mod sasl;
pub mod tls;

pub use capabilities::{Capabilities, UnknownCapability};
pub use client::{Client, ConnectOptions, DEFAULT_PORT, ScriptInfo, ScriptOutcome, Security};
pub use diagnostics::{Diagnostic, Severity, parse_diagnostics};
pub use error::{Error, ParseError, Result};
pub use response::{ExtensionItem, ResponseCode, ServerResponse, Status};
pub use sasl::{Credentials, Mechanism};

/// Re-export of the `rustls` version used for [`ConnectOptions::tls_config`].
pub use tokio_rustls::rustls;

/// Default maximum literal size accepted from the server (16 MiB).
pub const DEFAULT_MAX_LITERAL_SIZE: usize = 16 * 1024 * 1024;

/// Default maximum length of a server line, excluding literals (1 MiB).
pub const DEFAULT_MAX_LINE_LENGTH: usize = 1024 * 1024;
