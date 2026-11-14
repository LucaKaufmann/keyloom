//! Edit operations on drafts (spec 02) and the action-spec parser shared by CLI and MCP.
//!
//! Oryx stores keys as separate slots and has no `LT`/`MT` codes, so the parser turns QMK-style
//! expressions into Oryx-shaped keys: `LT(1, KC_SPC)` → tap `KC_SPACE` + hold `MO(1)`
//! (docs/oryx-encodings.md).

use crate::knowledge;
use crate::model::{Action, Key, Kind, Layer, LayerMeta, Layout, Modifiers, Slot};
use crate::positions::{parse_key, PositionMap};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub struct OpError {
    /// Index of the failing op in a batch.
    pub op: Option<usize>,
    pub message: String,
}

impl std::fmt::Display for OpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.op {
            Some(i) => write!(f, "op {}: {}", i + 1, self.message),
            None => write!(f, "{}", self.message),
        }
    }
}

impl std::error::Error for OpError {}

fn err<T>(message: impl Into<String>) -> Result<T, OpError> {
    Err(OpError { op: None, message: message.into() })
}

// MARK: selectors

/// A layer given as its number or its title (case-insensitive, unique prefix), or `base`.
pub fn resolve_layer(layout: &Layout, sel: &Value) -> Result<i64, OpError> {
    let s = match sel {
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.trim().to_string(),
        _ => return err(format!("layer must be a number or a title, got {sel}")),
    };
    resolve_layer_str(layout, &s)
}

pub fn resolve_layer_str(layout: &Layout, s: &str) -> Result<i64, OpError> {
    if let Ok(n) = s.parse::<i64>() {
        return if layout.layer(n).is_some() { Ok(n) } else { err(format!("no layer {n}")) };
    }
    if s.eq_ignore_ascii_case("base") {
        return Ok(0);
    }
    let lower = s.to_lowercase();
    let exact: Vec<&Layer> = layout.layers.iter().filter(|l| l.title.to_lowercase() == lower).collect();
    let found: Vec<&Layer> =
        if exact.is_empty() { layout.layers.iter().filter(|l| l.title.to_lowercase().starts_with(&lower)).collect() } else { exact };
    match found.as_slice() {
        [one] => Ok(one.position),
        [] => err(format!(
            "no layer called '{s}' (layers: {})",
            layout.layers.iter().map(|l| l.title.as_str()).collect::<Vec<_>>().join(", ")
        )),
        _ => err(format!("'{s}' matches several layers: {}", found.iter().map(|l| l.title.as_str()).collect::<Vec<_>>().join(", "))),
    }
}

/// A key given as a position name (`L.home.4`), an index (`#17` or 17), or `key:X` (the key whose
/// base-layer tap is X).
pub fn resolve_key(layout: &Layout, sel: &Value) -> Result<usize, OpError> {
    let n = layout.key_count();
    let map = PositionMap::for_geometry(&layout.geometry);
    match sel {
        Value::Number(i) => {
            let i = i.as_u64().unwrap_or(u64::MAX) as usize;
            if i < n {
                Ok(i)
            } else {
                err(format!("key #{i} doesn't exist"))
            }
        }
        Value::String(s) => {
            if let Some(legend) = s.strip_prefix("key:") {
                let code = knowledge::canonical(legend).unwrap_or_else(|| legend.to_string());
                let base = layout.layers.first().map(|l| &l.keys[..]).unwrap_or(&[]);
                let sends = |a: Option<&Action>| a.and_then(Action::code).is_some_and(|c| knowledge::same_code(c, &code));
                // The key that sends it on tap; failing that, in any slot (e.g. Esc on hold).
                let by_tap: Vec<usize> = base.iter().enumerate().filter(|(_, k)| sends(k.tap.as_ref())).map(|(i, _)| i).collect();
                let hits = if by_tap.is_empty() {
                    base.iter().enumerate().filter(|(_, k)| k.slots().iter().any(|(_, a)| sends(Some(a)))).map(|(i, _)| i).collect()
                } else {
                    by_tap
                };
                return match hits.as_slice() {
                    [one] => Ok(*one),
                    [] => {
                        err(format!("no key on the base layer sends {legend} (`keyloom find {legend}` / find_keycode shows where it is)"))
                    }
                    _ => err(format!("several base-layer keys send {legend}; use a position")),
                };
            }
            if let Ok(i) = s.trim().parse::<usize>() {
                return if i < n { Ok(i) } else { err(format!("key #{i} doesn't exist (0–{})", n.saturating_sub(1))) };
            }
            parse_key(map.as_ref(), s, n).map_err(|message| OpError { op: None, message })
        }
        _ => err(format!("key must be a position like L.home.4, got {sel}")),
    }
}

// MARK: action specs

/// What an action spec turns into.
#[derive(Debug, Clone, PartialEq)]
pub struct Binding {
    pub key: Key,
    /// e.g. an unknown keycode that was accepted as written.
    pub warnings: Vec<String>,
}

fn act(code: &str) -> Action {
    Action { code: Some(code.into()), ..Action::default() }
}

const MODS: &[(&str, &str)] = &[
    ("LCTL", "KC_LEFT_CTRL"),
    ("LSFT", "KC_LEFT_SHIFT"),
    ("LALT", "KC_LEFT_ALT"),
    ("LGUI", "KC_LEFT_GUI"),
    ("RCTL", "KC_RIGHT_CTRL"),
    ("RSFT", "KC_RIGHT_SHIFT"),
    ("RALT", "KC_RIGHT_ALT"),
    ("RGUI", "KC_RIGHT_GUI"),
];

fn mod_keycode(name: &str) -> Option<&'static str> {
    let n = name.trim().trim_start_matches("MOD_").to_uppercase();
    // QMK's short forms (C(kc), S(kc), CTL_T(kc), …) mean the left-hand modifier.
    let n = match n.as_str() {
        "LCMD" | "LWIN" | "G" | "GUI" | "CMD" | "WIN" => "LGUI",
        "RCMD" | "RWIN" => "RGUI",
        "LOPT" | "A" | "ALT" | "OPT" => "LALT",
        "ROPT" | "ALGR" => "RALT",
        "C" | "CTL" | "CTRL" | "LCTRL" => "LCTL",
        "RCTRL" => "RCTL",
        "S" | "SFT" | "SHIFT" | "LSHIFT" => "LSFT",
        "RSHIFT" => "RSFT",
        other => other,
    };
    MODS.iter().find(|(m, _)| *m == n).map(|(_, k)| *k).or_else(|| {
        // Single letters that aren't modifier short forms are keys (KC_B), not modifiers.
        if n.len() <= 1 {
            return None;
        }
        knowledge::canonical(name).and_then(|c| MODS.iter().find(|(_, k)| *k == c).map(|(_, k)| *k))
    })
}

fn set_modifier(m: &mut Modifiers, keycode: &str) {
    match keycode {
        "KC_LEFT_CTRL" => m.left_ctrl = true,
        "KC_LEFT_SHIFT" => m.left_shift = true,
        "KC_LEFT_ALT" => m.left_alt = true,
        "KC_LEFT_GUI" => m.left_gui = true,
        "KC_RIGHT_CTRL" => m.right_ctrl = true,
        "KC_RIGHT_SHIFT" => m.right_shift = true,
        "KC_RIGHT_ALT" => m.right_alt = true,
        _ => m.right_gui = true,
    }
}

/// Splits `LT(1, KC_SPC)` into ("LT", ["1", "KC_SPC"]), respecting nesting.
fn call(s: &str) -> Option<(String, Vec<String>)> {
    let s = s.trim();
    let open = s.find('(')?;
    if !s.ends_with(')') {
        return None;
    }
    let head = s[..open].trim().to_uppercase();
    let inner = &s[open + 1..s.len() - 1];
    let mut args = Vec::new();
    let mut depth = 0;
    let mut cur = String::new();
    for c in inner.chars() {
        match c {
            '(' => {
                depth += 1;
                cur.push(c)
            }
            ')' => {
                depth -= 1;
                cur.push(c)
            }
            ',' if depth == 0 => args.push(std::mem::take(&mut cur).trim().to_string()),
            _ => cur.push(c),
        }
    }
    if !cur.trim().is_empty() || !args.is_empty() {
        args.push(cur.trim().to_string());
    }
    Some((head, args))
}

/// A plain keycode, possibly wrapped in modifier calls (`LCTL(LSFT(KC_C))`), as one action.
fn parse_tap_action(spec: &str, warnings: &mut Vec<String>) -> Result<Action, OpError> {
    let s = spec.trim();
    if let Some((head, args)) = call(s) {
        if let (Some(m), [inner]) = (mod_keycode(&head), args.as_slice()) {
            let mut a = parse_tap_action(inner, warnings)?;
            if matches!(a.code(), Some("KC_TRANSPARENT" | "KC_TRNS" | "KC_NO" | "RGB")) {
                return err(format!(
                    "'{s}': a modifier needs a key to modify; write TRNS or NO on its own, or the modifier key itself (e.g. KC_LCTL)"
                ));
            }
            let mut mods = a.modifiers.take().unwrap_or_default();
            set_modifier(&mut mods, m);
            a.modifiers = Some(mods);
            return Ok(a);
        }
        if head == "RGB" {
            if let [color] = args.as_slice() {
                return Ok(Action { code: Some("RGB".into()), color: Some(check_color(color)?), ..Action::default() });
            }
        }
        return err(match head.as_str() {
            "MO" | "TG" | "TO" | "TT" | "OSL" | "DF" | "LT" | "MT" | "TAP_HOLD" => {
                format!("'{s}': {head}(…) is a whole-key action and can't be used inside another action or as a tap key")
            }
            "MEH" | "HYPR" => format!(
                "'{s}': Oryx only has Meh/Hyper as a hold action: write MEH_T(KC_x) or ALL_T(KC_x), or name the modifiers: LCTL(LSFT(LALT(KC_x)))"
            ),
            "MEH_T" | "ALL_T" | "HYPR_T" => format!(
                "'{s}': {head}(…) describes a whole key (tap + hold), so it can't go inside another action, in a single slot or on a combo. \
                 Set the whole key to it, or put just Meh/Hyper in the hold slot (slot hold, action MEH or ALL_T)"
            ),
            _ if head.ends_with("_T") && mod_keycode(head.trim_end_matches("_T")).is_some() => format!(
                "'{s}': {head}(…) describes a whole key (tap + hold), so it can't go inside another action, in a single slot or on a combo. \
                 Set the whole key to it, or put the modifier key in the hold slot"
            ),
            "OSM" | "LM" => format!("{head} isn't supported yet: keyloom doesn't know how Oryx stores it"),
            _ if head.ends_with("_T") => {
                format!("'{s}': Oryx mod-taps take one modifier: LCTL_T(KC_x), LSFT_T, LALT_T, LGUI_T (or R…), MEH_T, ALL_T")
            }
            _ => format!(
                "'{s}' isn't an action keyloom knows. Forms: KC_A, LCTL(KC_C), MO(layer), TG(layer), TO(layer), TT(layer), OSL(layer), \
                 DF(layer), LT(layer, KC_x), LCTL_T(KC_x), MT(LCTL, KC_x), MEH_T(KC_x), TAP_HOLD(KC_x, KC_y), RGB(#rrggbb), TRNS, NO"
            ),
        });
    }
    plain_code(s, warnings).map(|c| act(&c))
}

/// Spec heads that need arguments; written bare they must not fall through to a keycode
/// (`LT` would otherwise find `KC_LT`, the `<` key).
const BARE_HEADS: &[(&str, &str)] = &[
    ("LT", "LT(<layer>, KC_x). For the '<' key write KC_LT or '<'"),
    ("GT", "nothing; for the '>' key write KC_GT or '>'"),
    ("MT", "MT(LCTL, KC_x)"),
    ("MO", "MO(<layer>)"),
    ("TG", "TG(<layer>)"),
    ("TO", "TO(<layer>)"),
    ("TT", "TT(<layer>)"),
    ("OSL", "OSL(<layer>)"),
    ("DF", "DF(<layer>)"),
    ("TAP_HOLD", "TAP_HOLD(KC_x, KC_y)"),
    ("RGB", "RGB(#rrggbb)"),
    ("OSM", "OSM(…), which keyloom doesn't support yet"),
    ("LM", "LM(…), which keyloom doesn't support yet"),
];

fn plain_code(s: &str, warnings: &mut Vec<String>) -> Result<String, OpError> {
    if let Some((head, form)) = BARE_HEADS.iter().find(|(h, _)| h.eq_ignore_ascii_case(s)) {
        return err(format!("'{head}' on its own isn't a key; it needs: {form}"));
    }
    if let Some(k) = knowledge::lookup(s) {
        // A friendly alias of something that needs arguments ("Color" → RGB, "One Shot Mod" → OSM).
        if let Some((head, form)) = BARE_HEADS.iter().find(|(h, _)| *h == k.code) {
            return err(format!("'{s}' ({head}) on its own isn't a key; it needs: {form}"));
        }
        if k.layer {
            return err(format!("'{s}' is a layer action and needs a layer: {}(<layer>)", k.code));
        }
        if k.mod_tap {
            return err(format!(
                "'{s}' is only known as a hold action in Oryx: write {code}(KC_x), or set the hold slot to {code}. \
                 A standalone Meh/Hyper key's Oryx encoding isn't confirmed",
                code = k.code
            ));
        }
        return Ok(k.code.clone());
    }
    if let Some(c) = knowledge::canonical(s) {
        return Ok(c);
    }
    let up = s.to_uppercase();
    if up.ends_with("_T") {
        return err(format!("'{s}' looks like a mod-tap, which wraps a key: {up}(KC_x). Oryx mod-taps take one modifier (LCTL_T, LSFT_T, …), MEH_T or ALL_T"));
    }
    if !up.is_empty() && up.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') && up.contains('_') {
        warnings.push(format!("{up} isn't in keyloom's keycode reference; kept as written"));
        return Ok(up);
    }
    err(format!("unknown keycode '{s}' (search with `keyloom ref {s}` or the lookup_keycode tool)"))
}

/// A layer named in an action: an existing layer number or a title.
fn layer_arg(layout: &Layout, arg: &str) -> Result<i64, OpError> {
    let arg = arg.trim();
    match arg.parse::<i64>() {
        Ok(n) if layout.layer(n).is_some() => Ok(n),
        Ok(n) => err(format!(
            "layer {n} doesn't exist (layers 0–{}); add it first (earlier in the same batch is fine)",
            layout.layers.len().saturating_sub(1)
        )),
        _ => resolve_layer_str(layout, arg),
    }
}

/// A tap or hold argument of TAP_HOLD: `_` means empty.
fn optional_action(spec: &str, warnings: &mut Vec<String>) -> Result<Option<Action>, OpError> {
    if spec.trim() == "_" {
        return Ok(None);
    }
    parse_tap_action(spec, warnings).map(Some)
}

/// The hold side of TAP_HOLD: also Meh/Hyper, which only exist as hold actions.
fn optional_hold_action(spec: &str, warnings: &mut Vec<String>) -> Result<Option<Action>, OpError> {
    if call(spec).is_none() {
        if let Some(k) = knowledge::lookup(spec).filter(|k| k.mod_tap) {
            return Ok(Some(act(&k.code)));
        }
    }
    optional_action(spec, warnings)
}

/// Parses an action spec (spec 02) into an Oryx-shaped key.
pub fn parse_binding(layout: &Layout, spec: &str) -> Result<Binding, OpError> {
    let mut warnings = Vec::new();
    let s = spec.trim();
    let key = if let Some((head, args)) = call(s) {
        match (head.as_str(), args.as_slice()) {
            ("MO", [l]) => Key { hold: Some(Action { layer: Some(layer_arg(layout, l)?), ..act("MO") }), ..Key::default() },
            ("TG" | "TO" | "TT" | "OSL" | "DF", [l]) => {
                Key { tap: Some(Action { layer: Some(layer_arg(layout, l)?), ..act(&head) }), ..Key::default() }
            }
            ("LT", [l, k]) => Key {
                tap: Some(parse_tap_action(k, &mut warnings)?),
                hold: Some(Action { layer: Some(layer_arg(layout, l)?), ..act("MO") }),
                ..Key::default()
            },
            ("MT", [m, k]) => {
                let Some(mk) = mod_keycode(m) else { return err(format!("'{m}' isn't a single modifier (Oryx mod-taps take one)")) };
                Key { tap: Some(parse_tap_action(k, &mut warnings)?), hold: Some(act(mk)), ..Key::default() }
            }
            ("MEH_T" | "ALL_T" | "HYPR_T", [k]) => {
                let hold = if head == "MEH_T" { "MEH_T" } else { "ALL_T" };
                Key { tap: Some(parse_tap_action(k, &mut warnings)?), hold: Some(act(hold)), ..Key::default() }
            }
            (h, [k]) if h.ends_with("_T") && mod_keycode(h.trim_end_matches("_T")).is_some() => Key {
                tap: Some(parse_tap_action(k, &mut warnings)?),
                hold: Some(act(mod_keycode(h.trim_end_matches("_T")).expect("checked"))),
                ..Key::default()
            },
            ("TAP_HOLD", [t, h]) => {
                let (tap, hold) = (optional_action(t, &mut warnings)?, optional_hold_action(h, &mut warnings)?);
                if tap.is_none() && hold.is_none() {
                    return err("TAP_HOLD(_, _) is an empty key: use TRNS or NO");
                }
                Key { tap, hold, ..Key::default() }
            }
            ("OSM", _) | ("LM", _) => return err(format!("{head} isn't supported yet: keyloom doesn't know how Oryx stores it")),
            ("MO" | "TG" | "TO" | "TT" | "OSL" | "DF", _) => {
                return err(format!("{head} takes one layer: {head}(<layer number or title>)"))
            }
            ("LT", _) => return err("LT takes a layer and a key: LT(<layer>, KC_x)"),
            ("MT", _) => return err("MT takes a modifier and a key: MT(LCTL, KC_x)"),
            ("TAP_HOLD", _) => return err("TAP_HOLD takes a tap key and a hold key: TAP_HOLD(KC_x, KC_y)"),
            _ => Key { tap: Some(parse_tap_action(s, &mut warnings)?), ..Key::default() },
        }
    } else {
        Key { tap: Some(parse_tap_action(s, &mut warnings)?), ..Key::default() }
    };
    Ok(Binding { key, warnings })
}

/// Parses an action for a single slot (or a combo's trigger: `slot` = `None`). Layer actions
/// are allowed in any slot; Meh/Hyper only as a hold action.
pub fn parse_slot_action(layout: &Layout, spec: &str, slot: Option<Slot>) -> Result<(Action, Vec<String>), OpError> {
    let mut warnings = Vec::new();
    if let Some((head, args)) = call(spec) {
        if let ("MO" | "TG" | "TO" | "TT" | "OSL" | "DF", [l]) = (head.as_str(), args.as_slice()) {
            return Ok((Action { layer: Some(layer_arg(layout, l)?), ..act(&head) }, warnings));
        }
    } else if slot == Some(Slot::Hold) {
        // Meh/Hyper (and their aliases) as a bare hold action.
        if let Some(k) = knowledge::lookup(spec).filter(|k| k.mod_tap) {
            return Ok((act(&k.code), warnings));
        }
    }
    let a = parse_tap_action(spec, &mut warnings)?;
    Ok((a, warnings))
}

// MARK: operations

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Fill {
    /// Blank keys: fall through to the layer below (what Oryx does for empty keys).
    #[default]
    Trans,
    /// KC_NO: do nothing.
    None,
}

/// A field that may be `null` but has to be written out.
fn required_nullable<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    Option::<String>::deserialize(d)
}

/// `#rrggbb` (what Oryx stores), lower-cased.
fn check_color(color: &str) -> Result<String, OpError> {
    let c = color.trim();
    let hex = c.strip_prefix('#').unwrap_or("");
    if hex.len() == 6 && hex.chars().all(|ch| ch.is_ascii_hexdigit()) {
        Ok(c.to_lowercase())
    } else {
        err(format!("'{color}' isn't a color; write it as #rrggbb, e.g. #ff8800"))
    }
}

fn optional_color(color: &Option<String>) -> Result<Option<String>, OpError> {
    color.as_deref().filter(|s| !s.trim().is_empty()).map(check_color).transpose()
}

/// One edit. JSON: `{"op": "set_key", "layer": "Gaming", "key": "L.home.1", "action": "KC_A"}`.
///
/// Unknown fields are rejected, and fields that may be null (`action`, `label`, `color`) must
/// still be present: a misspelt field must never turn into a different edit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Op {
    SetKey {
        layer: Value,
        key: Value,
        action: String,
    },
    SetSlot {
        layer: Value,
        key: Value,
        slot: Slot,
        /// `null` empties the slot.
        #[serde(deserialize_with = "required_nullable")]
        action: Option<String>,
    },
    ClearKey {
        layer: Value,
        key: Value,
        /// `trans` (default) or `none`; `fill` is accepted as another name for it.
        #[serde(default, alias = "fill")]
        to: Fill,
    },
    SetLabel {
        layer: Value,
        key: Value,
        /// `null` removes the label.
        #[serde(deserialize_with = "required_nullable")]
        label: Option<String>,
    },
    SetColor {
        layer: Value,
        key: Value,
        /// `#rrggbb`; `null` removes the color.
        #[serde(deserialize_with = "required_nullable")]
        color: Option<String>,
    },
    AddLayer {
        title: String,
        #[serde(default)]
        after: Option<Value>,
        #[serde(default)]
        copy_from: Option<Value>,
        #[serde(default)]
        fill: Fill,
        #[serde(default)]
        tags: Vec<String>,
        #[serde(default)]
        color: Option<String>,
    },
    MoveLayer {
        layer: Value,
        to: i64,
    },
    RemoveLayer {
        layer: Value,
        #[serde(default)]
        force: bool,
    },
    RenameLayer {
        layer: Value,
        title: String,
    },
    TagLayer {
        layer: Value,
        tags: Vec<String>,
    },
    ColorLayer {
        layer: Value,
        /// `#rrggbb`; `null` removes the color.
        #[serde(deserialize_with = "required_nullable")]
        color: Option<String>,
    },
    AddCombo {
        layer: Value,
        keys: Vec<Value>,
        action: String,
        #[serde(default)]
        name: Option<String>,
    },
    RemoveCombo {
        layer: Value,
        keys: Vec<Value>,
    },
    SwapKeys {
        layer: Value,
        a: Value,
        b: Value,
    },
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Applied {
    pub warnings: Vec<String>,
}

fn fill_key(fill: Fill) -> Key {
    match fill {
        Fill::Trans => Key::default(),
        Fill::None => Key { tap: Some(act("KC_NO")), ..Key::default() },
    }
}

/// Rewrites every layer reference (key actions and combos) with `f(old) -> Option<new>`;
/// `None` means the target is gone.
fn remap_layers(layout: &mut Layout, f: impl Fn(i64) -> Option<i64>) -> Vec<String> {
    let mut dropped = Vec::new();
    let map = PositionMap::for_geometry(&layout.geometry);
    for layer in &mut layout.layers {
        let title = layer.title.clone();
        for (i, key) in layer.keys.iter_mut().enumerate() {
            for slot in [&mut key.tap, &mut key.hold, &mut key.double_tap, &mut key.tap_hold] {
                if let Some(a) = slot {
                    if let Some(n) = a.layer {
                        match f(n) {
                            Some(m) => a.layer = Some(m),
                            None => {
                                dropped.push(format!("{} on {title}", crate::positions::display_name(map.as_ref(), i)));
                                *slot = None;
                            }
                        }
                    }
                }
            }
        }
    }
    layout.combos.retain_mut(|c| {
        let target_ok = match c["trigger"]["layer"].as_i64() {
            Some(n) => match f(n) {
                Some(m) => {
                    c["trigger"]["layer"] = m.into();
                    true
                }
                None => false,
            },
            None => true,
        };
        let layer_ok = match c["layerIdx"].as_i64() {
            Some(n) => match f(n) {
                Some(m) => {
                    c["layerIdx"] = m.into();
                    true
                }
                None => false,
            },
            None => true,
        };
        if !(target_ok && layer_ok) {
            let keys =
                sorted_keys(c).iter().map(|&i| crate::positions::display_name(map.as_ref(), i as usize)).collect::<Vec<_>>().join("+");
            dropped.push(match c["name"].as_str().filter(|n| !n.is_empty()) {
                Some(n) => format!("combo \"{n}\" ({keys})"),
                None => format!("combo {keys}"),
            });
        }
        target_ok && layer_ok
    });
    dropped
}

/// Renumbers existing layers with `new_of(old)`. References to layers that don't exist are
/// removed and reported (they can come from imported files).
fn renumber(layout: &mut Layout, new_of: impl Fn(i64) -> Option<i64>, warnings: &mut Vec<String>) {
    let dropped = remap_layers(layout, &new_of);
    if !dropped.is_empty() {
        warnings.push(format!("removed references to layers that don't exist: {}", dropped.join(", ")));
    }
    if let Some(auto) = layout.settings.auto_mouse_layer {
        layout.settings.auto_mouse_layer = new_of(auto);
    }
    for l in &mut layout.layers {
        l.position = new_of(l.position).expect("every existing layer has a new position");
    }
    layout.layers.sort_by_key(|l| l.position);
}

pub const KNOWN_TAGS: &[&str] = &["base", "gaming", "nav", "symbols", "numbers", "mouse", "function", "media", "auto"];

/// Trimmed, lower-cased, de-duplicated tags; unknown ones are kept but reported.
fn normalize_tags(tags: &[String], warnings: &mut Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for t in tags.iter().flat_map(|t| t.split(',')).map(|t| t.trim().to_lowercase()).filter(|t| !t.is_empty()) {
        if !KNOWN_TAGS.contains(&t.as_str()) {
            warnings.push(format!("tag '{t}' has no meaning to keyloom (known: {})", KNOWN_TAGS.join(", ")));
        }
        if !out.contains(&t) {
            out.push(t);
        }
    }
    out
}

/// A usable, unique layer title (trimmed).
fn check_title(layout: &Layout, title: &str, except: Option<i64>) -> Result<String, OpError> {
    let t = title.trim();
    if t.is_empty() {
        return err("a layer needs a title");
    }
    if t.chars().any(char::is_control) {
        return err("a layer title is one line of text (no tabs or line breaks)");
    }
    if t.parse::<i64>().is_ok() {
        return err(format!("'{t}' would be read as a layer number; pick a title that isn't a number"));
    }
    if t.eq_ignore_ascii_case("base") || t.eq_ignore_ascii_case("all") {
        return err(format!("'{t}' is reserved ('base' means layer 0, 'all' means every layer)"));
    }
    if t.contains(['(', ')', ',']) {
        return err("layer titles can't contain '(', ')' or ',' (they are used in actions like MO(title))");
    }
    if layout.layers.iter().any(|l| Some(l.position) != except && l.title.trim().to_lowercase() == t.to_lowercase()) {
        return err(format!("a layer called '{t}' already exists"));
    }
    Ok(t.to_string())
}

fn key_mut(layout: &mut Layout, layer: i64, index: usize) -> &mut Key {
    &mut layout.layer_mut(layer).expect("resolved").keys[index]
}

fn combo_keys(layout: &Layout, keys: &[Value]) -> Result<Vec<i64>, OpError> {
    let mut out = keys.iter().map(|k| resolve_key(layout, k).map(|i| i as i64)).collect::<Result<Vec<_>, _>>()?;
    out.sort();
    out.dedup();
    if out.len() < 2 {
        return err("a combo needs at least two different keys");
    }
    Ok(out)
}

/// Applies one op in place.
pub fn apply_op(layout: &mut Layout, op: &Op) -> Result<Applied, OpError> {
    let mut applied = Applied::default();
    match op {
        Op::SetKey { layer, key, action } => {
            let (l, i) = (resolve_layer(layout, layer)?, resolve_key(layout, key)?);
            let b = parse_binding(layout, action)?;
            applied.warnings = b.warnings;
            let k = key_mut(layout, l, i);
            // The whole key is replaced. Only the LED color stays: it belongs to the position.
            let (old_label, glow) = (k.custom_label.take(), k.glow_color.take());
            *k = Key { glow_color: glow, ..b.key };
            if let Some(old) = old_label {
                applied.warnings.push(format!("custom label \"{old}\" removed (it described the old key); set a new one with set_label"));
            }
        }
        Op::SetSlot { layer, key, slot, action } => {
            let (l, i) = (resolve_layer(layout, layer)?, resolve_key(layout, key)?);
            let a = match action {
                Some(spec) => {
                    let (a, w) = parse_slot_action(layout, spec, Some(*slot))?;
                    applied.warnings = w;
                    Some(a)
                }
                None => None,
            };
            let k = key_mut(layout, l, i);
            match slot {
                Slot::Tap => k.tap = a,
                Slot::Hold => k.hold = a,
                Slot::DoubleTap => k.double_tap = a,
                Slot::TapHold => k.tap_hold = a,
            }
        }
        Op::ClearKey { layer, key, to } => {
            let (l, i) = (resolve_layer(layout, layer)?, resolve_key(layout, key)?);
            // Like set_key: the LED color belongs to the position and stays.
            let k = key_mut(layout, l, i);
            let glow = k.glow_color.take();
            *k = Key { glow_color: glow, ..fill_key(*to) };
        }
        Op::SetLabel { layer, key, label } => {
            let (l, i) = (resolve_layer(layout, layer)?, resolve_key(layout, key)?);
            let label = label.as_deref().map(str::trim).filter(|s| !s.is_empty());
            if label.is_some_and(|s| s.chars().any(char::is_control)) {
                return err("a label is one line of text (no tabs or line breaks)");
            }
            key_mut(layout, l, i).custom_label = label.map(str::to_string);
        }
        Op::SetColor { layer, key, color } => {
            let (l, i) = (resolve_layer(layout, layer)?, resolve_key(layout, key)?);
            key_mut(layout, l, i).glow_color = optional_color(color)?;
        }
        Op::AddLayer { title, after, copy_from, fill, tags, color } => {
            let title = check_title(layout, title, None)?;
            let color = optional_color(color)?;
            let tags = normalize_tags(tags, &mut applied.warnings);
            let n = layout.key_count();
            let end = layout.layers.len() as i64;
            // Resolve everything before changing anything.
            let source = copy_from.as_ref().map(|sel| resolve_layer(layout, sel)).transpose()?;
            let insert_at = match after {
                Some(sel) => resolve_layer(layout, sel)? + 1,
                None => end,
            };
            // Make room first, so the new layer's number is never mistaken for an old one.
            if insert_at != end {
                renumber(layout, |n| (0..end).contains(&n).then_some(if n < insert_at { n } else { n + 1 }), &mut applied.warnings);
            }
            // Copy after renumbering, so the copy's layer keys point where the source's do.
            let keys = match source {
                Some(from) => {
                    let mut keys = layout.layer(if from < insert_at { from } else { from + 1 }).expect("resolved").keys.clone();
                    // `fill: none` on a copy: what the source left transparent does nothing here.
                    if *fill == Fill::None {
                        for k in keys.iter_mut().filter(|k| k.is_transparent()) {
                            *k = Key { custom_label: k.custom_label.take(), glow_color: k.glow_color.take(), ..fill_key(Fill::None) };
                        }
                    }
                    keys
                }
                None => vec![fill_key(*fill); n],
            };
            layout.layers.push(Layer {
                title,
                position: insert_at,
                color,
                keyloom: LayerMeta { tags, notes: None },
                keys,
                extra: Default::default(),
            });
            layout.layers.sort_by_key(|l| l.position);
        }
        Op::MoveLayer { layer, to } => {
            let from = resolve_layer(layout, layer)?;
            let count = layout.layers.len() as i64;
            if !(0..count).contains(to) {
                return err(format!("layer position {to} is out of range (0–{})", count - 1));
            }
            if from == *to {
                return Ok(applied);
            }
            // `order[new] = old`
            let mut order: Vec<i64> = (0..count).filter(|&p| p != from).collect();
            order.insert(*to as usize, from);
            renumber(layout, |old| order.iter().position(|&o| o == old).map(|p| p as i64), &mut applied.warnings);
            if *to == 0 || from == 0 {
                applied.warnings.push(format!(
                    "this changes the base layer: \"{}\" is now layer 0; transparent keys fall through to it",
                    layout.layer_title(0)
                ));
            }
        }
        Op::RemoveLayer { layer, force } => {
            let target = resolve_layer(layout, layer)?;
            if layout.layers.len() == 1 {
                return err("can't remove the only layer");
            }
            let map = PositionMap::for_geometry(&layout.geometry);
            let combo_name = |c: &Value| match c["name"].as_str().filter(|n| !n.is_empty()) {
                Some(n) => format!("\"{n}\""),
                None => {
                    sorted_keys(c).iter().map(|&i| crate::positions::display_name(map.as_ref(), i as usize)).collect::<Vec<_>>().join("+")
                }
            };
            // Keys and combos elsewhere that switch to this layer block the removal unless forced.
            let mut refs: Vec<String> = crate::explore::layer_graph(layout)
                .edges
                .iter()
                .filter(|e| e.to == target && e.from != target && !e.via_transparent && !e.combo)
                .map(|e| format!("{} on layer {}", e.position, e.from))
                .collect();
            for c in &layout.combos {
                if c["trigger"]["layer"].as_i64() == Some(target) && c["layerIdx"].as_i64() != Some(target) {
                    refs.push(format!("combo {} on layer {}", combo_name(c), c["layerIdx"].as_i64().unwrap_or(0)));
                }
            }
            if !refs.is_empty() && !force {
                return err(format!("layer {target} is still used by {}; change those keys/combos first or use force", refs.join(", ")));
            }
            // Combos on the layer itself are part of it, like its keys.
            let own: Vec<String> = layout.combos.iter().filter(|c| c["layerIdx"].as_i64() == Some(target)).map(combo_name).collect();
            layout.combos.retain(|c| c["layerIdx"].as_i64() != Some(target));
            if !own.is_empty() {
                applied.warnings.push(format!("also removed {} combo(s) on the deleted layer: {}", own.len(), own.join(", ")));
            }
            layout.layers.retain(|l| l.position != target);
            let shift = |n: i64| match n.cmp(&target) {
                std::cmp::Ordering::Less => Some(n),
                std::cmp::Ordering::Equal => None,
                std::cmp::Ordering::Greater => Some(n - 1),
            };
            let dropped = remap_layers(layout, shift);
            if layout.settings.auto_mouse_layer == Some(target) {
                applied
                    .warnings
                    .push("this was the Navigator's auto-mouse layer; the draft now has none (change that setting in Oryx too)".into());
            }
            layout.settings.auto_mouse_layer = layout.settings.auto_mouse_layer.and_then(shift);
            for l in &mut layout.layers {
                if l.position > target {
                    l.position -= 1;
                }
            }
            if !dropped.is_empty() {
                applied.warnings.push(format!("removed references to the deleted layer: {}", dropped.join(", ")));
            }
            if target == 0 {
                applied.warnings.push(format!(
                    "this changes the base layer: \"{}\" is now layer 0; transparent keys fall through to it",
                    layout.layer_title(0)
                ));
            }
        }
        Op::RenameLayer { layer, title } => {
            let l = resolve_layer(layout, layer)?;
            let title = check_title(layout, title, Some(l))?;
            layout.layer_mut(l).expect("resolved").title = title;
        }
        Op::TagLayer { layer, tags } => {
            let l = resolve_layer(layout, layer)?;
            let tags = normalize_tags(tags, &mut applied.warnings);
            layout.layer_mut(l).expect("resolved").keyloom.tags = tags;
        }
        Op::ColorLayer { layer, color } => {
            let l = resolve_layer(layout, layer)?;
            layout.layer_mut(l).expect("resolved").color = optional_color(color)?;
        }
        Op::AddCombo { layer, keys, action, name } => {
            let l = resolve_layer(layout, layer)?;
            let keys = combo_keys(layout, keys)?;
            let (trigger, w) = parse_slot_action(layout, action, None)?;
            applied.warnings = w;
            let exists = layout.combos.iter().any(|c| c["layerIdx"].as_i64() == Some(l) && sorted_keys(c) == keys);
            if exists {
                return err("a combo with these keys already exists on that layer");
            }
            let mut combo = serde_json::json!({"keyIndices": keys, "layerIdx": l, "trigger": trigger});
            if let Some(n) = name {
                combo["name"] = n.clone().into();
            }
            layout.combos.push(combo);
        }
        Op::RemoveCombo { layer, keys } => {
            let l = resolve_layer(layout, layer)?;
            let keys = combo_keys(layout, keys)?;
            let before = layout.combos.len();
            layout.combos.retain(|c| !(c["layerIdx"].as_i64() == Some(l) && sorted_keys(c) == keys));
            if layout.combos.len() == before {
                return err("no combo with these keys on that layer");
            }
        }
        Op::SwapKeys { layer, a, b } => {
            let (ia, ib) = (resolve_key(layout, a)?, resolve_key(layout, b)?);
            let all = matches!(layer, Value::String(s) if s.eq_ignore_ascii_case("all"));
            let targets: Vec<i64> =
                if all { layout.layers.iter().map(|l| l.position).collect() } else { vec![resolve_layer(layout, layer)?] };
            for &t in &targets {
                layout.layer_mut(t).expect("resolved").keys.swap(ia, ib);
            }
            // Combos on those layers follow their keys.
            let (ia, ib) = (ia as i64, ib as i64);
            for c in &mut layout.combos {
                if !targets.contains(&c["layerIdx"].as_i64().unwrap_or(0)) {
                    continue;
                }
                if let Some(keys) = c["keyIndices"].as_array_mut() {
                    for k in keys {
                        match k.as_i64() {
                            Some(x) if x == ia => *k = ib.into(),
                            Some(x) if x == ib => *k = ia.into(),
                            _ => {}
                        }
                    }
                }
            }
        }
    }
    Ok(applied)
}

pub fn sorted_keys(combo: &Value) -> Vec<i64> {
    let mut k: Vec<i64> = combo["keyIndices"].as_array().into_iter().flatten().filter_map(Value::as_i64).collect();
    k.sort();
    k
}

/// The shape every edit relies on: layers numbered 0..n without gaps, each with the
/// geometry's number of keys. Oryx always produces that; a hand-edited file may not.
fn check_editable(layout: &Layout) -> Result<(), OpError> {
    let positions: Vec<i64> = layout.layers.iter().map(|l| l.position).collect();
    if positions.iter().enumerate().any(|(i, &p)| p != i as i64) {
        return err(format!(
            "this layout can't be edited: its layers are numbered {positions:?} instead of 0–{}; re-import it from Oryx",
            positions.len().saturating_sub(1)
        ));
    }
    let expected = PositionMap::for_geometry(&layout.geometry).map(|m| m.key_count()).unwrap_or_else(|| layout.key_count());
    if let Some(l) = layout.layers.iter().find(|l| l.keys.len() != expected) {
        return err(format!(
            "this layout can't be edited: layer \"{}\" has {} keys, the {} has {expected}; re-import it from Oryx",
            l.title,
            l.keys.len(),
            layout.geometry
        ));
    }
    Ok(())
}

/// Applies a batch atomically: all ops or none. Only drafts can be edited.
pub fn apply_ops(layout: &mut Layout, ops: &[Op]) -> Result<Applied, OpError> {
    if layout.kind != Kind::Draft {
        return err("only drafts can be edited; create one with `keyloom draft new <name>` (MCP: create_draft)");
    }
    check_editable(layout)?;
    let mut work = layout.clone();
    let mut all = Applied::default();
    for (i, op) in ops.iter().enumerate() {
        let a = apply_op(&mut work, op).map_err(|e| OpError { op: Some(i), message: e.message })?;
        all.warnings.extend(a.warnings);
    }
    *layout = work;
    Ok(all)
}
