//! The rule model: a script as an ordered list of rule cards.
//!
//! This is what the app edits. [`crate::import`] maps a parsed script onto it
//! (anything it cannot represent becomes a [`RawBlock`]) and
//! [`crate::export`] turns it back into Sieve text.

use serde::{Deserialize, Serialize};

/// A script viewed as rule cards.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleSet {
    /// Comment block at the top of the script, kept verbatim (without the
    /// trailing line break).
    pub preamble: Option<String>,
    /// Extensions listed in the original `require` commands. The exporter
    /// keeps these (unless they are known and unused) and adds whatever the
    /// items need.
    pub requires: Vec<String>,
    pub items: Vec<RuleItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum RuleItem {
    Rule(Rule),
    Raw(RawBlock),
}

/// A top-level construct the model cannot represent, kept verbatim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawBlock {
    /// Sieve source, including its comments, without a trailing line break.
    pub code: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    pub name: String,
    pub enabled: bool,
    pub conditions: Conditions,
    pub actions: Vec<Action>,
}

/// The tests of a rule. An empty list means "all messages".
///
/// With a single condition, `join` still matters for the written form:
/// `All` gives `allof (test)`, `Any` the bare test (as Roundcube does).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Conditions {
    pub join: Join,
    pub items: Vec<Condition>,
}

/// How several conditions combine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Join {
    /// `allof`
    #[default]
    All,
    /// `anyof`
    Any,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Condition {
    /// Wrapped in `not`.
    pub negate: bool,
    pub test: ConditionTest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ConditionTest {
    /// `header [:comparator] [match] <headers> <keys>`
    Header { headers: Vec<String>, matcher: Matcher, keys: Vec<String> },
    /// `address [:comparator] [address-part] [match] <headers> <keys>`
    Address { headers: Vec<String>, part: AddressPart, matcher: Matcher, keys: Vec<String> },
    /// `envelope [:comparator] [address-part] [match] <parts> <keys>` (RFC 5228 §5.4)
    Envelope { parts: Vec<String>, part: AddressPart, matcher: Matcher, keys: Vec<String> },
    /// `exists <headers>`
    Exists { headers: Vec<String> },
    /// `size :over|:under <limit>`
    Size { over: bool, limit: u64 },
    /// `body [:comparator] [match] [transform] <keys>` (RFC 5173)
    Body { transform: BodyTransform, matcher: Matcher, keys: Vec<String> },
    /// `currentdate [:zone] [:comparator] [match] <date-part> <keys>` (RFC 5260)
    CurrentDate {
        zone: Option<String>,
        matcher: Matcher,
        // Explicit rename: tauri-ts-generator ignores variant-level rename_all.
        #[serde(rename = "datePart")]
        date_part: String,
        keys: Vec<String>,
    },
    /// `date [:zone | :originalzone] [:comparator] [match] <header> <date-part> <keys>`
    Date {
        zone: DateZone,
        matcher: Matcher,
        header: String,
        #[serde(rename = "datePart")]
        date_part: String,
        keys: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "camelCase")]
pub enum DateZone {
    /// No zone argument: the server's local time zone.
    Local,
    /// `:zone "+0200"`
    Zone(String),
    /// `:originalzone`
    Original,
}

/// Match type and comparator of a test.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Matcher {
    pub match_type: MatchType,
    /// `None` means the default `i;ascii-casemap` (no `:comparator` argument).
    pub comparator: Option<String>,
}

impl Matcher {
    pub fn new(match_type: MatchType) -> Self {
        Self { match_type, comparator: None }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "op", rename_all = "camelCase")]
pub enum MatchType {
    Is,
    Contains,
    Matches,
    /// draft-ietf-sieve-regex
    Regex,
    /// `:value "<op>"` (RFC 5231)
    Value(RelOp),
    /// `:count "<op>"` (RFC 5231)
    Count(RelOp),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RelOp {
    Gt,
    Ge,
    Lt,
    Le,
    Eq,
    Ne,
}

impl RelOp {
    pub fn as_str(self) -> &'static str {
        match self {
            RelOp::Gt => "gt",
            RelOp::Ge => "ge",
            RelOp::Lt => "lt",
            RelOp::Le => "le",
            RelOp::Eq => "eq",
            RelOp::Ne => "ne",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s.to_ascii_lowercase().as_str() {
            "gt" => RelOp::Gt,
            "ge" => RelOp::Ge,
            "lt" => RelOp::Lt,
            "le" => RelOp::Le,
            "eq" => RelOp::Eq,
            "ne" => RelOp::Ne,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AddressPart {
    #[default]
    All,
    LocalPart,
    Domain,
    /// RFC 5233 subaddress
    User,
    /// RFC 5233 subaddress
    Detail,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "camelCase")]
pub enum BodyTransform {
    /// `:text` (the default)
    #[default]
    Text,
    /// `:raw`
    Raw,
    /// `:content [types]`
    Content(Vec<String>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Action {
    /// `fileinto [:copy] [:create] [:flags] <mailbox>`
    FileInto { mailbox: String, copy: bool, create: bool, flags: Option<Vec<String>> },
    /// `redirect [:copy] <address>`
    Redirect { address: String, copy: bool },
    /// `reject <reason>` or `ereject <reason>` (RFC 5429)
    Reject { reason: String, extended: bool },
    Discard,
    /// `keep [:flags]`
    Keep { flags: Option<Vec<String>> },
    Stop,
    /// `addflag` / `setflag` / `removeflag` (RFC 5232)
    Flags { op: FlagOp, flags: Vec<String> },
    Vacation(Vacation),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FlagOp {
    Add,
    Set,
    Remove,
}

impl FlagOp {
    pub fn command(self) -> &'static str {
        match self {
            FlagOp::Add => "addflag",
            FlagOp::Set => "setflag",
            FlagOp::Remove => "removeflag",
        }
    }
}

/// `vacation` (RFC 5230, RFC 6131)
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Vacation {
    pub period: Option<VacationPeriod>,
    pub subject: Option<String>,
    pub from: Option<String>,
    pub addresses: Vec<String>,
    pub mime: bool,
    pub handle: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "unit", content = "value", rename_all = "camelCase")]
pub enum VacationPeriod {
    Days(u64),
    /// `:seconds` (RFC 6131, `vacation-seconds`)
    Seconds(u64),
}

/// Extensions known to the model, in canonical order.
pub(crate) const KNOWN_EXTENSIONS: &[&str] = &[
    "fileinto",
    "reject",
    "ereject",
    "envelope",
    "body",
    "copy",
    "mailbox",
    "imap4flags",
    "relational",
    "regex",
    "subaddress",
    "date",
    "vacation",
    "vacation-seconds",
    "comparator-i;ascii-numeric",
];

impl RuleSet {
    /// Extensions needed by the rules (not raw blocks), in canonical order.
    pub fn needed_extensions(&self) -> Vec<&'static str> {
        let mut needed = Vec::new();
        for item in &self.items {
            if let RuleItem::Rule(rule) = item {
                rule.collect_extensions(&mut needed);
            }
        }
        KNOWN_EXTENSIONS.iter().copied().filter(|e| needed.contains(e)).collect()
    }
}

impl Rule {
    pub(crate) fn collect_extensions(&self, out: &mut Vec<&'static str>) {
        for c in &self.conditions.items {
            c.test.collect_extensions(out);
        }
        for a in &self.actions {
            a.collect_extensions(out);
        }
    }
}

impl ConditionTest {
    fn collect_extensions(&self, out: &mut Vec<&'static str>) {
        let (matcher, part) = match self {
            ConditionTest::Header { matcher, .. } => (Some(matcher), None),
            ConditionTest::Address { matcher, part, .. } => (Some(matcher), Some(part)),
            ConditionTest::Envelope { matcher, part, .. } => {
                out.push("envelope");
                (Some(matcher), Some(part))
            }
            ConditionTest::Exists { .. } | ConditionTest::Size { .. } => (None, None),
            ConditionTest::Body { matcher, .. } => {
                out.push("body");
                (Some(matcher), None)
            }
            ConditionTest::CurrentDate { matcher, .. } | ConditionTest::Date { matcher, .. } => {
                out.push("date");
                (Some(matcher), None)
            }
        };
        if let Some(AddressPart::User | AddressPart::Detail) = part {
            out.push("subaddress");
        }
        if let Some(m) = matcher {
            match m.match_type {
                MatchType::Regex => out.push("regex"),
                MatchType::Value(_) | MatchType::Count(_) => out.push("relational"),
                _ => {}
            }
            if m.comparator.as_deref().is_some_and(|c| c.eq_ignore_ascii_case("i;ascii-numeric")) {
                out.push("comparator-i;ascii-numeric");
            }
        }
    }
}

impl Action {
    fn collect_extensions(&self, out: &mut Vec<&'static str>) {
        match self {
            Action::FileInto { copy, create, flags, .. } => {
                out.push("fileinto");
                if *copy {
                    out.push("copy");
                }
                if *create {
                    out.push("mailbox");
                }
                if flags.is_some() {
                    out.push("imap4flags");
                }
            }
            Action::Redirect { copy, .. } => {
                if *copy {
                    out.push("copy");
                }
            }
            Action::Reject { extended, .. } => out.push(if *extended { "ereject" } else { "reject" }),
            Action::Keep { flags } => {
                if flags.is_some() {
                    out.push("imap4flags");
                }
            }
            Action::Flags { .. } => out.push("imap4flags"),
            Action::Vacation(v) => {
                out.push("vacation");
                if matches!(v.period, Some(VacationPeriod::Seconds(_))) {
                    out.push("vacation-seconds");
                }
            }
            Action::Discard | Action::Stop => {}
        }
    }
}
