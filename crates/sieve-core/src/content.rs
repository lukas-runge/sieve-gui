//! What a script does, without how it is written: for showing only the
//! changes that matter when a script is saved in a new layout.
//!
//! On top of [`canonical`](crate::canonical) (no comments, fixed layout),
//! spellings that mean the same are made equal:
//!
//! - `require` is left out: it only enables extensions, and a script that
//!   lacks one is rejected by the server's check anyway;
//! - a list of one string is the string itself (`["a"]` is `"a"`), a
//!   multi-line string a quoted one;
//! - arguments that only state the default are dropped: the match type
//!   `:is`, the comparator `"i;ascii-casemap"`, the address part `:all` and
//!   the body transform `:text` (RFC 5228 §2.7, RFC 5173 §5);
//! - command, test and tag names are lower case.
//!
//! Anything else stays: `fileinto "x"; stop;` and `set "folder" "x"; return;`
//! differ, because they can behave differently.

use crate::ast::{Argument, Arguments, Command, Script, SieveString, StringKind, Test, Tests};
use crate::printer::{print_script, PrintOptions};

/// The script as [`canonical`](crate::canonical), with equal spellings made
/// equal; `None` if it doesn't parse.
pub fn content(source: &str) -> Option<String> {
    let mut script: Script = crate::parse(source).ok()?;
    script.trailing_comments.clear();
    script.commands.retain(|c| !c.is("require"));
    for command in &mut script.commands {
        normalize_command(command);
    }
    Some(print_script(&script, &PrintOptions::readable()))
}

fn normalize_command(command: &mut Command) {
    command.strip_comments();
    command.name.name.make_ascii_lowercase();
    normalize_arguments(&mut command.arguments, "");
    if let Some(block) = &mut command.block {
        for inner in &mut block.commands {
            normalize_command(inner);
        }
    }
}

fn normalize_test(test: &mut Test) {
    test.name.name.make_ascii_lowercase();
    let name = test.name.name.clone();
    normalize_arguments(&mut test.arguments, &name);
}

/// Tests whose match type defaults to `:is` and comparator to `i;ascii-casemap`.
const MATCHING: &[&str] = &["header", "address", "envelope", "string", "hasflag", "date", "currentdate", "metadata", "servermetadata", "environment", "body"];

fn normalize_arguments(arguments: &mut Arguments, test: &str) {
    let mut args = Vec::with_capacity(arguments.args.len());
    let mut iter = std::mem::take(&mut arguments.args).into_iter().peekable();
    while let Some(arg) = iter.next() {
        match arg {
            Argument::Tag(mut tag) => {
                tag.name.make_ascii_lowercase();
                let matching = MATCHING.contains(&test);
                // `:comparator "i;ascii-casemap"`: the default comparator, with its argument.
                if matching
                    && tag.name == "comparator"
                    && let Some(Argument::String(s)) = iter.peek()
                    && s.value.eq_ignore_ascii_case("i;ascii-casemap")
                {
                    iter.next();
                    continue;
                }
                let default = (matching && tag.name == "is")
                    || ((test == "address" || test == "envelope") && tag.name == "all")
                    || (test == "body" && tag.name == "text");
                if !default {
                    args.push(Argument::Tag(tag));
                }
            }
            Argument::StringList(list) if list.items.len() == 1 => {
                let item = list.items.into_iter().next().expect("one item");
                args.push(Argument::String(quoted(item)));
            }
            Argument::StringList(mut list) => {
                list.items = list.items.into_iter().map(quoted).collect();
                args.push(Argument::StringList(list));
            }
            Argument::String(s) => args.push(Argument::String(quoted(s))),
            other => args.push(other),
        }
    }
    arguments.args = args;
    match &mut arguments.tests {
        Some(Tests::Single(test)) => normalize_test(test),
        Some(Tests::List(list)) => list.tests.iter_mut().for_each(normalize_test),
        None => {}
    }
}

fn quoted(mut s: SieveString) -> SieveString {
    s.kind = StringKind::Quoted;
    s
}

#[cfg(test)]
mod tests {
    use super::content;

    fn same(a: &str, b: &str) {
        assert_eq!(content(a), content(b), "\n{a}\n---\n{b}");
    }

    #[test]
    fn spellings_that_mean_the_same_are_equal() {
        same(
            "require \"fileinto\";\nrequire \"body\";\n# sort\nif address :domain :matches \"from\" [\"*a.de\"] { fileinto \"A\"; }",
            "require [\"body\",\"fileinto\"];\n\nif address :domain :matches \"from\" \"*a.de\"\n{\n\tfileinto \"A\";\n}\n",
        );
        same("if address \"from\" \"x@y\" { keep; }", "if address :all :is :comparator \"i;ascii-casemap\" \"from\" \"x@y\" { keep; }");
        same("if body :regex \"x\" { keep; }", "IF Body :text :REGEX \"x\" { Keep; }");
        same("global [\"move_to_folder\"];", "global \"move_to_folder\";");
    }

    #[test]
    fn what_can_behave_differently_stays_apart() {
        assert_ne!(content("if true { fileinto \"Junk\"; stop; }"), content("if true { set \"f\" \"Junk\"; return; }"));
        assert_ne!(content("if header :contains \"a\" \"b\" { keep; }"), content("if header :is \"a\" \"b\" { keep; }"));
        assert_ne!(content("if address :domain \"from\" \"x\" { keep; }"), content("if address \"from\" \"x\" { keep; }"));
        assert_ne!(
            content("if header :comparator \"i;octet\" \"a\" \"b\" { keep; }"),
            content("if header \"a\" \"b\" { keep; }")
        );
    }
}
