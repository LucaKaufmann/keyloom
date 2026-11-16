//! The real fixture layout: explore, drafts, validation, diff, guide, verify.

use keyloom::diff::{diff, Change, DiffOptions};
use keyloom::explore;
use keyloom::guide;
use keyloom::model::{Kind, Layout};
use keyloom::ops::{apply_ops, parse_binding, Op};
use keyloom::qmk;
use keyloom::source;
use keyloom::validate::{validate, Context, Severity};
use keyloom::workspace::Workspace;
use serde_json::{json, Value};
use std::path::PathBuf;

fn real_layout() -> Layout {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/oryx-9qMJK-ZPjbXz.json");
    let raw: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let mut l = source::parse_value(&raw).unwrap();
    l.settings.auto_mouse_layer = Some(4); // from Oryx's config.h (docs/oryx-encodings.md)
    l
}

fn draft() -> Layout {
    let mut d = real_layout();
    d.kind = Kind::Draft;
    d.draft = Some(keyloom::model::DraftMeta { name: "t".into(), created: "x".into(), updated: "x".into(), notes: None });
    d
}

fn ops(v: Value) -> Vec<Op> {
    serde_json::from_value(v).unwrap()
}

fn rules(findings: &[keyloom::validate::Finding]) -> Vec<&str> {
    findings.iter().map(|f| f.rule).collect()
}

// MARK: explore

#[test]
fn find_and_key_info() {
    let l = real_layout();
    let esc = explore::find(&l, "Esc");
    assert!(esc.iter().any(|h| h.layer == 0 && h.position == "L.num.0" && h.slot == "hold"), "{esc:?}");
    assert!(esc.iter().any(|h| h.layer == 1 && h.position == "L.num.0" && h.slot == "tap"));
    let sym = esc.iter().find(|h| h.layer == 1).unwrap();
    assert_eq!(sym.reach.as_deref(), Some("hold L.thumb.2 (MO(1))"));
    assert!(explore::find(&l, "KC_ESC").len() == esc.len(), "aliases normalise");
    let mo2 = explore::find(&l, "MO(Brd+Sys)");
    assert_eq!(mo2.len(), 1);
    assert_eq!(mo2[0].position, "R.thumb.2");

    let info = explore::key_info(&l, 25, None);
    assert_eq!(info.position, "L.thumb.2");
    assert_eq!(info.layers[0].qmk, "LT(1, KC_SPACE)");
    let gaming = info.layers.iter().find(|x| x.layer == 3).unwrap();
    assert_eq!(gaming.qmk, "KC_SPACE");
    let on_sym = info.layers.iter().find(|x| x.layer == 1).unwrap();
    assert!(on_sym.transparent && on_sym.resolved_from == Some(0));
}

#[test]
fn free_keys_are_sorted_by_effort() {
    let l = real_layout();
    let free = explore::free_keys(&l, 3);
    assert_eq!(free.len(), 51);
    assert_eq!(free[0].position, "L.home.4", "index finger home key is easiest");
    assert!(free.windows(2).all(|w| w[0].effort <= w[1].effort));
    assert!(explore::free_keys(&l, 0).is_empty());
}

#[test]
fn layer_graph_and_summary() {
    let l = real_layout();
    let g = explore::layer_graph(&l);
    let reach: Vec<bool> = g.layers.iter().map(|n| n.reachable).collect();
    assert_eq!(reach, [true, true, true, false, true], "Gaming unreachable, Mouse via auto-mouse");
    let s = explore::summary(&l);
    assert_eq!(s.layers[4].reached_by.as_deref(), Some("move the trackball (auto-mouse layer)"));
}

#[test]
fn validation_of_the_real_layout() {
    let f = validate(&real_layout(), &Context::default());
    let unreachable: Vec<_> = f.iter().filter(|f| f.rule == "layer-unreachable").collect();
    assert_eq!(unreachable.len(), 1, "{f:#?}");
    assert_eq!(unreachable[0].layer, Some(3));
    assert!(!f.iter().any(|f| f.severity == Severity::Error), "{f:#?}");
}

// MARK: action specs

#[test]
fn action_specs_become_oryx_keys() {
    let l = real_layout();
    let k = |s: &str| qmk::key(&parse_binding(&l, s).unwrap().key);
    assert_eq!(k("LT(1, KC_SPC)"), "LT(1, KC_SPACE)");
    assert_eq!(k("LT(sym, Space)"), "LT(1, KC_SPACE)");
    assert_eq!(k("LCTL_T(KC_A)"), "MT(MOD_LCTL, KC_A)");
    assert_eq!(k("MT(LSFT, KC_A)"), "MT(MOD_LSFT, KC_A)");
    assert_eq!(k("MEH_T(KC_BSPC)"), "MEH_T(KC_BSPC)");
    assert_eq!(k("TG(Gaming)"), "TG(3)");
    assert_eq!(k("MO(2)"), "MO(2)");
    assert_eq!(k("LCTL(LSFT(KC_C))"), "LCTL(LSFT(KC_C))");
    assert_eq!(k("TRNS"), "KC_TRANSPARENT");
    assert_eq!(k("NO"), "KC_NO");
    assert_eq!(k("Esc"), "KC_ESCAPE");
    assert!(parse_binding(&l, "TG(nope)").is_err());
    assert!(parse_binding(&l, "OSM(LSFT)").is_err(), "unknown Oryx encodings are rejected");
    assert!(parse_binding(&l, "what is this").is_err());
}

// MARK: drafts, ops, diff, guide, verify

/// A minimal WoW layer, the way an agent would build it.
fn wow_ops() -> Vec<Op> {
    ops(json!([
        {"op": "rename_layer", "layer": "Gaming", "title": "WoW"},
        {"op": "tag_layer", "layer": "WoW", "tags": ["gaming"]},
        {"op": "set_key", "layer": "WoW", "key": "L.top.2", "action": "KC_W"},
        {"op": "set_key", "layer": "WoW", "key": "L.home.1", "action": "KC_A"},
        {"op": "set_key", "layer": "WoW", "key": "L.home.2", "action": "KC_S"},
        {"op": "set_key", "layer": "WoW", "key": "L.home.3", "action": "KC_D"},
        {"op": "set_key", "layer": "WoW", "key": "L.num.0", "action": "KC_ESCAPE"},
        {"op": "set_key", "layer": "WoW", "key": "L.top.0", "action": "KC_TAB"},
        {"op": "set_key", "layer": "WoW", "key": "L.home.0", "action": "KC_LSFT"},
        {"op": "set_key", "layer": "WoW", "key": "L.bot.0", "action": "KC_LCTL"},
        {"op": "set_key", "layer": "WoW", "key": "R.home.0", "action": "KC_QUOTE"},
        {"op": "set_key", "layer": "WoW", "key": "R.top.0", "action": "KC_BSLS"},
        {"op": "set_key", "layer": "WoW", "key": "R.bot.1", "action": "KC_SLASH"},
        {"op": "set_key", "layer": "WoW", "key": "R.thumb.1", "action": "KC_BSPC"},
        {"op": "set_key", "layer": "Main", "key": "R.thumb.2", "action": "LT(Brd+Sys, KC_F13)"}
    ]))
}

#[test]
fn wow_layer_end_to_end() {
    let base = real_layout();
    let mut d = draft();
    apply_ops(&mut d, &wow_ops()).unwrap();
    // Not reachable yet: validation says so.
    assert!(rules(&validate(&d, &Context::default())).contains(&"layer-unreachable"));
    // A toggle on the base layer, and something else at that position on WoW: Oryx's editor
    // clears that key (it keeps the position transparent), so the base toggle is the way out.
    apply_ops(
        &mut d,
        &ops(json!([{"op": "set_key", "layer": "Main", "key": "L.thumb.1", "action": "TG(WoW)"},
                                   {"op": "set_key", "layer": "WoW", "key": "L.thumb.1", "action": "KC_LALT"}])),
    )
    .unwrap();
    let f = validate(&d, &Context::default());
    assert!(f.iter().any(|x| x.rule == "oryx-clears-key" && x.key.as_deref() == Some("L.thumb.1")), "{f:#?}");
    assert!(!rules(&f).contains(&"layer-no-exit"), "{f:#?}");
    // Transparent there, the way Oryx keeps it: the same key toggles back.
    apply_ops(&mut d, &ops(json!([{"op": "clear_key", "layer": "WoW", "key": "L.thumb.1"}]))).unwrap();
    let f = validate(&d, &Context::default());
    assert!(!rules(&f).contains(&"layer-no-exit") && !rules(&f).contains(&"oryx-clears-key"), "{f:#?}");
    assert!(!rules(&f).contains(&"layer-unreachable"), "{f:#?}");
    // Keys left transparent fall through to base mod-taps (Tab/Ctrl etc. are overridden above,
    // but e.g. R.bot.0 = /‍/RALT is not): gaming-home-row-mods warns about them.
    assert!(f.iter().any(|x| x.rule == "gaming-home-row-mods"), "{f:#?}");

    let dd = diff(&base, &d, DiffOptions::default());
    assert!(dd.changes.contains(&Change::LayerChanged {
        position: 3,
        was: 3,
        title: "WoW".into(),
        field: "title".into(),
        from: Some("Gaming".into()),
        to: Some("WoW".into())
    }));
    assert!(dd.summary.contains("key(s) changed"), "{}", dd.summary);

    let g = guide::guide(&base, &d);
    let text = guide::render(&g);
    assert!(text.contains("TG(3 WoW)"), "{text}");
    assert!(text.contains("Oryx: Layer → Toggle (TG)"), "{text}");
    assert!(g.sections.first().unwrap().heading == "Layers", "layer changes come first");

    // Verify: nothing applied yet → everything still to do; all applied → match.
    let v = guide::verify(&base, &d, &base);
    assert!(!v.matches && v.unexpected.is_empty() && !v.still_to_do.is_empty());
    let mut flashed = d.clone();
    flashed.kind = Kind::Revision;
    flashed.draft = None;
    flashed.revision_id = "NEWREV".into();
    for l in &mut flashed.layers {
        l.keyloom = Default::default(); // Oryx doesn't keep keyloom's tags
    }
    let v = guide::verify(&base, &d, &flashed);
    assert!(v.matches, "{v:#?}");
    // A mistake in Oryx shows up as still to do; an unrelated change as unexpected.
    let mut wrong = flashed.clone();
    wrong.layers[3].keys[13] = Default::default();
    wrong.layers[1].keys[1].tap = Some(keyloom::model::Action { code: Some("KC_Q".into()), ..Default::default() });
    let v = guide::verify(&base, &d, &wrong);
    assert_eq!(v.still_to_do.len(), 1, "{v:#?}");
    assert_eq!(v.unexpected.len(), 1, "{v:#?}");
}

#[test]
fn layer_moves_rewrite_references() {
    let mut d = draft();
    apply_ops(&mut d, &ops(json!([{"op": "add_layer", "title": "Nav", "after": 0, "tags": ["nav"]}]))).unwrap();
    let titles: Vec<_> = d.layers.iter().map(|l| l.title.as_str()).collect();
    assert_eq!(titles, ["Main", "Nav", "Sym+Num", "Brd+Sys", "Gaming", "Mouse"]);
    let info = explore::key_info(&d, 25, Some(0));
    assert_eq!(info.layers[0].qmk, "LT(2, KC_SPACE)", "MO(1) followed Sym+Num to position 2");
    assert_eq!(d.settings.auto_mouse_layer, Some(5));
    // The diff sees an added layer, moves, and no spurious key changes.
    let dd = diff(&real_layout(), &d, DiffOptions::default());
    assert!(!dd.changes.iter().any(|c| matches!(c, Change::KeyChanged { .. })), "{:#?}", dd.changes);
    // Removing a referenced layer needs force.
    assert!(apply_ops(&mut d, &ops(json!([{"op": "remove_layer", "layer": "Sym+Num"}]))).is_err());
    let r = apply_ops(&mut d, &ops(json!([{"op": "remove_layer", "layer": "Sym+Num", "force": true}]))).unwrap();
    assert!(r.warnings.iter().any(|w| w.contains("L.thumb.2")), "{r:?}");
    assert_eq!(d.layers.len(), 5);
}

#[test]
fn batches_are_atomic_and_revisions_read_only() {
    let mut rev = real_layout();
    assert!(apply_ops(&mut rev, &ops(json!([{"op": "set_key", "layer": 0, "key": "L.home.1", "action": "KC_B"}]))).is_err());
    let mut d = draft();
    let before = d.clone();
    let e = apply_ops(
        &mut d,
        &ops(json!([
            {"op": "set_key", "layer": 0, "key": "L.home.1", "action": "KC_B"},
            {"op": "set_key", "layer": 0, "key": "L.nope.1", "action": "KC_B"}
        ])),
    )
    .unwrap_err();
    assert_eq!(e.op, Some(1));
    assert_eq!(d, before);
}

#[test]
fn combos() {
    let mut d = draft();
    apply_ops(&mut d, &ops(json!([{"op": "add_combo", "layer": 0, "keys": ["L.home.3", "L.home.4"], "action": "KC_ESC", "name": "esc"}])))
        .unwrap();
    assert!(explore::find(&d, "Escape").iter().any(|h| h.slot == "combo" && h.position == "L.home.3+L.home.4"));
    assert!(
        apply_ops(&mut d, &ops(json!([{"op": "add_combo", "layer": 0, "keys": ["L.home.4", "L.home.3"], "action": "KC_TAB"}]))).is_err()
    );
    let g = guide::guide(&real_layout(), &d);
    assert!(guide::render(&g).contains("Add a combo L.home.3 + L.home.4 on layer 0 \"Main\" → KC_ESCAPE"), "{}", guide::render(&g));
}

#[test]
fn workspace_drafts_with_undo() {
    let dir = tempfile::tempdir().unwrap();
    let ws = Workspace::open(dir.path()).unwrap();
    ws.store_revision(&real_layout(), None).unwrap();
    let rev = ws.resolve(None).unwrap();
    let mut d = ws.create_draft("wow", &rev).unwrap();
    assert!(ws.create_draft("wow", &rev).is_err(), "names are unique");
    for batch in [
        ops(json!([{"op": "rename_layer", "layer": 3, "title": "WoW"}])),
        ops(json!([{"op": "set_key", "layer": "WoW", "key": "L.home.1", "action": "KC_A"}])),
    ] {
        apply_ops(&mut d, &batch).unwrap();
        ws.save_draft(&mut d, &batch).unwrap();
    }
    assert_eq!(ws.resolve(Some("draft:wow")).unwrap().layers[3].title, "WoW");
    assert_eq!(ws.drafts().unwrap()[0].ops, 2);
    let undone = ws.undo_draft("wow").unwrap();
    assert_eq!(undone.layers[3].title, "WoW");
    assert_eq!(qmk::key(&undone.layers[3].keys[13]), "KC_TRANSPARENT");
    assert_eq!(ws.draft_log("wow").unwrap().len(), 1);
    ws.undo_draft("wow").unwrap();
    assert!(ws.undo_draft("wow").is_err());
    assert_eq!(ws.load_draft("wow").unwrap().layers[3].title, "Gaming");
    // The revision itself never changed.
    assert_eq!(ws.resolve(Some("9qMJK@ZPjbXz")).unwrap().layers[3].title, "Gaming");
    let ctx = Context { newest_revision: Some("NEWER".into()), ..Default::default() };
    assert!(rules(&validate(&ws.load_draft("wow").unwrap(), &ctx)).contains(&"base-drift"));
    ws.archive_draft("wow").unwrap();
    assert!(ws.drafts().unwrap().is_empty());
}

#[test]
fn auto_mouse_layer_from_config_h() {
    let cfg = "#define USB_SUSPEND_WAKEUP_DELAY 0\n#define AUTOMOUSE_LAYER 4\n#define AUTOMOUSE_THRESHOLD 10\n";
    assert_eq!(source::parse_auto_mouse_layer(cfg), Some(4));
    assert_eq!(source::parse_auto_mouse_layer("#define X 1"), None);
}
