//! Tag providers: systems that keep names and colours for keywords, so they
//! can be taken over instead of typed in. The first is Synology MailPlus
//! (its labels, through DSM's web API; see the `mailplus` crate).

use std::collections::HashSet;

use serde::Serialize;
use tauri::{AppHandle, State};

use crate::accounts::{self, AccountStore};
use crate::error::{AppError, AppResult, ErrorKind};

/// Messages read per label to tell its keyword. A few are enough; more only
/// make the IMAP search longer.
const SAMPLES_PER_LABEL: u32 = 5;

/// A tag as the provider keeps it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderTag {
    /// The name with its parents, e.g. `xis/Newsletter`.
    pub name: String,
    /// `#rrggbb`, if the provider has one.
    pub color: Option<String>,
    /// The keyword the tag is stored as. `None`: it couldn't be told, e.g.
    /// because no message carries the tag.
    pub keyword: Option<String>,
}

/// The provider's tags, sorted by name.
///
/// MailPlus: logs in to DSM on the account's host and `port` with the
/// account's credentials (`otp`: a 2-step verification code, after
/// [`ErrorKind::SecondFactorRequired`]), reads the labels and a few messages
/// for each, and tells the labels' keywords from those messages' keywords
/// over IMAP. Nothing changes on the server.
#[tauri::command]
pub async fn provider_tags(
    app: AppHandle,
    store: State<'_, AccountStore>,
    id: String,
    provider: String,
    port: u16,
    otp: Option<String>,
) -> AppResult<Vec<ProviderTag>> {
    if provider != "mailplus" {
        return Err(AppError::new(ErrorKind::NotFound, format!("unknown tag provider {provider}")));
    }
    let account = store.get(&app, &id)?;
    let Some(imap) = account.imap.clone() else {
        return Err(AppError::new(ErrorKind::Config, "IMAP is needed to tell the labels' keywords"));
    };
    let password = accounts::password(&account.id)?;
    let pinned = account.mailplus.as_ref().and_then(|m| m.certificate.clone());

    let opts = mailplus::ConnectOptions::new(&account.host).port(port).pinned_certificates(pinned.iter());
    let dsm_error = |e: mailplus::Error| AppError::from(e).with_pinned(pinned.as_ref());
    let client = mailplus::Client::login(&opts, &account.username, &password, otp.as_deref().map(str::trim))
        .await
        .map_err(dsm_error)?;
    let read = async {
        let labels = client.labels().await?;
        let folders = client.folders().await?;
        let mut messages = Vec::new();
        let mut seen = HashSet::new();
        for label in &labels {
            for message in client.labelled_messages(label.id, SAMPLES_PER_LABEL).await? {
                if seen.insert((message.folder, message.message_id.clone())) {
                    messages.push(message);
                }
            }
        }
        Ok::<_, mailplus::Error>((labels, folders, messages))
    }
    .await;
    client.logout().await;
    let (labels, folders, messages) = read.map_err(dsm_error)?;

    // Messages in folders the API doesn't list can't be looked up.
    let messages: Vec<_> = messages.into_iter().filter(|m| folders.contains_key(&m.folder)).collect();
    let refs: Vec<mailstore::MessageRef> = messages
        .iter()
        .map(|m| mailstore::MessageRef { folder: folders[&m.folder].clone(), message_id: m.message_id.clone() })
        .collect();
    let imap_opts = mailstore::ConnectOptions::new(&imap.host)
        .port(imap.port)
        .security(imap.security.into())
        .pinned_certificates(imap.certificate.iter());
    let keywords = mailstore::message_keywords(&imap_opts, &account.username, &password, &refs)
        .await
        .map_err(|e| AppError::from(e).with_pinned(imap.certificate.as_ref()))?;
    let samples: Vec<_> = messages.iter().zip(keywords).map(|(m, k)| (m.labels.clone(), k)).collect();
    let matched = mailplus::match_keywords(&samples);

    let mut tags: Vec<ProviderTag> = labels
        .iter()
        .map(|label| ProviderTag {
            name: label.full_name.clone(),
            color: label.color(),
            keyword: matched.get(&label.id).cloned(),
        })
        .collect();
    tags.sort_by_key(|tag| tag.name.to_lowercase());
    Ok(tags)
}
