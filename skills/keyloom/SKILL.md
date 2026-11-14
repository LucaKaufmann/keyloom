---
name: keyloom
description: See, audit and improve the user's ZSA keyboard layout (Voyager) with the `keyloom` CLI. Use when the user asks about their keyboard layout, keymap, layers or key bindings, wants a layout reviewed, or wants a layer or keys added or changed (e.g. "add a gaming layer", "where is Esc", "review my layout").
---

# keyloom

`keyloom` imports ZSA layouts from Oryx into a local workspace and lets you render, explore, audit and edit
them in drafts. It never changes the user's Oryx layouts and can't flash firmware: the result of an edit is a step-by-step guide the
user follows in Oryx. Add `--json` to any command for structured output.

## Keys and layers

- Keys are named by position: `L.home.4` = left half, home row, column 4 counted from the OUTER edge
  (0 outer pinky, 1 pinky, 2 ring, 3 middle, 4 index, 5 inner index). Rows: `num`, `top`, `home`, `bot`.
  Thumbs: `L.thumb.1` (next to the main block), `L.thumb.2` (toward the centre). Same on the right with `R.`.
- Layers by number or title (case-insensitive prefix); `base` is layer 0.
- Layouts: omit for the active layout, or give `9qMJK`, `9qMJK@<revision>`, or a draft (`wow` / `draft:wow`).

## See

```sh
keyloom list                       # imported layouts, revisions, drafts (empty: ask for the Oryx link)
keyloom import <oryx link or id>   # or `keyloom import keymapp`
keyloom summary                    # start here: layers, how each is reached and left
keyloom show [-l <layer>] [--all] [--style codes]
keyloom layers [--graph]           # how layers connect
keyloom key L.thumb.1              # one key on every layer
keyloom find Esc                   # where a keycode / `MO(1)` / label lives
keyloom free -l Gaming             # free keys on a layer, easiest first
keyloom ref "volume up"            # keycode reference: never guess keycodes
keyloom guides [<name>]            # layer-design, gaming, mmo, nav, symbols, numbers, mouse, home-row-mods
```

## Audit

`keyloom validate [<layout>]` (exit code 3 on errors). For a review: run `summary`, `layers --graph`,
`validate` and `show --all`, read `keyloom guides layer-design`, then report problems, ergonomic issues
(frequent keys on high-effort positions, same-finger strain, layer keys off the thumbs) and 3–5 concrete
suggestions with positions. Don't edit unless asked.

## Improve

Before designing a layer, read `keyloom guides layer-design` and the guide for its purpose, and ask the
user what the guide suggests (e.g. which hand is on the mouse). Propose the change as a table of positions
and keys first.

```sh
keyloom draft new wow                                   # all edits happen in drafts
keyloom layer add wow WoW --after Gaming --fill none --tag gaming
keyloom set wow Main L.thumb.1 'TG(WoW)'                # whole key; --slot hold for one slot
keyloom set wow WoW L.home.1 KC_A
keyloom clear wow WoW L.home.2 [--none]
keyloom layer move|rename|rm|tag|color wow …
keyloom combo add wow 0 L.home.3+L.home.4 KC_ESC
keyloom apply wow ops.json                              # many edits at once, all or nothing
keyloom draft undo wow
keyloom draft show wow                                  # changes so far
keyloom diff 9qMJK draft:wow
keyloom validate wow                                    # fix every error
keyloom guide wow                                       # steps for the user in Oryx: show them this
keyloom push wow                                        # or, if the user agrees: a NEW public Oryx layout with the draft; give them the link
keyloom push wow --to 9qMJK                             # only when the user asks: an uncompiled revision of THEIR layout (needs their token, see below)
```

`ops.json` is a JSON array, e.g. `[{"op":"set_key","layer":"WoW","key":"L.home.1","action":"KC_A"}]`. Ops:
`set_key{layer,key,action}`, `set_slot{layer,key,slot,action}`, `clear_key{layer,key,to:trans|none}`,
`add_layer{title,after?,copy_from?,fill?,tags?,color?}`, `move_layer{layer,to}`, `remove_layer{layer,force?}`,
`rename_layer{layer,title}`, `tag_layer{layer,tags}`, `color_layer{layer,color}`, `add_combo{layer,keys,action}`,
`remove_combo{layer,keys}`, `swap_keys{layer,a,b}`, `set_label{layer,key,label}`, `set_color{layer,key,color}`.

Actions: `KC_A`, `Esc`, `LCTL(KC_C)`, `MO(3)`, `TG(Gaming)`, `TO(0)`, `OSL(sym)`, `TT(2)`, `DF(0)`,
`LT(3, KC_SPC)`, `LCTL_T(KC_A)`, `TAP_HOLD(KC_EQL, KC_ESC)`, `TRNS` (falls through), `NO` (does nothing).
Gaming layers: `--fill none`, plain keys only (no tap-hold), entered and left with the same `TG(layer)` key.

After the user applied the guide in Oryx and flashed: `keyloom import <layout id>`, then
`keyloom verify wow` (exit code 3 if something is still missing), then `keyloom draft archive wow`.

## On the keyboard (optional)

Needs Keymapp running with Settings → API on; check with `keyloom status`.
`keyloom live highlight L.home.1 L.home.2`, `keyloom live layer <layer>`, `--show` on `show`/`find`, and
`keyloom guide wow --step` (lights each step's key). Always finish with `keyloom live reset`.

## Pushing into the user's own layout

`keyloom push <draft> --to <layout>` changes the user's Oryx account, so run it only when they asked for it.
It needs their login token in `KEYLOOM_ORYX_TOKEN` or the macOS keychain item `keyloom-oryx`; if it is missing,
show them the command's message and let them set it up. Never ask them to paste the token into the chat. It
writes an uncompiled revision and never compiles. If it refuses because Oryx has newer or uncompiled changes,
tell the user what it said; use `--overwrite` only when they say to replace those changes.
