//! Deterministic pretty-printer for the syntax tree.
//!
//! Output is canonical: the same tree always prints the same text, and
//! printing a parsed script and parsing it again yields the same tree (modulo
//! spans, and modulo where comments inside a command header end up).

use crate::ast::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrintOptions {
    /// One level of indentation.
    pub indent: String,
    pub line_ending: LineEnding,
    /// Test lists longer than this are broken into one test per line.
    pub max_width: usize,
    pub brace_style: BraceStyle,
    /// Write string lists as `["a","b"]` instead of `["a", "b"]`.
    pub compact_string_lists: bool,
}

impl Default for PrintOptions {
    fn default() -> Self {
        Self {
            indent: "\t".into(),
            line_ending: LineEnding::Lf,
            max_width: 100,
            brace_style: BraceStyle::SameLine,
            compact_string_lists: false,
        }
    }
}

impl PrintOptions {
    /// The layout written by the Roundcube *managesieve* plugin, so that
    /// scripts edited alternately in both tools produce small diffs.
    pub fn roundcube() -> Self {
        Self {
            indent: "\t".into(),
            line_ending: LineEnding::CrLf,
            max_width: usize::MAX,
            brace_style: BraceStyle::NextLine,
            compact_string_lists: true,
        }
    }

    pub(crate) fn eol(&self) -> &'static str {
        self.line_ending.as_str()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BraceStyle {
    /// `if test {` and `} elsif test {`
    SameLine,
    /// `{` and `}` on lines of their own.
    NextLine,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEnding {
    Lf,
    CrLf,
}

impl LineEnding {
    fn as_str(self) -> &'static str {
        match self {
            LineEnding::Lf => "\n",
            LineEnding::CrLf => "\r\n",
        }
    }
}

/// Prints a whole script. Non-empty output always ends with a line ending.
pub fn print_script(script: &Script, options: &PrintOptions) -> String {
    let mut p = Printer { out: String::new(), options };
    p.commands(&script.commands, 0);
    p.comments(&script.trailing_comments, 0);
    p.out
}

/// Prints a single command (including its comments) at indentation `depth`.
pub fn print_command(command: &Command, depth: usize, options: &PrintOptions) -> String {
    let mut p = Printer { out: String::new(), options };
    p.commands(std::slice::from_ref(command), depth);
    p.out
}

/// Prints a test on one line, e.g. for showing it in a UI or comment.
pub fn print_test(test: &Test, options: &PrintOptions) -> String {
    let mut out = String::new();
    inline_test(&mut out, test, options.compact_string_lists);
    out
}

struct Printer<'a> {
    out: String,
    options: &'a PrintOptions,
}

impl Printer<'_> {
    fn nl(&mut self) {
        self.out.push_str(self.options.line_ending.as_str());
    }

    fn indent(&mut self, depth: usize) {
        for _ in 0..depth {
            self.out.push_str(&self.options.indent);
        }
    }

    fn commands(&mut self, commands: &[Command], depth: usize) {
        for (i, cmd) in commands.iter().enumerate() {
            let prev = i.checked_sub(1).map(|j| &commands[j]);
            let chained = self.options.brace_style == BraceStyle::SameLine
                && prev.is_some_and(|p| p.block.is_some() && p.trailing_comment.is_none())
                && (cmd.is("elsif") || cmd.is("else"))
                && cmd.leading_comments.is_empty()
                && !cmd.blank_line_before;
            if chained {
                // Join `} elsif … {` on one line: drop the line ending after `}`.
                let eol = self.options.line_ending.as_str();
                self.out.truncate(self.out.len() - eol.len());
                self.out.push(' ');
            } else {
                if prev.is_some() && cmd.blank_line_before {
                    self.nl();
                }
                self.comments(&cmd.leading_comments, depth);
                self.indent(depth);
            }
            self.command(cmd, depth);
        }
    }

    fn command(&mut self, cmd: &Command, depth: usize) {
        self.out.push_str(&cmd.name.name);
        self.arguments(&cmd.arguments, depth);

        // Comments inside the header stay before the terminator so that they
        // are inner comments again when the output is parsed.
        let ends_line = self.inline_comments(&cmd.inner_comments, depth);
        let brace_on_new_line = cmd.block.is_some() && self.options.brace_style == BraceStyle::NextLine;
        if ends_line || brace_on_new_line {
            self.nl();
            self.indent(depth);
        } else if cmd.block.is_some() {
            self.out.push(' ');
        }
        match &cmd.block {
            None => self.out.push(';'),
            Some(block) => {
                self.out.push('{');
                self.nl();
                self.commands(&block.commands, depth + 1);
                self.comments(&block.trailing_comments, depth + 1);
                self.indent(depth);
                self.out.push('}');
            }
        }
        if let Some(c) = &cmd.trailing_comment {
            self.inline_comments(std::slice::from_ref(c), depth);
        }
        self.nl();
    }

    /// Writes comments after code on the current line. A hash comment ends the
    /// line, so any further comment starts on its own line. Returns whether the
    /// output now needs a line break before more code.
    fn inline_comments(&mut self, comments: &[Comment], depth: usize) -> bool {
        let mut ends_line = false;
        for c in comments {
            if ends_line {
                self.nl();
                self.indent(depth);
            } else {
                self.out.push(' ');
            }
            ends_line = write_comment(&mut self.out, c);
        }
        ends_line
    }

    /// Writes comments each on its own line.
    fn comments(&mut self, comments: &[Comment], depth: usize) {
        for c in comments {
            self.indent(depth);
            write_comment(&mut self.out, c);
            self.nl();
        }
    }

    fn arguments(&mut self, arguments: &Arguments, depth: usize) {
        for arg in &arguments.args {
            self.out.push(' ');
            write_argument(&mut self.out, arg, self.options.compact_string_lists);
        }
        match &arguments.tests {
            None => {}
            Some(Tests::Single(test)) => {
                self.out.push(' ');
                self.test(test, depth);
            }
            Some(Tests::List(list)) => {
                self.out.push(' ');
                self.test_list(list, depth);
            }
        }
    }

    fn test(&mut self, test: &Test, depth: usize) {
        self.out.push_str(&test.name.name);
        self.arguments(&test.arguments, depth);
    }

    fn test_list(&mut self, list: &TestList, depth: usize) {
        let mut inline = String::new();
        inline_test_list(&mut inline, list, self.options.compact_string_lists);
        let line_start = self.out.rfind('\n').map_or(0, |i| i + 1);
        let width = self.out[line_start..].chars().count() + inline.chars().count();
        if width <= self.options.max_width || inline.contains('\n') || list.tests.len() < 2 {
            self.out.push_str(&inline);
            return;
        }
        self.out.push('(');
        self.nl();
        for (i, test) in list.tests.iter().enumerate() {
            self.indent(depth + 1);
            self.test(test, depth + 1);
            if i + 1 < list.tests.len() {
                self.out.push(',');
            }
            self.nl();
        }
        self.indent(depth);
        self.out.push(')');
    }
}

/// Returns whether the comment ends the line.
fn write_comment(out: &mut String, c: &Comment) -> bool {
    match c.kind {
        CommentKind::Hash => {
            out.push('#');
            out.push_str(&c.text);
            true
        }
        CommentKind::Bracket => {
            out.push_str("/*");
            out.push_str(&c.text);
            out.push_str("*/");
            false
        }
    }
}

fn inline_arguments(out: &mut String, arguments: &Arguments, compact: bool) {
    for arg in &arguments.args {
        out.push(' ');
        write_argument(out, arg, compact);
    }
    match &arguments.tests {
        None => {}
        Some(Tests::Single(test)) => {
            out.push(' ');
            inline_test(out, test, compact);
        }
        Some(Tests::List(list)) => {
            out.push(' ');
            inline_test_list(out, list, compact);
        }
    }
}

fn inline_test(out: &mut String, test: &Test, compact: bool) {
    out.push_str(&test.name.name);
    inline_arguments(out, &test.arguments, compact);
}

fn inline_test_list(out: &mut String, list: &TestList, compact: bool) {
    out.push('(');
    for (i, test) in list.tests.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        inline_test(out, test, compact);
    }
    out.push(')');
}

fn write_argument(out: &mut String, arg: &Argument, compact: bool) {
    match arg {
        Argument::String(s) => write_string(out, s),
        Argument::StringList(list) => {
            out.push('[');
            for (i, s) in list.items.iter().enumerate() {
                if i > 0 {
                    out.push_str(if compact { "," } else { ", " });
                }
                write_string(out, s);
            }
            out.push(']');
        }
        Argument::Number(n) => match n.quantifier {
            Some(q) if n.value.is_multiple_of(q.multiplier()) => {
                out.push_str(&(n.value / q.multiplier()).to_string());
                out.push(q.as_char());
            }
            _ => out.push_str(&n.value.to_string()),
        },
        Argument::Tag(t) => {
            out.push(':');
            out.push_str(&t.name);
        }
    }
}

/// Writes a string literal. Multi-line strings stay multi-line as long as the
/// value can be represented that way (it must end with a line break).
pub(crate) fn write_string(out: &mut String, s: &SieveString) {
    if s.kind == StringKind::MultiLine && (s.value.is_empty() || s.value.ends_with('\n')) {
        let eol = if s.value.ends_with("\r\n") { "\r\n" } else { "\n" };
        out.push_str("text:");
        out.push_str(eol);
        for line in s.value.split_inclusive('\n') {
            if line.starts_with('.') {
                out.push('.');
            }
            out.push_str(line);
        }
        out.push('.');
        out.push_str(eol);
    } else {
        out.push('"');
        for c in s.value.chars() {
            if c == '"' || c == '\\' {
                out.push('\\');
            }
            out.push(c);
        }
        out.push('"');
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;

    fn fmt(src: &str) -> String {
        print_script(&parse(src).unwrap(), &PrintOptions { indent: "  ".into(), ..Default::default() })
    }

    #[test]
    fn canonical_layout() {
        let src = r#"require ["fileinto","imap4flags"] ;
# rule:[Taxes]
if   header :contains "from" "@tax-office.example" { fileinto "Taxes" ; stop ; }
elsif true {addflag "\\Seen";}
else{keep;}"#;
        assert_eq!(
            fmt(src),
            r#"require ["fileinto", "imap4flags"];
# rule:[Taxes]
if header :contains "from" "@tax-office.example" {
  fileinto "Taxes";
  stop;
} elsif true {
  addflag "\\Seen";
} else {
  keep;
}
"#
        );
    }

    #[test]
    fn comments_are_kept() {
        let src = "# top\nif false # header :is \"to\" \"x\"\n{\n  keep; # same line\n  # block end\n}\n\n/* tail */\n";
        assert_eq!(
            fmt(src),
            "# top\nif false # header :is \"to\" \"x\"\n{\n  keep; # same line\n  # block end\n}\n/* tail */\n"
        );
        assert_eq!(fmt("keep /* inline */ ;"), "keep /* inline */;\n");
        assert_eq!(fmt("keep # a\n # b\n;"), "keep # a\n# b\n;\n");
    }

    #[test]
    fn blank_lines_are_kept() {
        assert_eq!(fmt("keep;\n\n\nstop;\nstop;"), "keep;\n\nstop;\nstop;\n");
    }

    #[test]
    fn long_test_lists_break() {
        let src = r#"if anyof (header :contains "from" "first-long-address@example.com", header :contains "from" "second-long-address@example.com", header :contains "to" "x") { stop; }"#;
        assert_eq!(
            fmt(src),
            r#"if anyof (
  header :contains "from" "first-long-address@example.com",
  header :contains "from" "second-long-address@example.com",
  header :contains "to" "x"
) {
  stop;
}
"#
        );
    }

    #[test]
    fn strings_and_numbers() {
        assert_eq!(fmt(r#"x "a\"b\\c" 100K 1023 :tag;"#), "x \"a\\\"b\\\\c\" 100K 1023 :tag;\n");
        assert_eq!(
            fmt("vacation :days 7 text:\nHello\n..dots\n.\n;"),
            "vacation :days 7 text:\nHello\n..dots\n.\n;\n"
        );
    }

    #[test]
    fn roundcube_layout() {
        let s = parse("if anyof (true, header :is [\"a\", \"b\"] \"c\") { keep; } else { stop; }").unwrap();
        assert_eq!(
            print_script(&s, &PrintOptions::roundcube()),
            "if anyof (true, header :is [\"a\",\"b\"] \"c\")\r\n{\r\n\tkeep;\r\n}\r\nelse\r\n{\r\n\tstop;\r\n}\r\n"
        );
    }

    #[test]
    fn crlf_output() {
        let s = parse("if true { keep; }").unwrap();
        let out = print_script(&s, &PrintOptions { line_ending: LineEnding::CrLf, ..Default::default() });
        assert_eq!(out, "if true {\r\n\tkeep;\r\n}\r\n");
    }
}
