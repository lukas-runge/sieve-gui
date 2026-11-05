//! Synology MailPlus Server (fixtures in `tests/fixtures/mailplus`).
//!
//! MailPlus' root script (`synomc`) includes two personal scripts that only
//! record a folder in global variables, and then files the message there:
//! a block list match first, else a filter's folder, else an allow list
//! match.
//!
//! - Filters from the web UI (`filter`): one `if` each, under a `# id: <n>`
//!   comment.
//!
//!   ```sieve
//!   # id: 1
//!   if header :contains "subject" "invoice"
//!   {
//!       set "filter_mailbox" "Invoices";
//!   }
//!   ```
//!
//! - Personal block and allow lists (`spam_rule`): one guarded `if` per
//!   entry, so the first matching entry wins, followed by fixed code that
//!   adjusts the subject and adds a header for list matches.
//!
//!   ```sieve
//!   # id: 2
//!   if allof(string :is "${spam_rule_blacklist_mailbox}" "", string :is "${spam_rule_whitelist_mailbox}" "")
//!   {
//!       if anyof(address :is :all "from" "friend@example.com", address :is :all "from" "friend@example.com")
//!       {
//!           set "spam_rule_whitelist_mailbox" "INBOX";
//!       }
//!   }
//!   ```
//!
//! The import keeps MailPlus' priority within one script, where the last
//! move wins: allow list entries first, then the filters, then block list
//! entries.

use crate::ast::{Argument, ClearSpans, Command, Script, Tests};
use crate::export::{command, hash_comment, list, string};
use crate::metadata::format_rule_name;
use crate::model::RuleSet;
use crate::printer::{print_script, PrintOptions};
use crate::ParseError;

use super::{any, fingerprint, strings, walk, Detection, Provider, ProviderScript, RuleNames};

/// The variable a filter records its folder in.
const FILTER: &str = "filter_mailbox";
/// The variable a block list match records its folder in.
const BLOCK: &str = "spam_rule_blacklist_mailbox";
/// The variable an allow list match records its folder in.
const ALLOW: &str = "spam_rule_whitelist_mailbox";

pub struct MailPlus;

struct Parsed<'a> {
    script: &'a ProviderScript,
    ast: Script,
}

/// MailPlus' scripts on an account: its root and the scripts it includes
/// that record folders for it, in the order the root includes them.
struct Setup<'a> {
    root: &'a ProviderScript,
    own: Vec<Parsed<'a>>,
}

fn setup(scripts: &[ProviderScript]) -> Option<Setup<'_>> {
    let parsed: Vec<Parsed> =
        scripts.iter().filter_map(|script| Some(Parsed { script, ast: crate::parse(&script.source).ok()? })).collect();
    let root = parsed.iter().find(|p| files_into(&p.ast, FILTER) && files_into(&p.ast, BLOCK))?.script;
    let includes = crate::root::script_links(&root.source).includes;
    let mut own: Vec<Parsed> =
        parsed.into_iter().filter(|p| includes.contains(&p.script.name) && declares(&p.ast, &[FILTER, BLOCK, ALLOW])).collect();
    own.sort_by_key(|p| includes.iter().position(|n| *n == p.script.name));
    Some(Setup { root, own })
}

impl Provider for MailPlus {
    fn id(&self) -> &'static str {
        "mailplus"
    }

    fn detect(&self, scripts: &[ProviderScript]) -> Option<Detection> {
        let setup = setup(scripts)?;
        let entries = entries(&setup.own, &RuleNames::default());
        Some(Detection {
            provider: self.id().to_owned(),
            root: setup.root.name.clone(),
            scripts: setup.own.iter().map(|p| p.script.name.clone()).collect(),
            filters: entries.filters.len(),
            list_entries: entries.allow.len() + entries.block.len(),
            reactivates_root: true,
            fingerprint: fingerprint(setup.own.iter().map(|p| p.script.source.as_str())),
        })
    }

    fn import(&self, scripts: &[ProviderScript], names: &RuleNames) -> Result<RuleSet, ParseError> {
        let Some(setup) = setup(scripts) else { return Ok(RuleSet::default()) };
        let entries = entries(&setup.own, names);
        // The extensions the filters may need. List entries only compare
        // addresses; `spam_rule` requires more for MailPlus' fixed code.
        let mut extensions: Vec<String> = vec!["fileinto".to_owned()];
        for p in setup.own.iter().filter(|p| declares(&p.ast, &[FILTER])) {
            for cmd in p.ast.commands.iter().filter(|c| c.is("require")) {
                for ext in cmd.arguments.args.iter().filter_map(Argument::strings).flatten() {
                    if !extensions.contains(&ext.value) {
                        extensions.push(ext.value.clone());
                    }
                }
            }
        }
        let mut commands = vec![command("require", vec![list(extensions)], None)];
        commands.extend(entries.allow.into_iter().chain(entries.filters).chain(entries.block));
        let script = Script { commands, trailing_comments: Vec::new() };
        crate::import(&print_script(&script, &PrintOptions::readable()))
    }
}

/// The filters and list entries, each turned into an `if` that files into
/// its folder, named with a `# rule:[…]` comment.
#[derive(Default)]
struct Entries {
    allow: Vec<Command>,
    filters: Vec<Command>,
    block: Vec<Command>,
}

fn entries(own: &[Parsed], names: &RuleNames) -> Entries {
    let mut out = Entries::default();
    for p in own {
        for cmd in p.ast.commands.iter().filter(|c| c.is("if")) {
            if let Some((inner, var)) = list_entry(cmd) {
                let addresses = addresses(inner);
                let (kind, target) =
                    if var == ALLOW { (&names.allow_list, &mut out.allow) } else { (&names.block_list, &mut out.block) };
                let name = if addresses.is_empty() { kind.clone() } else { format!("{kind}: {}", addresses.join(", ")) };
                target.push(rule(inner, var, &name));
            } else if sets(std::slice::from_ref(cmd), FILTER) {
                let name = match filter_id(cmd) {
                    Some(id) => format!("{} {id}", names.filter),
                    None => names.filter.clone(),
                };
                out.filters.push(rule(cmd, FILTER, &name));
            }
        }
    }
    out
}

/// A copy of `cmd` that files into the folder instead of recording it in
/// `var`, without its comments, with duplicate tests removed and named.
fn rule(cmd: &Command, var: &str, name: &str) -> Command {
    let mut out = cmd.clone();
    out.clear_spans();
    out.strip_comments();
    file_instead_of_set(&mut out, var);
    dedupe_tests(&mut out);
    out.leading_comments = vec![hash_comment(format_rule_name(name))];
    out
}

/// A list entry: an `if` guarded by `string` tests on the list variables,
/// around an `if` that records a list folder. Returns that inner `if` and
/// the variable it sets.
fn list_entry(cmd: &Command) -> Option<(&Command, &'static str)> {
    let guard = cmd.arguments.tests.as_ref()?.as_slice();
    let [allof] = guard else { return None };
    let conditions = allof.arguments.tests.as_ref()?.as_slice();
    if !allof.is("allof") || conditions.is_empty() || !conditions.iter().all(|t| t.is("string")) {
        return None;
    }
    let [inner] = cmd.block.as_ref()?.commands.as_slice() else { return None };
    if !inner.is("if") {
        return None;
    }
    let inner_slice = std::slice::from_ref(inner);
    [ALLOW, BLOCK].into_iter().find(|var| sets(inner_slice, var)).map(|var| (inner, var))
}

/// The number of a filter from its `# id: <n>` comment.
fn filter_id(cmd: &Command) -> Option<String> {
    cmd.leading_comments.iter().find_map(|c| c.text.trim().strip_prefix("id:").map(|id| id.trim().to_owned()))
}

/// The addresses an entry compares with, each once.
fn addresses(cmd: &Command) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for test in cmd.arguments.tests.iter().flat_map(Tests::as_slice) {
        let tests = match &test.arguments.tests {
            Some(inner) if test.is("anyof") || test.is("allof") => inner.as_slice(),
            _ => std::slice::from_ref(test),
        };
        for t in tests.iter().filter(|t| t.is("address")) {
            let keys = t.arguments.args.iter().rev().find_map(Argument::strings).unwrap_or_default();
            for key in keys {
                if !out.contains(&key.value) {
                    out.push(key.value.clone());
                }
            }
        }
    }
    out
}

/// Replaces `set "<var>" "<folder>"` with `fileinto "<folder>"`, at any depth.
fn file_instead_of_set(cmd: &mut Command, var: &str) {
    let Some(block) = &mut cmd.block else { return };
    for c in &mut block.commands {
        if c.is("set")
            && let [name, folder] = strings(c).as_slice()
            && *name == var
        {
            *c = command("fileinto", vec![string(folder)], None);
            continue;
        }
        file_instead_of_set(c, var);
    }
}

/// `anyof (a, a)` becomes `a`: MailPlus writes each list address twice.
fn dedupe_tests(cmd: &mut Command) {
    let Some(Tests::Single(group)) = &mut cmd.arguments.tests else { return };
    if !(group.is("anyof") || group.is("allof")) {
        return;
    }
    let Some(Tests::List(list)) = &group.arguments.tests else { return };
    let mut unique = Vec::new();
    for t in &list.tests {
        if !unique.contains(t) {
            unique.push(t.clone());
        }
    }
    if let [single] = unique.as_slice() {
        cmd.arguments.tests = Some(Tests::Single(Box::new(single.clone())));
    } else if unique.len() < list.tests.len() {
        let mut list = list.clone();
        list.tests = unique;
        group.arguments.tests = Some(Tests::List(list));
    }
}

/// Whether a command sets `var` (at any depth).
fn sets(commands: &[Command], var: &str) -> bool {
    any(commands, |c| c.is("set") && strings(c).first() == Some(&var))
}

/// Whether the script files into the folder recorded in `var`.
fn files_into(script: &Script, var: &str) -> bool {
    let target = format!("${{{var}}}");
    any(&script.commands, |c| c.is("fileinto") && strings(c).last() == Some(&target.as_str()))
}

/// Whether the script declares one of the variables as global.
fn declares(script: &Script, vars: &[&str]) -> bool {
    let mut found = false;
    walk(&script.commands, &mut |c| {
        if c.is("global") {
            found = found || c.arguments.args.iter().filter_map(Argument::strings).flatten().any(|s| vars.contains(&s.value.as_str()));
        }
    });
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Action, RuleItem};

    fn fixture(name: &str) -> String {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mailplus").join(name);
        std::fs::read_to_string(path).unwrap()
    }

    fn account(filter: &str, spam_rule: &str) -> Vec<ProviderScript> {
        let script = |name: &str, file: &str| ProviderScript { name: name.to_owned(), source: fixture(file) };
        vec![script("synomc", "synomc.sieve"), script("filter", filter), script("spam_rule", spam_rule)]
    }

    #[test]
    fn detects_an_account_without_filters() {
        let found = MailPlus.detect(&account("filter.sieve", "spam_rule.sieve")).unwrap();
        assert_eq!(found.root, "synomc");
        assert_eq!(found.scripts, ["spam_rule", "filter"]);
        assert_eq!((found.filters, found.list_entries), (0, 0));
        assert!(!found.has_rules());
    }

    #[test]
    fn counts_filters_and_list_entries() {
        let found = MailPlus.detect(&account("filter-one-rule.sieve", "spam_rule-two-entries.sieve")).unwrap();
        assert_eq!((found.filters, found.list_entries), (1, 2));
    }

    #[test]
    fn ignores_other_setups() {
        let root = crate::root::root_script(&["synomc".to_owned()], &[], true, &PrintOptions::readable());
        let scripts = vec![ProviderScript { name: "sieve-gui".to_owned(), source: root }];
        assert!(MailPlus.detect(&scripts).is_none());
        // Without its root, MailPlus' scripts alone are no setup.
        let mut no_root = account("filter-one-rule.sieve", "spam_rule.sieve");
        no_root.remove(0);
        assert!(MailPlus.detect(&no_root).is_none());
    }

    #[test]
    fn imports_lists_and_filters_in_mailplus_priority() {
        let rules = MailPlus.import(&account("filter-one-rule.sieve", "spam_rule-two-entries.sieve"), &RuleNames::default()).unwrap();
        let summary: Vec<(String, Vec<Action>)> = rules
            .items
            .iter()
            .map(|item| match item {
                RuleItem::Rule(r) => (r.name.clone(), r.actions.clone()),
                RuleItem::Raw(raw) => panic!("not a rule: {}", raw.code),
            })
            .collect();
        let move_to = |mailbox: &str| vec![Action::FileInto { mailbox: mailbox.to_owned(), copy: false, create: false, flags: None }];
        assert_eq!(
            summary,
            [
                ("Allow list: blah@example.com".to_owned(), move_to("INBOX")),
                ("Filter 1".to_owned(), move_to("Junk")),
                ("Block list: test.example@example.com".to_owned(), move_to("Junk")),
            ]
        );
        let text = crate::export(&rules, &PrintOptions::readable());
        assert!(text.starts_with("require [\"fileinto\"];"), "{text}");
        // MailPlus' variables are gone, and the doubled address with them.
        assert!(!text.contains("filter_mailbox") && !text.contains("spam_rule"), "{text}");
        assert_eq!(text.matches("blah@example.com").count(), 2, "{text}");
        assert!(text.contains("if address :is \"from\" \"blah@example.com\""), "{text}");
    }

    #[test]
    fn names_rules_in_the_apps_language() {
        let names = RuleNames { filter: "Filter".into(), allow_list: "Erlaubt".into(), block_list: "Blockiert".into() };
        let rules = MailPlus.import(&account("filter-one-rule.sieve", "spam_rule-two-entries.sieve"), &names).unwrap();
        let names: Vec<&str> = rules
            .items
            .iter()
            .filter_map(|item| match item {
                RuleItem::Rule(r) => Some(r.name.as_str()),
                RuleItem::Raw(_) => None,
            })
            .collect();
        assert_eq!(names, ["Erlaubt: blah@example.com", "Filter 1", "Blockiert: test.example@example.com"]);
    }

    #[test]
    fn fingerprint_follows_the_filters_not_their_layout() {
        let print = |scripts: &[ProviderScript]| MailPlus.detect(scripts).unwrap().fingerprint;
        let taken = print(&account("filter-one-rule.sieve", "spam_rule.sieve"));
        let mut relaid = account("filter-one-rule.sieve", "spam_rule.sieve");
        relaid[1].source = format!("# saved again\n{}", relaid[1].source.replace('\t', "    "));
        assert_eq!(print(&relaid), taken);
        assert_ne!(print(&account("filter.sieve", "spam_rule.sieve")), taken);
        assert_ne!(print(&account("filter-one-rule.sieve", "spam_rule-two-entries.sieve")), taken);
    }

    #[test]
    fn imports_nothing_without_filters() {
        let rules = MailPlus.import(&account("filter.sieve", "spam_rule.sieve"), &RuleNames::default()).unwrap();
        assert!(rules.items.is_empty());
    }
}
