# How Oryx encodes layouts

Notes from 2026-11-13. Sources: the author's layout `9qMJK` (revision `ZPjbXz`) and a second public Voyager layout,
fetched with the Oryx GraphQL layout query. The first is a test fixture in `crates/keyloom/tests/fixtures/`.

## Key order

52 keys per layer. Indices 0–23 = left half, 4 rows × 6 columns from the outer edge; 24–25 = left thumbs;
26–49 = right half, 4 rows × 6 columns **from the inner edge** (left to right as seen); 50–51 = right thumbs.
The author's base layer renders as coherent Colemak-DH with this order. Spec 01's position names count right-half
columns from the outer edge, so for the right half `col = 5 - (index - 26) % 6`.
Thumbs from the Voyager geometry (voyager-ghost's table): L #24 (x 5, y 4.5) sits right under the main block, L #25 (x 6, y 4.75) further
toward the centre; mirrored on the right: R #51 under the block, R #50 toward the centre.
So `thumb.1` = #24 / #51, `thumb.2` = #25 / #50. The author's Space/Sym key is `L.thumb.2`.

## Actions

Each key has slots `tap`, `hold`, `doubleTap`, `tapHold`. Each action has
`code, layer, color, macro, modifier, modifiers, description` (mostly null).

| QMK concept | Oryx encoding |
| --- | --- |
| plain key | `tap.code = "KC_A"` |
| `LT(1, KC_SPC)` | `tap.code = KC_SPACE` + `hold = {code: "MO", layer: 1}`. There is no `LT` code |
| mod-tap `LCTL_T(KC_TAB)` | `tap.code = KC_TAB` + `hold.code = KC_LEFT_CTRL` |
| any tap/hold pair | `hold.code` can be any keycode (`KC_EQUAL` / hold `KC_ESCAPE`) |
| Meh/Hyper hold | `hold.code = "MEH_T"` |
| modified key `LCTL(KC_LEFT)` | `tap.code = KC_LEFT`, `tap.modifiers = {leftCtrl: true, …}` (all 8 booleans) |
| layer keys | `code: "MO" / "TO" / …`, `layer: n` |
| RGB set color | `code: "RGB"`, `color: "#ffab00"` |
| ZSA specials | `TOGGLE_LAYER_COLOR`, `NAVIGATOR_INC_CPI`/`DEC_CPI`, `DRAG_SCROLL`, `TOGGLE_SCROLL`, `QK_LLCK` (layer lock), `QK_BOOT`, `MAC_MISSION_CONTROL` |
| transparent | explicit `tap.code = "KC_TRANSPARENT"` |
| blank key | `tap: null` and everything else null. **Compiles to `KC_TRANSPARENT`** (confirmed in Oryx's generated `keymap.c`) |

Not seen yet (need a layout that uses them): combos, `TG`/`TT`/`OSL`/`DF`, one-shot mods, macros
(`macro` field), the `modifier` field, tap dance with `tapHold`. Long names are used (`KC_LEFT_CTRL`,
`KC_SPACE`), so alias normalisation matters.

## Fields a lenient Oryx model drops

Key: `about`, `aboutPosition`, `tappingTerm` (per-key tapping term), `lockGlowColor`.
Action: `macro`, `modifier`. Confirms the need for the lossless model in spec 01.

## The fixture layout (`9qMJK@ZPjbXz`, "Voyager")

| # | Layer | Reached by | Notes |
| --- | --- | --- | --- |
| 0 | Main | | Colemak-DH; plain Shift and Alt on the outer pinky column, Ctrl on hold Tab, Shift on hold `'`, Alt on hold `/`, `=` tap / Esc hold, Meh on hold Backspace (no home-row mods) |
| 1 | Sym+Num | hold Space (L thumb) | F-keys, numbers, arrows, numpad on the right |
| 2 | Brd+Sys | hold R thumb 1 | RGB, media, brightness, volume, symbols, `QK_BOOT` |
| 3 | Gaming | **nothing** | empty except Space on a thumb, **unreachable** |
| 4 | Mouse | no key; presumably the Navigator auto-mouse layer | mouse buttons, CPI, drag-scroll, layer lock |

This makes it a good validation fixture: `layer-unreachable` fires for Gaming, and the auto-mouse layer
is the known exception.

## Oryx's generated source (revision `zipUrl`)

GraphQL also returns `createdAt`, `qmkVersion`, `config` (null when defaults) and `zipUrl`: a zip with the
compiled `.bin`, `keymap.c`, `config.h`, `rules.mk`, `keymap.json`. For `9qMJK@ZPjbXz`:

- Compiled forms: tap + hold `MO(n)` → `LT(n, KC_x)`; tap + hold modifier → `MT(MOD_LCTL, KC_TAB)`;
  hold-only `MO(2)` → `MO(2)`; hold `MEH_T` → `MEH_T(KC_BSPC)`; tap + hold *keycode* (`=`/Esc) →
  a custom `DUAL_FUNC_n` (an `LT(12, KC_5)` trick handled in `process_record_user`).
- `SERIAL_NUMBER "9qMJK/ZPjbXz"`: confirms the USB serial format used for flashed-layout detection.
- `AUTOMOUSE_LAYER 4`: the Navigator auto-mouse layer is a layout setting, not reachable via keys.
  It isn't in the GraphQL `config` (null), so keyloom reads it from `config.h` in the zip
  (or the user tags the layer). Feeds the `layer-unreachable` exemption.
- `LAYER_STATE_8BIT`: with ≤ 8 layers Oryx compiles 8-bit layer state. The real Oryx maximum is still
  unknown, but 8 is a safe first value for `too-many-layers` (warn above 8, not error).

## Keymapp's database

`~/Library/Application Support/.keymapp/keymapp.sqlite3`, table `revision(revisionId, data)`, `data` =
`{"layout": {…, "revision": {…}}}` with more fields than GraphQL (`createdAt`, `config`, `md5`, `zipUrl`,
`user`, per-layer `hashId`/`builtIn`). It can be older than the latest Oryx
revision. keyloom drops `user` on import (personal data it doesn't need).
