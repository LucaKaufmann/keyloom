//! Writes a draft into a NEW layout on Oryx (docs/oryx-write-api.md).
//!
//! No login is sent, so nothing in anyone's account can change: Oryx is asked for an editable copy
//! of the draft's base revision (an anonymous layout), and only that copy is edited. The copy is
//! not compiled; the user opens its link, checks it, and compiles and flashes in Oryx.
//!
//! Oryx's write calls are undocumented. Everything goes through [`Transport`], so tests run
//! against an in-memory fake.

use crate::diff::{diff, Change, DiffOptions};
use crate::model::{Layer, Layout, Source};
use serde::Serialize;
use serde_json::{json, Value};
use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum PushError {
    #[error("{0}")]
    Network(String),
    #[error("Oryx refused: {0}")]
    Oryx(String),
    #[error("{0}")]
    Unsupported(String),
    /// The copy exists but not everything was written.
    #[error("{message}. The new layout is incomplete: {url}")]
    Partial { url: String, message: String },
}

/// One GraphQL call. Returns the answer's `data`, or the first error as [`PushError::Oryx`].
pub trait Transport {
    fn call(&mut self, query: &str, variables: Value) -> Result<Value, PushError>;
}

/// The real Oryx.
pub struct OryxHttp {
    agent: ureq::Agent,
}

impl OryxHttp {
    pub fn new() -> Self {
        Self { agent: ureq::AgentBuilder::new().timeout(Duration::from_secs(60)).user_agent("keyloom").build() }
    }
}

impl Default for OryxHttp {
    fn default() -> Self {
        Self::new()
    }
}

impl OryxHttp {
    fn post(&self, body: &str) -> Result<String, Box<ureq::Error>> {
        let sent = self.agent.post(crate::source::ORYX_ENDPOINT).set("Content-Type", "application/json").send_string(body);
        sent.map_err(Box::new)?.into_string().map_err(|e| Box::new(e.into()))
    }
}

impl Transport for OryxHttp {
    fn call(&mut self, query: &str, variables: Value) -> Result<Value, PushError> {
        let body = json!({ "query": query, "variables": variables }).to_string();
        // Oryx sometimes answers 502/503/429 for a moment. Every call here sets a value, so repeating it is
        // safe; only creating things isn't, and those are not repeated.
        let repeatable = !query.contains("createLayout(") && !query.contains("createLayer(");
        let mut attempt = 0;
        let text = loop {
            match self.post(&body).map_err(|e| *e) {
                Ok(text) => break text,
                Err(ureq::Error::Status(code @ (429 | 502 | 503 | 504), _)) if repeatable && attempt < 4 => {
                    attempt += 1;
                    crate::debug!("push: HTTP {code}, trying again ({attempt})");
                    std::thread::sleep(Duration::from_millis(700 * (1 << attempt)));
                }
                Err(ureq::Error::Status(code, _)) => return Err(PushError::Oryx(format!("HTTP {code}"))),
                Err(other) => return Err(PushError::Network(format!("Could not reach Oryx: {other}"))),
            }
        };
        // Be gentle: a push is a handful of calls, there's no hurry.
        std::thread::sleep(Duration::from_millis(250));
        let mut answer: Value = serde_json::from_str(&text).map_err(|e| PushError::Oryx(format!("bad answer: {e}")))?;
        if let Some(message) = answer["errors"][0]["message"].as_str() {
            return Err(PushError::Oryx(message.to_string()));
        }
        Ok(answer["data"].take())
    }
}

const LAYOUT: &str = "query($hashId: String!, $revisionId: String!, $geometry: String) {
  layout(hashId: $hashId, geometry: $geometry, revisionId: $revisionId) {
    hashId title geometry
    revision { hashId qmkVersion layers { hashId automouse title position color keys } combos { keyIndices layerIdx name trigger } }
  }
}";
const CREATE_LAYOUT: &str = "mutation($title: String!, $revisionHashId: String!, $geometry: String!, $parentHashId: String) {
  createLayout(title: $title, revisionHashId: $revisionHashId, parentHashId: $parentHashId, geometry: $geometry) { hashId revision { hashId } }
}";
const UPDATE_LAYER: &str = "mutation($hashId: String!, $newKeys: Json, $title: String) {
  updateLayer(hashId: $hashId, newKeys: $newKeys, title: $title) { hashId }
}";
const CREATE_LAYER: &str = "mutation($keys: Json!, $revisionHashId: String!, $position: Int!, $title: String, $automouse: Boolean) {
  createLayer(newKeys: $keys, revisionHashId: $revisionHashId, position: $position, title: $title, automouse: $automouse) { hashId }
}";
const DELETE_LAYER: &str = "mutation($hashId: String!) { deleteLayer(hashId: $hashId) { hashId } }";
const LAYER_COLOR: &str = "mutation($hashId: String!, $color: String) { updateLayerColor(hashId: $hashId, color: $color) { hashId } }";
const AUTOMOUSE: &str = "mutation($revisionHash: String!, $layerPosition: Int!, $automouse: Boolean!) {
  updateRevisionAutomouse(revisionHash: $revisionHash, layerPosition: $layerPosition, automouse: $automouse) { status }
}";

/// What a push did.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pushed {
    pub draft: String,
    /// The new layout on Oryx.
    pub layout_id: String,
    pub revision_id: String,
    /// Open this in Oryx to check, compile and flash.
    pub url: String,
    /// The revision Oryx copied to start from (`layout@revision`).
    pub copied_from: String,
    pub layers_written: usize,
    pub layers_added: usize,
    pub layers_removed: usize,
    /// True when the new layout, read back from Oryx, has the same layers and keys as the draft.
    pub verified: bool,
    /// What still differs when `verified` is false (`from` = Oryx, `to` = the draft).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub differences: Vec<Change>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

/// A layout as the write calls need it: the model plus each layer's Oryx id.
struct Remote {
    layout: Layout,
    revision_compiled: bool,
}

impl Remote {
    fn layer_id(layer: &Layer) -> Result<String, PushError> {
        layer.extra.get("hashId").and_then(Value::as_str).map(str::to_string).ok_or_else(|| PushError::Oryx("a layer has no id".into()))
    }
    fn auto_mouse(&self) -> Option<i64> {
        self.layout.layers.iter().find(|l| l.extra.get("automouse") == Some(&Value::Bool(true))).map(|l| l.position)
    }
}

fn fetch(t: &mut dyn Transport, layout_id: &str, revision_id: &str, geometry: &str) -> Result<Remote, PushError> {
    let data = t.call(LAYOUT, json!({ "hashId": layout_id, "revisionId": revision_id, "geometry": geometry }))?;
    let found = &data["layout"];
    if found.is_null() {
        return Err(PushError::Oryx(format!("layout {layout_id} not found")));
    }
    let revision_compiled = !found["revision"]["qmkVersion"].is_null();
    let source = Source { kind: "oryx".into(), fetched_at: crate::source::now() };
    let layout = Layout::from_oryx(found, source).map_err(|e| PushError::Oryx(e.to_string()))?;
    Ok(Remote { layout, revision_compiled })
}

pub fn layout_url(geometry: &str, layout_id: &str) -> String {
    format!("https://configure.zsa.io/{geometry}/layouts/{layout_id}/latest/0")
}

/// Creates a new anonymous layout on Oryx holding `draft`, and reads it back to check.
/// `base` is the revision the draft started from (used to tell whether its combos changed).
pub fn push_new(t: &mut dyn Transport, draft: &Layout, base: Option<&Layout>) -> Result<Pushed, PushError> {
    let name = draft.draft.as_ref().map(|d| d.name.clone()).unwrap_or_else(|| draft.reference());
    if draft.layers.is_empty() {
        return Err(PushError::Unsupported("the draft has no layers".into()));
    }
    let mut warnings = Vec::new();

    // 1. What to copy: the draft's base revision if Oryx has it (that carries trackball settings, RGB and
    //    timing settings and combos over), otherwise ZSA's default layout for this keyboard.
    let source = match fetch(t, &draft.layout_id, &draft.revision_id, &draft.geometry) {
        Ok(r) if r.revision_compiled => r,
        other => {
            let why = match other {
                Ok(_) => "it isn't compiled".to_string(),
                Err(PushError::Network(e)) => return Err(PushError::Network(e)),
                Err(e) => e.to_string(),
            };
            crate::debug!("push: can't copy {}@{}: {why}", draft.layout_id, draft.revision_id);
            warnings.push(format!(
                "Oryx couldn't copy {}@{} ({why}), so the new layout starts from ZSA's default layout: \
                 trackball, RGB and timing settings are the defaults, set them in Oryx.",
                draft.layout_id, draft.revision_id
            ));
            fetch(t, "default", "latest", &draft.geometry)?
        }
    };
    let copied_base = source.layout.layout_id == draft.layout_id;
    let copied_from = source.layout.reference();

    // 2. The copy.
    let created = t.call(
        CREATE_LAYOUT,
        json!({
            "title": format!("{} ({name})", draft.title),
            "revisionHashId": source.layout.revision_id,
            "parentHashId": source.layout.layout_id,
            "geometry": draft.geometry,
        }),
    )?;
    let layout_id =
        created["createLayout"]["hashId"].as_str().ok_or_else(|| PushError::Oryx("no layout id in the answer".into()))?.to_string();
    let url = layout_url(&draft.geometry, &layout_id);
    if layout_id == draft.layout_id || layout_id == source.layout.layout_id {
        // Can only happen with a login, which keyloom doesn't send. Never edit an existing layout.
        return Err(PushError::Oryx(format!("Oryx answered with the existing layout {layout_id} instead of a copy; nothing was written")));
    }
    let partial = |e: PushError| match e {
        PushError::Partial { .. } => e,
        other => PushError::Partial { url: url.clone(), message: other.to_string() },
    };

    // 3. Layers, by position.
    let copy = fetch(t, &layout_id, "latest", &draft.geometry).map_err(partial)?;
    if copy.revision_compiled {
        return Err(partial(PushError::Oryx("the copy isn't editable".into())));
    }
    let revision_id = copy.layout.revision_id.clone();
    let (mut written, mut added) = (0, 0);
    for layer in &draft.layers {
        let keys = serde_json::to_value(&layer.keys).unwrap_or(Value::Null);
        match copy.layout.layer(layer.position) {
            Some(existing) => {
                let id = Remote::layer_id(existing).map_err(partial)?;
                if existing.keys != layer.keys {
                    t.call(UPDATE_LAYER, json!({ "hashId": id, "newKeys": keys })).map_err(partial)?;
                    written += 1;
                }
                if existing.title != layer.title {
                    t.call(UPDATE_LAYER, json!({ "hashId": id, "title": layer.title })).map_err(partial)?;
                }
                if existing.color != layer.color {
                    t.call(LAYER_COLOR, json!({ "hashId": id, "color": layer.color })).map_err(partial)?;
                }
            }
            None => {
                let made = t
                    .call(
                        CREATE_LAYER,
                        json!({
                            "keys": keys, "revisionHashId": revision_id, "position": layer.position, "title": layer.title,
                            "automouse": draft.settings.auto_mouse_layer == Some(layer.position),
                        }),
                    )
                    .map_err(partial)?;
                if let (Some(color), Some(id)) = (&layer.color, made["createLayer"]["hashId"].as_str()) {
                    t.call(LAYER_COLOR, json!({ "hashId": id, "color": color })).map_err(partial)?;
                }
                added += 1;
            }
        }
    }
    // Layers the draft no longer has, last first (Oryx renumbers the ones after a deleted layer).
    let surplus: Vec<&Layer> = copy.layout.layers.iter().filter(|l| draft.layer(l.position).is_none()).collect();
    for layer in surplus.iter().rev() {
        t.call(DELETE_LAYER, json!({ "hashId": Remote::layer_id(layer).map_err(partial)? })).map_err(partial)?;
    }

    // 4. The auto-mouse layer (Oryx keeps it to one layer by itself).
    let mut back = fetch(t, &layout_id, "latest", &draft.geometry).map_err(partial)?;
    match (draft.settings.auto_mouse_layer, back.auto_mouse()) {
        (want, have) if want == have => {}
        (Some(position), _) => {
            t.call(AUTOMOUSE, json!({ "revisionHash": revision_id, "layerPosition": position, "automouse": true })).map_err(partial)?;
            back = fetch(t, &layout_id, "latest", &draft.geometry).map_err(partial)?;
        }
        (None, Some(position)) => {
            t.call(AUTOMOUSE, json!({ "revisionHash": revision_id, "layerPosition": position, "automouse": false })).map_err(partial)?;
            back = fetch(t, &layout_id, "latest", &draft.geometry).map_err(partial)?;
        }
        (None, None) => {}
    }

    // 5. Things this doesn't write.
    let combos_differ = match base {
        Some(b) if copied_base => b.combos != draft.combos,
        _ => !draft.combos.is_empty(),
    };
    if combos_differ {
        warnings.push("The draft's combos differ from what the new layout has: combos aren't written yet, set them in Oryx.".into());
    }

    // 6. Check what Oryx has now.
    let mut differences = diff(&back.layout, draft, DiffOptions { ignore_metadata: true }).changes;
    if back.auto_mouse() != draft.settings.auto_mouse_layer {
        warnings.push(format!(
            "The auto-mouse layer is {} on Oryx, {} in the draft: set it in Oryx.",
            back.auto_mouse().map_or("off".into(), |p| p.to_string()),
            draft.settings.auto_mouse_layer.map_or("off".into(), |p| p.to_string())
        ));
    }
    differences.truncate(50);
    Ok(Pushed {
        draft: draft.reference(),
        layout_id,
        revision_id,
        url,
        copied_from,
        layers_written: written,
        layers_added: added,
        layers_removed: surplus.len(),
        verified: differences.is_empty(),
        differences,
        warnings,
    })
}

/// An in-memory Oryx that understands the calls above. For tests (also used by the CLI's tests).
#[doc(hidden)]
pub mod fake {
    use super::*;
    use std::collections::BTreeMap;

    #[derive(Default)]
    pub struct FakeOryx {
        /// Layout id → the `layout` object (with `revision.layers[].hashId`).
        pub layouts: BTreeMap<String, Value>,
        pub calls: Vec<String>,
        /// Fail the call with this text in its query (once).
        pub fail_on: Option<&'static str>,
        next: u32,
    }

    impl FakeOryx {
        fn id(&mut self, prefix: &str) -> String {
            self.next += 1;
            format!("{prefix}{}", self.next)
        }

        /// Adds a compiled layout, as Oryx would return it.
        pub fn add_compiled(&mut self, layout: &Layout) {
            let mut v = layout.to_oryx();
            v["revision"]["qmkVersion"] = "25.0".into();
            let auto = layout.settings.auto_mouse_layer;
            let ids: Vec<String> = (0..layout.layers.len()).map(|_| self.id("L")).collect();
            for (l, id) in v["revision"]["layers"].as_array_mut().unwrap().iter_mut().zip(ids) {
                l["hashId"] = id.into();
                l["automouse"] = (l["position"].as_i64() == auto).into();
            }
            self.layouts.insert(layout.layout_id.clone(), v);
        }

        fn layer_mut(&mut self, id: &str) -> Result<(&mut Value, bool), PushError> {
            for layout in self.layouts.values_mut() {
                let compiled = !layout["revision"]["qmkVersion"].is_null();
                if let Some(l) = layout["revision"]["layers"].as_array_mut().unwrap().iter_mut().find(|l| l["hashId"] == id) {
                    return Ok((l, compiled));
                }
            }
            Err(PushError::Oryx("NotFound".into()))
        }

        fn editable_layer(&mut self, id: &str) -> Result<&mut Value, PushError> {
            match self.layer_mut(id)? {
                (_, true) => Err(PushError::Oryx("Unauthorized".into())),
                (l, false) => Ok(l),
            }
        }

        fn revision_mut(&mut self, id: &str) -> Result<&mut Value, PushError> {
            let rev = self
                .layouts
                .values_mut()
                .map(|l| &mut l["revision"])
                .find(|r| r["hashId"] == id)
                .ok_or_else(|| PushError::Oryx("NotFound".into()))?;
            if !rev["qmkVersion"].is_null() {
                return Err(PushError::Oryx("Unauthorized".into()));
            }
            Ok(rev)
        }
    }

    impl Transport for FakeOryx {
        fn call(&mut self, query: &str, v: Value) -> Result<Value, PushError> {
            let op = ["createLayout", "updateLayerColor", "updateLayer", "createLayer", "deleteLayer", "updateRevisionAutomouse", "layout"]
                .into_iter()
                .find(|op| query.contains(&format!("{op}(")))
                .unwrap_or("?");
            self.calls.push(op.to_string());
            if self.fail_on.is_some_and(|f| f == op) {
                self.fail_on = None;
                return Err(PushError::Oryx("boom".into()));
            }
            let s = |k: &str| v[k].as_str().unwrap_or_default().to_string();
            match op {
                "layout" => Ok(json!({ "layout": self.layouts.get(&s("hashId")).cloned().unwrap_or(Value::Null) })),
                "createLayout" => {
                    let parent = self.layouts.get(&s("parentHashId")).cloned().ok_or_else(|| PushError::Oryx("NotFound".into()))?;
                    let (layout_id, revision_id) = (self.id("new"), self.id("rev"));
                    let mut copy = parent;
                    copy["hashId"] = layout_id.clone().into();
                    copy["title"] = v["title"].clone();
                    copy["revision"]["hashId"] = revision_id.clone().into();
                    copy["revision"]["qmkVersion"] = Value::Null;
                    let n = copy["revision"]["layers"].as_array().unwrap().len();
                    let ids: Vec<String> = (0..n).map(|_| self.id("L")).collect();
                    for (l, id) in copy["revision"]["layers"].as_array_mut().unwrap().iter_mut().zip(ids) {
                        l["hashId"] = id.into();
                    }
                    self.layouts.insert(layout_id.clone(), copy);
                    Ok(json!({ "createLayout": { "hashId": layout_id, "revision": { "hashId": revision_id } } }))
                }
                "updateLayer" => {
                    let l = self.editable_layer(&s("hashId"))?;
                    if !v["newKeys"].is_null() {
                        l["keys"] = v["newKeys"].clone();
                    }
                    if !v["title"].is_null() {
                        l["title"] = v["title"].clone();
                    }
                    Ok(json!({ "updateLayer": { "hashId": s("hashId") } }))
                }
                "updateLayerColor" => {
                    self.editable_layer(&s("hashId"))?["color"] = v["color"].clone();
                    Ok(json!({ "updateLayerColor": { "hashId": s("hashId") } }))
                }
                "createLayer" => {
                    let id = self.id("L");
                    let rev = self.revision_mut(&s("revisionHashId"))?;
                    if v["automouse"] == true {
                        rev["layers"].as_array_mut().unwrap().iter_mut().for_each(|l| l["automouse"] = false.into());
                    }
                    rev["layers"].as_array_mut().unwrap().push(json!({
                        "hashId": id, "keys": v["keys"], "position": v["position"], "title": v["title"],
                        "automouse": v["automouse"] == true, "color": null,
                    }));
                    Ok(json!({ "createLayer": { "hashId": id } }))
                }
                "deleteLayer" => {
                    let id = s("hashId");
                    self.editable_layer(&id)?;
                    for layout in self.layouts.values_mut() {
                        let layers = layout["revision"]["layers"].as_array_mut().unwrap();
                        if layers.iter().any(|l| l["hashId"] == id.as_str()) {
                            layers.retain(|l| l["hashId"] != id.as_str());
                            layers.iter_mut().enumerate().for_each(|(i, l)| l["position"] = i.into());
                        }
                    }
                    Ok(json!({ "deleteLayer": { "hashId": id } }))
                }
                "updateRevisionAutomouse" => {
                    let rev = self.revision_mut(&s("revisionHash"))?;
                    for l in rev["layers"].as_array_mut().unwrap() {
                        l["automouse"] = (v["automouse"] == true && l["position"] == v["layerPosition"]).into();
                    }
                    Ok(json!({ "updateRevisionAutomouse": { "status": true } }))
                }
                _ => Err(PushError::Oryx(format!("the fake doesn't know this call: {query}"))),
            }
        }
    }
}
