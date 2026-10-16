//! Sans-IO encoding of client commands (RFC 5804 §4).

use crate::error::{Error, Result};

/// Quoted strings are limited to 1024 octets (RFC 5804 `quoted`).
const MAX_QUOTED_LEN: usize = 1024;

/// Incrementally builds one command line.
#[derive(Debug)]
pub(crate) struct Command {
    buf: Vec<u8>,
}

impl Command {
    /// A line without a command name, used for SASL responses.
    pub(crate) fn bare() -> Self {
        Self { buf: Vec::new() }
    }

    pub(crate) fn new(name: &str) -> Self {
        Self {
            buf: name.as_bytes().to_vec(),
        }
    }

    fn separator(&mut self) {
        if !self.buf.is_empty() {
            self.buf.push(b' ');
        }
    }

    /// Appends a string, quoted if possible, otherwise as a
    /// non-synchronizing literal.
    pub(crate) fn string(mut self, s: &[u8]) -> Self {
        if s.len() > MAX_QUOTED_LEN || s.iter().any(|b| matches!(b, b'\r' | b'\n' | 0)) {
            return self.literal(s);
        }
        self.separator();
        self.buf.push(b'"');
        for &b in s {
            if matches!(b, b'"' | b'\\') {
                self.buf.push(b'\\');
            }
            self.buf.push(b);
        }
        self.buf.push(b'"');
        self
    }

    /// Appends a non-synchronizing literal (`{n+}` CRLF octets).
    pub(crate) fn literal(mut self, s: &[u8]) -> Self {
        self.separator();
        self.buf
            .extend_from_slice(format!("{{{}+}}\r\n", s.len()).as_bytes());
        self.buf.extend_from_slice(s);
        self
    }

    /// Appends a number.
    pub(crate) fn number(mut self, n: u32) -> Self {
        self.buf.extend_from_slice(format!(" {n}").as_bytes());
        self
    }

    /// Terminates the command with CRLF and returns the bytes to send.
    pub(crate) fn finish(mut self) -> Vec<u8> {
        self.buf.extend_from_slice(b"\r\n");
        self.buf
    }
}

/// Validates a script name per RFC 5804 §1.6. The empty name is only valid
/// for `SETACTIVE` (deactivation), which doesn't use this function.
pub(crate) fn validate_script_name(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(Error::InvalidScriptName("name must not be empty".into()));
    }
    if let Some(c) = name
        .chars()
        .find(|&c| matches!(c, '\u{0}'..='\u{1f}' | '\u{7f}'..='\u{9f}' | '\u{2028}' | '\u{2029}'))
    {
        return Err(Error::InvalidScriptName(format!(
            "forbidden character U+{:04X}",
            u32::from(c)
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_commands() {
        assert_eq!(Command::new("CAPABILITY").finish(), b"CAPABILITY\r\n");
        assert_eq!(
            Command::new("GETSCRIPT").string(b"my \"x\" \\ y").finish(),
            b"GETSCRIPT \"my \\\"x\\\" \\\\ y\"\r\n"
        );
        assert_eq!(
            Command::new("HAVESPACE").string(b"s").number(1234).finish(),
            b"HAVESPACE \"s\" 1234\r\n"
        );
        assert_eq!(
            Command::new("SETACTIVE").string(b"").finish(),
            b"SETACTIVE \"\"\r\n"
        );
    }

    #[test]
    fn script_literal_counts_octets() {
        let script = "# Grüße\r\nkeep;\r\n";
        assert_eq!(script.len(), 18);
        assert_eq!(
            Command::new("PUTSCRIPT")
                .string("n".as_bytes())
                .literal(script.as_bytes())
                .finish(),
            "PUTSCRIPT \"n\" {18+}\r\n# Grüße\r\nkeep;\r\n\r\n".as_bytes()
        );
        assert_eq!(
            Command::new("CHECKSCRIPT").literal(b"").finish(),
            b"CHECKSCRIPT {0+}\r\n\r\n"
        );
    }

    #[test]
    fn strings_fall_back_to_literal() {
        assert_eq!(
            Command::new("X").string(b"a\r\nb").finish(),
            b"X {4+}\r\na\r\nb\r\n"
        );
        let long = vec![b'a'; 1025];
        let encoded = Command::new("X").string(&long).finish();
        assert!(encoded.starts_with(b"X {1025+}\r\n"));
        let max = vec![b'a'; 1024];
        assert!(
            Command::new("X")
                .string(&max)
                .finish()
                .starts_with(b"X \"a")
        );
    }

    #[test]
    fn bare_lines() {
        assert_eq!(
            Command::bare().string(b"dXNlcg==").finish(),
            b"\"dXNlcg==\"\r\n"
        );
        assert_eq!(
            Command::bare().string(b"a\nb").finish(),
            b"{3+}\r\na\nb\r\n"
        );
    }

    #[test]
    fn script_names() {
        assert!(validate_script_name("vacation").is_ok());
        assert!(validate_script_name("Grüße – ✓").is_ok());
        assert!(validate_script_name("").is_err());
        assert!(validate_script_name("a\nb").is_err());
        assert!(validate_script_name("a\u{7f}").is_err());
        assert!(validate_script_name("a\u{85}").is_err());
        assert!(validate_script_name("a\u{2028}").is_err());
    }
}
