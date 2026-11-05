//! Account configuration. Accounts are stored as JSON in the app config
//! directory; passwords live in the OS keychain, never in the file.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

use crate::error::{AppError, AppResult, ErrorKind};
use crate::server::Sessions;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: String,
    /// Display name, e.g. "Work".
    pub label: String,
    pub host: String,
    pub port: u16,
    pub security: AccountSecurity,
    /// SHA-256 fingerprint of a ManageSieve server certificate the user
    /// trusts although the OS doesn't (see `managesieve::tls::PinningVerifier`).
    #[serde(default)]
    pub certificate: Option<String>,
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
    /// The scripts are also edited in Roundcube: warn about anything it would
    /// drop when it saves them.
    #[serde(default)]
    pub roundcube: bool,
    /// Names for custom keywords (IMAP flags such as `$label1` or
    /// `$synolabel_…`), shown instead of the keyword.
    #[serde(default)]
    pub keyword_names: HashMap<String, String>,
    /// Folder for the local copy of the scripts; `None` uses the app's data
    /// folder (see `copy`).
    #[serde(default)]
    pub copy_dir: Option<String>,
    /// How the account looks in the account switcher; `None`: its initials
    /// in a colour derived from its name.
    #[serde(default)]
    pub icon: Option<AccountIcon>,
    /// sieve-gui manages the filters alone, after taking them over from a
    /// filter provider (`sieve_core::providers`, concept D19). `None`: off.
    #[serde(default)]
    pub filters_alone: Option<FiltersAlone>,
}

/// The filter provider whose filters sieve-gui took over: if it makes its
/// root active again, the app re-activates its own root on connect.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FiltersAlone {
    /// The provider (`sieve_core::providers::Detection::provider`).
    pub provider: String,
    /// The provider's root script.
    pub root: String,
    /// The provider's scripts with filters.
    pub scripts: Vec<String>,
    /// What its filters were when they were taken over
    /// (`Detection::fingerprint`): only filters added since are offered.
    #[serde(default)]
    pub fingerprint: String,
}

/// An account's icon: a picture, or a short text on a colour.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountIcon {
    /// One or two characters (or an emoji) instead of the initials.
    #[serde(default)]
    pub text: Option<String>,
    /// Hue (0–359) of the colour, instead of one derived from the name.
    #[serde(default)]
    pub hue: Option<u16>,
    /// A picture as a `data:image/…` URL, small and square (the app scales
    /// uploads down); shown instead of text and colour.
    #[serde(default)]
    pub image: Option<String>,
}

/// Largest picture kept for an icon (as a data URL), so the account file stays small.
const MAX_ICON_IMAGE: usize = 256 * 1024;

impl AccountIcon {
    fn checked(self) -> AppResult<Option<Self>> {
        let text = self.text.map(|t| t.trim().to_owned()).filter(|t| !t.is_empty());
        if let Some(image) = &self.image {
            if !image.starts_with("data:image/") || image.len() > MAX_ICON_IMAGE {
                return Err(AppError::new(ErrorKind::Config, "the icon must be a small picture"));
            }
        }
        let icon = Self { text, hue: self.hue.map(|h| h % 360), image: self.image };
        Ok((icon.text.is_some() || icon.hue.is_some() || icon.image.is_some()).then_some(icon))
    }
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
    /// Like [`Account::certificate`], for the IMAP server.
    #[serde(default)]
    pub certificate: Option<String>,
}

/// The servers of an account.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Service {
    ManageSieve,
    Imap,
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
    pub certificate: Option<String>,
    pub username: String,
    pub imap: Option<ImapSettings>,
    pub multiple_scripts: bool,
    pub last_move_wins: bool,
    pub roundcube: bool,
    pub keyword_names: HashMap<String, String>,
    pub copy_dir: Option<String>,
    #[serde(default)]
    pub icon: Option<AccountIcon>,
    #[serde(default)]
    pub filters_alone: Option<FiltersAlone>,
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

/// The stored password of an account (see `passwords`).
pub fn password(id: &str) -> AppResult<String> {
    crate::passwords::get(id)
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
        certificate: input.certificate,
        username: input.username.trim().to_owned(),
        imap: input.imap.map(|imap| ImapSettings { host: imap.host.trim().to_owned(), ..imap }),
        multiple_scripts: input.multiple_scripts,
        last_move_wins: input.last_move_wins,
        roundcube: input.roundcube,
        keyword_names: input.keyword_names,
        copy_dir: input.copy_dir.map(|d| d.trim().to_owned()).filter(|d| !d.is_empty()),
        icon: match input.icon {
            Some(icon) => icon.checked()?,
            None => None,
        },
        filters_alone: input.filters_alone,
    };
    if let Some(pw) = &input.password {
        crate::passwords::set(&account.id, pw)?;
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

/// Sets the names of the account's custom keywords and returns the account.
/// Unlike `save_account`, this keeps the connection.
#[tauri::command]
pub fn set_keyword_names(
    app: AppHandle,
    store: State<'_, AccountStore>,
    id: String,
    names: HashMap<String, String>,
) -> AppResult<Account> {
    let names = names
        .into_iter()
        .map(|(k, v)| (k.trim().to_owned(), v.trim().to_owned()))
        .filter(|(k, v)| !k.is_empty() && !v.is_empty())
        .collect();
    let account = store
        .with(&app, |accounts| {
            let account = accounts.iter_mut().find(|a| a.id == id)?;
            account.keyword_names = names;
            Some(account.clone())
        })?
        .ok_or_else(|| AppError::new(ErrorKind::NotFound, format!("unknown account {id}")))?;
    store.save(&app)?;
    Ok(account)
}

/// Turns "sieve-gui manages the filters alone" on (with the provider whose
/// filters were taken over) or off, and returns the account. Unlike
/// `save_account`, this keeps the connection.
#[tauri::command]
pub fn set_filters_alone(
    app: AppHandle,
    store: State<'_, AccountStore>,
    id: String,
    value: Option<FiltersAlone>,
) -> AppResult<Account> {
    let account = store
        .with(&app, |accounts| {
            let account = accounts.iter_mut().find(|a| a.id == id)?;
            account.filters_alone = value;
            Some(account.clone())
        })?
        .ok_or_else(|| AppError::new(ErrorKind::NotFound, format!("unknown account {id}")))?;
    store.save(&app)?;
    Ok(account)
}

/// Trusts a server certificate of the account (by its SHA-256 fingerprint),
/// replacing any certificate trusted before, and returns the account.
#[tauri::command]
pub async fn trust_certificate(
    app: AppHandle,
    store: State<'_, AccountStore>,
    sessions: State<'_, Sessions>,
    id: String,
    service: Service,
    sha256: String,
) -> AppResult<Account> {
    let account = store
        .with(&app, |accounts| {
            let account = accounts.iter_mut().find(|a| a.id == id)?;
            match service {
                Service::ManageSieve => account.certificate = Some(sha256),
                Service::Imap => account.imap.as_mut()?.certificate = Some(sha256),
            }
            Some(account.clone())
        })?
        .ok_or_else(|| AppError::new(ErrorKind::NotFound, format!("no {service:?} server for account {id}")))?;
    store.save(&app)?;
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
    crate::passwords::remove(&id)
}
