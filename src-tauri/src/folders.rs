//! The IMAP folder list for the folder picker.

use mailstore::{ConnectOptions, Folder};
use tauri::{AppHandle, State};

use crate::accounts::{self, AccountStore};
use crate::error::{AppError, AppResult, ErrorKind};

/// All folders of the account's mailbox. Fails with `config` if IMAP is not
/// set up for the account.
#[tauri::command]
pub async fn list_folders(app: AppHandle, store: State<'_, AccountStore>, id: String) -> AppResult<Vec<Folder>> {
    let account = store.get(&app, &id)?;
    let Some(imap) = account.imap else {
        return Err(AppError::new(ErrorKind::Config, "IMAP is not set up for this account"));
    };
    let opts = ConnectOptions::new(&imap.host)
        .port(imap.port)
        .security(imap.security.into())
        .pinned_certificates(imap.certificate.iter());
    let password = accounts::password(&account.id)?;
    mailstore::list_folders(&opts, &account.username, &password)
        .await
        .map_err(|e| AppError::from(e).with_pinned(imap.certificate.as_ref()))
}
