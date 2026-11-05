//! Updates on Linux. macOS and Windows get theirs from Homebrew and winget;
//! on Linux the app was installed by `scripts/install.sh`, and runs it again
//! to update: it quits, the script installs the newest release and starts the
//! app again (see the script).

use serde::Serialize;
use tauri::AppHandle;

use crate::error::{AppError, AppResult, ErrorKind};

/// How the running app was installed, if it can update itself.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum UpdateChannel {
    /// A .deb or .rpm package: the update asks for the admin password.
    Package,
    /// An AppImage: the update replaces the file.
    AppImage,
}

/// `None` on macOS and Windows, and on Linux for builds that weren't
/// installed from a release (e.g. `pnpm tauri dev`).
#[tauri::command]
pub fn update_channel() -> Option<UpdateChannel> {
    channel()
}

#[cfg(target_os = "linux")]
fn channel() -> Option<UpdateChannel> {
    if std::env::var_os("APPIMAGE").is_some() {
        return Some(UpdateChannel::AppImage);
    }
    let exe = std::env::current_exe().ok()?;
    (exe == std::path::Path::new("/usr/bin/sieve-gui")).then_some(UpdateChannel::Package)
}

#[cfg(not(target_os = "linux"))]
fn channel() -> Option<UpdateChannel> {
    None
}

/// Starts the update and quits the app; the script starts it again.
#[tauri::command]
pub fn install_update(app: AppHandle) -> AppResult<()> {
    let channel = channel().ok_or_else(|| AppError::new(ErrorKind::Config, "this build can't update itself"))?;
    spawn_script(channel)?;
    app.exit(0);
    Ok(())
}

#[cfg(target_os = "linux")]
fn spawn_script(channel: UpdateChannel) -> AppResult<()> {
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};

    const SCRIPT: &str = include_str!("../../scripts/install.sh");
    let error = |e: std::io::Error| AppError::new(ErrorKind::Config, format!("update: {e}"));

    // Outside the AppImage's mount, which goes away when the app quits.
    let path = std::env::temp_dir().join(format!("sieve-gui-update-{}.sh", std::process::id()));
    std::fs::write(&path, SCRIPT).map_err(error)?;

    let mut command = Command::new("/bin/sh");
    command
        .arg(&path)
        .arg("--update")
        .arg(std::process::id().to_string())
        .current_dir("/")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        // Its own process group: it outlives the app.
        .process_group(0);
    if channel == UpdateChannel::AppImage {
        command.arg("--appimage");
        if let Some(appimage) = std::env::var_os("APPIMAGE") {
            command.env("SIEVE_GUI_APPIMAGE", appimage);
        }
        strip_appimage_env(&mut command);
    }
    command.spawn().map_err(error)?;
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn spawn_script(_: UpdateChannel) -> AppResult<()> {
    Err(AppError::new(ErrorKind::Config, "updates come from Homebrew or winget"))
}

/// The AppImage's runtime points libraries, GTK modules and data dirs into
/// its mount. Neither the package tools nor the next version must see that.
#[cfg(target_os = "linux")]
fn strip_appimage_env(command: &mut std::process::Command) {
    for name in ["APPIMAGE", "APPDIR", "ARGV0", "OWD"] {
        command.env_remove(name);
    }
    let Some(appdir) = std::env::var_os("APPDIR") else { return };
    let appdir = appdir.to_string_lossy().into_owned();
    if appdir.is_empty() {
        return;
    }
    for (name, value) in std::env::vars_os() {
        let value = value.to_string_lossy();
        if !value.contains(&appdir) {
            continue;
        }
        // Lists like XDG_DATA_DIRS keep the entries from outside the mount.
        let kept: Vec<&str> = value.split(':').filter(|part| !part.is_empty() && !part.contains(&appdir)).collect();
        if kept.is_empty() {
            command.env_remove(&name);
        } else {
            command.env(&name, kept.join(":"));
        }
    }
}
