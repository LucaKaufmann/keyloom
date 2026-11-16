//! Regression tests for the issues found in the in-depth review (round 1).

use keyloom::diff::{diff, Change, DiffOptions};
use keyloom::explore;
use keyloom::guide;
use keyloom::knowledge;
use keyloom::model::{Action, DraftMeta, Key, Kind, Layout};
use keyloom::ops::{apply_ops, parse_binding, parse_slot_action, Op};
use keyloom::qmk;
use keyloom::source;
use keyloom::validate::{validate, Context};
use keyloom::workspace::{Stored, Workspace};
use serde_json::{json, Value};
use std::path::PathBuf;

fn real_layout() -> Layout {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/oryx-9qMJK-ZPjbXz.json");
    let raw: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let mut l = source::parse_value(&raw).unwrap();
    l.settings.auto_mouse_layer = Some(4);
    l
}

fn draft() -> Layout {
    let mut d = real_layout();
    d.kind = Kind::Draft;
    d.draft = Some(DraftMeta { name: "t".into(), created: "x".into(), updated: "x".into(), notes: None });
    d
}

fn ops(v: Value) -> Vec<Op> {
    serde_json::from_value(v).unwrap()
}

fn edit(d: &mut Layout, v: Value) -> Vec<String> {
    apply_ops(d, &ops(v)).unwrap().warnings
}

fn fail(d: &mut Layout, v: Value) -> String {
    apply_ops(d, &ops(v)).unwrap_err().to_string()
}

fn rules(l: &Layout) -> Vec<&'static str> {
    validate(l, &Context::default()).iter().map(|f| f.rule).collect()
}

fn qmk_at(l: &Layout, layer: i64, pos: &str) -> String {
    let i = keyloom::positions::PositionMap::voyager().by_name(pos).unwrap().index;
    qmk::key(&l.layer(layer).unwrap().keys[i])
}

// MARK: operations

#[test]
fn layer_numbers_in_actions_must_exist() {
    let mut d = draft();
    let e = fail(&mut d, json!([{"op": "set_key", "layer": 0, "key": "L.num.0", "action": "MO(5)"}]));
    assert!(e.contains("layer 5 doesn't exist"), "{e}");
    // Adding the layer earlier in the same batch works.
    edit(&mut d, json!([{"op": "add_layer", "title": "Nav"}, {"op": "set_key", "layer": 0, "key": "L.num.0", "action": "MO(5)"}]));
    assert_eq!(qmk_at(&d, 0, "L.num.0"), "MO(5)");
}

#[test]
fn inserting_a_layer_never_repoints_dangling_references() {
    // A reference to a layer that doesn't exist can come from an imported file.
    let mut d = draft();
    d.layers[0].keys[0] =
        Key { hold: Some(Action { code: Some("MO".into()), layer: Some(5), ..Default::default() }), ..Default::default() };
    let w = edit(&mut d, json!([{"op": "add_layer", "title": "WoW", "after": 0}]));
    assert!(w.iter().any(|w| w.contains("don't exist") && w.contains("L.num.0")), "{w:?}");
    assert_eq!(qmk_at(&d, 0, "L.num.0"), "KC_TRANSPARENT", "dropped, not pointed at the new layer");
    assert_eq!(d.layers[1].title, "WoW");
    assert_eq!(qmk_at(&d, 0, "L.thumb.2"), "LT(2, KC_SPACE)", "real references follow their layer");
    assert_eq!(d.settings.auto_mouse_layer, Some(5));
}

#[test]
fn removing_a_layer_keeps_the_auto_mouse_layer_and_handles_combos() {
    let mut d = draft();
    edit(&mut d, json!([{"op": "remove_layer", "layer": "Gaming"}]));
    assert_eq!(d.settings.auto_mouse_layer, Some(3), "Mouse moved from 4 to 3");
    assert!(!rules(&d).contains(&"layer-unreachable"));

    let mut d = draft();
    edit(
        &mut d,
        json!([
            {"op": "add_combo", "layer": "Gaming", "keys": ["L.home.3", "L.home.4"], "action": "KC_ESC", "name": "esc"},
            {"op": "add_combo", "layer": 0, "keys": ["R.home.3", "R.home.4"], "action": "TG(Gaming)", "name": "game"}
        ]),
    );
    let e = fail(&mut d, json!([{"op": "remove_layer", "layer": "Gaming"}]));
    assert!(e.contains("combo \"game\" on layer 0"), "a combo elsewhere that switches to the layer blocks removal: {e}");
    let w = edit(&mut d, json!([{"op": "remove_layer", "layer": "Gaming", "force": true}]));
    assert!(w.iter().any(|w| w.contains("also removed 1 combo(s) on the deleted layer: \"esc\"")), "{w:?}");
    assert!(w.iter().any(|w| w.contains("removed references") && w.contains("game")), "{w:?}");
    assert!(d.combos.is_empty());
}

#[test]
fn removing_or_moving_the_base_layer_warns() {
    let mut d = draft();
    let w = edit(&mut d, json!([{"op": "move_layer", "layer": 0, "to": 0}]));
    assert!(w.is_empty(), "a move to the same place does nothing: {w:?}");
    let w = edit(&mut d, json!([{"op": "move_layer", "layer": "Gaming", "to": 0}]));
    assert!(w.iter().any(|w| w.contains("\"Gaming\" is now layer 0")), "{w:?}");
}

#[test]
fn bare_family_codes_are_not_keys() {
    let l = real_layout();
    for (spec, expect) in [
        ("MO", "needs"),
        ("TG", "needs"),
        ("LT", "LT(<layer>, KC_x)"),
        ("RGB", "RGB(#rrggbb)"),
        ("MEH", "hold action"),
        ("Hyper", "hold action"),
        ("KC_MEH", "hold action"),
        ("LT(1)", "LT takes a layer and a key"),
        ("MO(1, 2)", "MO takes one layer"),
        ("FOO(KC_A)", "isn't an action keyloom knows"),
        ("LT(1, MO(2))", "whole-key action"),
        ("LCAG_T(KC_A)", "one modifier"),
        ("OSM(LSFT)", "isn't supported yet"),
        ("nonsense", "unknown keycode"),
    ] {
        let e = parse_binding(&l, spec).unwrap_err().to_string();
        assert!(e.contains(expect), "{spec}: {e}");
    }
    // The real keys still work, including QMK's short modifier forms.
    let k = |s: &str| qmk::key(&parse_binding(&l, s).unwrap().key);
    assert_eq!(k("KC_LT"), "KC_LABK");
    assert_eq!(k("<"), "KC_LABK");
    assert_eq!(k("C(KC_C)"), "LCTL(KC_C)");
    assert_eq!(k("S(KC_1)"), "LSFT(KC_1)");
    assert_eq!(k("CTL_T(KC_A)"), "MT(MOD_LCTL, KC_A)");
    assert_eq!(k("SFT_T(KC_A)"), "MT(MOD_LSFT, KC_A)");
    assert_eq!(k("KC_S"), "KC_S");
    assert_eq!(k("s"), "KC_S");
    assert_eq!(k("MEH_T(KC_BSPC)"), "MEH_T(KC_BSPC)");
    // Meh/Hyper are fine as a hold action.
    assert_eq!(parse_slot_action(&l, "MEH", Some(keyloom::model::Slot::Hold)).unwrap().0.code.as_deref(), Some("MEH_T"));
    assert_eq!(parse_slot_action(&l, "Hyper", Some(keyloom::model::Slot::Hold)).unwrap().0.code.as_deref(), Some("ALL_T"));
}

#[test]
fn hold_only_keys_round_trip() {
    let l = real_layout();
    let key = parse_binding(&l, "TAP_HOLD(_, KC_RCBR)").unwrap().key;
    assert!(key.tap.is_none() && key.hold.is_some());
    assert_eq!(qmk::key(&key), "TAP_HOLD(_, KC_RCBR)");
    assert!(parse_binding(&l, "TAP_HOLD(_, _)").is_err());
    // the real fixture layout has one: R.home.2 on Brd+Sys.
    assert_eq!(qmk_at(&l, 2, "R.home.2"), "TAP_HOLD(_, KC_RCBR)");
}

#[test]
fn set_key_replaces_the_whole_key_but_keeps_the_led_color() {
    let mut d = draft();
    edit(
        &mut d,
        json!([{"op": "set_label", "layer": 0, "key": "L.home.1", "label": "Copy"}, {"op": "set_color", "layer": 0, "key": "L.home.1", "color": "#ff0000"}]),
    );
    let w = edit(&mut d, json!([{"op": "set_key", "layer": 0, "key": "L.home.1", "action": "KC_B"}]));
    assert!(w.iter().any(|w| w.contains("custom label \"Copy\" removed")), "{w:?}");
    let k = &d.layers[0].keys[13];
    assert_eq!((k.custom_label.as_deref(), k.glow_color.as_deref()), (None, Some("#FF0000")));
}

#[test]
fn layer_titles_and_tags_are_checked() {
    let mut d = draft();
    for (title, expect) in [
        ("", "needs a title"),
        ("  ", "needs a title"),
        ("2", "layer number"),
        ("Base", "reserved"),
        ("all", "reserved"),
        ("main", "already exists"),
        ("A(b)", "can't contain"),
    ] {
        let e = fail(&mut d, json!([{"op": "add_layer", "title": title}]));
        assert!(e.contains(expect), "{title:?}: {e}");
        let e = fail(&mut d, json!([{"op": "rename_layer", "layer": 3, "title": title}]));
        assert!(e.contains(expect), "rename to {title:?}: {e}");
    }
    edit(&mut d, json!([{"op": "rename_layer", "layer": "Gaming", "title": "gaming"}]));
    assert_eq!(d.layers[3].title, "gaming", "changing only the case of a layer's own title is fine");
    let w = edit(&mut d, json!([{"op": "add_layer", "title": " WoW ", "tags": ["Gaming", " gaming", "raid"]}]));
    let wow = d.layers.last().unwrap();
    assert_eq!((wow.title.as_str(), wow.keyloom.tags.clone()), ("WoW", vec!["gaming".to_string(), "raid".to_string()]));
    assert!(w.iter().any(|w| w.contains("tag 'raid' has no meaning")), "{w:?}");
    assert!(fail(&mut d, json!([{"op": "set_key", "layer": "m", "key": "L.home.1", "action": "A"}]))
        .contains("matches several layers: Main, Mouse"));
}

// MARK: diff, guide, verify

#[test]
fn keys_of_a_new_layer_are_in_the_diff_and_guide() {
    let base = real_layout();
    let mut d = draft();
    edit(
        &mut d,
        json!([
            {"op": "add_layer", "title": "WoW", "tags": ["gaming"]},
            {"op": "set_key", "layer": "WoW", "key": "L.home.1", "action": "KC_A"},
            {"op": "set_key", "layer": "WoW", "key": "L.thumb.1", "action": "TG(WoW)"},
            {"op": "set_key", "layer": 0, "key": "L.thumb.1", "action": "TG(WoW)"}
        ]),
    );
    let dd = diff(&base, &d, DiffOptions::default());
    let keys: Vec<_> = dd.changes.iter().filter(|c| matches!(c, Change::KeyChanged { layer_title, .. } if layer_title == "WoW")).collect();
    assert_eq!(keys.len(), 2, "{:#?}", dd.changes);
    assert!(!dd.changes.iter().any(|c| matches!(c, Change::LayerMoved { .. })));
    let text = guide::render(&guide::guide(&base, &d));
    assert!(text.contains("Add a layer \"WoW\"") && text.contains("Tap: empty (transparent) → KC_A"), "{text}");
    // Applied in Oryx without the keys: they are still to do, not "unexpected".
    let mut half = base.clone();
    half.revision_id = "NEW".into();
    let mut empty = d.layers.last().unwrap().clone();
    empty.keys = vec![Key::default(); 52];
    empty.keyloom = Default::default();
    half.layers.push(empty);
    let v = guide::verify(&base, &d, &half);
    assert!(v.unexpected.is_empty(), "{:#?}", v.unexpected);
    assert_eq!(v.still_to_do.len(), 3);
}

#[test]
fn inserting_a_layer_is_not_a_move_and_a_real_move_is_one() {
    let base = real_layout();
    let mut d = draft();
    edit(&mut d, json!([{"op": "add_layer", "title": "Nav", "after": 0}]));
    let dd = diff(&base, &d, DiffOptions::default());
    assert_eq!(dd.changes.len(), 1, "{:#?}", dd.changes);
    let text = guide::render(&guide::guide(&base, &d));
    assert!(text.contains("then drag it right after \"Main\""), "{text}");
    assert!(text.contains("go by the layer titles"), "{text}");

    let mut d = draft();
    edit(&mut d, json!([{"op": "move_layer", "layer": "Gaming", "to": 1}]));
    let moves: Vec<_> =
        diff(&base, &d, DiffOptions::default()).changes.into_iter().filter(|c| matches!(c, Change::LayerMoved { .. })).collect();
    assert_eq!(moves.len(), 1, "{moves:#?}");
    assert!(guide::render(&guide::guide(&base, &d)).contains("Drag layer \"Gaming\" right after \"Main\" (it ends up as layer 1)"));

    // Removing a layer isn't a move of the ones after it either.
    let mut d = draft();
    edit(&mut d, json!([{"op": "remove_layer", "layer": "Gaming"}]));
    let dd = diff(&base, &d, DiffOptions::default());
    assert_eq!(dd.changes, vec![Change::LayerRemoved { position: 3, title: "Gaming".into() }]);
}

#[test]
fn a_renamed_and_shifted_layer_is_a_rename() {
    let base = real_layout();
    let mut d = draft();
    edit(&mut d, json!([{"op": "add_layer", "title": "Nav", "after": 0}, {"op": "rename_layer", "layer": "Sym+Num", "title": "Symbols"}]));
    let dd = diff(&base, &d, DiffOptions::default());
    assert!(!dd.changes.iter().any(|c| matches!(c, Change::LayerRemoved { .. })), "{:#?}", dd.changes);
    assert!(dd
        .changes
        .iter()
        .any(|c| matches!(c, Change::LayerChanged { field, from, .. } if field == "title" && from.as_deref() == Some("Sym+Num"))));
    assert!(!dd.changes.iter().any(|c| matches!(c, Change::KeyChanged { .. })), "{:#?}", dd.changes);
}

#[test]
fn renaming_a_layer_with_combos_changes_no_combo() {
    let mut base = draft();
    edit(&mut base, json!([{"op": "add_combo", "layer": "Sym+Num", "keys": ["L.home.3", "L.home.4"], "action": "KC_ESC"}]));
    let mut d = base.clone();
    edit(
        &mut d,
        json!([{"op": "rename_layer", "layer": "Sym+Num", "title": "Symbols"}, {"op": "move_layer", "layer": "Symbols", "to": 2}]),
    );
    let dd = diff(&base, &d, DiffOptions::default());
    assert!(
        !dd.changes.iter().any(|c| matches!(c, Change::ComboAdded { .. } | Change::ComboRemoved { .. } | Change::ComboChanged { .. })),
        "{:#?}",
        dd.changes
    );
    // And a removed combo names its layer by title, with the final number.
    edit(&mut d, json!([{"op": "remove_combo", "layer": "Symbols", "keys": ["L.home.3", "L.home.4"]}]));
    let text = guide::render(&guide::guide(&base, &d));
    assert!(text.contains("Remove the combo L.home.3 + L.home.4 on layer 2 \"Symbols\""), "{text}");
}

// MARK: validation and the layer graph

#[test]
fn tap_toggle_and_default_layer_exits() {
    // TT on base; the same key is transparent on the layer: tapping it again leaves.
    let mut d = draft();
    edit(&mut d, json!([{"op": "set_key", "layer": 0, "key": "L.thumb.1", "action": "TT(Gaming)"}]));
    assert!(!rules(&d).contains(&"layer-no-exit"), "{:?}", rules(&d));
    // A key there on the layer doesn't survive Oryx's editor (it keeps the position transparent),
    // so the way out stays.
    edit(&mut d, json!([{"op": "set_key", "layer": "Gaming", "key": "L.thumb.1", "action": "KC_A"}]));
    assert!(rules(&d).contains(&"oryx-clears-key") && !rules(&d).contains(&"layer-no-exit"), "{:?}", rules(&d));

    // TO in, only DF(0) on the layer: DF doesn't switch the layer off.
    let mut d = draft();
    edit(
        &mut d,
        json!([{"op": "set_key", "layer": 0, "key": "L.thumb.1", "action": "TO(Gaming)"}, {"op": "set_key", "layer": "Gaming", "key": "L.thumb.1", "action": "DF(0)"}]),
    );
    assert!(rules(&d).contains(&"layer-no-exit"));
    assert!(!rules(&d).contains(&"oryx-clears-key"), "TO isn't a key Oryx reserves the position for");
    // DF in needs DF out.
    let mut d = draft();
    edit(&mut d, json!([{"op": "set_key", "layer": 0, "key": "L.thumb.1", "action": "DF(Gaming)"}]));
    let f = validate(&d, &Context::default());
    assert!(f.iter().any(|f| f.rule == "layer-no-exit" && f.hint.contains("DF(0)")), "{f:#?}");
    edit(&mut d, json!([{"op": "set_key", "layer": "Gaming", "key": "L.thumb.1", "action": "DF(0)"}]));
    assert!(!rules(&d).contains(&"layer-no-exit"));
}

#[test]
fn combos_are_edges_in_the_layer_graph() {
    let mut d = draft();
    edit(
        &mut d,
        json!([
            {"op": "add_combo", "layer": 0, "keys": ["R.home.3", "R.home.4"], "action": "TG(Gaming)"},
            {"op": "add_combo", "layer": "Gaming", "keys": ["R.home.3", "R.home.4"], "action": "TG(Gaming)"}
        ]),
    );
    let r = rules(&d);
    assert!(!r.contains(&"layer-unreachable") && !r.contains(&"layer-no-exit"), "{r:?}");
    let s = explore::summary(&d);
    assert_eq!(s.layers[3].reached_by.as_deref(), Some("press R.home.4+R.home.3 together (combo, TG(3))"));
    // A combo pointing nowhere is reported.
    d.combos.push(json!({"keyIndices": [1, 2], "layerIdx": 0, "trigger": {"code": "TG", "layer": 9}}));
    assert!(validate(&d, &Context::default()).iter().any(|f| f.rule == "layer-ref-missing" && f.message.contains("layer 9")));
}

#[test]
fn auto_tag_and_gaming_tag_are_honoured() {
    let mut l = draft();
    l.settings.auto_mouse_layer = None;
    assert_eq!(rules(&l).iter().filter(|r| **r == "layer-unreachable").count(), 2, "Gaming and Mouse");
    edit(&mut l, json!([{"op": "tag_layer", "layer": "Mouse", "tags": ["mouse", "auto"]}]));
    let f = validate(&l, &Context::default());
    assert_eq!(f.iter().filter(|f| f.rule == "layer-unreachable").map(|f| f.layer).collect::<Vec<_>>(), [Some(3)]);
    assert_eq!(explore::summary(&l).layers[4].reached_by.as_deref(), Some("entered by the firmware (tagged auto)"));
    // A tag written by hand with other casing still counts.
    l.layers[3].keyloom.tags = vec!["Gaming".into()];
    l.layers[3].keys[0] = parse_binding(&l, "LT(1, KC_A)").unwrap().key;
    assert!(rules(&l).contains(&"gaming-tap-hold"));
    // A layer key without a layer (only possible in hand-edited files) is reported.
    l.layers[0].keys[1] = Key { tap: Some(Action { code: Some("TG".into()), ..Default::default() }), ..Default::default() };
    assert!(validate(&l, &Context::default()).iter().any(|f| f.rule == "layer-ref-missing" && f.message.contains("has no layer")));
}

// MARK: workspace

fn git(dir: &std::path::Path, args: &[&str]) -> String {
    let o = std::process::Command::new("git").arg("-C").arg(dir).args(args).output().unwrap();
    String::from_utf8_lossy(&o.stdout).to_string()
}

#[test]
fn config_file_keeps_comments_and_unknown_keys() {
    let dir = tempfile::tempdir().unwrap();
    let ws = Workspace::open(dir.path()).unwrap();
    std::fs::write(dir.path().join("config.toml"), "# my notes\n[render]\nglyphs = \"ascii\"\n\n[live]\nhighlight_color = \"#112233\"\n")
        .unwrap();
    ws.store_revision(&real_layout(), None).unwrap();
    ws.set_active("9qMJK").unwrap();
    let text = std::fs::read_to_string(dir.path().join("config.toml")).unwrap();
    assert!(text.contains("# my notes") && text.contains("glyphs = \"ascii\"") && text.contains("active_layout = \"9qMJK\""), "{text}");
    assert_eq!(ws.config().unwrap().live.highlight_color, "#112233");
}

#[test]
fn a_git_repo_keyloom_did_not_create_is_never_committed_to() {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-q"]);
    std::fs::write(dir.path().join("private.txt"), "mine").unwrap();
    let ws = Workspace::open(dir.path()).unwrap();
    ws.store_revision(&real_layout(), None).unwrap();
    assert_eq!(git(dir.path(), &["log", "--oneline", "--all"]).trim(), "", "no commits in the user's repo");
    // In its own workspace keyloom does commit, and only there.
    let own = tempfile::tempdir().unwrap();
    let ws = Workspace::open(own.path()).unwrap();
    ws.store_revision(&real_layout(), None).unwrap();
    if own.path().join(".git").exists() {
        assert!(git(own.path(), &["log", "--format=%s"]).contains("import 9qMJK@ZPjbXz"));
        assert!(!git(own.path(), &["ls-files"]).contains(".lock"));
    }
}

#[test]
fn stored_revisions_are_not_changed_by_file_imports() {
    let dir = tempfile::tempdir().unwrap();
    let ws = Workspace::open(dir.path()).unwrap();
    let mut first = real_layout();
    first.settings = Default::default();
    ws.store_revision(&first, None).unwrap();
    // A file import with settings doesn't touch the stored revision …
    let mut edited = real_layout();
    edited.settings.auto_mouse_layer = Some(2);
    assert_eq!(ws.store_revision(&edited, None).unwrap(), Stored::AlreadyThere);
    assert_eq!(ws.resolve(None).unwrap().settings.auto_mouse_layer, None);
    // … a fetch from Oryx fills them in once …
    let raw = json!({});
    edited.settings.auto_mouse_layer = Some(4);
    ws.store_revision(&edited, Some(&raw)).unwrap();
    assert_eq!(ws.resolve(None).unwrap().settings.auto_mouse_layer, Some(4));
    // … and never replaces them.
    edited.settings.auto_mouse_layer = Some(1);
    ws.store_revision(&edited, Some(&raw)).unwrap();
    assert_eq!(ws.resolve(None).unwrap().settings.auto_mouse_layer, Some(4));

    // An older revision imported later doesn't rename the layout.
    let mut older = real_layout();
    older.revision_id = "OLD".into();
    older.created_at = Some("2020-01-01 00:00:00 UTC".into());
    older.title = "Old title".into();
    ws.store_revision(&older, None).unwrap();
    assert_eq!(ws.meta("9qMJK").unwrap().title, "Voyager");

    // A layout.toml that can't be read is an error, not silently replaced.
    let meta = dir.path().join("layouts/9qMJK/layout.toml");
    let before = std::fs::read_to_string(&meta).unwrap();
    std::fs::write(&meta, "this is not toml [").unwrap();
    let mut another = real_layout();
    another.revision_id = "NEWER".into();
    assert!(ws.store_revision(&another, None).is_err());
    assert!(!dir.path().join("layouts/9qMJK/revisions/NEWER.json").exists());
    std::fs::write(&meta, before).unwrap();
}

#[test]
fn drafts_resolve_by_bare_name_and_names_are_checked() {
    let dir = tempfile::tempdir().unwrap();
    let ws = Workspace::open(dir.path()).unwrap();
    ws.store_revision(&real_layout(), None).unwrap();
    let rev = ws.resolve(None).unwrap();
    ws.create_draft("wow", &rev).unwrap();
    assert_eq!(ws.resolve(Some("wow")).unwrap().reference(), "draft:wow");
    assert_eq!(ws.resolve(Some("draft:wow")).unwrap().reference(), "draft:wow");
    let e = ws.resolve(Some("nope")).unwrap_err().to_string();
    assert!(e.contains("no layout or draft called 'nope'"), "{e}");
    let e = ws.create_draft("wow", &rev).unwrap_err().to_string();
    assert!(e.starts_with("a draft called 'wow' already exists"), "no workspace path in front: {e}");
    assert!(ws.create_draft("9qMJK", &rev).unwrap_err().to_string().contains("is the id of a layout"));
    // The lock is re-entrant within one workspace handle.
    let _a = ws.lock().unwrap();
    let _b = ws.lock().unwrap();
    ws.create_draft("second", &rev).unwrap();
}

// MARK: knowledge

#[test]
fn keycode_search_ranks_names_before_descriptions() {
    let codes = |q: &str| knowledge::search(q, 8).iter().map(|k| k.code.as_str()).collect::<Vec<_>>();
    let shift = codes("shift");
    assert!(shift.contains(&"KC_LEFT_SHIFT") && shift.contains(&"KC_RIGHT_SHIFT"), "{shift:?}");
    assert!(shift[..2].iter().all(|c| c.contains("SHIFT")), "{shift:?}");
    assert_eq!(codes("right shift").first(), Some(&"KC_RIGHT_SHIFT"));
    assert_eq!(codes("volume up").first(), Some(&"KC_AUDIO_VOL_UP"));
    assert_eq!(codes("Esc").first(), Some(&"KC_ESCAPE"));
    assert!(codes("zzzz").is_empty());
    // Keyboard-page volume keys are different keycodes: no longer aliases of the media keys.
    assert!(knowledge::lookup("KC_KB_MUTE").is_none());
    assert!(knowledge::lookup("KC_ESCAPE").unwrap().verified);
}

#[test]
fn guides_only_describe_what_keyloom_does() {
    for (name, text) in knowledge::guides() {
        assert!(!text.contains("`mouse`/`auto`"), "{name}: only the `auto` tag exempts a layer from reachability");
        for line in text.lines().filter(|l| l.contains("OSM(")) {
            assert!(line.contains("can't") || line.contains("rejected") || line.contains("not supported"), "{name} suggests OSM: {line}");
        }
    }
}

// MARK: round 2

#[test]
fn copying_a_layer_while_inserting_it_keeps_its_layer_keys_right() {
    let mut d = draft();
    edit(&mut d, json!([{"op": "add_layer", "title": "Copy", "after": 0, "copy_from": 0}]));
    assert_eq!(d.layers[1].title, "Copy");
    for layer in [0, 1] {
        assert_eq!(qmk_at(&d, layer, "L.thumb.2"), "LT(2, KC_SPACE)", "layer {layer} points at Sym+Num");
        assert_eq!(qmk_at(&d, layer, "R.thumb.2"), "MO(3)", "layer {layer} points at Brd+Sys");
    }
    // The source sits after the insert point.
    let mut d = draft();
    edit(&mut d, json!([{"op": "add_layer", "title": "Copy", "after": 0, "copy_from": "Mouse"}]));
    assert_eq!(d.layers[1].keys, d.layers[5].keys);
}

#[test]
fn op_json_is_strict() {
    let parse = |v: Value| serde_json::from_value::<Vec<Op>>(v).map_err(|e| e.to_string());
    assert!(parse(json!([{"op": "set_key", "layer": 0, "key": 1, "action": "A", "slot": "hold"}])).unwrap_err().contains("unknown field"));
    assert!(parse(json!([{"op": "set_slot", "layer": 0, "key": 1, "slot": "hold"}])).unwrap_err().contains("missing field `action`"));
    assert!(parse(json!([{"op": "set_slot", "layer": 0, "key": 1, "slot": "hold", "action": null}])).is_ok());
    assert!(parse(json!([{"op": "set_label", "layer": 0, "key": 1}])).is_err());
    assert!(parse(json!([{"op": "color_layer", "layer": 0}])).is_err());
    assert!(parse(json!([{"op": "clear_key", "layer": 0, "key": 1, "fill": "none"}])).is_ok());
    assert!(parse(json!([{"op": "add_layer", "title": "X", "tag": ["gaming"]}])).is_err());
}

#[test]
fn swapping_keys_takes_combos_along_and_clear_keeps_the_led_color() {
    let mut d = draft();
    edit(
        &mut d,
        json!([
            {"op": "add_combo", "layer": 0, "keys": ["L.home.3", "L.home.4"], "action": "KC_ESC"},
            {"op": "swap_keys", "layer": 0, "a": "L.home.4", "b": "R.home.4"},
            {"op": "set_color", "layer": 0, "key": "L.home.1", "color": "#FF0000"},
            {"op": "clear_key", "layer": 0, "key": "L.home.1", "to": "none"}
        ]),
    );
    assert_eq!(keyloom::ops::sorted_keys(&d.combos[0]), vec![15, 39], "the combo moved with its key");
    let k = &d.layers[0].keys[13];
    assert_eq!((qmk::key(k).as_str(), k.glow_color.as_deref()), ("KC_NO", Some("#FF0000")));
    for bad in ["red", "#12", "banana", "#12345g"] {
        assert!(fail(&mut d, json!([{"op": "color_layer", "layer": 0, "color": bad}])).contains("#rrggbb"), "{bad}");
        assert!(
            fail(&mut d, json!([{"op": "set_key", "layer": 0, "key": 1, "action": format!("RGB({bad})")}])).contains("#rrggbb"),
            "{bad}"
        );
        assert!(fail(&mut d, json!([{"op": "add_layer", "title": "C", "color": bad}])).contains("#rrggbb"), "{bad}");
    }
    let w = edit(&mut d, json!([{"op": "remove_layer", "layer": "Mouse"}]));
    assert!(w.iter().any(|w| w.contains("auto-mouse layer")), "{w:?}");
}

#[test]
fn slot_actions_and_aliases_of_families() {
    let l = real_layout();
    use keyloom::model::Slot;
    for slot in [Some(Slot::Tap), Some(Slot::DoubleTap), None] {
        for spec in ["Hyper", "Meh", "KC_MEH", "MEH_T"] {
            assert!(parse_slot_action(&l, spec, slot).is_err(), "{spec} in {slot:?}");
        }
    }
    assert!(parse_slot_action(&l, "LCTL_T", Some(Slot::Hold)).unwrap_err().to_string().contains("looks like a mod-tap"));
    assert!(parse_slot_action(&l, "TG(Gaming)", None).is_ok());
    for (spec, expect) in [("Color", "RGB(#rrggbb)"), ("One Shot Mod", "doesn't support"), ("Layer Tap", "LT(<layer>, KC_x)")] {
        let e = parse_binding(&l, spec).unwrap_err().to_string();
        assert!(e.contains(expect), "{spec}: {e}");
    }
    // Old QMK spellings still resolve.
    for (old, code) in [("KC_BSPACE", "KC_BSPC"), ("KC_PGDOWN", "KC_PGDN"), ("KC_SCOLON", "KC_SCLN"), ("KC_BRMU", "KC_BRIGHTNESS_UP")] {
        assert_eq!(knowledge::canonical(old).as_deref(), Some(code));
    }
    assert_eq!(knowledge::canonical("KC_LT").as_deref(), Some("KC_LABK"));
    assert_eq!(explore::find(&l, "LT").len(), 1);
    assert!(!explore::find(&l, "MT").is_empty());
}

#[test]
fn guide_orders_layer_steps_so_they_can_be_followed() {
    // A new layer goes right after a layer that itself moves, and another layer is renamed.
    let base = real_layout();
    let mut d = draft();
    edit(
        &mut d,
        json!([
            {"op": "move_layer", "layer": "Gaming", "to": 1},
            {"op": "add_layer", "title": "Nav", "after": "Gaming"},
            {"op": "rename_layer", "layer": "Sym+Num", "title": "Symbols"}
        ]),
    );
    let g = guide::guide(&base, &d);
    let steps: Vec<&str> = g.sections[0].steps.iter().map(|s| s.what.as_str()).collect();
    assert_eq!(steps.len(), 3, "{steps:#?}");
    assert_eq!(steps[0], "Rename layer 1 \"Sym+Num\" to \"Symbols\"");
    assert!(steps[1].starts_with("Drag layer \"Gaming\" right after \"Main\""), "{steps:#?}");
    assert!(steps[2].starts_with("Add a layer \"Nav\", then drag it right after \"Gaming\""), "{steps:#?}");
    // A layer filled with KC_NO is one step.
    let mut d = draft();
    edit(
        &mut d,
        json!([{"op": "add_layer", "title": "WoW", "fill": "none"}, {"op": "set_key", "layer": "WoW", "key": "L.home.1", "action": "A"}]),
    );
    let g = guide::guide(&base, &d);
    let wow = g.sections.iter().find(|s| s.heading.contains("WoW")).unwrap();
    assert_eq!(wow.steps.len(), 2, "{:#?}", wow.steps);
    assert!(wow.steps[1].what.starts_with("Set the other 51 keys to None (KC_NO)") && wow.steps[1].keys.len() == 51);
    // A renamed and moved placeholder layer is still the same layer.
    let mut d = draft();
    edit(&mut d, json!([{"op": "rename_layer", "layer": "Gaming", "title": "WoW"}, {"op": "move_layer", "layer": "WoW", "to": 1}]));
    let dd = diff(&base, &d, DiffOptions::default());
    assert!(!dd.changes.iter().any(|c| matches!(c, Change::LayerAdded { .. } | Change::LayerRemoved { .. })), "{:#?}", dd.changes);
}

#[test]
fn a_toggle_key_on_a_held_layer_is_a_way_out() {
    // TG(Gaming) lives on Brd+Sys (held with R.thumb.2). From Gaming, holding that thumb still
    // reaches it, as long as the thumb key falls through on Gaming.
    let mut d = draft();
    edit(&mut d, json!([{"op": "set_key", "layer": "Brd+Sys", "key": "R.thumb.1", "action": "TG(Gaming)"}]));
    let r = rules(&d);
    assert!(!r.contains(&"layer-no-exit") && !r.contains(&"layer-unreachable"), "{r:?}");
    // If Gaming overrides the thumb key, the toggle is out of reach again.
    edit(&mut d, json!([{"op": "set_key", "layer": "Gaming", "key": "R.thumb.2", "action": "KC_SPACE"}]));
    assert!(rules(&d).contains(&"layer-no-exit"));
    // A toggle in the hold slot leaves as well as it enters.
    let mut d = draft();
    edit(&mut d, json!([{"op": "set_slot", "layer": 0, "key": "L.thumb.1", "slot": "hold", "action": "TG(Gaming)"}]));
    assert!(!rules(&d).contains(&"layer-no-exit"), "{:?}", rules(&d));
}

#[test]
fn every_validation_rule_can_fire_and_be_switched_off() {
    let mut d = draft();
    d.layers[1].keys.pop(); // key-count
    d.layers[0].keys[2] = Key { tap: Some(Action { code: Some("KC_NOT_A_KEY".into()), ..Default::default() }), ..Default::default() }; // unknown-keycode
    d.layers[0].keys[3] =
        Key { tap: Some(Action { code: Some("MO".into()), layer: Some(42), ..Default::default() }), ..Default::default() }; // layer-ref-missing
    for i in 0..5 {
        d.layers.push(keyloom::model::Layer {
            title: format!("Extra{i}"),
            position: 5 + i,
            color: None,
            keyloom: Default::default(),
            keys: vec![Key::default(); 52],
            extra: Default::default(),
        });
    } // too-many-layers
    d.combos.push(json!({"keyIndices": [13, 14], "layerIdx": 0, "trigger": {"code": "KC_ESCAPE"}}));
    d.combos.push(json!({"keyIndices": [14, 13], "layerIdx": 0, "trigger": {"code": "KC_TAB"}})); // combo-duplicate
    d.combos.push(json!({"keyIndices": [13, 14, 15], "layerIdx": 0, "trigger": {"code": "KC_TAB"}})); // combo-overlap
    d.layers[1].keys[25] = Key { tap: Some(Action { code: Some("KC_A".into()), ..Default::default() }), ..Default::default() }; // mo-target-overrides
    d.layers[0].keys[49] = Key::default(); // Enter gone from base: essential-unreachable
    d.layers[1].keys[49] = Key::default();
    let ctx = Context { newest_revision: Some("NEWER".into()), ..Default::default() };
    let found = validate(&d, &ctx).iter().map(|f| f.rule).collect::<std::collections::BTreeSet<_>>();
    for rule in [
        "key-count",
        "unknown-keycode",
        "layer-ref-missing",
        "too-many-layers",
        "combo-duplicate",
        "combo-overlap",
        "essential-unreachable",
        "duplicate-in-layer",
        "mo-target-overrides",
        "layer-unreachable",
        "base-drift",
    ] {
        assert!(found.contains(rule), "{rule} didn't fire: {found:?}");
    }
    let ctx = Context { disabled: found.iter().map(|r| r.to_string()).collect(), ..ctx };
    assert!(validate(&d, &ctx).is_empty(), "every rule can be disabled");
}

#[test]
fn keymapp_database_import() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("keymapp.sqlite3");
    let conn = rusqlite::Connection::open(&db).unwrap();
    conn.execute("CREATE TABLE revision (revisionId TEXT NOT NULL UNIQUE, data BLOB DEFAULT NULL)", []).unwrap();
    let mut newer = real_layout().to_oryx();
    newer["user"] = json!({"name": "someone"});
    let mut older = newer.clone();
    older["revision"]["hashId"] = json!("OLDER");
    older["revision"]["createdAt"] = json!("2020-01-01 00:00:00 UTC");
    // Stored newest first, as a blob and as text, plus a row that isn't JSON.
    conn.execute("INSERT INTO revision VALUES (?1, ?2)", rusqlite::params!["ZPjbXz", json!({"layout": newer}).to_string().into_bytes()])
        .unwrap();
    conn.execute("INSERT INTO revision VALUES (?1, ?2)", rusqlite::params!["OLDER", json!({"layout": older}).to_string()]).unwrap();
    conn.execute("INSERT INTO revision VALUES (?1, ?2)", rusqlite::params!["junk", "not json"]).unwrap();
    drop(conn);
    let layouts = source::read_keymapp(&db).unwrap();
    assert_eq!(layouts.len(), 2);
    assert!(layouts.iter().all(|l| l.source.kind == "keymapp" && !l.oryx.layout.contains_key("user")));
    assert!(source::read_keymapp(&dir.path().join("missing.sqlite3")).is_err());
}

#[test]
fn drafts_need_an_imported_base_and_get_late_settings() {
    let dir = tempfile::tempdir().unwrap();
    let ws = Workspace::open(dir.path()).unwrap();
    let mut rev = real_layout();
    rev.settings = Default::default();
    // Not imported: no draft, nothing written outside the layout's folder.
    let e = ws.create_draft("x", &rev).unwrap_err().to_string();
    assert!(e.contains("isn't imported"), "{e}");
    let mut evil = rev.clone();
    evil.layout_id = "../../outside".into();
    assert!(ws.create_draft("y", &evil).is_err());
    assert!(!dir.path().join("layouts/9qMJK/drafts").exists());
    ws.store_revision(&rev, None).unwrap();
    ws.create_draft("wow", &ws.resolve(None).unwrap()).unwrap();
    // The auto-mouse layer arrives with a later fetch from Oryx: the draft gets it too.
    let mut fetched = real_layout();
    fetched.settings.auto_mouse_layer = Some(4);
    ws.store_revision(&fetched, Some(&json!({}))).unwrap();
    assert_eq!(ws.load_draft("wow").unwrap().settings.auto_mouse_layer, Some(4));
    let mut d = ws.load_draft("wow").unwrap();
    let batch = ops(json!([{"op": "set_key", "layer": 3, "key": "L.home.1", "action": "A"}]));
    apply_ops(&mut d, &batch).unwrap();
    ws.save_draft(&mut d, &batch).unwrap();
    assert_eq!(ws.undo_draft("wow").unwrap().settings.auto_mouse_layer, Some(4), "undo starts from the updated base");
}

// MARK: round 3

#[test]
fn odd_layouts_are_refused_for_editing_not_a_panic() {
    let mut short = draft();
    short.layers[3].keys.truncate(40);
    let e = fail(&mut short, json!([{"op": "set_key", "layer": 3, "key": "R.thumb.1", "action": "KC_A"}]));
    assert!(e.contains("\"Gaming\" has 40 keys"), "{e}");
    assert!(fail(&mut short, json!([{"op": "swap_keys", "layer": "all", "a": 0, "b": 51}])).contains("can't be edited"));
    let mut gapped = draft();
    gapped.layers[4].position = 7;
    let e = fail(&mut gapped, json!([{"op": "add_layer", "title": "N", "after": 0}]));
    assert!(e.contains("numbered [0, 1, 2, 3, 7]"), "{e}");
    // They can still be looked at and validated.
    assert!(rules(&short).contains(&"key-count"));
}

#[test]
fn a_dangling_reference_on_both_sides_is_not_a_change() {
    let mut base = real_layout();
    base.layers[0].keys[14].hold = Some(Action { code: Some("MO".into()), layer: Some(9), ..Default::default() });
    let mut d = base.clone();
    d.kind = Kind::Draft;
    d.draft = Some(DraftMeta { name: "t".into(), created: "x".into(), updated: "x".into(), notes: None });
    assert!(diff(&base, &d, DiffOptions::default()).is_empty());
    // A reference to a layer that was removed is still a change.
    let mut d = draft();
    edit(&mut d, json!([{"op": "remove_layer", "layer": "Sym+Num", "force": true}]));
    assert!(diff(&real_layout(), &d, DiffOptions::default())
        .changes
        .iter()
        .any(|c| matches!(c, Change::KeyChanged { position, slot, .. } if position == "L.thumb.2" && slot == "hold")));
}

#[test]
fn a_held_lower_layer_is_shadowed_by_the_toggled_layer() {
    // TG(Gaming) sits on Sym+Num (layer 1, held with L.thumb.2). Gaming (layer 3) is higher, so
    // while Gaming is on, its own keys win at every position where it isn't transparent.
    let mut d = draft();
    edit(
        &mut d,
        json!([
            {"op": "clear_key", "layer": "Gaming", "key": "L.thumb.2"},
            {"op": "set_key", "layer": "Sym+Num", "key": "R.num.0", "action": "TG(Gaming)"}
        ]),
    );
    assert!(!rules(&d).contains(&"layer-no-exit"), "Gaming is transparent at R.num.0: the toggle is reachable");
    // A key at R.num.0 on Gaming would hide the toggle, but Oryx's editor clears it.
    edit(&mut d, json!([{"op": "set_key", "layer": "Gaming", "key": "R.num.0", "action": "KC_A"}]));
    assert!(rules(&d).contains(&"oryx-clears-key") && !rules(&d).contains(&"layer-no-exit"), "{:?}", rules(&d));
    // Gaming's own key at L.thumb.2 does hide the hold, so Sym+Num (and its toggle) can't be reached.
    edit(&mut d, json!([{"op": "set_key", "layer": "Gaming", "key": "L.thumb.2", "action": "KC_SPACE"}]));
    assert!(rules(&d).contains(&"layer-no-exit"), "Gaming's own key now hides the hold");
}

#[test]
fn a_toggle_back_where_oryx_keeps_the_key_transparent_is_no_exit() {
    // As on a real gaming layout: TG(Gaming) on a held layer, and TG(Gaming) at the same position on
    // Gaming to leave again. Oryx's editor clears that second key when it shows the layer, and the
    // held layer's toggle isn't reachable from Gaming, so after opening it in Oryx there is no way out.
    let mut d = draft();
    edit(
        &mut d,
        json!([
            {"op": "set_key", "layer": "Gaming", "key": "L.thumb.2", "action": "KC_SPACE"},
            {"op": "set_key", "layer": "Sym+Num", "key": "R.num.0", "action": "TG(Gaming)"},
            {"op": "set_key", "layer": "Gaming", "key": "R.num.0", "action": "TG(Gaming)"}
        ]),
    );
    let f = validate(&d, &Context::default());
    assert!(f.iter().any(|f| f.rule == "oryx-clears-key" && f.key.as_deref() == Some("R.num.0") && f.message.contains("TG(3)")), "{f:#?}");
    assert!(f.iter().any(|f| f.rule == "layer-no-exit" && f.hint.contains("TO(0)")), "{f:#?}");
    // TO(0) at the same position is cleared just the same.
    edit(&mut d, json!([{"op": "set_key", "layer": "Gaming", "key": "R.num.0", "action": "TO(0)"}]));
    assert!(rules(&d).contains(&"layer-no-exit") && rules(&d).contains(&"oryx-clears-key"));
    // At another position it stays.
    edit(
        &mut d,
        json!([{"op": "clear_key", "layer": "Gaming", "key": "R.num.0"}, {"op": "set_key", "layer": "Gaming", "key": "R.top.0", "action": "TO(0)"}]),
    );
    assert!(!rules(&d).contains(&"layer-no-exit") && !rules(&d).contains(&"oryx-clears-key"), "{:?}", rules(&d));

    // MO reserves the position too, unless the layout has Oryx's `ambidexLT` option on.
    let mut d = draft();
    edit(
        &mut d,
        json!([{"op": "set_key", "layer": 0, "key": "L.thumb.1", "action": "MO(Gaming)"}, {"op": "set_key", "layer": "Gaming", "key": "L.thumb.1", "action": "KC_A"}]),
    );
    assert_eq!(explore::oryx_cleared(&d).len(), 1, "{:?}", explore::oryx_cleared(&d));
    d.oryx.revision.insert("config".into(), json!({"ambidexLT": true}));
    assert!(explore::oryx_cleared(&d).is_empty());
}

#[test]
fn selectors_follow_titles_and_hold_actions() {
    let l = real_layout();
    assert_eq!(explore::find(&l, "MO(sym)").len(), 1, "layer titles by prefix, as everywhere else");
    assert_eq!(explore::find(&l, "MO(1)").len(), 1);
    // Esc is a hold action on the base layer: key:Esc still finds that key.
    assert_eq!(keyloom::ops::resolve_key(&l, &json!("key:Esc")).unwrap(), 0);
    assert_eq!(keyloom::ops::resolve_key(&l, &json!("key:Space")).unwrap(), 25);
    // A hold-only Meh key is written the way it is shown.
    let key = parse_binding(&l, "TAP_HOLD(_, MEH_T)").unwrap().key;
    assert_eq!(qmk::key(&key), "TAP_HOLD(_, MEH_T)");
    assert_eq!(parse_binding(&l, "TAP_HOLD(_, Hyper)").unwrap().key.hold.unwrap().code.as_deref(), Some("ALL_T"));
}

#[test]
fn workspace_edge_cases_from_round_three() {
    let dir = tempfile::tempdir().unwrap();
    let ws = Workspace::open(dir.path()).unwrap();
    let mut rev = real_layout();
    rev.settings = Default::default();
    ws.store_revision(&rev, None).unwrap();
    // Ids that differ only in letter case are kept apart (or refused), never mixed up.
    assert!(ws.meta("9qmjk").is_err());
    assert!(ws.load_revision("9qMJK", "zpjbxz").is_err());
    let mut other = rev.clone();
    other.layout_id = "9QMJK".into();
    let mut same_rev = rev.clone();
    same_rev.revision_id = "ZPJBXZ".into();
    let insensitive = dir.path().join("LAYOUTS").exists();
    if insensitive {
        assert!(ws.store_revision(&other, None).unwrap_err().to_string().contains("only in letter case"));
        assert!(ws.store_revision(&same_rev, None).unwrap_err().to_string().contains("only in letter case"));
    }
    // A stray file among the drafts doesn't break listing.
    ws.create_draft("wow", &ws.resolve(None).unwrap()).unwrap();
    std::fs::write(dir.path().join("layouts/9qMJK/drafts/notes.json"), "{}").unwrap();
    std::fs::copy(dir.path().join("layouts/9qMJK/drafts/wow.json"), dir.path().join("layouts/9qMJK/drafts/copy.json")).unwrap();
    assert_eq!(ws.drafts().unwrap().iter().map(|d| d.name.as_str()).collect::<Vec<_>>(), ["wow"]);
    // The auto-mouse layer arrives late; the draft has renumbered its layers in the meantime.
    let mut d = ws.load_draft("wow").unwrap();
    let batch = ops(json!([{"op": "add_layer", "title": "Nav", "after": 0}]));
    apply_ops(&mut d, &batch).unwrap();
    ws.save_draft(&mut d, &batch).unwrap();
    let mut fetched = real_layout();
    fetched.settings.auto_mouse_layer = Some(4);
    ws.store_revision(&fetched, Some(&json!({}))).unwrap();
    let d = ws.load_draft("wow").unwrap();
    assert_eq!(d.settings.auto_mouse_layer, Some(5));
    assert_eq!(d.layer(5).unwrap().title, "Mouse");
}

// MARK: round 4

#[test]
fn renames_are_recognised_alongside_other_layer_changes() {
    let base = real_layout();
    // Rename + move a one-key layer while another layer is added.
    let mut d = draft();
    edit(
        &mut d,
        json!([
            {"op": "rename_layer", "layer": "Gaming", "title": "Games"},
            {"op": "move_layer", "layer": "Games", "to": 1},
            {"op": "add_layer", "title": "WoW"}
        ]),
    );
    let dd = diff(&base, &d, DiffOptions::default());
    assert!(!dd.changes.iter().any(|c| matches!(c, Change::LayerRemoved { .. } | Change::KeyChanged { .. })), "{:#?}", dd.changes);
    assert_eq!(dd.changes.iter().filter(|c| matches!(c, Change::LayerAdded { .. })).count(), 1);
    // Swapping two titles is two renames.
    let mut d = draft();
    edit(
        &mut d,
        json!([
            {"op": "rename_layer", "layer": 1, "title": "Tmp"},
            {"op": "rename_layer", "layer": 2, "title": "Sym+Num"},
            {"op": "rename_layer", "layer": 1, "title": "Brd+Sys"}
        ]),
    );
    let dd = diff(&base, &d, DiffOptions::default());
    assert_eq!(dd.changes.len(), 2, "{:#?}", dd.changes);
    assert!(dd.changes.iter().all(|c| matches!(c, Change::LayerChanged { field, .. } if field == "title")));
}

#[test]
fn find_understands_the_expressions_keyloom_prints() {
    let l = real_layout();
    let positions = |q: &str| explore::find(&l, q).iter().map(|h| format!("{}:{}", h.layer, h.position)).collect::<Vec<_>>();
    assert_eq!(positions("LT(1, KC_SPACE)"), ["0:L.thumb.2"]);
    assert_eq!(positions("MT(MOD_LCTL, KC_TAB)"), ["0:L.top.0"]);
    assert_eq!(positions("MEH_T(KC_BSPC)"), ["0:R.thumb.1"]);
    assert_eq!(positions("TAP_HOLD(KC_EQUAL, KC_ESCAPE)"), ["0:L.num.0"]);
    assert_eq!(positions("lctl_t(tab)"), ["0:L.top.0"], "any spelling the editor accepts");
    // Either hand for a bare modifier name.
    let alt = positions("Alt");
    assert!(alt.contains(&"0:L.bot.0".to_string()) && alt.contains(&"0:R.bot.1".to_string()), "{alt:?}");
    assert_eq!(positions("KC_LEFT_ALT"), ["0:L.bot.0"]);
    let pm = source::parse_value(
        &serde_json::from_str(
            &std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/example-EXMPL-exmpl1.json")).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(!explore::find(&pm, "LCTL(KC_LEFT)").is_empty(), "modified keys are found by their expression");
}

#[test]
fn a_revision_id_keeps_its_content() {
    let dir = tempfile::tempdir().unwrap();
    let ws = Workspace::open(dir.path()).unwrap();
    ws.store_revision(&real_layout(), None).unwrap();
    // Same id, different keys (e.g. an edited export): refused, the stored revision stays.
    let mut edited = real_layout();
    edited.layers[0].keys[13] = parse_binding(&edited, "KC_B").unwrap().key;
    let e = ws.store_revision(&edited, None).unwrap_err().to_string();
    assert!(e.contains("already imported with different content"), "{e}");
    assert_eq!(qmk_at(&ws.resolve(None).unwrap(), 0, "L.home.1"), "KC_A");
    // A draft exported in Oryx's shape doesn't carry its base revision's id.
    let d = ws.create_draft("wow", &ws.resolve(None).unwrap()).unwrap();
    assert_eq!(d.to_oryx()["revision"]["hashId"], "ZPjbXz-draft-wow");
    assert_eq!(ws.resolve(None).unwrap().to_oryx()["revision"]["hashId"], "ZPjbXz");
}

// MARK: round 5

#[test]
fn layer_tags_survive_the_round_trip_through_oryx() {
    let dir = tempfile::tempdir().unwrap();
    let ws = Workspace::open(dir.path()).unwrap();
    ws.store_revision(&real_layout(), None).unwrap();
    let mut d = ws.create_draft("wow", &ws.resolve(None).unwrap()).unwrap();
    let batch =
        ops(json!([{"op": "tag_layer", "layer": "Gaming", "tags": ["gaming"]}, {"op": "add_layer", "title": "WoW", "tags": ["gaming"]}]));
    apply_ops(&mut d, &batch).unwrap();
    ws.save_draft(&mut d, &batch).unwrap();
    // What comes back from Oryx after flashing has no tags.
    let mut flashed = d.clone();
    flashed.kind = Kind::Revision;
    flashed.draft = None;
    flashed.revision_id = "FLASHED".into();
    flashed.created_at = Some("2030-01-01 00:00:00 UTC".into());
    flashed.layers.iter_mut().for_each(|l| l.keyloom = Default::default());
    ws.store_revision(&flashed, None).unwrap();
    let stored = ws.resolve(None).unwrap();
    assert_eq!(stored.revision_id, "FLASHED");
    assert_eq!(stored.layers[3].keyloom.tags, ["gaming"]);
    assert_eq!(stored.layers[5].keyloom.tags, ["gaming"]);
    assert!(guide::verify(&real_layout(), &ws.load_draft("wow").unwrap(), &stored).matches);
}

#[test]
fn a_copy_in_front_of_its_original_is_an_added_layer() {
    let base = real_layout();
    let mut d = draft();
    edit(&mut d, json!([{"op": "add_layer", "title": "Sym2", "after": 0, "copy_from": "Sym+Num"}]));
    let dd = diff(&base, &d, DiffOptions::default());
    let added: Vec<_> =
        dd.changes.iter().filter_map(|c| if let Change::LayerAdded { title, .. } = c { Some(title.as_str()) } else { None }).collect();
    assert_eq!(added, ["Sym2"], "{:#?}", dd.changes.iter().filter(|c| !matches!(c, Change::KeyChanged { .. })).collect::<Vec<_>>());
    assert!(!dd.changes.iter().any(|c| matches!(c, Change::LayerChanged { .. } | Change::LayerRemoved { .. })));
}

#[test]
fn rename_steps_identify_the_layer_by_its_current_number() {
    let base = real_layout();
    let mut d = draft();
    edit(&mut d, json!([{"op": "remove_layer", "layer": "Gaming"}, {"op": "rename_layer", "layer": "Mouse", "title": "Gaming"}]));
    let text = guide::render(&guide::guide(&base, &d));
    assert!(text.contains("Rename layer 4 \"Mouse\" to \"Gaming\""), "{text}");
    assert!(text.contains("Delete layer \"Gaming\" (was position 3)"), "{text}");
}

#[test]
fn odd_action_specs_are_refused_clearly() {
    let l = real_layout();
    for spec in ["LCTL(TRNS)", "LSFT(NO)", "LCTL(LSFT(TRNS))", "LCTL(RGB(#ff0000))"] {
        assert!(parse_binding(&l, spec).unwrap_err().to_string().contains("needs a key to modify"), "{spec}");
    }
    assert!(parse_binding(&l, "LT(1, TRNS)").is_ok(), "a hold-only key is fine");
    let e = parse_slot_action(&l, "MEH_T(KC_A)", Some(keyloom::model::Slot::Hold)).unwrap_err().to_string();
    assert!(e.contains("whole key") && e.contains("hold slot"), "{e}");
    let e = parse_binding(&l, "LT(1, LCTL_T(KC_A))").unwrap_err().to_string();
    assert!(e.contains("whole key"), "{e}");
    // A hand-edited file with a modifier on a transparent key isn't hidden as plain transparent.
    let mut odd = real_layout();
    odd.layers[1].keys[13] = serde_json::from_value(json!({"tap": {"code": "KC_TRANSPARENT", "modifiers": {"leftCtrl": true}}})).unwrap();
    assert!(!odd.layers[1].keys[13].is_transparent());
    let mut d = draft();
    assert!(fail(&mut d, json!([{"op": "rename_layer", "layer": 3, "title": "a\tb"}])).contains("one line"));
    // An exported draft is marked and refused as a revision, whatever its ids.
    let mut d = draft();
    d.revision_id = "ZPjbXz".into();
    let exported = d.to_oryx();
    assert_eq!(exported["revision"]["keyloomDraft"], "t");
    assert!(source::parse_value(&exported).unwrap_err().to_string().contains("export of the keyloom draft 't'"));
    assert!(source::parse_value(&json!({"data": {"layout": exported}})).is_err());
}

// MARK: round 6

#[test]
fn editing_the_original_of_a_copied_layer_is_one_key_change() {
    let base = real_layout();
    // Against the base: the copy is an added layer, the original has one changed key.
    let mut d = draft();
    edit(&mut d, json!([{"op": "add_layer", "title": "SymOld", "copy_from": "Sym+Num"}]));
    let with_copy = d.clone();
    edit(&mut d, json!([{"op": "set_key", "layer": "Sym+Num", "key": "L.home.1", "action": "KC_F13"}]));
    let dd = diff(&base, &d, DiffOptions::default());
    let other: Vec<_> =
        dd.changes.iter().filter(|c| !matches!(c, Change::KeyChanged { layer_title, .. } if layer_title == "SymOld")).collect();
    assert_eq!(other.len(), 2, "{other:#?}");
    assert!(matches!(other[0], Change::LayerAdded { title, .. } if title == "SymOld"));
    assert!(matches!(other[1], Change::KeyChanged { layer_title, position, .. } if layer_title == "Sym+Num" && position == "L.home.1"));
    // Against the draft before the edit (what an edit result shows): just the key.
    let dd = diff(&with_copy, &d, DiffOptions::default());
    assert_eq!(dd.changes.len(), 1, "{:#?}", dd.changes);
    // The round 5 cases still hold: a title swap is two renames.
    let mut d = draft();
    edit(
        &mut d,
        json!([{"op": "rename_layer", "layer": 1, "title": "Tmp"}, {"op": "rename_layer", "layer": 2, "title": "Sym+Num"}, {"op": "rename_layer", "layer": 1, "title": "Brd+Sys"}]),
    );
    assert_eq!(diff(&base, &d, DiffOptions::default()).changes.len(), 2);
}

// MARK: round 7

#[test]
fn colors_compare_without_regard_to_case() {
    let mut d = draft();
    edit(
        &mut d,
        json!([{"op": "color_layer", "layer": "Gaming", "color": "#FF8800"}, {"op": "set_color", "layer": 0, "key": "L.home.1", "color": "#AABBCC"}]),
    );
    // What Oryx hands back after applying it, in upper case.
    let mut flashed = d.clone();
    flashed.kind = Kind::Revision;
    flashed.draft = None;
    flashed.layers[3].color = Some("#FF8800".into());
    flashed.layers[0].keys[13].glow_color = Some("#AABBCC".into());
    assert!(diff(&flashed, &d, DiffOptions::default()).is_empty());
    assert!(guide::verify(&real_layout(), &d, &flashed).matches);
    // A different color is still a change.
    flashed.layers[3].color = Some("#00FF00".into());
    assert_eq!(diff(&flashed, &d, DiffOptions::default()).changes.len(), 1);
}

// MARK: round 8

#[test]
fn a_missing_color_or_title_on_an_added_layer_is_still_to_do() {
    let base = real_layout();
    let mut d = draft();
    edit(
        &mut d,
        json!([
            {"op": "add_layer", "title": "WoW", "color": "#ff0000"},
            {"op": "set_key", "layer": "WoW", "key": "L.home.2", "action": "KC_S"},
            {"op": "set_key", "layer": "WoW", "key": "L.home.3", "action": "KC_D"},
            {"op": "set_key", "layer": "WoW", "key": "L.top.2", "action": "KC_W"},
            {"op": "set_key", "layer": 0, "key": "L.thumb.1", "action": "TG(WoW)"}
        ]),
    );
    let mut applied = d.clone();
    applied.kind = Kind::Revision;
    applied.draft = None;
    applied.revision_id = "NEW".into();
    // Everything was done in Oryx except the layer's color.
    let mut no_color = applied.clone();
    no_color.layers[5].color = None;
    let v = guide::verify(&base, &d, &no_color);
    assert!(v.unexpected.is_empty(), "{:#?}", v.unexpected);
    assert!(matches!(v.still_to_do.as_slice(), [Change::LayerChanged { field, .. }] if field == "color"), "{:#?}", v.still_to_do);
    // Or the layer got another name in Oryx.
    let mut other_name = applied.clone();
    other_name.layers[5].title = "Wow2".into();
    let v = guide::verify(&base, &d, &other_name);
    assert!(v.unexpected.is_empty(), "{:#?}", v.unexpected);
    assert!(matches!(v.still_to_do.as_slice(), [Change::LayerChanged { field, .. }] if field == "title"), "{:#?}", v.still_to_do);
    // A key the draft didn't touch is still unexpected.
    let mut stray = applied.clone();
    stray.layers[1].keys[0] = parse_binding(&stray, "KC_F24").unwrap().key;
    assert_eq!(guide::verify(&base, &d, &stray).unexpected.len(), 1);
}

// MARK: round 9

#[test]
fn a_reused_title_does_not_outrank_the_layers_content() {
    let base = real_layout();
    // Delete Sym+Num, give its name to Brd+Sys, change one key there.
    let mut d = draft();
    edit(
        &mut d,
        json!([
            {"op": "remove_layer", "layer": "Sym+Num", "force": true},
            {"op": "rename_layer", "layer": "Brd+Sys", "title": "Sym+Num"},
            {"op": "set_key", "layer": "Sym+Num", "key": "L.top.1", "action": "KC_F13"}
        ]),
    );
    let dd = diff(&base, &d, DiffOptions::default());
    assert!(dd.changes.iter().any(|c| matches!(c, Change::LayerRemoved { position: 1, .. })), "{:#?}", dd.changes);
    assert!(dd
        .changes
        .iter()
        .any(|c| matches!(c, Change::LayerChanged { field, from, .. } if field == "title" && from.as_deref() == Some("Brd+Sys"))));
    let on_layer = dd.changes.iter().filter(|c| matches!(c, Change::KeyChanged { layer_title, .. } if layer_title == "Sym+Num")).count();
    assert_eq!(on_layer, 1, "{:#?}", dd.changes);
    // Swap two titles and change a key on each: two renames, two key changes.
    let mut d = draft();
    edit(
        &mut d,
        json!([
            {"op": "rename_layer", "layer": 1, "title": "Tmp"},
            {"op": "rename_layer", "layer": 2, "title": "Sym+Num"},
            {"op": "rename_layer", "layer": 1, "title": "Brd+Sys"},
            {"op": "set_key", "layer": 1, "key": "L.top.1", "action": "KC_F13"},
            {"op": "set_key", "layer": 2, "key": "L.top.1", "action": "KC_F14"}
        ]),
    );
    let dd = diff(&base, &d, DiffOptions::default());
    assert_eq!(dd.changes.len(), 4, "{:#?}", dd.changes);
    // A layer that keeps its title but is rewritten is still the same layer.
    let mut d = draft();
    let rewrite: Vec<Value> = (0..40).map(|i| json!({"op": "set_key", "layer": "Sym+Num", "key": i, "action": "KC_F13"})).collect();
    edit(&mut d, Value::Array(rewrite));
    let dd = diff(&base, &d, DiffOptions::default());
    assert!(
        dd.changes.iter().all(|c| matches!(c, Change::KeyChanged { layer_title, .. } if layer_title == "Sym+Num")),
        "{:#?}",
        &dd.changes[..2]
    );
}

#[test]
fn copying_a_layer_with_fill_none_closes_its_transparent_keys() {
    let mut d = draft();
    edit(
        &mut d,
        json!([{"op": "set_label", "layer": "Gaming", "key": "L.home.1", "label": "strafe"}, {"op": "add_layer", "title": "WoW", "copy_from": "Gaming", "fill": "none", "tags": ["gaming"]}]),
    );
    let wow = d.layers.last().unwrap();
    assert!(wow.keys.iter().all(|k| !k.is_transparent()));
    assert_eq!(qmk_at(&d, 5, "L.thumb.2"), "KC_SPACE", "what the source binds is copied");
    assert_eq!(wow.keys[13].custom_label.as_deref(), Some("strafe"));
    assert!(!rules(&d).contains(&"gaming-home-row-mods"));
    // Without fill, a copy is an exact copy.
    let mut d = draft();
    edit(&mut d, json!([{"op": "add_layer", "title": "Copy", "copy_from": "Gaming"}]));
    assert_eq!(d.layers[5].keys, d.layers[3].keys);
}

// MARK: round 10

#[test]
fn a_new_layer_left_at_the_end_is_still_to_do_not_unexpected() {
    let base = real_layout();
    let mut d = draft();
    edit(
        &mut d,
        json!([
            {"op": "add_layer", "title": "WoW", "after": "Gaming", "fill": "none"},
            {"op": "set_key", "layer": "WoW", "key": "L.home.2", "action": "KC_S"},
            {"op": "set_key", "layer": 0, "key": "L.thumb.1", "action": "TG(WoW)"}
        ]),
    );
    // In Oryx everything was done except dragging the new layer up: it is still the last one.
    let mut applied = d.clone();
    applied.kind = Kind::Revision;
    applied.draft = None;
    applied.revision_id = "NEW".into();
    let mut as_draft = applied.clone();
    as_draft.kind = Kind::Draft;
    as_draft.draft = d.draft.clone();
    edit(&mut as_draft, json!([{"op": "move_layer", "layer": "WoW", "to": 5}]));
    let mut imported = as_draft.clone();
    imported.kind = Kind::Revision;
    imported.draft = None;
    assert_eq!(imported.layers[5].title, "WoW");
    let v = guide::verify(&base, &d, &imported);
    assert!(v.unexpected.is_empty(), "{:#?}", v.unexpected);
    assert!(matches!(v.still_to_do.as_slice(), [Change::LayerMoved { .. }]), "{:#?}", v.still_to_do);
    // The remaining guide is one drag step that gets to the draft's order.
    let g = guide::guide(&imported, &d);
    let steps: Vec<&str> = g.sections.iter().flat_map(|s| &s.steps).map(|s| s.what.as_str()).collect();
    assert_eq!(steps.len(), 1, "{steps:#?}");
    assert!(steps[0].starts_with("Drag layer"), "{steps:#?}");
    // A draft that doesn't touch the order still reports a reorder in Oryx as unexpected.
    let mut plain = draft();
    edit(&mut plain, json!([{"op": "set_key", "layer": 3, "key": "L.home.2", "action": "KC_S"}]));
    let mut reordered = plain.clone();
    edit(&mut reordered, json!([{"op": "move_layer", "layer": "Mouse", "to": 1}]));
    reordered.kind = Kind::Revision;
    reordered.draft = None;
    assert!(guide::verify(&base, &plain, &reordered).unexpected.iter().any(|c| matches!(c, Change::LayerMoved { .. })));
}
