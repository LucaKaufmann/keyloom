//! The workspace on disk (spec 01): plain files under `$KEYLOOM_HOME`, kept in a local git repo.
//!
//! ```text
//! $KEYLOOM_HOME/
//!   config.toml
//!   layouts/<layoutId>/layout.toml
//!   layouts/<layoutId>/revisions/<revisionId>.json   immutable
//!   layouts/<layoutId>/drafts/<name>.json
//!   cache/oryx/                                      raw answers, not committed
//! ```

use crate::model::{DraftMeta, Kind, Layout};
use crate::ops::Op;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, thiserror::Error)]
pub enum WorkspaceError {
    // `error`, not `source`: the message already includes it, and a `source` would be printed twice.
    #[error("{path}: {error}")]
    Io { path: PathBuf, error: std::io::Error },
    /// A file in the workspace that can't be read as what it should be.
    #[error("{path}: {message}")]
    Format { path: PathBuf, message: String },
    #[error("{0}")]
    NotFound(String),
    /// A request that can't be carried out (not about a particular file).
    #[error("{0}")]
    Invalid(String),
    #[error("invalid id '{0}' (letters, digits, '-' and '_', at most 32)")]
    InvalidId(String),
}

type Result<T> = std::result::Result<T, WorkspaceError>;

fn io(path: &Path) -> impl FnOnce(std::io::Error) -> WorkspaceError + '_ {
    move |error| WorkspaceError::Io { path: path.to_path_buf(), error }
}

/// Ids end up in file names, so they are restricted (like `oryx::is_valid_id`, plus `-`/`_`).
pub fn validate_id(id: &str) -> Result<()> {
    let ok = !id.is_empty() && id.len() <= 32 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if ok {
        Ok(())
    } else {
        Err(WorkspaceError::InvalidId(id.to_string()))
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_layout: Option<String>,
    #[serde(default)]
    pub workspace: WorkspaceConfig,
    #[serde(default)]
    pub validate: ValidateConfig,
    #[serde(default)]
    pub live: LiveConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ValidateConfig {
    #[serde(default)]
    pub disabled_rules: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveConfig {
    #[serde(default = "default_highlight")]
    pub highlight_color: String,
}

fn default_highlight() -> String {
    "#00ffaa".into()
}

impl Default for LiveConfig {
    fn default() -> Self {
        Self { highlight_color: default_highlight() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    /// Commit every change in the workspace to its local git repo.
    #[serde(default = "yes")]
    pub git: bool,
}

fn yes() -> bool {
    true
}

impl Default for WorkspaceConfig {
    fn default() -> Self {
        Self { git: true }
    }
}

/// `layouts/<id>/layout.toml`
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LayoutMeta {
    pub layout_id: String,
    pub title: String,
    pub geometry: String,
    #[serde(default)]
    pub revisions: Vec<RevisionMeta>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RevisionMeta {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    pub imported_at: String,
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

impl RevisionMeta {
    /// "YYYY-MM-DD HH:MM:SS" from Oryx's "2026-11-12 09:56:22 UTC" or our "2026-11-13T19:28:01Z".
    pub fn sort_time(&self) -> String {
        let t = self.created_at.as_deref().unwrap_or(&self.imported_at);
        t.replace('T', " ").chars().take(19).collect()
    }
}

impl LayoutMeta {
    /// Newest revision by Oryx creation time. A revision without one (e.g. from a file) counts as
    /// created when it was imported. Ties go to the later import.
    pub fn newest(&self) -> Option<&RevisionMeta> {
        self.revisions.iter().enumerate().max_by_key(|(i, r)| (r.sort_time(), *i)).map(|(_, r)| r)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stored {
    New,
    AlreadyThere,
}

pub struct Workspace {
    root: PathBuf,
    /// Re-entrant advisory lock on `.lock`: the open file and how many guards hold it.
    lock: std::cell::RefCell<Option<(std::fs::File, usize)>>,
}

/// Held while reading-then-writing the workspace, so two keyloom processes (e.g. a terminal and
/// an MCP server) can't lose each other's changes.
pub struct LockGuard<'a> {
    ws: &'a Workspace,
}

impl Drop for LockGuard<'_> {
    fn drop(&mut self) {
        let mut slot = self.ws.lock.borrow_mut();
        if let Some((_, n)) = slot.as_mut() {
            *n -= 1;
            if *n == 0 {
                *slot = None; // closing the file releases the lock
            }
        }
    }
}

/// Lines keyloom needs in the workspace's `.gitignore`.
const IGNORED: &[&str] = &["cache/", ".lock", "*.tmp*"];

impl Workspace {
    /// `$KEYLOOM_HOME`, else `$XDG_DATA_HOME/keyloom`, else `~/.local/share/keyloom` (also on macOS).
    pub fn default_root() -> PathBuf {
        if let Some(p) = std::env::var_os("KEYLOOM_HOME").filter(|p| !p.is_empty()) {
            let p = PathBuf::from(p);
            return std::path::absolute(&p).unwrap_or(p);
        }
        // The XDG spec says relative values must be ignored.
        if let Some(p) = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).filter(|p| p.is_absolute()) {
            return p.join("keyloom");
        }
        dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")).join(".local/share/keyloom")
    }

    pub fn open_default() -> Result<Self> {
        Self::open(Self::default_root())
    }

    /// Opens (and creates on first use) a workspace.
    pub fn open(root: impl Into<PathBuf>) -> Result<Self> {
        let ws = Self { root: root.into(), lock: Default::default() };
        let fresh = !ws.root.join("layouts").is_dir();
        std::fs::create_dir_all(ws.root.join("layouts")).map_err(io(&ws.root))?;
        std::fs::create_dir_all(ws.root.join("cache/oryx")).map_err(io(&ws.root))?;
        {
            let _guard = ws.lock()?;
            ws.ensure_ignored()?;
            if fresh && !ws.root.join("config.toml").exists() {
                ws.save_config(&Config::default())?;
            }
            // Only a repo keyloom creates itself is ever committed to (see `git_owned`).
            if ws.config()?.workspace.git && !ws.root.join(".git").exists() && ws.git(&["init", "-q"]) {
                ws.git(&["config", "--local", "keyloom.workspace", "true"]);
                ws.commit("create keyloom workspace");
            }
        }
        Ok(ws)
    }

    fn ensure_ignored(&self) -> Result<()> {
        let path = self.root.join(".gitignore");
        let current = std::fs::read_to_string(&path).unwrap_or_default();
        let missing: Vec<&str> = IGNORED.iter().copied().filter(|i| !current.lines().any(|l| l.trim() == *i)).collect();
        if missing.is_empty() {
            return Ok(());
        }
        let mut text = current;
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        for m in missing {
            text.push_str(m);
            text.push('\n');
        }
        std::fs::write(&path, text).map_err(io(&path))
    }

    /// Takes the workspace lock (re-entrant within this `Workspace`). Waits up to 10 s for
    /// another keyloom process to finish.
    pub fn lock(&self) -> Result<LockGuard<'_>> {
        let mut slot = self.lock.borrow_mut();
        if let Some((_, n)) = slot.as_mut() {
            *n += 1;
            return Ok(LockGuard { ws: self });
        }
        let path = self.root.join(".lock");
        let file = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(&path).map_err(io(&path))?;
        let started = std::time::Instant::now();
        loop {
            match file.try_lock() {
                Ok(()) => break,
                Err(std::fs::TryLockError::WouldBlock) if started.elapsed() < std::time::Duration::from_secs(10) => {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                Err(std::fs::TryLockError::WouldBlock) => {
                    return Err(WorkspaceError::Invalid(
                        "the workspace is busy: another keyloom process is writing to it; try again".into(),
                    ));
                }
                // Locks aren't supported on this file system: carry on without one.
                Err(std::fs::TryLockError::Error(_)) => break,
            }
        }
        *slot = Some((file, 1));
        Ok(LockGuard { ws: self })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    // MARK: config

    pub fn config(&self) -> Result<Config> {
        let path = self.root.join("config.toml");
        match std::fs::read_to_string(&path) {
            Ok(text) => toml::from_str(&text).map_err(|e| WorkspaceError::Format { path, message: e.to_string() }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
            Err(e) => Err(WorkspaceError::Io { path, error: e }),
        }
    }

    pub fn save_config(&self, config: &Config) -> Result<()> {
        let path = self.root.join("config.toml");
        let text = toml::to_string_pretty(config).map_err(|e| WorkspaceError::Format { path: path.clone(), message: e.to_string() })?;
        write_atomic(&path, text.as_bytes())
    }

    /// Sets one top-level string in config.toml, keeping comments and keys keyloom doesn't know.
    fn set_config_string(&self, key: &str, value: &str) -> Result<()> {
        let path = self.root.join("config.toml");
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(WorkspaceError::Io { path, error: e }),
        };
        let mut doc: toml_edit::DocumentMut =
            text.parse().map_err(|e: toml_edit::TomlError| WorkspaceError::Format { path: path.clone(), message: e.to_string() })?;
        doc[key] = toml_edit::value(value);
        write_atomic(&path, doc.to_string().as_bytes())
    }

    pub fn set_active(&self, layout_id: &str) -> Result<()> {
        let _guard = self.lock()?;
        self.meta(layout_id)?;
        self.set_config_string("active_layout", layout_id)?;
        self.commit(&format!("use {layout_id}"));
        Ok(())
    }

    // MARK: layouts

    fn layout_dir(&self, layout_id: &str) -> PathBuf {
        self.root.join("layouts").join(layout_id)
    }

    pub fn layouts(&self) -> Result<Vec<LayoutMeta>> {
        let dir = self.root.join("layouts");
        let mut out = Vec::new();
        for entry in std::fs::read_dir(&dir).map_err(io(&dir))? {
            let entry = entry.map_err(io(&dir))?;
            if entry.path().join("layout.toml").is_file() {
                out.push(self.meta(&entry.file_name().to_string_lossy())?);
            }
        }
        out.sort_by(|a, b| a.layout_id.cmp(&b.layout_id));
        Ok(out)
    }

    pub fn meta(&self, layout_id: &str) -> Result<LayoutMeta> {
        validate_id(layout_id)?;
        let path = self.layout_dir(layout_id).join("layout.toml");
        let text = std::fs::read_to_string(&path).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => WorkspaceError::NotFound(format!(
                "layout {layout_id} isn't imported (see `keyloom list`; import with `keyloom import <oryx link or id>`)"
            )),
            _ => WorkspaceError::Io { path: path.clone(), error: e },
        })?;
        let meta: LayoutMeta = toml::from_str(&text).map_err(|e| WorkspaceError::Format { path, message: e.to_string() })?;
        // Oryx ids are case-sensitive; on a case-insensitive file system `9qmjk` would open `9qMJK`.
        if meta.layout_id != layout_id {
            return Err(WorkspaceError::NotFound(format!(
                "layout {layout_id} isn't imported (there is {}, whose id differs only in letter case)",
                meta.layout_id
            )));
        }
        Ok(meta)
    }

    fn save_meta(&self, meta: &LayoutMeta) -> Result<()> {
        let path = self.layout_dir(&meta.layout_id).join("layout.toml");
        let text = toml::to_string_pretty(meta).map_err(|e| WorkspaceError::Format { path: path.clone(), message: e.to_string() })?;
        write_atomic(&path, text.as_bytes())
    }

    fn revision_path(&self, layout_id: &str, revision_id: &str) -> PathBuf {
        self.layout_dir(layout_id).join("revisions").join(format!("{revision_id}.json"))
    }

    /// Stores an imported revision. Revisions are immutable: storing one that exists is a no-op.
    /// `raw` (e.g. the GraphQL answer) goes to the uncommitted cache.
    pub fn store_revision(&self, layout: &Layout, raw: Option<&Value>) -> Result<Stored> {
        validate_id(&layout.layout_id)?;
        validate_id(&layout.revision_id)?;
        if layout.kind != Kind::Revision {
            return Err(WorkspaceError::Invalid("only revisions can be stored as revisions".into()));
        }
        let _guard = self.lock()?;
        // Ids that differ only in letter case would share a folder or file on a case-insensitive
        // file system (the macOS default): refuse instead of mixing two layouts or revisions.
        let clash = |dir: PathBuf, name: &str, suffix: &str| {
            std::fs::read_dir(dir)
                .ok()
                .into_iter()
                .flatten()
                .filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().to_string())
                .find(|f| {
                    let stem = f.strip_suffix(suffix).unwrap_or(f);
                    stem != name && stem.eq_ignore_ascii_case(name)
                })
        };
        if let Some(other) = clash(self.root.join("layouts"), &layout.layout_id, "") {
            return Err(WorkspaceError::Invalid(format!(
                "layout id {} differs from the imported {other} only in letter case, which this workspace can't keep apart",
                layout.layout_id
            )));
        }
        if let Some(other) = clash(self.layout_dir(&layout.layout_id).join("revisions"), &layout.revision_id, ".json") {
            return Err(WorkspaceError::Invalid(format!(
                "revision id {} differs from the imported {} only in letter case, which this workspace can't keep apart",
                layout.revision_id,
                other.trim_end_matches(".json")
            )));
        }
        // A layout.toml that exists but can't be read is an error, not a reason to start over.
        let mut meta = match self.meta(&layout.layout_id) {
            Ok(m) => m,
            Err(WorkspaceError::NotFound(_)) => LayoutMeta {
                layout_id: layout.layout_id.clone(),
                title: layout.title.clone(),
                geometry: layout.geometry.clone(),
                revisions: vec![],
            },
            Err(e) => return Err(e),
        };
        if let Some(raw) = raw {
            let path = self.root.join("cache/oryx").join(format!("{}-{}.json", layout.layout_id, layout.revision_id));
            write_atomic(&path, serde_json::to_string_pretty(raw).unwrap_or_default().as_bytes())?;
        }
        let path = self.revision_path(&layout.layout_id, &layout.revision_id);
        let known = meta.revisions.iter().any(|r| r.id == layout.revision_id);
        if path.exists() && known {
            // The layout itself never changes. Firmware settings that keyloom derives from Oryx's
            // generated source (the auto-mouse layer) may be filled in once, by a fetch from Oryx
            // (`raw` is its answer), if the stored revision doesn't have them yet.
            let mut stored = self.load_revision(&layout.layout_id, &layout.revision_id)?;
            // Same id, different keys: not the same revision. Keep what is stored and say so.
            let differs = crate::diff::diff(&stored, layout, crate::diff::DiffOptions { ignore_metadata: true });
            if !differs.is_empty() {
                return Err(WorkspaceError::Invalid(format!(
                    "{} is already imported with different content ({}); revisions don't change, so this was not imported. If it is an edited copy, give it another revision id",
                    stored.reference(),
                    differs.summary
                )));
            }
            if raw.is_some() && stored.settings == Default::default() && layout.settings != Default::default() {
                stored.settings = layout.settings.clone();
                write_atomic(&path, pretty(&stored).as_bytes())?;
                // Drafts already based on this revision get them too (if they have none).
                for info in self.drafts()?.into_iter().filter(|d| d.layout_id == layout.layout_id && d.based_on == layout.revision_id) {
                    let (draft_path, base_path, _) = self.draft_paths(&info.layout_id, &info.name);
                    for p in [draft_path, base_path] {
                        let Ok(text) = std::fs::read_to_string(&p) else { continue };
                        let Ok(mut d) = serde_json::from_str::<Layout>(&text) else { continue };
                        if d.settings == Default::default() {
                            // The draft may have renumbered its layers: follow the layer by title.
                            let by_title =
                                |n: i64| layout.layer(n).and_then(|l| d.layers.iter().find(|x| x.title == l.title)).map(|x| x.position);
                            d.settings.auto_mouse_layer = layout.settings.auto_mouse_layer.and_then(by_title);
                            write_atomic(&p, pretty(&d).as_bytes())?;
                        }
                    }
                }
                self.commit(&format!("{}: firmware settings from Oryx", stored.reference()));
            }
            return Ok(Stored::AlreadyThere);
        }
        std::fs::create_dir_all(path.parent().expect("has parent")).map_err(io(&path))?;
        if !path.exists() {
            // Oryx knows nothing of keyloom's layer tags: carry them over to the new revision
            // from the drafts and the newest revision so far, by layer title.
            let mut layout = layout.clone();
            let mut sources: Vec<Layout> = self
                .drafts()
                .unwrap_or_default()
                .iter()
                .filter(|d| d.layout_id == layout.layout_id)
                .filter_map(|d| self.load_draft(&d.name).ok())
                .collect();
            sources.extend(meta.newest().and_then(|r| self.load_revision(&layout.layout_id, &r.id).ok()));
            for l in layout.layers.iter_mut().filter(|l| l.keyloom.tags.is_empty()) {
                if let Some(tags) = sources
                    .iter()
                    .flat_map(|s| &s.layers)
                    .find(|o| o.title == l.title && !o.keyloom.tags.is_empty())
                    .map(|o| o.keyloom.tags.clone())
                {
                    l.keyloom.tags = tags;
                }
            }
            write_atomic(&path, pretty(&layout).as_bytes())?;
        }
        if !known {
            meta.revisions.push(RevisionMeta {
                id: layout.revision_id.clone(),
                created_at: layout.created_at.clone(),
                imported_at: layout.source.fetched_at.clone(),
                source: layout.source.kind.clone(),
                title: layout.revision_title.clone(),
            });
        }
        // The layout's title follows its newest revision, not whichever was imported last.
        if meta.newest().map(|r| r.id.as_str()) == Some(layout.revision_id.as_str()) {
            meta.title = layout.title.clone();
        }
        self.save_meta(&meta)?;
        if self.config()?.active_layout.is_none() {
            self.set_config_string("active_layout", &layout.layout_id)?;
        }
        self.commit(&format!("import {}@{} ({})", layout.layout_id, layout.revision_id, layout.source.kind));
        Ok(Stored::New)
    }

    pub fn load_revision(&self, layout_id: &str, revision_id: &str) -> Result<Layout> {
        validate_id(layout_id)?;
        validate_id(revision_id)?;
        let path = self.revision_path(layout_id, revision_id);
        let text = std::fs::read_to_string(&path).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => WorkspaceError::NotFound(format!("revision {layout_id}@{revision_id} isn't imported")),
            _ => WorkspaceError::Io { path: path.clone(), error: e },
        })?;
        let layout: Layout = serde_json::from_str(&text).map_err(|e| WorkspaceError::Format { path, message: e.to_string() })?;
        if layout.revision_id != revision_id {
            return Err(WorkspaceError::NotFound(format!(
                "revision {layout_id}@{revision_id} isn't imported (there is @{}, which differs only in letter case)",
                layout.revision_id
            )));
        }
        Ok(layout)
    }

    /// Resolves `[layoutId][@revisionId]` or `draft:<name>` (spec 01). No layout id → the active
    /// layout, else the only one. No revision → the newest stored. (`@latest` is handled by callers.)
    pub fn resolve(&self, reference: Option<&str>) -> Result<Layout> {
        if let Some(name) = reference.and_then(|r| r.trim().strip_prefix("draft:")) {
            return self.load_draft(name);
        }
        let (layout, revision) = match reference.map(str::trim).filter(|s| !s.is_empty()) {
            Some(r) => match r.split_once('@') {
                Some((l, rev)) => (Some(l).filter(|l| !l.is_empty()), Some(rev)),
                None => (Some(r), None),
            },
            None => (None, None),
        };
        let layout_id = match layout {
            Some(l) => l.to_string(),
            None => self.default_layout()?,
        };
        let meta = match self.meta(&layout_id) {
            Ok(m) => m,
            // A bare name that is a draft, not a layout: `wow` means `draft:wow`.
            Err(WorkspaceError::NotFound(_)) if revision.is_none() && layout.is_some() && self.find_draft(&layout_id).is_ok() => {
                return self.load_draft(&layout_id);
            }
            Err(WorkspaceError::NotFound(_)) => {
                return Err(WorkspaceError::NotFound(format!(
                    "no layout or draft called '{layout_id}' (see `keyloom list` / MCP list_layouts; layouts are imported with `keyloom import <oryx link or id>` / import_layout)"
                )));
            }
            Err(e) => return Err(e),
        };
        let revision_id = match revision {
            Some(r) => r.to_string(),
            None => meta
                .newest()
                .map(|r| r.id.clone())
                .ok_or_else(|| WorkspaceError::NotFound(format!("layout {layout_id} has no revisions")))?,
        };
        self.load_revision(&layout_id, &revision_id)
    }

    pub fn default_layout(&self) -> Result<String> {
        if let Some(active) = self.config()?.active_layout {
            return Ok(active);
        }
        let layouts = self.layouts()?;
        match layouts.as_slice() {
            [only] => Ok(only.layout_id.clone()),
            [] => Err(WorkspaceError::NotFound("no layouts yet; import one with `keyloom import <oryx link>` (MCP: import_layout)".into())),
            _ => Err(WorkspaceError::NotFound("several layouts; pick one with `keyloom use <layoutId>` or name it".into())),
        }
    }

    // MARK: drafts

    fn drafts_dir(&self, layout_id: &str) -> PathBuf {
        self.layout_dir(layout_id).join("drafts")
    }

    /// Which layout a draft belongs to (draft names are unique across the workspace).
    pub fn find_draft(&self, name: &str) -> Result<String> {
        let name = name.strip_prefix("draft:").unwrap_or(name);
        validate_id(name)?;
        for meta in self.layouts()? {
            if self.drafts_dir(&meta.layout_id).join(format!("{name}.json")).is_file() {
                return Ok(meta.layout_id);
            }
        }
        Err(WorkspaceError::NotFound(format!("no draft called '{name}' (see `keyloom draft list`; MCP: list_layouts)")))
    }

    fn draft_paths(&self, layout_id: &str, name: &str) -> (PathBuf, PathBuf, PathBuf) {
        let d = self.drafts_dir(layout_id);
        (d.join(format!("{name}.json")), d.join(format!("{name}.base.json")), d.join(format!("{name}.ops.jsonl")))
    }

    /// Creates a draft as a copy of `from` (a revision or another draft).
    pub fn create_draft(&self, name: &str, from: &Layout) -> Result<Layout> {
        validate_id(name)?;
        let _guard = self.lock()?;
        if self.find_draft(name).is_ok() {
            return Err(WorkspaceError::Invalid(format!(
                "a draft called '{name}' already exists (see `keyloom draft list`; MCP: list_layouts)"
            )));
        }
        if self.layout_dir(name).join("layout.toml").is_file() {
            return Err(WorkspaceError::Invalid(format!("'{name}' is the id of a layout; pick another draft name")));
        }
        // A draft belongs to an imported revision: that is what the guide and verify compare with.
        validate_id(&from.layout_id)?;
        validate_id(&from.revision_id)?;
        if !self.revision_path(&from.layout_id, &from.revision_id).is_file() {
            return Err(WorkspaceError::Invalid(format!(
                "{}@{} isn't imported, so a draft can't be based on it; import it first (`keyloom import <file or id>`, MCP: import_layout)",
                from.layout_id, from.revision_id
            )));
        }
        let now = crate::time::now_rfc3339();
        let mut draft = from.clone();
        draft.kind = Kind::Draft;
        draft.draft = Some(DraftMeta { name: name.into(), created: now.clone(), updated: now, notes: None });
        let (path, base, ops) = self.draft_paths(&draft.layout_id, name);
        write_atomic(&base, pretty(&draft).as_bytes())?;
        write_atomic(&path, pretty(&draft).as_bytes())?;
        write_atomic(&ops, b"")?;
        self.commit(&format!("draft {name}: create from {}", from.reference()));
        Ok(draft)
    }

    pub fn load_draft(&self, name: &str) -> Result<Layout> {
        let name = name.strip_prefix("draft:").unwrap_or(name);
        let layout_id = self.find_draft(name)?;
        let (path, _, _) = self.draft_paths(&layout_id, name);
        let text = std::fs::read_to_string(&path).map_err(io(&path))?;
        serde_json::from_str(&text).map_err(|e| WorkspaceError::Format { path, message: e.to_string() })
    }

    /// Saves a draft after `ops` were applied and logs them (for undo and explanations).
    pub fn save_draft(&self, draft: &mut Layout, ops: &[Op]) -> Result<()> {
        let _guard = self.lock()?;
        let Some(meta) = draft.draft.as_mut() else {
            return Err(WorkspaceError::Invalid("not a draft".into()));
        };
        meta.updated = crate::time::now_rfc3339();
        let name = meta.name.clone();
        let (path, _, log) = self.draft_paths(&draft.layout_id, &name);
        write_atomic(&path, pretty(draft).as_bytes())?;
        if !ops.is_empty() {
            use std::io::Write;
            let line = serde_json::json!({"at": meta_updated(draft), "ops": ops});
            let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&log).map_err(io(&log))?;
            writeln!(f, "{line}").map_err(io(&log))?;
            let what: Vec<String> = ops.iter().map(op_summary).collect();
            self.commit(&format!("draft {name}: {}", what.join("; ")));
        }
        Ok(())
    }

    /// Batches of ops applied to a draft so far.
    pub fn draft_log(&self, name: &str) -> Result<Vec<Vec<Op>>> {
        let name = name.strip_prefix("draft:").unwrap_or(name);
        let layout_id = self.find_draft(name)?;
        let (_, _, log) = self.draft_paths(&layout_id, name);
        let text = std::fs::read_to_string(&log).unwrap_or_default();
        text.lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| {
                let v: Value = serde_json::from_str(l).map_err(|e| WorkspaceError::Format { path: log.clone(), message: e.to_string() })?;
                serde_json::from_value(v["ops"].clone()).map_err(|e| WorkspaceError::Format { path: log.clone(), message: e.to_string() })
            })
            .collect()
    }

    /// Reverts the last batch of ops by replaying the others on the draft's starting point.
    pub fn undo_draft(&self, name: &str) -> Result<Layout> {
        let _guard = self.lock()?;
        let name = name.strip_prefix("draft:").unwrap_or(name);
        let layout_id = self.find_draft(name)?;
        let (path, base, log) = self.draft_paths(&layout_id, name);
        let mut batches = self.draft_log(name)?;
        if batches.pop().is_none() {
            return Err(WorkspaceError::NotFound(format!("draft '{name}' has nothing to undo")));
        }
        let text = std::fs::read_to_string(&base).map_err(io(&base))?;
        let mut draft: Layout =
            serde_json::from_str(&text).map_err(|e| WorkspaceError::Format { path: base.clone(), message: e.to_string() })?;
        for ops in &batches {
            crate::ops::apply_ops(&mut draft, ops)
                .map_err(|e| WorkspaceError::Format { path: log.clone(), message: format!("replay failed: {e}") })?;
        }
        if let Some(m) = draft.draft.as_mut() {
            m.updated = crate::time::now_rfc3339();
        }
        write_atomic(&path, pretty(&draft).as_bytes())?;
        let lines: Vec<String> =
            std::fs::read_to_string(&log).unwrap_or_default().lines().filter(|l| !l.trim().is_empty()).map(str::to_string).collect();
        let kept = lines[..lines.len().saturating_sub(1)].join("\n");
        write_atomic(&log, if kept.is_empty() { Vec::new() } else { format!("{kept}\n").into_bytes() }.as_slice())?;
        self.commit(&format!("draft {name}: undo"));
        Ok(draft)
    }

    pub fn drafts(&self) -> Result<Vec<DraftInfo>> {
        let mut out = Vec::new();
        for meta in self.layouts()? {
            let dir = self.drafts_dir(&meta.layout_id);
            let Ok(rd) = std::fs::read_dir(&dir) else { continue };
            for e in rd.filter_map(|e| e.ok()) {
                let file = e.file_name().to_string_lossy().to_string();
                let Some(name) = file.strip_suffix(".json").filter(|n| !n.ends_with(".base")) else { continue };
                // A stray or unreadable file in the folder is not a draft: skip it.
                let Ok(d) = self.load_draft(name) else { continue };
                if d.draft.as_ref().map(|m| m.name.as_str()) != Some(name) || d.layout_id != meta.layout_id {
                    continue;
                }
                let ops = self.draft_log(name).map(|b| b.iter().map(Vec::len).sum()).unwrap_or(0);
                out.push(DraftInfo {
                    name: name.to_string(),
                    layout_id: d.layout_id.clone(),
                    based_on: d.revision_id.clone(),
                    updated: d.draft.as_ref().map(|m| m.updated.clone()).unwrap_or_default(),
                    ops,
                });
            }
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
    }

    pub fn remove_draft(&self, name: &str) -> Result<()> {
        let _guard = self.lock()?;
        let name = name.strip_prefix("draft:").unwrap_or(name);
        let layout_id = self.find_draft(name)?;
        let (a, b, c) = self.draft_paths(&layout_id, name);
        for p in [a, b, c] {
            if p.exists() {
                std::fs::remove_file(&p).map_err(io(&p))?;
            }
        }
        self.commit(&format!("draft {name}: remove"));
        Ok(())
    }

    /// Moves a finished draft to `drafts/archive/`.
    pub fn archive_draft(&self, name: &str) -> Result<PathBuf> {
        let _guard = self.lock()?;
        let name = name.strip_prefix("draft:").unwrap_or(name);
        let layout_id = self.find_draft(name)?;
        let stamp = crate::time::now_rfc3339().replace([':', '-'], "");
        let archive = self.drafts_dir(&layout_id).join("archive");
        std::fs::create_dir_all(&archive).map_err(io(&archive))?;
        let (a, b, c) = self.draft_paths(&layout_id, name);
        let target = archive.join(format!("{name}-{stamp}.json"));
        std::fs::rename(&a, &target).map_err(io(&a))?;
        let _ = std::fs::rename(&c, archive.join(format!("{name}-{stamp}.ops.jsonl")));
        let _ = std::fs::remove_file(&b);
        self.commit(&format!("draft {name}: archive"));
        Ok(target)
    }

    // MARK: git

    /// `git -C <root>`, shielded from an environment that points git at another repository
    /// (e.g. when keyloom is run from a git hook).
    fn git_command(root: &Path) -> Command {
        let mut c = Command::new("git");
        for var in ["GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE", "GIT_COMMON_DIR", "GIT_NAMESPACE"] {
            c.env_remove(var);
        }
        c.arg("-C").arg(root);
        c
    }

    fn git_enabled(&self) -> bool {
        self.config().map(|c| c.workspace.git).unwrap_or(false)
    }

    fn git(&self, args: &[&str]) -> bool {
        Self::git_command(&self.root)
            .args(["-c", "user.name=keyloom", "-c", "user.email=keyloom@localhost", "-c", "commit.gpgsign=false"])
            .args(args)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    /// keyloom only commits to a repo it created (marked in the repo's config), never to a
    /// repo of the user's that happens to contain the workspace. Workspaces from before the
    /// marker existed are recognised by their first commit.
    fn git_owned(&self) -> bool {
        if !self.root.join(".git").exists() {
            return false;
        }
        if self.git(&["config", "--local", "--get", "keyloom.workspace"]) {
            return true;
        }
        let first = Self::git_command(&self.root).args(["log", "--max-parents=0", "--format=%s"]).output();
        let ours = first.is_ok_and(|o| String::from_utf8_lossy(&o.stdout).lines().any(|l| l == "create keyloom workspace"));
        if ours {
            self.git(&["config", "--local", "keyloom.workspace", "true"]);
        }
        ours
    }

    /// Commits everything in the workspace. Best effort: no git, nothing to commit or a failing
    /// commit never fail the operation.
    pub fn commit(&self, message: &str) {
        if !self.git_enabled() || !self.git_owned() {
            return;
        }
        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "-m", message]);
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DraftInfo {
    pub name: String,
    pub layout_id: String,
    pub based_on: String,
    pub updated: String,
    pub ops: usize,
}

fn meta_updated(l: &Layout) -> String {
    l.draft.as_ref().map(|m| m.updated.clone()).unwrap_or_default()
}

/// One-line description of an op for commit messages.
pub fn op_summary(op: &Op) -> String {
    let v = serde_json::to_value(op).unwrap_or_default();
    let name = v["op"].as_str().unwrap_or("op").to_string();
    let mut parts = vec![name];
    for k in ["layer", "key", "title", "action", "slot", "to"] {
        if let Some(x) = v.get(k).filter(|x| !x.is_null()) {
            parts.push(x.as_str().map(str::to_string).unwrap_or_else(|| x.to_string()));
        }
    }
    parts.join(" ")
}

pub fn pretty(layout: &Layout) -> String {
    let mut s = serde_json::to_string_pretty(layout).unwrap_or_default();
    s.push('\n');
    s
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(io(parent))?;
    }
    // Unique per process, next to the target (same file system, so the rename is atomic).
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".tmp{}", std::process::id()));
    let tmp = path.with_file_name(name);
    std::fs::write(&tmp, bytes).map_err(io(&tmp))?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        io(path)(e)
    })
}
