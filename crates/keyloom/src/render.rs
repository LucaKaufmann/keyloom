//! Layer views: a structured model (for `--json` and MCP) and the ASCII board (spec 04).

use crate::model::{Key, Layout};
use crate::positions::{display_name, PositionMap};
use crate::qmk;
use serde::Serialize;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    /// Readable legends (`⌫`, `Sym+Num`), hold action on the second line.
    Legend,
    /// QMK-style expressions (`LT(1, KC_SPACE)`), shortened to fit.
    Codes,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyView {
    pub index: usize,
    pub position: String,
    /// QMK-style expression of the whole key.
    pub qmk: String,
    /// QMK-style action per slot.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tap: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hold: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub double_tap: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tap_hold: Option<String>,
    pub legend: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sub: Option<String>,
    /// Falls through to the layer below (shown with the base layer's legend).
    pub transparent: bool,
    /// Does nothing (`KC_NO`).
    pub empty: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_label: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LayerView {
    pub reference: String,
    pub layout_title: String,
    pub geometry: String,
    pub layer: i64,
    pub title: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    pub keys: Vec<KeyView>,
}

pub fn layer_view(layout: &Layout, position: i64) -> Option<LayerView> {
    let layer = layout.layer(position)?;
    let map = PositionMap::for_geometry(&layout.geometry);
    let vg = layout.legend_layers();
    let base = layout.layers.first();
    let keys = layer
        .keys
        .iter()
        .enumerate()
        .map(|(i, key)| {
            let transparent = position != 0 && key.is_transparent();
            // Show what you'd get: the base layer's key (an approximation when other layers are
            // also active, the usual on-screen approximation).
            let shown: &Key = if transparent { base.and_then(|b| b.keys.get(i)).unwrap_or(key) } else { key };
            let shown_vg = serde_json::to_value(shown).ok().and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
            let legend = crate::labels::legend(&shown_vg, &vg);
            let slot = |a: &Option<crate::model::Action>| a.as_ref().and_then(qmk::action);
            KeyView {
                index: i,
                position: display_name(map.as_ref(), i),
                qmk: qmk::key(key),
                tap: slot(&key.tap),
                hold: slot(&key.hold),
                double_tap: slot(&key.double_tap),
                tap_hold: slot(&key.tap_hold),
                legend: legend.main,
                sub: legend.sub.map(|s| nicer_sub(shown, s)),
                transparent,
                empty: key.is_no(),
                custom_label: key.custom_label.clone(),
            }
        })
        .collect();
    Some(LayerView {
        reference: layout.reference(),
        layout_title: layout.title.clone(),
        geometry: layout.geometry.clone(),
        layer: layer.position,
        title: layer.title.clone(),
        tags: layer.keyloom.tags.clone(),
        keys,
    })
}

// MARK: ASCII

pub struct AsciiOptions {
    pub style: Style,
    /// ANSI dim for transparent keys.
    pub color: bool,
    /// Inner cell width in terminal columns.
    pub cell: usize,
}

impl Default for AsciiOptions {
    fn default() -> Self {
        Self { style: Style::Legend, color: false, cell: 7 }
    }
}

/// Fits `s` into `w` columns, centered; truncated with `…` when too long.
fn fit(s: &str, w: usize) -> String {
    // A label with a line break or tab (from a hand-edited file) must not break the grid.
    let cleaned: String = s.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    let s = cleaned.as_str();
    let s = if s.width() > w {
        let mut out = String::new();
        let mut used = 0;
        for c in s.chars() {
            let cw = c.width().unwrap_or(0);
            if used + cw + 1 > w {
                break;
            }
            out.push(c);
            used += cw;
        }
        out.push('…');
        out
    } else {
        s.to_string()
    };
    let pad = w - s.width();
    let left = pad / 2;
    format!("{}{}{}", " ".repeat(left), s, " ".repeat(pad - left))
}

/// `labels` labels `MEH_T`/`ALL_T` holds as "Meh T"/"All T".
fn nicer_sub(key: &Key, sub: String) -> String {
    match key.hold.as_ref().and_then(|a| a.code()) {
        Some("MEH_T") => "Meh".into(),
        Some("ALL_T") => "Hyper".into(),
        _ => sub,
    }
}

/// QMK's short aliases for the long names Oryx uses.
const SHORT: &[(&str, &str)] = &[
    ("LEFT_SHIFT", "LSFT"),
    ("RIGHT_SHIFT", "RSFT"),
    ("LEFT_CTRL", "LCTL"),
    ("RIGHT_CTRL", "RCTL"),
    ("LEFT_ALT", "LALT"),
    ("RIGHT_ALT", "RALT"),
    ("LEFT_GUI", "LGUI"),
    ("RIGHT_GUI", "RGUI"),
    ("TRANSPARENT", "▽"),
    ("ESCAPE", "ESC"),
    ("SPACE", "SPC"),
    ("ENTER", "ENT"),
    ("EQUAL", "EQL"),
    ("QUOTE", "QUOT"),
    ("COMMA", "COMM"),
    ("SLASH", "SLSH"),
    ("MINUS", "MINS"),
    ("GRAVE", "GRV"),
    ("DELETE", "DEL"),
    ("BSPACE", "BSPC"),
    ("AUDIO_VOL_UP", "VOLU"),
    ("AUDIO_VOL_DOWN", "VOLD"),
    ("AUDIO_MUTE", "MUTE"),
    ("BRIGHTNESS_UP", "BRIU"),
    ("BRIGHTNESS_DOWN", "BRID"),
    ("MEDIA_PLAY_PAUSE", "MPLY"),
    ("MEDIA_NEXT_TRACK", "MNXT"),
    ("MEDIA_PREV_TRACK", "MPRV"),
    ("MEDIA_STOP", "MSTP"),
    ("RGB_MODE_FORWARD", "RGB_MOD"),
    ("TOGGLE_LAYER_COLOR", "TGLCLR"),
    ("NAVIGATOR_INC_CPI", "CPI+"),
    ("NAVIGATOR_DEC_CPI", "CPI-"),
    ("DRAG_SCROLL", "DRGSCR"),
    ("TOGGLE_SCROLL", "TGLSCR"),
    ("MS_WH_UP", "Whl↑"),
    ("MS_WH_DOWN", "Whl↓"),
    ("MS_WH_LEFT", "Whl←"),
    ("MS_WH_RIGHT", "Whl→"),
    ("MS_ACCEL", "MsAcc"),
    ("MS_UP", "Ms↑"),
    ("MS_DOWN", "Ms↓"),
    ("MS_LEFT", "Ms←"),
    ("MS_RIGHT", "Ms→"),
];

/// Short form of a QMK action for a cell: drops `KC_`, uses short aliases.
fn short_code(qmk: &str) -> String {
    let mut s = qmk.replace("KC_", "").replace(", ", ",");
    for (long, short) in SHORT {
        s = s.replace(long, short);
    }
    s
}

/// The two text lines of a cell.
fn cell_lines(k: &KeyView, style: Style) -> (String, String) {
    match style {
        Style::Legend => {
            // A transparent key shows what it falls through to, hold action included.
            let sub = match (&k.sub, k.transparent) {
                (Some(s), true) => format!("▽{s}"),
                (None, true) => "▽".to_string(),
                (s, false) => s.clone().unwrap_or_default(),
            };
            (k.legend.clone(), sub)
        }
        Style::Codes => {
            // Line 1: tap (or the whole key if it has no tap); line 2: hold, else 2× double tap.
            let short = |s: &Option<String>| s.as_deref().map(short_code);
            if k.transparent {
                return ("▽".into(), String::new());
            }
            let line1 = short(&k.tap).or_else(|| short(&k.hold)).unwrap_or_else(|| short_code(&k.qmk));
            let line2 = if k.tap.is_some() { short(&k.hold) } else { None }
                .or_else(|| short(&k.double_tap).map(|d| format!("2×{d}")))
                .or_else(|| short(&k.tap_hold).map(|t| format!("TH:{t}")))
                .unwrap_or_default();
            (line1, line2)
        }
    }
}

/// A box table: `rows` × `cols`, each cell two lines. `None` cells are drawn as blank space.
fn table(cells: &[Vec<(String, String, bool)>], w: usize, color: bool) -> Vec<String> {
    let cols = cells.first().map(Vec::len).unwrap_or(0);
    let rule = |l: &str, m: &str, r: &str| format!("{l}{}{r}", vec!["─".repeat(w); cols].join(m));
    let mut out = vec![rule("┌", "┬", "┐")];
    for (ri, row) in cells.iter().enumerate() {
        for line in 0..2 {
            let mut s = String::from("│");
            for (a, b, dim) in row {
                let text = fit(if line == 0 { a } else { b }, w);
                if *dim && color {
                    s.push_str(&format!("\x1b[2m{text}\x1b[0m"));
                } else {
                    s.push_str(&text);
                }
                s.push('│');
            }
            out.push(s);
        }
        out.push(if ri + 1 == cells.len() { rule("└", "┴", "┘") } else { rule("├", "┼", "┤") });
    }
    out
}

/// Renders one layer of a Voyager-shaped board. Other geometries get a plain key list until
/// their position maps exist.
pub fn ascii(view: &LayerView, opts: &AsciiOptions) -> String {
    let mut out = String::new();
    let tags = if view.tags.is_empty() { String::new() } else { format!(" [{}]", view.tags.join(", ")) };
    out.push_str(&format!("{} · {} · layer {} \"{}\"{}\n", view.layout_title, view.reference, view.layer, view.title, tags));
    let cell = |i: usize| -> (String, String, bool) {
        let k = &view.keys[i];
        let (a, b) = cell_lines(k, opts.style);
        (a, b, k.transparent)
    };
    if view.geometry != "voyager" || view.keys.len() != 52 {
        for k in &view.keys {
            out.push_str(&format!("{:>10}  {}\n", k.position, k.qmk));
        }
        return out;
    }
    let w = opts.cell;
    let left: Vec<Vec<_>> = (0..4).map(|r| (0..6).map(|c| cell(r * 6 + c)).collect()).collect();
    let right: Vec<Vec<_>> = (0..4).map(|r| (0..6).map(|c| cell(26 + r * 6 + c)).collect()).collect();
    let gap = "   ";
    for (l, r) in table(&left, w, opts.color).iter().zip(table(&right, w, opts.color)) {
        out.push_str(&format!("{l}{gap}{r}\n"));
    }
    // Thumbs: left #24 #25 under the inner columns of the left half, right #50 #51 under the
    // inner columns of the right half, as on the board.
    let half_width = 6 * (w + 1) + 1;
    let lt = table(&[vec![cell(24), cell(25)]], w, opts.color);
    let rt = table(&[vec![cell(50), cell(51)]], w, opts.color);
    let lt_indent = half_width - (2 * (w + 1) + 1);
    for (l, r) in lt.iter().zip(rt) {
        out.push_str(&format!("{}{l}{gap}{r}\n", " ".repeat(lt_indent)));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fitting() {
        assert_eq!(fit("A", 5), "  A  ");
        assert_eq!(fit("Sym+Num", 7), "Sym+Num");
        assert_eq!(fit("Brightness", 7), "Bright…");
        assert_eq!(fit("⌫", 3), " ⌫ ");
    }
}
