# MMO layers (World of Warcraft and similar)

MMOs need far more binds than shooters: 12+ action-bar slots, often ×2–3 with modifiers, plus targeting,
mount, map, bags, push-to-talk. Everything in `gaming.md` applies (plain keys only, no tap-hold, no
home-row mods leaking through, same key in and out). This guide adds how to fit the binds.

## Interview first

Ask these before touching the draft, and write the answers into your plan:

1. Mouse hand, and is it a mouse or the Navigator trackball (which half)? Is the other half used at all?
2. How many action bars do you press from the keyboard (1, 2, 3)? How many slots each (usually 1–6 or 1–12)?
3. Bar pages with modifiers? Which: Shift, Ctrl, Alt? (Shift+1 … is common; Ctrl/Alt combos too.)
4. Movement: WASD keys, or mouse-steer with strafing only (`Q`/`E` strafe in WoW)? Autorun key?
5. Push-to-talk app and key (Discord often uses a spare key like `KC_F13`, which nothing else uses)?
6. Must-have extras: target nearest (`Tab`), interact (WoW: often unbound by default; suggest `KC_F` or `KC_T`),
   mount, map (`M`), bags (`B`), character (`C`), spellbook (`P`), jump (`Space`), chat (`Enter`), Esc.
7. Keep the game's default binds, or are they happy to rebind in game? (Defaults are less work for them.)

## Fitting the binds

- Bars 1–12 default to `KC_1`…`KC_0`, `KC_MINUS`, `KC_EQUAL`. Reaching `R.num.*` with the left hand isn't
  possible, so on a split board move 6–0 to the left half (codes stay the same, positions change) or keep
  only 1–6 on the number row and put 7–12 on the keys around movement.
- Modifiers for bar pages go on thumbs and column 0 as plain keys: `L.thumb.2` Shift, `L.home.0` Ctrl,
  `L.bot.0` Alt. A thumb modifier lets the pinky stay on its bind.
- Push-to-talk and target/interact sit next to movement, where a finger lands without lifting off the cluster.
- The mouse-hand half is mostly idle. Fill it with `NO` unless the user has a use (e.g. a few rarely used
  window binds), so a resting hand can't trigger anything.
- `KC_F13`–`KC_F24` are excellent for addon binds and push-to-talk: no collision with chat or OS shortcuts.

## Example: WoW, right-hand mouse, left half (ESDF positions sending WASD)

| Position | Code | Why |
| --- | --- | --- |
| `L.num.0` | `KC_ESCAPE` | menus, cancel cast |
| `L.num.1`–`L.num.5` | `KC_1` `KC_2` `KC_3` `KC_4` `KC_5` | bar slots 1–5 |
| `L.top.0` | `KC_TAB` | target nearest enemy |
| `L.top.1` | `KC_6` | bar slot 6, pinky |
| `L.top.2` | `KC_Q` | strafe left (or ability) |
| `L.top.3` | `KC_W` | forward |
| `L.top.4` | `KC_E` | strafe right (or ability) |
| `L.top.5` | `KC_R` | ability |
| `L.home.0` | `KC_LEFT_CTRL` | bar page modifier |
| `L.home.1` | `KC_7` | bar slot 7 |
| `L.home.2` | `KC_A` | turn/strafe left |
| `L.home.3` | `KC_S` | back |
| `L.home.4` | `KC_D` | turn/strafe right |
| `L.home.5` | `KC_F` | interact (bind in game) |
| `L.bot.0` | `KC_LEFT_ALT` | bar page modifier |
| `L.bot.1` | `KC_8` | bar slot 8 |
| `L.bot.2` | `KC_Z` | ability / sheathe |
| `L.bot.3` | `KC_X` | ability / sit |
| `L.bot.4` | `KC_C` | character / ability |
| `L.bot.5` | `KC_V` | nameplates / ability |
| `L.thumb.1` | `KC_SPACE` | jump |
| `L.thumb.2` | `KC_LEFT_SHIFT` | bar page modifier |
| `R.thumb.1` | `KC_ENTER` | chat |
| `R.thumb.2` | `KC_F13` | push-to-talk (bind in Discord) |
| `R.num.0` | `TG(WoW)` | exit; same position as the entry on base |
| rest of `R.*` | `NO` | mouse-hand half stays inert |

Adjust to the answers: if the user has a Navigator on the right half, they may prefer Enter and
push-to-talk on the left thumbs; if they want 12 slots unmodified, keep the eight
slots of the table where they are and put `KC_9`, `KC_0`, `KC_MINUS`, `KC_EQUAL` on four keys the user
doesn't need (e.g. `L.bot.2`–`L.bot.5`, in place of Z/X/C/V). Map (`KC_M`) and bags
(`KC_B`) are pressed out of combat, so they can live on the right half (`R.top.*` / `R.home.*`) next to
the Enter thumb. Positions here put the exit on `R.num.0`; put the base `TG(WoW)` at the same spot.

## Checklist

- [ ] Interview answers recorded: mouse hand / Navigator, bars × slots, modifiers, movement style, PTT key.
- [ ] Everything in the `gaming.md` checklist passes (plain keys only, `fill none`, tag `gaming`).
- [ ] Every bar slot the user asked for has a key; the codes match the game's binds (or the rebinds are listed
      for the user).
- [ ] Bar modifiers are plain `KC_LEFT_SHIFT` / `KC_LEFT_CTRL` / `KC_LEFT_ALT` on thumbs or column 0.
- [ ] Push-to-talk and interact are adjacent to movement or on a thumb.
- [ ] Esc, Enter (chat), Space (jump) present.
- [ ] Unused keys on the mouse-hand half are `NO`, not `TRNS`.
- [ ] Entry and exit `TG` at the same position; `validate` clean.
- [ ] Told the user which binds they must set in game (interact, push-to-talk, moved bar slots).
