# keyloom

A CLI and MCP server that lets AI agents **see, audit and improve** ZSA keyboard layouts.

Your layout lives in Oryx, ZSA's web configurator, where an AI agent can't read it. keyloom imports it into
local, versioned files and gives the agent what it needs to work with it: a picture of every layer, where
each key is and how hard it is to reach, how the layers connect, a keycode reference, validation rules and
layer-design guides. Changes happen in drafts. keyloom gets a finished draft into Oryx, either as step-by-step instructions or
as a new Oryx layout it creates for you, and checks the result after you flash.

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
**Not a flasher:** keyloom never builds or flashes firmware, and it only writes to one of your own Oryx
layouts when you ask it to with `push --to`. You compile and flash in Oryx as usual.

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

With Homebrew (macOS and Linux):

```sh
brew install lucakaufmann/tap/keyloom
```

Or with the install script, which downloads the binary for your system to `~/.local/bin`:

```sh
curl -fsSL https://github.com/LucaKaufmann/keyloom/releases/latest/download/install.sh | sh
```

Or download a binary from the [releases page](https://github.com/LucaKaufmann/keyloom/releases), or build
from source with a [Rust toolchain](https://rustup.rs):

```sh
cargo install --git https://github.com/LucaKaufmann/keyloom keyloom-cli
```

Any of these gives you one binary, `keyloom`. It is both the CLI and the MCP server (`keyloom mcp`). Check
it with `keyloom --version`.

There are binaries for macOS (Apple silicon and Intel) and Linux (x86-64 and ARM). keyloom is developed on
macOS; on Linux it builds and passes its tests, but the keyboard features haven't been tried there. Windows
is untested.

## Set up your agent

Pick the one that matches your agent. You don't need both.

### Agents with a shell (Claude Code, Codex, Cursor, …)

These can run the `keyloom` CLI directly; every command takes `--json`. For Claude Code, install the skill.
It teaches Claude the commands and only loads when you talk about your keyboard:

```sh
mkdir -p ~/.claude/skills/keyloom
curl -fsSL https://raw.githubusercontent.com/LucaKaufmann/keyloom/main/skills/keyloom/SKILL.md \
  -o ~/.claude/skills/keyloom/SKILL.md
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
keyloom guide wow                              # 4a. the steps to make in Oryx yourself
keyloom push wow                               # 4b. or: a new Oryx layout with the draft in it
keyloom push wow --to <id>                     # 4c. or: an uncompiled revision of your own layout (needs a token)
keyloom push wow --to new                      # 4d. or: a new layout in your account (needs a token)
#    … in Oryx: make the changes (4a) or open the link (4b to 4d), compile, flash …
keyloom import <id>                            # 5. fetch the new revision
keyloom verify wow                             #    and confirm it matches the draft
```

`keyloom draft undo wow` takes back the last edit. `keyloom guide wow --step` walks through the Oryx steps
one at a time and lights the key for each step on the keyboard.

An agent does steps 1 to 4 for you and hands you the guide or the link.

### `keyloom push`

`keyloom push <draft>` asks Oryx for a copy of the revision your draft started from, writes the draft's
layers into that copy, reads it back to check, and prints its link:

```
Created a new layout on Oryx from draft:wow: https://configure.zsa.io/voyager/layouts/Xx6vM/latest/0
  copied 9qMJK@GGKdog, then wrote 4 layer(s), added 1, removed 0
  ✓ read back from Oryx: layers and keys match the draft
```

- It sends no login, so it can't change anything in your account. The copy is a new, public layout that
  belongs to no account, like one made by a visitor who isn't signed in.
- The copy keeps your trackball, RGB and timing settings. Combos aren't written yet; if the draft changes
  them, keyloom says so.
- It doesn't compile or flash. Open the link, look it over, then compile and flash in Oryx.
- It uses the calls Oryx's own website uses. They are undocumented and ZSA may change them; `keyloom guide`
  always works. See [docs/oryx-write-api.md](docs/oryx-write-api.md).

### `keyloom push --to`: into your own layout

`keyloom push <draft> --to <layout id or link>` writes the draft into a layout in your Oryx account, as a new
uncompiled revision. You open it in Oryx, look at the changes, and compile and flash when you are happy.

```
Wrote draft:wow into your layout 9qMJK on Oryx as an uncompiled revision: https://configure.zsa.io/voyager/layouts/9qMJK/latest/0
  started from 9qMJK@jZjnDa, then wrote 2 layer(s), added 1, removed 0
  ✓ read back from Oryx: layers and keys match the draft
```

This needs your Oryx login token, because only you may change your layouts:

1. Sign in at [configure.zsa.io](https://configure.zsa.io), open the browser's developer console and run
   `localStorage.getItem("jwtToken")`.
2. Give it to keyloom in one of two ways. keyloom reads it and never stores it anywhere else.

   ```sh
   export KEYLOOM_ORYX_TOKEN='eyJ…'                                    # for this shell
   security add-generic-password -U -a oryx -s keyloom-oryx -w        # macOS keychain; asks for the token
   ```

The token gives full access to your Oryx account for months. Treat it like a password; signing out of Oryx is
the way to stop using it.

What `push --to` will and won't do:

- It never compiles, and it can't change a revision you already compiled: Oryx freezes those.
- It refuses, before writing anything, if the layout's newest compiled revision isn't the one your draft
  started from. Import the layout again and redo the draft, so the push can't undo newer changes.
- It refuses if the layout has uncompiled changes on Oryx that didn't come from this draft, because it would
  write over them.
- `--overwrite` skips both checks.
- Pushing the same draft again updates the same uncompiled revision.

### `keyloom push --to new`: a new layout in your account

`keyloom push <draft> --to new` creates a new layout in your Oryx account that holds the draft, with the same
token. Your existing layouts get no new revision. The new layout keeps the trackball, RGB and timing settings
of the revision the draft started from.

```
Created a new layout in your Oryx account from draft:wow: https://configure.zsa.io/voyager/layouts/jy5bY/latest/0
  copied 9qMJK@GGKdog, then wrote 4 layer(s), added 1, removed 0
  ✓ read back from Oryx: layers and keys match the draft
```

Oryx only copies a layout into your account when the source isn't yours, so keyloom first makes a copy that
belongs to no account and then copies that one. The first copy stays on Oryx as a public layout nobody owns.

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
| `guide`, `push`, `verify` | Oryx steps for a draft, or a new Oryx layout holding it; check after flashing |
| `status`, `live` | Keymapp and the keyboard |
| `export` | Write a layout to a file |
| `mcp` | Run the MCP server |

`keyloom help <command>` has the details. Every command takes `--json` and prints the same object the
matching MCP tool returns. `validate` and `verify` exit with code 3 when they find errors.

## Where your data lives

Layouts live in `~/.local/share/keyloom` (set `KEYLOOM_HOME` to change it). It is a git repository that
keyloom commits to after every change, so you have a full history. Imported revisions never change; drafts
keep a log of their edits.

keyloom talks to two things: Oryx (to fetch a layout you name and, only when you run `keyloom push`, to
write a draft there) and Keymapp on your own machine. Nothing else leaves your computer. Your Oryx login
token is only sent to Oryx, and only by `push --to`.

## Limits

- Voyager only. Other ZSA boards (Moonlander, ErgoDox EZ) aren't supported yet.
- keyloom doesn't edit your existing Oryx layouts unless you run `push --to <layout>`. `guide` gives you the steps to
  make yourself; plain `push` creates a separate new layout.
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
