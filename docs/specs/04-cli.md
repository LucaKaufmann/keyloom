# 04: CLI

Binary `keyloom` (`ln -sf ~/.cargo/bin/keyloom ~/.cargo/bin/kl` for a short alias). `clap` with subcommands;
every command and argument has `--help`. Every command takes `--json` (the same objects MCP returns).
Exit codes: 0 ok, 1 error, 2 usage, 3 a check didn't pass (`validate` found errors, `verify` doesn't match).

## Commands

```
Setup / data
  keyloom import <oryx-link|layoutId[/revisionId]|file.json|keymapp>   Oryx, a file, or Keymapp's local copies
  keyloom list                                                  layouts, revisions, drafts
  keyloom use <layoutId>                                        set active layout
  keyloom export [ref] [-o file] [--format keyloom|oryx]

Explore   (ref is optional and positional; key/find/free also accept it as --in ref)
  keyloom show [ref] [--layer L | --all] [--style legend|codes] [--show]
  keyloom summary [ref]
  keyloom layers [ref] [--graph]
  keyloom key <key> [ref] [--layer L] [--show]
  keyloom find <keycode|MO(1)|text> [ref] [--show]
  keyloom free [ref] [--layer L] [--show]
  keyloom ref <query>                                           keycode reference
  keyloom guides [name]                                         layer-design guides
  keyloom validate [ref]

Drafts
  keyloom draft new <name> [--from ref]
  keyloom draft list | show <name> [--layer L] | undo <name> | archive <name> | rm <name>
  keyloom set <draft> <layer> <key> <action> [--slot tap|hold|double-tap|tap-hold]
  keyloom clear <draft> <layer> <key> [--none]
  keyloom layer add <draft> <title> [--after L] [--copy-from L] [--fill trans|none] [--tag gaming]... [--color C]
  keyloom layer move|rename|rm|tag|color <draft> ...
  keyloom combo add <draft> <layer> <key>+<key>[+...] <action> [--name n] | rm <draft> <layer> <keys>
  keyloom apply <draft> <ops.json|->                            a batch of operations, all or nothing
  keyloom diff <refA> <refB>

Apply
  keyloom guide <draft> [--step]                                Oryx change list; --step walks it with LEDs
  keyloom verify <draft> [--against ref]                        compare with the newest imported revision

Live (Keymapp)
  keyloom status                                                Keymapp, keyboard, flashed layout, current layer
  keyloom live layer <L> [ref] | highlight <key>... [--layer L] [--color C] [--in ref] | reset

Other
  keyloom mcp                                                   MCP server on stdio
```

A reference (`ref`) is `layoutId`, `layoutId@revisionId`, `@revisionId`, a draft (`draft:wow` or just `wow`), or a
`.json` layout file; `layoutId@latest` fetches from Oryx first. Draft commands take the draft's name.

`--show` is best effort: if Keymapp isn't reachable the command still prints its result and says why nothing lit
up. What `--show` and `keyloom live …` change stays on the keyboard until `keyloom live reset`; a failing
`keyloom live …` command exits 1.

## ASCII render

Voyager, `--style legend` (default). Two-line cells: tap on line 1, hold (or `2×` double tap) on line 2.
A transparent key shows what it falls through to, with `▽` on line 2 (dimmed on a color terminal).
`--style codes` shows QMK short names: tap on line 1, hold on line 2 (`SPC` / `MO(1)`).

```
Voyager · 9qMJK@ZPjbXz · layer 0 "Main"
┌───────┬───────┬───────┬───────┬───────┬───────┐   ┌───────┬───────┬───────┬───────┬───────┬───────┐
│   =   │   1   │   2   │   3   │   4   │   5   │   │   6   │   7   │   8   │   9   │   0   │   ⌫   │
│  Esc  │       │       │       │       │       │   │       │       │       │       │       │       │
├───────┼───────┼───────┼───────┼───────┼───────┤   ├───────┼───────┼───────┼───────┼───────┼───────┤
…
└───────┴───────┴───────┴───────┴───────┴───────┘   └───────┴───────┴───────┴───────┴───────┴───────┘
                                ┌───────┬───────┐   ┌───────┬───────┐
                                │ Super │   ␣   │   │Brd+Sys│   ⌫   │
                                │       │Sym+Num│   │       │  Meh  │
                                └───────┴───────┘   └───────┴───────┘
```

- Legends come from `keyloom::labels`. Long legends are cut with `…`; `keyloom key <pos>` shows everything.
- `--all` renders every layer; no color when not a TTY or with `NO_COLOR` / `--no-color`.
- `find` and `free` print lists (position, slot, QMK expression, how to reach the layer), not a grid.
- Other geometries than the Voyager print a plain key list until they get a position map.
- Not implemented: an ASCII-only glyph mode and adjustable cell width.

## Config (`$KEYLOOM_HOME/config.toml`)

Comments and keys keyloom doesn't know are kept when it updates the file.

```toml
active_layout = "9qMJK"

[workspace]
git = true                    # commit every import / draft change in the workspace's own git repo

[validate]
disabled_rules = []           # e.g. ["duplicate-in-layer"]

[live]
highlight_color = "#00ffaa"
```
