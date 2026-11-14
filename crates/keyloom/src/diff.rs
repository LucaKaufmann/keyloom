//! Structured diff between two layouts (spec 02): the input to the apply guide and `verify`.
//!
//! Layers are matched by title, then by content (a renamed layer that also moved), then by
//! position (a renamed, nearly empty layer). Layer references are compared through the matched
//! pairs, so renaming or moving a layer doesn't make every `MO(n)` look changed. Only real
//! reorders are reported as moves: layers shifted by an insert or a removal are not.

use crate::knowledge;
use crate::model::{Action, Key, Layer, Layout, Slot};
use crate::positions::{display_name, PositionMap};
use crate::qmk;
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "change", rename_all = "snake_case", rename_all_fields = "camelCase")]
pub enum Change {
    LayerAdded {
        position: i64,
        title: String,
        /// Title of the layer right before it in the target, `None` when it becomes the first.
        #[serde(skip_serializing_if = "Option::is_none")]
        after: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        color: Option<String>,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        tags: Vec<String>,
    },
    LayerRemoved {
        position: i64,
        title: String,
    },
    /// A real reorder (not a shift caused by an added or removed layer).
    LayerMoved {
        title: String,
        from: i64,
        to: i64,
        #[serde(skip_serializing_if = "Option::is_none")]
        after: Option<String>,
    },
    LayerChanged {
        position: i64,
        /// The layer's position in the layout compared from.
        was: i64,
        title: String,
        field: String,
        from: Option<String>,
        to: Option<String>,
    },
    KeyChanged {
        layer: i64,
        layer_title: String,
        index: usize,
        position: String,
        /// `tap`, `hold`, `doubleTap`, `tapHold`, `label` or `color`.
        slot: String,
        from: Option<String>,
        to: Option<String>,
    },
    ComboAdded {
        layer: i64,
        layer_title: String,
        keys: Vec<String>,
        trigger: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,
    },
    ComboRemoved {
        /// Position in the target layout; `None` when the layer itself is removed.
        layer: Option<i64>,
        layer_title: String,
        keys: Vec<String>,
        trigger: String,
    },
    ComboChanged {
        layer: i64,
        layer_title: String,
        keys: Vec<String>,
        from: String,
        to: String,
    },
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Diff {
    pub from: String,
    pub to: String,
    pub changes: Vec<Change>,
    pub summary: String,
}

impl Diff {
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DiffOptions {
    /// Ignore keyloom-only metadata (tags), e.g. for `verify`.
    pub ignore_metadata: bool,
}

/// Comparable form of an action: canonical code, matched layer id, modifiers.
fn norm_action(layer_id: &dyn Fn(i64) -> String, a: &Action) -> Option<String> {
    let code = a.code()?;
    let code = knowledge::canonical(code).unwrap_or_else(|| code.to_string());
    let mut s = match a.layer {
        Some(n) => format!("{code}({})", layer_id(n)),
        None => code,
    };
    if let Some(c) = a.color.as_ref().filter(|_| a.code() == Some("RGB")) {
        s = format!("RGB({})", c.to_lowercase());
    }
    if let Some(m) = &a.modifiers {
        for name in m.qmk_names().iter().rev() {
            s = format!("{name}({s})");
        }
    }
    Some(s)
}

/// Display form: QMK action with the layer's title, e.g. `MO(3 Gaming)`.
fn show_action(layout: &Layout, a: &Action) -> Option<String> {
    let s = qmk::action(a)?;
    match a.layer {
        Some(n) => Some(s.replacen(&format!("({n})"), &format!("({n} {})", layout.layer_title(n)), 1)),
        None => Some(s),
    }
}

fn slots(k: &Key) -> [(Slot, Option<&Action>); 4] {
    let transparent = k.is_transparent();
    [
        // Blank and explicit KC_TRANSPARENT keys compare equal (Oryx compiles both the same).
        (Slot::Tap, if transparent { None } else { k.tap.as_ref() }),
        (Slot::Hold, k.hold.as_ref()),
        (Slot::DoubleTap, k.double_tap.as_ref()),
        (Slot::TapHold, k.tap_hold.as_ref()),
    ]
}

fn slot_label(s: Slot) -> &'static str {
    match s {
        Slot::Tap => "tap",
        Slot::Hold => "hold",
        Slot::DoubleTap => "doubleTap",
        Slot::TapHold => "tapHold",
    }
}

fn combo_trigger(layout: &Layout, layer_id: &dyn Fn(i64) -> String, c: &Value) -> (Option<String>, String) {
    let a: Option<Action> = serde_json::from_value(c["trigger"].clone()).ok();
    let norm = a.as_ref().and_then(|a| norm_action(layer_id, a));
    let show = a.as_ref().and_then(|a| show_action(layout, a)).unwrap_or_else(|| "?".into());
    (norm, show)
}

/// How alike two layers are: (keys that are equal, keys where either side does something).
/// Layer targets are ignored (the pairing isn't known yet).
fn similarity(a: &Layer, b: &Layer) -> (usize, usize) {
    let any = |_: i64| "*".to_string();
    let norm = |k: &Key| slots(k).map(|(_, a)| a.and_then(|a| norm_action(&any, a)));
    let default = Key::default();
    let (mut same, mut active) = (0, 0);
    for i in 0..a.keys.len().max(b.keys.len()) {
        let (ka, kb) = (a.keys.get(i).unwrap_or(&default), b.keys.get(i).unwrap_or(&default));
        if ka.is_transparent() && kb.is_transparent() {
            continue;
        }
        active += 1;
        if norm(ka) == norm(kb) {
            same += 1;
        }
    }
    (same, active)
}

fn bound_keys(l: &Layer) -> usize {
    l.keys.iter().filter(|k| !k.is_transparent()).count()
}

/// Indices (into `seq`) of one longest strictly increasing subsequence; deterministic.
fn longest_increasing(seq: &[i64]) -> Vec<usize> {
    let n = seq.len();
    let mut len = vec![1usize; n];
    let mut prev = vec![usize::MAX; n];
    for i in 0..n {
        for j in 0..i {
            if seq[j] < seq[i] && len[j] + 1 > len[i] {
                len[i] = len[j] + 1;
                prev[i] = j;
            }
        }
    }
    let Some(mut at) = (0..n).max_by_key(|&i| (len[i], std::cmp::Reverse(i))) else { return vec![] };
    let mut out = vec![at];
    while prev[at] != usize::MAX {
        at = prev[at];
        out.push(at);
    }
    out.reverse();
    out
}

/// Which layer of `a` is which layer of `b`: `(pairs of (a position, b position) in b's order,
/// a's unmatched layers, b's unmatched layers)`.
pub fn match_layers(a: &Layout, b: &Layout) -> (Vec<(i64, i64)>, Vec<i64>, Vec<i64>) {
    let mut pairs: Vec<(i64, i64)> = Vec::new();
    let mut a_left: Vec<i64> = a.layers.iter().map(|l| l.position).collect();
    let mut b_left: Vec<i64> = b.layers.iter().map(|l| l.position).collect();
    let mut pair = |pa: i64, pb: i64, a_left: &mut Vec<i64>, b_left: &mut Vec<i64>| {
        pairs.push((pa, pb));
        a_left.retain(|&x| x != pa);
        b_left.retain(|&x| x != pb);
    };
    // 0. Identical content (with at least 3 bound keys): the same layer whatever it is called
    //    or wherever it sits now. So swapping two titles is two renames, and "delete A, rename
    //    B to A" isn't mistaken for re-keying A. If several layers are identical (a copy), the
    //    one that kept the title is the original; without a title to go by, later passes decide.
    let same_title = |x: &str, y: &str| x.trim().eq_ignore_ascii_case(y.trim());
    let identical = |la: &Layer, lb: &Layer| {
        let (same, active) = similarity(la, lb);
        same == active && active >= 3
    };
    // 0a. An identical layer that also kept its title.
    for la in &a.layers {
        if let Some(lb) = b.layers.iter().find(|lb| b_left.contains(&lb.position) && same_title(&lb.title, &la.title) && identical(la, lb))
        {
            pair(la.position, lb.position, &mut a_left, &mut b_left);
        }
    }
    // 0b. A single identical twin under another title. Not if a layer that still has this
    //     layer's title is mostly the same: then that one is the (edited) original and the
    //     twin is a copy of it.
    let alike = |la: &Layer, lb: &Layer| {
        let (same, active) = similarity(la, lb);
        same >= 3 && same * 2 >= active
    };
    for la in a.layers.iter().filter(|la| a_left.contains(&la.position)).collect::<Vec<_>>() {
        let twins: Vec<&Layer> = b.layers.iter().filter(|lb| b_left.contains(&lb.position) && identical(la, lb)).collect();
        let original_edited = b.layers.iter().any(|lb| b_left.contains(&lb.position) && same_title(&lb.title, &la.title) && alike(la, lb));
        if let ([lb], false) = (twins.as_slice(), original_edited) {
            pair(la.position, lb.position, &mut a_left, &mut b_left);
        }
    }
    // 1. Same title. Unless the title was reused: the two layers have little in common and one
    //    of them is mostly the same as a layer under another title (e.g. "delete A, rename B to A,
    //    change a key on it"). Those are left to the content pass; pass 2b picks up the rest.
    for lb in b.layers.iter().filter(|lb| b_left.contains(&lb.position)).collect::<Vec<_>>() {
        let Some(pa) = a_left.iter().copied().find(|&pa| same_title(&a.layer_title(pa), &lb.title)) else { continue };
        let la = a.layer(pa).expect("exists");
        let reused = !alike(la, lb)
            && (b.layers.iter().any(|o| o.position != lb.position && b_left.contains(&o.position) && alike(la, o))
                || a.layers.iter().any(|o| o.position != pa && a_left.contains(&o.position) && alike(o, lb)));
        if !reused {
            pair(pa, lb.position, &mut a_left, &mut b_left);
        }
    }
    // 2. Same content: a renamed layer, wherever it went. Best matches first.
    loop {
        let mut best: Option<(usize, i64, i64)> = None;
        for &pa in &a_left {
            for &pb in &b_left {
                let (same, active) = similarity(a.layer(pa).expect("exists"), b.layer(pb).expect("exists"));
                // Mostly the same, or (for a nearly empty layer) exactly the same.
                let alike = (same >= 3 && same * 2 >= active) || (same >= 1 && same == active);
                if alike && best.is_none_or(|(s, _, _)| same > s) {
                    best = Some((same, pa, pb));
                }
            }
        }
        let Some((_, pa, pb)) = best else { break };
        pair(pa, pb, &mut a_left, &mut b_left);
    }
    // 2b. Same title, for what the content pass left over.
    for lb in b.layers.iter().filter(|lb| b_left.contains(&lb.position)).collect::<Vec<_>>() {
        if let Some(pa) = a_left.iter().copied().find(|&pa| same_title(&a.layer_title(pa), &lb.title)) {
            pair(pa, lb.position, &mut a_left, &mut b_left);
        }
    }
    // 3. Same position, when the old layer was (nearly) empty: a placeholder that got renamed
    //    and filled. A populated layer replaced by a different one stays remove + add.
    for pb in b_left.clone() {
        if a_left.contains(&pb) && bound_keys(a.layer(pb).expect("exists")) <= 3 {
            pair(pb, pb, &mut a_left, &mut b_left);
        }
    }
    // 4. One nearly empty layer left on each side: the same placeholder, renamed and moved.
    if let ([pa], [pb]) = (a_left.as_slice(), b_left.as_slice()) {
        let (pa, pb) = (*pa, *pb);
        if bound_keys(a.layer(pa).expect("exists")) <= 3 {
            pair(pa, pb, &mut a_left, &mut b_left);
        }
    }
    pairs.sort_by_key(|&(_, pb)| pb);
    (pairs, a_left, b_left)
}

pub fn diff(a: &Layout, b: &Layout, opts: DiffOptions) -> Diff {
    let map = PositionMap::for_geometry(&b.geometry);
    let mut changes = Vec::new();

    let (pairs, a_left, b_left) = match_layers(a, b);

    // Layer ids for comparing references: b's positions; a's layers mapped through the pairs.
    let pairs_for_ids = pairs.clone();
    // A reference to a layer that never existed (possible in imported files) is its own kind,
    // equal on both sides, and not the same as a reference to a layer that was removed.
    let id_a = move |n: i64| match pairs_for_ids.iter().find(|(pa, _)| *pa == n) {
        Some((_, pb)) => pb.to_string(),
        None if a.layer(n).is_some() => format!("gone:{n}"),
        None => format!("missing:{n}"),
    };
    let id_b = |n: i64| if b.layer(n).is_some() { n.to_string() } else { format!("missing:{n}") };
    let before = |pb: i64| b.layers.iter().filter(|l| l.position < pb).max_by_key(|l| l.position).map(|l| l.title.clone());

    // MARK: layer changes
    for &pb in &b_left {
        let l = b.layer(pb).expect("exists");
        changes.push(Change::LayerAdded {
            position: pb,
            title: l.title.clone(),
            after: before(pb),
            color: l.color.clone(),
            tags: l.keyloom.tags.clone(),
        });
    }
    // Real reorders only: layers outside the longest run that kept its relative order.
    let kept = longest_increasing(&pairs.iter().map(|&(pa, _)| pa).collect::<Vec<_>>());
    for (i, &(pa, pb)) in pairs.iter().enumerate() {
        let (la, lb) = (a.layer(pa).expect("exists"), b.layer(pb).expect("exists"));
        if !kept.contains(&i) {
            changes.push(Change::LayerMoved { title: lb.title.clone(), from: pa, to: pb, after: before(pb) });
        }
        let mut field = |name: &str, x: Option<String>, y: Option<String>| {
            if x != y {
                changes.push(Change::LayerChanged { position: pb, was: pa, title: lb.title.clone(), field: name.into(), from: x, to: y });
            }
        };
        if la.title != lb.title {
            field("title", Some(la.title.clone()), Some(lb.title.clone()));
        }
        // Oryx doesn't normalise the case of hex colors; keyloom writes lower case.
        if la.color.as_deref().map(str::to_lowercase) != lb.color.as_deref().map(str::to_lowercase) {
            field("color", la.color.clone(), lb.color.clone());
        }
        if !opts.ignore_metadata && la.keyloom.tags != lb.keyloom.tags {
            field("tags", Some(la.keyloom.tags.join(",")), Some(lb.keyloom.tags.join(",")));
        }
    }

    // MARK: keys, slot by slot, in the target's layer order. A new layer compares against an
    // empty one, so everything on it shows up.
    let no_keys: Vec<Key> = Vec::new();
    for lb in &b.layers {
        let a_keys: &[Key] = match pairs.iter().find(|&&(_, pb)| pb == lb.position) {
            Some(&(pa, _)) => &a.layer(pa).expect("exists").keys,
            None => &no_keys,
        };
        let default = Key::default();
        for i in 0..lb.keys.len().max(a_keys.len()) {
            let (ka, kb) = (a_keys.get(i).unwrap_or(&default), lb.keys.get(i).unwrap_or(&default));
            let mut push = |slot: &str, from: Option<String>, to: Option<String>| {
                changes.push(Change::KeyChanged {
                    layer: lb.position,
                    layer_title: lb.title.clone(),
                    index: i,
                    position: display_name(map.as_ref(), i),
                    slot: slot.into(),
                    from,
                    to,
                });
            };
            for ((slot, x), (_, y)) in slots(ka).into_iter().zip(slots(kb)) {
                let nx = x.and_then(|x| norm_action(&id_a, x));
                let ny = y.and_then(|y| norm_action(&id_b, y));
                if nx != ny {
                    push(slot_label(slot), x.and_then(|x| show_action(a, x)), y.and_then(|y| show_action(b, y)));
                }
            }
            if ka.custom_label != kb.custom_label {
                push("label", ka.custom_label.clone(), kb.custom_label.clone());
            }
            if ka.glow_color.as_deref().map(str::to_lowercase) != kb.glow_color.as_deref().map(str::to_lowercase) {
                push("color", ka.glow_color.clone(), kb.glow_color.clone());
            }
        }
    }

    // MARK: combos, identified by (matched layer, sorted keys)
    let layer_of = |c: &Value| c["layerIdx"].as_i64().unwrap_or(0);
    let key_a = |c: &Value| (id_a(layer_of(c)), crate::ops::sorted_keys(c));
    let key_b = |c: &Value| (id_b(layer_of(c)), crate::ops::sorted_keys(c));
    let names = |ks: &[i64]| ks.iter().map(|&i| display_name(map.as_ref(), i as usize)).collect::<Vec<_>>();
    for cb in &b.combos {
        let kb = key_b(cb);
        let (nb, sb) = combo_trigger(b, &id_b, cb);
        let layer = layer_of(cb);
        match a.combos.iter().find(|ca| key_a(ca) == kb) {
            None => changes.push(Change::ComboAdded {
                layer,
                layer_title: b.layer_title(layer),
                keys: names(&kb.1),
                trigger: sb,
                name: cb["name"].as_str().map(str::to_string),
            }),
            Some(ca) => {
                let (na, sa) = combo_trigger(a, &id_a, ca);
                if na != nb {
                    changes.push(Change::ComboChanged { layer, layer_title: b.layer_title(layer), keys: names(&kb.1), from: sa, to: sb });
                }
            }
        }
    }
    for ca in &a.combos {
        let ka = key_a(ca);
        if !b.combos.iter().any(|cb| key_b(cb) == ka) {
            let now = pairs.iter().find(|&&(pa, _)| pa == layer_of(ca)).map(|&(_, pb)| pb);
            changes.push(Change::ComboRemoved {
                layer: now,
                layer_title: now.map(|pb| b.layer_title(pb)).unwrap_or_else(|| a.layer_title(layer_of(ca))),
                keys: names(&ka.1),
                trigger: combo_trigger(a, &id_a, ca).1,
            });
        }
    }
    for &pa in &a_left {
        changes.push(Change::LayerRemoved { position: pa, title: a.layer_title(pa) });
    }
    let summary = summarize(&changes);
    Diff { from: a.reference(), to: b.reference(), changes, summary }
}

fn summarize(changes: &[Change]) -> String {
    if changes.is_empty() {
        return "no differences".into();
    }
    let count = |f: fn(&Change) -> bool| changes.iter().filter(|c| f(c)).count();
    let mut parts = Vec::new();
    let mut add = |n: usize, what: &str| {
        if n > 0 {
            parts.push(format!("{n} {what}"));
        }
    };
    add(count(|c| matches!(c, Change::LayerAdded { .. })), "layer(s) added");
    add(count(|c| matches!(c, Change::LayerRemoved { .. })), "layer(s) removed");
    add(count(|c| matches!(c, Change::LayerMoved { .. })), "layer(s) moved");
    add(count(|c| matches!(c, Change::LayerChanged { .. })), "layer setting(s) changed");
    let mut keys: Vec<(i64, usize)> = changes
        .iter()
        .filter_map(|c| if let Change::KeyChanged { layer, index, .. } = c { Some((*layer, *index)) } else { None })
        .collect();
    keys.dedup();
    add(keys.len(), "key(s) changed");
    add(count(|c| matches!(c, Change::ComboAdded { .. } | Change::ComboRemoved { .. } | Change::ComboChanged { .. })), "combo change(s)");
    parts.join(", ")
}

#[cfg(test)]
mod tests {
    #[test]
    fn longest_increasing_run() {
        let lis = |s: &[i64]| super::longest_increasing(s).iter().map(|&i| s[i]).collect::<Vec<_>>();
        assert_eq!(lis(&[0, 1, 2, 3]), [0, 1, 2, 3]);
        assert_eq!(lis(&[0, 3, 1, 2]), [0, 1, 2], "moving 3 up is one move");
        assert_eq!(lis(&[1, 0]).len(), 1, "a swap is one move");
        assert!(lis(&[]).is_empty());
    }
}
