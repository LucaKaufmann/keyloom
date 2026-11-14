# Home-row mods

Home-row mods (HRM) put Ctrl/Alt/Shift/GUI on the hold of home-row letters: tap `A` types a, hold `A` is
GUI. They save the pinky and make shortcuts reachable from home, but they are the most timing-sensitive
feature of a layout. Only add them when the user asks or agrees after hearing the trade-offs.

## How they look in keyloom and Oryx

- Spec: `LGUI_T(KC_A)`, `LALT_T(KC_S)`, `LCTL_T(KC_D)`, `LSFT_T(KC_F)` (or `MT(LCTL, KC_D)`).
- Oryx stores them as tap `KC_D` + hold `KC_LEFT_CTRL`; the apply guide tells the user to set both.
- Mirror on the right: `R.home.4` Shift, `R.home.3` Ctrl, `R.home.2` Alt, `R.home.1` GUI.

## Order

Two common orders (index → pinky, `col 4` → `col 1`):

| Name | `home.4` | `home.3` | `home.2` | `home.1` |
| --- | --- | --- | --- | --- |
| GACS (pinky first) | Shift | Ctrl | Alt | GUI |
| CAGS (macOS friendly) | Shift | GUI | Alt | Ctrl |

Shift on the index is common because Shift is used most and needs the fastest release. On macOS put GUI
(Cmd) on a strong finger. Ask which shortcuts the user presses most and match.

## Timing trade-offs

- Every HRM key waits the tapping term before deciding tap vs hold. Fast rolls (`as`, `df`) can produce
  `Ctrl+…` instead of letters, and slow typists trigger holds by lingering.
- Oryx exposes a global tapping term and per-key tapping terms. Typical: 200–250 ms for pinky/ring keys,
  ~180 ms for index Shift. Suggest raising the pinky keys first if the user reports accidental mods.
- Oryx settings worth mentioning (configured in Oryx, not by keyloom): "permissive hold", "hold on other key
  press", "quick tap term" / tapping force hold (lets a quick tap-then-hold repeat the letter), and Chordal
  Hold/opposite-hands where offered. Recommend opposite-hands behaviour if Oryx offers it: it removes most
  same-hand misfires.
- `QK_DYNAMIC_TAPPING_TERM_UP` / `DOWN` / `PRINT` on a rarely used layer helps tune without reflashing.

## Per-layer disabling

- HRM must never be active on a `gaming` layer, including through `TRNS` fall-through. Set those positions
  explicitly to plain keys or `NO` on gaming layers (`gaming-home-row-mods`).
- On nav/symbols/numbers layers the home row usually gets other symbols anyway; if the layer leaves home-row
  keys transparent, the mods still work there, which is often wanted (Shift+arrow on nav).
- Alternative for users who want both: a base layer without HRM plus a `DF`/`TG` "typing" layer with them,
  or HRM only on the non-mouse hand.

## Lighter alternatives

- Mods on thumbs (hold of `L.thumb.*` / `R.thumb.*`) or plain mods on column 0 (`L.home.0` Shift) avoid letter misfires entirely.
- One-shot mods on a thumb or layer: tap Shift, then the letter; no timing on letters. keyloom can't
  draft these yet (`OSM(...)` is rejected): the user sets them by hand in Oryx.
- Only Shift on `home.4` (the rest on a nav layer) is a gentle first step.

## Checklist

- [ ] The user asked for HRM or agreed after hearing the misfire trade-off.
- [ ] Order chosen with the user's OS and most used shortcuts in mind; mirrored on both halves.
- [ ] No HRM reaches a `gaming` layer (explicit plain keys or `NO` there); `validate` has no
      `gaming-home-row-mods`.
- [ ] Tapping-term advice given (per-key for pinky/ring, Oryx tap-hold settings).
- [ ] Thumb layer keys still work (no conflict between an `LT` thumb and same-hand HRM in common shortcuts).
- [ ] Offered a lighter alternative if the user types very fast or rolls a lot.
