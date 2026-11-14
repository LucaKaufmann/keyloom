# Navigation layer

A nav layer puts arrows, Home/End, Page Up/Down and editing keys under the home position, so the hands
never travel. It is a momentary layer: held with a thumb, used with the other hand.

## Shape

- Reach it with `LT(nav, KC_x)` or `MO(nav)` on a thumb of the half that does NOT hold the arrows. Most
  people hold with the left thumb (`L.thumb.1` or `L.thumb.2`) and navigate with the right hand.
- Arrows on the right home row: `R.home.5` ←, `R.home.4` ↓, `R.home.3` ↑, `R.home.2` →
  (vim order, on the keys that are H J K L on QWERTY), or an inverted-T on `R.top.3` ↑ / `R.home.4` ← `R.home.3` ↓ `R.home.2` →. Ask which;
  vim users want the first, everyone else usually the second.
- Line and page movement on the row above or below the arrows: `R.top.*` Home / PgDn / PgUp / End in the
  same order as the arrows, so "left-ish" stays left.
- Editing on the inner and outer columns: `R.home.5` or `R.top.5` = `KC_BSPC`, `R.bot.5` = `KC_DELETE`,
  `R.home.0` = `KC_ENTER`.
- Platform matters: on macOS, line start/end are `LGUI(KC_LEFT)` / `LGUI(KC_RIGHT)` and word jumps are
  `LALT(KC_LEFT)` / `LALT(KC_RIGHT)`; on Windows/Linux `KC_HOME` / `KC_END` and `LCTL(KC_LEFT)` /
  `LCTL(KC_RIGHT)`. Ask the OS before binding word/line keys.

## The holding hand

- On the holding half, put plain modifiers on the home row in the user's usual order
  (e.g. `L.home.4` `KC_LEFT_SHIFT`, `L.home.3` `KC_LEFT_CTRL`, `L.home.2` `KC_LEFT_ALT`, `L.home.1` `KC_LEFT_GUI`). Hold layer + Shift +
  arrow = select. These are plain keys on this layer, not mod-taps, so no timing issues.
- Leave the rest of the holding half transparent or put clipboard shortcuts there:
  `LCTL(KC_Z)`/`LCTL(KC_X)`/`LCTL(KC_C)`/`LCTL(KC_V)` on `L.bot.1`–`L.bot.4` (use `LGUI(...)` on macOS).
- The key holding `MO`/`LT` must stay transparent on the nav layer.

## Optional

- `QK_LLCK` on the nav layer to lock it on for long reading/scrolling sessions; tell the user how to unlock.
- Mouse wheel (`KC_MS_WH_UP`/`KC_MS_WH_DOWN`) next to PgUp/PgDn if they have no trackball.
- Tag: `nav`. Fill: `trans` (overlay layer).

## Checklist

- [ ] Asked: OS (word/line shortcuts), vim-style or inverted-T arrows, which thumb holds the layer.
- [ ] Layer held with `MO`/`LT` on a thumb of the opposite half from the arrows.
- [ ] Four arrows on one home row, Home/End/PgUp/PgDn adjacent in a matching order.
- [ ] Plain modifiers on the holding half's home row for selection.
- [ ] Backspace, Delete, Enter reachable on the layer.
- [ ] The holding key is transparent on the nav layer.
- [ ] Tagged `nav`; `validate` clean.
