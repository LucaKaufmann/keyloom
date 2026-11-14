//! The embedded reference data (spec 07) is complete and consistent.

use keyloom::knowledge;
use std::collections::HashMap;

#[test]
fn keycodes_are_consistent() {
    let codes = knowledge::keycodes();
    assert!(codes.len() > 200, "only {} keycodes", codes.len());
    let mut seen: HashMap<String, &str> = HashMap::new();
    for k in codes {
        for name in std::iter::once(&k.code).chain(k.aliases.iter()) {
            if let Some(prev) = seen.insert(name.to_lowercase(), &k.code) {
                assert_eq!(prev, k.code, "'{name}' names both {prev} and {}", k.code);
            }
        }
        assert!(!k.category.is_empty(), "{} has no category", k.code);
    }
}

#[test]
fn codes_used_by_real_layouts_are_known() {
    // Everything in the fixtures (a real layout and a made-up one) must be in the reference.
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    for entry in std::fs::read_dir(dir).unwrap() {
        let raw: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(entry.unwrap().path()).unwrap()).unwrap();
        let layout = keyloom::source::parse_value(&raw).unwrap();
        for l in &layout.layers {
            for k in &l.keys {
                for (_, a) in k.slots() {
                    if let Some(c) = a.code() {
                        assert!(knowledge::is_known(c), "{c} (layer {}) isn't in keycodes.json", l.title);
                    }
                }
            }
        }
    }
}

#[test]
fn lookups() {
    let canon = |s: &str| knowledge::canonical(s);
    assert_eq!(canon("Esc").as_deref(), Some("KC_ESCAPE"));
    assert_eq!(canon("KC_ESC").as_deref(), Some("KC_ESCAPE"));
    assert_eq!(canon("a").as_deref(), Some("KC_A"));
    assert_eq!(canon("1").as_deref(), Some("KC_1"));
    assert_eq!(canon("!").as_deref(), Some("KC_EXLM"));
    assert_eq!(canon("KC_LCTL").as_deref(), Some("KC_LEFT_CTRL"));
    assert_eq!(canon("Space").as_deref(), Some("KC_SPACE"));
    assert!(!knowledge::search("volume", 5).is_empty());
}

#[test]
fn guides_are_embedded() {
    for name in ["layer-design", "gaming", "mmo", "nav", "symbols", "numbers", "mouse", "home-row-mods"] {
        let g = knowledge::guide(name).unwrap_or_else(|| panic!("guide {name} missing"));
        assert!(g.contains("## Checklist"), "{name} has no checklist");
    }
}
