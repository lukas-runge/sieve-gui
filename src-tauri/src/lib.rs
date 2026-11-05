mod accounts;
mod copy;
mod error;
mod folders;
mod passwords;
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

// ── Window placement ──────────────────────────────────────────────────────────

/// Top-left corner, in logical points, that centers a window of
/// `width`×`height` in the work area of the monitor under the mouse cursor.
///
/// Tauri's `center` uses the primary display (the one with the menu bar), so
/// the app would open there even when launched from another screen.
///
/// Everything is computed in logical points: on macOS each monitor reports
/// physical values in its own scale factor and the cursor in the primary
/// monitor's, so mixing physical values across displays lands off target.
fn center_on_cursor_monitor(app: &tauri::AppHandle, width: f64, height: f64) -> Option<(f64, f64)> {
    let primary_scale = app.primary_monitor().ok()??.scale_factor();
    let cursor = app.cursor_position().ok()?.to_logical::<f64>(primary_scale);
    app.available_monitors()
        .ok()?
        .into_iter()
        .find_map(|monitor| {
            let scale = monitor.scale_factor();
            let pos = monitor.position().to_logical::<f64>(scale);
            let size = monitor.size().to_logical::<f64>(scale);
            let inside = (pos.x..pos.x + size.width).contains(&cursor.x)
                && (pos.y..pos.y + size.height).contains(&cursor.y);
            if !inside {
                return None;
            }
            let area = monitor.work_area();
            let area_pos = area.position.to_logical::<f64>(scale);
            let area_size = area.size.to_logical::<f64>(scale);
            Some((
                area_pos.x + ((area_size.width - width) / 2.0).max(0.0),
                area_pos.y + ((area_size.height - height) / 2.0).max(0.0),
            ))
        })
}

/// Builds the main window from its config (`create: false` in
/// tauri.conf.json), already placed on the monitor under the cursor.
///
/// The position has to be known when the window is created: moving it
/// afterwards is asynchronous on macOS, so the window would flash on the
/// primary display first.
fn create_main_window(app: &tauri::App) -> tauri::Result<()> {
    let Some(mut config) = app.config().app.windows.first().cloned() else {
        return Ok(());
    };
    if let Some((x, y)) = center_on_cursor_monitor(app.handle(), config.width, config.height) {
        config.center = false;
        config.x = Some(x);
        config.y = Some(y);
    }
    tauri::WebviewWindowBuilder::from_config(app, &config)?.build()?;
    Ok(())
}

// ── Entry point ───────────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| Ok(create_main_window(app)?))
        .manage(accounts::AccountStore::default())
        .manage(server::Sessions::default())
        .invoke_handler(tauri::generate_handler![
            get_app_info,
            rules::script_content,
            rules::import_rules,
            rules::import_rules_single_folder,
            rules::export_rules,
            rules::roundcube_risks,
            rules::root_script,
            rules::parse_root,
            rules::script_links,
            rules::detect_providers,
            rules::import_provider,
            accounts::list_accounts,
            accounts::save_account,
            accounts::delete_account,
            accounts::trust_certificate,
            accounts::set_keyword_names,
            accounts::set_filters_alone,
            server::connect_account,
            server::disconnect_account,
            server::list_scripts,
            server::get_script,
            server::check_script,
            server::deploy_script,
            server::activate_script,
            server::rename_script,
            server::delete_script,
            server::copy_status,
            server::resolve_copy,
            folders::list_folders,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
