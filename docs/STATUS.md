# Status and roadmap

Version 0.1.0, 2026-11-14.

## What works

Everything below is implemented and covered by 112 tests (engine, CLI, MCP over stdio, a fake Keymapp gRPC
server, golden renders). The tests need no network and no keyboard.

| Area | Commands | Spec |
| --- | --- | --- |
| Import and storage | `import` (Oryx link or id, file, Keymapp's database), `list`, `use`, `export` | [01](specs/01-layout-format.md) |
| See | `show`, `layers`, `summary`, `key`, `find`, `free`, `ref`, `guides` | [02](specs/02-engine.md), [07](specs/07-knowledge.md) |
| Audit | `validate` (14 rules), `diff` | [02](specs/02-engine.md) |
| Improve | `draft`, `set`, `clear`, `layer`, `combo`, `apply`, undo | [02](specs/02-engine.md) |
| Carry over to Oryx | `guide`, `verify` | [03](specs/03-apply-guide.md) |
| On the keyboard | `status`, `live`, `--show`, `guide --step` | [06](specs/06-live.md) |
| Agents | `keyloom mcp` (16 tools, resources, 4 prompts), the Claude Code skill | [05](specs/05-mcp.md) |

## Checked on real hardware

On a Voyager with Keymapp 1.3.7 (API on), macOS, 2026-11-14:

- Keymapp API: status, `live layer`, `live highlight`, `live reset`.
- Keymapp's `firmware_version` is `<layout>/<revision>` (for example `9qMJK/ZPjbXz`), so keyloom finds the
  flashed revision.
- USB serial detection on macOS: `ioreg` shows the Voyager (vendor 12951) with the same
  `<layout>/<revision>` serial.
- LED index = Oryx key index: `L.num.0`, `R.bot.0` and `L.thumb.2` lit the right keys.
- `guide --step`: next, back, `q` and Ctrl-C (exit 130) all restore the lights and clear the saved state.
- Keymapp reports a layer switch in its status after up to a second, not immediately. keyloom doesn't read
  it back right after switching, so nothing depends on this.
- The MCP server against the real Keymapp, including a full draft session (add a layer, set keys, validate,
  guide, undo, verify) in a copy of the workspace.
- After a replug or a disconnect Keymapp doesn't reconnect by itself; keyloom asks it to. Just after that
  Keymapp refuses commands for a moment ("keyboard requires an updated firmware") and can refuse to
  connect. keyloom retries both briefly. Checked with replugging and with repeated disconnects.

## Not checked yet

- Oryx encodings for combos, `TG`, `OSL`, `TT`, `DF` and tap dance are written as
  [the encoding notes](oryx-encodings.md) predict, but no real layout using them has been seen. `OSM` and
  `LM` are rejected until their encoding is known.
- The guide's Oryx wording ("Layer → Toggle", "+ next to the layer list") is a best guess at Oryx's UI.
- Linux and Windows: the code has paths for both (Keymapp socket or port, USB serial via sysfs on Linux),
  but only macOS has been run. CI builds and tests on Linux.
- The real Oryx layer limit. keyloom warns above 8 layers.

Where the build differs from the specs: see the end of [DESIGN.md](DESIGN.md).

## Roadmap

Not planned in detail. Roughly in order of usefulness:

- Confirm the unchecked encodings above with real layouts.
- Easier install: crates.io, prebuilt binaries, Homebrew.
- Other ZSA keyboards (Moonlander, ErgoDox EZ). The engine, format and rendering are geometry-generic;
  what's missing is position names, the effort map and testing.
- QMK `keymap.c` export.

Out of scope for now: writing to Oryx, building or flashing firmware, a GUI.

## Risks

| Risk | Impact | Mitigation |
| --- | --- | --- |
| Oryx's GraphQL API is undocumented and may change | Import breaks | Lenient decoding, stored revisions keep working offline, fixtures, one adapter module |
| Oryx encodings keyloom doesn't understand (tap dance, macros) | Wrong renders, lossy diffs | The model keeps raw JSON for unknown parts and shows "?" rather than guessing |
| Oryx's UI differs from keyloom's edit model | Guide steps are hard to follow | The guide speaks in Oryx's terms; `verify` catches mistakes |
| Keymapp's API is missing, disabled, or the keyboard is held by another app | No live features | They're optional, and the messages say why |

## History

keyloom started in November 2026 as a fork of [voyager-ghost](https://gitlab.com/tbulva/voyager-ghost), a
keyboard overlay app, and at first could show drafts in that overlay. It became a standalone project so
that an agent-facing CLI and MCP server wouldn't need a separate macOS app. Two small modules adapted from
voyager-ghost remain (`oryx.rs`, `labels.rs`).
