//! Compatibility with webmail filter editors that own a script.
//!
//! Roundcube's *managesieve* plugin regenerates the whole script from its own
//! model whenever it saves, and silently drops what it cannot represent (see
//! `tests/fixtures/roundcube/README.md`). sieve-gui writes the same format
//! (decisions D4, D8). For accounts whose filters are also edited in
//! Roundcube (an account setting), these checks tell what would be lost there.

use serde::Serialize;

use crate::ast::{Command, Comment, CommentKind};
use crate::metadata::parse_rule_name;
use crate::model::{ConditionTest, Rule, RuleItem, RuleSet};

/// Tests Roundcube's filter form can represent, as seen in its output.
const ROUNDCUBE_TESTS: &[&str] = &[
    "header", "address", "envelope", "exists", "size", "body", "date", "currentdate", "string", "duplicate", "true",
    "false", "allof", "anyof", "not",
];

/// Actions Roundcube's filter form can represent, as seen in its output.
const ROUNDCUBE_ACTIONS: &[&str] = &[
    "fileinto", "redirect", "reject", "discard", "keep", "stop", "vacation", "setflag", "addflag", "removeflag",
    "addheader", "deleteheader", "set", "notify",
];

/// What Roundcube would drop or change when it saves the script next time.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoundcubeRisks {
    /// Indices (into `RuleSet::items`) of raw blocks Roundcube can't
    /// represent: unknown tests or actions, `elsif`/`else`, nested `if`s and
    /// free comments. Roundcube drops or rewrites them. (Raw blocks for
    /// features only sieve-gui's cards lack, such as `notify`, are fine.)
    pub raw_blocks: Vec<usize>,
    /// Indices of rule cards with nested condition groups. Roundcube's form
    /// has a single level of "all of" / "any of".
    pub rules: Vec<usize>,
    /// A `/* … */` comment anywhere. Roundcube then shows no rules at all and
    /// replaces every rule on its next save.
    pub bracket_comments: bool,
}

impl RoundcubeRisks {
    pub fn is_empty(&self) -> bool {
        self.raw_blocks.is_empty() && self.rules.is_empty() && !self.bracket_comments
    }
}

/// What Roundcube would lose of `set`.
pub fn roundcube_risks(set: &RuleSet) -> RoundcubeRisks {
    let raw: Vec<(usize, &str)> = set
        .items
        .iter()
        .enumerate()
        .filter_map(|(i, item)| if let RuleItem::Raw(raw) = item { Some((i, raw.code.as_str())) } else { None })
        .collect();
    // Rule cards never carry comments other than their name, so bracket
    // comments can only be in the preamble or in raw blocks.
    let bracket_comments =
        set.preamble.iter().map(String::as_str).chain(raw.iter().map(|(_, code)| *code)).any(has_bracket_comment);
    let raw_blocks = raw.iter().filter(|(_, code)| !roundcube_understands(code)).map(|(i, _)| *i).collect();
    let rules = set
        .items
        .iter()
        .enumerate()
        .filter_map(|(i, item)| matches!(item, RuleItem::Rule(rule) if has_groups(rule)).then_some(i))
        .collect();
    RoundcubeRisks { raw_blocks, rules, bracket_comments }
}

fn has_groups(rule: &Rule) -> bool {
    rule.conditions.items.iter().any(|c| matches!(c.test, ConditionTest::Group { .. }))
}

/// Whether Roundcube reads a raw block as rules and writes it back intact.
fn roundcube_understands(code: &str) -> bool {
    let Ok(script) = crate::parse(code) else { return false };
    script.trailing_comments.is_empty() && script.commands.iter().all(top_level_understood)
}

fn top_level_understood(cmd: &Command) -> bool {
    // Only rule names survive as comments.
    if !cmd.leading_comments.iter().all(|c| parse_rule_name(c).is_some()) || cmd.trailing_comment.is_some() {
        return false;
    }
    if !cmd.is("if") {
        // A bare action at the top level becomes an "all messages" rule.
        return cmd.inner_comments.is_empty() && action_understood(cmd);
    }
    let Some(tests) = &cmd.arguments.tests else { return false };
    let tests = tests.as_slice();
    // `if false # <test>` is how Roundcube disables a rule.
    let disabled = matches!(tests, [t] if t.is("false"));
    let comments_ok = cmd.inner_comments.is_empty() || disabled;
    let block_ok = cmd.block.as_ref().is_some_and(|block| {
        block.trailing_comments.is_empty() && block.commands.iter().all(|c| !has_comments(c) && action_understood(c))
    });
    comments_ok && block_ok && tests.iter().all(|t| test_understood(t, false))
}

fn action_understood(cmd: &Command) -> bool {
    cmd.block.is_none() && ROUNDCUBE_ACTIONS.iter().any(|name| cmd.is(name))
}

/// `nested`: inside a test list or `not`, where Roundcube can't take
/// another group.
fn test_understood(test: &crate::ast::Test, nested: bool) -> bool {
    let group = test.is("allof") || test.is("anyof");
    ROUNDCUBE_TESTS.iter().any(|name| test.is(name))
        && !(group && nested)
        && test.arguments.tests.as_ref().is_none_or(|tests| tests.as_slice().iter().all(|t| test_understood(t, true)))
}

fn has_comments(cmd: &Command) -> bool {
    !cmd.leading_comments.is_empty() || !cmd.inner_comments.is_empty() || cmd.trailing_comment.is_some()
}

fn has_bracket_comment(source: &str) -> bool {
    match crate::parse(source) {
        Ok(script) => script.commands.iter().any(command_has_bracket) || script.trailing_comments.iter().any(is_bracket),
        // Raw blocks come from parsed scripts; be careful if one doesn't parse.
        Err(_) => source.contains("/*"),
    }
}

fn command_has_bracket(cmd: &Command) -> bool {
    cmd.leading_comments.iter().chain(&cmd.inner_comments).chain(&cmd.trailing_comment).any(is_bracket)
        || cmd.block.as_ref().is_some_and(|block| {
            block.commands.iter().any(command_has_bracket) || block.trailing_comments.iter().any(is_bracket)
        })
}

fn is_bracket(comment: &Comment) -> bool {
    comment.kind == CommentKind::Bracket
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::RawBlock;

    fn raw(code: &str) -> RuleItem {
        RuleItem::Raw(RawBlock { code: code.into() })
    }

    #[test]
    fn counts_raw_blocks_and_finds_bracket_comments() {
        let mut set = RuleSet { items: vec![raw("if true { keep; } elsif false { stop; }")], ..Default::default() };
        assert_eq!(roundcube_risks(&set), RoundcubeRisks { raw_blocks: vec![0], rules: vec![], bracket_comments: false });

        set.items.push(raw("if true {\n  /* note */\n  keep;\n}"));
        assert_eq!(roundcube_risks(&set), RoundcubeRisks { raw_blocks: vec![0, 1], rules: vec![], bracket_comments: true });
    }

    #[test]
    fn raw_blocks_roundcube_understands() {
        let ok = [
            "# rule:[Notify]\nif header :contains \"subject\" \"x\" { notify \"mailto:me@example.org\"; }",
            "if false # header :index 2 :contains \"received\" \"x\"\n{ addheader \"X-A\" \"b\"; }",
            "if anyof (not exists \"x\", duplicate) { set \"a\" \"b\"; stop; }",
            "fileinto \"Newsletters\";",
        ];
        for code in ok {
            assert!(roundcube_understands(code), "{code}");
        }
        let lost = [
            "if mailboxexists \"x\" { keep; }",
            "if true { debug_log \"x\"; }",
            "if true { if false { keep; } }",
            "if true { keep; } else { stop; }",
            "# a note\nif true { keep; }",
            "if true { keep; } # same line",
            "if true {\n  # inside\n  keep;\n}",
            "if true # not disabled\n{ keep; }",
            "if anyof (exists \"a\", allof (exists \"b\", exists \"c\")) { keep; }",
            "if not anyof (exists \"a\", exists \"b\") { keep; }",
        ];
        for code in lost {
            assert!(!roundcube_understands(code), "{code}");
        }
    }

    #[test]
    fn nested_groups_in_rules() {
        let set = crate::import(
            "if anyof (exists \"a\", exists \"b\") { keep; }\nif anyof (exists \"a\", allof (exists \"b\")) { keep; }\n",
        )
        .unwrap();
        assert_eq!(roundcube_risks(&set).rules, vec![1]);
    }

    #[test]
    fn bracket_in_preamble() {
        let set = RuleSet { preamble: Some("/* generated */".into()), ..Default::default() };
        assert!(roundcube_risks(&set).bracket_comments);
        assert!(roundcube_risks(&set).raw_blocks.is_empty());
    }

    #[test]
    fn strings_are_not_comments() {
        let set = RuleSet { items: vec![raw("if header :matches \"subject\" \"/* x */\" { keep; }")], ..Default::default() };
        assert!(!roundcube_risks(&set).bracket_comments);
    }
}
