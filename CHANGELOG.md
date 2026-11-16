# Changelog

## Unreleased

- Key and layer colors are stored in upper case (`#FF8800`), the way Oryx's editor stores them.
- New validation warning `oryx-clears-key`. When a lower layer has `TG(n)`, `TT(n)`, `MO(n)` or `LM(n)` at a
  position, Oryx's editor clears any key at that position on layer n as soon as it shows the layer, so
  the lower key shows through. Oryx's API stores such a key, so a pushed draft read back fine and then
  lost it on opening. `push` now warns about these keys too.
- `layer-no-exit` judges exits by what survives Oryx's editor. A layer toggled from a held layer with
  `TG` at the same position to leave again has no way out once opened in Oryx; this is now an error. Its
  hint suggests `TO(0)` at a different position.
- The gaming, MMO, numbers, mouse and layer-design guides, the skill and the gaming prompt no longer
  suggest the same `TG` key on the toggled layer. They suggest leaving that position transparent when
  the toggle is on the base layer, and `TO(0)` at another position when it isn't.

## 0.2.0 (2026-11-15)

- `keyloom push <draft>` (MCP: `apply_guide` with `push: true`): creates a new Oryx layout holding the draft,
  without a login, and prints its link. Your own layouts are never changed.
- `keyloom push <draft> --to <layout>` (MCP: `apply_guide` with `to`): writes the draft into one of your own
  Oryx layouts as an uncompiled revision, using your login token from `KEYLOOM_ORYX_TOKEN` or the macOS
  keychain. Refuses to replace changes on Oryx the draft doesn't have unless `--overwrite` is given.
- `keyloom push <draft> --to new`: creates a new layout in your Oryx account holding the draft, with the same
  token. Existing layouts are not changed.

## 0.1.0 (2026-11-14)

First release.

- Import ZSA Voyager layouts from Oryx (link or id), from a file, or from Keymapp's database, into a local
  git-backed workspace.
- See: render layers, look up keys and keycodes, list free keys by effort, show how layers connect, search
  the keycode reference, read layer-design guides.
- Audit: 14 validation rules, diffs between revisions and drafts.
- Improve: drafts with typed edits and undo, a step-by-step change list for Oryx, and `verify` after flashing.
- On the keyboard, through Keymapp's API: light keys, switch layers, walk the Oryx steps key by key.
- For agents: `--json` on every command, an MCP server (`keyloom mcp`, 16 tools, resources, 4 prompts) and
  a Claude Code skill.
