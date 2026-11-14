//! Adapted from voyager-ghost (<https://gitlab.com/tbulva/voyager-ghost>), MIT License,
//! Copyright (c) 2026 Tomas Bulva.
//!
//! Shape of a layout as returned by the Oryx GraphQL API. Decoding is deliberately lenient:
//! unknown or malformed fields are dropped instead of failing the whole layout. keyloom uses it to
//! compute key legends; the lossless model is in `model`.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};

fn lenient<'de, D, T>(d: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    let value = serde_json::Value::deserialize(d)?;
    Ok(serde_json::from_value(value).ok())
}

macro_rules! lenient_struct {
    ($(#[$meta:meta])* pub struct $name:ident { $(pub $field:ident : Option<$ty:ty>,)* }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub struct $name {
            $(
                #[serde(default, deserialize_with = "lenient", skip_serializing_if = "Option::is_none")]
                pub $field: Option<$ty>,
            )*
        }
    };
}

lenient_struct! {
    pub struct OryxModifiers {
        pub left_ctrl: Option<bool>,
        pub left_shift: Option<bool>,
        pub left_alt: Option<bool>,
        pub left_gui: Option<bool>,
        pub right_ctrl: Option<bool>,
        pub right_shift: Option<bool>,
        pub right_alt: Option<bool>,
        pub right_gui: Option<bool>,
    }
}

impl OryxModifiers {
    pub fn ctrl(&self) -> bool {
        self.left_ctrl == Some(true) || self.right_ctrl == Some(true)
    }
    pub fn shift(&self) -> bool {
        self.left_shift == Some(true) || self.right_shift == Some(true)
    }
    pub fn alt(&self) -> bool {
        self.left_alt == Some(true) || self.right_alt == Some(true)
    }
    pub fn gui(&self) -> bool {
        self.left_gui == Some(true) || self.right_gui == Some(true)
    }
}

lenient_struct! {
    pub struct OryxAction {
        pub code: Option<String>,
        pub layer: Option<i64>,
        pub color: Option<String>,
        pub modifiers: Option<OryxModifiers>,
        pub description: Option<String>,
    }
}

lenient_struct! {
    pub struct OryxKey {
        pub tap: Option<OryxAction>,
        pub hold: Option<OryxAction>,
        pub double_tap: Option<OryxAction>,
        pub tap_hold: Option<OryxAction>,
        pub custom_label: Option<String>,
        pub icon: Option<String>,
        pub emoji: Option<String>,
        pub glow_color: Option<String>,
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OryxLayer {
    pub title: String,
    pub position: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    pub keys: Vec<OryxKey>,
}

impl<'de> Deserialize<'de> for OryxLayer {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Raw {
            #[serde(default, deserialize_with = "lenient")]
            title: Option<String>,
            #[serde(default, deserialize_with = "lenient")]
            position: Option<i64>,
            #[serde(default, deserialize_with = "lenient")]
            color: Option<String>,
            #[serde(default, deserialize_with = "lenient")]
            keys: Option<Vec<serde_json::Value>>,
        }
        let raw = Raw::deserialize(d)?;
        let position = raw.position.unwrap_or(0);
        let keys = raw.keys.unwrap_or_default().into_iter().map(|v| serde_json::from_value(v).unwrap_or_default()).collect();
        Ok(Self { title: raw.title.unwrap_or_else(|| format!("Layer {position}")), position, color: raw.color, keys })
    }
}

/// A combo: pressing all `key_indices` together (on layer `layer_idx`) fires `trigger`.
/// Carried through the cache; not drawn yet.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OryxCombo {
    #[serde(default)]
    pub key_indices: Vec<i64>,
    #[serde(default, deserialize_with = "lenient", skip_serializing_if = "Option::is_none")]
    pub layer_idx: Option<i64>,
    #[serde(default, deserialize_with = "lenient", skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, deserialize_with = "lenient", skip_serializing_if = "Option::is_none")]
    pub trigger: Option<OryxAction>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OryxRevision {
    pub hash_id: String,
    #[serde(default, deserialize_with = "lenient", skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub layers: Vec<OryxLayer>,
    #[serde(default, deserialize_with = "lenient", skip_serializing_if = "Option::is_none")]
    pub combos: Option<Vec<serde_json::Value>>,
}

impl OryxRevision {
    pub fn sorted_layers(&self) -> Vec<OryxLayer> {
        let mut layers = self.layers.clone();
        layers.sort_by_key(|l| l.position);
        layers
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OryxLayout {
    pub hash_id: String,
    pub title: String,
    #[serde(default, deserialize_with = "lenient", skip_serializing_if = "Option::is_none")]
    pub geometry: Option<String>,
    pub revision: OryxRevision,
}

/// Which layout + revision to show. `revision_id` is a revision hash or "latest".
/// `geometry` is the Oryx geometry name ("voyager", "moonlander") when known.
#[derive(Debug, Clone)]
pub struct LayoutRef {
    pub hash_id: String,
    pub revision_id: String,
    pub geometry: Option<String>,
}

impl PartialEq for LayoutRef {
    fn eq(&self, other: &Self) -> bool {
        self.hash_id == other.hash_id && self.revision_id == other.revision_id
    }
}

impl std::fmt::Display for LayoutRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.hash_id, self.revision_id)
    }
}

impl LayoutRef {
    pub fn new(hash_id: &str, revision_id: &str, geometry: Option<String>) -> Self {
        Self { hash_id: hash_id.into(), revision_id: revision_id.into(), geometry }
    }

    pub fn is_latest(&self) -> bool {
        self.revision_id == "latest"
    }

    /// Persisted form, e.g. "voyager:aBcDe/xYz123".
    pub fn storage_string(&self) -> String {
        match &self.geometry {
            Some(g) => format!("{g}:{self}"),
            None => self.to_string(),
        }
    }

    /// Oryx-built firmware reports `<layoutId>/<revisionId>` as its USB serial number.
    pub fn from_serial(serial: &str, geometry: Option<String>) -> Option<Self> {
        let parts: Vec<&str> = serial.split('/').collect();
        if parts.len() != 2 || !parts.iter().all(|p| is_valid_id(p)) {
            return None;
        }
        Some(Self::new(parts[0], parts[1], geometry))
    }

    /// Accepts a configure.zsa.io link, "layoutId/revisionId", a bare layout id,
    /// or the persisted "geometry:layoutId/revisionId" form.
    pub fn from_user_input(input: &str) -> Option<Self> {
        let text = input.trim();
        let mut geometry = None;
        let parts: Vec<String>;
        if let Some(rest) = text.strip_prefix("https://").or_else(|| text.strip_prefix("http://")) {
            let rest = rest.split(['?', '#']).next().unwrap_or("");
            let mut comps = rest.split('/').filter(|c| !c.is_empty());
            let host = comps.next()?;
            if !host.ends_with("zsa.io") {
                return None;
            }
            let comps: Vec<&str> = comps.collect(); // ["voyager", "layouts", "aBcDe", "latest", "0"]
            let i = comps.iter().position(|c| *c == "layouts")?;
            if i >= 1 {
                geometry = Some(comps[i - 1].to_string());
            }
            parts = comps[i + 1..].iter().take(2).map(|s| s.to_string()).collect();
        } else {
            let mut body = text;
            if let Some((g, rest)) = text.split_once(':') {
                geometry = Some(g.to_string());
                body = rest;
            }
            parts = body.split('/').filter(|p| !p.is_empty()).map(|s| s.to_string()).collect();
        }
        if !(1..=2).contains(&parts.len()) || !parts.iter().all(|p| is_valid_id(p)) {
            return None;
        }
        let revision = parts.get(1).map(String::as_str).unwrap_or("latest");
        Some(Self::new(&parts[0], revision, geometry))
    }
}

fn is_valid_id(s: &str) -> bool {
    !s.is_empty() && s.len() <= 32 && s.chars().all(|c| c.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_refs() {
        let r = LayoutRef::from_user_input("https://configure.zsa.io/voyager/layouts/aBcDe/latest/0").unwrap();
        assert_eq!((r.hash_id.as_str(), r.revision_id.as_str(), r.geometry.as_deref()), ("aBcDe", "latest", Some("voyager")));
        let r = LayoutRef::from_user_input("voyager:aBcDe/xYz123").unwrap();
        assert_eq!(r.storage_string(), "voyager:aBcDe/xYz123");
        assert_eq!(LayoutRef::from_user_input("aBcDe").unwrap().revision_id, "latest");
        assert!(LayoutRef::from_user_input("../etc/passwd").is_none());
        assert!(LayoutRef::from_serial("aBcDe/xYz123", None).is_some());
        assert!(LayoutRef::from_serial("0", None).is_none());
    }

    #[test]
    fn lenient_decoding() {
        let json = r#"{"title":"T","position":"oops","keys":[null,{"tap":{"code":"KC_A","layer":"x"}},{"tap":5}]}"#;
        let layer: OryxLayer = serde_json::from_str(json).unwrap();
        assert_eq!(layer.position, 0);
        assert_eq!(layer.keys.len(), 3);
        assert_eq!(layer.keys[1].tap.as_ref().unwrap().code.as_deref(), Some("KC_A"));
        assert!(layer.keys[2].tap.is_none());
    }
}
