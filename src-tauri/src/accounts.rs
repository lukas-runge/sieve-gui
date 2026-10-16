//! Account configuration. Accounts are stored as JSON in the app config
//! directory; passwords live in the OS keychain, never in the file.

use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

use crate::error::{AppError, AppResult, ErrorKind};
use crate::server::Sessions;

const KEYCHAIN_SERVICE: &str = "sieve-gui";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: String,
    /// Display name, e.g. "Work".
    pub label: String,
    pub host: String,
    pub port: u16,
    pub security: AccountSecurity,
    pub username: String,
    /// IMAP access for the folder list, with the same credentials.
    /// `None` disables it.
    #[serde(default)]
    pub imap: Option<ImapSettings>,
    /// Several scripts may be active at once, run through a root script
    /// (`sieve_core::root`), if the server supports it.
    #[serde(default = "enabled")]
    pub multiple_scripts: bool,
    /// Each message lands in at most one folder: of several "move to folder"
    /// rules, the last one that runs wins (if the server supports it).
    #[serde(default = "enabled")]
    pub last_move_wins: bool,
}

fn enabled() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImapSettings {
    pub host: String,
    pub port: u16,
    pub security: AccountSecurity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AccountSecurity {
    /// STARTTLS on the ManageSieve port (default 4190).
    StartTls,
    /// TLS from the first byte.
    ImplicitTls,
    /// No encryption. Only for local test servers.
    Insecure,
}

impl From<AccountSecurity> for managesieve::Security {
    fn from(security: AccountSecurity) -> Self {
        match security {
            AccountSecurity::StartTls => Self::StartTls,
            AccountSecurity::ImplicitTls => Self::ImplicitTls,
            AccountSecurity::Insecure => Self::Insecure,
        }
    }
}

/// An account as entered in the account form.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountInput {
    /// `None` creates a new account.
    pub id: Option<String>,
    pub label: String,
    pub host: String,
    pub port: u16,
    pub security: AccountSecurity,
    pub username: String,
    pub imap: Option<ImapSettings>,
    pub multiple_scripts: bool,
    pub last_move_wins: bool,
    /// `None` keeps the stored password.
    pub password: Option<String>,
}

#[derive(Default)]
pub struct AccountStore {
    accounts: Mutex<Option<Vec<Account>>>,
}

impl AccountStore {
    pub fn get(&self, app: &AppHandle, id: &str) -> AppResult<Account> {
        self.with(app, |accounts| accounts.iter().find(|a| a.id == id).cloned())?
            .ok_or_else(|| AppError::new(ErrorKind::NotFound, format!("unknown account {id}")))
    }

    /// Runs `f` on the loaded account list.
    fn with<T>(&self, app: &AppHandle, f: impl FnOnce(&mut Vec<Account>) -> T) -> AppResult<T> {
        let mut guard = self.accounts.lock().expect("account store poisoned");
        if guard.is_none() {
            *guard = Some(load(app)?);
        }
        Ok(f(guard.as_mut().expect("loaded above")))
    }

    fn save(&self, app: &AppHandle) -> AppResult<()> {
        let guard = self.accounts.lock().expect("account store poisoned");
        let accounts = guard.as_deref().unwrap_or_default();
        let path = config_file(app)?;
        let json = serde_json::to_string_pretty(accounts).expect("accounts serialize");
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(config_error)?;
        }
        std::fs::write(&path, json).map_err(config_error)
    }
}

fn config_file(app: &AppHandle) -> AppResult<PathBuf> {
    let dir = app.path().app_config_dir().map_err(|e| AppError::new(ErrorKind::Config, e.to_string()))?;
    Ok(dir.join("accounts.json"))
}

fn load(app: &AppHandle) -> AppResult<Vec<Account>> {
    match std::fs::read_to_string(config_file(app)?) {
        Ok(json) => serde_json::from_str(&json).map_err(|e| AppError::new(ErrorKind::Config, e.to_string())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(config_error(e)),
    }
}

fn config_error(e: std::io::Error) -> AppError {
    AppError::new(ErrorKind::Config, e.to_string())
}

fn keychain_entry(id: &str) -> AppResult<keyring::Entry> {
    keyring::Entry::new(KEYCHAIN_SERVICE, id).map_err(keychain_error)
}

fn keychain_error(e: keyring::Error) -> AppError {
    AppError::new(ErrorKind::Keychain, e.to_string())
}

/// The stored password of an account.
pub fn password(id: &str) -> AppResult<String> {
    keychain_entry(id)?.get_password().map_err(|e| match e {
        keyring::Error::NoEntry => AppError::new(ErrorKind::Auth, "no password stored for this account"),
        e => keychain_error(e),
    })
}

// ── Commands ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn list_accounts(app: AppHandle, store: State<'_, AccountStore>) -> AppResult<Vec<Account>> {
    store.with(&app, |accounts| accounts.clone())
}

/// Creates or updates an account and returns it.
#[tauri::command]
pub async fn save_account(
    app: AppHandle,
    store: State<'_, AccountStore>,
    sessions: State<'_, Sessions>,
    input: AccountInput,
) -> AppResult<Account> {
    let account = Account {
        id: input.id.clone().unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
        label: input.label.trim().to_owned(),
        host: input.host.trim().to_owned(),
        port: input.port,
        security: input.security,
        username: input.username.trim().to_owned(),
        imap: input.imap.map(|imap| ImapSettings { host: imap.host.trim().to_owned(), ..imap }),
        multiple_scripts: input.multiple_scripts,
        last_move_wins: input.last_move_wins,
    };
    if let Some(pw) = &input.password {
        keychain_entry(&account.id)?.set_password(pw).map_err(keychain_error)?;
    }
    store.with(&app, |accounts| match accounts.iter_mut().find(|a| a.id == account.id) {
        Some(existing) => *existing = account.clone(),
        None => accounts.push(account.clone()),
    })?;
    store.save(&app)?;
    // Settings may have changed; connect again on next use.
    sessions.disconnect(&account.id).await;
    Ok(account)
}

#[tauri::command]
pub async fn delete_account(
    app: AppHandle,
    store: State<'_, AccountStore>,
    sessions: State<'_, Sessions>,
    id: String,
) -> AppResult<()> {
    sessions.disconnect(&id).await;
    store.with(&app, |accounts| accounts.retain(|a| a.id != id))?;
    store.save(&app)?;
    match keychain_entry(&id)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(keychain_error(e)),
    }
}
