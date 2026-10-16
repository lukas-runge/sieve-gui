//! SASL mechanisms: `PLAIN` (RFC 4616) and `LOGIN`
//! (draft-murchison-sasl-login).

use std::fmt;
use std::str::FromStr;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// A supported SASL mechanism.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Mechanism {
    /// `PLAIN` (RFC 4616), sent as an initial response; supports an
    /// authorization identity.
    Plain,
    /// `LOGIN`: username and password in response to two server challenges.
    Login,
}

impl Mechanism {
    /// The IANA mechanism name.
    pub fn name(self) -> &'static str {
        match self {
            Mechanism::Plain => "PLAIN",
            Mechanism::Login => "LOGIN",
        }
    }

    /// Mechanisms in order of preference for automatic selection.
    pub const PREFERENCE: [Mechanism; 2] = [Mechanism::Plain, Mechanism::Login];
}

impl fmt::Display for Mechanism {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl FromStr for Mechanism {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_ascii_uppercase().as_str() {
            "PLAIN" => Ok(Mechanism::Plain),
            "LOGIN" => Ok(Mechanism::Login),
            other => Err(Error::Sasl(format!("unsupported mechanism {other}"))),
        }
    }
}

/// Credentials for password-based authentication.
#[derive(Clone)]
pub struct Credentials {
    /// Authentication identity (login name).
    pub username: String,
    /// Password.
    pub password: String,
    /// Optional authorization identity ("log in as"), only used by `PLAIN`.
    pub authzid: Option<String>,
}

impl Credentials {
    /// Credentials without an authorization identity.
    pub fn new(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self {
            username: username.into(),
            password: password.into(),
            authzid: None,
        }
    }

    /// Sets the authorization identity (proxy authentication, `PLAIN` only).
    pub fn with_authzid(mut self, authzid: impl Into<String>) -> Self {
        self.authzid = Some(authzid.into());
        self
    }
}

impl fmt::Debug for Credentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Credentials")
            .field("username", &self.username)
            .field("password", &"<redacted>")
            .field("authzid", &self.authzid)
            .finish()
    }
}

/// Encodes the `PLAIN` message: `[authzid] NUL authcid NUL passwd`.
pub(crate) fn plain_message(creds: &Credentials) -> Result<Vec<u8>> {
    let authzid = creds.authzid.as_deref().unwrap_or("");
    for (field, value) in [
        ("authorization identity", authzid),
        ("username", creds.username.as_str()),
        ("password", creds.password.as_str()),
    ] {
        if value.contains('\0') {
            return Err(Error::Sasl(format!("{field} must not contain NUL")));
        }
    }
    if creds.username.is_empty() || creds.password.is_empty() {
        return Err(Error::Sasl(
            "username and password must not be empty".into(),
        ));
    }
    let mut out =
        Vec::with_capacity(authzid.len() + creds.username.len() + creds.password.len() + 2);
    out.extend_from_slice(authzid.as_bytes());
    out.push(0);
    out.extend_from_slice(creds.username.as_bytes());
    out.push(0);
    out.extend_from_slice(creds.password.as_bytes());
    Ok(out)
}

/// Client side of one SASL exchange.
pub(crate) struct Session<'a> {
    mechanism: Mechanism,
    creds: &'a Credentials,
    step: u8,
}

impl<'a> Session<'a> {
    pub(crate) fn new(mechanism: Mechanism, creds: &'a Credentials) -> Result<Self> {
        if mechanism == Mechanism::Login && creds.authzid.is_some() {
            return Err(Error::Sasl(
                "LOGIN does not support an authorization identity".into(),
            ));
        }
        Ok(Self {
            mechanism,
            creds,
            step: 0,
        })
    }

    /// The initial response to send with `AUTHENTICATE`, base64-encoded.
    pub(crate) fn initial_response(&mut self) -> Result<Option<String>> {
        match self.mechanism {
            Mechanism::Plain => {
                self.step = 1;
                Ok(Some(BASE64.encode(plain_message(self.creds)?)))
            }
            Mechanism::Login => Ok(None),
        }
    }

    /// Answers a base64-encoded server challenge with a base64-encoded
    /// response.
    pub(crate) fn respond(&mut self, challenge_b64: &[u8]) -> Result<String> {
        // The challenge content isn't needed by PLAIN or LOGIN, but it must
        // at least be valid base64.
        let challenge: Vec<u8> = challenge_b64
            .iter()
            .copied()
            .filter(|b| !b.is_ascii_whitespace())
            .collect();
        BASE64
            .decode(&challenge)
            .map_err(|e| Error::Sasl(format!("invalid base64 challenge: {e}")))?;

        let response = match (self.mechanism, self.step) {
            (Mechanism::Plain, 0) => plain_message(self.creds)?,
            (Mechanism::Login, 0) => {
                if self.creds.username.is_empty() {
                    return Err(Error::Sasl("username must not be empty".into()));
                }
                self.creds.username.as_bytes().to_vec()
            }
            (Mechanism::Login, 1) => self.creds.password.as_bytes().to_vec(),
            _ => {
                return Err(Error::Sasl(format!(
                    "unexpected additional {} challenge",
                    self.mechanism
                )));
            }
        };
        self.step += 1;
        Ok(BASE64.encode(response))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_rfc4616_examples() {
        // RFC 4616 §4 examples.
        let creds = Credentials::new("tim", "tanstaaftanstaaf");
        assert_eq!(plain_message(&creds).unwrap(), b"\0tim\0tanstaaftanstaaf");
        let creds = Credentials::new("Kurt", "xipj3plmq").with_authzid("Ursel");
        assert_eq!(plain_message(&creds).unwrap(), b"Ursel\0Kurt\0xipj3plmq");
    }

    #[test]
    fn plain_initial_response_base64() {
        let creds = Credentials::new("tim", "tanstaaftanstaaf");
        let mut s = Session::new(Mechanism::Plain, &creds).unwrap();
        assert_eq!(
            s.initial_response().unwrap().as_deref(),
            Some("AHRpbQB0YW5zdGFhZnRhbnN0YWFm")
        );
        // A further challenge after the initial response is a protocol error.
        assert!(s.respond(b"").is_err());
    }

    #[test]
    fn plain_utf8() {
        let creds = Credentials::new("jürgen", "pässwörd");
        assert_eq!(
            plain_message(&creds).unwrap(),
            "\0jürgen\0pässwörd".as_bytes()
        );
    }

    #[test]
    fn plain_rejects_nul_and_empty() {
        assert!(plain_message(&Credentials::new("a\0b", "x")).is_err());
        assert!(plain_message(&Credentials::new("a", "x\0")).is_err());
        assert!(plain_message(&Credentials::new("a", "x").with_authzid("\0")).is_err());
        assert!(plain_message(&Credentials::new("", "x")).is_err());
        assert!(plain_message(&Credentials::new("a", "")).is_err());
    }

    #[test]
    fn login_exchange() {
        let creds = Credentials::new("user", "secret");
        let mut s = Session::new(Mechanism::Login, &creds).unwrap();
        assert_eq!(s.initial_response().unwrap(), None);
        // "Username:" / "Password:"
        assert_eq!(s.respond(b"VXNlcm5hbWU6").unwrap(), "dXNlcg==");
        assert_eq!(s.respond(b"UGFzc3dvcmQ6").unwrap(), "c2VjcmV0");
        assert!(s.respond(b"").is_err());
    }

    #[test]
    fn login_rejects_authzid_and_bad_base64() {
        let creds = Credentials::new("u", "p").with_authzid("other");
        assert!(Session::new(Mechanism::Login, &creds).is_err());
        let creds = Credentials::new("u", "p");
        let mut s = Session::new(Mechanism::Login, &creds).unwrap();
        assert!(s.respond(b"!!!").is_err());
    }

    #[test]
    fn mechanism_names() {
        assert_eq!("plain".parse::<Mechanism>().unwrap(), Mechanism::Plain);
        assert_eq!("LOGIN".parse::<Mechanism>().unwrap(), Mechanism::Login);
        assert!("CRAM-MD5".parse::<Mechanism>().is_err());
    }

    #[test]
    fn debug_redacts_password() {
        let s = format!("{:?}", Credentials::new("u", "hunter2"));
        assert!(!s.contains("hunter2"));
    }
}
