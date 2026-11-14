//! Reference knowledge for agents and humans (spec 07): keycodes with aliases, and guides.
//! The data lives in `data/` and is embedded at build time (see build.rs).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::OnceLock;

include!(concat!(env!("OUT_DIR"), "/guides.rs"));

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Keycode {
    pub code: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legend: Option<String>,
    /// Takes a layer argument (MO, TG, …).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub layer: bool,
    /// Used as a hold action that wraps a tap (MEH_T, ALL_T, …).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub mod_tap: bool,
    /// A whole-key form keyloom writes (LT, MT, TAP_HOLD), not a code Oryx stores.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub family: bool,
    /// Seen under this name in a real Oryx layout. Others follow QMK's and Oryx's naming
    /// pattern but are unconfirmed.
    #[serde(default)]
    pub verified: bool,
}

struct Table {
    codes: Vec<Keycode>,
    /// Lower-cased code or alias → index.
    by_name: HashMap<String, usize>,
}

/// Aliases that must work even without the data file (and win over it).
const BUILTIN: &[(&str, &str)] = &[
    ("KC_TRNS", "KC_TRANSPARENT"),
    ("TRNS", "KC_TRANSPARENT"),
    ("_______", "KC_TRANSPARENT"),
    ("▽", "KC_TRANSPARENT"),
    ("NO", "KC_NO"),
    ("XXXXXXX", "KC_NO"),
    ("KC_ESC", "KC_ESCAPE"),
    ("KC_SPC", "KC_SPACE"),
    ("KC_ENT", "KC_ENTER"),
    ("KC_LCTL", "KC_LEFT_CTRL"),
    ("KC_LSFT", "KC_LEFT_SHIFT"),
    ("KC_LALT", "KC_LEFT_ALT"),
    ("KC_LGUI", "KC_LEFT_GUI"),
    ("KC_RCTL", "KC_RIGHT_CTRL"),
    ("KC_RSFT", "KC_RIGHT_SHIFT"),
    ("KC_RALT", "KC_RIGHT_ALT"),
    ("KC_RGUI", "KC_RIGHT_GUI"),
];

fn table() -> &'static Table {
    static T: OnceLock<Table> = OnceLock::new();
    T.get_or_init(|| {
        let codes: Vec<Keycode> = serde_json::from_str(include_str!(concat!(env!("OUT_DIR"), "/keycodes.json"))).unwrap_or_default();
        let mut by_name = HashMap::new();
        for (i, k) in codes.iter().enumerate() {
            by_name.entry(k.code.to_lowercase()).or_insert(i);
        }
        for (i, k) in codes.iter().enumerate() {
            for a in &k.aliases {
                by_name.entry(a.to_lowercase()).or_insert(i);
            }
        }
        Table { codes, by_name }
    })
}

pub fn keycodes() -> &'static [Keycode] {
    &table().codes
}

/// Finds a keycode by its code or any alias, case-insensitive; `X` also finds `KC_X`.
pub fn lookup(name: &str) -> Option<&'static Keycode> {
    let t = table();
    let n = name.trim();
    let target = BUILTIN.iter().find(|(a, _)| a.eq_ignore_ascii_case(n)).map(|(_, c)| *c).unwrap_or(n);
    let lower = target.to_lowercase();
    t.by_name.get(&lower).or_else(|| t.by_name.get(&format!("kc_{lower}"))).map(|&i| &t.codes[i])
}

/// The canonical (Oryx) name for a code or alias, or `None` if unknown.
pub fn canonical(name: &str) -> Option<String> {
    if let Some(k) = lookup(name) {
        return Some(k.code.clone());
    }
    let n = name.trim();
    BUILTIN.iter().find(|(a, _)| a.eq_ignore_ascii_case(n)).map(|(_, c)| c.to_string())
}

/// Same code after normalisation (`KC_ESC` == `KC_ESCAPE`). Unknown codes compare literally.
pub fn same_code(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b) || matches!((canonical(a), canonical(b)), (Some(x), Some(y)) if x == y)
}

pub fn is_known(code: &str) -> bool {
    lookup(code).is_some() || BUILTIN.iter().any(|(_, c)| *c == code)
}

/// Search for `lookup_keycode` / `keyloom ref`. Every word of the query has to match the code,
/// an alias or the description. Best matches first: the exact code or alias, then names that
/// start with or contain the query, then matches in the description only.
pub fn search(query: &str, limit: usize) -> Vec<&'static Keycode> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return vec![];
    }
    let words: Vec<&str> = q.split_whitespace().collect();
    let exact = lookup(query).map(|k| k.code.as_str());
    let mut scored: Vec<(u8, usize, &Keycode)> = Vec::new();
    for (i, k) in keycodes().iter().enumerate() {
        let names: Vec<String> = std::iter::once(&k.code).chain(k.aliases.iter()).map(|n| n.to_lowercase().replace('_', " ")).collect();
        let desc = k.description.to_lowercase();
        let spaced = q.replace('_', " ");
        let in_names = |w: &str| names.iter().any(|n| n.contains(&w.replace('_', " ")));
        if !words.iter().all(|w| in_names(w) || desc.contains(w)) {
            continue;
        }
        let score = if exact == Some(k.code.as_str()) {
            0
        } else if names.iter().any(|n| *n == spaced || n.strip_prefix("kc ") == Some(spaced.as_str())) {
            1
        } else if names.iter().any(|n| n.starts_with(&spaced) || n.strip_prefix("kc ").is_some_and(|r| r.starts_with(&spaced))) {
            2
        } else if words.iter().all(|w| in_names(w)) {
            3
        } else {
            4
        };
        scored.push((score, i, k));
    }
    scored.sort_by_key(|&(s, i, _)| (s, i));
    scored.into_iter().take(limit).map(|(_, _, k)| k).collect()
}

pub fn guides() -> &'static [(&'static str, &'static str)] {
    GUIDES
}

pub fn guide(name: &str) -> Option<&'static str> {
    GUIDES.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, t)| *t)
}

/// Layer actions explained (resource `keyloom://reference/layer-actions`).
pub const LAYER_ACTIONS: &str = "\
# Layer actions on ZSA boards (Oryx)

| Action | Written | Behaviour | Typical use |
| --- | --- | --- | --- |
| Momentary | `MO(n)` | layer n while held | symbols, nav, numbers on a thumb |
| Layer-tap | `LT(n, KC_x)` | tap: KC_x, hold: layer n. Oryx stores tap KC_x + hold MO(n) | Space/Sym thumb |
| Toggle | `TG(n)` | tap turns layer n on, tap again turns it off | gaming, numpad |
| Go to | `TO(n)` | turns on layer n and turns off all others (except the default) | leaving a toggled layer: TO(0) |
| Tap-toggle | `TT(n)` | hold: momentary; tap: toggle | nav that you sometimes want to stay in |
| One-shot | `OSL(n)` | next key press uses layer n | occasional symbols |
| Default | `DF(n)` | changes the default (base) layer | switching between QWERTY and Colemak |

Transparent keys (`KC_TRANSPARENT`, blank keys in Oryx) fall through to the next active lower layer.
Every layer you can get stuck on needs a way out on it (or falling through to it), and the way out has to
match the way in: a layer entered with TG, TO or TT is left with TG/TT(that layer) or TO(another layer);
a layer made the default with DF is only left with DF(another layer).
";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_aliases_work_without_data() {
        assert_eq!(canonical("KC_TRNS").as_deref(), Some("KC_TRANSPARENT"));
        assert!(same_code("KC_ESC", "KC_ESCAPE"));
        assert!(same_code("KC_LCTL", "KC_LEFT_CTRL"));
        assert!(!same_code("KC_A", "KC_B"));
    }
}
