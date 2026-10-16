//! ManageSieve sessions and the commands that use them.

use std::collections::HashMap;

use managesieve::{Capabilities, Client, ConnectOptions, Credentials, Diagnostic, ScriptInfo};
use serde::Serialize;
use tauri::{AppHandle, State};
use tokio::sync::Mutex;

use crate::accounts::{self, Account, AccountStore};
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
    let opts = ConnectOptions::new(&account.host).port(account.port).security(account.security.into());
    let mut client = Client::connect(&opts).await.map_err(|e| AppError::from_sieve(e, false))?;
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
    with_client!(app, store, sessions, id, |client| client.put_script(&name, &content).await)
        .map(|outcome| outcome.diagnostics())
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
    match name {
        Some(name) => with_client!(app, store, sessions, id, |client| client.set_active(&name).await),
        None => with_client!(app, store, sessions, id, |client| client.deactivate().await),
    }
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
    with_client!(app, store, sessions, id, |client| client.rename_script(&name, &new_name).await)
}

#[tauri::command]
pub async fn delete_script(
    app: AppHandle,
    store: State<'_, AccountStore>,
    sessions: State<'_, Sessions>,
    id: String,
    name: String,
) -> AppResult<()> {
    with_client!(app, store, sessions, id, |client| client.delete_script(&name).await)
}
