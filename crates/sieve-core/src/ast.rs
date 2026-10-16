//! Generic syntax tree for Sieve scripts (RFC 5228 §8).
//!
//! The tree is purely syntactic: it knows commands, tests and arguments but
//! nothing about what `fileinto` or `header` mean. Every node keeps its source
//! span, and comments are attached to the nearest node so that a script can be
//! printed again without losing them.

use serde::Serialize;

use crate::span::Span;

/// A parsed script.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct Script {
    pub commands: Vec<Command>,
    /// Comments after the last command.
    pub trailing_comments: Vec<Comment>,
}

/// A command such as `require`, `if` or `fileinto`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Command {
    pub name: Identifier,
    pub arguments: Arguments,
    /// `Some` for block commands (`if … { … }`), `None` for `…;` commands.
    pub block: Option<Block>,
    /// Comments between the previous command (or block start) and this one.
    pub leading_comments: Vec<Comment>,
    /// Comments between the command name and its `;` or `{`, e.g. the
    /// commented-out test in `if false # header :contains …`.
    pub inner_comments: Vec<Comment>,
    /// A comment on the same line directly after the `;` or `}`.
    pub trailing_comment: Option<Comment>,
    /// Whether the source had an empty line before this command (after its
    /// leading comments are taken into account).
    pub blank_line_before: bool,
    /// From the name to the terminating `;` or `}`, excluding comments.
    pub span: Span,
}

impl Command {
    /// Whether the command name matches `name` (case-insensitively).
    pub fn is(&self, name: &str) -> bool {
        self.name.is(name)
    }

    /// Span from the first leading comment (if any) to the end of the command,
    /// including a same-line trailing comment.
    pub fn full_span(&self) -> Span {
        let mut span = self.span;
        if let Some(first) = self.leading_comments.first() {
            span = span.to(first.span);
        }
        if let Some(trailing) = &self.trailing_comment {
            span = span.to(trailing.span);
        }
        span
    }
}

/// The `{ … }` body of a block command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Block {
    pub commands: Vec<Command>,
    /// Comments after the last command in the block, before `}`.
    pub trailing_comments: Vec<Comment>,
    /// From `{` to `}`.
    pub span: Span,
}

/// A test such as `header :contains "from" "x"` or `anyof (…)`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Test {
    pub name: Identifier,
    pub arguments: Arguments,
    pub span: Span,
}

impl Test {
    pub fn is(&self, name: &str) -> bool {
        self.name.is(name)
    }
}

/// The arguments of a command or test: positional arguments optionally
/// followed by a single test or a parenthesized test list.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct Arguments {
    pub args: Vec<Argument>,
    pub tests: Option<Tests>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum Tests {
    /// A bare test: `if header … {`, `not exists …`.
    Single(Box<Test>),
    /// A parenthesized list: `anyof (a, b)`.
    List(TestList),
}

impl Tests {
    pub fn as_slice(&self) -> &[Test] {
        match self {
            Tests::Single(t) => std::slice::from_ref(t),
            Tests::List(l) => &l.tests,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TestList {
    pub tests: Vec<Test>,
    /// From `(` to `)`.
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Argument {
    /// A single string, not in brackets.
    String(SieveString),
    /// A bracketed string list `["a", "b"]`.
    StringList(StringList),
    Number(Number),
    /// A tagged argument such as `:contains`.
    Tag(Identifier),
}

impl Argument {
    pub fn span(&self) -> Span {
        match self {
            Argument::String(s) => s.span,
            Argument::StringList(l) => l.span,
            Argument::Number(n) => n.span,
            Argument::Tag(t) => t.span,
        }
    }

    /// Whether this is the tag `:name` (case-insensitively, without colon).
    pub fn is_tag(&self, name: &str) -> bool {
        matches!(self, Argument::Tag(t) if t.is(name))
    }

    /// The strings of a string or string-list argument.
    pub fn strings(&self) -> Option<&[SieveString]> {
        match self {
            Argument::String(s) => Some(std::slice::from_ref(s)),
            Argument::StringList(l) => Some(&l.items),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StringList {
    pub items: Vec<SieveString>,
    /// From `[` to `]`.
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SieveString {
    /// The decoded value (escapes and dot-stuffing removed).
    pub value: String,
    pub kind: StringKind,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum StringKind {
    /// `"…"`
    Quoted,
    /// `text: … .`
    MultiLine,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Number {
    /// The value with the quantifier already applied.
    pub value: u64,
    pub quantifier: Option<Quantifier>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Quantifier {
    K,
    M,
    G,
}

impl Quantifier {
    pub fn multiplier(self) -> u64 {
        match self {
            Quantifier::K => 1 << 10,
            Quantifier::M => 1 << 20,
            Quantifier::G => 1 << 30,
        }
    }

    pub fn as_char(self) -> char {
        match self {
            Quantifier::K => 'K',
            Quantifier::M => 'M',
            Quantifier::G => 'G',
        }
    }
}

/// An identifier as written in the source. Sieve identifiers are
/// case-insensitive; use [`Identifier::is`] to compare.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Identifier {
    pub name: String,
    pub span: Span,
}

impl Identifier {
    pub fn is(&self, name: &str) -> bool {
        self.name.eq_ignore_ascii_case(name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Comment {
    /// Text without the `#` / `/* */` delimiters and without the line ending.
    pub text: String,
    pub kind: CommentKind,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CommentKind {
    /// `# …` up to the end of the line.
    Hash,
    /// `/* … */`
    Bracket,
}

/// Resets every span in a tree to [`Span::default`], so that trees parsed from
/// differently formatted sources can be compared structurally.
pub trait ClearSpans {
    fn clear_spans(&mut self);
}

impl ClearSpans for Script {
    fn clear_spans(&mut self) {
        self.commands.iter_mut().for_each(ClearSpans::clear_spans);
        self.trailing_comments.iter_mut().for_each(ClearSpans::clear_spans);
    }
}

impl ClearSpans for Command {
    fn clear_spans(&mut self) {
        self.span = Span::default();
        self.name.clear_spans();
        self.arguments.clear_spans();
        if let Some(block) = &mut self.block {
            block.clear_spans();
        }
        self.leading_comments.iter_mut().for_each(ClearSpans::clear_spans);
        self.inner_comments.iter_mut().for_each(ClearSpans::clear_spans);
        if let Some(c) = &mut self.trailing_comment {
            c.clear_spans();
        }
    }
}

impl ClearSpans for Block {
    fn clear_spans(&mut self) {
        self.span = Span::default();
        self.commands.iter_mut().for_each(ClearSpans::clear_spans);
        self.trailing_comments.iter_mut().for_each(ClearSpans::clear_spans);
    }
}

impl ClearSpans for Test {
    fn clear_spans(&mut self) {
        self.span = Span::default();
        self.name.clear_spans();
        self.arguments.clear_spans();
    }
}

impl ClearSpans for Arguments {
    fn clear_spans(&mut self) {
        for arg in &mut self.args {
            match arg {
                Argument::String(s) => s.span = Span::default(),
                Argument::StringList(l) => {
                    l.span = Span::default();
                    l.items.iter_mut().for_each(|s| s.span = Span::default());
                }
                Argument::Number(n) => n.span = Span::default(),
                Argument::Tag(t) => t.clear_spans(),
            }
        }
        match &mut self.tests {
            Some(Tests::Single(t)) => t.clear_spans(),
            Some(Tests::List(l)) => {
                l.span = Span::default();
                l.tests.iter_mut().for_each(ClearSpans::clear_spans);
            }
            None => {}
        }
    }
}

impl ClearSpans for Identifier {
    fn clear_spans(&mut self) {
        self.span = Span::default();
    }
}

impl ClearSpans for Comment {
    fn clear_spans(&mut self) {
        self.span = Span::default();
    }
}
