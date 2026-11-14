//! Apply guide (spec 03): a draft's changes as ordered steps in Oryx's terms, and `verify`.

use crate::diff::{diff, Change, Diff, DiffOptions};
use crate::model::Layout;
use crate::positions::PositionMap;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Step {
    /// e.g. "3.2" (layer 3, second change) or "c.1" for combos.
    pub n: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layer: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<String>,
    /// Where the key is, in words.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    pub what: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oryx_hint: Option<String>,
    /// For a step that covers several keys: their indices.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub keys: Vec<usize>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Section {
    pub heading: String,
    pub steps: Vec<Step>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Guide {
    pub draft: String,
    pub base: String,
    pub summary: String,
    /// Things to know before starting.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
    pub sections: Vec<Section>,
    pub finally: Vec<String>,
}

/// How Oryx calls an action type.
fn oryx_hint(action: &str) -> Option<String> {
    let head = action.split('(').next().unwrap_or(action);
    Some(
        match head {
            "MO" => "Oryx: Layer → Momentary (MO)",
            "TG" => "Oryx: Layer → Toggle (TG)",
            "TO" => "Oryx: Layer → Go to (TO)",
            "TT" => "Oryx: Layer → Tap toggle (TT)",
            "OSL" => "Oryx: Layer → One shot (OSL)",
            "DF" => "Oryx: Layer → Set default (DF)",
            "RGB" => "Oryx: Lighting → color",
            "LCTL" | "LSFT" | "LALT" | "LGUI" | "RCTL" | "RSFT" | "RALT" | "RGUI" => "Oryx: pick the key, then tick the modifier(s)",
            "MEH_T" | "ALL_T" => "Oryx: Hold → Meh / Hyper",
            _ => return None,
        }
        .to_string(),
    )
}

fn slot_words(slot: &str) -> &'static str {
    match slot {
        "tap" => "Tap",
        "hold" => "Hold",
        "doubleTap" => "Double tap",
        "tapHold" => "Tap then hold",
        "label" => "Custom label",
        _ => "Key color",
    }
}

fn shown(v: &Option<String>) -> String {
    v.clone().unwrap_or_else(|| "—".into())
}

pub fn guide(base: &Layout, draft: &Layout) -> Guide {
    from_diff(&diff(base, draft, DiffOptions { ignore_metadata: true }), draft)
}

pub fn from_diff(d: &Diff, target: &Layout) -> Guide {
    let map = PositionMap::for_geometry(&target.geometry);
    let mut sections: Vec<Section> = Vec::new();
    let mut layers = Section { heading: "Layers".into(), steps: vec![] };
    let mut removals = Section { heading: "Remove layers (last)".into(), steps: vec![] };
    let mut combos = Section { heading: "Combos".into(), steps: vec![] };
    let count = target.layers.len();
    // Relative wording stays right while layers that get deleted (last) are still in Oryx's list.
    let place = |after: &Option<String>| match after {
        Some(t) => format!("right after \"{t}\""),
        None => "to the top (it becomes the base layer)".to_string(),
    };

    for c in &d.changes {
        match c {
            Change::LayerAdded { position, title, color, after, .. } => {
                let mut what = format!("Add a layer \"{title}\"");
                if let Some(c) = color {
                    what.push_str(&format!(" with color {c}"));
                }
                if (*position as usize) + 1 < count {
                    what.push_str(&format!(", then drag it {} (Oryx adds new layers at the end)", place(after)));
                }
                layers.steps.push(Step {
                    n: String::new(),
                    layer: Some(*position),
                    index: None,
                    position: None,
                    location: None,
                    what,
                    keys: vec![],
                    oryx_hint: Some("Oryx: \"+\" next to the layer list".into()),
                });
            }
            Change::LayerMoved { title, to, after, .. } => layers.steps.push(Step {
                n: String::new(),
                layer: Some(*to),
                index: None,
                position: None,
                location: None,
                what: format!("Drag layer \"{title}\" {} (it ends up as layer {to})", place(after)),
                keys: vec![],
                oryx_hint: Some("Oryx: drag the layer in the layer list; layer keys follow".into()),
            }),
            Change::LayerChanged { position, was, title, field, from, to } => layers.steps.push(Step {
                n: String::new(),
                layer: Some(*position),
                index: None,
                position: None,
                location: None,
                what: if field == "title" {
                    // Renames come first, while Oryx still numbers the layers as in the base:
                    // the number tells layers apart when two share a title for a moment.
                    format!("Rename layer {was} \"{}\" to \"{title}\"", shown(from))
                } else {
                    format!("Layer \"{title}\": change {field} from {} to {}", shown(from), shown(to))
                },
                keys: vec![],
                oryx_hint: Some("Oryx: layer settings".into()),
            }),
            Change::LayerRemoved { position, title } => removals.steps.push(Step {
                n: String::new(),
                layer: Some(*position),
                index: None,
                position: None,
                location: None,
                what: format!("Delete layer \"{title}\" (was position {position})"),
                keys: vec![],
                oryx_hint: Some("Oryx: layer settings → delete".into()),
            }),
            Change::KeyChanged { layer, layer_title, index, position, slot, from, to } => {
                let heading = format!("Layer {layer} \"{layer_title}\"");
                let sec = match sections.iter_mut().find(|s| s.heading == heading) {
                    Some(s) => s,
                    None => {
                        sections.push(Section { heading: heading.clone(), steps: vec![] });
                        sections.last_mut().expect("just pushed")
                    }
                };
                let tap_side = |v: &Option<String>| if slot == "tap" && v.is_none() { "empty (transparent)".to_string() } else { shown(v) };
                let what = format!("{}: {} → {}", slot_words(slot), tap_side(from), tap_side(to));
                sec.steps.push(Step {
                    n: String::new(),
                    layer: Some(*layer),
                    index: Some(*index),
                    position: Some(position.clone()),
                    location: map.as_ref().and_then(|m| m.get(*index)).map(|p| p.describe()),
                    what,
                    keys: vec![],
                    oryx_hint: to.as_deref().and_then(oryx_hint),
                });
            }
            Change::ComboAdded { layer, layer_title, keys, trigger, .. } => combos.steps.push(Step {
                n: String::new(),
                layer: Some(*layer),
                index: None,
                position: Some(keys.join("+")),
                location: None,
                what: format!("Add a combo {} on layer {layer} \"{layer_title}\" → {trigger}", keys.join(" + ")),
                keys: vec![],
                oryx_hint: Some("Oryx: Combos → add".into()),
            }),
            // A combo on a layer that is deleted goes away with the layer: no step of its own.
            Change::ComboRemoved { layer: None, .. } => {}
            Change::ComboRemoved { layer: Some(layer), layer_title, keys, trigger } => combos.steps.push(Step {
                n: String::new(),
                layer: Some(*layer),
                index: None,
                position: Some(keys.join("+")),
                location: None,
                what: format!("Remove the combo {} on layer {layer} \"{layer_title}\" ({trigger})", keys.join(" + ")),
                keys: vec![],
                oryx_hint: Some("Oryx: Combos".into()),
            }),
            Change::ComboChanged { layer, layer_title, keys, from, to } => combos.steps.push(Step {
                n: String::new(),
                layer: Some(*layer),
                index: None,
                position: Some(keys.join("+")),
                location: None,
                what: format!("Combo {} on layer {layer} \"{layer_title}\": {from} → {to}", keys.join(" + ")),
                keys: vec![],
                oryx_hint: Some("Oryx: Combos".into()),
            }),
        }
    }
    // Layer steps: renames first (later steps use the new titles), then adding and dragging in
    // final order, top to bottom, so "right after X" always names a layer that is already in
    // place; other settings last.
    layers.steps.sort_by_key(|st| {
        let group = if st.what.starts_with("Rename layer") {
            0
        } else if st.what.starts_with("Add a layer") || st.what.starts_with("Drag layer") {
            1
        } else {
            2
        };
        (group, st.layer.unwrap_or(0))
    });
    // A layer filled with KC_NO (so nothing falls through) is one step, not one per key.
    for sec in &mut sections {
        const NONE: &str = "Tap: empty (transparent) → KC_NO";
        let same: Vec<usize> = sec.steps.iter().filter(|st| st.what == NONE).filter_map(|st| st.index).collect();
        if same.len() >= 6 {
            let names: Vec<String> = sec.steps.iter().filter(|st| st.what == NONE).filter_map(|st| st.position.clone()).collect();
            let layer = sec.steps.first().and_then(|st| st.layer);
            sec.steps.retain(|st| st.what != NONE);
            sec.steps.push(Step {
                n: String::new(),
                layer,
                index: None,
                position: None,
                location: None,
                what: format!(
                    "Set the other {} keys to None (KC_NO), so nothing falls through from the base layer: {}",
                    same.len(),
                    names.join(", ")
                ),
                keys: same,
                oryx_hint: Some("Oryx: click the key → None (the empty action, not Transparent)".into()),
            });
        }
    }
    // Then key changes by layer, combos, removals.
    sections.sort_by_key(|s| s.steps.first().and_then(|st| st.layer).unwrap_or(0));
    let mut all = Vec::new();
    if !layers.steps.is_empty() {
        all.push(layers);
    }
    all.extend(sections);
    if !combos.steps.is_empty() {
        all.push(combos);
    }
    if !removals.steps.is_empty() {
        all.push(removals);
    }
    for (si, sec) in all.iter_mut().enumerate() {
        let prefix = match sec.heading.as_str() {
            "Layers" => "L".to_string(),
            "Combos" => "c".to_string(),
            h if h.starts_with("Remove") => "r".to_string(),
            _ => sec.steps.first().and_then(|s| s.layer).map(|l| l.to_string()).unwrap_or_else(|| si.to_string()),
        };
        for (i, st) in sec.steps.iter_mut().enumerate() {
            st.n = format!("{prefix}.{}", i + 1);
        }
    }
    let mut notes = Vec::new();
    if d.changes.iter().any(|c| matches!(c, Change::LayerAdded { .. } | Change::LayerRemoved { .. } | Change::LayerMoved { .. })) {
        notes.push(
            "Layer numbers are the final ones. While you work, Oryx's numbers can differ (deleted layers go last): go by the layer titles."
                .to_string(),
        );
    }
    Guide {
        draft: d.to.clone(),
        base: d.from.clone(),
        summary: d.summary.clone(),
        notes,
        sections: all,
        finally: vec![
            "Compile the layout in Oryx and flash it (Keymapp or Oryx's flash button).".into(),
            format!(
                "Then run: keyloom import {} && keyloom verify {} (agents: import_layout, then verify_draft)",
                target.layout_id,
                target.reference()
            ),
        ],
    }
}

pub fn render(g: &Guide) -> String {
    let mut out = format!("Apply {} (based on {}): {}\n", g.draft, g.base, g.summary);
    if g.sections.is_empty() {
        out.push_str(&format!("\nNothing to do: {} already has everything in {}.\n", g.base, g.draft));
        return out;
    }
    for n in &g.notes {
        out.push_str(&format!("Note: {n}\n"));
    }
    for s in &g.sections {
        out.push_str(&format!("\n{}\n", s.heading));
        for st in &s.steps {
            let loc = match (&st.position, &st.location) {
                (Some(p), Some(l)) => format!("{p:<10} ({l})  "),
                (Some(p), None) => format!("{p:<10} "),
                _ => String::new(),
            };
            out.push_str(&format!("  {:<5} {loc}{}\n", st.n, st.what));
            if let Some(h) = &st.oryx_hint {
                out.push_str(&format!("        {h}\n"));
            }
        }
    }
    out.push('\n');
    for f in &g.finally {
        out.push_str(&format!("→ {f}\n"));
    }
    out
}

// MARK: verify

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Verification {
    pub draft: String,
    pub against: String,
    pub matches: bool,
    /// Draft changes not (correctly) applied yet.
    pub still_to_do: Vec<Change>,
    /// Differences the draft didn't ask for (changed in Oryx, or a mistake).
    pub unexpected: Vec<Change>,
}

fn change_key(c: &Change) -> String {
    match c {
        Change::KeyChanged { layer_title, index, slot, .. } => format!("key:{}:{index}:{slot}", layer_title.to_lowercase()),
        Change::LayerAdded { title, .. } | Change::LayerRemoved { title, .. } | Change::LayerMoved { title, .. } => {
            format!("layer:{}", title.to_lowercase())
        }
        Change::LayerChanged { title, field, .. } => format!("layer:{}:{field}", title.to_lowercase()),
        Change::ComboAdded { layer_title, keys, .. }
        | Change::ComboRemoved { layer_title, keys, .. }
        | Change::ComboChanged { layer_title, keys, .. } => format!("combo:{}:{}", layer_title.to_lowercase(), keys.join("+")),
    }
}

/// Compares a draft with the revision imported after applying it.
pub fn verify(base: &Layout, draft: &Layout, imported: &Layout) -> Verification {
    let intended = diff(base, draft, DiffOptions { ignore_metadata: true });
    let remaining = diff(imported, draft, DiffOptions { ignore_metadata: true });
    let mut intended_keys: Vec<String> = intended.changes.iter().map(change_key).collect();
    // A layer the draft adds comes with its title and color: if the layer exists in the
    // revision but one of those isn't right yet, that is still part of what the draft asked for.
    for c in &intended.changes {
        if let Change::LayerAdded { title, .. } = c {
            for field in ["title", "color"] {
                intended_keys.push(format!("layer:{}:{field}", title.to_lowercase()));
            }
        }
    }
    // If the draft changes the layer order (adds or moves a layer), a remaining order
    // difference is the draft's doing, whichever layer the diff happens to name as moved.
    let reorders = intended.changes.iter().any(|c| matches!(c, Change::LayerAdded { .. } | Change::LayerMoved { .. }));
    let (still_to_do, unexpected): (Vec<Change>, Vec<Change>) = remaining
        .changes
        .into_iter()
        .partition(|c| intended_keys.contains(&change_key(c)) || (reorders && matches!(c, Change::LayerMoved { .. })));
    Verification {
        draft: draft.reference(),
        against: imported.reference(),
        matches: still_to_do.is_empty() && unexpected.is_empty(),
        still_to_do,
        unexpected,
    }
}
