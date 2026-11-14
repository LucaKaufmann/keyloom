//! QMK-style names for Oryx keys, e.g. tap `KC_SPACE` + hold `MO(1)` → `LT(1, KC_SPACE)`.
//!
//! Oryx stores slots separately and has no `LT`/`MT` codes (docs/oryx-encodings.md); people and agents think
//! in QMK terms, so explore output shows both.

use crate::model::{Action, Key};

/// One action as QMK would write it: `KC_A`, `MO(1)`, `LCTL(KC_LEFT)`, `RGB(#ffab00)`.
pub fn action(a: &Action) -> Option<String> {
    let code = a.code()?;
    let mut s = match a.layer {
        Some(n) => format!("{code}({n})"),
        None => code.to_string(),
    };
    if code == "RGB" {
        if let Some(c) = &a.color {
            s = format!("RGB({c})");
        }
    }
    if let Some(m) = &a.modifiers {
        for name in m.qmk_names().iter().rev() {
            s = format!("{name}({s})");
        }
    }
    Some(s)
}

/// `MOD_*` name for a modifier keycode used as a hold action.
fn mod_name(code: &str) -> Option<&'static str> {
    Some(match code {
        "KC_LEFT_CTRL" | "KC_LCTL" => "MOD_LCTL",
        "KC_LEFT_SHIFT" | "KC_LSFT" => "MOD_LSFT",
        "KC_LEFT_ALT" | "KC_LALT" => "MOD_LALT",
        "KC_LEFT_GUI" | "KC_LGUI" => "MOD_LGUI",
        "KC_RIGHT_CTRL" | "KC_RCTL" => "MOD_RCTL",
        "KC_RIGHT_SHIFT" | "KC_RSFT" => "MOD_RSFT",
        "KC_RIGHT_ALT" | "KC_RALT" => "MOD_RALT",
        "KC_RIGHT_GUI" | "KC_RGUI" => "MOD_RGUI",
        _ => return None,
    })
}

/// The whole key as one QMK-style expression.
///
/// - blank or `KC_TRANSPARENT` → `KC_TRANSPARENT`
/// - tap + hold `MO(n)` → `LT(n, tap)`; tap + hold modifier → `MT(MOD_x, tap)`;
///   tap + hold `MEH_T`/`ALL_T`/… → `MEH_T(tap)`
/// - tap + hold other keycode → `TAP_HOLD(tap, hold)` (Oryx compiles a custom dual-function key)
/// - double tap / tap-then-hold → `TD(tap, hold, doubleTap, tapHold)` with `_` for empty slots
pub fn key(k: &Key) -> String {
    if k.is_transparent() {
        return "KC_TRANSPARENT".into();
    }
    let tap = k.tap.as_ref().and_then(action);
    let hold = k.hold.as_ref().and_then(action);
    let dt = k.double_tap.as_ref().and_then(action);
    let th = k.tap_hold.as_ref().and_then(action);
    if dt.is_some() || th.is_some() {
        let part = |s: &Option<String>| s.clone().unwrap_or_else(|| "_".into());
        return format!("TD({}, {}, {}, {})", part(&tap), part(&hold), part(&dt), part(&th));
    }
    match (tap, hold) {
        (Some(t), None) => t,
        // A hold-only layer key is how Oryx stores MO(n); any other hold-only key is shown as
        // such, so it can't be mistaken for a tap key.
        (None, Some(h)) => {
            if k.hold.as_ref().is_some_and(|a| a.layer.is_some()) {
                h
            } else {
                format!("TAP_HOLD(_, {h})")
            }
        }
        (None, None) => "KC_NO".into(),
        (Some(t), Some(h)) => {
            let hold_action = k.hold.as_ref().expect("hold exists");
            let hold_code = hold_action.code().unwrap_or("");
            if hold_code == "MO" {
                if let Some(n) = hold_action.layer {
                    return format!("LT({n}, {t})");
                }
            }
            if hold_action.modifiers.is_none() {
                if let Some(m) = mod_name(hold_code) {
                    return format!("MT({m}, {t})");
                }
                if hold_code.ends_with("_T") {
                    return format!("{hold_code}({t})");
                }
            }
            format!("TAP_HOLD({t}, {h})")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn k(v: serde_json::Value) -> Key {
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn expressions() {
        assert_eq!(key(&k(json!({"tap": {"code": "KC_SPACE"}, "hold": {"code": "MO", "layer": 1}}))), "LT(1, KC_SPACE)");
        assert_eq!(key(&k(json!({"tap": {"code": "KC_TAB"}, "hold": {"code": "KC_LEFT_CTRL"}}))), "MT(MOD_LCTL, KC_TAB)");
        assert_eq!(key(&k(json!({"tap": {"code": "KC_BSPC"}, "hold": {"code": "MEH_T"}}))), "MEH_T(KC_BSPC)");
        assert_eq!(key(&k(json!({"tap": {"code": "KC_EQUAL"}, "hold": {"code": "KC_ESCAPE"}}))), "TAP_HOLD(KC_EQUAL, KC_ESCAPE)");
        assert_eq!(key(&k(json!({"hold": {"code": "MO", "layer": 2}}))), "MO(2)");
        assert_eq!(key(&k(json!({"hold": {"code": "KC_RCBR"}}))), "TAP_HOLD(_, KC_RCBR)");
        assert_eq!(key(&k(json!({}))), "KC_TRANSPARENT");
        assert_eq!(key(&k(json!({"tap": {"code": "KC_TRANSPARENT"}}))), "KC_TRANSPARENT");
        assert_eq!(key(&k(json!({"tap": {"code": "KC_NO"}}))), "KC_NO");
        assert_eq!(key(&k(json!({"tap": {"code": "KC_LEFT", "modifiers": {"leftCtrl": true, "leftShift": false}}}))), "LCTL(KC_LEFT)");
        assert_eq!(key(&k(json!({"tap": {"code": "RGB", "color": "#ff0000"}}))), "RGB(#ff0000)");
        assert_eq!(key(&k(json!({"tap": {"code": "KC_A"}, "doubleTap": {"code": "KC_CAPS"}}))), "TD(KC_A, _, KC_CAPS, _)");
        assert_eq!(key(&k(json!({"tap": {"code": "TO", "layer": 0}}))), "TO(0)");
    }
}
