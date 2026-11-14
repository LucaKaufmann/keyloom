//! Import, store, render. Fixtures are real Oryx answers (no network in tests).
//! Golden renders: run with `KEYLOOM_BLESS=1` to (re)write `tests/golden/*.txt`, then review the diff.

use keyloom::model::{Kind, Layout};
use keyloom::render::{self, AsciiOptions, Style};
use keyloom::source;
use keyloom::workspace::{Stored, Workspace};
use serde_json::Value;
use std::path::PathBuf;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

fn fixture(name: &str) -> (Value, Layout) {
    let raw: Value = serde_json::from_str(&std::fs::read_to_string(fixture_path(name)).unwrap()).unwrap();
    let layout = source::parse_value(&raw).unwrap();
    (raw, layout)
}

fn real_layout() -> Layout {
    fixture("oryx-9qMJK-ZPjbXz.json").1
}

#[test]
fn imports_graphql_answer() {
    let l = real_layout();
    assert_eq!(l.reference(), "9qMJK@ZPjbXz");
    assert_eq!(l.geometry, "voyager");
    assert_eq!(l.kind, Kind::Revision);
    let titles: Vec<_> = l.layers.iter().map(|x| x.title.as_str()).collect();
    assert_eq!(titles, ["Main", "Sym+Num", "Brd+Sys", "Gaming", "Mouse"]);
    assert!(l.layers.iter().all(|x| x.keys.len() == 52));
}

#[test]
fn nothing_oryx_sends_is_lost() {
    // Every non-null leaf in the raw answer must survive into our model and back out of to_oryx().
    fn leaves(v: &Value, path: String, out: &mut Vec<(String, Value)>) {
        match v {
            Value::Object(m) => m.iter().for_each(|(k, v)| leaves(v, format!("{path}/{k}"), out)),
            Value::Array(a) => a.iter().enumerate().for_each(|(i, v)| leaves(v, format!("{path}/{i}"), out)),
            Value::Null => {}
            // Oryx sends all modifier flags; false ones carry no information.
            Value::Bool(false) if path.contains("/modifiers/") => {}
            other => out.push((path, other.clone())),
        }
    }
    for name in ["oryx-9qMJK-ZPjbXz.json", "example-EXMPL-exmpl1.json"] {
        let (raw, layout) = fixture(name);
        let mut original = Vec::new();
        let mut original_layout = raw["data"]["layout"].clone();
        // Layers are re-sorted by position; sort the original the same way before comparing.
        if let Some(layers) = original_layout["revision"]["layers"].as_array_mut() {
            layers.sort_by_key(|l| l["position"].as_i64());
        }
        leaves(&original_layout, String::new(), &mut original);
        let mut back = Vec::new();
        leaves(&layout.to_oryx(), String::new(), &mut back);
        let missing: Vec<_> = original.iter().filter(|x| !back.contains(x)).collect();
        assert!(missing.is_empty(), "{name}: lost {:?}", &missing[..missing.len().min(5)]);
    }
}

#[test]
fn layout_file_round_trips() {
    let l = real_layout();
    let text = keyloom::workspace::pretty(&l);
    let back: Layout = serde_json::from_str(&text).unwrap();
    assert_eq!(back, l);
    assert_eq!(source::parse_value(&serde_json::from_str(&text).unwrap()).unwrap(), l);
}

#[test]
fn reads_keymapp_and_bare_shapes() {
    let (raw, l) = fixture("oryx-9qMJK-ZPjbXz.json");
    let mut keymapp_like = serde_json::json!({"layout": raw["data"]["layout"].clone()});
    keymapp_like["layout"]["user"] = serde_json::json!({"name": "someone", "pictureUrl": "https://example.invalid"});
    let from_keymapp = source::parse_value(&keymapp_like).unwrap();
    assert_eq!(from_keymapp.layers, l.layers);
    assert!(!from_keymapp.oryx.layout.contains_key("user"), "personal data is dropped");
    let bare = source::parse_value(&raw["data"]["layout"]).unwrap();
    assert_eq!(bare.layers, l.layers);
}

#[test]
fn qmk_names_of_the_real_layouts_keys() {
    let l = real_layout();
    let view = render::layer_view(&l, 0).unwrap();
    let qmk = |pos: &str| view.keys.iter().find(|k| k.position == pos).unwrap().qmk.clone();
    assert_eq!(qmk("L.num.0"), "TAP_HOLD(KC_EQUAL, KC_ESCAPE)");
    assert_eq!(qmk("L.top.0"), "MT(MOD_LCTL, KC_TAB)");
    assert_eq!(qmk("L.thumb.2"), "LT(1, KC_SPACE)");
    assert_eq!(qmk("R.thumb.2"), "MO(2)");
    assert_eq!(qmk("R.thumb.1"), "MEH_T(KC_BSPC)");
    assert_eq!(qmk("R.home.0"), "MT(MOD_RSFT, KC_QUOTE)");
    // Same names as Oryx's own generated keymap.c (docs/oryx-encodings.md).
}

#[test]
fn blank_keys_are_transparent() {
    let l = real_layout();
    let gaming = render::layer_view(&l, 3).unwrap();
    assert_eq!(gaming.keys.iter().filter(|k| k.transparent).count(), 51);
    let space = gaming.keys.iter().find(|k| k.position == "L.thumb.2").unwrap();
    assert_eq!((space.qmk.as_str(), space.transparent), ("KC_SPACE", false));
    // A transparent key shows what the base layer does there.
    let q = gaming.keys.iter().find(|k| k.position == "L.top.1").unwrap();
    assert_eq!(q.legend, "Q");
}

#[test]
fn layer_summary() {
    let layers = keyloom::summary::layers(&real_layout());
    let reached: Vec<Vec<String>> = layers.iter().map(|l| l.reached_by.iter().map(|r| r.describe()).collect()).collect();
    assert_eq!(reached[1], ["hold L.thumb.2 on Main (MO)"]);
    assert_eq!(reached[2], ["hold R.thumb.2 on Main (MO)"]);
    assert!(reached[3].is_empty(), "Gaming is unreachable");
    assert_eq!(layers[3].bound, 1);
}

#[test]
fn workspace_stores_immutable_revisions() {
    let dir = tempfile::tempdir().unwrap();
    let ws = Workspace::open(dir.path()).unwrap();
    assert!(ws.layouts().unwrap().is_empty());
    let (raw, l) = fixture("oryx-9qMJK-ZPjbXz.json");
    assert_eq!(ws.store_revision(&l, Some(&raw)).unwrap(), Stored::New);
    assert_eq!(ws.store_revision(&l, None).unwrap(), Stored::AlreadyThere);

    // An older revision imported later doesn't become the newest.
    let mut older = l.clone();
    older.revision_id = "6aw0An".into();
    older.created_at = Some("2025-08-26 08:34:17 UTC".into());
    older.layers.truncate(4);
    assert_eq!(ws.store_revision(&older, None).unwrap(), Stored::New);
    let meta = ws.meta("9qMJK").unwrap();
    assert_eq!(meta.revisions.len(), 2);
    assert_eq!(meta.newest().unwrap().id, "ZPjbXz");
    // A revision without an Oryx date (e.g. from a file) counts as created when imported.
    let mut undated = l.clone();
    undated.revision_id = "fromfile".into();
    undated.created_at = None;
    ws.store_revision(&undated, None).unwrap();
    assert_eq!(ws.meta("9qMJK").unwrap().newest().unwrap().id, "fromfile");
    assert_eq!(ws.resolve(None).unwrap().revision_id, "fromfile");

    // Resolution: active layout (set by the first import), newest revision, explicit revision.
    assert_eq!(ws.resolve(Some("9qMJK@6aw0An")).unwrap().layers.len(), 4);
    assert_eq!(ws.resolve(Some("@6aw0An")).unwrap().revision_id, "6aw0An");
    assert!(ws.resolve(Some("nope")).is_err());
    assert!(ws.resolve(Some("../etc")).is_err(), "ids are validated before touching the disk");

    // A stored revision file is never rewritten.
    let path = dir.path().join("layouts/9qMJK/revisions/ZPjbXz.json");
    let before = std::fs::read_to_string(&path).unwrap();
    let mut changed = l.clone();
    changed.title = "changed".into();
    ws.store_revision(&changed, None).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), before);

    // Raw answers go to the uncommitted cache.
    assert!(dir.path().join("cache/oryx/9qMJK-ZPjbXz.json").is_file());
    if dir.path().join(".git").exists() {
        let log = std::process::Command::new("git").arg("-C").arg(dir.path()).args(["log", "--format=%s"]).output().unwrap();
        let log = String::from_utf8_lossy(&log.stdout);
        assert!(log.contains("import 9qMJK@ZPjbXz (oryx)"), "{log}");
    }
}

#[test]
fn golden_renders() {
    let l = real_layout();
    let bless = std::env::var_os("KEYLOOM_BLESS").is_some();
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    for (name, layer, style) in [
        ("9qMJK-main-legend", 0, Style::Legend),
        ("9qMJK-main-codes", 0, Style::Codes),
        ("9qMJK-symnum-legend", 1, Style::Legend),
        ("9qMJK-mouse-codes", 4, Style::Codes),
    ] {
        let view = render::layer_view(&l, layer).unwrap();
        let text = render::ascii(&view, &AsciiOptions { style, ..Default::default() });
        let path = dir.join(format!("{name}.txt"));
        if bless {
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(&path, &text).unwrap();
        } else {
            let expected = std::fs::read_to_string(&path).unwrap_or_else(|_| panic!("missing {name}.txt; run with KEYLOOM_BLESS=1"));
            assert_eq!(text, expected, "{name} changed; review and re-bless");
        }
    }
}
