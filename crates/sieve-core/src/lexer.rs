//! Tokenizer for RFC 5228 §2 / §8.1.

use crate::ast::{Comment, CommentKind, Quantifier, StringKind};
use crate::span::{ParseError, Span};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TokenKind {
    Identifier(String),
    /// A tag without its leading colon.
    Tag(String),
    Number {
        value: u64,
        quantifier: Option<Quantifier>,
    },
    String {
        value: String,
        kind: StringKind,
    },
    Comment(Comment),
    LBracket,
    RBracket,
    LParen,
    RParen,
    LBrace,
    RBrace,
    Comma,
    Semicolon,
    Eof,
}

impl TokenKind {
    pub(crate) fn describe(&self) -> &'static str {
        match self {
            TokenKind::Identifier(_) => "identifier",
            TokenKind::Tag(_) => "tag",
            TokenKind::Number { .. } => "number",
            TokenKind::String { .. } => "string",
            TokenKind::Comment(_) => "comment",
            TokenKind::LBracket => "'['",
            TokenKind::RBracket => "']'",
            TokenKind::LParen => "'('",
            TokenKind::RParen => "')'",
            TokenKind::LBrace => "'{'",
            TokenKind::RBrace => "'}'",
            TokenKind::Comma => "','",
            TokenKind::Semicolon => "';'",
            TokenKind::Eof => "end of script",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

pub(crate) struct Lexer<'a> {
    src: &'a str,
    pos: usize,
}

impl<'a> Lexer<'a> {
    pub(crate) fn new(src: &'a str) -> Self {
        Self { src, pos: 0 }
    }

    /// The source text between two offsets; used by the parser to detect line
    /// breaks between tokens.
    pub(crate) fn slice(&self, span: Span) -> &'a str {
        &self.src[span.start..span.end]
    }

    fn rest(&self) -> &'a str {
        &self.src[self.pos..]
    }

    fn peek_char(&self) -> Option<char> {
        self.rest().chars().next()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek_char()?;
        self.pos += c.len_utf8();
        Some(c)
    }

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.peek_char() {
            if matches!(c, ' ' | '\t' | '\r' | '\n') {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    pub(crate) fn next_token(&mut self) -> Result<Token, ParseError> {
        self.skip_whitespace();
        let start = self.pos;
        let Some(c) = self.bump() else {
            return Ok(Token { kind: TokenKind::Eof, span: Span::new(start, start) });
        };
        let kind = match c {
            '[' => TokenKind::LBracket,
            ']' => TokenKind::RBracket,
            '(' => TokenKind::LParen,
            ')' => TokenKind::RParen,
            '{' => TokenKind::LBrace,
            '}' => TokenKind::RBrace,
            ',' => TokenKind::Comma,
            ';' => TokenKind::Semicolon,
            '#' => {
                let text = self.read_line_rest();
                TokenKind::Comment(Comment {
                    text: text.to_owned(),
                    kind: CommentKind::Hash,
                    span: Span::new(start, self.pos),
                })
            }
            '/' if self.peek_char() == Some('*') => {
                self.pos += 1;
                let body_start = self.pos;
                let Some(len) = self.rest().find("*/") else {
                    return Err(ParseError::new(
                        "unterminated comment",
                        Span::new(start, self.src.len()),
                    ));
                };
                self.pos += len + 2;
                TokenKind::Comment(Comment {
                    text: self.src[body_start..body_start + len].to_owned(),
                    kind: CommentKind::Bracket,
                    span: Span::new(start, self.pos),
                })
            }
            '"' => TokenKind::String {
                value: self.read_quoted(start)?,
                kind: StringKind::Quoted,
            },
            ':' => {
                if !self.peek_char().is_some_and(is_ident_start) {
                    return Err(ParseError::new(
                        "expected a tag name after ':'",
                        Span::new(start, self.pos),
                    ));
                }
                TokenKind::Tag(self.read_ident_rest().to_owned())
            }
            '0'..='9' => self.read_number(start)?,
            c if is_ident_start(c) => {
                self.pos = start;
                let name = self.read_ident_rest();
                if name.eq_ignore_ascii_case("text") && self.peek_char() == Some(':') {
                    self.pos += 1;
                    TokenKind::String {
                        value: self.read_multiline(start)?,
                        kind: StringKind::MultiLine,
                    }
                } else {
                    TokenKind::Identifier(name.to_owned())
                }
            }
            other => {
                return Err(ParseError::new(
                    format!("unexpected character {other:?}"),
                    Span::new(start, self.pos),
                ));
            }
        };
        Ok(Token { kind, span: Span::new(start, self.pos) })
    }

    /// Consumes the rest of the line up to the line ending; returns it
    /// without trailing `\r`s (which are left to the whitespace skipper).
    fn read_line_rest(&mut self) -> &'a str {
        let rest = self.rest();
        let len = rest.find('\n').unwrap_or(rest.len());
        let line = rest[..len].trim_end_matches('\r');
        self.pos += line.len();
        line
    }

    fn read_ident_rest(&mut self) -> &'a str {
        let start = self.pos;
        while self.peek_char().is_some_and(is_ident_char) {
            self.pos += 1;
        }
        &self.src[start..self.pos]
    }

    fn read_quoted(&mut self, start: usize) -> Result<String, ParseError> {
        let mut value = String::new();
        loop {
            match self.bump() {
                None => {
                    return Err(ParseError::new(
                        "unterminated string",
                        Span::new(start, self.src.len()),
                    ));
                }
                Some('"') => return Ok(value),
                // RFC 5228 §2.4.2: "\\" and "\"" are escapes; any other
                // backslash is dropped ("\a" means "a").
                Some('\\') => match self.bump() {
                    Some(c) => value.push(c),
                    None => {
                        return Err(ParseError::new(
                            "unterminated string",
                            Span::new(start, self.src.len()),
                        ));
                    }
                },
                Some(c) => value.push(c),
            }
        }
    }

    fn read_number(&mut self, start: usize) -> Result<TokenKind, ParseError> {
        while self.peek_char().is_some_and(|c| c.is_ascii_digit()) {
            self.pos += 1;
        }
        let digits = &self.src[start..self.pos];
        let quantifier = match self.peek_char() {
            Some('K' | 'k') => Some(Quantifier::K),
            Some('M' | 'm') => Some(Quantifier::M),
            Some('G' | 'g') => Some(Quantifier::G),
            _ => None,
        };
        if quantifier.is_some() {
            self.pos += 1;
        }
        if self.peek_char().is_some_and(is_ident_char) {
            self.read_ident_rest();
            return Err(ParseError::new("invalid number", Span::new(start, self.pos)));
        }
        let span = Span::new(start, self.pos);
        let value = digits
            .parse::<u64>()
            .ok()
            .and_then(|v| v.checked_mul(quantifier.map_or(1, Quantifier::multiplier)))
            .ok_or_else(|| ParseError::new("number too large", span))?;
        Ok(TokenKind::Number { value, quantifier })
    }

    /// Reads the body of a `text:` string; `self.pos` is just after the colon.
    fn read_multiline(&mut self, start: usize) -> Result<String, ParseError> {
        while matches!(self.peek_char(), Some(' ' | '\t')) {
            self.pos += 1;
        }
        // An optional hash comment may follow `text:` on the same line. It is
        // not part of the value and, being inside a token, is not kept.
        if self.peek_char() == Some('#') {
            self.read_line_rest();
        }
        if self.rest().starts_with("\r\n") {
            self.pos += 2;
        } else if self.rest().starts_with('\n') {
            self.pos += 1;
        } else {
            return Err(ParseError::new(
                "expected a line break after 'text:'",
                Span::new(start, self.pos),
            ));
        }

        let mut value = String::new();
        loop {
            let rest = self.rest();
            if rest.is_empty() {
                return Err(ParseError::new(
                    "unterminated multi-line string (missing '.' line)",
                    Span::new(start, self.src.len()),
                ));
            }
            let line_len = rest.find('\n').map_or(rest.len(), |i| i + 1);
            let line = &rest[..line_len];
            self.pos += line_len;
            let content = line.trim_end_matches(['\r', '\n']);
            if content == "." {
                return Ok(value);
            }
            // RFC 5228 §8.1: remove a leading '.' if followed by another '.'.
            value.push_str(line.strip_prefix("..").map_or(line, |_| &line[1..]));
        }
    }
}

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(src: &str) -> Vec<TokenKind> {
        let mut lexer = Lexer::new(src);
        let mut out = Vec::new();
        loop {
            let tok = lexer.next_token().unwrap();
            if tok.kind == TokenKind::Eof {
                return out;
            }
            out.push(tok.kind);
        }
    }

    fn err(src: &str) -> ParseError {
        let mut lexer = Lexer::new(src);
        loop {
            match lexer.next_token() {
                Ok(t) if t.kind == TokenKind::Eof => panic!("no error for {src:?}"),
                Ok(_) => {}
                Err(e) => return e,
            }
        }
    }

    fn string(v: &str, kind: StringKind) -> TokenKind {
        TokenKind::String { value: v.into(), kind }
    }

    #[test]
    fn punctuation_identifiers_tags() {
        assert_eq!(
            kinds("if header :contains [\"a\", \"b\"] { stop; }"),
            vec![
                TokenKind::Identifier("if".into()),
                TokenKind::Identifier("header".into()),
                TokenKind::Tag("contains".into()),
                TokenKind::LBracket,
                string("a", StringKind::Quoted),
                TokenKind::Comma,
                string("b", StringKind::Quoted),
                TokenKind::RBracket,
                TokenKind::LBrace,
                TokenKind::Identifier("stop".into()),
                TokenKind::Semicolon,
                TokenKind::RBrace,
            ]
        );
    }

    #[test]
    fn numbers_with_quantifiers() {
        assert_eq!(
            kinds("100 10K 2m 1G"),
            vec![
                TokenKind::Number { value: 100, quantifier: None },
                TokenKind::Number { value: 10 * 1024, quantifier: Some(Quantifier::K) },
                TokenKind::Number { value: 2 << 20, quantifier: Some(Quantifier::M) },
                TokenKind::Number { value: 1 << 30, quantifier: Some(Quantifier::G) },
            ]
        );
        assert_eq!(err("10KB").message, "invalid number");
        assert_eq!(err("99999999999999999999").message, "number too large");
    }

    #[test]
    fn quoted_string_escapes() {
        assert_eq!(
            kinds(r#""a\"b\\c\d" "multi
line""#),
            vec![string("a\"b\\cd", StringKind::Quoted), string("multi\nline", StringKind::Quoted)]
        );
        assert_eq!(err("\"open").message, "unterminated string");
        assert_eq!(err("\"open\\").message, "unterminated string");
    }

    #[test]
    fn multiline_strings() {
        let src = "text:\r\nline 1\r\n..dot\r\n.single\r\n.\r\n;";
        assert_eq!(
            kinds(src),
            vec![
                string("line 1\r\n.dot\r\n.single\r\n", StringKind::MultiLine),
                TokenKind::Semicolon,
            ]
        );
        assert_eq!(
            kinds("TEXT:  # note\nhi\n.\n"),
            vec![string("hi\n", StringKind::MultiLine)]
        );
        assert_eq!(kinds("text:\n.\n"), vec![string("", StringKind::MultiLine)]);
        // final "." without newline at end of input is accepted
        assert_eq!(kinds("text:\nx\n."), vec![string("x\n", StringKind::MultiLine)]);
        assert!(err("text:\nno end\n").message.starts_with("unterminated multi-line"));
        assert!(err("text: x\n.\n").message.starts_with("expected a line break"));
        // `text` without colon is an ordinary identifier
        assert_eq!(kinds("text"), vec![TokenKind::Identifier("text".into())]);
    }

    #[test]
    fn comments() {
        let toks = kinds("# hash\r\n/* multi\nline */ x # end");
        let texts: Vec<_> = toks
            .iter()
            .map(|t| match t {
                TokenKind::Comment(c) => (c.kind, c.text.as_str()),
                TokenKind::Identifier(i) => (CommentKind::Hash, i.as_str()),
                _ => unreachable!(),
            })
            .collect();
        assert_eq!(
            texts,
            vec![
                (CommentKind::Hash, " hash"),
                (CommentKind::Bracket, " multi\nline "),
                (CommentKind::Hash, "x"),
                (CommentKind::Hash, " end"),
            ]
        );
        assert_eq!(err("/* open").message, "unterminated comment");
    }

    #[test]
    fn unexpected_characters() {
        assert_eq!(err("a @ b").span, Span::new(2, 3));
        assert_eq!(err(": x").message, "expected a tag name after ':'");
        assert_eq!(err("ä").message, "unexpected character 'ä'");
    }
}
