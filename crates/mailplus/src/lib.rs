//! Synology MailPlus' labels, through DSM's web API.
//!
//! MailPlus keeps a label's name and colour to itself: over IMAP a label is
//! only a keyword such as `$synolabel_tu0exccz`. Its web client reads the
//! labels from DSM's web API (`SYNO.MailClient.Label`), which this crate
//! calls the same way. That API is undocumented and may change with MailPlus
//! updates; it was checked against MailPlus 4.1.1 on 2026-11-05.
//!
//! The API doesn't name a label's keyword. [`Client::labelled_messages`]
//! lists messages carrying a label, and [`match_keywords`] matches labels to
//! keywords from those messages' keywords, read over IMAP by the caller.
//!
//! TLS works like the ManageSieve client: the platform verifier, plus
//! certificates the user pinned (DSM often runs with a self-signed one, or
//! with a certificate for another host name than the one in the account).

use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;
use std::time::Duration;

use managesieve::tls::{CertificateInfo, PinningVerifier};
use serde::Deserialize;
use serde::de::DeserializeOwned;

/// DSM's default HTTPS port.
pub const DSM_PORT: u16 = 5001;

/// The prefix of the keywords MailPlus stores labels as.
pub const KEYWORD_PREFIX: &str = "$synolabel_";

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Network or HTTP problems.
    #[error("{0}")]
    Http(String),
    #[error("TLS: {0}")]
    Tls(String),
    /// The server's certificate is neither trusted by the platform nor
    /// pinned.
    #[error("untrusted server certificate: {reason}")]
    UntrustedCertificate {
        certificate: Box<CertificateInfo>,
        reason: String,
    },
    /// The server is no DSM with MailPlus: its web API has no labels.
    #[error("no MailPlus web API at this address")]
    NotMailPlus,
    /// DSM rejected the user name or password, or the account may not log in.
    #[error("DSM rejected the login (error {0})")]
    Auth(i64),
    /// The DSM account uses 2-step verification: log in again with a code.
    #[error("DSM asks for a 2-step verification code")]
    SecondFactorRequired,
    /// The 2-step verification code was wrong.
    #[error("DSM rejected the 2-step verification code")]
    SecondFactorFailed,
    /// The API answered with an error code.
    #[error("DSM answered {api} with error {code}")]
    Api { api: &'static str, code: i64 },
    /// The answer didn't have the expected shape.
    #[error("unexpected answer from {api}: {message}")]
    Response { api: &'static str, message: String },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Where DSM's web API is.
#[derive(Debug, Clone)]
pub struct ConnectOptions {
    pub host: String,
    pub port: u16,
    /// SHA-256 fingerprints of server certificates to accept even if the
    /// platform doesn't trust them.
    pub pinned_certificates: Vec<String>,
    /// Timeout per request.
    pub timeout: Duration,
}

impl ConnectOptions {
    /// Options for `host` on DSM's default HTTPS port with a 30 s timeout.
    pub fn new(host: impl Into<String>) -> Self {
        Self { host: host.into(), port: DSM_PORT, pinned_certificates: Vec::new(), timeout: Duration::from_secs(30) }
    }

    pub fn port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    /// Accepts the server certificates with these SHA-256 fingerprints.
    pub fn pinned_certificates<S: Into<String>>(mut self, pins: impl IntoIterator<Item = S>) -> Self {
        self.pinned_certificates = pins.into_iter().map(Into::into).collect();
        self
    }

    fn base_url(&self) -> String {
        // An IPv6 address needs brackets in a URL.
        let host = if self.host.contains(':') && !self.host.starts_with('[') {
            format!("[{}]", self.host)
        } else {
            self.host.clone()
        };
        format!("https://{host}:{}/webapi", self.port)
    }
}

/// A MailPlus label.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Label {
    pub id: i64,
    /// The label's own name, e.g. `Newsletter`.
    pub name: String,
    /// With its parents, separated by `/`, e.g. `xis/Newsletter`.
    pub full_name: String,
    /// Hex without `#`, e.g. `DCE1E6`.
    #[serde(default)]
    pub background_color: String,
    /// Hex without `#`, e.g. `50555A`.
    #[serde(default)]
    pub text_color: String,
}

impl Label {
    /// One colour for the label as `#rrggbb`: the background if it is strong
    /// enough to stand for the label (a dot, a tinted chip), otherwise the
    /// text colour (MailPlus pairs pale backgrounds with a dark text).
    pub fn color(&self) -> Option<String> {
        let background = parse_hex(&self.background_color);
        let text = parse_hex(&self.text_color);
        let rgb = match (background, text) {
            (Some(bg), Some(fg)) if luminance(bg) > 0.6 => fg,
            (Some(bg), _) => bg,
            (None, fg) => fg?,
        };
        Some(format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2]))
    }
}

fn parse_hex(hex: &str) -> Option<[u8; 3]> {
    let hex = hex.trim().trim_start_matches('#');
    if hex.len() != 6 || !hex.is_ascii() {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
    Some([byte(0)?, byte(2)?, byte(4)?])
}

/// Relative luminance (sRGB coefficients, without linearization: enough to
/// tell pale from strong).
fn luminance(rgb: [u8; 3]) -> f64 {
    let [r, g, b] = rgb.map(|c| f64::from(c) / 255.0);
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

/// A message carrying a label, as the web API lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelledMessage {
    /// The folder's id, see [`Client::folders`].
    pub folder: i64,
    /// The `Message-ID` header, without angle brackets.
    pub message_id: String,
    /// Ids of all labels the message carries.
    pub labels: Vec<i64>,
}

/// A logged-in session with DSM's web API.
pub struct Client {
    http: reqwest::Client,
    base: String,
    sid: String,
    verifier: Arc<PinningVerifier>,
}

#[derive(Deserialize)]
struct Envelope<T> {
    success: bool,
    data: Option<T>,
    error: Option<ApiError>,
}

#[derive(Deserialize)]
struct ApiError {
    code: i64,
}

impl Client {
    /// Checks that the server runs MailPlus and logs in. `otp` is the 2-step
    /// verification code, if DSM asks for one ([`Error::SecondFactorRequired`]).
    pub async fn login(opts: &ConnectOptions, username: &str, password: &str, otp: Option<&str>) -> Result<Self> {
        let verifier = PinningVerifier::new(&opts.pinned_certificates).map_err(|e| Error::Tls(e.to_string()))?;
        let tls = verifier.client_config().map_err(|e| Error::Tls(e.to_string()))?;
        let http = reqwest::Client::builder()
            .tls_backend_preconfigured((*tls).clone())
            .timeout(opts.timeout)
            .build()
            .map_err(|e| Error::Http(e.to_string()))?;
        let mut client = Self { http, base: opts.base_url(), sid: String::new(), verifier };

        // Works without a session: which APIs the server has.
        let info: HashMap<String, serde_json::Value> = client
            .call("SYNO.API.Info", "query.cgi", &[("version", "1"), ("method", "query"), ("query", "SYNO.MailClient.Label,SYNO.MailClient.Thread")])
            .await?;
        if !info.contains_key("SYNO.MailClient.Label") || !info.contains_key("SYNO.MailClient.Thread") {
            return Err(Error::NotMailPlus);
        }

        #[derive(Deserialize)]
        struct Login {
            sid: String,
        }
        let mut params = vec![
            ("version", "6"),
            ("method", "login"),
            ("account", username),
            ("passwd", password),
            // The DSM web session; a session named after the app is refused.
            ("session", "webui"),
            ("format", "sid"),
        ];
        if let Some(otp) = otp {
            params.push(("otp_code", otp));
        }
        let login: Login = client.call("SYNO.API.Auth", "entry.cgi", &params).await.map_err(|e| match e {
            Error::Api { code: 403, .. } => Error::SecondFactorRequired,
            Error::Api { code: 404, .. } => Error::SecondFactorFailed,
            Error::Api { code, .. } => Error::Auth(code),
            e => e,
        })?;
        client.sid = login.sid;
        Ok(client)
    }

    /// All labels of the account.
    pub async fn labels(&self) -> Result<Vec<Label>> {
        #[derive(Deserialize)]
        struct Labels {
            label: Vec<Label>,
        }
        let labels: Labels = self.call("SYNO.MailClient.Label", "entry.cgi", &[("version", "3"), ("method", "list")]).await?;
        Ok(labels.label)
    }

    /// The account's folders: id → path as the user sees it (UTF-8, with
    /// the server's hierarchy delimiter).
    pub async fn folders(&self) -> Result<HashMap<i64, String>> {
        #[derive(Deserialize)]
        struct Folder {
            id: i64,
            path: String,
        }
        #[derive(Deserialize)]
        struct Folders {
            mailbox: Vec<Folder>,
        }
        let folders: Folders = self.call("SYNO.MailClient.Mailbox", "entry.cgi", &[("version", "1"), ("method", "list")]).await?;
        Ok(folders.mailbox.into_iter().map(|f| (f.id, f.path)).collect())
    }

    /// Up to `limit` messages carrying the label, newest first.
    pub async fn labelled_messages(&self, label: i64, limit: u32) -> Result<Vec<LabelledMessage>> {
        #[derive(Deserialize)]
        struct Message {
            mailbox_id: i64,
            #[serde(default)]
            rfc_message_id: String,
        }
        #[derive(Deserialize)]
        struct LabelRef {
            id: i64,
        }
        #[derive(Deserialize)]
        struct Thread {
            #[serde(default)]
            label: Vec<LabelRef>,
            #[serde(default)]
            message: Vec<Message>,
        }
        #[derive(Deserialize)]
        struct Threads {
            thread: Vec<Thread>,
        }
        let condition = serde_json::json!([{ "name": "label", "value": label.to_string() }]).to_string();
        let limit = limit.to_string();
        let threads: Threads = self
            .call(
                "SYNO.MailClient.Thread",
                "entry.cgi",
                &[
                    ("version", "11"),
                    ("method", "list"),
                    ("condition", &condition),
                    // One message per entry, with its own labels.
                    ("conversation_view", "false"),
                    ("offset", "0"),
                    ("limit", &limit),
                ],
            )
            .await?;
        Ok(threads
            .thread
            .into_iter()
            .flat_map(|thread| {
                let labels: Vec<i64> = thread.label.iter().map(|l| l.id).collect();
                thread
                    .message
                    .into_iter()
                    .filter(|m| !m.rfc_message_id.is_empty())
                    .map(move |m| LabelledMessage { folder: m.mailbox_id, message_id: m.rfc_message_id, labels: labels.clone() })
            })
            .collect())
    }

    /// Ends the session.
    pub async fn logout(self) {
        let _: Result<serde_json::Value> = self
            .call("SYNO.API.Auth", "entry.cgi", &[("version", "6"), ("method", "logout"), ("session", "webui")])
            .await;
    }

    async fn call<T: DeserializeOwned>(&self, api: &'static str, path: &str, params: &[(&str, &str)]) -> Result<T> {
        let mut form: Vec<(&str, &str)> = vec![("api", api)];
        form.extend_from_slice(params);
        if !self.sid.is_empty() {
            form.push(("_sid", &self.sid));
        }
        let response = self
            .http
            .post(format!("{}/{path}", self.base))
            .form(&form)
            .send()
            .await
            .map_err(|e| self.request_error(e))?;
        let status = response.status();
        let body = response.bytes().await.map_err(|e| self.request_error(e))?;
        if !status.is_success() {
            return Err(if status == reqwest::StatusCode::NOT_FOUND {
                Error::NotMailPlus
            } else {
                Error::Http(format!("HTTP {status}"))
            });
        }
        let envelope: Envelope<T> = serde_json::from_slice(&body).map_err(|e| Error::Response { api, message: e.to_string() })?;
        match envelope {
            Envelope { success: true, data: Some(data), .. } => Ok(data),
            Envelope { success: true, data: None, .. } => serde_json::from_value(serde_json::Value::Null)
                .map_err(|e| Error::Response { api, message: e.to_string() }),
            Envelope { error, .. } => Err(Error::Api { api, code: error.map_or(0, |e| e.code) }),
        }
    }

    fn request_error(&self, e: reqwest::Error) -> Error {
        match self.verifier.rejected_certificate() {
            Some(cert) => Error::UntrustedCertificate {
                certificate: Box::new(CertificateInfo::from_der(&cert)),
                reason: error_chain(&e),
            },
            None => Error::Http(error_chain(&e)),
        }
    }
}

/// An error with its causes: reqwest's own message is only "error sending
/// request".
fn error_chain(e: &dyn std::error::Error) -> String {
    let mut text = e.to_string();
    let mut source = e.source();
    while let Some(cause) = source {
        text.push_str(": ");
        text.push_str(&cause.to_string());
        source = cause.source();
    }
    text
}

/// Matches labels to their keywords.
///
/// Each sample is a message (each message once): the labels the web API
/// lists for it and the keywords it has over IMAP. Only MailPlus' keywords
/// ([`KEYWORD_PREFIX`]) count.
///
/// A label and a keyword belong together the more of the label's messages
/// have the keyword and the more of the keyword's messages have the label.
/// The second part tells a child label's keyword from its parent's, which
/// its messages usually carry too. Pairs are taken best first; a label or
/// keyword taken is out for the others. One stray message (the web API and
/// IMAP don't always agree) lowers a pair's score but doesn't rule it out.
/// Labels on no message, or whose best keyword ties with another, are
/// missing from the result.
pub fn match_keywords(samples: &[(Vec<i64>, Vec<String>)]) -> HashMap<i64, String> {
    let samples: Vec<(BTreeSet<i64>, BTreeSet<&str>)> = samples
        .iter()
        .map(|(labels, keywords)| {
            let own = keywords.iter().map(String::as_str).filter(|k| k.starts_with(KEYWORD_PREFIX)).collect();
            (labels.iter().copied().collect(), own)
        })
        .collect();
    let mut per_label: HashMap<i64, usize> = HashMap::new();
    let mut per_keyword: HashMap<&str, usize> = HashMap::new();
    let mut together: HashMap<(i64, &str), usize> = HashMap::new();
    for (labels, keywords) in &samples {
        for label in labels {
            *per_label.entry(*label).or_default() += 1;
        }
        for keyword in keywords {
            *per_keyword.entry(keyword).or_default() += 1;
            for label in labels {
                *together.entry((*label, keyword)).or_default() += 1;
            }
        }
    }
    let mut pairs: Vec<(f64, i64, &str)> = together
        .iter()
        .map(|(&(label, keyword), &n)| {
            let n = n as f64;
            (n / per_label[&label] as f64 * (n / per_keyword[keyword] as f64), label, keyword)
        })
        .filter(|(score, ..)| *score >= 0.5)
        .collect();
    pairs.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(b.2)));

    let mut matched: HashMap<i64, String> = HashMap::new();
    let mut taken_labels: BTreeSet<i64> = BTreeSet::new();
    let mut taken_keywords: BTreeSet<&str> = BTreeSet::new();
    for &(score, label, keyword) in &pairs {
        if taken_labels.contains(&label) || taken_keywords.contains(keyword) {
            continue;
        }
        // Pairs just as good that share the label or keyword (and those
        // sharing one with them): undecided, all of them.
        let mut group: Vec<(i64, &str)> = vec![(label, keyword)];
        let mut grew = true;
        while grew {
            grew = false;
            for &(other, l, k) in &pairs {
                let open = !taken_labels.contains(&l) && !taken_keywords.contains(k);
                let linked = group.iter().any(|&(gl, gk)| gl == l || gk == k);
                if open && linked && (other - score).abs() < 1e-9 && !group.contains(&(l, k)) {
                    group.push((l, k));
                    grew = true;
                }
            }
        }
        if group.len() == 1 {
            matched.insert(label, keyword.to_owned());
        }
        for (l, k) in group {
            taken_labels.insert(l);
            taken_keywords.insert(k);
        }
    }
    matched
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(labels: &[i64], keywords: &[&str]) -> (Vec<i64>, Vec<String>) {
        (labels.to_vec(), keywords.iter().map(|k| (*k).to_owned()).collect())
    }

    #[test]
    fn a_label_on_its_own_message() {
        let matched = match_keywords(&[sample(&[8], &["NonJunk", "$synolabel_tu0exccz"])]);
        assert_eq!(matched, HashMap::from([(8, "$synolabel_tu0exccz".to_owned())]));
    }

    #[test]
    fn keywords_common_to_all_messages_of_a_label() {
        let matched = match_keywords(&[
            sample(&[4], &["$synolabel_a", "$synolabel_b"]),
            sample(&[4], &["$synolabel_a", "$synolabel_c"]),
        ]);
        assert_eq!(matched, HashMap::from([(4, "$synolabel_a".to_owned())]));
    }

    #[test]
    fn a_matched_keyword_is_ruled_out_for_other_labels() {
        // A parent label always comes with its child: known from the child's
        // other messages.
        let matched = match_keywords(&[
            sample(&[18, 4], &["$synolabel_x", "$synolabel_n"]),
            sample(&[4, 18], &["$synolabel_n", "$synolabel_x"]),
            sample(&[2], &["$synolabel_b"]),
            sample(&[18, 2], &["$synolabel_x", "$synolabel_b"]),
        ]);
        assert_eq!(
            matched,
            HashMap::from([(2, "$synolabel_b".to_owned()), (18, "$synolabel_x".to_owned()), (4, "$synolabel_n".to_owned())])
        );
    }

    #[test]
    fn a_stray_message_doesnt_rule_a_label_out() {
        // BHT (6) with its children Moodle (11) and StuPa (13); one message
        // the web API lists with BHT lacks BHT's keyword over IMAP.
        let matched = match_keywords(&[
            sample(&[6], &["NonJunk", "$synolabel_bht"]),
            sample(&[6], &["$synolabel_bht"]),
            sample(&[6, 11], &["$synolabel_bht", "$synolabel_moodle"]),
            sample(&[6, 11], &["$synolabel_bht", "$synolabel_moodle"]),
            sample(&[6, 13], &["$synolabel_bht", "$synolabel_stupa"]),
            sample(&[6, 16], &["$synolabel_other"]),
            sample(&[16], &["$synolabel_other"]),
        ]);
        assert_eq!(
            matched,
            HashMap::from([
                (6, "$synolabel_bht".to_owned()),
                (11, "$synolabel_moodle".to_owned()),
                (13, "$synolabel_stupa".to_owned()),
                (16, "$synolabel_other".to_owned()),
            ])
        );
    }

    #[test]
    fn ambiguous_and_missing_labels_stay_open() {
        let matched = match_keywords(&[sample(&[1, 2], &["$synolabel_a", "$synolabel_b"]), sample(&[3], &["NonJunk"])]);
        assert!(matched.is_empty());
    }

    #[test]
    fn one_colour_per_label() {
        let label = |bg: &str, fg: &str| Label {
            id: 1,
            name: "n".into(),
            full_name: "n".into(),
            background_color: bg.into(),
            text_color: fg.into(),
        };
        // Strong background: it stands for the label.
        assert_eq!(label("CC3D9C", "FFFFFF").color().as_deref(), Some("#cc3d9c"));
        // Pale background: the text colour does.
        assert_eq!(label("FFEC8C", "997F00").color().as_deref(), Some("#997f00"));
        assert_eq!(label("", "997F00").color().as_deref(), Some("#997f00"));
        assert_eq!(label("nope", "").color(), None);
    }

    #[test]
    fn base_urls() {
        assert_eq!(ConnectOptions::new("nas.local").base_url(), "https://nas.local:5001/webapi");
        assert_eq!(ConnectOptions::new("fd00::1").port(443).base_url(), "https://[fd00::1]:443/webapi");
    }
}
