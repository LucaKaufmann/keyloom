# Mouse layer (Navigator trackball and mouse keys)

Two cases: the user has the ZSA Navigator trackball attached to one half, or not. Ask first. `layout_summary`
shows `autoMouseLayer` when the layout has one set in Oryx.

## With the Navigator

- **Auto-mouse layer.** Oryx can set one layer as the auto-mouse layer: it turns on when the ball moves and
  off after a short timeout or when a non-mouse key is pressed. No key enters it. keyloom detects it
  when importing from Oryx (layout_summary shows `autoMouseLayer`); if it didn't, tag the layer `auto` so it
  isn't reported as `layer-unreachable`.
- Buttons go under the hand that does NOT touch the ball, on the home row:
  `KC_MS_BTN1` (left click) on index, `KC_MS_BTN2` (right click) on middle, `KC_MS_BTN3` (middle click) on
  ring. E.g. ball on the right: `L.home.4` Click, `L.home.3` RClick, `L.home.2` MClick.
- `DRAG_SCROLL` (hold: the ball scrolls) on the same hand, near the buttons (`L.home.5` or `L.thumb.1`).
  `TOGGLE_SCROLL` if they scroll long documents.
- `NAVIGATOR_INC_CPI` / `NAVIGATOR_DEC_CPI` on a far key (`L.num.*`) – adjusted rarely.
- Back/forward (`KC_MS_BTN4` / `KC_MS_BTN5`) on `L.top.3`/`L.top.4` are popular with browsers.
- `QK_LLCK` on the layer lets the user keep it on while dragging or editing images.
- Every key of the auto-mouse layer you don't need: `TRNS`, so typing straight after moving the ball still
  works (pressing a non-mouse key also ends the auto layer). Keep Esc reachable.
- The ball half's keys on the mouse layer can carry modifiers (`KC_LEFT_SHIFT`, `KC_LEFT_CTRL`, `KC_LEFT_GUI`)
  for Shift-click, Cmd-click.

## Without a trackball (mouse keys)

- Pointer movement with mouse keys is slow and imprecise; it's for occasional use, not as a mouse replacement.
- Movement as an arrow cluster on the hand opposite the layer key: `KC_MS_UP`/`DOWN`/`LEFT`/`RIGHT`, mirroring
  the nav layer's arrow shape (same positions as arrows → easy to learn).
- Buttons under the holding hand's home row, wheel (`KC_MS_WH_UP`/`DOWN`) beside the movement keys,
  speed keys `KC_MS_ACCEL0`–`2` on column 0 or 5.
- Reach with `MO(mouse)` / `LT` on a thumb, or `TG(mouse)` with an exit at the same position.

## Gaming and the auto-mouse layer

If a gaming layer is active and the ball moves, the auto-mouse layer can activate on top and change what
nearby keys do. Tell the user; options are making the mouse layer's keys transparent where gaming binds live,
or turning auto-mouse off in Oryx while gaming with a trackball.

## Checklist

- [ ] Asked: Navigator or not, which half, mouse buttons wanted, OS.
- [ ] Auto-mouse layer detected (`autoMouseLayer`) or tagged `auto`; a manual mouse layer has an entry and exit.
- [ ] Left/right click on the strongest fingers of the non-ball hand's home row.
- [ ] `DRAG_SCROLL` (and CPI keys if Navigator) present; `QK_LLCK` offered.
- [ ] Unneeded keys transparent so typing resumes immediately; Esc reachable.
- [ ] Interaction with gaming layers explained if both exist.
- [ ] `validate` clean.
