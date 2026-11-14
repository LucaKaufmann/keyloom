# Designing a layer

Read this before adding or reshaping any layer. The other guides (gaming, mmo, nav, symbols, numbers,
mouse, home-row-mods) assume it.

## The board

The Voyager has 52 keys: per half 4 rows (`num`, `top`, `home`, `bot`) × 6 columns, plus 2 thumb keys.
Positions are `half.row.col`, columns counted from the OUTER edge: `0` outer pinky column, `1` pinky,
`2` ring, `3` middle, `4` index, `5` index inner. So `L.home.4` is QWERTY F and `R.home.4` is J.
Thumbs: `L.thumb.1` sits right under the main block, `L.thumb.2` is further toward the centre (same on `R`).
Four thumb keys are the scarcest, most valuable keys on the board. Look before you plan:
`layout_summary` with `graph` (CLI: `keyloom summary`, `keyloom layers --graph`), then `render_layer`
(`keyloom show -l <layer>`) for the layers you will touch.

## Choosing the layer action

| Action | Behaviour | Use it for |
| --- | --- | --- |
| `MO(n)` | on while held | short bursts: symbols, numbers, nav, function keys |
| `LT(n, KC_x)` | tap = key, hold = `MO(n)` | the same, when the thumb key also needs a tap (Space, Enter, Backspace) |
| `TG(n)` | tap on, tap again off | modes you stay in: gaming, numpad for data entry, a mouse layer |
| `TO(n)` | switch to n, everything else off | a reliable exit (`TO(0)`), or entering a mode without stacking |
| `TT(n)` | hold = MO, tap repeatedly = toggle | rarely worth it; tap-hold timing, hard to explain to users |
| `OSL(n)` | next key only comes from n | an occasional symbol or F-key without holding |
| `DF(n)` | changes the default layer | switching the whole base (QWERTY ↔ Colemak). Unconfirmed in Oryx; check before use |

Oryx has no `LT` code: keyloom stores `LT(3, KC_SPC)` as tap `KC_SPACE` + hold `MO(3)`, and the apply guide
tells the user to set the tap and the hold separately. Any tap + hold key adds a decision delay (the tapping
term) to the tap; fine for typing, bad for games.

## Always a way back out

- Momentary layers (`MO`, `LT`, `OSL`) exit by themselves. Toggled or switched layers (`TG`, `TO`, `TT`, `DF`)
  need an exit key ON that layer, or the user is stuck until replug. `validate` reports `layer-no-exit`.
- Put the exit on the same physical key that entered: `TG(gaming)` on the base and `TG(gaming)` at the same
  position on the gaming layer. Muscle memory works both ways.
- If the layer can be reached from several places, add `TO(0)` as the exit instead; it always lands on base.
- With `MO(n)`, the key you hold to reach layer n must stay transparent (or the same `MO`) on layer n. Don't
  bind something else there (`mo-target-overrides`).
- `QK_LLCK` (layer lock) keeps a momentary layer on after release; press it again to unlock. Good on nav and
  mouse layers, and an alternative to a separate `TG`.

## Placement

- Layer keys go on thumbs: `L.thumb.*` / `R.thumb.*`, usually as `LT` so the thumb still taps Space,
  Enter or Backspace. Second choice: the outer pinky column (`L.home.0`, `R.home.0`) as `MO` for rare layers.
- Hold-to-use layers are most comfortable when the layer key and the layer's content are on opposite halves
  (hold with the left thumb, use the right hand), so one hand isn't contorting.
- Use `free_keys(layer)` to find spots; it sorts by effort. Home row (`*.home.1`–`*.home.4`) and `thumb.1` are
  cheapest; `num` row, column 0 and column 5 cost more.
- Keep Esc, Enter, Backspace, Tab and Space reachable from base in at most one layer step
  (`essential-unreachable`). Keep `QK_BOOT` on a rarely used layer, away from thumbs and home row.

## Transparency

- A blank key in Oryx compiles to `KC_TRANSPARENT`. On a non-base layer, transparent means "do what the layer
  below does". Use `TRNS` deliberately where the base should keep working (e.g. modifiers on a nav layer).
- Use `NO` where falling through would be harmful (a mod-tap on base leaking into a gaming layer, a typing key
  on a numpad layer that the user would hit by mistake).
- When you create a layer with `add_layer`, pick `fill trans` for overlay layers (nav, symbols) and
  `fill none` for mode layers (gaming, numpad mode) and then fill what you need.

## Tags and names

Tag every layer you create: `base`, `gaming`, `nav`, `symbols`, `numbers`, `mouse`, `function`, `media`.
Add `auto` to a layer the firmware enters by itself (an auto-mouse layer keyloom didn't detect).
Tags drive validation (`gaming` → no tap-hold; `auto` → no key has to lead there) and help the next
agent. Short titles (≤ 8 chars) read best in renders and on screen: `Sym`, `Nav`, `Num`, `Game`, `Mouse`.
Prefer 8 layers or fewer: that is the most keyloom has seen Oryx compile (Oryx's real limit is unknown).
Above 8, `validate` warns `too-many-layers`: tell the user it is untested and let them decide.

## Working with the user

1. Ask what the layer is for, which hand is on the mouse (or whether they use the Navigator trackball), and
   which keys they currently miss or reach for awkwardly.
2. Propose the plan in words plus a `render_layer` picture before applying anything.
3. `create_draft`, then one `edit_draft` batch (CLI: `keyloom draft new`, `keyloom apply` or `set`), then
   `validate` and `diff`.
4. `apply_guide` gives the user step-by-step Oryx instructions; they apply them in Oryx, compile and flash
   with Keymapp. Then `verify_draft`. Offer to light the changed keys if Keymapp is connected
   (`keyboard` action `highlight`; CLI: `keyloom live highlight`).

## Checklist

- [ ] Every `TG`/`TO`/`TT`/`DF` target layer has an exit key on it (same position as the entry where possible).
- [ ] Every new layer is reachable from base (or tagged `auto` if the firmware enters it by itself).
- [ ] Layer keys sit on thumbs or column 0, never on a home-row letter key.
- [ ] The key holding an `MO`/`LT` is transparent or the same layer key on the target layer.
- [ ] Transparent vs `NO` chosen on purpose for every non-assigned key of the new layer.
- [ ] Esc, Enter, Backspace, Tab, Space reachable from base in ≤ 1 layer step.
- [ ] Layer has a tag and a short title; more than 8 layers in total is flagged to the user (untested).
- [ ] `validate` shows no errors; warnings are explained to the user.
- [ ] The user saw `render_layer` of the result and the `diff` before `apply_guide`.
