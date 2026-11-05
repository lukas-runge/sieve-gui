//! Conversion between Sieve text and rule cards.

use sieve_core::compat::RoundcubeRisks;
use sieve_core::model::RuleSet;
use sieve_core::providers::{Detection, ProviderScript, RuleNames};
use sieve_core::root::{Root, ScriptLinks};
use sieve_core::PrintOptions;

use crate::error::{AppError, AppResult, ErrorKind};

/// What the script does, without comments, layout and equal spellings (see
/// `sieve_core::content`), to show only the changes that matter; `None` if
/// it doesn't parse.
#[tauri::command]
pub fn script_content(source: String) -> Option<String> {
    sieve_core::content(&source)
}

/// Parses a script into rule cards.
#[tauri::command]
pub fn import_rules(source: String) -> AppResult<RuleSet> {
    Ok(sieve_core::import(&source)?)
}

/// Parses a script that the root runs in single-folder mode, even if it lost
/// its `global` command since (see `sieve_core::import_single_folder`).
#[tauri::command]
pub fn import_rules_single_folder(source: String) -> AppResult<RuleSet> {
    Ok(sieve_core::import_single_folder(&source)?)
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

/// The root script that includes `scripts` in this order, those in
/// `unchanged` marked as not to be rewritten (see `sieve_core::root`).
#[tauri::command]
pub fn root_script(scripts: Vec<String>, unchanged: Vec<String>, last_move_wins: bool) -> String {
    sieve_core::root::root_script(&scripts, &unchanged, last_move_wins, &PrintOptions::readable())
}

/// Which scripts a script includes and whether it moves messages itself.
#[tauri::command]
pub fn script_links(source: String) -> ScriptLinks {
    sieve_core::root::script_links(&source)
}

/// What a root script does, or `None` if `source` is no root.
#[tauri::command]
pub fn parse_root(source: String) -> Option<Root> {
    sieve_core::root::parse_root(&source)
}

/// The setups of filter providers found among the account's scripts (see
/// `sieve_core::providers`).
#[tauri::command]
pub fn detect_providers(scripts: Vec<ProviderScript>) -> Vec<Detection> {
    sieve_core::providers::detect(&scripts)
}

/// A filter provider's filters as rules, to take them over, named in the
/// app's language.
#[tauri::command]
pub fn import_provider(provider: String, scripts: Vec<ProviderScript>, names: RuleNames) -> AppResult<RuleSet> {
    match sieve_core::providers::import(&provider, &scripts, &names) {
        Some(rules) => Ok(rules?),
        None => Err(AppError::new(ErrorKind::NotFound, format!("unknown filter provider {provider}"))),
    }
}
