//! Import/export of the rule model.

use sieve_core::model::*;
use sieve_core::{export, import, parse, PrintOptions};

fn opts() -> PrintOptions {
    PrintOptions::default()
}

fn rule(set: &RuleSet, i: usize) -> &Rule {
    match &set.items[i] {
        RuleItem::Rule(r) => r,
        RuleItem::Raw(r) => panic!("item {i} is raw: {}", r.code),
    }
}

fn raw(set: &RuleSet, i: usize) -> &str {
    match &set.items[i] {
        RuleItem::Raw(r) => &r.code,
        RuleItem::Rule(r) => panic!("item {i} is a rule: {r:?}"),
    }
}

/// export → import must give the same items again, and the output must be
/// stable when exported a second time. (`requires` may grow by the
/// extensions the exporter added.)
fn assert_stable(set: &RuleSet) -> String {
    let text = export(set, &opts());
    let again = import(&text).unwrap_or_else(|e| panic!("{e}\n{text}"));
    assert_eq!(again.items, set.items, "model changed after export:\n{text}");
    assert_eq!(export(&again, &opts()), text, "export is not stable");
    text
}

#[test]
fn imports_named_rules() {
    let set = import(
        r#"require ["fileinto", "imap4flags"];
# rule:[Taxes]
if anyof (header :contains "from" "@tax-office.example", not address :domain :is "to" ["a.example", "b.example"])
{
	fileinto "Taxes";
	addflag "\\Flagged";
	stop;
}
# rule:[Big]
if size :over 10M { discard; }
"#,
    )
    .unwrap();
    assert_eq!(set.requires, vec!["fileinto", "imap4flags"]);
    assert_eq!(set.items.len(), 2);

    let taxes = rule(&set, 0);
    assert_eq!(taxes.name, "Taxes");
    assert!(taxes.enabled);
    assert_eq!(taxes.conditions.join, Join::Any);
    assert_eq!(
        taxes.conditions.items,
        vec![
            Condition {
                negate: false,
                test: ConditionTest::Header {
                    headers: vec!["from".into()],
                    matcher: Matcher::new(MatchType::Contains),
                    keys: vec!["@tax-office.example".into()],
                },
            },
            Condition {
                negate: true,
                test: ConditionTest::Address {
                    headers: vec!["to".into()],
                    part: AddressPart::Domain,
                    matcher: Matcher::new(MatchType::Is),
                    keys: vec!["a.example".into(), "b.example".into()],
                },
            },
        ]
    );
    assert_eq!(
        taxes.actions,
        vec![
            Action::FileInto { mailbox: "Taxes".into(), copy: false, create: false, flags: None },
            Action::Flags { op: FlagOp::Add, flags: vec!["\\Flagged".into()] },
            Action::Stop,
        ]
    );

    let big = rule(&set, 1);
    assert_eq!(big.conditions.items[0].test, ConditionTest::Size { over: true, limit: 10 << 20 });
    assert_eq!(big.actions, vec![Action::Discard]);
    assert_stable(&set);
}

#[test]
fn imports_disabled_rules() {
    let set = import(
        "# rule:[off]\nif false # allof (header :is \"subject\" \"x\", exists \"list-id\")\n{\n\tkeep;\n}\n# rule:[off, always]\nif false # true\n{\n\tdiscard;\n}\n",
    )
    .unwrap();
    let off = rule(&set, 0);
    assert!(!off.enabled);
    assert_eq!(off.conditions.items.len(), 2);
    let always = rule(&set, 1);
    assert!(!always.enabled);
    assert!(always.conditions.items.is_empty());
    let text = assert_stable(&set);
    assert!(text.contains("if false # allof (header :is \"subject\" \"x\", exists \"list-id\")\n{"));
    assert!(text.contains("if false # true\n{"));
}

#[test]
fn bare_actions_form_all_messages_rules() {
    let set = import("require \"fileinto\";\n# rule:[all]\nfileinto \"x\";\nstop;\n# rule:[second]\nkeep;\n").unwrap();
    assert_eq!(set.items.len(), 2);
    assert_eq!(rule(&set, 0).name, "all");
    assert_eq!(rule(&set, 0).actions.len(), 2);
    assert!(rule(&set, 0).conditions.items.is_empty());
    assert_eq!(rule(&set, 1).actions, vec![Action::Keep { flags: None }]);
    assert_stable(&set);
}

#[test]
fn unknown_constructs_become_raw_blocks_verbatim() {
    let src = r#"require ["fileinto", "variables", "vnd.dovecot.debug"];
# rule:[ok]
if header :contains "subject" "a" { fileinto "A"; }
# my own comment
if header :matches "subject" "*" { set "s" "${1}"; }
if true { keep; } else { discard; }
# rule:[comment inside]
if exists "x" {
    # explain
    keep;
}
debug_log "hi";
if header :index 2 :is "received" "x" { stop; }
# trailing
"#;
    let set = import(src).unwrap();
    assert_eq!(set.items.len(), 7);
    assert_eq!(rule(&set, 0).name, "ok");
    assert_eq!(raw(&set, 1), "# my own comment\nif header :matches \"subject\" \"*\" { set \"s\" \"${1}\"; }");
    assert_eq!(raw(&set, 2), "if true { keep; } else { discard; }");
    assert!(raw(&set, 3).starts_with("# rule:[comment inside]\nif exists"));
    assert_eq!(raw(&set, 4), "debug_log \"hi\";");
    assert!(raw(&set, 5).contains(":index 2"));
    assert_eq!(raw(&set, 6), "# trailing");

    let text = assert_stable(&set);
    // With raw blocks present, every original require is kept.
    assert!(text.starts_with("require [\"fileinto\", \"variables\", \"vnd.dovecot.debug\"];\n"));
    for i in 1..7 {
        assert!(text.contains(raw(&set, i)));
    }
}

#[test]
fn requires_follow_the_rules() {
    let mut set = import("require [\"fileinto\", \"vacation\", \"x-unknown\"];\nif true { fileinto \"a\"; }").unwrap();
    // vacation is unused and known → dropped; unknown extensions are kept.
    assert!(export(&set, &opts()).starts_with("require [\"fileinto\", \"x-unknown\"];"));
    set.items.push(RuleItem::Rule(Rule {
        name: "copy".into(),
        enabled: true,
        conditions: Conditions {
            join: Join::All,
            items: vec![Condition {
                negate: false,
                test: ConditionTest::Envelope {
                    parts: vec!["to".into()],
                    part: AddressPart::Detail,
                    matcher: Matcher::new(MatchType::Regex),
                    keys: vec!["^x".into()],
                },
            }],
        },
        actions: vec![Action::Redirect { address: "a@b.example".into(), copy: true }],
    }));
    assert!(export(&set, &opts()).starts_with(
        "require [\"copy\", \"envelope\", \"fileinto\", \"regex\", \"subaddress\", \"x-unknown\"];"
    ));
}

#[test]
fn all_tests_and_actions_roundtrip() {
    let set = import(
        r#"require ["body", "date", "relational", "vacation", "imap4flags", "reject", "ereject", "copy", "mailbox", "comparator-i;ascii-numeric"];
# rule:[everything]
if allof (header :comparator "i;ascii-numeric" :value "ge" "x-spam-score" "5",
          header :count "gt" ["to", "cc"] "3",
          body :raw :contains "x",
          body :content ["text/plain", "text/html"] :matches "*y*",
          currentdate :zone "+0200" :value "le" "date" "2026-12-31",
          date :originalzone :is "date" "weekday" "0",
          envelope :user "from" "me",
          not exists "list-id",
          size :under 1K)
{
    fileinto :copy :create :flags ["\\Seen"] "Archive/2026";
    redirect :copy "x@example.com";
    keep :flags "\\Flagged";
    setflag "\\Seen";
    removeflag ["a", "b"];
    reject "no";
    ereject text:
multi
line
.
;
    vacation :days 3 :subject "Away" :from "me@example.com" :addresses ["a@x", "b@x"] :handle "h" :mime "Hi
there";
}
"#,
    )
    .unwrap();
    let r = rule(&set, 0);
    assert_eq!(r.conditions.items.len(), 9);
    assert_eq!(r.actions.len(), 8);
    assert_eq!(
        r.actions[7],
        Action::Vacation(Vacation {
            period: Some(VacationPeriod::Days(3)),
            subject: Some("Away".into()),
            from: Some("me@example.com".into()),
            addresses: vec!["a@x".into(), "b@x".into()],
            mime: true,
            handle: Some("h".into()),
            reason: "Hi\nthere".into(),
        })
    );
    assert_stable(&set);
}

#[test]
fn invalid_arguments_stay_raw() {
    for src in [
        "if header :contains :is \"a\" \"b\" { keep; }",  // two match types
        "if header :contains \"a\" { keep; }",            // missing keys
        "if header \"a\" :contains \"b\" { keep; }",      // tag after positional
        "if size 10 { keep; }",                           // missing :over/:under
        "if header :value \"xx\" \"a\" \"b\" { keep; }",   // bad relational op
        "if anyof (allof (true, true)) { keep; }",        // nested lists
        "fileinto [\"a\", \"b\"];",                       // list where string expected
        "vacation :days 1 :seconds 5 \"x\";",             // conflicting tags
        "if not not exists \"a\" { keep; }",
        "keep :unknown;",
    ] {
        let set = import(src).unwrap();
        assert_eq!(set.items.len(), 1, "{src}");
        assert_eq!(raw(&set, 0), src);
    }
}

/// Small deterministic PRNG so the test needs no extra dependency.
struct Lcg(u64);

impl Lcg {
    fn below(&mut self, n: usize) -> usize {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 33) % n as u64) as usize
    }
}

#[test]
fn import_export_is_stable_on_mutated_scripts() {
    const BASE: &str = "require [\"fileinto\", \"imap4flags\", \"vacation\"];\n# rule:[A]\nif anyof (header :contains \"from\" \"a\", not size :over 1M) {\n\tfileinto \"A\";\n\tstop;\n}\n# rule:[B]\nif false # header :is \"to\" \"b\"\n{\n\taddflag \"x\";\n}\n# rule:[C]\nkeep;\nvacation :days 2 \"away\";\n";
    const PIECES: &[&str] = &[
        "\"", "[", "]", "(", ")", "{", "}", ",", ";", ":", "#", "\n", " ", "not ", ":is ", ":copy ",
        "stop;", "keep;", "if true {", "# rule:[x]\n", "elsif false {}", "\"y\" ",
    ];
    let env = |k: &str, d: u64| std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d);
    let mut rng = Lcg(env("SIEVE_FUZZ_SEED", 7));
    let mut valid = 0;
    for _ in 0..env("SIEVE_FUZZ_ITERS", 10_000) {
        let mut s = BASE.to_owned();
        for _ in 0..=rng.below(3) {
            let mut at = rng.below(s.len() + 1);
            while !s.is_char_boundary(at) {
                at -= 1;
            }
            if rng.below(2) == 0 && at < s.len() {
                let mut end = at + 1;
                while !s.is_char_boundary(end) {
                    end += 1;
                }
                s.replace_range(at..end, "");
            } else {
                s.insert_str(at, PIECES[rng.below(PIECES.len())]);
            }
        }
        let Ok(set) = import(&s) else { continue };
        valid += 1;
        let text = export(&set, &opts());
        parse(&text).unwrap_or_else(|e| panic!("export produced invalid Sieve: {e}\ninput:\n{s}\noutput:\n{text}"));
        let again = import(&text).unwrap();
        assert_eq!(again.items, set.items, "model changed\ninput:\n{s}\noutput:\n{text}");
        assert_eq!(export(&again, &opts()), text, "export not stable\ninput:\n{s}");
    }
    assert!(valid > 1000, "too few valid mutations: {valid}");
}

#[test]
fn json_shape_matches_the_generated_typescript() {
    let set = import(
        "# rule:[d]\nif currentdate :value \"ge\" \"date\" \"2026-01-01\" { fileinto :copy \"A\"; vacation :days 1 \"x\"; }",
    )
    .unwrap();
    let json = serde_json::to_value(&set).unwrap();
    assert_eq!(
        json,
        serde_json::json!({
            "preamble": null,
            "requires": [],
            "singleFolder": false,
            "items": [{
                "type": "rule",
                "name": "d",
                "enabled": true,
                "conditions": { "join": "any", "items": [{
                    "negate": false,
                    "test": {
                        "type": "currentDate",
                        "zone": null,
                        "matcher": { "matchType": { "type": "value", "op": "ge" }, "comparator": null },
                        "datePart": "date",
                        "keys": ["2026-01-01"]
                    }
                }]},
                "actions": [
                    { "type": "fileInto", "mailbox": "A", "copy": true, "create": false, "flags": null },
                    { "type": "vacation", "period": { "unit": "days", "value": 1 }, "subject": null,
                      "from": null, "addresses": [], "mime": false, "handle": null, "reason": "x" }
                ]
            }]
        })
    );
    let back: RuleSet = serde_json::from_value(json).unwrap();
    assert_eq!(back, set);
}

/// What the editor writes for "Sender · domain · is (incl. subdomains)".
#[test]
fn address_parts_roundtrip() {
    let matcher = Matcher::new(MatchType::Matches);
    let test = ConditionTest::Address {
        headers: vec!["from".into()],
        part: AddressPart::Domain,
        matcher,
        keys: vec!["instagram.com".into(), "*.instagram.com".into()],
    };
    let set = RuleSet {
        items: vec![RuleItem::Rule(Rule {
            name: "Instagram".into(),
            enabled: true,
            conditions: Conditions { join: Join::All, items: vec![Condition { negate: false, test }] },
            actions: vec![Action::FileInto { mailbox: "Accounts/Instagram".into(), copy: false, create: false, flags: None }],
        })],
        ..Default::default()
    };
    let text = assert_stable(&set);
    assert!(
        text.contains(r#"address :domain :matches "from" ["instagram.com", "*.instagram.com"]"#),
        "{text}"
    );
}

/// Single-folder mode (scripts included by the root script): moves record
/// the folder in the global variable, "stop" ends only this script.
#[test]
fn single_folder_mode() {
    let src = r#"require ["fileinto", "imap4flags"];
# rule:[Work]
if header :contains "from" "boss@"
{
	fileinto "Work";
	addflag "\\Flagged";
}
# rule:[Copy]
if header :contains "subject" "invoice"
{
	fileinto :copy "Invoices";
	fileinto :create "New";
	stop;
}
"#;
    let mut set = import(src).unwrap();
    assert!(!set.single_folder);
    set.single_folder = true;
    let text = assert_stable(&set);
    assert_eq!(
        text,
        r#"require ["copy", "fileinto", "imap4flags", "include", "mailbox", "variables"];
global "move_to_folder";
# rule:[Work]
if header :contains "from" "boss@" {
	set "move_to_folder" "Work";
	addflag "\\Flagged";
}
# rule:[Copy]
if header :contains "subject" "invoice" {
	fileinto :copy "Invoices";
	fileinto :create "New";
	return;
}
"#
    );
    let again = import(&text).unwrap();
    assert!(again.single_folder);
    assert_eq!(again.items, set.items);

    // Back to normal mode gives plain moves again; unused extensions go.
    let mut normal = again;
    normal.single_folder = false;
    let text = export(&normal, &opts());
    assert!(text.starts_with("require [\"copy\", \"fileinto\", \"imap4flags\", \"mailbox\"];\n# rule:[Work]"), "{text}");
    assert!(text.contains("\tfileinto \"Work\";") && text.contains("\tstop;"), "{text}");
}

#[test]
fn single_folder_mode_without_moves_needs_no_fileinto() {
    let set = RuleSet {
        single_folder: true,
        items: vec![RuleItem::Rule(Rule {
            name: "Lists".into(),
            enabled: false,
            conditions: Conditions::default(),
            actions: vec![Action::FileInto { mailbox: "Lists".into(), copy: false, create: false, flags: None }],
        })],
        ..Default::default()
    };
    let text = assert_stable(&set);
    assert!(text.starts_with("require [\"include\", \"variables\"];\nglobal \"move_to_folder\";\n"), "{text}");
    assert!(text.contains("set \"move_to_folder\" \"Lists\";"), "{text}");
}

#[test]
fn single_folder_mode_reads_foreign_moves_and_stops() {
    // E.g. a rule added in Roundcube to an included script.
    let set = import(
        "require [\"fileinto\", \"include\", \"variables\"];\nglobal \"move_to_folder\";\nif true { fileinto \"A\"; stop; }\n",
    )
    .unwrap();
    assert!(set.single_folder);
    assert_eq!(rule(&set, 0).actions, vec![
        Action::FileInto { mailbox: "A".into(), copy: false, create: false, flags: None },
        Action::Stop,
    ]);
    let text = export(&set, &opts());
    assert!(text.contains("set \"move_to_folder\" \"A\";\n\treturn;"), "{text}");

    // Other variables, `global` elsewhere or with comments stay raw.
    let set = import("require [\"include\", \"variables\"];\nglobal \"move_to_folder\";\nif true { set \"other\" \"x\"; }\n").unwrap();
    assert!(set.single_folder);
    assert_eq!(raw(&set, 0), "if true { set \"other\" \"x\"; }");
    let set = import("require [\"include\", \"variables\"];\n# note\nglobal \"move_to_folder\";\n").unwrap();
    assert!(!set.single_folder);
    let set = import("require \"variables\";\nif true { set \"move_to_folder\" \"x\"; return; }\n").unwrap();
    assert!(!set.single_folder);
    assert!(raw(&set, 0).starts_with("if true"));
}
