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

/// Centers the main window on the monitor under the mouse cursor, then shows it.
///
/// Tauri's `center` always uses the window's initial monitor, which is the
/// primary display, so the app would open there even when launched from
/// another screen. The window starts hidden (`visible: false` in
/// tauri.conf.json) so it never flashes on the wrong monitor.
///
/// Everything is computed in logical points: on macOS each monitor reports
/// physical values in its own scale factor and the cursor in the primary
/// monitor's, so mixing physical values across displays lands off target.
fn place_on_cursor_monitor(window: &tauri::WebviewWindow) {
    let target = (|| {
        let primary_scale = window.primary_monitor().ok()??.scale_factor();
        let cursor = window
            .cursor_position()
            .ok()?
            .to_logical::<f64>(primary_scale);
        let window_size = window
            .outer_size()
            .ok()?
            .to_logical::<f64>(window.scale_factor().ok()?);
        window
            .available_monitors()
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
                Some(tauri::LogicalPosition::new(
                    area_pos.x + ((area_size.width - window_size.width) / 2.0).max(0.0),
                    area_pos.y + ((area_size.height - window_size.height) / 2.0).max(0.0),
                ))
            })
    })();
    if let Some(position) = target {
        let _ = window.set_position(position);
    }
    let _ = window.show();
    let _ = window.set_focus();
}

// ── Entry point ───────────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            use tauri::Manager;
            if let Some(window) = app.get_webview_window("main") {
                place_on_cursor_monitor(&window);
            }
            Ok(())
        })
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
            accounts::list_accounts,
            accounts::save_account,
            accounts::delete_account,
            accounts::trust_certificate,
            accounts::set_keyword_names,
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
