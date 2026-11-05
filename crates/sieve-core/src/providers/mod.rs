//! Filter providers (concept §3, decision D19).
//!
//! Some systems write Sieve themselves from their own filter settings: a
//! webmail, a NAS mail server, a hosting panel. They rewrite their scripts
//! on every save and may make their own script the active one again, so
//! their filters can't run next to sieve-gui's. Instead sieve-gui takes them
//! over: a provider reads its filters in its own format and turns them into
//! ordinary rules.
//!
//! Each provider implements [`Provider`]; [`detect`] and [`import`] go
//! through all of them. Nothing outside a provider's module knows about the
//! product.

use serde::{Deserialize, Serialize};

use crate::ast::{Argument, Command};
use crate::model::RuleSet;
use crate::ParseError;

mod mailplus;

/// A script on the server, by name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderScript {
    pub name: String,
    pub source: String,
}

/// A provider's setup found on an account.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Detection {
    /// Which provider found it ([`Provider::id`]), e.g. `mailplus`.
    pub provider: String,
    /// The provider's root script: the one it makes active.
    pub root: String,
    /// The scripts holding its filters, which `root` includes.
    pub scripts: Vec<String>,
    /// Filters set up in the provider's UI.
    pub filters: usize,
    /// Entries of its personal block and allow lists.
    pub list_entries: usize,
    /// Saving in the provider's UI rewrites its scripts and makes `root` the
    /// active script again.
    pub reactivates_root: bool,
    /// Changes when the filters change (not with comments or layout), to
    /// tell filters added after a take-over from those taken over.
    pub fingerprint: String,
}

impl Detection {
    /// Whether there is anything to take over.
    pub fn has_rules(&self) -> bool {
        self.filters + self.list_entries > 0
    }
}

/// Names for the rules a provider creates, in the app's language.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleNames {
    /// A filter, followed by its number if it has one.
    pub filter: String,
    /// An allow list entry, followed by its addresses.
    pub allow_list: String,
    /// A block list entry, followed by its addresses.
    pub block_list: String,
}

impl Default for RuleNames {
    fn default() -> Self {
        Self { filter: "Filter".to_owned(), allow_list: "Allow list".to_owned(), block_list: "Block list".to_owned() }
    }
}

/// A system that writes Sieve from its own filter settings.
pub trait Provider: Sync {
    /// A stable identifier, used in [`Detection::provider`] and by the app
    /// for its texts.
    fn id(&self) -> &'static str;
    /// Finds the provider's setup among the account's scripts, by their
    /// content.
    fn detect(&self, scripts: &[ProviderScript]) -> Option<Detection>;
    /// The provider's filters as rules, in the order they should run.
    /// Anything that can't be mapped stays a code card.
    fn import(&self, scripts: &[ProviderScript], names: &RuleNames) -> Result<RuleSet, ParseError>;
}

static PROVIDERS: &[&dyn Provider] = &[&mailplus::MailPlus];

/// The setups of all providers found among the account's scripts.
pub fn detect(scripts: &[ProviderScript]) -> Vec<Detection> {
    PROVIDERS.iter().filter_map(|p| p.detect(scripts)).collect()
}

/// The filters of `provider` as rules, or `None` for an unknown provider.
pub fn import(provider: &str, scripts: &[ProviderScript], names: &RuleNames) -> Option<Result<RuleSet, ParseError>> {
    PROVIDERS.iter().find(|p| p.id() == provider).map(|p| p.import(scripts, names))
}

// ── Helpers for providers ─────────────────────────────────────────────────────

/// A short hash of what the scripts do (FNV-1a over their canonical form),
/// the same across app versions and platforms.
fn fingerprint<'a>(sources: impl IntoIterator<Item = &'a str>) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for source in sources {
        let text = crate::canonical(source).unwrap_or_else(|| source.to_owned());
        for byte in text.bytes().chain([0]) {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
    }
    format!("{hash:016x}")
}

/// The string arguments of a command, in order.
fn strings(cmd: &Command) -> Vec<&str> {
    cmd.arguments
        .args
        .iter()
        .filter_map(|a| match a {
            Argument::String(s) => Some(s.value.as_str()),
            _ => None,
        })
        .collect()
}

/// Calls `f` on every command, depth first.
fn walk<'a>(commands: &'a [Command], f: &mut impl FnMut(&'a Command)) {
    for cmd in commands {
        f(cmd);
        if let Some(block) = &cmd.block {
            walk(&block.commands, f);
        }
    }
}

/// Whether any command (at any depth) matches.
fn any(commands: &[Command], mut f: impl FnMut(&Command) -> bool) -> bool {
    let mut found = false;
    walk(commands, &mut |c| found = found || f(c));
    found
}
