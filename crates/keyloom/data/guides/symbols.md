# Symbols layer

A symbols layer replaces the shifted number row and the far-away punctuation keys of a row-staggered board.
The Voyager has no keys beyond column 0, so symbols that live on `[`, `]`, `\`, `` ` ``, `=`, `-` on a laptop
need a home here.

## Ask first

- Main uses: programming (which languages?), prose, shell. Brackets and operators matter most for code.
- OS keyboard layout (US, UK, German ...). keyloom codes are US-layout codes: `KC_LBRC` produces `[` only
  with a US layout on the computer. On other OS layouts, warn the user and check each symbol.
- Do they already have symbols on another layer? (`find_keycode` for `KC_LCBR`, `KC_PIPE` ...).

## Placement principles

- Held with a thumb (`LT(sym, KC_SPC)` or `MO(sym)`), symbols on both halves; the holding thumb's half still
  gets symbols, just not the key under the thumb.
- Pairs side by side under neighbouring fingers, opening on the left of closing:
  `(` `)` on `L.home.3`/`L.home.4` or `R.home.4`/`R.home.3`; `{` `}` and `[` `]` directly above or below.
  Brackets on the home row beat everything else for programmers.
- Frequent operators on strong fingers: `=` `-` `_` `+` `*` `/` on home and top rows; rare ones (`~` `` ` ``
  `^` `%`) on column 0, column 5 or the bottom row.
- Mnemonics help memory: `-` `_` `+` `=` near each other; `<` `>` above `(` `)`; `!` `?` together.
- Use the dedicated shifted codes (`KC_EXLM`, `KC_LCBR`, `KC_PIPE` ...) rather than `LSFT(KC_1)`;
  both work, the named codes read better in `render_layer` and `find_keycode`.
- Avoid putting a shifted symbol (`KC_EXLM`) on a key the user holds with another modifier: QMK's shifted
  codes send Shift too, so Ctrl+`!` becomes Ctrl+Shift+1.

## A compact starting point (programmer, US layout)

| Row | Left (`col 1`→`5`) | Right (`col 5`→`1`) |
| --- | --- | --- |
| `top` | `!` `@` `{` `}` `\|` | `&` `*` `<` `>` `\` |
| `home` | `#` `$` `(` `)` `` ` `` | `+` `-` `=` `_` `:` |
| `bot` | `%` `^` `[` `]` `~` | `?` `/` `;` `"` `'` |

Each symbol appears once; it's a sketch to adapt, not a law. Leave thumbs as they are (transparent) except the holder.
Numbers usually go on their own layer (`numbers.md`) or on the `num` row of this one if the user wants a
single "Sym+Num" layer (common on 52-key boards: digits on `R.home.*`, symbols on the left).

## Checklist

- [ ] Asked: languages / use, OS keyboard layout, existing symbol locations.
- [ ] Every symbol the user needs is on the layer exactly once (`duplicate-in-layer` checked).
- [ ] Bracket pairs adjacent, opening on the left.
- [ ] Most frequent symbols on home row index/middle/ring.
- [ ] Holding thumb key transparent on the layer.
- [ ] Named shifted codes used; warned about non-US OS layouts if relevant.
- [ ] Tagged `symbols`; `validate` clean.
