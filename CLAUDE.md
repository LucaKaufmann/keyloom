# keyloom: notes for Claude

A CLI and MCP server (Rust) that lets AI agents see, audit and improve ZSA keyboard layouts. It imports
layouts from Oryx into a local git-backed workspace, renders and explores them, validates them, edits
drafts, and turns a draft into step-by-step Oryx instructions. It never writes to Oryx or flashes firmware.
Read `README.md`, then `docs/DESIGN.md` and `docs/STATUS.md` (status, what was checked on hardware).

## Build, run, check

```sh
cargo test --workspace                          # no network, no keyboard needed
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
KEYLOOM_BLESS=1 cargo test -p keyloom           # rewrite golden renders, then review the diff
cargo install --path crates/keyloom-cli         # the `keyloom` binary (CLI, and `keyloom mcp`)
KEYLOOM_HOME=/tmp/kl-test keyloom import 9qMJK  # try things in a throwaway workspace
KEYLOOM_DEBUG=1 keyloom …                       # debug logging on stderr
```

## Layout

- `crates/keyloom`: the engine, a library that never prints. `model` (lossless Oryx-shaped layout),
  `positions` (`L.home.4` names, fingers, effort), `qmk`, `render` (ASCII), `source` (Oryx GraphQL, Keymapp
  SQLite, files), `workspace` (store, drafts, git, lock), `explore`, `ops`, `diff`, `validate`, `guide`,
  `knowledge` (`data/`: keycodes.json, guides/*.md). `oryx` and `labels` are adapted from voyager-ghost
  (MIT, credited in LICENSE) and only compute key legends.
- `skills/keyloom/SKILL.md`: the Claude Code skill that teaches the CLI. Keep it in step with the CLI
  (every command in it should run).
- `crates/keyloom-cli`: the `keyloom` binary. `main` (CLI), `mcp` (rmcp, stdio), `app` (operations shared
  by both), `keymapp` (hand-rolled gRPC client for Keymapp's API, plus a fake server for tests), `live`.

## Rules

- The CLI and the MCP server expose the same operations: put shared logic in `app.rs`. Keep the MCP tool
  list small (16 tools, about 2,900 tokens): add an operation to `edit_draft` or an action to `keyboard`
  rather than a new tool. `--json` output is
  the same object the MCP tool returns.
- Never touch the user's real workspace (`~/.local/share/keyloom`) in tests or experiments: set
  `KEYLOOM_HOME` to a temporary directory (copy the real one if you need its data).
- Tests must not depend on the machine: e2e tests set `KEYMAPP_SOCKET` to a missing socket and
  `KEYLOOM_USB_SERIALS=""`.
- Live features change the physical keyboard; always `keyloom live reset` after trying them.
- The test fixture `oryx-9qMJK-ZPjbXz.json` is a real Voyager layout; `example-EXMPL-exmpl1.json` is made up.
  Hardware checks so far were done with Keymapp 1.3.7.

## Releasing

Bump `version` in `Cargo.toml`, add a `CHANGELOG.md` entry, commit, then push a tag `vX.Y.Z`.
`.github/workflows/release.yml` builds the binaries, publishes the GitHub release (with `install.sh`) and,
if the `HOMEBREW_TAP_TOKEN` secret is set, updates `Formula/keyloom.rb` in `LucaKaufmann/homebrew-tap`.
Without the secret, copy `keyloom.rb` from the release to the tap by hand.
