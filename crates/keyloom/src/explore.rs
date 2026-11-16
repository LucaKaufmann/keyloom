//! Explore queries (spec 02): key info, find, free keys, layer graph, layout summary.

use crate::knowledge;
use crate::model::{Action, Key, Layout, Slot};
use crate::positions::{display_name, Finger, PositionMap};
use crate::qmk;
use serde::Serialize;
use std::collections::BTreeSet;

fn positions(layout: &Layout) -> Option<PositionMap> {
    PositionMap::for_geometry(&layout.geometry)
}

fn legend_of(layout: &Layout, key: &Key) -> String {
    let vg = layout.legend_layers();
    let k = serde_json::to_value(key).ok().and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
    let l = crate::labels::legend(&k, &vg);
    match l.sub {
        Some(sub) if !l.main.is_empty() => format!("{} / {sub}", l.main),
        Some(sub) => sub,
        None => l.main,
    }
}

// MARK: key info

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyInfo {
    pub index: usize,
    pub position: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finger: Option<Finger>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<f32>,
    pub layers: Vec<KeyOnLayer>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyOnLayer {
    pub layer: i64,
    pub title: String,
    pub qmk: String,
    pub legend: String,
    pub transparent: bool,
    /// When transparent: the layer whose key acts instead.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_from: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_qmk: Option<String>,
}

pub fn key_info(layout: &Layout, index: usize, layer: Option<i64>) -> KeyInfo {
    let map = positions(layout);
    let pos = map.as_ref().and_then(|m| m.get(index)).cloned();
    let layers = layout
        .layers
        .iter()
        .filter(|l| layer.is_none_or(|n| n == l.position))
        .filter_map(|l| {
            let key = l.keys.get(index)?;
            let transparent = l.position != 0 && key.is_transparent();
            let (eff, from) = layout.effective_key(l.position, index)?;
            Some(KeyOnLayer {
                layer: l.position,
                title: l.title.clone(),
                qmk: qmk::key(key),
                legend: legend_of(layout, eff),
                transparent,
                resolved_from: transparent.then_some(from),
                resolved_qmk: transparent.then(|| qmk::key(eff)),
            })
        })
        .collect();
    KeyInfo {
        index,
        position: display_name(map.as_ref(), index),
        description: pos.as_ref().map(|p| p.describe()),
        finger: pos.as_ref().map(|p| p.finger),
        effort: pos.as_ref().map(|p| p.effort),
        layers,
    }
}

// MARK: find

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Hit {
    pub layer: i64,
    pub title: String,
    pub index: usize,
    pub position: String,
    /// `tap`, `hold`, `doubleTap`, `tapHold`, `label` or `combo`.
    pub slot: String,
    pub qmk: String,
    /// Layer changes needed from the base layer, e.g. "hold L.thumb.2 (MO Sym+Num)".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reach: Option<String>,
}

fn slot_name(s: Slot) -> &'static str {
    match s {
        Slot::Tap => "tap",
        Slot::Hold => "hold",
        Slot::DoubleTap => "doubleTap",
        Slot::TapHold => "tapHold",
    }
}

/// Parses `MO(3)`/`TG(Gaming)` style queries: (code, layer).
fn layer_query(layout: &Layout, q: &str) -> Option<(String, i64)> {
    let (head, rest) = q.split_once('(')?;
    let arg = rest.strip_suffix(')')?.trim();
    // Numbers as written (also for layers that don't exist); titles like everywhere else.
    let n = arg.parse::<i64>().ok().or_else(|| crate::ops::resolve_layer_str(layout, arg).ok())?;
    Some((head.trim().to_uppercase(), n))
}

fn action_matches(layout: &Layout, a: &Action, query: &str, code: Option<&str>) -> bool {
    let Some(ac) = a.code() else { return false };
    if let Some((lc, ln)) = layer_query(layout, query) {
        return ac.eq_ignore_ascii_case(&lc) && a.layer == Some(ln);
    }
    match code {
        Some(c) => knowledge::same_code(ac, c),
        None => false,
    }
}

/// Where a keycode, layer action (`MO(1)`, `TG(Gaming)`) or label lives. Text that isn't a known
/// keycode matches custom labels and legends.
pub fn find(layout: &Layout, query: &str) -> Vec<Hit> {
    let map = positions(layout);
    let q = query.trim();
    // `find LT` / `find MT`: every key of that kind (Oryx has no such code; it's tap + hold).
    let family = ["LT", "MT", "TAP_HOLD", "TD"].iter().find(|f| f.eq_ignore_ascii_case(q));
    if let Some(family) = family {
        let graph = layer_graph(layout);
        let prefix = format!("{family}(");
        return layout
            .layers
            .iter()
            .flat_map(|layer| layer.keys.iter().enumerate().map(move |(i, key)| (layer, i, key)))
            .filter(|(_, _, key)| qmk::key(key).starts_with(&prefix) || (*family == "MT" && qmk::key(key).contains("_T(")))
            .map(|(layer, i, key)| Hit {
                layer: layer.position,
                title: layer.title.clone(),
                index: i,
                position: display_name(map.as_ref(), i),
                slot: "hold".into(),
                qmk: qmk::key(key),
                reach: graph.how_to_reach(layer.position),
            })
            .collect();
    }
    // A whole-key or modified-key expression as keyloom prints it: `LT(1, KC_SPACE)`,
    // `MT(MOD_LCTL, KC_TAB)`, `LCTL(KC_C)`, `TAP_HOLD(KC_EQUAL, KC_ESCAPE)`.
    if q.contains('(') && layer_query(layout, q).is_none() {
        let spec = q.replace("MOD_", "");
        if let Ok(wanted) = crate::ops::parse_binding(layout, &spec) {
            let target = qmk::key(&wanted.key);
            let graph = layer_graph(layout);
            let mut hits = Vec::new();
            for layer in &layout.layers {
                for (i, key) in layer.keys.iter().enumerate() {
                    if layer.position != 0 && key.is_transparent() {
                        continue;
                    }
                    let whole = qmk::key(key) == target;
                    let slot = key.slots().into_iter().find(|(_, a)| qmk::action(a).as_deref() == Some(target.as_str()));
                    if whole || slot.is_some() {
                        hits.push(Hit {
                            layer: layer.position,
                            title: layer.title.clone(),
                            index: i,
                            position: display_name(map.as_ref(), i),
                            slot: if whole { "key".into() } else { slot_name(slot.expect("checked").0).into() },
                            qmk: qmk::key(key),
                            reach: graph.how_to_reach(layer.position),
                        });
                    }
                }
            }
            return hits;
        }
    }
    // "Alt", "Shift", "Ctrl", "Cmd": either hand.
    let either_hand: Option<[&str; 2]> = match q.to_lowercase().as_str() {
        "alt" | "option" | "opt" => Some(["KC_LEFT_ALT", "KC_RIGHT_ALT"]),
        "shift" => Some(["KC_LEFT_SHIFT", "KC_RIGHT_SHIFT"]),
        "ctrl" | "control" => Some(["KC_LEFT_CTRL", "KC_RIGHT_CTRL"]),
        "gui" | "cmd" | "command" | "win" | "super" => Some(["KC_LEFT_GUI", "KC_RIGHT_GUI"]),
        _ => None,
    };
    if let Some(codes) = either_hand {
        let mut hits: Vec<Hit> = codes.iter().flat_map(|c| find(layout, c)).collect();
        hits.sort_by_key(|h| (h.layer, h.index));
        return hits;
    }
    let code = knowledge::canonical(q).or_else(|| {
        let up = q.to_uppercase();
        (up.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') && up.contains('_')).then_some(up)
    });
    let graph = layer_graph(layout);
    let mut hits = Vec::new();
    for layer in &layout.layers {
        for (i, key) in layer.keys.iter().enumerate() {
            let mut push = |slot: &str| {
                hits.push(Hit {
                    layer: layer.position,
                    title: layer.title.clone(),
                    index: i,
                    position: display_name(map.as_ref(), i),
                    slot: slot.to_string(),
                    qmk: qmk::key(key),
                    reach: graph.how_to_reach(layer.position),
                })
            };
            let mut matched = false;
            for (slot, a) in key.slots() {
                if action_matches(layout, a, q, code.as_deref()) {
                    push(slot_name(slot));
                    matched = true;
                }
            }
            if !matched && code.is_none() && layer_query(layout, q).is_none() {
                let label = key.custom_label.as_deref().unwrap_or("");
                if !label.is_empty() && label.eq_ignore_ascii_case(q) {
                    push("label");
                } else if !(layer.position != 0 && key.is_transparent()) && legend_of(layout, key).split(" / ").any(|l| l == q) {
                    push("tap");
                }
            }
        }
    }
    for combo in &layout.combos {
        let trigger: Option<Action> = serde_json::from_value(combo["trigger"].clone()).ok();
        if let Some(t) = trigger.filter(|t| action_matches(layout, t, q, code.as_deref())) {
            let layer = combo["layerIdx"].as_i64().unwrap_or(0);
            let keys: Vec<String> = combo["keyIndices"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|v| v.as_u64())
                .map(|i| display_name(map.as_ref(), i as usize))
                .collect();
            hits.push(Hit {
                layer,
                title: layout.layer_title(layer),
                index: combo["keyIndices"][0].as_u64().unwrap_or(0) as usize,
                position: keys.join("+"),
                slot: "combo".into(),
                qmk: qmk::action(&t).unwrap_or_default(),
                reach: graph.how_to_reach(layer),
            });
        }
    }
    hits
}

// MARK: free keys

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FreeKey {
    pub index: usize,
    pub position: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finger: Option<Finger>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<f32>,
    /// `transparent` (falls through; shows `fallsThrough`) or `no` (KC_NO) or `blank` (base layer).
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub falls_through_to: Option<String>,
}

/// Keys that are free to assign on a layer, easiest first.
pub fn free_keys(layout: &Layout, layer: i64) -> Vec<FreeKey> {
    let map = positions(layout);
    let Some(l) = layout.layer(layer) else { return vec![] };
    let mut out: Vec<FreeKey> = l
        .keys
        .iter()
        .enumerate()
        .filter_map(|(i, key)| {
            let kind = if key.is_no() {
                "no"
            } else if key.is_transparent() {
                if layer == 0 {
                    "blank"
                } else {
                    "transparent"
                }
            } else {
                return None;
            };
            let pos = map.as_ref().and_then(|m| m.get(i));
            let falls = (kind == "transparent").then(|| layout.effective_key(layer, i).map(|(k, _)| qmk::key(k))).flatten();
            Some(FreeKey {
                index: i,
                position: display_name(map.as_ref(), i),
                finger: pos.map(|p| p.finger),
                effort: pos.map(|p| p.effort),
                kind: kind.into(),
                falls_through_to: falls,
            })
        })
        .collect();
    out.sort_by(|a, b| a.effort.unwrap_or(1.0).total_cmp(&b.effort.unwrap_or(1.0)).then(a.index.cmp(&b.index)));
    out
}

// MARK: layer graph

/// A key Oryx's editor clears, because a lower layer has a layer key to this layer at the same
/// position.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OryxCleared {
    pub layer: i64,
    pub index: usize,
    /// The lower layer whose key reserves this position.
    pub by_layer: i64,
    /// That key's layer action, e.g. `TG`.
    pub action: String,
}

/// Layer actions that make Oryx keep the same position on the target layer transparent. Read from
/// the Oryx editor's code: a key on layer N is "preceded" when a lower layer has a layer key to N at
/// the same position (keycodes whose layer parameter is `forwardLayers`, except `OSL`), and the
/// editor clears a preceded key as soon as it shows it, so that the key falls through to the layer
/// key below. `MO`, `LM` and `TT` stop counting when the layout's `ambidexLT` option is on. Whether
/// `TO` and `DF` count wasn't checked, so they're left out.
const ORYX_PRECEDING: &[&str] = &["TG", "TT", "MO", "LM"];

/// Keys that Oryx's editor turns transparent the next time it shows their layer (see
/// `ORYX_PRECEDING`). A draft can hold such keys, and Oryx's API stores them, but they don't survive
/// opening the layout in Oryx, which is where it gets compiled.
pub fn oryx_cleared(layout: &Layout) -> Vec<OryxCleared> {
    let ambidex = layout.oryx.revision.get("config").and_then(|c| c.get("ambidexLT")).and_then(|v| v.as_bool()) == Some(true);
    let mut out = Vec::new();
    for layer in layout.layers.iter().filter(|l| l.position != 0) {
        for (index, key) in layer.keys.iter().enumerate() {
            if key.is_transparent() {
                continue;
            }
            let by = layout.layers.iter().filter(|l| l.position < layer.position).find_map(|lower| {
                let k = lower.keys.get(index)?;
                k.slots().into_iter().find_map(|(_, a)| {
                    let code = a.code()?;
                    let counts = ORYX_PRECEDING.contains(&code) && !(ambidex && matches!(code, "MO" | "LM" | "TT"));
                    (counts && a.layer == Some(layer.position)).then(|| (lower.position, code.to_string()))
                })
            });
            if let Some((by_layer, action)) = by {
                out.push(OryxCleared { layer: layer.position, index, by_layer, action });
            }
        }
    }
    out
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Edge {
    pub from: i64,
    pub to: i64,
    /// MO, TG, TO, TT, OSL, DF.
    pub action: String,
    /// The key (for a combo: its first key).
    pub index: usize,
    /// Position name; for a combo all its keys joined with `+`.
    pub position: String,
    pub slot: Slot,
    /// The edge works by falling through a transparent key on `from`.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub via_transparent: bool,
    /// The edge is a combo's trigger, not a key.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub combo: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LayerNode {
    pub position: i64,
    pub title: String,
    pub reachable: bool,
    /// Entered only while a key is held (MO, LT, OSL), so leaving is automatic.
    pub momentary_only: bool,
    /// Entered by the Navigator trackball (auto-mouse setting from Oryx).
    pub auto_mouse: bool,
    /// Entered by the firmware rather than a key: auto-mouse, or tagged `auto`.
    pub firmware_entered: bool,
    /// Some key switches to this layer and stays there: TG, TO or TT.
    pub entered_by_toggle: bool,
    /// Some key makes this the default layer: DF.
    pub entered_by_default: bool,
    /// Keys on (or falling through to) this layer that switch it off: TG/TT(this layer), TO(other).
    pub exits: Vec<String>,
    /// Keys that change the default layer away from this one: DF(other). TG/TO don't do that.
    pub default_exits: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LayerGraph {
    pub edges: Vec<Edge>,
    pub layers: Vec<LayerNode>,
}

impl LayerGraph {
    /// Shortest description of how to get to a layer from the base layer.
    pub fn how_to_reach(&self, layer: i64) -> Option<String> {
        if layer == 0 {
            return None;
        }
        let node = self.layers.iter().find(|l| l.position == layer);
        if node.is_some_and(|l| l.auto_mouse) {
            return Some("move the trackball (auto-mouse layer)".into());
        }
        let edge = self.edges.iter().filter(|e| e.to == layer && e.from != layer).min_by_key(|e| (e.from != 0, e.via_transparent, e.combo));
        let Some(edge) = edge else {
            return node.filter(|l| l.firmware_entered).map(|_| "entered by the firmware (tagged auto)".to_string());
        };
        let from = if edge.from == 0 { String::new() } else { format!(" on layer {}", edge.from) };
        if edge.combo {
            return Some(format!("press {} together{from} (combo, {}({}))", edge.position, edge.action, edge.to));
        }
        let verb = match edge.slot {
            Slot::Hold => "hold",
            Slot::DoubleTap => "double-tap",
            Slot::TapHold => "tap-then-hold",
            Slot::Tap => "tap",
        };
        Some(format!("{verb} {}{from} ({}({}))", edge.position, edge.action, edge.to))
    }
}

const LAYER_CODES: &[&str] = &["MO", "TG", "TO", "TT", "OSL", "DF", "LM"];

/// Tag for layers the firmware enters by itself (e.g. an auto-mouse layer keyloom couldn't detect).
pub const AUTO_TAG: &str = "auto";

pub fn layer_graph(layout: &Layout) -> LayerGraph {
    let map = positions(layout);
    let mut edges = Vec::new();
    // A key that targets its own layer only matters when it toggles: TG(self), TT(self).
    let relevant = |code: &str, from: i64, to: i64| LAYER_CODES.contains(&code) && (to != from || matches!(code, "TG" | "TT"));
    // Keys Oryx's editor will clear count as what they become: transparent.
    let cleared: BTreeSet<(i64, usize)> = oryx_cleared(layout).iter().map(|c| (c.layer, c.index)).collect();
    let transparent =
        |l: &crate::model::Layer, i: usize| cleared.contains(&(l.position, i)) || l.keys.get(i).is_some_and(Key::is_transparent);
    for layer in &layout.layers {
        for i in 0..layer.keys.len() {
            let resolved = if cleared.contains(&(layer.position, i)) {
                layout.layers.first().and_then(|base| base.keys.get(i).map(|k| (k, base.position)))
            } else {
                layout.effective_key(layer.position, i)
            };
            let Some((key, from)) = resolved else { continue };
            for (slot, a) in key.slots() {
                let (Some(code), Some(to)) = (a.code(), a.layer) else { continue };
                if !relevant(code, layer.position, to) {
                    continue;
                }
                edges.push(Edge {
                    from: layer.position,
                    to,
                    action: code.into(),
                    index: i,
                    position: display_name(map.as_ref(), i),
                    slot,
                    via_transparent: from != layer.position,
                    combo: false,
                });
            }
        }
    }
    // Combos whose trigger is a layer action, on their own layer only (how Oryx scopes combos
    // across layers is unverified, so nothing is assumed about other layers).
    for c in &layout.combos {
        let from = c["layerIdx"].as_i64().unwrap_or(0);
        let Ok(trigger) = serde_json::from_value::<Action>(c["trigger"].clone()) else { continue };
        let (Some(code), Some(to)) = (trigger.code(), trigger.layer) else { continue };
        if !relevant(code, from, to) {
            continue;
        }
        let keys = crate::ops::sorted_keys(c);
        edges.push(Edge {
            from,
            to,
            action: code.into(),
            index: keys.first().copied().unwrap_or(0) as usize,
            position: keys.iter().map(|&i| display_name(map.as_ref(), i as usize)).collect::<Vec<_>>().join("+"),
            slot: Slot::Tap,
            via_transparent: false,
            combo: true,
        });
    }
    // Reachability from the base layer and from layers the firmware enters by itself.
    let auto = layout.settings.auto_mouse_layer;
    let firmware: BTreeSet<i64> = layout
        .layers
        .iter()
        .filter(|l| auto == Some(l.position) || l.keyloom.tags.iter().any(|t| t.trim().eq_ignore_ascii_case(AUTO_TAG)))
        .map(|l| l.position)
        .collect();
    let mut reachable: BTreeSet<i64> = [0].into_iter().chain(firmware.iter().copied()).collect();
    loop {
        let before = reachable.len();
        for e in &edges {
            if reachable.contains(&e.from) && e.to != e.from {
                reachable.insert(e.to);
            }
        }
        if reachable.len() == before {
            break;
        }
    }
    let layers = layout
        .layers
        .iter()
        .map(|l| {
            let incoming: Vec<&Edge> = edges.iter().filter(|e| e.to == l.position && e.from != l.position).collect();
            let momentary_only = !incoming.is_empty() && incoming.iter().all(|e| matches!(e.action.as_str(), "MO" | "OSL" | "LM"));
            // Keys within reach while on this layer: its own, and those of layers that can be held
            // from here (MO/LT/OSL), e.g. a toggle key that lives on a momentary layer.
            let mut within: BTreeSet<i64> = [l.position].into();
            loop {
                let before = within.len();
                for e in &edges {
                    let usable = e.from == l.position || !e.via_transparent;
                    if within.contains(&e.from) && usable && matches!(e.action.as_str(), "MO" | "OSL" | "LM") {
                        within.insert(e.to);
                    }
                }
                if within.len() == before {
                    break;
                }
            }
            let describe = |e: &Edge| {
                let via = if e.from == l.position { String::new() } else { format!(" (on layer {})", e.from) };
                format!("{} {}({}){via}", e.position, e.action, e.to)
            };
            // On a held layer only its own keys count: a transparent key there falls through to
            // this layer, not to the base layer. And the higher-numbered layer wins: a key of a
            // lower held layer is only reachable where this layer is transparent.
            let shadowed = |e: &Edge| e.from < l.position && !e.combo && l.keys.get(e.index).is_some() && !transparent(l, e.index);
            let usable = |e: &Edge| e.from == l.position || (within.contains(&e.from) && !e.via_transparent && !shadowed(e));
            let own = || edges.iter().filter(|e| usable(e));
            let exits = own()
                .filter(|e| (e.action == "TO" && e.to != l.position) || (matches!(e.action.as_str(), "TG" | "TT") && e.to == l.position))
                .map(describe)
                .collect();
            let default_exits = own().filter(|e| e.action == "DF" && e.to != l.position).map(describe).collect();
            LayerNode {
                position: l.position,
                title: l.title.clone(),
                reachable: reachable.contains(&l.position),
                momentary_only,
                auto_mouse: auto == Some(l.position),
                firmware_entered: firmware.contains(&l.position),
                entered_by_toggle: incoming.iter().any(|e| matches!(e.action.as_str(), "TG" | "TO" | "TT")),
                entered_by_default: incoming.iter().any(|e| e.action == "DF"),
                exits,
                default_exits,
            }
        })
        .collect();
    LayerGraph { edges, layers }
}

// MARK: summary

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutSummary {
    pub reference: String,
    pub title: String,
    pub geometry: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    pub key_count: usize,
    pub layers: Vec<LayerInfo>,
    pub combos: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_mouse_layer: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LayerInfo {
    pub position: i64,
    pub title: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    pub bound: usize,
    pub transparent: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reached_by: Option<String>,
    pub reachable: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub exits: Vec<String>,
}

pub fn summary(layout: &Layout) -> LayoutSummary {
    let graph = layer_graph(layout);
    let basic = crate::summary::layers(layout);
    let layers = basic
        .iter()
        .map(|b| {
            let node = graph.layers.iter().find(|n| n.position == b.position);
            LayerInfo {
                position: b.position,
                title: b.title.clone(),
                tags: b.tags.clone(),
                bound: b.bound,
                transparent: b.transparent,
                reached_by: graph.how_to_reach(b.position),
                reachable: node.is_some_and(|n| n.reachable),
                // How to leave, matching how the layer is entered.
                exits: node
                    .map(|n| {
                        let mut e = n.exits.clone();
                        if n.entered_by_default {
                            e.extend(n.default_exits.iter().cloned());
                        }
                        e
                    })
                    .unwrap_or_default(),
            }
        })
        .collect();
    LayoutSummary {
        reference: layout.reference(),
        title: layout.title.clone(),
        geometry: layout.geometry.clone(),
        created_at: layout.created_at.clone(),
        key_count: layout.key_count(),
        layers,
        combos: layout.combos.len(),
        auto_mouse_layer: layout.settings.auto_mouse_layer,
    }
}
