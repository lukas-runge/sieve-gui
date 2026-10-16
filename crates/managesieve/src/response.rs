//! Interpretation of tokenized server lines: `OK`/`NO`/`BYE` responses,
//! response codes and data lines.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::diagnostics::{Diagnostic, Severity, parse_diagnostics};
use crate::error::ParseError;
use crate::parser::Token;

/// The status of a server completion response.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    /// `OK`: the command succeeded.
    Ok,
    /// `NO`: the command failed.
    No,
    /// `BYE`: the server is closing the connection.
    Bye,
}

/// A machine-readable response code (RFC 5804 §1.3).
///
/// Hierarchical codes the client doesn't know in detail are mapped to their
/// known parent (e.g. `QUOTA/FOO` becomes [`ResponseCode::Quota`]). Codes that
/// are not known at all are preserved in [`ResponseCode::Other`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "camelCase")]
#[non_exhaustive]
pub enum ResponseCode {
    /// `AUTH-TOO-WEAK`: site policy forbids the mechanism for this user.
    AuthTooWeak,
    /// `ENCRYPT-NEEDED`: a strong encryption mechanism is required.
    EncryptNeeded,
    /// `QUOTA`: a quota would be exceeded.
    Quota,
    /// `QUOTA/MAXSCRIPTS`: the maximum number of scripts would be exceeded.
    QuotaMaxScripts,
    /// `QUOTA/MAXSIZE`: the maximum script size would be exceeded.
    QuotaMaxSize,
    /// `REFERRAL <sieveurl>`: use another server.
    Referral(String),
    /// `SASL <string>`: final server SASL data (base64).
    Sasl(String),
    /// `TRANSITION-NEEDED`: authenticate once with PLAIN over TLS first.
    TransitionNeeded,
    /// `TRYLATER`: temporary server failure.
    TryLater,
    /// `ACTIVE`: not allowed on the active script.
    Active,
    /// `NONEXISTENT`: the referenced script does not exist.
    Nonexistent,
    /// `ALREADYEXISTS`: the referenced script name already exists.
    AlreadyExists,
    /// `WARNINGS`: the script is valid but the text contains warnings.
    Warnings,
    /// `TAG <string>`: echo of the tag given to `NOOP`.
    Tag(String),
    /// Any other response code, preserved verbatim.
    Other {
        /// The response code name as sent by the server.
        name: String,
        /// Extension data following the name.
        data: Vec<ExtensionItem>,
    },
}

/// Extension data of an unknown response code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "camelCase")]
pub enum ExtensionItem {
    /// An atom or number.
    Atom(String),
    /// A quoted or literal string.
    String(String),
    /// A parenthesised list.
    List(Vec<ExtensionItem>),
}

impl ExtensionItem {
    fn from_token(token: &Token) -> Self {
        match token {
            Token::Atom(a) => ExtensionItem::Atom(a.clone()),
            Token::String(s) => ExtensionItem::String(String::from_utf8_lossy(s).into_owned()),
            Token::List(items) => {
                ExtensionItem::List(items.iter().map(ExtensionItem::from_token).collect())
            }
        }
    }
}

impl fmt::Display for ExtensionItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExtensionItem::Atom(a) => f.write_str(a),
            ExtensionItem::String(s) => write!(f, "{s:?}"),
            ExtensionItem::List(items) => {
                f.write_str("(")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        f.write_str(" ")?;
                    }
                    write!(f, "{item}")?;
                }
                f.write_str(")")
            }
        }
    }
}

impl ResponseCode {
    /// Parses a response code from the items of a parenthesised list.
    pub(crate) fn from_tokens(items: &[Token]) -> Option<Self> {
        let (first, rest) = items.split_first()?;
        let name = first.as_atom()?;
        let upper = name.to_ascii_uppercase();
        let arg = || rest.first().and_then(Token::to_text_lossy);

        if let Some(code) = Self::known(&upper, arg) {
            return Some(code);
        }
        // Tolerate unknown hierarchical detail of a known code.
        let mut prefix = upper.as_str();
        while let Some((parent, _)) = prefix.rsplit_once('/') {
            if let Some(code) = Self::known(parent, arg) {
                return Some(code);
            }
            prefix = parent;
        }
        Some(ResponseCode::Other {
            name: name.to_owned(),
            data: rest.iter().map(ExtensionItem::from_token).collect(),
        })
    }

    fn known(upper: &str, arg: impl Fn() -> Option<String>) -> Option<Self> {
        Some(match upper {
            "AUTH-TOO-WEAK" => ResponseCode::AuthTooWeak,
            "ENCRYPT-NEEDED" => ResponseCode::EncryptNeeded,
            "QUOTA" => ResponseCode::Quota,
            "QUOTA/MAXSCRIPTS" => ResponseCode::QuotaMaxScripts,
            "QUOTA/MAXSIZE" => ResponseCode::QuotaMaxSize,
            "REFERRAL" => ResponseCode::Referral(arg()?),
            "SASL" => ResponseCode::Sasl(arg()?),
            "TRANSITION-NEEDED" => ResponseCode::TransitionNeeded,
            "TRYLATER" => ResponseCode::TryLater,
            "ACTIVE" => ResponseCode::Active,
            "NONEXISTENT" => ResponseCode::Nonexistent,
            "ALREADYEXISTS" => ResponseCode::AlreadyExists,
            "WARNINGS" => ResponseCode::Warnings,
            "TAG" => ResponseCode::Tag(arg()?),
            _ => return None,
        })
    }

    /// `true` for `QUOTA` and its sub-codes.
    pub fn is_quota(&self) -> bool {
        matches!(
            self,
            ResponseCode::Quota | ResponseCode::QuotaMaxScripts | ResponseCode::QuotaMaxSize
        )
    }
}

impl fmt::Display for ResponseCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResponseCode::AuthTooWeak => f.write_str("AUTH-TOO-WEAK"),
            ResponseCode::EncryptNeeded => f.write_str("ENCRYPT-NEEDED"),
            ResponseCode::Quota => f.write_str("QUOTA"),
            ResponseCode::QuotaMaxScripts => f.write_str("QUOTA/MAXSCRIPTS"),
            ResponseCode::QuotaMaxSize => f.write_str("QUOTA/MAXSIZE"),
            ResponseCode::Referral(url) => write!(f, "REFERRAL {url}"),
            ResponseCode::Sasl(data) => write!(f, "SASL {data:?}"),
            ResponseCode::TransitionNeeded => f.write_str("TRANSITION-NEEDED"),
            ResponseCode::TryLater => f.write_str("TRYLATER"),
            ResponseCode::Active => f.write_str("ACTIVE"),
            ResponseCode::Nonexistent => f.write_str("NONEXISTENT"),
            ResponseCode::AlreadyExists => f.write_str("ALREADYEXISTS"),
            ResponseCode::Warnings => f.write_str("WARNINGS"),
            ResponseCode::Tag(tag) => write!(f, "TAG {tag:?}"),
            ResponseCode::Other { name, data } => {
                f.write_str(name)?;
                for item in data {
                    write!(f, " {item}")?;
                }
                Ok(())
            }
        }
    }
}

/// The response code and human-readable text of an `OK`, `NO` or `BYE`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerResponse {
    /// Optional machine-readable response code.
    pub code: Option<ResponseCode>,
    /// Optional human-readable text. For `PUTSCRIPT`/`CHECKSCRIPT` failures
    /// this carries the server's compile errors (possibly multi-line).
    pub text: Option<String>,
}

impl ServerResponse {
    /// Best-effort line-numbered diagnostics from [`ServerResponse::text`].
    /// Messages default to warnings for `WARNINGS` responses, errors
    /// otherwise.
    pub fn diagnostics(&self) -> Vec<Diagnostic> {
        let default = if self.code == Some(ResponseCode::Warnings) {
            Severity::Warning
        } else {
            Severity::Error
        };
        self.text
            .as_deref()
            .map(|t| parse_diagnostics(t, default))
            .unwrap_or_default()
    }

    pub(crate) fn describe(&self) -> String {
        let mut out = String::new();
        if let Some(code) = &self.code {
            out.push_str(&format!(" ({code})"));
        }
        if let Some(text) = &self.text {
            out.push_str(": ");
            out.push_str(text);
        }
        out
    }
}

/// A completion response line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StatusLine {
    pub status: Status,
    pub response: ServerResponse,
}

/// A classified server line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Line {
    /// Command-specific data, e.g. a capability or a script name.
    Data(Vec<Token>),
    /// `OK`, `NO` or `BYE`.
    Status(StatusLine),
}

impl Line {
    pub(crate) fn classify(tokens: Vec<Token>) -> Result<Line, ParseError> {
        let status = match tokens.first().and_then(Token::as_atom) {
            Some(atom) if atom.eq_ignore_ascii_case("OK") => Status::Ok,
            Some(atom) if atom.eq_ignore_ascii_case("NO") => Status::No,
            Some(atom) if atom.eq_ignore_ascii_case("BYE") => Status::Bye,
            Some(atom) => {
                return Err(ParseError::UnexpectedResponse(format!(
                    "unknown response {atom:?}"
                )));
            }
            None => return Ok(Line::Data(tokens)),
        };

        let mut rest = tokens.iter().skip(1).peekable();
        let code = match rest.peek() {
            Some(Token::List(items)) => {
                rest.next();
                ResponseCode::from_tokens(items)
            }
            _ => None,
        };
        // Some servers send the text as an atom; accept that too. Anything
        // after the text is ignored.
        let text = rest.next().and_then(Token::to_text_lossy);
        Ok(Line::Status(StatusLine {
            status,
            response: ServerResponse { code, text },
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{Limits, Parsed, parse_line};

    fn line(input: &[u8]) -> Line {
        match parse_line(input, &Limits::default()).unwrap() {
            Parsed::Complete { tokens, .. } => Line::classify(tokens).unwrap(),
            Parsed::Incomplete { .. } => panic!("incomplete"),
        }
    }

    fn status(input: &[u8]) -> StatusLine {
        match line(input) {
            Line::Status(s) => s,
            Line::Data(d) => panic!("expected status, got data {d:?}"),
        }
    }

    #[test]
    fn plain_statuses() {
        assert_eq!(status(b"OK\r\n").status, Status::Ok);
        assert_eq!(status(b"Ok\r\n").status, Status::Ok);
        assert_eq!(status(b"no \"x\"\r\n").status, Status::No);
        let bye = status(b"BYE \"Too many invalid commands.\"\r\n");
        assert_eq!(bye.status, Status::Bye);
        assert_eq!(
            bye.response.text.as_deref(),
            Some("Too many invalid commands.")
        );
    }

    #[test]
    fn response_codes() {
        let cases: &[(&[u8], ResponseCode)] = &[
            (b"NO (AUTH-TOO-WEAK)\r\n", ResponseCode::AuthTooWeak),
            (b"NO (encrypt-needed)\r\n", ResponseCode::EncryptNeeded),
            (b"NO (QUOTA)\r\n", ResponseCode::Quota),
            (b"NO (QUOTA/MAXSCRIPTS)\r\n", ResponseCode::QuotaMaxScripts),
            (b"NO (QUOTA/MAXSIZE)\r\n", ResponseCode::QuotaMaxSize),
            (b"NO (QUOTA/FUTURE/THING)\r\n", ResponseCode::Quota),
            (
                b"BYE (REFERRAL sieve://other.example.com)\r\n",
                ResponseCode::Referral("sieve://other.example.com".into()),
            ),
            (
                b"OK (SASL \"cnNwYXV0aD1lYTQwZjYwMzM1YzQyN2I1NTI3Yjg0ZGJhYmNkZmZmZA==\")\r\n",
                ResponseCode::Sasl(
                    "cnNwYXV0aD1lYTQwZjYwMzM1YzQyN2I1NTI3Yjg0ZGJhYmNkZmZmZA==".into(),
                ),
            ),
            (
                b"NO (TRANSITION-NEEDED)\r\n",
                ResponseCode::TransitionNeeded,
            ),
            (b"NO (TRYLATER)\r\n", ResponseCode::TryLater),
            (b"NO (ACTIVE)\r\n", ResponseCode::Active),
            (b"NO (NONEXISTENT)\r\n", ResponseCode::Nonexistent),
            (b"NO (ALREADYEXISTS)\r\n", ResponseCode::AlreadyExists),
            (b"OK (WARNINGS)\r\n", ResponseCode::Warnings),
            (
                b"OK (TAG {16}\r\nSTARTTLS-SYNC-42) \"Done\"\r\n",
                ResponseCode::Tag("STARTTLS-SYNC-42".into()),
            ),
        ];
        for (input, expected) in cases {
            assert_eq!(
                status(input).response.code.as_ref(),
                Some(expected),
                "{}",
                String::from_utf8_lossy(input)
            );
        }
    }

    #[test]
    fn unknown_code_preserved() {
        let s = status(b"NO (X-FOO/BAR 12 \"x\" (a)) \"text\"\r\n");
        assert_eq!(
            s.response.code,
            Some(ResponseCode::Other {
                name: "X-FOO/BAR".into(),
                data: vec![
                    ExtensionItem::Atom("12".into()),
                    ExtensionItem::String("x".into()),
                    ExtensionItem::List(vec![ExtensionItem::Atom("a".into())]),
                ],
            })
        );
        assert_eq!(
            s.response.code.as_ref().map(ToString::to_string).as_deref(),
            Some("X-FOO/BAR 12 \"x\" (a)")
        );
        assert_eq!(s.response.text.as_deref(), Some("text"));
    }

    #[test]
    fn multiline_literal_text() {
        let s = status(b"NO {22}\r\nline 1: a\r\nline 2: b\r\n\r\n");
        assert_eq!(s.response.code, None);
        assert_eq!(
            s.response.text.as_deref(),
            Some("line 1: a\r\nline 2: b\r\n")
        );
    }

    #[test]
    fn data_lines() {
        assert!(matches!(line(b"\"SIEVE\" \"fileinto\"\r\n"), Line::Data(t) if t.len() == 2));
        assert!(matches!(line(b"{3}\r\nfoo ACTIVE\r\n"), Line::Data(t) if t.len() == 2));
    }

    #[test]
    fn unknown_status_atom_is_error() {
        let tokens = vec![Token::Atom("MAYBE".into())];
        assert!(Line::classify(tokens).is_err());
    }

    #[test]
    fn serde_shape() {
        let r = ServerResponse {
            code: Some(ResponseCode::QuotaMaxSize),
            text: Some("too big".into()),
        };
        let json = serde_json::to_string(&r).unwrap();
        assert_eq!(json, r#"{"code":{"type":"quotaMaxSize"},"text":"too big"}"#);
        let back: ServerResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(back, r);
    }
}
