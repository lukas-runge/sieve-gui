//! Maps a parsed script onto the rule model.
//!
//! The mapping is conservative: a top-level construct becomes a [`Rule`] only
//! if every part of it is understood, including its comments. Everything else
//! becomes a [`RawBlock`] holding the original source text, so importing and
//! exporting never drops anything.

use crate::ast::{Argument, Command, CommentKind, Script, Test, Tests};
use crate::metadata::parse_rule_name;
use crate::model::*;
use crate::parser::parse;
use crate::root::FOLDER_VARIABLE;
use crate::span::{ParseError, Span};

/// Parses `src` and maps it onto the rule model.
pub fn import(src: &str) -> Result<RuleSet, ParseError> {
    let script = parse(src)?;
    Ok(import_script(&script, src))
}

/// Maps an already parsed script; `src` must be the text it was parsed from.
pub fn import_script(script: &Script, src: &str) -> RuleSet {
    let mut set = RuleSet::default();
    let commands = &script.commands;
    let mut i = 0;

    if commands.is_empty() && is_empty_script_marker(script) {
        return set;
    }

    // A comment block at the very top is the preamble. A rule name directly
    // before the first rule stays with that rule.
    let mut first_leading = 0;
    if let Some(first) = commands.first() {
        let comments = &first.leading_comments;
        let keep = match comments.last() {
            Some(c) if !first.is("require") && parse_rule_name(c).is_some() => 1,
            _ => 0,
        };
        first_leading = comments.len() - keep;
        if first_leading > 0 {
            let span = comments[0].span.to(comments[first_leading - 1].span);
            set.preamble = Some(src[span.start..span.end].to_owned());
        }
    }

    // Leading `require` commands without comments become the require list.
    while let Some(cmd) = commands.get(i) {
        let leading = if i == 0 { &cmd.leading_comments[first_leading..] } else { &cmd.leading_comments[..] };
        if !cmd.is("require") || !leading.is_empty() || !cmd.inner_comments.is_empty() || cmd.trailing_comment.is_some() {
            break;
        }
        let Some(exts) = single_string_list(&cmd.arguments) else { break };
        for ext in exts {
            if !set.requires.iter().any(|r| r.eq_ignore_ascii_case(&ext)) {
                set.requires.push(ext);
            }
        }
        i += 1;
    }

    // `global "<folder variable>";` right after the requires: single-folder mode.
    if let Some(cmd) = commands.get(i) {
        let leading = if i == 0 { &cmd.leading_comments[first_leading..] } else { &cmd.leading_comments[..] };
        let variables = single_string_list(&cmd.arguments);
        let folder = matches!(variables.as_deref(), Some([v]) if v.eq_ignore_ascii_case(FOLDER_VARIABLE));
        let comments = !leading.is_empty() || !cmd.inner_comments.is_empty() || cmd.trailing_comment.is_some();
        if cmd.is("global") && cmd.block.is_none() && folder && !comments {
            set.single_folder = true;
            i += 1;
        }
    }
    let single_folder = set.single_folder;

    while i < commands.len() {
        let cmd = &commands[i];
        if cmd.is("if") {
            // An if/elsif/else chain is one unit.
            let mut end = i + 1;
            while end < commands.len() && (commands[end].is("elsif") || commands[end].is("else")) {
                end += 1;
            }
            let skip = if i == 0 { first_leading } else { 0 };
            let rule = (end == i + 1).then(|| rule_from_if(cmd, skip, single_folder)).flatten();
            push(&mut set, rule, &commands[i..end], skip, src);
            i = end;
        } else if let Some(actions) = action_run(&commands[i..], single_folder) {
            // Actions outside of `if` form an "all messages" rule. A run ends
            // at the next command with comments (such as a rule name).
            let skip = if i == 0 { first_leading } else { 0 };
            let name = match &commands[i].leading_comments[skip..] {
                [] => Some(String::new()),
                [c] => parse_rule_name(c),
                _ => None,
            };
            let mut end = i + actions.0;
            // The rule is written as `if true { … }`, so a following (orphan)
            // elsif/else would attach to it. Keep such code raw.
            let orphan = |c: &Command| c.is("elsif") || c.is("else");
            let rule = if commands.get(end).is_some_and(orphan) {
                while commands.get(end).is_some_and(orphan) {
                    end += 1;
                }
                None
            } else {
                name.map(|name| Rule { name, enabled: true, conditions: Conditions::default(), actions: actions.1 })
            };
            push(&mut set, rule, &commands[i..end], skip, src);
            i = end;
        } else {
            let skip = if i == 0 { first_leading } else { 0 };
            push(&mut set, None, &commands[i..=i], skip, src);
            i += 1;
        }
    }

    if !script.trailing_comments.is_empty() {
        let first = script.trailing_comments.first().map(|c| c.span).unwrap_or_default();
        let last = script.trailing_comments.last().map(|c| c.span).unwrap_or_default();
        set.items.push(RuleItem::Raw(RawBlock { code: src[first.to(last).start..last.end].to_owned() }));
    }
    set
}

/// Adds a rule, or the commands as a raw block. `skip` leading comments of
/// the first command belong to the preamble.
fn push(set: &mut RuleSet, rule: Option<Rule>, commands: &[Command], skip: usize, src: &str) {
    let item = match rule {
        Some(rule) => RuleItem::Rule(rule),
        None => {
            let first = &commands[0];
            let start = first.leading_comments.get(skip).map_or(first.span.start, |c| c.span.start);
            let end = commands.iter().map(Command::full_span).reduce(Span::to).expect("at least one command").end;
            RuleItem::Raw(RawBlock { code: src[start..end].to_owned() })
        }
    };
    set.items.push(item);
}

fn is_empty_script_marker(script: &Script) -> bool {
    matches!(script.trailing_comments.as_slice(),
        [c] if c.kind == CommentKind::Bracket && c.text.trim() == "empty script")
}

fn has_comments(cmd: &Command) -> bool {
    !cmd.leading_comments.is_empty() || !cmd.inner_comments.is_empty() || cmd.trailing_comment.is_some()
}

/// The longest run of plain action commands at the start of `commands`, as
/// (length, actions). Only the first command may carry (name) comments.
fn action_run(commands: &[Command], single_folder: bool) -> Option<(usize, Vec<Action>)> {
    let mut actions = Vec::new();
    for (n, cmd) in commands.iter().enumerate() {
        let comments_ok = if n == 0 {
            cmd.inner_comments.is_empty() && cmd.trailing_comment.is_none()
        } else {
            !has_comments(cmd)
        };
        let action = comments_ok.then(|| action(cmd, single_folder)).flatten();
        match action {
            Some(a) => actions.push(a),
            None => break,
        }
    }
    (!actions.is_empty()).then_some((actions.len(), actions))
}

fn rule_from_if(cmd: &Command, skip_comments: usize, single_folder: bool) -> Option<Rule> {
    if cmd.trailing_comment.is_some() || !cmd.arguments.args.is_empty() {
        return None;
    }
    let name = match &cmd.leading_comments[skip_comments..] {
        [] => String::new(),
        [c] => parse_rule_name(c)?,
        _ => return None,
    };
    let Some(Tests::Single(test)) = &cmd.arguments.tests else { return None };

    let (enabled, conditions) = match cmd.inner_comments.as_slice() {
        // Roundcube treats a plain `if false` as an enabled rule with the
        // test `false`, which the model cannot represent.
        [] if test.is("false") => return None,
        [] => (true, conditions(test)?),
        // A disabled rule keeps its test in a comment: `if false # <test>`.
        [c] if c.kind == CommentKind::Hash && test.is("false") && test.arguments == Default::default() => {
            let disabled = parse(&format!("if {} {{}}", c.text.trim())).ok()?;
            let [inner] = disabled.commands.as_slice() else { return None };
            let Some(Tests::Single(inner_test)) = &inner.arguments.tests else { return None };
            if has_comments(inner) || !inner.arguments.args.is_empty() {
                return None;
            }
            (false, conditions(inner_test)?)
        }
        _ => return None,
    };

    let block = cmd.block.as_ref()?;
    if !block.trailing_comments.is_empty() {
        return None;
    }
    let actions = block
        .commands
        .iter()
        .map(|c| if has_comments(c) { None } else { action(c, single_folder) })
        .collect::<Option<Vec<_>>>()?;
    Some(Rule { name, enabled, conditions, actions })
}

fn conditions(test: &Test) -> Option<Conditions> {
    let join = if test.is("allof") {
        Join::All
    } else if test.is("anyof") {
        Join::Any
    } else if test.is("true") && test.arguments == Default::default() {
        return Some(Conditions::default());
    } else {
        // A bare test is "any of" one test, as in Roundcube.
        return Some(Conditions { join: Join::Any, items: vec![condition(test)?] });
    };
    if !test.arguments.args.is_empty() {
        return None;
    }
    let Some(Tests::List(list)) = &test.arguments.tests else { return None };
    let items = list.tests.iter().map(condition).collect::<Option<Vec<_>>>()?;
    Some(Conditions { join, items })
}

fn condition(test: &Test) -> Option<Condition> {
    if test.is("not") {
        let Some(Tests::Single(inner)) = &test.arguments.tests else { return None };
        if !test.arguments.args.is_empty() {
            return None;
        }
        return Some(Condition { negate: true, test: condition_test(inner)? });
    }
    Some(Condition { negate: false, test: condition_test(test)? })
}

// ── Argument handling ─────────────────────────────────────────────────────────

/// What follows a tagged argument.
#[derive(Clone, Copy)]
enum TagArg {
    None,
    String,
    StringList,
    Number,
}

const MATCH_TAGS: &[(&str, TagArg)] = &[
    ("is", TagArg::None),
    ("contains", TagArg::None),
    ("matches", TagArg::None),
    ("regex", TagArg::None),
    ("value", TagArg::String),
    ("count", TagArg::String),
    ("comparator", TagArg::String),
];

const ADDRESS_PART_TAGS: &[(&str, TagArg)] = &[
    ("all", TagArg::None),
    ("localpart", TagArg::None),
    ("domain", TagArg::None),
    ("user", TagArg::None),
    ("detail", TagArg::None),
];

/// Arguments split into tags (lower-cased, with their value) and positional
/// arguments.
struct Args<'a> {
    tags: Vec<(String, Option<&'a Argument>)>,
    positional: Vec<&'a Argument>,
}

impl<'a> Args<'a> {
    /// Splits `args` according to the allowed tag sets. Fails on unknown or
    /// repeated tags, missing tag values and tags after positional arguments.
    fn split(args: &'a [Argument], allowed: &[&[(&str, TagArg)]]) -> Option<Self> {
        let mut tags: Vec<(String, Option<&Argument>)> = Vec::new();
        let mut positional = Vec::new();
        let mut iter = args.iter();
        while let Some(arg) = iter.next() {
            let Argument::Tag(tag) = arg else {
                positional.push(arg);
                continue;
            };
            if !positional.is_empty() {
                return None;
            }
            let name = tag.name.to_ascii_lowercase();
            let kind = allowed.iter().flat_map(|s| s.iter()).find(|(n, _)| *n == name)?.1;
            if tags.iter().any(|(n, _)| *n == name) {
                return None;
            }
            let value = match kind {
                TagArg::None => None,
                TagArg::String => Some(iter.next().filter(|a| matches!(a, Argument::String(_)))?),
                TagArg::StringList => Some(iter.next().filter(|a| a.strings().is_some())?),
                TagArg::Number => Some(iter.next().filter(|a| matches!(a, Argument::Number(_)))?),
            };
            tags.push((name, value));
        }
        Some(Self { tags, positional })
    }

    fn has(&self, name: &str) -> bool {
        self.tags.iter().any(|(n, _)| n == name)
    }

    fn string(&self, name: &str) -> Option<String> {
        match self.tags.iter().find(|(n, _)| n == name)?.1? {
            Argument::String(s) => Some(s.value.clone()),
            _ => None,
        }
    }

    fn strings(&self, name: &str) -> Option<Vec<String>> {
        Some(values(self.tags.iter().find(|(n, _)| n == name)?.1?.strings()?))
    }

    fn number(&self, name: &str) -> Option<u64> {
        match self.tags.iter().find(|(n, _)| n == name)?.1? {
            Argument::Number(n) => Some(n.value),
            _ => None,
        }
    }

    /// Positional string-list arguments; fails unless there are exactly `n`.
    fn string_lists(&self, n: usize) -> Option<Vec<Vec<String>>> {
        if self.positional.len() != n {
            return None;
        }
        self.positional.iter().map(|a| a.strings().map(values)).collect()
    }

    /// Positional single strings (not lists); fails unless there are exactly `n`.
    fn single_strings(&self, n: usize) -> Option<Vec<String>> {
        if self.positional.len() != n {
            return None;
        }
        self.positional
            .iter()
            .map(|a| match a {
                Argument::String(s) => Some(s.value.clone()),
                _ => None,
            })
            .collect()
    }

    /// At most one of `names` may be present; returns it.
    fn one_of(&self, names: &[&'static str]) -> Option<Option<&'static str>> {
        let mut found = names.iter().copied().filter(|n| self.has(n));
        let first = found.next();
        if found.next().is_some() {
            return None;
        }
        Some(first)
    }

    fn matcher(&self) -> Option<Matcher> {
        let match_type = match self.one_of(&["is", "contains", "matches", "regex", "value", "count"])? {
            None | Some("is") => MatchType::Is,
            Some("contains") => MatchType::Contains,
            Some("matches") => MatchType::Matches,
            Some("regex") => MatchType::Regex,
            Some("value") => MatchType::Value(RelOp::parse(&self.string("value")?)?),
            Some(_) => MatchType::Count(RelOp::parse(&self.string("count")?)?),
        };
        Some(Matcher { match_type, comparator: self.string("comparator") })
    }

    fn address_part(&self) -> Option<AddressPart> {
        Some(match self.one_of(&["all", "localpart", "domain", "user", "detail"])? {
            None | Some("all") => AddressPart::All,
            Some("localpart") => AddressPart::LocalPart,
            Some("domain") => AddressPart::Domain,
            Some("user") => AddressPart::User,
            Some(_) => AddressPart::Detail,
        })
    }
}

fn values(strings: &[crate::ast::SieveString]) -> Vec<String> {
    strings.iter().map(|s| s.value.clone()).collect()
}

fn single_string_list(arguments: &crate::ast::Arguments) -> Option<Vec<String>> {
    if arguments.tests.is_some() {
        return None;
    }
    match arguments.args.as_slice() {
        [arg] => arg.strings().map(values),
        _ => None,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

fn condition_test(test: &Test) -> Option<ConditionTest> {
    // Nested groups; `true`/`false` are the empty ones.
    if test.is("allof") || test.is("anyof") {
        let join = if test.is("allof") { Join::All } else { Join::Any };
        let Some(Tests::List(list)) = &test.arguments.tests else { return None };
        if !test.arguments.args.is_empty() {
            return None;
        }
        let items = list.tests.iter().map(condition).collect::<Option<Vec<_>>>()?;
        return Some(ConditionTest::Group { join, items });
    }
    if (test.is("true") || test.is("false")) && test.arguments == Default::default() {
        let join = if test.is("true") { Join::All } else { Join::Any };
        return Some(ConditionTest::Group { join, items: Vec::new() });
    }
    if test.arguments.tests.is_some() {
        return None;
    }
    let args = &test.arguments.args;
    let name = test.name.name.to_ascii_lowercase();
    Some(match name.as_str() {
        "header" => {
            let a = Args::split(args, &[MATCH_TAGS])?;
            let [headers, keys] = <[_; 2]>::try_from(a.string_lists(2)?).ok()?;
            ConditionTest::Header { headers, matcher: a.matcher()?, keys }
        }
        "address" | "envelope" => {
            let a = Args::split(args, &[MATCH_TAGS, ADDRESS_PART_TAGS])?;
            let [fields, keys] = <[_; 2]>::try_from(a.string_lists(2)?).ok()?;
            let (part, matcher) = (a.address_part()?, a.matcher()?);
            if name == "address" {
                ConditionTest::Address { headers: fields, part, matcher, keys }
            } else {
                ConditionTest::Envelope { parts: fields, part, matcher, keys }
            }
        }
        "exists" => {
            let a = Args::split(args, &[])?;
            let [headers] = <[_; 1]>::try_from(a.string_lists(1)?).ok()?;
            ConditionTest::Exists { headers }
        }
        "size" => {
            let a = Args::split(args, &[&[("over", TagArg::None), ("under", TagArg::None)]])?;
            let over = a.one_of(&["over", "under"])?? == "over";
            let [Argument::Number(limit)] = a.positional.as_slice() else { return None };
            ConditionTest::Size { over, limit: limit.value }
        }
        "body" => {
            let transforms = &[("text", TagArg::None), ("raw", TagArg::None), ("content", TagArg::StringList)];
            let a = Args::split(args, &[MATCH_TAGS, transforms])?;
            let transform = match a.one_of(&["text", "raw", "content"])? {
                None | Some("text") => BodyTransform::Text,
                Some("raw") => BodyTransform::Raw,
                Some(_) => BodyTransform::Content(a.strings("content")?),
            };
            let [keys] = <[_; 1]>::try_from(a.string_lists(1)?).ok()?;
            ConditionTest::Body { transform, matcher: a.matcher()?, keys }
        }
        "currentdate" => {
            let a = Args::split(args, &[MATCH_TAGS, &[("zone", TagArg::String)]])?;
            let [date_part, keys] = <[_; 2]>::try_from(a.positional.clone()).ok()?;
            let Argument::String(date_part) = date_part else { return None };
            ConditionTest::CurrentDate {
                zone: a.string("zone"),
                matcher: a.matcher()?,
                date_part: date_part.value.clone(),
                keys: values(keys.strings()?),
            }
        }
        "date" => {
            let zones = &[("zone", TagArg::String), ("originalzone", TagArg::None)];
            let a = Args::split(args, &[MATCH_TAGS, zones])?;
            let zone = match a.one_of(&["zone", "originalzone"])? {
                None => DateZone::Local,
                Some("zone") => DateZone::Zone(a.string("zone")?),
                Some(_) => DateZone::Original,
            };
            let [Argument::String(header), Argument::String(date_part), keys] = a.positional.as_slice() else {
                return None;
            };
            ConditionTest::Date {
                zone,
                matcher: a.matcher()?,
                header: header.value.clone(),
                date_part: date_part.value.clone(),
                keys: values(keys.strings()?),
            }
        }
        _ => return None,
    })
}

// ── Actions ───────────────────────────────────────────────────────────────────

/// In single-folder mode, `set "<folder variable>" "<folder>"` is a move and
/// `return` is "stop". A plain `fileinto` or `stop` there (e.g. written by
/// another editor) is read as move and stop too, so saving turns them into
/// the single-folder form.
fn action(cmd: &Command, single_folder: bool) -> Option<Action> {
    if cmd.block.is_some() || cmd.arguments.tests.is_some() {
        return None;
    }
    let args = &cmd.arguments.args;
    let name = cmd.name.name.to_ascii_lowercase();
    Some(match name.as_str() {
        "set" if single_folder => {
            let a = Args::split(args, &[])?;
            let [variable, mailbox] = <[_; 2]>::try_from(a.single_strings(2)?).ok()?;
            if !variable.eq_ignore_ascii_case(FOLDER_VARIABLE) {
                return None;
            }
            Action::FileInto { mailbox, copy: false, create: false, flags: None }
        }
        "return" if single_folder && args.is_empty() => Action::Stop,
        "fileinto" => {
            let tags = &[("copy", TagArg::None), ("create", TagArg::None), ("flags", TagArg::StringList)];
            let a = Args::split(args, &[tags])?;
            let [mailbox] = <[_; 1]>::try_from(a.single_strings(1)?).ok()?;
            Action::FileInto {
                mailbox,
                copy: a.has("copy"),
                create: a.has("create"),
                flags: a.has("flags").then(|| a.strings("flags")).flatten(),
            }
        }
        "redirect" => {
            let a = Args::split(args, &[&[("copy", TagArg::None)]])?;
            let [address] = <[_; 1]>::try_from(a.single_strings(1)?).ok()?;
            Action::Redirect { address, copy: a.has("copy") }
        }
        "reject" | "ereject" => {
            let a = Args::split(args, &[])?;
            let [reason] = <[_; 1]>::try_from(a.single_strings(1)?).ok()?;
            Action::Reject { reason, extended: name == "ereject" }
        }
        "discard" | "stop" if args.is_empty() => {
            if name == "stop" { Action::Stop } else { Action::Discard }
        }
        "keep" => {
            let a = Args::split(args, &[&[("flags", TagArg::StringList)]])?;
            if !a.positional.is_empty() {
                return None;
            }
            Action::Keep { flags: a.has("flags").then(|| a.strings("flags")).flatten() }
        }
        "addflag" | "setflag" | "removeflag" => {
            let a = Args::split(args, &[])?;
            let [flags] = <[_; 1]>::try_from(a.string_lists(1)?).ok()?;
            let op = match name.as_str() {
                "addflag" => FlagOp::Add,
                "setflag" => FlagOp::Set,
                _ => FlagOp::Remove,
            };
            Action::Flags { op, flags }
        }
        "vacation" => {
            let tags = &[
                ("days", TagArg::Number),
                ("seconds", TagArg::Number),
                ("subject", TagArg::String),
                ("from", TagArg::String),
                ("addresses", TagArg::StringList),
                ("mime", TagArg::None),
                ("handle", TagArg::String),
            ];
            let a = Args::split(args, &[tags])?;
            let period = match a.one_of(&["days", "seconds"])? {
                None => None,
                Some("days") => Some(VacationPeriod::Days(a.number("days")?)),
                Some(_) => Some(VacationPeriod::Seconds(a.number("seconds")?)),
            };
            let [reason] = <[_; 1]>::try_from(a.single_strings(1)?).ok()?;
            Action::Vacation(Vacation {
                period,
                subject: a.string("subject"),
                from: a.string("from"),
                addresses: a.strings("addresses").unwrap_or_default(),
                mime: a.has("mime"),
                handle: a.string("handle"),
                reason,
            })
        }
        _ => return None,
    })
}
