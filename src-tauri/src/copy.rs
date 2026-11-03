//! The local copy of an account's scripts (concept §3, decision D13).
//!
//! One folder per account holds every script on the server as a
//! `<name>.sieve` file and a manifest with the active script and each
//! script's SHA-256 at the last sync. The app updates the copy after each of
//! its own changes on the server. On connect, [`compare`] lists what differs
//! between server and copy, and the user decides per script which side wins.

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const MANIFEST: &str = "sieve-gui.json";
const EXTENSION: &str = ".sieve";

/// What the copy recorded at the last sync.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    /// The active script on the server.
    pub active: Option<String>,
    /// SHA-256 (hex) of each script's content.
    pub scripts: BTreeMap<String, String>,
}

/// Which side changed since the last sync.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Side {
    Server,
    Local,
}

/// A script that differs between server and copy. `None` means missing on
/// that side.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Difference {
    pub name: String,
    pub server: Option<String>,
    pub local: Option<String>,
    /// The sides that changed since the last sync.
    pub changed: Vec<Side>,
}

/// The active script differs: on the server now, and at the last sync.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveDifference {
    pub server: Option<String>,
    pub local: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyStatus {
    /// Where the copy is.
    pub dir: String,
    pub differences: Vec<Difference>,
    pub active: Option<ActiveDifference>,
}

pub fn hash(content: &str) -> String {
    Sha256::digest(content.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

/// What differs between the scripts on the server and in the copy.
pub fn compare(
    server: &BTreeMap<String, String>,
    server_active: Option<&str>,
    local: &BTreeMap<String, String>,
    manifest: &Manifest,
) -> (Vec<Difference>, Option<ActiveDifference>) {
    let names: BTreeSet<&String> = server.keys().chain(local.keys()).collect();
    let differences = names
        .into_iter()
        .filter_map(|name| {
            let (s, l) = (server.get(name), local.get(name));
            if s == l {
                return None;
            }
            let base = manifest.scripts.get(name);
            let differs_from_base = |content: Option<&String>| content.map(|c| hash(c)).as_ref() != base;
            let mut changed = Vec::new();
            if differs_from_base(s) {
                changed.push(Side::Server);
            }
            if differs_from_base(l) {
                changed.push(Side::Local);
            }
            Some(Difference { name: name.clone(), server: s.cloned(), local: l.cloned(), changed })
        })
        .collect();
    let active = (server_active != manifest.active.as_deref()).then(|| ActiveDifference {
        server: server_active.map(str::to_owned),
        local: manifest.active.clone(),
    });
    (differences, active)
}

/// File name for a script: characters file systems don't allow (and `%`)
/// are percent-encoded, as is a leading dot.
pub fn file_name(script: &str) -> String {
    let mut out = String::new();
    for (i, c) in script.chars().enumerate() {
        let special = matches!(c, '%' | '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control();
        if special || (i == 0 && c == '.') {
            let mut buf = [0; 4];
            for b in c.encode_utf8(&mut buf).bytes() {
                out.push_str(&format!("%{b:02X}"));
            }
        } else {
            out.push(c);
        }
    }
    out + EXTENSION
}

/// The script name of a file name, if it is a script file.
pub fn script_name(file: &str) -> Option<String> {
    let stem = file.strip_suffix(EXTENSION)?;
    let bytes = stem.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// A copy on disk.
pub struct LocalCopy {
    dir: PathBuf,
}

impl LocalCopy {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The manifest, or `None` if there is no copy yet.
    pub fn manifest(&self) -> io::Result<Option<Manifest>> {
        match std::fs::read_to_string(self.dir.join(MANIFEST)) {
            Ok(json) => serde_json::from_str(&json).map(Some).map_err(io::Error::other),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    fn save_manifest(&self, manifest: &Manifest) -> io::Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        let json = serde_json::to_string_pretty(manifest).map_err(io::Error::other)?;
        std::fs::write(self.dir.join(MANIFEST), json + "\n")
    }

    fn update(&self, f: impl FnOnce(&mut Manifest)) -> io::Result<()> {
        let mut manifest = self.manifest()?.unwrap_or_default();
        f(&mut manifest);
        self.save_manifest(&manifest)
    }

    /// The scripts in the copy: every `.sieve` file in the folder.
    pub fn scripts(&self) -> io::Result<BTreeMap<String, String>> {
        let mut out = BTreeMap::new();
        let entries = match std::fs::read_dir(&self.dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(out),
            Err(e) => return Err(e),
        };
        for entry in entries {
            let entry = entry?;
            let Some(name) = entry.file_name().to_str().and_then(script_name) else { continue };
            if entry.file_type()?.is_file() {
                out.insert(name, std::fs::read_to_string(entry.path())?);
            }
        }
        Ok(out)
    }

    /// Replaces the whole copy with the server's state.
    pub fn take_over(&self, scripts: &BTreeMap<String, String>, active: Option<&str>) -> io::Result<()> {
        for name in self.scripts()?.keys() {
            if !scripts.contains_key(name) {
                std::fs::remove_file(self.dir.join(file_name(name)))?;
            }
        }
        for (name, content) in scripts {
            self.write_file(name, content)?;
        }
        self.save_manifest(&Manifest {
            active: active.map(str::to_owned),
            scripts: scripts.iter().map(|(n, c)| (n.clone(), hash(c))).collect(),
        })
    }

    fn write_file(&self, name: &str, content: &str) -> io::Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        std::fs::write(self.dir.join(file_name(name)), content)
    }

    /// Records a script as it is on the server now.
    pub fn put(&self, name: &str, content: &str) -> io::Result<()> {
        self.write_file(name, content)?;
        self.update(|m| {
            m.scripts.insert(name.to_owned(), hash(content));
        })
    }

    /// Records that a script is gone from the server.
    pub fn remove(&self, name: &str) -> io::Result<()> {
        match std::fs::remove_file(self.dir.join(file_name(name))) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
            _ => {}
        }
        self.update(|m| {
            m.scripts.remove(name);
            if m.active.as_deref() == Some(name) {
                m.active = None;
            }
        })
    }

    pub fn rename(&self, name: &str, new_name: &str) -> io::Result<()> {
        let from = self.dir.join(file_name(name));
        if from.exists() {
            std::fs::rename(from, self.dir.join(file_name(new_name)))?;
        }
        self.update(|m| {
            if let Some(h) = m.scripts.remove(name) {
                m.scripts.insert(new_name.to_owned(), h);
            }
            if m.active.as_deref() == Some(name) {
                m.active = Some(new_name.to_owned());
            }
        })
    }

    pub fn set_active(&self, name: Option<&str>) -> io::Result<()> {
        self.update(|m| m.active = name.map(str::to_owned))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(items: &[(&str, &str)]) -> BTreeMap<String, String> {
        items.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    fn manifest(items: &[(&str, &str)], active: Option<&str>) -> Manifest {
        Manifest { active: active.map(str::to_owned), scripts: items.iter().map(|(k, v)| (k.to_string(), hash(v))).collect() }
    }

    #[test]
    fn compares_server_and_copy() {
        let base = manifest(&[("a", "A"), ("b", "B"), ("c", "C"), ("d", "D")], Some("root"));
        let server = map(&[("a", "A"), ("b", "B2"), ("d", "D3"), ("new", "N")]);
        let local = map(&[("a", "A"), ("b", "B"), ("c", "C"), ("d", "D4")]);
        let (diffs, active) = compare(&server, Some("synomc"), &local, &base);
        let by_name = |n: &str| diffs.iter().find(|d| d.name == n).cloned();
        assert!(by_name("a").is_none(), "unchanged");
        assert_eq!(by_name("b").unwrap().changed, [Side::Server]);
        let c = by_name("c").unwrap();
        assert_eq!((c.server, c.changed), (None, vec![Side::Server]), "deleted on the server");
        assert_eq!(by_name("d").unwrap().changed, [Side::Server, Side::Local]);
        assert_eq!(by_name("new").unwrap().changed, [Side::Server]);
        assert_eq!(active, Some(ActiveDifference { server: Some("synomc".into()), local: Some("root".into()) }));
        let (_, active) = compare(&server, Some("root"), &local, &base);
        assert_eq!(active, None);
    }

    #[test]
    fn file_names_round_trip() {
        for name in ["sort", "sort.Banken", "a/b", "50% off", ".hidden", "x:y*z?", "Grüße"] {
            let file = file_name(name);
            assert!(!file.contains('/') && !file.starts_with('.'), "{file}");
            assert_eq!(script_name(&file).as_deref(), Some(name), "{file}");
        }
        assert_eq!(file_name("sort.Banken"), "sort.Banken.sieve");
        assert_eq!(script_name("sieve-gui.json"), None);
    }

    #[test]
    fn records_changes_on_disk() {
        let dir = std::env::temp_dir().join(format!("sieve-gui-copy-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let copy = LocalCopy::new(&dir);
        assert_eq!(copy.manifest().unwrap(), None);
        copy.take_over(&map(&[("a", "A"), ("b", "B")]), Some("a")).unwrap();
        copy.put("c", "C").unwrap();
        copy.rename("a", "a2").unwrap();
        copy.remove("b").unwrap();
        assert_eq!(copy.scripts().unwrap(), map(&[("a2", "A"), ("c", "C")]));
        let m = copy.manifest().unwrap().unwrap();
        assert_eq!(m, manifest(&[("a2", "A"), ("c", "C")], Some("a2")));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
