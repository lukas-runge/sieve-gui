//! Recursive-descent parser for the RFC 5228 §8.2 grammar.

use crate::ast::*;
use crate::lexer::{Lexer, Token, TokenKind};
use crate::span::{ParseError, Span};

/// Maximum nesting of blocks and test lists. Protects against stack overflow
/// on hostile input; real scripts stay far below this.
const MAX_DEPTH: usize = 128;

/// Parses a complete script.
pub fn parse(src: &str) -> Result<Script, ParseError> {
    let mut parser = Parser {
        lexer: Lexer::new(src),
        peeked: None,
        pending_comments: Vec::new(),
        last_end: 0,
        depth: 0,
    };
    let (commands, trailing_comments) = parser.parse_commands(false)?;
    Ok(Script { commands, trailing_comments })
}

struct Parser<'a> {
    lexer: Lexer<'a>,
    peeked: Option<Token>,
    /// Comments read while looking for the next real token, waiting to be
    /// attached to a node.
    pending_comments: Vec<Comment>,
    /// End offset of the last consumed real token.
    last_end: usize,
    depth: usize,
}

impl Parser<'_> {
    /// Returns the next non-comment token without consuming it. Comments read
    /// on the way are moved to `pending_comments`.
    fn peek(&mut self) -> Result<&Token, ParseError> {
        if self.peeked.is_none() {
            loop {
                let tok = self.lexer.next_token()?;
                match tok.kind {
                    TokenKind::Comment(c) => self.pending_comments.push(c),
                    _ => {
                        self.peeked = Some(tok);
                        break;
                    }
                }
            }
        }
        Ok(self.peeked.as_ref().expect("peeked token was just set"))
    }

    fn next(&mut self) -> Result<Token, ParseError> {
        self.peek()?;
        let tok = self.peeked.take().expect("peeked token was just set");
        self.last_end = tok.span.end;
        Ok(tok)
    }

    fn expect(&mut self, kind: TokenKind) -> Result<Token, ParseError> {
        let tok = self.next()?;
        if tok.kind == kind {
            Ok(tok)
        } else {
            Err(unexpected(&tok, kind.describe()))
        }
    }

    fn enter(&mut self, span: Span) -> Result<(), ParseError> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(ParseError::new("script is nested too deeply", span));
        }
        Ok(())
    }

    fn leave(&mut self) {
        self.depth -= 1;
    }

    /// Takes the pending comments that start before `before`.
    fn take_comments_before(&mut self, before: usize) -> Vec<Comment> {
        let split = self.pending_comments.partition_point(|c| c.span.start < before);
        self.pending_comments.drain(..split).collect()
    }

    /// Parses commands until `}` (if `in_block`) or end of input, leaving the
    /// terminator unconsumed. Returns the commands and the comments after them.
    fn parse_commands(&mut self, in_block: bool) -> Result<(Vec<Command>, Vec<Comment>), ParseError> {
        let mut commands = Vec::new();
        loop {
            let tok = self.peek()?;
            match &tok.kind {
                TokenKind::RBrace if in_block => break,
                TokenKind::Eof if !in_block => break,
                TokenKind::Eof => return Err(ParseError::new("missing '}'", tok.span)),
                TokenKind::Identifier(_) => {
                    let start = tok.span.start;
                    let prev_end = self.last_end;
                    let leading_comments = self.take_comments_before(start);
                    let full_start = leading_comments.first().map_or(start, |c| c.span.start);
                    let blank_line_before = !commands.is_empty()
                        && has_blank_line(self.lexer.slice(Span::new(prev_end, full_start)));
                    let mut cmd = self.parse_command(leading_comments)?;
                    cmd.blank_line_before = blank_line_before;
                    commands.push(cmd);
                }
                _ => return Err(unexpected(tok, "a command")),
            }
        }
        let end = self.peek()?.span.start;
        let trailing = self.take_comments_before(end);
        Ok((commands, trailing))
    }

    fn parse_command(&mut self, leading_comments: Vec<Comment>) -> Result<Command, ParseError> {
        let name = self.parse_identifier()?;
        let arguments = self.parse_arguments()?;
        let tok = self.next()?;
        let inner_comments = self.take_comments_before(tok.span.start);
        let block = match tok.kind {
            TokenKind::Semicolon => None,
            TokenKind::LBrace => {
                self.enter(tok.span)?;
                let (commands, trailing_comments) = self.parse_commands(true)?;
                let close = self.expect(TokenKind::RBrace)?;
                self.leave();
                Some(Block { commands, trailing_comments, span: tok.span.to(close.span) })
            }
            _ => return Err(unexpected(&tok, "';' or '{'")),
        };
        let span = name.span.to(Span::new(self.last_end, self.last_end));
        let trailing_comment = self.take_same_line_comment()?;
        Ok(Command {
            name,
            arguments,
            block,
            leading_comments,
            inner_comments,
            trailing_comment,
            blank_line_before: false,
            span,
        })
    }

    /// If the next comment starts on the same line as the last token, takes it.
    fn take_same_line_comment(&mut self) -> Result<Option<Comment>, ParseError> {
        self.peek()?;
        let Some(first) = self.pending_comments.first() else {
            return Ok(None);
        };
        let between = self.lexer.slice(Span::new(self.last_end, first.span.start));
        if between.contains('\n') {
            return Ok(None);
        }
        Ok(Some(self.pending_comments.remove(0)))
    }

    fn parse_identifier(&mut self) -> Result<Identifier, ParseError> {
        let tok = self.next()?;
        match tok.kind {
            TokenKind::Identifier(name) => Ok(Identifier { name, span: tok.span }),
            _ => Err(unexpected(&tok, "an identifier")),
        }
    }

    fn parse_arguments(&mut self) -> Result<Arguments, ParseError> {
        let mut args = Vec::new();
        loop {
            let tok = self.peek()?;
            let arg = match &tok.kind {
                TokenKind::LBracket => Argument::StringList(self.parse_string_list()?),
                TokenKind::String { .. } => Argument::String(self.parse_string()?),
                TokenKind::Tag(_) => {
                    let tok = self.next()?;
                    let TokenKind::Tag(name) = tok.kind else { unreachable!() };
                    Argument::Tag(Identifier { name, span: tok.span })
                }
                TokenKind::Number { .. } => {
                    let tok = self.next()?;
                    let TokenKind::Number { value, quantifier } = tok.kind else { unreachable!() };
                    Argument::Number(Number { value, quantifier, span: tok.span })
                }
                _ => break,
            };
            args.push(arg);
        }

        let tok = self.peek()?;
        let tests = match tok.kind {
            TokenKind::Identifier(_) => Some(Tests::Single(Box::new(self.parse_test()?))),
            TokenKind::LParen => Some(Tests::List(self.parse_test_list()?)),
            _ => None,
        };
        Ok(Arguments { args, tests })
    }

    fn parse_test(&mut self) -> Result<Test, ParseError> {
        let name = self.parse_identifier()?;
        self.enter(name.span)?;
        let arguments = self.parse_arguments()?;
        self.leave();
        let span = name.span.to(Span::new(self.last_end, self.last_end));
        Ok(Test { name, arguments, span })
    }

    fn parse_test_list(&mut self) -> Result<TestList, ParseError> {
        let open = self.expect(TokenKind::LParen)?;
        let mut tests = Vec::new();
        loop {
            let tok = self.peek()?;
            if !matches!(tok.kind, TokenKind::Identifier(_)) {
                return Err(unexpected(tok, "a test"));
            }
            tests.push(self.parse_test()?);
            let tok = self.next()?;
            match tok.kind {
                TokenKind::Comma => {}
                TokenKind::RParen => {
                    return Ok(TestList { tests, span: open.span.to(tok.span) });
                }
                _ => return Err(unexpected(&tok, "',' or ')'")),
            }
        }
    }

    fn parse_string(&mut self) -> Result<SieveString, ParseError> {
        let tok = self.next()?;
        match tok.kind {
            TokenKind::String { value, kind } => Ok(SieveString { value, kind, span: tok.span }),
            _ => Err(unexpected(&tok, "a string")),
        }
    }

    fn parse_string_list(&mut self) -> Result<StringList, ParseError> {
        let open = self.expect(TokenKind::LBracket)?;
        let mut items = Vec::new();
        loop {
            items.push(self.parse_string()?);
            let tok = self.next()?;
            match tok.kind {
                TokenKind::Comma => {}
                TokenKind::RBracket => {
                    return Ok(StringList { items, span: open.span.to(tok.span) });
                }
                _ => return Err(unexpected(&tok, "',' or ']'")),
            }
        }
    }
}

fn unexpected(tok: &Token, expected: &str) -> ParseError {
    ParseError::new(format!("expected {expected}, found {}", tok.kind.describe()), tok.span)
}

/// Whether the whitespace between two tokens contains an empty line.
fn has_blank_line(between: &str) -> bool {
    let lines: Vec<_> = between.split('\n').collect();
    lines.len() > 2 && lines[1..lines.len() - 1].iter().any(|l| l.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(src: &str) -> Script {
        parse(src).unwrap_or_else(|e| panic!("{e} in {src:?}"))
    }

    fn err(src: &str) -> String {
        parse(src).expect_err(src).message
    }

    #[test]
    fn simple_rule() {
        let s = ok(r#"require ["fileinto"];
if header :contains "from" "@tax-office.example" {
    fileinto "Taxes";
    stop;
}"#);
        assert_eq!(s.commands.len(), 2);
        let req = &s.commands[0];
        assert!(req.is("require"));
        assert!(matches!(&req.arguments.args[0], Argument::StringList(l) if l.items[0].value == "fileinto"));

        let rule = &s.commands[1];
        assert!(rule.is("IF"));
        let Some(Tests::Single(test)) = &rule.arguments.tests else { panic!() };
        assert!(test.is("header"));
        assert!(test.arguments.args[0].is_tag("contains"));
        assert_eq!(test.arguments.args.len(), 3);
        let block = rule.block.as_ref().unwrap();
        assert_eq!(block.commands.len(), 2);
        assert!(block.commands[1].is("stop"));
        assert!(!rule.blank_line_before);
    }

    #[test]
    fn test_lists_and_nesting() {
        let s = ok(r#"if anyof (not exists "x", allof (size :over 1M, true)) { keep; } elsif false { discard; } else { stop; }"#);
        assert_eq!(s.commands.len(), 3);
        let Some(Tests::Single(anyof)) = &s.commands[0].arguments.tests else { panic!() };
        let Some(Tests::List(list)) = &anyof.arguments.tests else { panic!() };
        assert_eq!(list.tests.len(), 2);
        let Some(Tests::Single(inner)) = &list.tests[0].arguments.tests else { panic!() };
        assert!(inner.is("exists"));
        let Some(Tests::List(all)) = &list.tests[1].arguments.tests else { panic!() };
        assert!(matches!(&all.tests[0].arguments.args[1], Argument::Number(n) if n.value == 1 << 20));
        assert!(s.commands[2].is("else"));
    }

    #[test]
    fn spans() {
        let src = "  keep;\nif true {\n  stop;\n}";
        let s = ok(src);
        assert_eq!(&src[s.commands[0].span.start..s.commands[0].span.end], "keep;");
        assert_eq!(&src[s.commands[1].span.start..s.commands[1].span.end], "if true {\n  stop;\n}");
        let block = s.commands[1].block.as_ref().unwrap();
        assert_eq!(&src[block.span.start..block.span.end], "{\n  stop;\n}");
    }

    #[test]
    fn comment_attachment() {
        let src = "# header\n\n# rule:[Taxes]\nif false # header :contains \"from\" \"x\"\n{\n  fileinto \"Taxes\"; # move\n  # end of block\n}\n/* bye */\n";
        let s = ok(src);
        let rule = &s.commands[0];
        let texts = |cs: &[Comment]| cs.iter().map(|c| c.text.clone()).collect::<Vec<_>>();
        assert_eq!(texts(&rule.leading_comments), vec![" header", " rule:[Taxes]"]);
        assert_eq!(texts(&rule.inner_comments), vec![" header :contains \"from\" \"x\""]);
        let block = rule.block.as_ref().unwrap();
        assert_eq!(block.commands[0].trailing_comment.as_ref().unwrap().text, " move");
        assert_eq!(texts(&block.trailing_comments), vec![" end of block"]);
        assert_eq!(texts(&s.trailing_comments), vec![" bye "]);
        assert_eq!(rule.full_span().start, 0);
    }

    #[test]
    fn blank_lines() {
        let s = ok("keep;\nkeep;\n\nkeep;\n\n# c\nkeep;\n  \n  keep;");
        let blanks: Vec<_> = s.commands.iter().map(|c| c.blank_line_before).collect();
        assert_eq!(blanks, vec![false, false, true, true, true]);
    }

    #[test]
    fn errors() {
        assert_eq!(err("keep"), "expected ';' or '{', found end of script");
        assert_eq!(err("if true { keep;"), "missing '}'");
        assert_eq!(err("}"), "expected a command, found '}'");
        assert_eq!(err("if anyof () {}"), "expected a test, found ')'");
        assert_eq!(err("if anyof (true; false) {}"), "expected ',' or ')', found ';'");
        assert_eq!(err("x [];"), "expected a string, found ']'");
        assert_eq!(err("x [\"a\" \"b\"];"), "expected ',' or ']', found string");
        assert_eq!(err("\"a\";"), "expected a command, found string");
        let deep = "if not ".repeat(200) + "true {}";
        assert_eq!(err(&deep), "script is nested too deeply");
        let blocks = "if true {".repeat(200) + &"}".repeat(200);
        assert_eq!(err(&blocks), "script is nested too deeply");
    }

    #[test]
    fn empty_and_comment_only() {
        assert_eq!(ok(""), Script::default());
        let s = ok("# only a comment");
        assert!(s.commands.is_empty());
        assert_eq!(s.trailing_comments.len(), 1);
    }
}
