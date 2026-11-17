//! Where layouts come from: Oryx GraphQL, Keymapp's local database, files (spec 01).

use crate::model::{Layout, ModelError, Source, SCHEMA};
pub use crate::oryx::LayoutRef;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const ORYX_ENDPOINT: &str = "https://oryx.zsa.io/graphql";

/// The layout query (geometry, revision, layers, combos) plus `createdAt`, `config`, `qmkVersion` and `zipUrl` (docs/oryx-encodings.md).
pub const ORYX_QUERY: &str = "query getLayout($hashId: String!, $revisionId: String!, $geometry: String) {
  layout(hashId: $hashId, geometry: $geometry, revisionId: $revisionId) {
    hashId
    title
    geometry
    revision {
      hashId
      title
      createdAt
      config
      qmkVersion
      zipUrl
      layers { title position color keys }
      combos { keyIndices layerIdx name trigger }
    }
  }
}";

#[derive(Debug, thiserror::Error)]
pub enum SourceError {
    #[error("{0}")]
    Network(String),
    #[error("Oryx: {0}")]
    Oryx(String),
    /// The layout is private: only its owner, signed in, can read it.
    #[error("Oryx: {0}")]
    Private(String),
    #[error("{0}")]
    Model(#[from] ModelError),
    #[error("{path}: {message}")]
    File { path: PathBuf, message: String },
    #[error("Keymapp database: {0}")]
    Keymapp(String),
}

/// What the user asked to import.
#[derive(Debug, Clone)]
pub enum ImportInput {
    Oryx(LayoutRef),
    File(PathBuf),
    /// Everything Keymapp has stored locally.
    Keymapp,
}

impl ImportInput {
    pub fn parse(input: &str) -> Result<Self, String> {
        let s = input.trim();
        if s.eq_ignore_ascii_case("keymapp") {
            return Ok(Self::Keymapp);
        }
        let path = Path::new(s);
        // A file is named by its .json ending or a path; a bare word is always a layout id,
        // even if a file of that name happens to be in the current directory.
        if s.ends_with(".json") || (s.contains(std::path::MAIN_SEPARATOR) && path.is_file()) {
            return Ok(Self::File(path.to_path_buf()));
        }
        LayoutRef::from_user_input(s)
            .map(Self::Oryx)
            .ok_or_else(|| format!("'{s}' is not an Oryx link, a layout id (like 9qMJK or 9qMJK/ZPjbXz), a .json file or 'keymapp'"))
    }
}

pub fn now() -> String {
    crate::time::now_rfc3339()
}

fn source(kind: &str) -> Source {
    Source { kind: kind.into(), fetched_at: now() }
}

// MARK: Oryx

/// Fetches a revision from Oryx. Returns the raw answer (kept for debugging) and the layout.
pub fn fetch_oryx(r: &LayoutRef) -> Result<(Value, Layout), SourceError> {
    fetch_oryx_with(r, None)
}

/// Whether the revision in an Oryx answer is compiled. Compiled revisions never change; an
/// uncompiled one (changes in the owner's account not compiled yet) can change under the same id.
pub fn is_compiled(answer: &Value) -> bool {
    !answer["data"]["layout"]["revision"]["qmkVersion"].is_null()
}

/// Like `fetch_oryx`, signed in with the user's login token when given: then private layouts can be
/// read, and for the owner `latest` is the newest revision even if it isn't compiled.
pub fn fetch_oryx_with(r: &LayoutRef, token: Option<&str>) -> Result<(Value, Layout), SourceError> {
    let body = serde_json::json!({
        "operationName": "getLayout",
        "query": ORYX_QUERY,
        "variables": {
            "hashId": r.hash_id,
            "revisionId": r.revision_id,
            "geometry": r.geometry.as_deref().unwrap_or("voyager"),
        },
    });
    let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(30)).user_agent("keyloom").build();
    let mut request = agent.post(ORYX_ENDPOINT).set("Content-Type", "application/json");
    if let Some(token) = token.map(str::trim).filter(|t| !t.is_empty()) {
        request = request.set("Authorization", &format!("Bearer {token}"));
    }
    let response = request.send_string(&body.to_string()).map_err(|e| match e {
        ureq::Error::Status(code, _) => SourceError::Oryx(format!("HTTP {code}")),
        other => SourceError::Network(format!("Could not reach Oryx: {other}")),
    })?;
    let text = response.into_string().map_err(|e| SourceError::Network(e.to_string()))?;
    let answer: Value = serde_json::from_str(&text).map_err(|e| SourceError::Oryx(format!("bad answer: {e}")))?;
    let mut layout = layout_from_graphql(&answer, r)?;
    if let Some(url) = answer["data"]["layout"]["revision"]["zipUrl"].as_str() {
        match fetch_config_h(&agent, url) {
            Ok(config) => layout.settings.auto_mouse_layer = parse_auto_mouse_layer(&config),
            Err(e) => crate::debug!("source zip: {e}"),
        }
    }
    Ok((answer, layout))
}

/// Reads `config.h` from Oryx's generated source zip (only that file is unpacked).
fn fetch_config_h(agent: &ureq::Agent, url: &str) -> Result<String, String> {
    use std::io::Read;
    if !url.starts_with("https://oryx.zsa.io/") {
        return Err(format!("unexpected source URL {url}"));
    }
    let mut bytes = Vec::new();
    agent
        .get(url)
        .call()
        .map_err(|e| e.to_string())?
        .into_reader()
        .take(20 * 1024 * 1024)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| e.to_string())?;
    for i in 0..archive.len() {
        let file = archive.by_index(i).map_err(|e| e.to_string())?;
        let name = file.name().map_err(|e| e.to_string())?.to_string();
        if name.ends_with("/config.h") || name == "config.h" {
            let mut text = String::new();
            file.take(1024 * 1024).read_to_string(&mut text).map_err(|e| e.to_string())?;
            return Ok(text);
        }
    }
    Err("no config.h in the source zip".into())
}

/// `#define AUTOMOUSE_LAYER 4` → 4.
pub fn parse_auto_mouse_layer(config_h: &str) -> Option<i64> {
    config_h.lines().find_map(|line| {
        let mut parts = line.split_whitespace();
        (parts.next() == Some("#define") && parts.next() == Some("AUTOMOUSE_LAYER")).then(|| parts.next()?.parse().ok())?
    })
}

fn layout_from_graphql(answer: &Value, r: &LayoutRef) -> Result<Layout, SourceError> {
    if let Some(message) = answer["errors"][0]["message"].as_str() {
        if message.eq_ignore_ascii_case("NotFound") || message.to_lowercase().contains("not found") {
            return Err(SourceError::Oryx(format!(
                "there is no public layout {r} (check the id or link; a private layout can't be fetched; a file needs its path ending in .json)"
            )));
        }
        if message.to_lowercase().contains("private") {
            return Err(SourceError::Private(format!("layout {r} is private, so only its owner, signed in, can read it")));
        }
        return Err(SourceError::Oryx(message.to_string()));
    }
    let found = &answer["data"]["layout"];
    if found.is_null() {
        return Err(SourceError::Oryx(format!("layout {r} not found")));
    }
    Ok(Layout::from_oryx(found, source("oryx"))?)
}

// MARK: Keymapp (read-only)

/// Keymapp's local database. macOS path verified; the Linux and Windows paths are educated guesses.
pub fn keymapp_db_path() -> Option<PathBuf> {
    if cfg!(target_os = "macos") {
        dirs::home_dir().map(|h| h.join("Library/Application Support/.keymapp/keymapp.sqlite3"))
    } else {
        dirs::config_dir().map(|c| c.join(".keymapp/keymapp.sqlite3"))
    }
}

/// All revisions Keymapp has stored, as layouts. Opens the database read-only.
pub fn read_keymapp(db: &Path) -> Result<Vec<Layout>, SourceError> {
    use rusqlite::{Connection, OpenFlags};
    let err = |e: rusqlite::Error| SourceError::Keymapp(e.to_string());
    let conn = Connection::open_with_flags(db, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX).map_err(err)?;
    let mut stmt = conn.prepare("SELECT revisionId, data FROM revision").map_err(err)?;
    let rows = stmt
        .query_map([], |row| {
            let id: String = row.get(0)?;
            // `data` is a BLOB holding JSON text; accept TEXT too.
            let data: Vec<u8> = match row.get_ref(1)? {
                rusqlite::types::ValueRef::Blob(b) | rusqlite::types::ValueRef::Text(b) => b.to_vec(),
                _ => Vec::new(),
            };
            Ok((id, data))
        })
        .map_err(err)?;
    let mut out = Vec::new();
    for row in rows {
        let (id, data) = row.map_err(err)?;
        let value: Value = match serde_json::from_slice(&data) {
            Ok(v) => v,
            Err(e) => {
                crate::debug!("Keymapp revision {id} isn't JSON: {e}");
                continue;
            }
        };
        match Layout::from_oryx(&value["layout"], source("keymapp")) {
            Ok(l) => out.push(l),
            Err(e) => crate::debug!("Keymapp revision {id} didn't decode: {e}"),
        }
    }
    Ok(out)
}

// MARK: Files

/// Reads a keyloom layout file, a GraphQL answer, Keymapp's `{"layout": …}` or a bare Oryx layout.
pub fn read_file(path: &Path) -> Result<Layout, SourceError> {
    let file_err = |message: String| SourceError::File { path: path.to_path_buf(), message };
    let text = std::fs::read_to_string(path).map_err(|e| file_err(e.to_string()))?;
    let value: Value = serde_json::from_str(&text).map_err(|e| file_err(format!("not JSON: {e}")))?;
    parse_value(&value).map_err(|e| file_err(e.to_string()))
}

pub fn parse_value(value: &Value) -> Result<Layout, SourceError> {
    if value["schema"].as_str() == Some(SCHEMA) {
        return Ok(serde_json::from_value(value.clone()).map_err(ModelError::from)?);
    }
    if !value["data"]["layout"].is_null() || !value["errors"].is_null() {
        let r = LayoutRef::new("file", "latest", None);
        return layout_from_graphql(value, &r);
    }
    let layout = if value["layout"].is_object() { &value["layout"] } else { value };
    Ok(Layout::from_oryx(layout, source("file"))?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_revisions_have_a_qmk_version() {
        assert!(is_compiled(&serde_json::json!({"data": {"layout": {"revision": {"qmkVersion": "25.0"}}}})));
        assert!(!is_compiled(&serde_json::json!({"data": {"layout": {"revision": {"qmkVersion": null}}}})));
    }

    #[test]
    fn a_private_layout_is_its_own_error() {
        let r = LayoutRef::new("ABCDE", "latest", None);
        let e = layout_from_graphql(&serde_json::json!({"errors": [{"message": "PrivateLayout"}]}), &r).unwrap_err();
        assert!(matches!(e, SourceError::Private(_)), "{e:?}");
    }
}
