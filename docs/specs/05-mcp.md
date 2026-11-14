# 05: MCP server

`keyloom mcp` runs an MCP server on stdio using the official Rust SDK (`rmcp`). Same engine as the CLI; tool
results are the CLI's `--json` objects, plus a text rendering where a picture of the board helps.

Setup for Claude Code: `claude mcp add keyloom -- keyloom mcp`. In Claude Code the skill in
`skills/keyloom/SKILL.md` (CLI only, loaded on demand) is the cheaper alternative; the MCP server is for clients
without a shell and for those who prefer typed tools.

## Design rules

1. **Knowledge first.** The agent should never guess a keycode, a position or how layers interact; there is
   a tool or resource for each.
2. **Write only to drafts.** No tool can modify a revision. Write tools require a `draft` argument.
3. **Every write returns the diff** of that call plus validation findings, so the agent sees consequences.
4. **Compact by default.** Full layouts are big (52 keys × N layers). `summary` and `render_layer` are the
   default views; full JSON only on request.
5. **Positions by name** (`L.home.4`) in all outputs, with index alongside.
6. **No hidden side effects.** The `keyboard` tool (LEDs, layer switch) is separate, opt-in and always restorable.
7. **Few tools.** Every tool's schema costs context in every session. One edit tool takes a list of operations
   instead of a tool per operation; one keyboard tool takes an `action`. 16 tools, about 2,900 tokens.

## Tools

Read:

| Tool | Purpose |
| --- | --- |
| `list_layouts` | layouts, revisions, drafts, which is active |
| `import_layout(source)` | import from Oryx (link/id), a file, or Keymapp; also the refresh before `verify_draft` |
| `layout_summary(layout?, graph?)` | layers, tags, how each is reached and left, combos, counts; `graph` adds every layer key as an edge |
| `render_layer(layout?, layer?, style?, json?)` | ASCII grid; `json` adds the structured grid |
| `key_info(layout?, key, layer?)` | one key on one or all layers, finger, effort |
| `find_keycode(layout?, query)` | where a key/function lives |
| `free_keys(layout?, layer)` | empty/transparent keys sorted by effort |
| `lookup_keycode(query, limit?)` | reference search: codes, aliases, descriptions, Oryx support |
| `validate(layout?)` | findings (the audit) |
| `diff(from, to)` | structured diff |

Write (drafts only):

| Tool | Purpose |
| --- | --- |
| `create_draft(name, from?)` | |
| `edit_draft(draft, ops[])` | atomic list of edit operations (02): JSON objects whose actions are action-spec strings. Unknown fields are errors |
| `undo(draft)` | undo the last `edit_draft` |
| `apply_guide(draft, push?, to?, overwrite?, force?)` | Oryx change list (03); with `push` a new Oryx layout holding the draft and its link, or with `to` an uncompiled revision of the user's own layout (docs/oryx-write-api.md). The login token is read by the server from the environment or keychain, never passed as an argument |
| `verify_draft(draft, against?)` | after the user applied and flashed |

Live:

| Tool | Purpose |
| --- | --- |
| `keyboard(action, layout?, layer?, keys?, color?)` | `status`, `show_layer`, `highlight`, `reset` (undo keyloom's changes to LEDs and layer) |

Full layout JSON isn't a tool: `render_layer(json=true)` has every key, and `keyloom export` writes the file.

## Resources

```
keyloom://reference/keycodes            categorized keycode list (07)
keyloom://reference/voyager             positions, fingers, effort map, ASCII position chart
keyloom://reference/layer-actions       MO / LT / TG / TO / TT / OSL / DF explained, with pros/cons
keyloom://guides/{topic}                gaming, mmo, nav, symbols, numbers, mouse, home-row-mods, layer-design
keyloom://layouts/{id}/summary          same as layout_summary
```

## Prompts

| Prompt | Arguments | What it does |
| --- | --- | --- |
| `design_layer` | purpose, layout?, notes? | Interview (what for, which hand on mouse, which keys matter), read the guide for the purpose, look at free keys and the layer graph, propose a plan, then build it in a draft and validate |
| `add_gaming_layer` | game, layout? | `design_layer` specialised: game-specific guide (e.g. MMO), no tap-hold, toggle in and out |
| `review_layout` | layout? | validate + ergonomics review + suggestions, no edits |
| `explain_layout` | layout? | teach the user their own layout layer by layer |

Prompts end with: show the `diff`, run `validate`, offer `apply_guide` and, if Keymapp is connected,
`keyboard(highlight)` on the changed keys.

## Example flow: "Add a WoW layer"

1. `layout_summary` → base + 4 layers, thumbs used for Sym/Nav.
2. `keyloom://guides/mmo` and `keyloom://guides/gaming`.
3. Ask the user: mouse hand, how many action bars, modifiers for bars, keep chat (Enter) and Esc?
4. `free_keys(base)` → where the toggle key can go; `layout_summary(graph=true)` → no `TG` used yet.
5. `create_draft("wow")`, `edit_draft([...add_layer "WoW" tags [gaming] fill none, set keys..., set base key TG(WoW), set WoW key TG(WoW) for exit])`.
6. `validate` → clean; `render_layer(draft:wow, WoW)` shown to the user; `keyboard(highlight)` optional.
7. `apply_guide(draft:wow)` → steps for Oryx; after flashing, `verify_draft`.

## Testing

- Scenario tests (`keyloom-cli/tests/e2e.rs`) drive the real binary over stdio with scripted tool calls
  against fixtures (no LLM), including schema checks for the things agents trip over (numeric layers).
- Not done: snapshots of every tool's schema.
- Manual acceptance per PLAN with Claude Code.

## Arguments

Layers and keys are accepted as numbers or text everywhere (`3` or `"Gaming"`, `17` or `"L.home.4"`).
`fill` and `style` are enums; unknown values are rejected. Edit results carry `diff`, `warnings`, `findings`
(errors and warnings of the whole draft) and `newFindings` (caused by this edit).
