# Gaming layers

A gaming layer is a mode the user toggles into and stays in. Its job: every key the game needs, with zero
input delay and zero surprises. Read `layer-design.md` first; for MMOs also read `mmo.md`.

## Ask first

- Which game(s)? Which keys does the game actually use (movement, abilities, reload, jump, crouch, sprint,
  chat, map, inventory, push-to-talk)? Ask for the in-game keybind list if they have it.
- Which hand is on the mouse? If they use the Navigator trackball instead of a mouse, which half has it?
- Do they want to keep the game's default binds (put the keys where their fingers are) or rebind in-game?
- Is one gaming layer enough, or one per game?

## Rules

1. **Plain keys only.** No `hold`, `tapHold` or `doubleTap` slots, no `LT`, no mod-taps (`LCTL_T(...)`), no
   `TT`. Tap-hold has to wait the tapping term (~200 ms) before it knows what you meant, and fast or
   overlapping game input misfires as holds. `validate` flags this as `gaming-tap-hold`.
2. **No home-row mods on or below the layer.** If the base has mod-taps (home-row mods, Ctrl on hold Tab,
   Meh on hold Backspace), a transparent key on the gaming layer inherits them. The gaming layer must define
   every key the game needs explicitly, and use `NO` (not `TRNS`) everywhere the base has tap-hold.
   `validate` flags `gaming-home-row-mods`. Two ways: `add_layer` with `fill none` and set what you need (the
   guide then has one step "set the other keys to None"), or the default `fill trans` and put `NO` only on the
   keys where the base has a tap-hold (fewer clicks in Oryx; plain base keys keep working in the game).
3. **Enter and leave with the same key.** `TG(gaming)` on the base at a spot that's hard to hit by accident
   (e.g. `R.num.0`, or a thumb on a symbol layer; not the key that is Esc on the gaming layer), and `TG(gaming)` at the same position on the
   gaming layer. `TO(0)` also works as the exit. Never leave a gaming layer without an exit (`layer-no-exit`).
4. **Keep Esc and Enter.** Esc for menus, Enter for chat. Usually `L.num.0` = Esc is natural; put Enter on a
   thumb or `L.bot.0`. For typing longer chat messages the user leaves the layer with the `TG` key and comes
   back; don't add tap-hold just for chat.
5. **Games read key codes, not legends.** The game sees `KC_W`, not "the key labelled W". You can move the
   physical position of a bind freely as long as the code stays the same, so no in-game rebinding is needed.
   Custom labels in Oryx are only for the user (Oryx and keyloom's renders).

## Movement cluster

On the Voyager's columnar layout, QWERTY WASD sits at `L.top.2` (W), `L.home.1` (A), `L.home.2` (S),
`L.home.3` (D): pinky on A, ring on S, middle on D. That leaves only column 0 for the pinky and wastes the
strong index finger on extra binds.

Preferred: **ESDF placement sending WASD codes** (rule 5):

| Position | Code | Finger |
| --- | --- | --- |
| `L.top.3` | `KC_W` | middle |
| `L.home.2` | `KC_A` | ring |
| `L.home.3` | `KC_S` | middle |
| `L.home.4` | `KC_D` | index |

This frees columns 0–1 (8 keys for the pinky: Shift, Ctrl, Tab, Q, Z ...) and `L.*.5` for the index.
If the user prefers the classic feel, keep WASD at their QWERTY positions; ask, don't impose.

Abilities go around the cluster in order of frequency: `L.top.2`/`L.top.4` (Q/E style), `L.home.5`,
`L.top.5`, `L.bot.2`–`L.bot.5`, then the number row `L.num.1`–`L.num.5` for 1–5.

## Modifiers and thumbs

- Thumbs carry what's held while moving: `L.thumb.1` = `KC_SPACE` (jump), `L.thumb.2` = `KC_LEFT_SHIFT`
  (sprint) or `KC_LEFT_CTRL` (crouch). Plain modifier keys, not mod-taps.
- Column 0 takes the rest: `L.home.0` = `KC_LEFT_SHIFT` or `KC_LEFT_CTRL`, `L.top.0` = `KC_TAB`,
  `L.bot.0` = `KC_LEFT_CTRL` or `KC_LEFT_ALT`. Avoid `KC_LEFT_GUI` on a gaming layer (opens Start / steals
  focus); set it to `NO`.

## The other half

The mouse hand's half is mostly idle. Fill it with `NO` (safest), or with binds the user presses between
fights (map `KC_M`, inventory `KC_I`, push-to-talk). If the Navigator trackball is on that half, the
auto-mouse layer may pop up when the ball moves; keep mouse buttons (`KC_MS_BTN1`, `KC_MS_BTN2`) reachable
or tell the user how the auto-mouse layer behaves in the game.

## Checklist

- [ ] Asked: game, key list, mouse hand / Navigator side, keep default binds or rebind.
- [ ] Layer tagged `gaming`, created with `fill none`.
- [ ] No hold / tapHold / doubleTap / `LT` / mod-tap / `TT` anywhere on the layer.
- [ ] Every key whose base binding has tap-hold is explicitly set (to a code or `NO`) on the gaming layer.
- [ ] Movement cluster placed (ESDF positions sending WASD, or classic WASD) and the user agreed.
- [ ] Space and Shift/Ctrl on thumbs as plain keys; `KC_LEFT_GUI` not on the layer.
- [ ] Esc and Enter reachable on the layer.
- [ ] Entry `TG(gaming)` (or `TO`) on base, exit at the same position on the gaming layer.
- [ ] `validate` has no `gaming-tap-hold`, `gaming-home-row-mods` or `layer-no-exit` findings.
