//! Turns the rule model back into Sieve text.

use crate::ast::*;
use crate::metadata::format_rule_name;
use crate::model::*;
use crate::printer::{print_command, print_commands, print_test, PrintOptions};
use crate::root::FOLDER_VARIABLE;

/// What [`ConditionTest::Compare`] captures with and compares against.
pub(crate) const COMPARE_PATTERN: &str = "*";
pub(crate) const COMPARE_KEY: &str = "${1}";
use crate::span::Span;

/// What Roundcube writes for a script without rules.
pub const EMPTY_SCRIPT: &str = "/* empty script */";

/// Generates the script for a rule set.
pub fn export(set: &RuleSet, options: &PrintOptions) -> String {
    let eol = options.eol();
    if set.items.is_empty() && set.preamble.is_none() {
        return EMPTY_SCRIPT.to_owned();
    }
    let mut out = String::new();

    if let Some(preamble) = &set.preamble {
        out.push_str(preamble.trim_end_matches(['\r', '\n']));
        out.push_str(eol);
        out.push_str(eol);
    }

    let requires = requires(set);
    if !requires.is_empty() {
        let cmd = command("require", vec![list(requires)], None);
        out.push_str(&print_command(&cmd, 0, options));
    }
    if set.single_folder {
        out.push_str(&print_command(&command("global", vec![string(FOLDER_VARIABLE)], None), 0, options));
    }

    for item in &set.items {
        let text = match item {
            RuleItem::Rule(rule) => print_commands(&rule_commands(rule, set.single_folder, options), 0, options),
            RuleItem::Raw(raw) => match raw.code.trim_end_matches(['\r', '\n']) {
                "" => continue,
                code => format!("{code}{eol}"),
            },
        };
        if options.separate_rules && !out.is_empty() {
            out.push_str(eol);
        }
        out.push_str(&text);
    }
    out
}

/// The `require` list: the original entries (dropping known extensions that
/// are no longer used, unless raw blocks might use them) plus new ones.
fn requires(set: &RuleSet) -> Vec<String> {
    let needed = set.needed_extensions();
    let has_raw = set.items.iter().any(|i| matches!(i, RuleItem::Raw(_)));
    let mut out: Vec<String> = set
        .requires
        .iter()
        .filter(|r| {
            has_raw
                || needed.iter().any(|n| n.eq_ignore_ascii_case(r))
                || !KNOWN_EXTENSIONS.iter().any(|k| k.eq_ignore_ascii_case(r))
        })
        .cloned()
        .collect();
    for n in needed {
        if !out.iter().any(|r| r.eq_ignore_ascii_case(n)) {
            out.push(n.to_owned());
        }
    }
    out.sort_by_key(|r| r.to_ascii_lowercase());
    out
}

/// The `if` command for one rule, in the Roundcube form:
///
/// ```sieve
/// # rule:[Name]
/// if allof (header :contains "from" "x")
/// {
///     fileinto "X";
/// }
/// ```
///
/// A disabled rule becomes `if false # <test>`. In single-folder mode, plain
/// moves become `set` on the folder variable and "stop" becomes `return`.
pub fn rule_command(rule: &Rule, single_folder: bool, options: &PrintOptions) -> Command {
    let actions = action_commands(&rule.actions, single_folder);
    let test = conditions_test(&rule.conditions);
    let (test, inner_comments) = if rule.enabled {
        (test, Vec::new())
    } else {
        // The test lives in a line comment, so it must fit on one line. Line
        // breaks can only come from string values; those are rare enough in
        // tests to accept the lossy replacement.
        let text = print_test(&test, options).replace(['\r', '\n'], " ");
        (bare_test("false"), vec![hash_comment(format!(" {text}"))])
    };
    let mut cmd = command("if", Vec::new(), Some(Tests::Single(Box::new(test))));
    if !rule.name.trim().is_empty() {
        cmd.leading_comments.push(hash_comment(format_rule_name(rule.name.trim())));
    }
    cmd.inner_comments = inner_comments;
    cmd.block = Some(Block { commands: actions, trailing_comments: Vec::new(), span: Span::default() });
    cmd
}

fn conditions_test(conditions: &Conditions) -> Test {
    match (conditions.items.as_slice(), conditions.join) {
        ([], _) => bare_test("true"),
        ([single], Join::Any) => condition_test(single),
        (items, join) => {
            let name = match join {
                Join::All => "allof",
                Join::Any => "anyof",
            };
            let tests = items.iter().map(condition_test).collect();
            test(name, Vec::new(), Some(Tests::List(TestList { tests, span: Span::default() })))
        }
    }
}

/// A nested group keeps its `allof`/`anyof` even with a single test, so it
/// reads back as a group.
fn group_test(join: Join, items: &[Condition]) -> Test {
    let name = match (join, items.is_empty()) {
        (Join::All, true) => return bare_test("true"),
        (Join::Any, true) => return bare_test("false"),
        (Join::All, false) => "allof",
        (Join::Any, false) => "anyof",
    };
    let tests = items.iter().map(condition_test).collect();
    test(name, Vec::new(), Some(Tests::List(TestList { tests, span: Span::default() })))
}

fn condition_test(condition: &Condition) -> Test {
    let inner = basic_test(&condition.test);
    if condition.negate {
        test("not", Vec::new(), Some(Tests::Single(Box::new(inner))))
    } else {
        inner
    }
}

fn basic_test(t: &ConditionTest) -> Test {
    let mut args = Vec::new();
    let name = match t {
        ConditionTest::Group { join, items } => return group_test(*join, items),
        ConditionTest::Compare { headers, part, other, differs } => return compare_test(headers, *part, other, *differs),
        ConditionTest::String { sources, matcher, keys } => {
            push_matcher(&mut args, matcher);
            args.push(string_or_list(sources.clone()));
            args.push(string_or_list(keys.clone()));
            "string"
        }
        ConditionTest::Header { headers, matcher, keys } => {
            push_matcher(&mut args, matcher);
            args.push(string_or_list(headers.clone()));
            args.push(string_or_list(keys.clone()));
            "header"
        }
        ConditionTest::Address { headers, part, matcher, keys } => {
            push_address_part(&mut args, *part);
            push_matcher(&mut args, matcher);
            args.push(string_or_list(headers.clone()));
            args.push(string_or_list(keys.clone()));
            "address"
        }
        ConditionTest::Envelope { parts, part, matcher, keys } => {
            push_address_part(&mut args, *part);
            push_matcher(&mut args, matcher);
            args.push(string_or_list(parts.clone()));
            args.push(string_or_list(keys.clone()));
            "envelope"
        }
        ConditionTest::Exists { headers } => {
            args.push(string_or_list(headers.clone()));
            "exists"
        }
        ConditionTest::Size { over, limit } => {
            args.push(tag(if *over { "over" } else { "under" }));
            args.push(number(*limit));
            "size"
        }
        ConditionTest::Body { transform, matcher, keys } => {
            match transform {
                BodyTransform::Text => args.push(tag("text")),
                BodyTransform::Raw => args.push(tag("raw")),
                BodyTransform::Content(types) => {
                    args.push(tag("content"));
                    args.push(string_or_list(types.clone()));
                }
            }
            push_matcher(&mut args, matcher);
            args.push(string_or_list(keys.clone()));
            "body"
        }
        ConditionTest::CurrentDate { zone, matcher, date_part, keys } => {
            if let Some(zone) = zone {
                args.push(tag("zone"));
                args.push(string(zone));
            }
            push_matcher(&mut args, matcher);
            args.push(string(date_part));
            args.push(string_or_list(keys.clone()));
            "currentdate"
        }
        ConditionTest::Date { zone, matcher, header, date_part, keys } => {
            match zone {
                DateZone::Local => {}
                DateZone::Zone(z) => {
                    args.push(tag("zone"));
                    args.push(string(z));
                }
                DateZone::Original => args.push(tag("originalzone")),
            }
            push_matcher(&mut args, matcher);
            args.push(string(header));
            args.push(string(date_part));
            args.push(string_or_list(keys.clone()));
            "date"
        }
    };
    test(name, args, None)
}

/// The capture of the first value of `headers` and the test of `other`
/// against it, see [`ConditionTest::Compare`].
fn compare_test(headers: &[String], part: Option<AddressPart>, other: &[String], differs: bool) -> Test {
    let side = |fields: &[String], match_type: MatchType, key: &str| {
        let mut args = Vec::new();
        if let Some(part) = part {
            push_address_part(&mut args, part);
        }
        push_matcher(&mut args, &Matcher::new(match_type));
        args.push(string_or_list(fields.to_vec()));
        args.push(string(key));
        test(if part.is_some() { "address" } else { "header" }, args, None)
    };
    let capture = side(headers, MatchType::Matches, COMPARE_PATTERN);
    let mut compare = side(other, MatchType::Is, COMPARE_KEY);
    if differs {
        compare = test("not", Vec::new(), Some(Tests::Single(Box::new(compare))));
    }
    test("allof", Vec::new(), Some(Tests::List(TestList { tests: vec![capture, compare], span: Span::default() })))
}

/// Match type first, then comparator (`:value "gt" :comparator "…"`).
fn push_matcher(args: &mut Vec<Argument>, matcher: &Matcher) {
    match matcher.match_type {
        MatchType::Is => args.push(tag("is")),
        MatchType::Contains => args.push(tag("contains")),
        MatchType::Matches => args.push(tag("matches")),
        MatchType::Regex => args.push(tag("regex")),
        MatchType::Value(op) => {
            args.push(tag("value"));
            args.push(string(op.as_str()));
        }
        MatchType::Count(op) => {
            args.push(tag("count"));
            args.push(string(op.as_str()));
        }
    }
    if let Some(c) = &matcher.comparator {
        args.push(tag("comparator"));
        args.push(string(c));
    }
}

fn push_address_part(args: &mut Vec<Argument>, part: AddressPart) {
    let name = match part {
        AddressPart::All => return,
        AddressPart::LocalPart => "localpart",
        AddressPart::Domain => "domain",
        AddressPart::User => "user",
        AddressPart::Detail => "detail",
    };
    args.push(tag(name));
}

/// The commands for a rule: [`rule_command`], or for a rule with branches
/// its `if`/`elsif`/`else` chain. A disabled rule with branches is wrapped
/// in `if false { … }`: the `if false # <test>` form would let the `elsif`
/// branches run.
pub fn rule_commands(rule: &Rule, single_folder: bool, options: &PrintOptions) -> Vec<Command> {
    if !rule.has_branches() {
        return vec![rule_command(rule, single_folder, options)];
    }
    let chain = if_chain(&rule.conditions, &rule.actions, &rule.elsif, &rule.otherwise, single_folder);
    let mut commands = if rule.enabled {
        chain
    } else {
        let mut wrapper = command("if", Vec::new(), Some(Tests::Single(Box::new(bare_test("false")))));
        wrapper.block = Some(Block { commands: chain, trailing_comments: Vec::new(), span: Span::default() });
        vec![wrapper]
    };
    if !rule.name.trim().is_empty() {
        commands[0].leading_comments.push(hash_comment(format_rule_name(rule.name.trim())));
    }
    commands
}

/// `if <conditions> { … } elsif … { … } else { … }`
fn if_chain(
    conditions: &Conditions,
    actions: &[Action],
    elsif: &[Branch],
    otherwise: &Option<Vec<Action>>,
    single_folder: bool,
) -> Vec<Command> {
    let branch = |name: &str, conditions: Option<&Conditions>, actions: &[Action]| {
        let tests = conditions.map(|c| Tests::Single(Box::new(conditions_test(c))));
        let mut cmd = command(name, Vec::new(), tests);
        cmd.block =
            Some(Block { commands: action_commands(actions, single_folder), trailing_comments: Vec::new(), span: Span::default() });
        cmd
    };
    let mut out = vec![branch("if", Some(conditions), actions)];
    out.extend(elsif.iter().map(|b| branch("elsif", Some(&b.conditions), &b.actions)));
    if let Some(actions) = otherwise {
        out.push(branch("else", None, actions));
    }
    out
}

/// The commands for actions; a nested `if` becomes its chain.
fn action_commands(actions: &[Action], single_folder: bool) -> Vec<Command> {
    actions
        .iter()
        .flat_map(|a| match a {
            Action::If(n) => if_chain(&n.conditions, &n.actions, &n.elsif, &n.otherwise, single_folder),
            a => vec![action_command(a, single_folder)],
        })
        .collect()
}

fn action_command(action: &Action, single_folder: bool) -> Command {
    if single_folder {
        match action {
            Action::FileInto { mailbox, .. } if action.is_plain_move() => {
                return command("set", vec![string(FOLDER_VARIABLE), string(mailbox)], None);
            }
            Action::Stop => return command("return", Vec::new(), None),
            _ => {}
        }
    }
    let mut args = Vec::new();
    let push_flags = |args: &mut Vec<Argument>, flags: &Option<Vec<String>>| {
        if let Some(flags) = flags {
            args.push(tag("flags"));
            args.push(string_or_list(flags.clone()));
        }
    };
    let name = match action {
        Action::FileInto { mailbox, copy, create, flags } => {
            if *copy {
                args.push(tag("copy"));
            }
            if *create {
                args.push(tag("create"));
            }
            push_flags(&mut args, flags);
            args.push(string(mailbox));
            "fileinto"
        }
        Action::Redirect { address, copy } => {
            if *copy {
                args.push(tag("copy"));
            }
            args.push(string(address));
            "redirect"
        }
        Action::Reject { reason, extended } => {
            args.push(text(reason));
            if *extended { "ereject" } else { "reject" }
        }
        Action::Discard => "discard",
        Action::Keep { flags } => {
            push_flags(&mut args, flags);
            "keep"
        }
        Action::Stop => "stop",
        Action::Flags { op, flags } => {
            args.push(string_or_list(flags.clone()));
            op.command()
        }
        Action::Set { name, value, modifiers } => {
            args.extend(modifiers.iter().map(|m| tag(m.tag())));
            args.push(string(name));
            args.push(text(value));
            "set"
        }
        Action::Vacation(v) => {
            match v.period {
                Some(VacationPeriod::Days(d)) => {
                    args.push(tag("days"));
                    args.push(number(d));
                }
                Some(VacationPeriod::Seconds(s)) => {
                    args.push(tag("seconds"));
                    args.push(number(s));
                }
                None => {}
            }
            if !v.addresses.is_empty() {
                args.push(tag("addresses"));
                args.push(list(v.addresses.clone()));
            }
            if let Some(s) = &v.subject {
                args.push(tag("subject"));
                args.push(string(s));
            }
            if let Some(f) = &v.from {
                args.push(tag("from"));
                args.push(string(f));
            }
            if let Some(h) = &v.handle {
                args.push(tag("handle"));
                args.push(string(h));
            }
            if v.mime {
                args.push(tag("mime"));
            }
            args.push(text(&v.reason));
            "vacation"
        }
        Action::If(_) => unreachable!("nested ifs are written by action_commands"),
    };
    command(name, args, None)
}

// ── AST construction helpers ──────────────────────────────────────────────────

fn ident(name: &str) -> Identifier {
    Identifier { name: name.to_owned(), span: Span::default() }
}

pub(crate) fn command(name: &str, args: Vec<Argument>, tests: Option<Tests>) -> Command {
    Command {
        name: ident(name),
        arguments: Arguments { args, tests },
        block: None,
        leading_comments: Vec::new(),
        inner_comments: Vec::new(),
        trailing_comment: None,
        blank_line_before: false,
        span: Span::default(),
    }
}

pub(crate) fn test(name: &str, args: Vec<Argument>, tests: Option<Tests>) -> Test {
    Test { name: ident(name), arguments: Arguments { args, tests }, span: Span::default() }
}

fn bare_test(name: &str) -> Test {
    test(name, Vec::new(), None)
}

pub(crate) fn hash_comment(text: String) -> Comment {
    Comment { text, kind: CommentKind::Hash, span: Span::default() }
}

fn sieve_string(value: &str, kind: StringKind) -> SieveString {
    SieveString { value: value.to_owned(), kind, span: Span::default() }
}

pub(crate) fn string(value: &str) -> Argument {
    Argument::String(sieve_string(value, StringKind::Quoted))
}

/// A string that uses `text:` syntax when it spans several lines.
fn text(value: &str) -> Argument {
    let kind = if value.contains('\n') && value.ends_with('\n') {
        StringKind::MultiLine
    } else {
        StringKind::Quoted
    };
    Argument::String(sieve_string(value, kind))
}

pub(crate) fn list(values: Vec<String>) -> Argument {
    Argument::StringList(StringList {
        items: values.iter().map(|v| sieve_string(v, StringKind::Quoted)).collect(),
        span: Span::default(),
    })
}

fn string_or_list(values: Vec<String>) -> Argument {
    match values.as_slice() {
        [single] => string(single),
        _ => list(values),
    }
}

fn number(value: u64) -> Argument {
    let quantifier = [Quantifier::G, Quantifier::M, Quantifier::K]
        .into_iter()
        .find(|q| value != 0 && value.is_multiple_of(q.multiplier()));
    Argument::Number(Number { value, quantifier, span: Span::default() })
}

pub(crate) fn tag(name: &str) -> Argument {
    Argument::Tag(ident(name))
}
