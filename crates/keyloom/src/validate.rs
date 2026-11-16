//! Validation rules (spec 02). Each finding names its rule so rules can be switched off.

use crate::explore::{layer_graph, oryx_cleared};
use crate::knowledge;
use crate::model::{Action, Key, Layout};
use crate::positions::{display_name, PositionMap};
use crate::qmk;
use serde::Serialize;
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warn,
    Info,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    pub rule: &'static str,
    pub severity: Severity,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layer: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    pub message: String,
    pub hint: String,
}

pub const RULES: &[(&str, Severity)] = &[
    ("layer-ref-missing", Severity::Error),
    ("layer-no-exit", Severity::Error),
    ("key-count", Severity::Error),
    ("combo-duplicate", Severity::Error),
    ("oryx-clears-key", Severity::Warn),
    ("layer-unreachable", Severity::Warn),
    ("too-many-layers", Severity::Warn),
    ("unknown-keycode", Severity::Warn),
    ("gaming-tap-hold", Severity::Warn),
    ("gaming-home-row-mods", Severity::Warn),
    ("combo-overlap", Severity::Info),
    ("essential-unreachable", Severity::Info),
    ("duplicate-in-layer", Severity::Info),
    ("mo-target-overrides", Severity::Info),
    ("base-drift", Severity::Info),
];

/// Safe layer count for Oryx builds (8-bit layer state, docs/oryx-encodings.md).
pub const SAFE_LAYERS: usize = 8;

#[derive(Debug, Clone, Default)]
pub struct Context {
    /// Newest stored revision of the draft's layout (for `base-drift`).
    pub newest_revision: Option<String>,
    pub disabled: Vec<String>,
}

fn severity(rule: &str) -> Severity {
    RULES.iter().find(|(r, _)| *r == rule).map(|(_, s)| *s).unwrap_or(Severity::Info)
}

struct Out<'a> {
    findings: Vec<Finding>,
    disabled: &'a [String],
}

impl Out<'_> {
    fn push(&mut self, rule: &'static str, layer: Option<i64>, key: Option<String>, message: String, hint: &str) {
        if self.disabled.iter().any(|d| d == rule) {
            return;
        }
        self.findings.push(Finding { rule, severity: severity(rule), layer, key, message, hint: hint.into() });
    }
}

fn has_tap_hold(k: &Key) -> bool {
    let tap = k.tap.as_ref().and_then(Action::code).is_some();
    let extra = [&k.hold, &k.double_tap, &k.tap_hold].iter().any(|s| s.as_ref().and_then(Action::code).is_some());
    (tap && extra) || k.double_tap.as_ref().and_then(Action::code).is_some() || k.tap_hold.as_ref().and_then(Action::code).is_some()
}

const ESSENTIAL: &[(&str, &str)] =
    &[("KC_ESCAPE", "Esc"), ("KC_ENTER", "Enter"), ("KC_BSPC", "Backspace"), ("KC_TAB", "Tab"), ("KC_SPACE", "Space")];

pub fn validate(layout: &Layout, ctx: &Context) -> Vec<Finding> {
    let map = PositionMap::for_geometry(&layout.geometry);
    let name = |i: usize| display_name(map.as_ref(), i);
    let mut out = Out { findings: Vec::new(), disabled: &ctx.disabled };
    let positions: BTreeSet<i64> = layout.layers.iter().map(|l| l.position).collect();
    let expected = map.as_ref().map(|m| m.key_count()).unwrap_or_else(|| layout.key_count());

    if layout.layers.len() > SAFE_LAYERS {
        out.push(
            "too-many-layers",
            None,
            None,
            format!("{} layers; Oryx builds above {SAFE_LAYERS} are untested by keyloom", layout.layers.len()),
            "merge layers or check that Oryx compiles it",
        );
    }

    for layer in &layout.layers {
        let lp = Some(layer.position);
        if layer.keys.len() != expected {
            out.push(
                "key-count",
                lp,
                None,
                format!("layer \"{}\" has {} keys, the {} has {expected}", layer.title, layer.keys.len(), layout.geometry),
                "re-import the layout",
            );
        }
        let gaming = layer.keyloom.tags.iter().any(|t| t.trim().eq_ignore_ascii_case("gaming"));
        let mut seen: Vec<(String, usize)> = Vec::new();
        for (i, key) in layer.keys.iter().enumerate() {
            for (_, a) in key.slots() {
                if let (Some(n), Some(code)) = (a.layer, a.code()) {
                    if !positions.contains(&n) {
                        out.push(
                            "layer-ref-missing",
                            lp,
                            Some(name(i)),
                            format!("{code}({n}) on \"{}\" points to a layer that doesn't exist", layer.title),
                            "change the key or add the layer",
                        );
                    }
                    continue;
                }
                if let Some(code) = a.code() {
                    if knowledge::lookup(code).is_some_and(|k| k.layer) {
                        out.push(
                            "layer-ref-missing",
                            lp,
                            Some(name(i)),
                            format!("{code} on \"{}\" has no layer", layer.title),
                            "write it with its layer, e.g. MO(2), or change the key",
                        );
                        continue;
                    }
                    if a.code() != Some("RGB") && !knowledge::is_known(code) && !knowledge::keycodes().is_empty() {
                        out.push(
                            "unknown-keycode",
                            lp,
                            Some(name(i)),
                            format!("{code} isn't in keyloom's keycode reference"),
                            "check the spelling with `keyloom ref`",
                        );
                    }
                }
            }
            let tap_toggle = key.slots().iter().any(|(_, a)| a.code() == Some("TT"));
            if gaming && (has_tap_hold(key) || tap_toggle) {
                out.push(
                    "gaming-tap-hold",
                    lp,
                    Some(name(i)),
                    format!("{} on gaming layer \"{}\" has a hold/double-tap action", qmk::key(key), layer.title),
                    "use plain keys on gaming layers: tap-hold adds delay and misfires",
                );
            }
            if gaming && key.is_transparent() {
                if let Some((base, _)) = layout.effective_key(layer.position, i) {
                    if has_tap_hold(base) {
                        out.push(
                            "gaming-home-row-mods",
                            lp,
                            Some(name(i)),
                            format!("transparent key falls through to {} on the base layer", qmk::key(base)),
                            "bind the key explicitly on the gaming layer (or KC_NO)",
                        );
                    }
                }
            }
            if !(layer.position != 0 && key.is_transparent()) && key.hold.is_none() && key.double_tap.is_none() && key.tap_hold.is_none() {
                if let Some(a) = key.tap.as_ref().filter(|a| a.layer.is_none() && a.modifiers.is_none()) {
                    if let Some(c) = a.code().filter(|c| !matches!(*c, "KC_NO" | "KC_TRANSPARENT" | "KC_TRNS")) {
                        let c = knowledge::canonical(c).unwrap_or_else(|| c.to_string());
                        match seen.iter().find(|(s, _)| *s == c) {
                            Some((_, first)) => out.push(
                                "duplicate-in-layer",
                                lp,
                                Some(name(i)),
                                format!("{c} is also on {} of \"{}\"", name(*first), layer.title),
                                "fine if intentional",
                            ),
                            None => seen.push((c, i)),
                        }
                    }
                }
            }
            // A key holding MO(n)/LT on this layer that is something else on layer n.
            if let Some(h) = key.hold.as_ref().filter(|h| h.code() == Some("MO")) {
                if let Some(target) = h.layer.and_then(|n| layout.layer(n)) {
                    if let Some(tk) = target.keys.get(i) {
                        if !tk.is_transparent() && target.position != layer.position {
                            out.push(
                                "mo-target-overrides",
                                Some(target.position),
                                Some(name(i)),
                                format!("{} holds layer \"{}\" but is {} there", name(i), target.title, qmk::key(tk)),
                                "fine in QMK (the held key stays held), but often surprising",
                            );
                        }
                    }
                }
            }
        }
    }

    // Keys Oryx's editor clears when it shows the layer (explore::oryx_cleared).
    for c in oryx_cleared(layout) {
        let (Some(layer), Some(by)) = (layout.layer(c.layer), layout.layer(c.by_layer)) else { continue };
        let Some(key) = layer.keys.get(c.index) else { continue };
        out.push(
            "oryx-clears-key",
            Some(c.layer),
            Some(name(c.index)),
            format!(
                "{} is here, but Oryx's editor clears it as soon as it shows layer \"{}\": \"{}\" has {}({}) at the same position, and Oryx keeps this one transparent so that key is reached",
                qmk::key(key),
                layer.title,
                by.title,
                c.action,
                c.layer
            ),
            "leave it transparent; a key that leaves the layer (TO(0)) has to go at another position",
        );
    }

    // Reachability and exits.
    let graph = layer_graph(layout);
    for node in &graph.layers {
        if node.position == 0 {
            continue;
        }
        if !node.reachable {
            out.push(
                "layer-unreachable",
                Some(node.position),
                None,
                format!("no key leads to layer \"{}\"", node.title),
                "add a layer key (MO/TG/TO/OSL) on a reachable layer; if the firmware enters it by itself (e.g. auto-mouse), tag the layer `auto`",
            );
            continue;
        }
        // The way out has to match the way in: TG/TO/TT are undone by TG/TT/TO, DF only by DF.
        if node.entered_by_toggle && node.exits.is_empty() {
            out.push(
                "layer-no-exit",
                Some(node.position),
                None,
                format!("layer \"{}\" is entered with a toggle/go-to key but has no key that leaves it", node.title),
                "add TO(0) on the layer, at a position where no lower layer has a layer key (Oryx clears those); if the layer is toggled from the base layer, leaving that position transparent also works",
            );
        }
        if node.entered_by_default && node.default_exits.is_empty() {
            out.push(
                "layer-no-exit",
                Some(node.position),
                None,
                format!("layer \"{}\" becomes the default layer (DF) but has no key that sets another default", node.title),
                "add DF(0) on the layer (TG/TO don't change the default layer)",
            );
        }
    }

    // Essential keys: on the base layer or one layer step away.
    let near: BTreeSet<i64> = [0].into_iter().chain(graph.edges.iter().filter(|e| e.from == 0).map(|e| e.to)).collect();
    for (code, label) in ESSENTIAL {
        let found = near.iter().filter_map(|&l| layout.layer(l)).any(|l| {
            l.keys.iter().enumerate().any(|(i, _)| {
                layout
                    .effective_key(l.position, i)
                    .is_some_and(|(k, _)| k.slots().iter().any(|(_, a)| a.code().is_some_and(|c| knowledge::same_code(c, code))))
            })
        });
        if !found {
            out.push(
                "essential-unreachable",
                None,
                None,
                format!("{label} isn't on the base layer or one layer away"),
                "keep Esc, Enter, Backspace, Tab and Space easy to reach",
            );
        }
    }

    // Combos.
    for c in &layout.combos {
        let on = c["layerIdx"].as_i64().unwrap_or(0);
        let keys = crate::ops::sorted_keys(c).iter().map(|&k| name(k as usize)).collect::<Vec<_>>().join("+");
        if !positions.contains(&on) {
            out.push(
                "layer-ref-missing",
                None,
                Some(keys.clone()),
                format!("a combo is on layer {on}, which doesn't exist"),
                "remove the combo",
            );
        }
        if let Some(n) = c["trigger"]["layer"].as_i64().filter(|n| !positions.contains(n)) {
            out.push(
                "layer-ref-missing",
                Some(on).filter(|l| positions.contains(l)),
                Some(keys),
                format!("a combo switches to layer {n}, which doesn't exist"),
                "change the combo or add the layer",
            );
        }
    }
    let combos: Vec<(i64, Vec<i64>)> =
        layout.combos.iter().map(|c| (c["layerIdx"].as_i64().unwrap_or(0), crate::ops::sorted_keys(c))).collect();
    for (i, a) in combos.iter().enumerate() {
        for b in &combos[i + 1..] {
            if a.0 != b.0 {
                continue;
            }
            let keys = a.1.iter().map(|&k| name(k as usize)).collect::<Vec<_>>().join("+");
            if a.1 == b.1 {
                out.push("combo-duplicate", Some(a.0), Some(keys), "two combos use the same keys on the same layer".into(), "remove one");
            } else if a.1.iter().all(|k| b.1.contains(k)) || b.1.iter().all(|k| a.1.contains(k)) {
                out.push(
                    "combo-overlap",
                    Some(a.0),
                    Some(keys),
                    "one combo's keys are part of another's".into(),
                    "the smaller combo may fire first",
                );
            }
        }
    }

    if let (Some(newest), true) = (&ctx.newest_revision, layout.draft.is_some()) {
        if *newest != layout.revision_id {
            out.push(
                "base-drift",
                None,
                None,
                format!("this draft is based on {} but {newest} is newer", layout.revision_id),
                "check that the newer revision's changes don't conflict before applying",
            );
        }
    }

    out.findings.sort_by(|a, b| a.severity.cmp(&b.severity).then(a.layer.cmp(&b.layer)));
    out.findings
}

pub fn has_errors(findings: &[Finding]) -> bool {
    findings.iter().any(|f| f.severity == Severity::Error)
}
