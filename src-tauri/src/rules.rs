//! Conversion between Sieve text and rule cards.

use sieve_core::model::RuleSet;
use sieve_core::PrintOptions;

use crate::error::AppResult;

/// Parses a script into rule cards.
#[tauri::command]
pub fn import_rules(source: String) -> AppResult<RuleSet> {
    Ok(sieve_core::import(&source)?)
}

/// Generates the script for rule cards, in the layout Roundcube uses.
#[tauri::command]
pub fn export_rules(rules: RuleSet) -> String {
    sieve_core::export(&rules, &PrintOptions::roundcube())
}
