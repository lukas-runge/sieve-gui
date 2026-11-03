//! ManageSieve sessions and the commands that use them.

use std::collections::{BTreeMap, HashMap};

use managesieve::{Capabilities, Client, ConnectOptions, Credentials, Diagnostic, ScriptInfo};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};
use tokio::sync::Mutex;

use crate::accounts::{self, Account, AccountStore};
use crate::copy::{self, CopyStatus, LocalCopy, Side};
use crate::error::{AppError, AppResult, ErrorKind};

/// One open connection per account, reused across commands.
#[derive(Default)]
pub struct Sessions {
    clients: Mutex<HashMap<String, Client>>,
}

impl Sessions {
    pub async fn disconnect(&self, id: &str) {
        if let Some(client) = self.clients.lock().await.remove(id) {
            let _ = client.logout().await;
        }
    }
}

async fn connect(account: &Account) -> AppResult<Client> {
    let opts = ConnectOptions::new(&account.host)
        .port(account.port)
        .security(account.security.into())
        .pinned_certificates(account.certificate.iter());
    let mut client = Client::connect(&opts)
        .await
        .map_err(|e| AppError::from_sieve(e, false).with_pinned(account.certificate.as_ref()))?;
    let credentials = Credentials::new(&account.username, accounts::password(&account.id)?);
    client.authenticate(&credentials).await.map_err(|e| AppError::from_sieve(e, true))?;
    Ok(client)
}

/// Runs a client operation on the account's session, connecting first if
/// needed. If the connection turns out to be dead (servers drop idle
/// sessions), it reconnects once and retries.
macro_rules! with_client {
    ($app:expr, $store:expr, $sessions:expr, $id:expr, |$client:ident| $body:expr) => {{
        let account = $store.get(&$app, &$id)?;
        let mut clients = $sessions.clients.lock().await;
        let mut attempt = 0;
        loop {
            attempt += 1;
            if !clients.contains_key(&account.id) {
                let client = connect(&account).await?;
                clients.insert(account.id.clone(), client);
            }
            let $client = clients.get_mut(&account.id).expect("inserted above");
            match $body {
                Ok(value) => break Ok::<_, AppError>(value),
                Err(e) if e.is_fatal() => {
                    clients.remove(&account.id);
                    if attempt >= 2 {
                        break Err(AppError::from_sieve(e, false));
                    }
                }
                Err(e) => break Err(AppError::from_sieve(e, false)),
            }
        }
    }};
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerInfo {
    pub capabilities: Capabilities,
    pub tls: bool,
    pub scripts: Vec<ScriptInfo>,
}

/// Connects (if needed) and returns capabilities and the script list.
#[tauri::command]
pub async fn connect_account(
    app: AppHandle,
    store: State<'_, AccountStore>,
    sessions: State<'_, Sessions>,
    id: String,
) -> AppResult<ServerInfo> {
    with_client!(app, store, sessions, id, |client| {
        match client.list_scripts().await {
            Ok(scripts) => Ok(ServerInfo { capabilities: client.capabilities().clone(), tls: client.is_tls(), scripts }),
            Err(e) => Err(e),
        }
    })
}

#[tauri::command]
pub async fn disconnect_account(sessions: State<'_, Sessions>, id: String) -> AppResult<()> {
    sessions.disconnect(&id).await;
    Ok(())
}

#[tauri::command]
pub async fn list_scripts(
    app: AppHandle,
    store: State<'_, AccountStore>,
    sessions: State<'_, Sessions>,
    id: String,
) -> AppResult<Vec<ScriptInfo>> {
    with_client!(app, store, sessions, id, |client| client.list_scripts().await)
}

#[tauri::command]
pub async fn get_script(
    app: AppHandle,
    store: State<'_, AccountStore>,
    sessions: State<'_, Sessions>,
    id: String,
    name: String,
) -> AppResult<String> {
    with_client!(app, store, sessions, id, |client| client.get_script(&name).await)
}

/// Asks the server to compile a script without storing it.
#[tauri::command]
pub async fn check_script(
    app: AppHandle,
    store: State<'_, AccountStore>,
    sessions: State<'_, Sessions>,
    id: String,
    content: String,
) -> AppResult<Vec<Diagnostic>> {
    with_client!(app, store, sessions, id, |client| client.check_script(&content).await)
        .map(|outcome| outcome.diagnostics())
}

/// Uploads a script. If `expected` is given, the upload only happens when
/// the script on the server still has that content (ManageSieve has no
/// ETags, so the content itself is compared). Returns server warnings.
#[tauri::command]
pub async fn deploy_script(
    app: AppHandle,
    store: State<'_, AccountStore>,
    sessions: State<'_, Sessions>,
    id: String,
    name: String,
    content: String,
    expected: Option<String>,
) -> AppResult<Vec<Diagnostic>> {
    if let Some(expected) = expected {
        let current = with_client!(app, store, sessions, id, |client| client.get_script(&name).await)?;
        if current != expected {
            return Err(AppError::new(
                ErrorKind::Conflict,
                "the script was changed on the server since it was loaded",
            ));
        }
    }
    with_client!(app, store, sessions, id, |client| client.check_script(&content).await)?;
    let diagnostics = with_client!(app, store, sessions, id, |client| client.put_script(&name, &content).await)
        .map(|outcome| outcome.diagnostics())?;
    record(&app, &store, &id, |c| c.put(&name, &content));
    Ok(diagnostics)
}

/// Makes a script the active one, or deactivates all scripts for `None`.
#[tauri::command]
pub async fn activate_script(
    app: AppHandle,
    store: State<'_, AccountStore>,
    sessions: State<'_, Sessions>,
    id: String,
    name: Option<String>,
) -> AppResult<()> {
    match &name {
        Some(name) => with_client!(app, store, sessions, id, |client| client.set_active(name).await)?,
        None => with_client!(app, store, sessions, id, |client| client.deactivate().await)?,
    }
    record(&app, &store, &id, |c| c.set_active(name.as_deref()));
    Ok(())
}

#[tauri::command]
pub async fn rename_script(
    app: AppHandle,
    store: State<'_, AccountStore>,
    sessions: State<'_, Sessions>,
    id: String,
    name: String,
    new_name: String,
) -> AppResult<()> {
    with_client!(app, store, sessions, id, |client| client.rename_script(&name, &new_name).await)?;
    record(&app, &store, &id, |c| c.rename(&name, &new_name));
    Ok(())
}

#[tauri::command]
pub async fn delete_script(
    app: AppHandle,
    store: State<'_, AccountStore>,
    sessions: State<'_, Sessions>,
    id: String,
    name: String,
) -> AppResult<()> {
    with_client!(app, store, sessions, id, |client| client.delete_script(&name).await)?;
    record(&app, &store, &id, |c| c.remove(&name));
    Ok(())
}

// ── Local copy ────────────────────────────────────────────────────────────────

fn local_copy(app: &AppHandle, account: &Account) -> AppResult<LocalCopy> {
    let dir = match &account.copy_dir {
        Some(dir) => std::path::PathBuf::from(dir),
        None => app
            .path()
            .app_data_dir()
            .map_err(|e| AppError::new(ErrorKind::Config, e.to_string()))?
            .join("copies")
            .join(&account.id),
    };
    Ok(LocalCopy::new(dir))
}

fn copy_error(e: std::io::Error) -> AppError {
    AppError::new(ErrorKind::Config, format!("local copy: {e}"))
}

/// Records a change the app made on the server in the local copy, once the
/// copy exists. The change on the server stands either way, so a failure
/// here is only logged.
fn record(app: &AppHandle, store: &AccountStore, id: &str, f: impl FnOnce(&LocalCopy) -> std::io::Result<()>) {
    let Ok(account) = store.get(app, id) else { return };
    let Ok(copy) = local_copy(app, &account) else { return };
    if !matches!(copy.manifest(), Ok(Some(_))) {
        return;
    }
    if let Err(e) = f(&copy) {
        eprintln!("local copy of {id}: {e}");
    }
}

/// All scripts on the server with their content, and the active one.
async fn server_state(
    app: &AppHandle,
    store: &State<'_, AccountStore>,
    sessions: &State<'_, Sessions>,
    id: &str,
) -> AppResult<(BTreeMap<String, String>, Option<String>)> {
    let scripts = with_client!(app, store, sessions, id, |client| client.list_scripts().await)?;
    let mut contents = BTreeMap::new();
    for script in &scripts {
        let content = with_client!(app, store, sessions, id, |client| client.get_script(&script.name).await)?;
        contents.insert(script.name.clone(), content);
    }
    Ok((contents, scripts.iter().find(|s| s.active).map(|s| s.name.clone())))
}

/// Compares the server with the account's local copy. Without a copy yet,
/// creates it from the server and reports no differences.
#[tauri::command]
pub async fn copy_status(
    app: AppHandle,
    store: State<'_, AccountStore>,
    sessions: State<'_, Sessions>,
    id: String,
) -> AppResult<CopyStatus> {
    let account = store.get(&app, &id)?;
    let copy = local_copy(&app, &account)?;
    let (server, active) = server_state(&app, &store, &sessions, &id).await?;
    let dir = copy.dir().display().to_string();
    let Some(manifest) = copy.manifest().map_err(copy_error)? else {
        copy.take_over(&server, active.as_deref()).map_err(copy_error)?;
        return Ok(CopyStatus { dir, ..CopyStatus::default() });
    };
    let local = copy.scripts().map_err(copy_error)?;
    let (mut differences, active) = copy::compare(&server, active.as_deref(), &local, &manifest);
    // Only comments or formatting changed on the server: nothing to decide,
    // the copy just follows.
    let mut kept = Vec::new();
    for d in differences.drain(..) {
        match (&d.server, d.same_rules && d.changed == [Side::Server]) {
            (Some(content), true) => copy.put(&d.name, content).map_err(copy_error)?,
            _ => kept.push(d),
        }
    }
    Ok(CopyStatus { dir, differences: kept, active })
}

/// Which side wins for a script that differs.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyChoice {
    pub name: String,
    pub keep: Side,
}

/// Resolves differences between server and local copy: `Local` uploads the
/// copy's version (or deletes the script on the server if the copy has
/// none), `Server` takes the server's version into the copy. `active` does
/// the same for the active script.
#[tauri::command]
pub async fn resolve_copy(
    app: AppHandle,
    store: State<'_, AccountStore>,
    sessions: State<'_, Sessions>,
    id: String,
    choices: Vec<CopyChoice>,
    active: Option<Side>,
) -> AppResult<()> {
    let account = store.get(&app, &id)?;
    let copy = local_copy(&app, &account)?;
    let (server, server_active) = server_state(&app, &store, &sessions, &id).await?;
    let local = copy.scripts().map_err(copy_error)?;
    // Uploads first, so that a script to activate exists.
    for choice in &choices {
        let name = &choice.name;
        match (choice.keep, local.get(name), server.get(name)) {
            (Side::Local, Some(content), _) => {
                with_client!(app, store, sessions, id, |client| client.check_script(content).await)?;
                with_client!(app, store, sessions, id, |client| client.put_script(name, content).await)?;
                copy.put(name, content).map_err(copy_error)?;
            }
            (Side::Local, None, Some(_)) => {
                with_client!(app, store, sessions, id, |client| client.delete_script(name).await)?;
                copy.remove(name).map_err(copy_error)?;
            }
            (Side::Server, _, Some(content)) => copy.put(name, content).map_err(copy_error)?,
            (Side::Server, _, None) => copy.remove(name).map_err(copy_error)?,
            (Side::Local, None, None) => {}
        }
    }
    match active {
        Some(Side::Local) => {
            let manifest = copy.manifest().map_err(copy_error)?.unwrap_or_default();
            match &manifest.active {
                Some(name) => with_client!(app, store, sessions, id, |client| client.set_active(name).await)?,
                None => with_client!(app, store, sessions, id, |client| client.deactivate().await)?,
            }
        }
        Some(Side::Server) => copy.set_active(server_active.as_deref()).map_err(copy_error)?,
        None => {}
    }
    Ok(())
}
