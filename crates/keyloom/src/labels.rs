//! Adapted from voyager-ghost (<https://gitlab.com/tbulva/voyager-ghost>), MIT License,
//! Copyright (c) 2026 Tomas Bulva.
//!
//! What to draw on a key cap.

use crate::oryx::{OryxAction, OryxKey, OryxLayer, OryxModifiers};

/// Name of the GUI key on this platform.
#[cfg(windows)]
pub const GUI: &str = "Win";
#[cfg(not(windows))]
pub const GUI: &str = "Super";

#[derive(Debug, Clone, Default, PartialEq)]
pub struct KeyLegend {
    pub main: String,
    pub sub: Option<String>,
    /// Hex color for the main text (layer-switch keys use their target layer's color).
    pub main_color: Option<String>,
    pub sub_color: Option<String>,
    /// KC_TRNS: the legend shown is borrowed from the base layer.
    pub is_transparent: bool,
    /// KC_NO or unassigned.
    pub is_empty: bool,
}

struct Described {
    text: String,
    color: Option<String>,
}

pub fn legend_at(index: usize, position: i64, layers: &[OryxLayer]) -> KeyLegend {
    let key = layers.iter().find(|l| l.position == position).and_then(|l| l.keys.get(index));
    let Some(key) = key else {
        return KeyLegend { is_empty: true, ..Default::default() };
    };
    if is_transparent(key) && position != 0 {
        if let Some(base) = layers.iter().find(|l| l.position == 0).and_then(|l| l.keys.get(index)) {
            let mut borrowed = legend(base, layers);
            borrowed.is_transparent = true;
            return borrowed;
        }
    }
    legend(key, layers)
}

pub fn legend(key: &OryxKey, layers: &[OryxLayer]) -> KeyLegend {
    let mut legend = KeyLegend::default();
    let tap = key.tap.as_ref().and_then(|a| describe(a, layers));
    let hold = key.hold.as_ref().and_then(|a| describe(a, layers));
    let double_tap = key.double_tap.as_ref().and_then(|a| describe(a, layers));

    if let Some(emoji) = key.emoji.as_deref().filter(|s| !s.is_empty()) {
        legend.main = emoji.to_string();
    } else if let Some(label) = key.custom_label.as_deref().filter(|s| !s.is_empty()) {
        legend.main = label.to_string();
    } else if let Some(tap) = tap {
        legend.main = tap.text;
        legend.main_color = tap.color;
    } else if let Some(hold) = hold {
        // Hold-only keys (e.g. a plain hold-for-Shift) show the hold action as the main legend.
        legend.main = hold.text;
        legend.main_color = hold.color;
        return legend;
    }

    if let Some(hold) = hold {
        legend.sub = Some(hold.text);
        legend.sub_color = hold.color;
    } else if let Some(double_tap) = double_tap {
        legend.sub = Some(format!("2× {}", double_tap.text));
        legend.sub_color = double_tap.color;
    }

    if legend.main.is_empty() && legend.sub.is_none() {
        legend.is_empty = true;
    }
    legend
}

pub fn is_transparent(key: &OryxKey) -> bool {
    let code = key.tap.as_ref().and_then(|a| a.code.as_deref()).unwrap_or("");
    let has_hold = key.hold.as_ref().and_then(|a| a.code.as_ref()).is_some();
    (code == "KC_TRANSPARENT" || code == "KC_TRNS") && !has_hold
}

fn layer_prefix(code: &str) -> Option<&'static str> {
    Some(match code {
        "MO" | "LT" | "LM" => "",
        "TG" | "TT" => "⇄",
        "TO" => "→",
        "OSL" => "1×",
        "DF" => "⌂",
        _ => return None,
    })
}

fn modifier_prefix(m: &OryxModifiers) -> String {
    let mut out = String::new();
    if m.ctrl() {
        out.push('^');
    }
    if m.alt() {
        out.push_str("Alt+");
    }
    if m.shift() {
        out.push('⇧');
    }
    if m.gui() {
        out.push_str(GUI);
        out.push('+');
    }
    out
}

fn describe(action: &OryxAction, layers: &[OryxLayer]) -> Option<Described> {
    let code = action.code.as_deref().filter(|c| !c.is_empty() && *c != "KC_NO")?;

    if let (Some(prefix), Some(target)) = (layer_prefix(code), action.layer) {
        let name = layers.iter().find(|l| l.position == target).map(|l| l.title.clone()).unwrap_or_else(|| format!("L{target}"));
        return Some(Described { text: format!("{prefix}{name}"), color: Some(layer_color(target, layers)) });
    }
    if code == "RGB" {
        return Some(Described { text: "●".into(), color: action.color.clone() });
    }
    if code == "KC_TRANSPARENT" || code == "KC_TRNS" {
        return Some(Described { text: "▽".into(), color: None });
    }
    let mods = action.modifiers.as_ref().map(modifier_prefix).unwrap_or_default();
    Some(Described { text: mods + &label(code), color: None })
}

pub fn label(code: &str) -> String {
    if let Some(known) = known_name(code) {
        return known.to_string();
    }
    // Letters, digits and F-keys: KC_A -> A, KC_1 -> 1, KC_F12 -> F12
    if let Some(rest) = code.strip_prefix("KC_") {
        if rest.chars().count() == 1 {
            return rest.to_string();
        }
        if rest.starts_with('F') && rest[1..].parse::<u32>().is_ok() {
            return rest.to_string();
        }
        if let Some(kp) = rest.strip_prefix("KP_") {
            return format!("kp{kp}");
        }
    }
    fallback(code)
}

fn fallback(code: &str) -> String {
    let mut text = code;
    for prefix in ["KC_", "QK_", "MAC_", "RGB_"] {
        if let Some(rest) = text.strip_prefix(prefix) {
            text = rest;
            break;
        }
    }
    let words: Vec<String> = text
        .split('_')
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut chars = w.chars();
            let first: String = chars.next().map(|c| c.to_uppercase().collect()).unwrap_or_default();
            first + &chars.as_str().to_lowercase()
        })
        .collect();
    let joined = words.join(" ");
    if joined.chars().count() > 9 {
        joined.chars().take(8).collect::<String>() + "…"
    } else {
        joined
    }
}

/// Colors used for layer badges and for keys that switch to a layer.
pub const LAYER_COLORS: [&str; 8] = ["#A9B1D6", "#7AA2F7", "#F7768E", "#9ECE6A", "#E0AF68", "#BB9AF7", "#7DCFFF", "#FF9E64"];

pub fn layer_color(position: i64, layers: &[OryxLayer]) -> String {
    if let Some(custom) = layers.iter().find(|l| l.position == position).and_then(|l| l.color.as_deref()) {
        if !custom.is_empty() {
            return custom.to_string();
        }
    }
    LAYER_COLORS[position.rem_euclid(LAYER_COLORS.len() as i64) as usize].to_string()
}

#[rustfmt::skip]
fn known_name(code: &str) -> Option<&'static str> {
    Some(match code {
        "KC_ENTER" => "⏎",
        "KC_ENT" => "⏎",
        "KC_ESCAPE" => "Esc",
        "KC_ESC" => "Esc",
        "KC_BSPC" => "⌫",
        "KC_BACKSPACE" => "⌫",
        "KC_DELETE" => "⌦",
        "KC_DEL" => "⌦",
        "KC_TAB" => "⇥",
        "KC_SPACE" => "␣",
        "KC_SPC" => "␣",
        "KC_CAPS" => "⇪",
        "KC_CAPS_LOCK" => "⇪",
        "KC_INSERT" => "Ins",
        "KC_INS" => "Ins",
        "KC_PSCR" => "PrtSc",
        "KC_PRINT_SCREEN" => "PrtSc",
        "KC_APPLICATION" => "Menu",
        "KC_APP" => "Menu",
        "KC_LEFT" => "←",
        "KC_RIGHT" => "→",
        "KC_UP" => "↑",
        "KC_DOWN" => "↓",
        "KC_HOME" => "Home",
        "KC_END" => "End",
        "KC_PAGE_UP" => "PgUp",
        "KC_PGUP" => "PgUp",
        "KC_PAGE_DOWN" => "PgDn",
        "KC_PGDN" => "PgDn",
        "KC_LEFT_SHIFT" => "⇧",
        "KC_LSFT" => "⇧",
        "KC_RIGHT_SHIFT" => "⇧",
        "KC_RSFT" => "⇧",
        "KC_LEFT_CTRL" => "Ctrl",
        "KC_LCTL" => "Ctrl",
        "KC_RIGHT_CTRL" => "Ctrl",
        "KC_RCTL" => "Ctrl",
        "KC_LEFT_ALT" => "Alt",
        "KC_LALT" => "Alt",
        "KC_RIGHT_ALT" => "Alt",
        "KC_RALT" => "Alt",
        "KC_LEFT_GUI" => GUI,
        "KC_LGUI" => GUI,
        "KC_RIGHT_GUI" => GUI,
        "KC_RGUI" => GUI,
        "KC_HYPR" => "Hyper",
        "KC_MEH" => "Meh",
        "KC_MINUS" => "-",
        "KC_MINS" => "-",
        "KC_EQUAL" => "=",
        "KC_EQL" => "=",
        "KC_LBRC" => "[",
        "KC_LEFT_BRACKET" => "[",
        "KC_RBRC" => "]",
        "KC_RIGHT_BRACKET" => "]",
        "KC_BSLS" => "\\",
        "KC_BACKSLASH" => "\\",
        "KC_SCLN" => ";",
        "KC_SEMICOLON" => ";",
        "KC_QUOTE" => "'",
        "KC_QUOT" => "'",
        "KC_GRAVE" => "`",
        "KC_GRV" => "`",
        "KC_COMMA" => ",",
        "KC_COMM" => ",",
        "KC_DOT" => ".",
        "KC_SLASH" => "/",
        "KC_SLSH" => "/",
        "KC_EXLM" => "!",
        "KC_AT" => "@",
        "KC_HASH" => "#",
        "KC_DLR" => "$",
        "KC_PERC" => "%",
        "KC_CIRC" => "^",
        "KC_AMPR" => "&",
        "KC_ASTR" => "*",
        "KC_LPRN" => "(",
        "KC_RPRN" => ")",
        "KC_UNDS" => "_",
        "KC_PLUS" => "+",
        "KC_LCBR" => "{",
        "KC_RCBR" => "}",
        "KC_PIPE" => "|",
        "KC_COLN" => ":",
        "KC_DQUO" => "\"",
        "KC_DQT" => "\"",
        "KC_TILD" => "~",
        "KC_TILDE" => "~",
        "KC_LABK" => "<",
        "KC_LT" => "<",
        "KC_RABK" => ">",
        "KC_GT" => ">",
        "KC_QUES" => "?",
        "KC_AUDIO_VOL_UP" => "Vol+",
        "KC_VOLU" => "Vol+",
        "KC_AUDIO_VOL_DOWN" => "Vol−",
        "KC_VOLD" => "Vol−",
        "KC_AUDIO_MUTE" => "Mute",
        "KC_MUTE" => "Mute",
        "KC_MEDIA_PLAY_PAUSE" => "⏯",
        "KC_MPLY" => "⏯",
        "KC_MEDIA_NEXT_TRACK" => "⏭",
        "KC_MNXT" => "⏭",
        "KC_MEDIA_PREV_TRACK" => "⏮",
        "KC_MPRV" => "⏮",
        "KC_MEDIA_STOP" => "⏹",
        "KC_MSTP" => "⏹",
        "KC_BRIGHTNESS_UP" => "☀︎+",
        "KC_BRIU" => "☀︎+",
        "KC_BRIGHTNESS_DOWN" => "☀︎−",
        "KC_BRID" => "☀︎−",
        "MAC_MISSION_CONTROL" => "Mission",
        "MAC_LAUNCHPAD" => "Launch",
        "MAC_SPOTLIGHT" => "Spot",
        "MAC_DND" => "DND",
        "MAC_LOCK" => "Lock",
        "MAC_DICTATION" => "Dictate",
        "KC_MS_BTN1" => "Click",
        "KC_BTN1" => "Click",
        "MS_BTN1" => "Click",
        "KC_MS_BTN2" => "R-Click",
        "KC_BTN2" => "R-Click",
        "MS_BTN2" => "R-Click",
        "KC_MS_BTN3" => "M-Click",
        "KC_BTN3" => "M-Click",
        "MS_BTN3" => "M-Click",
        "KC_MS_UP" => "M↑",
        "KC_MS_DOWN" => "M↓",
        "KC_MS_LEFT" => "M←",
        "KC_MS_RIGHT" => "M→",
        "KC_MS_WH_UP" => "W↑",
        "KC_MS_WH_DOWN" => "W↓",
        "KC_MS_WH_LEFT" => "W←",
        "KC_MS_WH_RIGHT" => "W→",
        "KC_MS_ACCEL0" => "Acc0",
        "KC_MS_ACCEL1" => "Acc1",
        "KC_MS_ACCEL2" => "Acc2",
        "DRAG_SCROLL" => "Scroll",
        "TOGGLE_SCROLL" => "Scroll⇄",
        "NAVIGATOR_INC_CPI" => "CPI+",
        "NAVIGATOR_DEC_CPI" => "CPI−",
        "NAVIGATOR_TURBO" => "Turbo",
        "NAVIGATOR_AIM" => "Aim",
        "RGB_TOG" => "RGB",
        "RGB_MODE_FORWARD" => "RGB▸",
        "RGB_MOD" => "RGB▸",
        "RGB_MODE_REVERSE" => "◂RGB",
        "RGB_VAI" => "☼+",
        "RGB_VAD" => "☼−",
        "RGB_HUI" => "Hue+",
        "RGB_HUD" => "Hue−",
        "RGB_SAI" => "Sat+",
        "RGB_SAD" => "Sat−",
        "RGB_SPI" => "Spd+",
        "RGB_SPD" => "Spd−",
        "RGB_SLD" => "Solid",
        "TOGGLE_LAYER_COLOR" => "LyrClr",
        "LED_LEVEL" => "LED",
        "QK_BOOT" => "Boot",
        "RESET" => "Boot",
        "QK_LLCK" => "Lock",
        "QK_LAYER_LOCK" => "Lock",
        "CW_TOGG" => "CapsWd",
        "QK_CAPS_WORD_TOGGLE" => "CapsWd",
        "QK_REP" => "Repeat",
        "QK_AREP" => "AltRep",
        "KC_NO" => "",
        "XXXXXXX" => "",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keycode_labels() {
        assert_eq!(label("KC_A"), "A");
        assert_eq!(label("KC_F12"), "F12");
        assert_eq!(label("KC_BSPC"), "⌫");
        assert_eq!(label("KC_LEFT_CTRL"), "Ctrl");
        assert_eq!(label("KC_LEFT_GUI"), GUI);
        assert_eq!(label("QK_DYNAMIC_TAPPING_TERM_PRINT"), "Dynamic …");
    }

    #[test]
    fn layer_colors_wrap() {
        assert_eq!(layer_color(9, &[]), LAYER_COLORS[1]);
    }
}
