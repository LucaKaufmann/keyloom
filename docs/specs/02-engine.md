# 02: Engine: explore queries, edit operations, validation, diff

The `keyloom` crate is a library. CLI and MCP are thin wrappers: every feature is one engine function with a
typed input and a serializable output. No printing, no network, no keyboard access in the engine
(those live in `keyloom-cli` behind traits so tests use fakes).

## Key semantics the engine must get right

- **Effective binding.** On layer N, `KC_TRNS` falls through to the next *active* lower layer. For queries,
  keyloom resolves transparency against the base layer (like Oryx's own preview) and reports it as
  `"transparent": true, "resolvedFrom": 0`. For momentary layer keys (`MO`, `LT`), the holding key itself
  is what you press, so it stays the same on the target layer.
- **Layer actions**: `MO` (hold), `LT` (hold layer / tap key), `TG` (toggle), `TO` (switch to, others off),
  `TT` (tap-toggle), `OSL` (one-shot), `DF` (set default). Exact Oryx encodings are in `docs/oryx-encodings.md`.
- **Action shapes**: tap, hold, double tap, tap-then-hold (Oryx tap dance), modifiers on an action,
  custom labels. A key with no action at all is **transparent** (Oryx compiles blank keys to
`KC_TRANSPARENT`, docs/oryx-encodings.md); only `KC_NO` does nothing. `add_layer` with `fill: none` therefore writes
explicit `KC_NO`, so base-layer mod-taps can't leak into the new layer.

## Explore queries

| Function | Input | Output |
| --- | --- | --- |
| `summary` | ref | title, layers (title, tags, key counts by kind), combos, how each layer is reached |
| `render_layer` | ref, layer, style (`legend`/`codes`) | grid model (used by ASCII render and MCP) |
| `key_info` | ref, key selector, layer? | per layer: tap/hold/doubleTap/tapHold, legend, transparent?, finger, effort |
| `find` | ref, keycode or text | all (layer, position, slot) where it occurs, incl. combos and modified keys |
| `free_keys` | ref, layer | empty keys and transparent keys, sorted by effort (07), with finger |
| `layer_graph` | ref | edges `from layer → to layer` with action type and key; reachability from base; exits |
| `diff` | ref A, ref B | see Diff |
| `validate` | ref | findings (rules below) |

`find` normalises aliases (`KC_ESC` = `KC_ESCAPE`, `Esc`) through the keycode reference (07).

## Edit operations (drafts only)

Every operation: takes a draft, checks preconditions, applies, returns `{changes: [...], warnings: [...]}`.
Operations are atomic. A batch of operations (MCP `edit_draft`, CLI `keyloom apply`) is also atomic: all or nothing.
Each successful call writes the draft file and appends to `drafts/<name>.ops.jsonl` (an op log for
explanations and undo).

| Op | Arguments | Notes |
| --- | --- | --- |
| `draft_create` | name, from ref | copies a revision or another draft |
| `set_key` | layer, key, action spec | replaces the whole key (a custom label is dropped with a warning; the LED color stays) |
| `set_slot` | layer, key, slot (`tap`/`hold`/`doubleTap`/`tapHold`), action spec or `none` | edits one part |
| `clear_key` | layer, key, `trans`\|`none` | |
| `set_label` / `set_color` | layer, key, value | customLabel, glowColor |
| `add_layer` | title, after?, copy_from?, fill (`trans`/`none`), tags | appended at the end by default; with copy_from, `fill: none` turns the copy's transparent keys into KC_NO |
| `move_layer` | layer, to position | **rewrites every layer reference** (keys, combos) |
| `remove_layer` | layer, `force` | fails if referenced unless force; force removes or reports the references |
| `rename_layer` / `tag_layer` / `color_layer` | | |
| `add_combo` / `remove_combo` | keys, layer, trigger action, name | |
| `swap_keys` | layer (or all), key A, key B | combos on those layers follow their keys |
| `undo` | draft | reverts the last op from the op log |

**Action spec** (one small parser, shared by CLI and MCP; actions are always written as these strings):

```
KC_A | A | Esc                    plain key (aliases from 07)
LCTL(KC_C)                        modified key
MO(3) | TG(gaming) | TO(0) | OSL(sym) | TT(2) | DF(0)
LT(3, KC_SPC)                     tap key, hold layer
MT(LCTL, KC_A) | LCTL_T(KC_A)     mod-tap
TRNS | NO
```

Also accepted: QMK's short modifier forms (`C(KC_C)`, `S(…)`, `CTL_T(…)`, `SFT_T(…)`), `TAP_HOLD(tap, hold)`
with `_` for an empty side, `MEH_T`/`ALL_T`, `RGB(#rrggbb)`. Rejected with an explanation: `OSM`, `LM`,
combined mod-taps (`LCAG_T`), bare family names (`MO`, `LT`, `RGB`), layer numbers that don't exist.

The parser turns these into Oryx-shaped key objects. Oryx has no `LT`/`MT` codes: `LT(3, KC_SPC)` becomes
`tap KC_SPACE` + `hold MO(3)`, and `LCTL_T(KC_A)` becomes `tap KC_A` + `hold KC_LEFT_CTRL`
(`docs/oryx-encodings.md`). Rendering and `find` do the reverse to show QMK-style names. Anything Oryx can't represent (per the encoding notes) is
rejected with a message, never silently changed.

## Validation rules

Findings: `{rule, severity (error|warn|info), layer?, key?, message, hint}`.

| Rule | Sev | Check |
| --- | --- | --- |
| `layer-ref-missing` | error | a key or combo refers to a layer that doesn't exist, or a layer key has no layer |
| `layer-no-exit` | error | a layer entered with `TG`/`TO`/`TT` has no `TG`/`TT`(itself) or `TO`(other) on it, own or resolved; a layer entered with `DF` has no `DF`(other). Combos count as keys |
| `layer-unreachable` | warn | non-base layer with no path from base. Exempt: the auto-mouse layer (detected on import) and layers tagged `auto` |
| `too-many-layers` | warn | more than 8 layers (Oryx's real limit is unknown; 8 is what keyloom has seen compile) |
| `key-count` | error | a layer doesn't have exactly the geometry's key count |
| `unknown-keycode` | warn | code not in the reference (could be valid but unknown to us) |
| `combo-duplicate` | error | two combos on the same layer with the same key set |
| `combo-overlap` | info | one combo's keys are a subset of another's |
| `gaming-tap-hold` | warn | a `gaming`-tagged layer has hold/tapHold/doubleTap actions or `TT` (adds input delay/misfires) |
| `gaming-home-row-mods` | warn | home-row mod-taps active on (or falling through to) a `gaming` layer |
| `essential-unreachable` | info | Esc, Enter, Backspace, Tab, Space not reachable from base in ≤ 1 layer step |
| `duplicate-in-layer` | info | same plain keycode twice on one layer (often intentional) |
| `mo-target-overrides` | info | the key that holds `MO(n)` is bound to something else on layer n (fine in QMK, often surprising) |
| `base-drift` | info | a draft's base revision isn't the newest stored revision of its layout |

Rules are individually switchable in `config.toml` (`[validate] disabled_rules`) and tested on the real fixture layout
with the edits that trigger each rule.

## Diff

`diff(A, B)` produces structured changes, the input to the apply guide:

```
layer_added   {position, title, color, tags}
layer_removed {position, title}
layer_moved   {from, to}            (references rewritten implicitly)
layer_changed {position, field, from, to}      title, color
key_changed   {layer, index, position, slot, from, to}   slot-level, not whole-key
combo_added / combo_removed / combo_changed
```

Layers are matched by identical content first (so a rename, a title swap or "delete A, rename B to A" is seen
for what it is; of several identical layers the one that kept its title is the original), then by title
(unless the title was reused: the two layers have little in common and one of them is mostly the same as
a layer under another title), then by mostly-equal content (a renamed layer that was also edited), then by
position when the old layer was nearly empty. `layer_moved` is only reported for real reorders, not for layers shifted by an
insert or a removal; added layers list all their keys as `key_changed` from empty. Output also has a human summary (`+1 layer, 23 keys changed, 1 combo added`).
