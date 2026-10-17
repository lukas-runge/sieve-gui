//! Conversion between Sieve text and rule cards.

use sieve_core::compat::RoundcubeRisks;
use sieve_core::model::RuleSet;
use sieve_core::root::Root;
use sieve_core::PrintOptions;

use crate::error::AppResult;

/// Parses a script into rule cards.
#[tauri::command]
pub fn import_rules(source: String) -> AppResult<RuleSet> {
    Ok(sieve_core::import(&source)?)
}

/// Generates the script for rule cards: in the layout Roundcube uses if the
/// account's filters are also edited there (smallest diffs), else in
/// sieve-gui's readable layout.
#[tauri::command]
pub fn export_rules(rules: RuleSet, roundcube: bool) -> String {
    let options = if roundcube { PrintOptions::roundcube() } else { PrintOptions::readable() };
    sieve_core::export(&rules, &options)
}

/// What Roundcube would drop of these rules the next time it saves them.
#[tauri::command]
pub fn roundcube_risks(rules: RuleSet) -> RoundcubeRisks {
    sieve_core::compat::roundcube_risks(&rules)
}

/// The root script that includes `scripts` in this order (see
/// `sieve_core::root`).
#[tauri::command]
pub fn root_script(scripts: Vec<String>, last_move_wins: bool) -> String {
    sieve_core::root::root_script(&scripts, last_move_wins, &PrintOptions::readable())
}

/// What a root script does, or `None` if `source` is no root.
#[tauri::command]
pub fn parse_root(source: String) -> Option<Root> {
    sieve_core::root::parse_root(&source)
}
