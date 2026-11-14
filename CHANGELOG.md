# Changelog

## Unreleased

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
