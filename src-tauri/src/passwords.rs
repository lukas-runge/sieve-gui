//! Account passwords in the OS keychain (Keychain on macOS, Credential
//! Manager on Windows, Secret Service on Linux).
//!
//! All passwords live in one keychain item, so a start reads the keychain
//! once, however many accounts there are: each read may ask the user (e.g.
//! for an app build the keychain doesn't know yet). The item holds a JSON
//! object from account id to password; it stays small enough for Windows'
//! limit of 2560 bytes for a few dozen accounts. Concurrent connections
//! wait for the first read instead of reading too.
//!
//! Earlier versions kept one item per account; such an item is moved into
//! the shared one the first time its password is needed.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use crate::error::{AppError, AppResult, ErrorKind};

const SERVICE: &str = "sieve-gui";
/// The account name of the shared item (the per-account items used ids).
const ITEM: &str = "passwords";

#[derive(Default)]
struct Vault {
    /// Whether `passwords` holds the shared item's content.
    loaded: bool,
    passwords: HashMap<String, String>,
}

static VAULT: LazyLock<Mutex<Vault>> = LazyLock::new(Default::default);

/// The platform's credential store, set up on first use.
static STORE: LazyLock<Result<(), String>> = LazyLock::new(|| use_platform_store().map_err(|e| e.to_string()));

fn use_platform_store() -> keyring_core::Result<()> {
    #[cfg(target_os = "macos")]
    let store = apple_native_keyring_store::keychain::Store::new()?;
    #[cfg(target_os = "windows")]
    let store = windows_native_keyring_store::Store::new()?;
    #[cfg(all(unix, not(any(target_os = "macos", target_os = "ios", target_os = "android"))))]
    let store = dbus_secret_service_keyring_store::Store::new()?;
    keyring_core::set_default_store(store);
    Ok(())
}

fn entry(name: &str) -> AppResult<keyring_core::Entry> {
    if let Err(e) = &*STORE {
        return Err(AppError::new(ErrorKind::Keychain, e.clone()));
    }
    keyring_core::Entry::new(SERVICE, name).map_err(keychain_error)
}

fn keychain_error(e: keyring_core::Error) -> AppError {
    AppError::new(ErrorKind::Keychain, e.to_string())
}

fn lock() -> std::sync::MutexGuard<'static, Vault> {
    VAULT.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl Vault {
    /// Reads the shared item, once per run.
    fn load(&mut self) -> AppResult<()> {
        if self.loaded {
            return Ok(());
        }
        self.passwords = match entry(ITEM)?.get_password() {
            Ok(json) => serde_json::from_str(&json).map_err(|e| AppError::new(ErrorKind::Keychain, e.to_string()))?,
            Err(keyring_core::Error::NoEntry) => HashMap::new(),
            Err(e) => return Err(keychain_error(e)),
        };
        self.loaded = true;
        Ok(())
    }

    fn save(&self) -> AppResult<()> {
        let json = serde_json::to_string(&self.passwords).expect("passwords serialize");
        entry(ITEM)?.set_password(&json).map_err(keychain_error)
    }
}

/// The stored password of an account.
pub fn get(id: &str) -> AppResult<String> {
    let mut vault = lock();
    vault.load()?;
    if let Some(password) = vault.passwords.get(id) {
        return Ok(password.clone());
    }
    // An item of an earlier version: move it into the shared one.
    let legacy = entry(id)?;
    match legacy.get_password() {
        Ok(password) => {
            vault.passwords.insert(id.to_owned(), password.clone());
            vault.save()?;
            let _ = legacy.delete_credential();
            Ok(password)
        }
        Err(keyring_core::Error::NoEntry) => Err(AppError::new(ErrorKind::Auth, "no password stored for this account")),
        Err(e) => Err(keychain_error(e)),
    }
}

pub fn set(id: &str, password: &str) -> AppResult<()> {
    let mut vault = lock();
    vault.load()?;
    vault.passwords.insert(id.to_owned(), password.to_owned());
    vault.save()?;
    // A per-account item of an earlier version would only be outdated now.
    if let Ok(legacy) = entry(id) {
        let _ = legacy.delete_credential();
    }
    Ok(())
}

pub fn remove(id: &str) -> AppResult<()> {
    let mut vault = lock();
    vault.load()?;
    if vault.passwords.remove(id).is_some() {
        vault.save()?;
    }
    match entry(id)?.delete_credential() {
        Ok(()) | Err(keyring_core::Error::NoEntry) => Ok(()),
        Err(e) => Err(keychain_error(e)),
    }
}
