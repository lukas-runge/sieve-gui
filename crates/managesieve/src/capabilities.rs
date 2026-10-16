use serde::{Deserialize, Serialize};

use crate::error::ParseError;
use crate::parser::Token;

/// Server capabilities (RFC 5804 §1.7).
///
/// Capability names are case-insensitive; values are kept as sent. Entries
/// this crate doesn't know are preserved in [`Capabilities::other`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    /// `IMPLEMENTATION`: server implementation name and version.
    pub implementation: Option<String>,
    /// `SASL`: offered SASL mechanisms, upper-cased. Empty before STARTTLS on
    /// servers that require TLS.
    pub sasl: Vec<String>,
    /// `SIEVE`: supported Sieve extensions (as used in `require`).
    pub sieve: Vec<String>,
    /// `STARTTLS` is offered.
    pub starttls: bool,
    /// `MAXREDIRECTS`: limit of `redirect` actions per script execution.
    pub max_redirects: Option<u32>,
    /// `NOTIFY`: supported notification URI schemes (e.g. `mailto`).
    pub notify: Vec<String>,
    /// `LANGUAGE`: language of human-readable messages.
    pub language: Option<String>,
    /// `OWNER`: the authorization identity (after authentication only).
    pub owner: Option<String>,
    /// `VERSION`: protocol version, `"1.0"` for RFC 5804 servers. Absence
    /// means `RENAMESCRIPT`, `CHECKSCRIPT` and `NOOP` are not supported.
    pub version: Option<String>,
    /// `UNAUTHENTICATE` extension is offered.
    pub unauthenticate: bool,
    /// Capabilities not interpreted by this crate, in server order.
    pub other: Vec<UnknownCapability>,
}

/// A capability not interpreted by [`Capabilities`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnknownCapability {
    /// Capability name as sent by the server.
    pub name: String,
    /// Optional value.
    pub value: Option<String>,
}

impl Capabilities {
    /// Builds capabilities from the data lines of a capability response.
    pub(crate) fn from_lines(lines: &[Vec<Token>]) -> Result<Self, ParseError> {
        let mut caps = Capabilities::default();
        for line in lines {
            // Names and values are strings per RFC 5804; atoms are
            // tolerated for robustness, lists are not.
            let malformed = || ParseError::UnexpectedResponse("malformed capability line".into());
            let mut texts = line.iter().map(|t| match t {
                Token::List(_) => None,
                other => other.to_text_lossy(),
            });
            let name = texts.next().flatten().ok_or_else(malformed)?;
            let value = match texts.next() {
                None => None,
                Some(Some(value)) => Some(value),
                Some(None) => return Err(malformed()),
            };
            caps.insert(name, value);
        }
        Ok(caps)
    }

    fn insert(&mut self, name: String, value: Option<String>) {
        fn words(v: &Option<String>) -> Vec<String> {
            v.as_deref()
                .unwrap_or("")
                .split_ascii_whitespace()
                .map(str::to_owned)
                .collect()
        }
        match name.to_ascii_uppercase().as_str() {
            "IMPLEMENTATION" => self.implementation = value,
            "SASL" => {
                self.sasl = words(&value)
                    .into_iter()
                    .map(|m| m.to_ascii_uppercase())
                    .collect();
            }
            "SIEVE" => self.sieve = words(&value),
            "STARTTLS" => self.starttls = true,
            "NOTIFY" => self.notify = words(&value),
            "LANGUAGE" => self.language = value,
            "OWNER" => self.owner = value,
            "VERSION" => self.version = value,
            "UNAUTHENTICATE" => self.unauthenticate = true,
            "MAXREDIRECTS" => match value.as_deref().map(|v| v.trim().parse::<u32>()) {
                Some(Ok(n)) => self.max_redirects = Some(n),
                _ => self.other.push(UnknownCapability { name, value }),
            },
            _ => self.other.push(UnknownCapability { name, value }),
        }
    }

    /// `true` if the server supports the Sieve extension (case-sensitive,
    /// as `require` comparisons are).
    pub fn has_extension(&self, extension: &str) -> bool {
        self.sieve.iter().any(|e| e == extension)
    }

    /// `true` if the server offers the SASL mechanism (case-insensitive).
    pub fn has_sasl(&self, mechanism: &str) -> bool {
        self.sasl.iter().any(|m| m.eq_ignore_ascii_case(mechanism))
    }

    /// `true` if the server implements RFC 5804 (advertises `VERSION`), and
    /// therefore `RENAMESCRIPT`, `CHECKSCRIPT` and `NOOP`.
    pub fn is_rfc5804(&self) -> bool {
        self.version.is_some()
    }

    /// Looks up an unknown capability by name (case-insensitive).
    pub fn get_other(&self, name: &str) -> Option<&UnknownCapability> {
        self.other
            .iter()
            .find(|c| c.name.eq_ignore_ascii_case(name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{Limits, Parsed, parse_line};

    fn lines(input: &[u8]) -> Vec<Vec<Token>> {
        let mut out = Vec::new();
        let mut rest = input;
        while !rest.is_empty() {
            match parse_line(rest, &Limits::default()).unwrap() {
                Parsed::Complete { tokens, consumed } => {
                    out.push(tokens);
                    rest = &rest[consumed..];
                }
                Parsed::Incomplete { .. } => panic!("incomplete"),
            }
        }
        out
    }

    #[test]
    fn rfc_example() {
        let caps = Capabilities::from_lines(&lines(
            b"\"IMPlemENTATION\" \"Example1 ManageSieved v001\"\r\n\
              \"SASl\" \"DIGEST-MD5 GSSAPI\"\r\n\
              \"SIeVE\" \"fileinto vacation\"\r\n\
              \"StaRTTLS\"\r\n\
              \"NOTIFY\" \"xmpp mailto\"\r\n\
              \"MAXREdIRECTS\" \"5\"\r\n\
              \"VERSION\" \"1.0\"\r\n\
              \"OWNER\" \"alexey@example.com\"\r\n\
              \"LANGUAGE\" \"de\"\r\n\
              \"X-FUTURE\" \"yes\"\r\n\
              {6}\r\nX-FLAG\r\n",
        ))
        .unwrap();
        assert_eq!(
            caps.implementation.as_deref(),
            Some("Example1 ManageSieved v001")
        );
        assert_eq!(caps.sasl, ["DIGEST-MD5", "GSSAPI"]);
        assert_eq!(caps.sieve, ["fileinto", "vacation"]);
        assert!(caps.starttls);
        assert_eq!(caps.notify, ["xmpp", "mailto"]);
        assert_eq!(caps.max_redirects, Some(5));
        assert_eq!(caps.version.as_deref(), Some("1.0"));
        assert_eq!(caps.owner.as_deref(), Some("alexey@example.com"));
        assert_eq!(caps.language.as_deref(), Some("de"));
        assert!(caps.has_extension("vacation"));
        assert!(caps.has_sasl("gssapi"));
        assert!(caps.is_rfc5804());
        assert_eq!(
            caps.other,
            vec![
                UnknownCapability {
                    name: "X-FUTURE".into(),
                    value: Some("yes".into())
                },
                UnknownCapability {
                    name: "X-FLAG".into(),
                    value: None
                },
            ]
        );
        assert!(caps.get_other("x-future").is_some());
    }

    #[test]
    fn empty_sasl_and_bad_maxredirects() {
        let caps = Capabilities::from_lines(&lines(
            b"\"SASL\" \"\"\r\n\"MAXREDIRECTS\" \"many\"\r\n\"SIEVE\" \"\"\r\n",
        ))
        .unwrap();
        assert!(caps.sasl.is_empty());
        assert!(caps.sieve.is_empty());
        assert_eq!(caps.max_redirects, None);
        assert_eq!(caps.other[0].name, "MAXREDIRECTS");
    }

    #[test]
    fn malformed_line_rejected() {
        assert!(Capabilities::from_lines(&[vec![Token::List(vec![])]]).is_err());
        assert!(
            Capabilities::from_lines(&[vec![Token::String(b"X".to_vec()), Token::List(vec![])]])
                .is_err()
        );
        assert!(Capabilities::from_lines(&[vec![]]).is_err());
        // Atoms are tolerated.
        let caps = Capabilities::from_lines(&[vec![Token::Atom("STARTTLS".into())]]).unwrap();
        assert!(caps.starttls);
    }

    #[test]
    fn serde_round_trip() {
        let caps = Capabilities {
            implementation: Some("x".into()),
            sasl: vec!["PLAIN".into()],
            max_redirects: Some(4),
            ..Default::default()
        };
        let json = serde_json::to_value(&caps).unwrap();
        assert_eq!(json["maxRedirects"], 4);
        let back: Capabilities = serde_json::from_value(json).unwrap();
        assert_eq!(back, caps);
    }
}
