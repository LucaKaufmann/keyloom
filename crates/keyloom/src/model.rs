//! keyloom's layout model: Oryx's own shape inside a small envelope (spec 01).
//!
//! Unlike the lenient view in `oryx`, nothing Oryx sends is dropped: every struct keeps unknown fields in
//! `extra`, so a layout survives import → draft → export. Null values are dropped on the way in
//! (they carry no information), which keeps files and diffs small.

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value};

pub const SCHEMA: &str = "keyloom.layout/1";

/// Unknown fields, kept verbatim.
pub type Extra = Map<String, Value>;

fn is_false(b: &bool) -> bool {
    !*b
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Modifiers {
    #[serde(default, skip_serializing_if = "is_false")]
    pub left_ctrl: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub left_shift: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub left_alt: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub left_gui: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub right_ctrl: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub right_shift: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub right_alt: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub right_gui: bool,
    #[serde(flatten)]
    pub extra: Extra,
}

impl Modifiers {
    pub fn is_empty(&self) -> bool {
        self.qmk_names().is_empty() && self.extra.is_empty()
    }

    /// QMK modifier names in a stable order, e.g. `["LCTL", "LSFT"]`.
    pub fn qmk_names(&self) -> Vec<&'static str> {
        let all = [
            (self.left_ctrl, "LCTL"),
            (self.left_shift, "LSFT"),
            (self.left_alt, "LALT"),
            (self.left_gui, "LGUI"),
            (self.right_ctrl, "RCTL"),
            (self.right_shift, "RSFT"),
            (self.right_alt, "RALT"),
            (self.right_gui, "RGUI"),
        ];
        all.iter().filter(|(on, _)| *on).map(|(_, n)| *n).collect()
    }
}

/// One action in a key slot (tap, hold, double tap, tap-then-hold).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Action {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "modifiers_or_none")]
    pub modifiers: Option<Modifiers>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(flatten)]
    pub extra: Extra,
}

/// Oryx sends all eight modifier flags, usually all false: treat that as "no modifiers".
fn modifiers_or_none<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Modifiers>, D::Error> {
    let m: Option<Modifiers> = Option::deserialize(d)?;
    Ok(m.filter(|m| !m.is_empty()))
}

impl Action {
    pub fn code(&self) -> Option<&str> {
        self.code.as_deref().filter(|c| !c.is_empty())
    }
}

/// One physical key on one layer.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Key {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tap: Option<Action>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hold: Option<Action>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub double_tap: Option<Action>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tap_hold: Option<Action>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub glow_color: Option<String>,
    #[serde(flatten)]
    pub extra: Extra,
}

impl Key {
    /// No action in any slot. Oryx compiles such a key to `KC_TRANSPARENT`
    /// (docs/oryx-encodings.md).
    pub fn is_blank(&self) -> bool {
        self.slots().iter().all(|(_, a)| a.code().is_none())
    }

    /// Falls through to the layer below: blank, or an explicit `KC_TRANSPARENT`/`KC_TRNS` tap
    /// with nothing else.
    pub fn is_transparent(&self) -> bool {
        if self.is_blank() {
            return true;
        }
        let tap = self.tap.as_ref().and_then(Action::code);
        // (A modifier on it would be something else, and must stay visible.)
        matches!(tap, Some("KC_TRANSPARENT" | "KC_TRNS"))
            && self.tap.as_ref().is_some_and(|a| a.modifiers.is_none())
            && self.hold.as_ref().and_then(Action::code).is_none()
            && self.double_tap.as_ref().and_then(Action::code).is_none()
            && self.tap_hold.as_ref().and_then(Action::code).is_none()
    }

    /// Does nothing: `KC_NO` only.
    pub fn is_no(&self) -> bool {
        let tap = self.tap.as_ref().and_then(Action::code);
        matches!(tap, Some("KC_NO")) && self.slots().iter().filter(|(_, a)| a.code().is_some()).count() == 1
    }

    /// Slots that have an action, in display order.
    pub fn slots(&self) -> Vec<(Slot, &Action)> {
        let mut out = Vec::new();
        for (slot, action) in
            [(Slot::Tap, &self.tap), (Slot::Hold, &self.hold), (Slot::DoubleTap, &self.double_tap), (Slot::TapHold, &self.tap_hold)]
        {
            if let Some(a) = action {
                out.push((slot, a));
            }
        }
        out
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Slot {
    Tap,
    Hold,
    DoubleTap,
    TapHold,
}

/// keyloom-only layer metadata; ignored by Oryx.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LayerMeta {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

impl LayerMeta {
    fn is_empty(&self) -> bool {
        self.tags.is_empty() && self.notes.is_none()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Layer {
    pub title: String,
    pub position: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "LayerMeta::is_empty")]
    pub keyloom: LayerMeta,
    #[serde(deserialize_with = "lenient_keys")]
    pub keys: Vec<Key>,
    #[serde(flatten)]
    pub extra: Extra,
}

/// A key that doesn't decode (unexpected shape) is kept whole in `extra` instead of failing the
/// layout.
fn lenient_keys<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<Key>, D::Error> {
    let raw: Vec<Value> = Vec::deserialize(d)?;
    Ok(raw
        .into_iter()
        .map(|v| match serde_json::from_value::<Key>(v.clone()) {
            Ok(k) => k,
            Err(_) => Key { extra: v.as_object().cloned().unwrap_or_default(), ..Key::default() },
        })
        .collect())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    Revision,
    Draft,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    /// "oryx" | "keymapp" | "file"
    #[serde(rename = "type")]
    pub kind: String,
    pub fetched_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DraftMeta {
    pub name: String,
    pub created: String,
    pub updated: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

/// Oryx fields at layout and revision level that keyloom doesn't model.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OryxExtra {
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub layout: Extra,
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub revision: Extra,
}

impl OryxExtra {
    fn is_empty(&self) -> bool {
        self.layout.is_empty() && self.revision.is_empty()
    }
}

/// A stored layout: an imported revision or a draft.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Layout {
    pub schema: String,
    pub layout_id: String,
    /// Revisions: their Oryx id. Drafts: the revision they are based on.
    pub revision_id: String,
    pub kind: Kind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub draft: Option<DraftMeta>,
    pub source: Source,
    pub title: String,
    pub geometry: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision_title: Option<String>,
    /// When Oryx created the revision, e.g. "2026-11-12 09:56:22 UTC".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    pub layers: Vec<Layer>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub combos: Vec<Value>,
    #[serde(default, skip_serializing_if = "OryxExtra::is_empty")]
    pub oryx: OryxExtra,
    /// Firmware settings keyloom reads from Oryx's generated source (docs/oryx-encodings.md).
    #[serde(default, skip_serializing_if = "Settings::is_empty")]
    pub settings: Settings,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    /// Navigator auto-mouse layer (`AUTOMOUSE_LAYER` in config.h): entered by moving the trackball.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_mouse_layer: Option<i64>,
}

impl Settings {
    fn is_empty(&self) -> bool {
        self.auto_mouse_layer.is_none()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ModelError {
    #[error("not an Oryx layout: {0}")]
    NotALayout(String),
    #[error("layout data didn't decode: {0}")]
    Decode(#[from] serde_json::Error),
}

/// Removes null values recursively (objects only; arrays keep their length).
fn strip_nulls(v: &mut Value) {
    match v {
        Value::Object(map) => {
            map.retain(|_, v| !v.is_null());
            map.values_mut().for_each(strip_nulls);
        }
        Value::Array(items) => items.iter_mut().for_each(strip_nulls),
        _ => {}
    }
}

/// Marks the Oryx-shaped export of a draft (in its `revision` object).
pub const DRAFT_MARKER: &str = "keyloomDraft";

/// Personal or volatile fields we never store.
const DROPPED_LAYOUT_FIELDS: &[&str] = &["user"];

impl Layout {
    /// Builds a revision from an Oryx `layout` object (GraphQL answer or Keymapp's `data.layout`).
    pub fn from_oryx(layout: &Value, source: Source) -> Result<Self, ModelError> {
        let mut layout = layout.clone();
        strip_nulls(&mut layout);
        let Value::Object(mut obj) = layout else {
            return Err(ModelError::NotALayout("expected an object".into()));
        };
        let take_str = |obj: &mut Extra, key: &str| obj.remove(key).and_then(|v| v.as_str().map(str::to_string));
        let layout_id = take_str(&mut obj, "hashId").ok_or_else(|| ModelError::NotALayout("no hashId".into()))?;
        let title = take_str(&mut obj, "title").unwrap_or_else(|| layout_id.clone());
        let geometry = take_str(&mut obj, "geometry").unwrap_or_else(|| "voyager".into());
        let Some(Value::Object(mut rev)) = obj.remove("revision") else {
            return Err(ModelError::NotALayout("no revision".into()));
        };
        for f in DROPPED_LAYOUT_FIELDS {
            obj.remove(*f);
        }
        // A draft exported in Oryx's shape (to look at, or for another tool) is not a revision.
        if let Some(name) = rev.get(DRAFT_MARKER).and_then(|v| v.as_str()) {
            return Err(ModelError::NotALayout(format!(
                "this is an export of the keyloom draft '{name}', not a revision from Oryx, so it can't be imported"
            )));
        }
        let revision_id = take_str(&mut rev, "hashId").ok_or_else(|| ModelError::NotALayout("revision has no hashId".into()))?;
        let revision_title = take_str(&mut rev, "title");
        let created_at = take_str(&mut rev, "createdAt");
        let mut layers: Vec<Layer> = serde_json::from_value(rev.remove("layers").unwrap_or(Value::Array(vec![])))?;
        layers.sort_by_key(|l| l.position);
        let combos = match rev.remove("combos") {
            Some(Value::Array(c)) => c,
            _ => vec![],
        };
        Ok(Layout {
            schema: SCHEMA.into(),
            layout_id,
            revision_id,
            kind: Kind::Revision,
            draft: None,
            source,
            title,
            geometry,
            revision_title,
            created_at,
            layers,
            combos,
            oryx: OryxExtra { layout: obj, revision: rev },
            settings: Settings::default(),
        })
    }

    /// The bare Oryx `layout` object (as Oryx's API returns it). keyloom-only metadata is left out.
    pub fn to_oryx(&self) -> Value {
        let mut rev = self.oryx.revision.clone();
        // A draft isn't its base revision: exported, it must not come back in under that id.
        let revision_id = match &self.draft {
            Some(d) => format!("{}-draft-{}", self.revision_id, d.name),
            None => self.revision_id.clone(),
        };
        rev.insert("hashId".into(), revision_id.into());
        if let Some(d) = &self.draft {
            rev.insert(DRAFT_MARKER.into(), d.name.clone().into());
        }
        if let Some(t) = &self.revision_title {
            rev.insert("title".into(), t.clone().into());
        }
        if let Some(c) = &self.created_at {
            rev.insert("createdAt".into(), c.clone().into());
        }
        let layers: Vec<Value> = self
            .layers
            .iter()
            .map(|l| {
                let mut l = l.clone();
                l.keyloom = LayerMeta::default();
                serde_json::to_value(l).unwrap_or(Value::Null)
            })
            .collect();
        rev.insert("layers".into(), layers.into());
        rev.insert("combos".into(), self.combos.clone().into());
        let mut obj = self.oryx.layout.clone();
        obj.insert("hashId".into(), self.layout_id.clone().into());
        obj.insert("title".into(), self.title.clone().into());
        obj.insert("geometry".into(), self.geometry.clone().into());
        obj.insert("revision".into(), Value::Object(rev));
        Value::Object(obj)
    }

    /// Layers in the lenient Oryx view (`oryx`), for legends.
    pub fn legend_layers(&self) -> Vec<crate::oryx::OryxLayer> {
        let oryx = self.to_oryx();
        serde_json::from_value(oryx["revision"]["layers"].clone()).unwrap_or_default()
    }

    pub fn layer(&self, position: i64) -> Option<&Layer> {
        self.layers.iter().find(|l| l.position == position)
    }

    pub fn layer_mut(&mut self, position: i64) -> Option<&mut Layer> {
        self.layers.iter_mut().find(|l| l.position == position)
    }

    pub fn layer_title(&self, position: i64) -> String {
        self.layer(position).map(|l| l.title.clone()).unwrap_or_else(|| format!("L{position}"))
    }

    /// The key that actually acts at `index` on `layer`, following transparency down to the base
    /// layer (an approximation: other active layers are ignored).
    pub fn effective_key(&self, layer: i64, index: usize) -> Option<(&Key, i64)> {
        let key = self.layer(layer)?.keys.get(index)?;
        if layer != 0 && key.is_transparent() {
            let base = self.layers.first()?;
            return base.keys.get(index).map(|k| (k, base.position));
        }
        Some((key, layer))
    }

    pub fn key_count(&self) -> usize {
        self.layers.first().map(|l| l.keys.len()).unwrap_or(0)
    }

    /// `9qMJK@ZPjbXz` or `draft:wow`.
    pub fn reference(&self) -> String {
        match &self.draft {
            Some(d) => format!("draft:{}", d.name),
            None => format!("{}@{}", self.layout_id, self.revision_id),
        }
    }
}
