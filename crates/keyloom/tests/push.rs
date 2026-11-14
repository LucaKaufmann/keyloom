//! Pushing a draft to a new Oryx layout, against an in-memory Oryx (no network).

use keyloom::model::{Kind, Layout};
use keyloom::ops::{apply_ops, Op};
use keyloom::push::fake::FakeOryx;
use keyloom::push::{push_into, push_new, push_new_owned, PushError};
use keyloom::source;
use serde_json::{json, Value};
use std::path::PathBuf;

fn fixture(name: &str) -> Layout {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name);
    source::parse_value(&serde_json::from_str::<Value>(&std::fs::read_to_string(path).unwrap()).unwrap()).unwrap()
}

/// The real fixture (auto-mouse layer 4) and a draft of it with the given edits.
fn base_and_draft(ops: Value) -> (Layout, Layout) {
    let mut base = fixture("oryx-9qMJK-ZPjbXz.json");
    base.settings.auto_mouse_layer = Some(4);
    let mut draft = base.clone();
    draft.kind = Kind::Draft;
    draft.draft = Some(keyloom::model::DraftMeta { name: "wow".into(), created: "x".into(), updated: "x".into(), notes: None });
    let ops: Vec<Op> = serde_json::from_value(ops).unwrap();
    apply_ops(&mut draft, &ops).unwrap();
    (base, draft)
}

fn oryx_with(base: &Layout) -> FakeOryx {
    let mut oryx = FakeOryx::default();
    oryx.add_compiled(base);
    oryx
}

/// What the fake holds for a layout, as keyloom reads it.
fn stored(oryx: &FakeOryx, id: &str) -> Layout {
    source::parse_value(&json!({ "data": { "layout": oryx.layouts[id] } })).unwrap()
}

#[test]
fn a_draft_with_a_new_layer_lands_in_a_new_layout() {
    let (base, draft) = base_and_draft(json!([
        {"op": "add_layer", "title": "WoW", "after": "Gaming", "fill": "none"},
        {"op": "set_key", "layer": "Main", "key": "L.thumb.1", "action": "TG(WoW)"},
        {"op": "set_key", "layer": "WoW", "key": "L.thumb.1", "action": "TG(WoW)"},
        {"op": "set_key", "layer": "WoW", "key": "L.home.1", "action": "KC_A"},
    ]));
    let mut oryx = oryx_with(&base);
    let before = oryx.layouts[&base.layout_id].clone();

    let p = push_new(&mut oryx, &draft, Some(&base)).unwrap();
    assert!(p.verified && p.differences.is_empty() && p.warnings.is_empty(), "{p:?}");
    assert_eq!(p.copied_from, "9qMJK@ZPjbXz");
    assert_eq!((p.layers_added, p.layers_removed), (1, 0));
    assert!(p.layout_id != base.layout_id && p.url.ends_with(&format!("/voyager/layouts/{}/latest/0", p.layout_id)));

    // The user's layout is untouched; the new one has the draft's layers.
    assert_eq!(oryx.layouts[&base.layout_id], before);
    let new = stored(&oryx, &p.layout_id);
    assert_eq!(
        new.layers.iter().map(|l| l.title.as_str()).collect::<Vec<_>>(),
        draft.layers.iter().map(|l| l.title.as_str()).collect::<Vec<_>>()
    );
    for (a, b) in new.layers.iter().zip(&draft.layers) {
        assert_eq!(a.keys, b.keys, "layer {}", b.title);
    }
    // The auto-mouse layer moved with its layer (the inserted layer shifted it).
    let auto: Vec<i64> = new.layers.iter().filter(|l| l.extra.get("automouse") == Some(&json!(true))).map(|l| l.position).collect();
    assert_eq!(Some(auto[0]), draft.settings.auto_mouse_layer);
    assert_eq!(auto.len(), 1);
    // Unchanged layers aren't rewritten: only the layers from the insertion point on, plus Main.
    assert!(p.layers_written < draft.layers.len(), "{p:?}");
    assert!(oryx.calls.iter().all(|c| c != "compileRevision"));
}

#[test]
fn a_removed_layer_is_deleted_in_the_copy() {
    let (base, draft) = base_and_draft(json!([{"op": "remove_layer", "layer": "Gaming", "force": true}]));
    let mut oryx = oryx_with(&base);
    let p = push_new(&mut oryx, &draft, Some(&base)).unwrap();
    assert!(p.verified, "{p:?}");
    assert_eq!(p.layers_removed, 1);
    assert_eq!(stored(&oryx, &p.layout_id).layers.len(), base.layers.len() - 1);
}

#[test]
fn falls_back_to_the_default_layout_when_oryx_does_not_have_the_base() {
    let (base, draft) = base_and_draft(json!([{"op": "set_key", "layer": "Main", "key": "L.home.1", "action": "KC_X"}]));
    let mut oryx = FakeOryx::default();
    let mut default = fixture("example-EXMPL-exmpl1.json");
    default.layout_id = "default".into();
    oryx.add_compiled(&default);
    let p = push_new(&mut oryx, &draft, Some(&base)).unwrap();
    assert!(p.verified, "{p:?}");
    assert_eq!(p.copied_from, "default@exmpl1");
    assert!(p.warnings.iter().any(|w| w.contains("ZSA's default layout")), "{:?}", p.warnings);
    assert_eq!(stored(&oryx, &p.layout_id).layers.len(), draft.layers.len());
}

#[test]
fn a_failure_part_way_names_the_incomplete_layout() {
    let (base, draft) = base_and_draft(json!([{"op": "set_key", "layer": "Main", "key": "L.home.1", "action": "KC_X"}]));
    let mut oryx = oryx_with(&base);
    oryx.fail_on = Some("updateLayer");
    let e = push_new(&mut oryx, &draft, Some(&base)).unwrap_err();
    assert!(matches!(&e, PushError::Partial { url, .. } if url.contains("/layouts/new")), "{e}");
    assert!(e.to_string().contains("incomplete"), "{e}");
}

#[test]
fn changed_combos_are_reported_not_written() {
    let (base, mut draft) = base_and_draft(json!([]));
    draft.combos.push(json!({"name": "esc", "layerIdx": 0, "keyIndices": [1, 2], "trigger": {"tap": {"code": "KC_ESCAPE"}}}));
    let mut oryx = oryx_with(&base);
    let p = push_new(&mut oryx, &draft, Some(&base)).unwrap();
    assert!(p.warnings.iter().any(|w| w.contains("combos")), "{:?}", p.warnings);
}

/// The user's own layout on Oryx, logged in.
fn own(base: &Layout) -> FakeOryx {
    let mut oryx = oryx_with(base);
    oryx.owned.insert(base.layout_id.clone());
    oryx.login = true;
    oryx
}

const WOW: &str = r#"[
    {"op": "add_layer", "title": "WoW", "after": "Gaming", "fill": "none"},
    {"op": "set_key", "layer": "Main", "key": "L.thumb.1", "action": "TG(WoW)"},
    {"op": "set_key", "layer": "WoW", "key": "L.thumb.1", "action": "TG(WoW)"}
]"#;

#[test]
fn into_my_own_layout_makes_an_uncompiled_revision_and_keeps_the_compiled_one() {
    let (base, draft) = base_and_draft(serde_json::from_str(WOW).unwrap());
    let mut oryx = own(&base);
    let compiled = oryx.layouts[&base.layout_id].clone();

    let p = push_into(&mut oryx, &draft, Some(&base), &base.layout_id, false).unwrap();
    assert!(p.verified && p.warnings.is_empty() && !p.new_layout, "{p:?}");
    assert_eq!((p.layout_id.as_str(), p.copied_from.as_str(), p.layers_added), ("9qMJK", "9qMJK@ZPjbXz", 1));
    assert_eq!(oryx.layouts.len(), 1, "no separate layout was made");
    // The compiled revision is kept as it was; the new one isn't compiled.
    assert_eq!(oryx.frozen["ZPjbXz"], compiled);
    assert!(oryx.layouts["9qMJK"]["revision"]["qmkVersion"].is_null() && p.revision_id != "ZPjbXz");
    assert!(stored(&oryx, "9qMJK").layers.iter().any(|l| l.title == "WoW"));

    // Pushing the same draft again, or after more edits, goes into the same uncompiled revision.
    let again = push_into(&mut oryx, &draft, Some(&base), &base.layout_id, false).unwrap();
    assert_eq!((again.revision_id, again.layers_written, again.layers_added), (p.revision_id, 0, 0));
}

#[test]
fn into_refuses_to_lose_changes_made_on_oryx() {
    let (base, draft) = base_and_draft(serde_json::from_str(WOW).unwrap());

    // The layout moved on to a newer compiled revision than the draft's base.
    let mut newer = base.clone();
    newer.revision_id = "NEWER1".into();
    let mut oryx = own(&newer);
    let before = oryx.layouts.clone();
    let e = push_into(&mut oryx, &draft, Some(&base), &base.layout_id, false).unwrap_err();
    assert!(matches!(e, PushError::Unsupported(_)) && e.to_string().contains("is at revision NEWER1"), "{e}");
    assert_eq!(oryx.layouts, before);
    assert!(!oryx.calls.iter().any(|c| c != "layout"), "only read: {:?}", oryx.calls);
    let forced = push_into(&mut oryx, &draft, Some(&base), &base.layout_id, true).unwrap();
    assert!(forced.verified && forced.warnings.iter().any(|w| w.contains("replaced by the draft")), "{forced:?}");

    // The layout has uncompiled edits of the user's own.
    let mut oryx = own(&base);
    oryx.layouts.get_mut("9qMJK").unwrap()["revision"]["qmkVersion"] = Value::Null;
    oryx.layouts.get_mut("9qMJK").unwrap()["revision"]["hashId"] = "DRAFT1".into();
    oryx.layouts.get_mut("9qMJK").unwrap()["revision"]["layers"][0]["title"] = "Mine".into();
    let before = oryx.layouts.clone();
    let e = push_into(&mut oryx, &draft, Some(&base), &base.layout_id, false).unwrap_err();
    assert!(e.to_string().contains("uncompiled changes"), "{e}");
    assert_eq!(oryx.layouts, before);
    let forced = push_into(&mut oryx, &draft, Some(&base), &base.layout_id, true).unwrap();
    assert!(forced.verified && forced.warnings.iter().any(|w| w.contains("Overwrote uncompiled changes")), "{forced:?}");
}

#[test]
fn into_a_layout_that_isnt_mine_writes_nothing_there() {
    let (base, draft) = base_and_draft(serde_json::from_str(WOW).unwrap());
    let mut oryx = oryx_with(&base); // not owned: Oryx makes a separate layout
    let before = oryx.layouts[&base.layout_id].clone();
    let e = push_into(&mut oryx, &draft, Some(&base), &base.layout_id, false).unwrap_err();
    assert!(e.to_string().contains("separate layout") && e.to_string().contains("Nothing was written to 9qMJK"), "{e}");
    assert_eq!(oryx.layouts[&base.layout_id], before);
    assert!(!oryx.calls.iter().any(|c| c.starts_with("update") || c.ends_with("Layer")), "{:?}", oryx.calls);
}

#[test]
fn a_new_layout_in_my_account_leaves_my_existing_layout_alone() {
    let (base, draft) = base_and_draft(serde_json::from_str(WOW).unwrap());
    let mut oryx = own(&base);
    let before = oryx.layouts[&base.layout_id].clone();

    let p = push_new_owned(&mut oryx, &draft, Some(&base)).unwrap();
    assert!(p.verified && p.new_layout && p.in_account && p.warnings.is_empty(), "{p:?}");
    assert_eq!(p.copied_from, "9qMJK@ZPjbXz");
    // My layout has no new revision; the new layout is mine and holds the draft, auto-mouse layer included.
    assert_eq!(oryx.layouts[&base.layout_id], before);
    assert!(oryx.frozen.is_empty() && p.layout_id != base.layout_id && oryx.owned.contains(&p.layout_id));
    let new = stored(&oryx, &p.layout_id);
    assert!(new.layers.iter().any(|l| l.title == "WoW") && new.layers.len() == base.layers.len() + 1);
    assert_eq!(oryx.layouts.len(), 3, "mine, the anonymous stepping stone, the new one");

    // Without a login Oryx can't put it in an account: nothing is written anywhere.
    let mut oryx = oryx_with(&base);
    let e = push_new_owned(&mut oryx, &draft, Some(&base)).unwrap_err();
    assert!(e.to_string().contains("doesn't belong to an account"), "{e}");
    assert!(!oryx.calls.iter().any(|c| c.starts_with("update") || c.ends_with("Layer")), "{:?}", oryx.calls);
}
