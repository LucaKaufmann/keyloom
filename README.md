# keyloom

A CLI and MCP server that lets AI agents **see, audit and improve** ZSA keyboard layouts.

Your layout lives in Oryx, ZSA's web configurator, where an AI agent can't read it. keyloom imports it into
local, versioned files and gives the agent what it needs to work with it: a picture of every layer, where
each key is and how hard it is to reach, how the layers connect, a keycode reference, validation rules and
layer-design guides. Changes happen in drafts. keyloom turns a finished draft into step-by-step instructions
for Oryx and checks the result after you flash.

```
Voyager · 9qMJK@ZPjbXz · layer 0 "Main"
┌───────┬───────┬───────┬───────┬───────┬───────┐   ┌───────┬───────┬───────┬───────┬───────┬───────┐
│   =   │   1   │   2   │   3   │   4   │   5   │   │   6   │   7   │   8   │   9   │   0   │   ⌫   │
│  Esc  │       │       │       │       │       │   │       │       │       │       │       │       │
├───────┼───────┼───────┼───────┼───────┼───────┤   ├───────┼───────┼───────┼───────┼───────┼───────┤
│   ⇥   │   Q   │   W   │   F   │   P   │   B   │   │   J   │   L   │   U   │   Y   │   ;   │   \   │
│ Ctrl  │       │       │       │       │       │   │       │       │       │       │       │       │
…
```

**Supported keyboard:** ZSA Voyager.
**Not a flasher:** keyloom never writes to Oryx and never builds or flashes firmware. You apply its change
list in Oryx and flash as usual.

## Contents

- [What you can do with it](#what-you-can-do-with-it)
- [Install](#install)
- [Set up your agent](#set-up-your-agent)
- [Quick start](#quick-start)
- [Making a change, start to finish](#making-a-change-start-to-finish)
- [Naming keys, layers and actions](#naming-keys-layers-and-actions)
- [On the keyboard (optional)](#on-the-keyboard-optional)
- [Command reference](#command-reference)
- [Where your data lives](#where-your-data-lives)
- [Limits](#limits)
- [Troubleshooting](#troubleshooting)

## What you can do with it

Things to ask an agent once keyloom is set up:

| You ask | What happens |
|---|---|
| "Explain my layout to me." | The agent reads every layer and walks you through it: what each layer is for and how you get in and out. |
| "Review my layout. What would you change?" | Runs the 14 validation rules, then an ergonomics review: awkward reaches, wasted easy keys, conflicting hold keys. |
| "Where is Esc? How do I type `~`?" | Finds the key on every layer and the keys you hold to get there. |
| "Add a gaming layer I can toggle from the left thumb." | Creates a draft, adds the layer following the gaming guide (no tap-hold keys, a way back out), and gives you the Oryx steps. |
| "Add a WoW layer with my action bars on the left hand." | Same, using the MMO guide: number row and modifiers under the left hand. |
| "I never use my bottom-right keys. What should go there?" | Lists free keys, easiest first, and suggests what to bind. |
| "Set up home-row mods." | Drafts them using the home-row-mods guide and warns about the usual pitfalls. |
| "What changed between my last two revisions?" | Diffs any two revisions or drafts, key by key. |
| "Show me which keys you mean." | Lights those keys on the keyboard (needs Keymapp, see [below](#on-the-keyboard-optional)). |
| "I flashed it. Did I get everything right?" | Re-imports the layout and checks it against the draft. |

Without an agent, keyloom is also a handy terminal tool: print a layer, search for a keycode, validate a
layout in a script, or keep a git history of your layout's revisions.

## Install

You need a [Rust toolchain](https://rustup.rs) (and a C compiler, which macOS and most Linux systems have).

```sh
git clone https://github.com/LucaKaufmann/keyloom.git
cd keyloom
cargo install --path crates/keyloom-cli
keyloom --version
```

This puts one binary, `keyloom`, in `~/.cargo/bin`. It is both the CLI and the MCP server (`keyloom mcp`).

Developed and tested on macOS. Linux should work; Windows is untested.

## Set up your agent

Pick the one that matches your agent. You don't need both.

### Agents with a shell (Claude Code, Codex, Cursor, …)

These can run the `keyloom` CLI directly; every command takes `--json`. For Claude Code, install the skill.
It teaches Claude the commands and only loads when you talk about your keyboard:

```sh
mkdir -p ~/.claude/skills/keyloom
cp skills/keyloom/SKILL.md ~/.claude/skills/keyloom/
```

For other agents, point them at [skills/keyloom/SKILL.md](skills/keyloom/SKILL.md) or at `keyloom --help`.

### Agents without a shell (Claude Desktop, chat apps, IDE chat panels)

Add keyloom as an MCP server over stdio:

```json
{ "mcpServers": { "keyloom": { "command": "keyloom", "args": ["mcp"] } } }
```

In Claude Code that is `claude mcp add keyloom -- keyloom mcp`.

The server has 16 tools (about 3,000 tokens of context), reference resources (keycodes, the Voyager's keys,
layer actions, design guides) and four prompts: `review_layout`, `explain_layout`, `design_layer` and
`add_gaming_layer`. It only ever edits drafts.

## Quick start

1. Open your layout in Oryx and copy the link from the address bar. It looks like
   `https://configure.zsa.io/voyager/layouts/<id>/latest/0`. The layout has to be public.
2. Import it:

   ```sh
   keyloom import https://configure.zsa.io/voyager/layouts/<id>/latest/0
   ```

   If you use Keymapp, `keyloom import keymapp` imports every revision Keymapp has stored instead.
3. Look around:

   ```sh
   keyloom show                 # the base layer; -l Gaming for another, --all for every layer
   keyloom layers               # how each layer is reached and left
   keyloom find Esc             # where a key is, and how to get there
   keyloom validate             # the audit
   ```

4. Or ask your agent: "Review my keyboard layout."

`keyloom layers` on a five-layer layout:

```
Voyager · 9qMJK@GGKdog
   0  Main         52 bound,  0 transparent   base layer
   1  Num+Nav      44 bound,  8 transparent   hold L.thumb.2 (MO(1)) [numbers,nav]
   2  Gaming        1 bound, 51 transparent   no key leads here
   3  Mouse         8 bound, 44 transparent   move the trackball (auto-mouse layer)
   4  Brd+Sys      43 bound,  9 transparent   hold R.thumb.2 (MO(4))
```

and `keyloom validate` on the same layout:

```
warn  [layer-unreachable] Gaming: no key leads to layer "Gaming"
       → add a layer key (MO/TG/TO/OSL) on a reachable layer; …
```

## Making a change, start to finish

Imported revisions never change. You edit a draft, then carry the draft over to Oryx by hand.

```sh
keyloom draft new wow                          # 1. an editable copy of your layout
keyloom layer add wow WoW --after Gaming --fill none   # 2. edit it
keyloom set wow Main L.thumb.1 'TG(WoW)'
keyloom set wow WoW L.thumb.1 'TG(WoW)'           #    the same key leaves the layer again
keyloom set wow WoW L.home.1 A
keyloom validate wow                           # 3. check it
keyloom diff <id> draft:wow                    #    and see what changed
keyloom guide wow                              # 4. the steps to make in Oryx
#    … make the changes in Oryx, compile, flash …
keyloom import <id>                            # 5. fetch the new revision
keyloom verify wow                             #    and confirm it matches the draft
```

`keyloom draft undo wow` takes back the last edit. `keyloom guide wow --step` walks through the Oryx steps
one at a time and lights the key for each step on the keyboard.

An agent does steps 1 to 4 for you and hands you the guide.

## Naming keys, layers and actions

**Keys** are named by position: `L.home.4` is the left half, home row, column 4.

- Halves: `L`, `R`.
- Rows: `num`, `top`, `home`, `bot`.
- Columns: 0 to 5, counted from the **outer** edge on both halves. 0 is the outer pinky column, 5 the inner
  index column.
- Thumbs: `L.thumb.1` (next to the main block) and `L.thumb.2` (toward the centre).

You can also write `key:Esc` (the base-layer key that sends Esc) or an index like `#17`.

**Layers** are named by number or title. Case doesn't matter and a unique prefix is enough: `num` finds
"Num+Nav".

**Actions** use QMK-style names: `A`, `Esc`, `LCTL(KC_C)`, `MO(3)`, `TG(Gaming)`, `LT(1, KC_SPC)`,
`LCTL_T(KC_A)`, `TRNS`, `NO`. Quote them in the shell. `keyloom ref <word>` searches the keycode reference.

## On the keyboard (optional)

keyloom can light keys and switch layers on the keyboard, so you can see what the agent means. This goes
through [Keymapp](https://www.zsa.io/flash), ZSA's desktop app:

1. Start Keymapp and connect the keyboard.
2. Turn on its API: Keymapp → Settings → API.

```sh
keyloom status                         # Keymapp, the keyboard, the flashed revision
keyloom live highlight L.home.1 L.home.2 --color '#ff0000'
keyloom live layer Gaming
keyloom live reset                     # put the lights and layer back
```

Everything else works without Keymapp.

## Command reference

| Command | What it does |
|---|---|
| `import`, `list`, `use` | Import layouts, list them, pick the default |
| `show`, `layers`, `summary` | Draw layers, show how they connect |
| `key`, `find`, `free` | One key on every layer, where a keycode is, free keys (easiest first) |
| `ref`, `guides` | Keycode reference, layer-design guides |
| `validate`, `diff` | Audit a layout, compare two |
| `draft`, `set`, `clear`, `layer`, `combo`, `apply` | Edit drafts (`apply` takes a JSON file of edits, all or nothing) |
| `guide`, `verify` | Oryx steps for a draft, check after flashing |
| `status`, `live` | Keymapp and the keyboard |
| `export` | Write a layout to a file |
| `mcp` | Run the MCP server |

`keyloom help <command>` has the details. Every command takes `--json` and prints the same object the
matching MCP tool returns. `validate` and `verify` exit with code 3 when they find errors.

## Where your data lives

Layouts live in `~/.local/share/keyloom` (set `KEYLOOM_HOME` to change it). It is a git repository that
keyloom commits to after every change, so you have a full history. Imported revisions never change; drafts
keep a log of their edits.

keyloom talks to two things: Oryx's public API (to fetch a layout you name) and Keymapp on your own machine.
Nothing else leaves your computer.

## Limits

- Voyager only. Other ZSA boards (Moonlander, ErgoDox EZ) aren't supported yet.
- Read-only towards Oryx: you make the changes there yourself, following the guide.
- Private Oryx layouts can't be fetched. Make the layout public or use `keyloom import keymapp`.
- Combos, tap dance and the `TG`, `OSL`, `TT` and `DF` layer keys follow Oryx's format as far as it is
  known, but haven't been checked against a real layout that uses them. `OSM` and `LM` are rejected for now.
- keyloom is an independent project. It isn't affiliated with or endorsed by ZSA, and it relies on Oryx and
  Keymapp interfaces that ZSA may change.

## Troubleshooting

| Message | Fix |
|---|---|
| `no layouts yet` | Run `keyloom import <oryx link>` first. |
| `there is no public layout …` | Check the link or id. Private layouts can't be fetched. |
| `Keymapp's API isn't reachable` | Start Keymapp and turn on Settings → API. |
| `no keyboard is connected` | Check the cable, or quit any other app that holds the keyboard. |
| Lights stay on after a crash | `keyloom live reset`. |

`KEYLOOM_DEBUG=1 keyloom …` prints debug logging on stderr.

## More

- [docs/DESIGN.md](docs/DESIGN.md): goals, decisions, where the build differs from the specs
- [docs/STATUS.md](docs/STATUS.md): what works, what has been checked on real hardware, roadmap
- [docs/specs/](docs/specs/): layout format, engine, apply guide, CLI, MCP, live features, knowledge
- [docs/oryx-encodings.md](docs/oryx-encodings.md): how Oryx stores layouts
- [CHANGELOG.md](CHANGELOG.md)
- [CLAUDE.md](CLAUDE.md): how to build, test and contribute

## License

MIT. keyloom started as a fork of [voyager-ghost](https://gitlab.com/tbulva/voyager-ghost) by Tomas Bulva
(MIT). Two small modules adapted from it are credited in [LICENSE](LICENSE).
