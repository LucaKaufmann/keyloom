# Numbers layer

Two choices: keep digits on the `num` row (already there on the Voyager base), or add a numpad-shaped layer
under one hand. They solve different problems; ask what the user does with numbers.

## Number row vs numpad layer

| Number row (`L.num.1`… `R.num.1`) | Numpad layer |
| --- | --- |
| no layer needed, matches muscle memory | home-row reach, no finger travel |
| top row is the most expensive row (effort +0.4) | needs a held or toggled layer |
| good for occasional digits, games (bars 1–0) | good for data entry, spreadsheets, long numbers |

Many 52-key users do both: keep the number row on base and add a numpad on a symbols/numbers layer.

## Numpad shape

Put it under the hand that does NOT hold the layer (usually the right):

| Row | `col 4` | `col 3` | `col 2` |
| --- | --- | --- | --- |
| `R.top` | 7 | 8 | 9 |
| `R.home` | 4 | 5 | 6 |
| `R.bot` | 1 | 2 | 3 |

`0` on the right thumb (`R.thumb.1`) or `R.bot.5`; `.` and `,` on `R.bot.1`/`R.home.1`; `-` `+` `*` `/` `=`
on column 5 and column 1. Index finger gets the 7-4-1 column like a real numpad.

- Digit codes: `KC_1`…`KC_0` work everywhere, regardless of Num Lock. Numpad codes (`KC_KP_1`…) need Num Lock
  on Windows/Linux and are different keys to games and apps (some spreadsheets and Blender treat them
  specially). Default to `KC_1`…; use `KC_KP_*` only if the user asks for real numpad keys.
- Entry: `MO(num)` / `LT(num, KC_x)` for short bursts. For data entry offer `TG(num)` with a `TG(num)` exit at
  the same position, or `QK_LLCK` to lock the held layer.
- Holding half: keep Tab, Enter, Backspace and arithmetic operators reachable; leave the rest transparent.
- A numbers layer can also host F-keys on the other half (`L.top.*`/`L.home.*`/`L.bot.*` = F1–F12 in the
  same 7-8-9 / 4-5-6 / 1-2-3 pattern), which keeps F-keys memorable.

## Checklist

- [ ] Asked what numbers are for (typing, data entry, games, F-keys) and which hand is free.
- [ ] Digits laid out as 7-8-9 top, 4-5-6 home, 1-2-3 bottom, 0 on a thumb or next to 1.
- [ ] `KC_1`… vs `KC_KP_1`… chosen on purpose; Num Lock caveat told if `KC_KP_*` used.
- [ ] Operators and `.` reachable without leaving the layer.
- [ ] Toggled variant has its exit at the entry position; held variant's holding key is transparent.
- [ ] Tagged `numbers`; `validate` clean.
