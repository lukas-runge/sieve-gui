//! Best-effort extraction of line-numbered diagnostics from the
//! human-readable text of `PUTSCRIPT`/`CHECKSCRIPT` responses.
//!
//! RFC 5804 only says the text "SHOULD contain a specific error message
//! giving the line number". Common formats handled here:
//!
//! - RFC 5804 examples: `line 2: Syntax error`
//! - Dovecot Pigeonhole: `<tmpname>: line 4: error: unexpected '}'.`
//!   and summary lines like `<tmpname>: error: validation failed.`
//!
//! Lines that don't match are still returned (without a line number) so no
//! information is lost.

use serde::{Deserialize, Serialize};

/// Severity of a [`Diagnostic`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Severity {
    /// The script was rejected.
    Error,
    /// The script was accepted but may not do what was intended.
    Warning,
}

/// One message from a server response, optionally tied to a script line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    /// 1-based script line, if the server reported one.
    pub line: Option<u32>,
    /// Severity, from an explicit `error:`/`warning:` marker or the
    /// response status.
    pub severity: Severity,
    /// The message with location prefixes removed.
    pub message: String,
}

/// Splits server text into diagnostics. `default` is used when a message
/// has no explicit `error:`/`warning:` marker; pass [`Severity::Error`] for
/// `NO` responses and [`Severity::Warning`] for `OK (WARNINGS)`.
pub fn parse_diagnostics(text: &str, default: Severity) -> Vec<Diagnostic> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(|l| parse_one(l, default))
        .collect()
}

fn parse_one(text: &str, default: Severity) -> Diagnostic {
    let (line, rest) = match find_line_number(text) {
        Some((n, rest)) => (Some(n), rest),
        None => (None, text),
    };
    let (severity, message) = match find_severity(rest) {
        Some((sev, msg)) => (sev, msg),
        None => (default, rest),
    };
    Diagnostic {
        line,
        severity,
        message: message.trim().to_owned(),
    }
}

/// Positions where a marker may begin: the start or right after `": "`.
fn marker_positions(s: &str) -> impl Iterator<Item = usize> + '_ {
    std::iter::once(0).chain(s.match_indices(": ").map(|(i, _)| i + 2))
}

fn find_line_number(s: &str) -> Option<(u32, &str)> {
    marker_positions(s).find_map(|pos| {
        let after = s.get(pos..)?;
        let after = strip_prefix_ci(after, "line ")?;
        let digits = after.bytes().take_while(u8::is_ascii_digit).count();
        if digits == 0 {
            return None;
        }
        let n: u32 = after[..digits].parse().ok()?;
        let rest = after[digits..].strip_prefix(':')?;
        Some((n, rest.trim_start()))
    })
}

fn find_severity(s: &str) -> Option<(Severity, &str)> {
    marker_positions(s).find_map(|pos| {
        let after = s.get(pos..)?;
        if let Some(msg) = strip_prefix_ci(after, "error:") {
            Some((Severity::Error, msg))
        } else {
            strip_prefix_ci(after, "warning:").map(|msg| (Severity::Warning, msg))
        }
    })
}

fn strip_prefix_ci<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    let head = s.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix)
        .then(|| &s[prefix.len()..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc_style() {
        assert_eq!(
            parse_diagnostics("line 2: Syntax error", Severity::Error),
            vec![Diagnostic {
                line: Some(2),
                severity: Severity::Error,
                message: "Syntax error".into()
            }]
        );
        assert_eq!(
            parse_diagnostics(
                "line 8: server redirect action limit is 2, this redirect might be ignored",
                Severity::Warning
            )[0]
            .line,
            Some(8)
        );
    }

    #[test]
    fn pigeonhole_style() {
        let text = "1792168766.M440484P19.3fc9ddc8aa43.tmp: line 4: error: expected end of command ';' or the beginning of a compound block '{', but found '}'.\r\n\
                    1792168766.M440484P19.3fc9ddc8aa43.tmp: line 8: warning: something odd.\r\n\
                    1792168766.M440484P19.3fc9ddc8aa43.tmp: error: parse failed.\r\n";
        let d = parse_diagnostics(text, Severity::Error);
        assert_eq!(d.len(), 3);
        assert_eq!(d[0].line, Some(4));
        assert_eq!(d[0].severity, Severity::Error);
        assert!(d[0].message.starts_with("expected end of command"));
        assert_eq!(d[1].line, Some(8));
        assert_eq!(d[1].severity, Severity::Warning);
        assert_eq!(d[1].message, "something odd.");
        assert_eq!(d[2].line, None);
        assert_eq!(d[2].message, "parse failed.");
    }

    #[test]
    fn unstructured_text_kept() {
        let d = parse_diagnostics("Something went wrong", Severity::Error);
        assert_eq!(
            d,
            vec![Diagnostic {
                line: None,
                severity: Severity::Error,
                message: "Something went wrong".into()
            }]
        );
        assert!(parse_diagnostics("", Severity::Error).is_empty());
        // Non-ASCII text must not trip up byte slicing.
        let d = parse_diagnostics("Zeile ü: Fehler – line ✓", Severity::Error);
        assert_eq!(d[0].line, None);
    }
}
