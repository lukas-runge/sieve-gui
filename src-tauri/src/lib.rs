mod accounts;
mod error;
mod folders;
mod rules;
mod server;

// ── Shared types ──────────────────────────────────────────────────────────────

#[derive(serde::Serialize)]
struct AppInfo {
    name: String,
    version: String,
}

// ── Commands ──────────────────────────────────────────────────────────────────

#[tauri::command]
fn get_app_info() -> AppInfo {
    AppInfo {
        name: env!("CARGO_PKG_NAME").to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    }
}

// ── Entry point ───────────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(accounts::AccountStore::default())
        .manage(server::Sessions::default())
        .invoke_handler(tauri::generate_handler![
            get_app_info,
            rules::import_rules,
            rules::export_rules,
            rules::is_roundcube_format,
            rules::roundcube_risks,
            accounts::list_accounts,
            accounts::save_account,
            accounts::delete_account,
            server::connect_account,
            server::disconnect_account,
            server::list_scripts,
            server::get_script,
            server::check_script,
            server::deploy_script,
            server::activate_script,
            server::rename_script,
            server::delete_script,
            folders::list_folders,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
