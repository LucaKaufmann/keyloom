# keyloom design: see, audit and improve ZSA keyboard layouts, for AI agents and the terminal

> Binary `keyloom` (short alias `kl`), crates `keyloom` (engine) and `keyloom-cli`, env `KEYLOOM_HOME`.
> Status: implemented and checked on a real Voyager (November 2026); see [STATUS.md](STATUS.md).

## The idea in one paragraph

`keyloom` imports your ZSA keyboard layouts from Oryx into local, versioned files and lets you **explore** them
from the terminal: render layers, find keys, see how layers connect, spot problems. It lets you and AI agents
**draft** changes ("add a gaming layer", "add a WoW layer") safely as drafts, validates them, and turns
a draft into a **step-by-step change list to apply in Oryx**. Optionally it **lights the keys up on the
physical keyboard** through Keymapp. The same functionality is exposed as a CLI and as an MCP server, so an
agent knows the layout, the keycodes and the Voyager's physical layout as well as you do.

## Goals (v1)

1. Import Oryx layouts (public GraphQL, Keymapp's local DB) into our own local format, keeping every revision.
2. Explore layouts in the terminal: ASCII renders, key lookup, free keys, layer graph, a keycode reference.
3. Drafts: edit copies of a layout with typed operations, `validate`, `diff`.
4. Apply guide: an ordered Oryx change list for a draft, plus `verify` after you have applied and flashed it.
5. Live: highlight keys and switch layers on the real keyboard through Keymapp's API (kontroll).
6. MCP server with the same capabilities plus reference knowledge and prompt templates for agents.
7. Import and export layouts as files.

## Non-goals (v1)

- Building firmware (QMK), flashing. Oryx stays the place where firmware is compiled and flashed from.
- Writing to Oryx (no public write API; the private one would be fragile and possibly against its terms).
- A GUI. keyloom is a CLI and an MCP server; its pictures are ASCII renders.
- Two-way sync with Oryx. The flow is one way: Oryx → keyloom → change list → you in Oryx → `keyloom verify`.

The format stays close to Oryx and QMK so a `keymap.c` export can be added later without a redesign.

## Decisions so far

| Decision | Choice | Why |
| --- | --- | --- |
| Project | Standalone (started as a fork of [voyager-ghost](https://gitlab.com/tbulva/voyager-ghost), MIT; its overlay was dropped and the few borrowed pieces live in `oryx.rs` and `labels.rs`) | A CLI/MCP for agents needs no GUI |
| Language | Rust, two crates | `rmcp` is Rust; one static binary for CLI + MCP |
| Keyboard control | Keymapp's gRPC API (the proto from [zsa/kontroll](https://github.com/zsa/kontroll)) | Official, gives layer switch + per-LED RGB |
| UI | Terminal first | Smallest useful surface; agents use MCP anyway |
| Write path | Drafts + Oryx change list, no flashing | Removes the riskiest part (toolchain, flashing) |

## Documents

- [STATUS.md](STATUS.md): what works, what has been checked on hardware, roadmap, risks
- [oryx-encodings.md](oryx-encodings.md): how Oryx stores layouts
- [specs/01-layout-format.md](specs/01-layout-format.md): workspace on disk, layout file, key positions, references
- [specs/02-engine.md](specs/02-engine.md): explore queries, edit operations, validation rules, diff
- [specs/03-apply-guide.md](specs/03-apply-guide.md): draft → Oryx change list, `verify`
- [specs/04-cli.md](specs/04-cli.md): commands, flags, output formats, ASCII render
- [specs/05-mcp.md](specs/05-mcp.md): MCP tools, resources, prompts, safety rules
- [specs/06-live.md](specs/06-live.md): Keymapp/kontroll integration, LEDs, flashed-layout detection
- [specs/07-knowledge.md](specs/07-knowledge.md): keycode reference, Voyager ergonomics map, layer-design guides

## Glossary

- **Oryx**: ZSA's web configurator (configure.zsa.io). A *layout* has a `hashId` (e.g. `9qMJK`); each saved
  version is a *revision* with its own `hashId`.
- **Keymapp**: ZSA's desktop app (flashing, live layer view). Has an optional local gRPC API.
- **Revision (in keyloom)**: an immutable snapshot imported from Oryx.
- **Draft**: a mutable copy of a revision that keyloom, you or an agent edit.
- **Position**: a physical key, named like `L.home.4` (left half, home row, 4th column from the outer edge).

## Quick start

```sh
keyloom import https://configure.zsa.io/voyager/layouts/9qMJK/latest/0   # or `keyloom import keymapp`
keyloom show                      # base layer; -l sym, --all, --style codes, --json
keyloom layers                    # how layers are reached and left
keyloom find Esc                  # where is it, and how do I get there
keyloom free -l Gaming            # free keys, easiest first (a draft: keyloom free wow -l Gaming)
keyloom validate

keyloom draft new wow             # an editable copy
keyloom layer rename wow Gaming WoW && keyloom layer tag wow WoW gaming
keyloom set wow Main L.thumb.1 'TG(WoW)'
keyloom set wow WoW L.thumb.1 'TG(WoW)'
keyloom set wow WoW L.home.1 A    # … or let an agent do it (below)
keyloom draft show wow
keyloom guide wow                 # the steps to make in Oryx (--step walks them with the keys lit)
# apply in Oryx, compile, flash, then:
keyloom import 9qMJK && keyloom verify wow

keyloom status                    # Keymapp + flashed layout
keyloom find Esc --show           # light it up on the keyboard (Keymapp API on)
```

Key positions: `L.home.4` = left half, home row, column 4 counted from the outer edge (0 outer pinky …
5 inner index). Thumbs `L.thumb.1` (next to the main block), `L.thumb.2` (toward the centre).
`keyloom guides` lists the layer-design guides; `keyloom ref <query>` searches keycodes.

## Agents (MCP)

```sh
claude mcp add keyloom -- keyloom mcp
```

Then e.g. "Use keyloom to add a WoW layer to my keyboard" or the `add_gaming_layer` prompt. The server
offers 16 tools (import, explore, audit, drafts, guide/verify, the keyboard), the keycode/position/guide resources, and
the prompts `design_layer`, `add_gaming_layer`, `review_layout`, `explain_layout`. It only edits drafts.

## Development

```sh
cargo test --workspace                         # engine + CLI tests (no network, no keyboard needed)
KEYLOOM_BLESS=1 cargo test -p keyloom          # rewrite golden renders in crates/keyloom/tests/golden, then review the diff
cargo clippy --workspace --all-targets -- -D warnings
cargo install --path crates/keyloom-cli        # installs `keyloom`; `ln -sf ~/.cargo/bin/keyloom ~/.cargo/bin/kl` for the alias
KEYLOOM_HOME=/tmp/kl-test keyloom import 9qMJK # try things in a throwaway workspace
```

Crates: `crates/keyloom` (engine: `model`, `positions`, `qmk`, `render`, `source`, `summary`,
`workspace`, `explore`, `ops`, `diff`, `validate`, `guide`, `knowledge`, plus `oryx` and `labels` for key
legends) and `crates/keyloom-cli` (the `keyloom` binary: `main` CLI, `mcp` server, `app` shared operations,
`keymapp` client, `live`).

## Where the build differs from the specs

- **Keymapp client:** not the `kontroll` crate (it needs `protoc` at build time) but
  `keyloom-cli/src/keymapp.rs`; spec 06 describes it. Tested against a fake gRPC server and Keymapp 1.3.7.
- **Layout file (spec 01):** an extra `settings` object (`autoMouseLayer`, read from `config.h` in Oryx's
  generated source zip at import). It's the only part of a stored revision that a later import may fill in.
- **Undo (spec 02):** replays the op log on the draft's starting copy (`<name>.base.json`) instead of
  storing snapshots.
- **Extra commands/tools:** `keyloom apply <draft> <ops.json>`, `keyloom guides`. `OSM(...)` and `LM(...)` are rejected until their Oryx encoding is known.
- **Workspace default:** `~/.local/share/keyloom`; `cache/` and `.lock` are git-ignored. keyloom
  only commits to a git repo it created itself, and locks the workspace while it writes.
