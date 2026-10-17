//! Round-trip properties of the syntax layer:
//! `parse(print(parse(src))) == parse(src)` (ignoring spans) and printing is
//! idempotent. Also feeds mutated scripts to the parser to make sure it never
//! panics.

mod common;

use common::roundtrip;
use sieve_core::printer::LineEnding;
use sieve_core::{parse, PrintOptions};

const CORPUS: &[&str] = &[
    // RFC 5228 §9 example, slightly extended
    r#"#
# Example Sieve Filter
# Declare any optional features or extension used by the script
#
require ["fileinto"];

#
# Handle messages from known mailing lists
# Move messages from IETF filter discussion list to filter mailbox
#
if header :is "Sender" "owner-ietf-mta-filters@imc.org"
        {
        fileinto "filter";  # move to "filter" mailbox
        }
#
# Keep all messages to or from people in my company
#
elsif address :DOMAIN :is ["From", "To"] "example.com"
        {
        keep;               # keep in "In" mailbox
        }

#
# Try and catch unsolicited email.  If a message is not to me,
# or it contains a subject known to be spam, file it away.
#
elsif anyof (NOT address :all :contains
               ["To", "Cc", "Bcc"] "me@example.com",
             header :matches "subject"
               ["*make*money*fast*", "*university*dipl*mas*"])
        {
        fileinto "spam";   # move to "spam" mailbox
        }
else
        {
        # Move all other (non-company) mail to "personal"
        # mailbox.
        fileinto "personal";
        }
"#,
    r#"require ["vacation", "date", "relational", "variables"];
if allof (currentdate :value "ge" "date" "2026-07-01",
          currentdate :value "le" "date" "2026-07-21") {
    vacation :days 7 :subject "Out of office: ${1}"
        :addresses ["me@example.com", "me@example.org"] text:
Hallo,

ich bin bis zum 21. Juli nicht erreichbar.
..
Grüße
.
;
}
"#,
    r#"require ["imap4flags", "envelope", "subaddress", "body", "copy", "regex", "editheader", "mailbox"];
if envelope :detail "to" "newsletters" { fileinto :create "Newsletters"; }
if body :text :contains "unsubscribe" { addflag "$Newsletter"; }
if header :regex "subject" "^\\[[a-z-]+\\]" { redirect :copy "archive@example.com"; }
if size :over 500K { discard; stop; }
if not exists ["List-Id", "List-Unsubscribe"] { keep; } else { setflag ["\\Seen", "\\Flagged"]; }
addheader :last "X-Filtered" "yes";
/* multi
   line comment */
reject text:
no thanks
.
;
"#,
    // Roundcube-like layout with metadata comments and a disabled rule
    "require [\"fileinto\"];\r\n# rule:[Steuer-Bescheide äöü]\r\nif header :contains \"from\" \"@finanzamt.example\"\r\n{\r\n\tfileinto \"Taxes\";\r\n\tstop;\r\n}\r\n# rule:[disabled one]\r\nif false # anyof (header :contains \"subject\" \"x\", true)\r\n{\r\n\tdiscard;\r\n}\r\n",
    "",
    "# only a comment",
    "keep;",
];

#[test]
fn corpus_roundtrips() {
    let lf = PrintOptions::default();
    let crlf = PrintOptions { line_ending: LineEnding::CrLf, indent: "    ".into(), max_width: 40, ..Default::default() };
    for src in CORPUS {
        roundtrip(src, &lf);
        roundtrip(src, &crlf);
        roundtrip(src, &PrintOptions::roundcube());
        roundtrip(src, &PrintOptions::readable());
    }
}

/// Small deterministic PRNG so the test needs no extra dependency.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.0 >> 33
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

#[test]
fn mutated_scripts_never_panic_and_roundtrip_when_valid() {
    const ALPHABET: &[&str] = &[
        "\"", "\\", "[", "]", "(", ")", "{", "}", ",", ";", ":", "#", "/*", "*/", "\n", "\r\n",
        " ", "text:", ".", "..", "1", "K", "a", "if", "ä", "€",
    ];
    let options = PrintOptions::default();
    // SIEVE_FUZZ_ITERS / SIEVE_FUZZ_SEED allow longer local runs.
    let env = |k: &str, d: u64| std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d);
    let iterations = env("SIEVE_FUZZ_ITERS", 20_000);
    let mut rng = Lcg(env("SIEVE_FUZZ_SEED", 0x5eed));
    let mut valid = 0;
    for _ in 0..iterations {
        let base = CORPUS[rng.below(CORPUS.len())];
        let mut chars: Vec<char> = base.chars().collect();
        for _ in 0..=rng.below(3) {
            let at = rng.below(chars.len() + 1);
            if rng.below(2) == 0 && at < chars.len() {
                chars.remove(at);
            } else {
                let ins = ALPHABET[rng.below(ALPHABET.len())];
                chars.splice(at..at, ins.chars());
            }
        }
        let s: String = chars.into_iter().collect();
        if parse(&s).is_ok() {
            valid += 1;
            roundtrip(&s, &options);
        }
    }
    assert!(valid > 1000, "mutations should regularly stay valid, got {valid}");
}
