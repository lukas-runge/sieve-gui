//! Sans-IO tokenizer for ManageSieve server lines (RFC 5804 §4).
//!
//! The server sends lines made of atoms, numbers, strings (quoted or
//! literal) and parenthesised lists, terminated by CRLF. A literal
//! (`{n}` / `{n+}` followed by CRLF and `n` octets) may contain line breaks,
//! so a "line" can span several physical lines.
//!
//! [`parse_line`] inspects a byte buffer and either returns one complete
//! logical line plus the number of bytes it consumed, or reports that more
//! data is needed. It never performs I/O and never panics on malformed input.

use crate::error::ParseError;

/// Size limits applied while parsing untrusted server data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Limits {
    /// Maximum size of a single literal in octets.
    pub max_literal: usize,
    /// Maximum size of a logical line, excluding literal payloads.
    pub max_line: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_literal: crate::DEFAULT_MAX_LITERAL_SIZE,
            max_line: crate::DEFAULT_MAX_LINE_LENGTH,
        }
    }
}

/// Maximum nesting depth of parenthesised lists.
const MAX_LIST_DEPTH: usize = 8;

/// A syntactic element of a server line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Token {
    /// An atom or number, e.g. `OK`, `ACTIVE`, `QUOTA/MAXSIZE`, `42`.
    Atom(String),
    /// A quoted string or literal, unescaped, as raw octets.
    String(Vec<u8>),
    /// A parenthesised list, e.g. a response code.
    List(Vec<Token>),
}

impl Token {
    pub(crate) fn as_atom(&self) -> Option<&str> {
        match self {
            Token::Atom(a) => Some(a),
            _ => None,
        }
    }

    pub(crate) fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Token::String(s) => Some(s),
            _ => None,
        }
    }

    /// Strings and atoms as text (lossy for invalid UTF-8).
    pub(crate) fn to_text_lossy(&self) -> Option<String> {
        match self {
            Token::Atom(a) => Some(a.clone()),
            Token::String(s) => Some(String::from_utf8_lossy(s).into_owned()),
            Token::List(_) => None,
        }
    }
}

/// Result of a parse attempt.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Parsed {
    /// A complete line; `consumed` bytes (including the line terminator)
    /// should be removed from the buffer.
    Complete { tokens: Vec<Token>, consumed: usize },
    /// More data is required. If known, `needed` is the minimum total buffer
    /// length before another attempt can make progress.
    Incomplete { needed: Option<usize> },
}

enum Step<T> {
    Done(T),
    NeedMore(Option<usize>),
}

/// Attempts to parse one logical line from the start of `buf`.
pub(crate) fn parse_line(buf: &[u8], limits: &Limits) -> Result<Parsed, ParseError> {
    let mut p = Cursor {
        buf,
        pos: 0,
        literal_bytes: 0,
        limits,
    };
    match p.line()? {
        Step::Done(tokens) => Ok(Parsed::Complete {
            tokens,
            consumed: p.pos,
        }),
        Step::NeedMore(needed) => {
            if buf.len().saturating_sub(p.literal_bytes) > limits.max_line {
                return Err(ParseError::LineTooLong {
                    limit: limits.max_line,
                });
            }
            Ok(Parsed::Incomplete { needed })
        }
    }
}

struct Cursor<'a> {
    buf: &'a [u8],
    pos: usize,
    /// Literal payload bytes seen so far (excluded from the line limit).
    literal_bytes: usize,
    limits: &'a Limits,
}

macro_rules! need {
    ($e:expr) => {
        match $e {
            Step::Done(v) => v,
            Step::NeedMore(n) => return Ok(Step::NeedMore(n)),
        }
    };
}

impl Cursor<'_> {
    fn peek(&self) -> Option<u8> {
        self.buf.get(self.pos).copied()
    }

    fn check_line_len(&self) -> Result<(), ParseError> {
        if self.pos.saturating_sub(self.literal_bytes) > self.limits.max_line {
            Err(ParseError::LineTooLong {
                limit: self.limits.max_line,
            })
        } else {
            Ok(())
        }
    }

    fn skip_spaces(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t')) {
            self.pos += 1;
        }
    }

    /// Consumes a line terminator (CRLF, or a bare LF for leniency).
    /// Returns `Done(true)` if one was consumed, `Done(false)` if the next
    /// byte is something else.
    fn eol(&mut self) -> Step<bool> {
        match self.peek() {
            None => Step::NeedMore(None),
            Some(b'\n') => {
                self.pos += 1;
                Step::Done(true)
            }
            Some(b'\r') => match self.buf.get(self.pos + 1) {
                None => Step::NeedMore(None),
                Some(b'\n') => {
                    self.pos += 2;
                    Step::Done(true)
                }
                Some(_) => Step::Done(false),
            },
            Some(_) => Step::Done(false),
        }
    }

    fn line(&mut self) -> Result<Step<Vec<Token>>, ParseError> {
        let mut tokens = Vec::new();
        loop {
            self.skip_spaces();
            if need!(self.eol()) {
                return Ok(Step::Done(tokens));
            }
            let token = need!(self.token(0)?);
            tokens.push(token);
            self.check_line_len()?;
        }
    }

    fn token(&mut self, depth: usize) -> Result<Step<Token>, ParseError> {
        let Some(b) = self.peek() else {
            return Ok(Step::NeedMore(None));
        };
        match b {
            b'"' => self.quoted(),
            b'{' => self.literal(),
            b'(' => self.list(depth),
            b')' => Err(ParseError::Unexpected {
                found: ')',
                offset: self.pos,
            }),
            b'\r' => Err(ParseError::Unexpected {
                found: '\r',
                offset: self.pos,
            }),
            _ if is_atom_char(b) => self.atom(),
            _ => Err(ParseError::Unexpected {
                found: char::from(b),
                offset: self.pos,
            }),
        }
    }

    fn atom(&mut self) -> Result<Step<Token>, ParseError> {
        let start = self.pos;
        while let Some(b) = self.peek() {
            if !is_atom_char(b) {
                let atom = String::from_utf8_lossy(&self.buf[start..self.pos]).into_owned();
                return Ok(Step::Done(Token::Atom(atom)));
            }
            self.pos += 1;
        }
        Ok(Step::NeedMore(None))
    }

    fn quoted(&mut self) -> Result<Step<Token>, ParseError> {
        // Opening quote.
        self.pos += 1;
        let mut out = Vec::new();
        loop {
            let Some(b) = self.peek() else {
                return Ok(Step::NeedMore(None));
            };
            match b {
                b'"' => {
                    self.pos += 1;
                    return Ok(Step::Done(Token::String(out)));
                }
                b'\\' => {
                    let Some(&next) = self.buf.get(self.pos + 1) else {
                        return Ok(Step::NeedMore(None));
                    };
                    if matches!(next, b'\r' | b'\n') {
                        return Err(ParseError::UnterminatedQuoted { offset: self.pos });
                    }
                    // RFC 5804 only allows `\"` and `\\`; other escapes are
                    // tolerated by taking the escaped octet literally.
                    out.push(next);
                    self.pos += 2;
                }
                b'\r' | b'\n' => {
                    return Err(ParseError::UnterminatedQuoted { offset: self.pos });
                }
                _ => {
                    out.push(b);
                    self.pos += 1;
                }
            }
            self.check_line_len()?;
        }
    }

    fn literal(&mut self) -> Result<Step<Token>, ParseError> {
        let open = self.pos;
        self.pos += 1;
        let mut size: usize = 0;
        let mut digits = 0usize;
        loop {
            let Some(b) = self.peek() else {
                return Ok(Step::NeedMore(None));
            };
            if b.is_ascii_digit() {
                digits += 1;
                size = size
                    .checked_mul(10)
                    .and_then(|s| s.checked_add(usize::from(b - b'0')))
                    .ok_or(ParseError::LiteralTooLarge {
                        limit: self.limits.max_literal,
                    })?;
                if size > self.limits.max_literal {
                    return Err(ParseError::LiteralTooLarge {
                        limit: self.limits.max_literal,
                    });
                }
                self.pos += 1;
            } else {
                break;
            }
        }
        if digits == 0 {
            return Err(ParseError::InvalidLiteral { offset: open });
        }
        // Accept the non-synchronizing form `{n+}` as well.
        if self.peek() == Some(b'+') {
            self.pos += 1;
        }
        match self.peek() {
            None => return Ok(Step::NeedMore(None)),
            Some(b'}') => self.pos += 1,
            Some(_) => return Err(ParseError::InvalidLiteral { offset: open }),
        }
        match self.eol() {
            Step::NeedMore(n) => return Ok(Step::NeedMore(n)),
            Step::Done(true) => {}
            Step::Done(false) => return Err(ParseError::InvalidLiteral { offset: open }),
        }
        let start = self.pos;
        let end = start + size;
        if self.buf.len() < end {
            // Partial payload must not count toward the line length limit.
            self.literal_bytes += self.buf.len() - start;
            return Ok(Step::NeedMore(Some(end)));
        }
        self.pos = end;
        self.literal_bytes += size;
        Ok(Step::Done(Token::String(self.buf[start..end].to_vec())))
    }

    fn list(&mut self, depth: usize) -> Result<Step<Token>, ParseError> {
        if depth >= MAX_LIST_DEPTH {
            return Err(ParseError::TooDeeplyNested);
        }
        // Opening paren.
        self.pos += 1;
        let mut items = Vec::new();
        loop {
            self.skip_spaces();
            match self.peek() {
                None => return Ok(Step::NeedMore(None)),
                Some(b')') => {
                    self.pos += 1;
                    return Ok(Step::Done(Token::List(items)));
                }
                Some(b'\r' | b'\n') => {
                    return Err(ParseError::UnterminatedList { offset: self.pos });
                }
                Some(_) => {
                    let token = need!(self.token(depth + 1)?);
                    items.push(token);
                    self.check_line_len()?;
                }
            }
        }
    }
}

fn is_atom_char(b: u8) -> bool {
    // ATOM-SPECIALS = "(" / ")" / "{" / SP / CTL / QUOTED-SPECIALS.
    // Octets >= 0x80 are tolerated so odd servers don't break parsing.
    !matches!(
        b,
        b'(' | b')' | b'{' | b' ' | b'"' | b'\\' | 0x00..=0x1f | 0x7f
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn atom(s: &str) -> Token {
        Token::Atom(s.to_owned())
    }

    fn string(s: &str) -> Token {
        Token::String(s.as_bytes().to_vec())
    }

    fn parse(buf: &[u8]) -> Result<Parsed, ParseError> {
        parse_line(buf, &Limits::default())
    }

    fn complete(buf: &[u8]) -> (Vec<Token>, usize) {
        match parse(buf) {
            Ok(Parsed::Complete { tokens, consumed }) => (tokens, consumed),
            other => panic!("expected complete line, got {other:?}"),
        }
    }

    #[test]
    fn simple_ok() {
        let (tokens, consumed) = complete(b"OK\r\n");
        assert_eq!(tokens, vec![atom("OK")]);
        assert_eq!(consumed, 4);
    }

    #[test]
    fn ok_with_code_and_text() {
        let (tokens, _) = complete(b"NO (QUOTA/MAXSIZE) \"Quota exceeded\"\r\n");
        assert_eq!(
            tokens,
            vec![
                atom("NO"),
                Token::List(vec![atom("QUOTA/MAXSIZE")]),
                string("Quota exceeded")
            ]
        );
    }

    #[test]
    fn quoted_escapes() {
        let (tokens, _) = complete(b"\"a \\\"b\\\" \\\\ c\"\r\n");
        assert_eq!(tokens, vec![string("a \"b\" \\ c")]);
    }

    #[test]
    fn quoted_utf8() {
        let (tokens, _) = complete("\"Grüße ✓\"\r\n".as_bytes());
        assert_eq!(tokens, vec![string("Grüße ✓")]);
    }

    #[test]
    fn literal_sync_and_nonsync() {
        let (tokens, consumed) = complete(b"{13}\r\nclever\"script\r\n");
        assert_eq!(tokens, vec![string("clever\"script")]);
        assert_eq!(consumed, 21);
        let (tokens, _) = complete(b"{3+}\r\nabc ACTIVE\r\n");
        assert_eq!(tokens, vec![string("abc"), atom("ACTIVE")]);
    }

    #[test]
    fn literal_with_crlf_inside() {
        let (tokens, _) = complete(b"NO {12}\r\nline1\r\nline2\r\n");
        assert_eq!(tokens, vec![atom("NO"), string("line1\r\nline2")]);
    }

    #[test]
    fn literal_inside_list() {
        let (tokens, _) = complete(b"OK (TAG {16}\r\nSTARTTLS-SYNC-42) \"Done\"\r\n");
        assert_eq!(
            tokens,
            vec![
                atom("OK"),
                Token::List(vec![atom("TAG"), string("STARTTLS-SYNC-42")]),
                string("Done"),
            ]
        );
    }

    #[test]
    fn empty_literal_and_empty_quoted() {
        let (tokens, _) = complete(b"{0}\r\n \"\"\r\n");
        assert_eq!(tokens, vec![string(""), string("")]);
    }

    #[test]
    fn nested_list() {
        let (tokens, _) = complete(b"OK (X-EXT 1 (a \"b\" ()) c)\r\n");
        assert_eq!(
            tokens,
            vec![
                atom("OK"),
                Token::List(vec![
                    atom("X-EXT"),
                    atom("1"),
                    Token::List(vec![atom("a"), string("b"), Token::List(vec![])]),
                    atom("c"),
                ]),
            ]
        );
    }

    #[test]
    fn bare_lf_is_accepted() {
        let (tokens, consumed) = complete(b"OK \"x\"\n");
        assert_eq!(tokens, vec![atom("OK"), string("x")]);
        assert_eq!(consumed, 7);
    }

    #[test]
    fn consumes_only_first_line() {
        let buf = b"\"SIEVE\" \"fileinto\"\r\nOK\r\n";
        let (tokens, consumed) = complete(buf);
        assert_eq!(tokens, vec![string("SIEVE"), string("fileinto")]);
        let (tokens, _) = complete(&buf[consumed..]);
        assert_eq!(tokens, vec![atom("OK")]);
    }

    /// Feeding any strict prefix of a valid line must report `Incomplete`,
    /// never an error or a bogus complete line.
    #[test]
    fn every_prefix_is_incomplete() {
        let lines: &[&[u8]] = &[
            b"OK\r\n",
            b"NO (QUOTA/MAXSIZE) \"Quota \\\"exceeded\\\"\"\r\n",
            b"OK (TAG {16}\r\nSTARTTLS-SYNC-42) \"Done\"\r\n",
            b"{13}\r\nclever\"script ACTIVE\r\n",
            b"OK (SASL \"dGVzdA==\") {5+}\r\nhello\r\n",
            "\"IMPLEMENTATION\" \"Grüße\"\r\n".as_bytes(),
        ];
        for line in lines {
            for cut in 0..line.len() {
                match parse(&line[..cut]) {
                    Ok(Parsed::Incomplete { needed }) => {
                        if let Some(n) = needed {
                            assert!(n > cut && n <= line.len(), "bad hint {n} at {cut}");
                        }
                    }
                    other => panic!(
                        "prefix {:?} gave {other:?}",
                        String::from_utf8_lossy(&line[..cut])
                    ),
                }
            }
            assert!(
                matches!(parse(line), Ok(Parsed::Complete { consumed, .. }) if consumed == line.len())
            );
        }
    }

    #[test]
    fn literal_hint_points_past_payload() {
        assert_eq!(
            parse(b"{10}\r\nabc").unwrap(),
            Parsed::Incomplete { needed: Some(16) }
        );
    }

    #[test]
    fn literal_too_large() {
        let limits = Limits {
            max_literal: 10,
            max_line: 1000,
        };
        assert!(matches!(
            parse_line(b"{11}\r\n", &limits),
            Err(ParseError::LiteralTooLarge { limit: 10 })
        ));
        assert!(matches!(
            parse(b"{99999999999999999999999999}\r\n"),
            Err(ParseError::LiteralTooLarge { .. })
        ));
    }

    #[test]
    fn line_too_long() {
        let limits = Limits {
            max_literal: 1000,
            max_line: 16,
        };
        assert!(matches!(
            parse_line(b"\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", &limits),
            Err(ParseError::LineTooLong { limit: 16 })
        ));
        // Literal payload does not count toward the line limit.
        let (tokens, _) = match parse_line(b"{30}\r\naaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\r\n", &limits) {
            Ok(Parsed::Complete { tokens, consumed }) => (tokens, consumed),
            other => panic!("{other:?}"),
        };
        assert_eq!(tokens.len(), 1);
    }

    #[test]
    fn malformed_inputs_error() {
        let bad: &[&[u8]] = &[
            b"\"abc\r\n",
            b"{abc}\r\n",
            b"{}\r\n",
            b"{3}x\r\n",
            b"{3\r\n",
            b"OK )\r\n",
            b"OK (abc\r\n",
            b"OK \x01\r\n",
            b"OK \r\r\n",
        ];
        for input in bad {
            assert!(
                parse(input).is_err(),
                "expected error for {:?}",
                String::from_utf8_lossy(input)
            );
        }
    }

    #[test]
    fn deep_nesting_rejected() {
        let mut input = b"OK ".to_vec();
        input.extend(std::iter::repeat_n(b'(', 50));
        assert!(matches!(parse(&input), Err(ParseError::TooDeeplyNested)));
    }

    #[test]
    fn garbage_never_panics() {
        // Cheap deterministic fuzzing over a byte alphabet rich in specials.
        let alphabet = b"{}()\"\\ \r\n+0123AZaz\x00\xff";
        let mut state: u32 = 0x1234_5678;
        for _ in 0..20_000 {
            let len = (state % 24) as usize;
            let mut input = Vec::with_capacity(len);
            for _ in 0..len {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                input.push(alphabet[(state as usize) % alphabet.len()]);
            }
            let _ = parse(&input);
        }
    }
}
