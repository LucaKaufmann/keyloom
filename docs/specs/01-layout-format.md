# 01: Workspace, layout format, positions, references

## Workspace on disk

Root: `$KEYLOOM_HOME`, default `$XDG_DATA_HOME/keyloom`, i.e. `~/.local/share/keyloom` (also on macOS: no spaces in the
path, easy to find, easy to back up or put in dotfiles). Plain files, no database.

**History is git.** On first use keyloom runs `git init` in the workspace (if `git` is installed) and commits
after every import, draft operation and archive, with messages like `import 9qMJK@ZPjbXz` or
`draft wow: add_layer "WoW"`. That gives layout history, `git log`, and recovery for free without keyloom
building its own versioning. Toggle with `[workspace] git = false`. keyloom never pushes.

```
$KEYLOOM_HOME/
  config.toml                     active layout, Keymapp settings, preferences
  layouts/
    <layoutId>/                   the layout's Oryx hashId (files need one too), [A-Za-z0-9_-]{1,32}
      layout.toml                 title, geometry, revision list
      revisions/<revisionId>.json immutable snapshots
      drafts/<name>.json          mutable working copies
      drafts/<name>.base.json     what the draft started from (for undo)
      drafts/<name>.ops.jsonl     the edits so far, one batch per line
      drafts/archive/             finished drafts
  cache/oryx/                     raw GraphQL answers (not committed)
  cache/live.json                 what keyloom changed on the keyboard, until reset
  .lock                           held while a keyloom process writes
```

Rules:
- Revisions are written once (write to temp file + rename). Any operation that would change one fails.
- Importing `latest` resolves to the concrete revision id; importing a known revision is a no-op.
- All JSON is pretty-printed with stable key order so `git diff` is readable.
- Ids are validated (same rule as `oryx::is_valid_id`, plus `-`/`_` for local ids), so paths are safe.

## Layout file (`revisions/*.json`, `drafts/*.json`)

The body uses Oryx's own shape, so other Oryx tools can read it. We wrap it in an
envelope with our metadata.

```jsonc
{
  "schema": "keyloom.layout/1",
  "layoutId": "9qMJK",
  "revisionId": "ZPjbXz",             // drafts: the revision they are based on
  "kind": "revision",                 // "revision" | "draft"
  "draft": { "name": "wow", "created": "…", "updated": "…" },   // drafts only
  "source": { "type": "oryx", "fetchedAt": "2026-11-13T10:00:00Z" },  // "oryx" | "keymapp" | "file"
  "title": "Voyager",
  "geometry": "voyager",
  "revisionTitle": "Keyboard layout edited.",
  "createdAt": "2026-11-12 09:56:22 UTC",       // when Oryx created the revision
  "layers": [
    {
      "title": "Main", "position": 0, "color": "#…",
      "keyloom": { "tags": ["base"] },          // keyloom's metadata, never sent to Oryx
      "keys": [ /* one Oryx key object per key, in index order */ ]
    }
  ],
  "combos": [ { "keyIndices": [1, 2], "layerIdx": 0, "name": "esc", "trigger": { "code": "KC_ESCAPE" } } ],
  "oryx": { "layout": { }, "revision": { "qmkVersion": "25.0", "zipUrl": "…" } },  // Oryx fields keyloom doesn't model
  "settings": { "autoMouseLayer": 4 }           // read from Oryx's generated config.h
}
```

Key objects keep Oryx's fields: `tap`, `hold`, `doubleTap`, `tapHold` (each `{code, layer, modifiers,
color, description}`), `customLabel`, `icon`, `emoji`, `glowColor`.

**Lossless round trip.** A lenient Oryx model (like `keyloom::oryx`) drops unknown fields. `keyloom` uses its own serde model that keeps
unknown fields (flattened `extra` maps on layer, key and action; `oryx.layout` / `oryx.revision` at the top), so
they survive import → draft → export. Null values are dropped. `keyloom` converts to the lenient `oryx` types only for
rendering legends. Layers must be numbered 0..n and have the geometry's key count to be edited.

**Layer tags** (`keyloom.tags`): free-form, with known values the engine understands:
`base`, `gaming`, `nav`, `symbols`, `numbers`, `mouse`, `function`, `media`; `auto` marks a layer the firmware
enters by itself. They drive validation rules
(e.g. no tap-hold on `gaming`) and help agents. Tags are keyloom-only and never appear in the Oryx guide.

## Positions

Oryx key indices (0–51 on the Voyager, 0–71 on the Moonlander) are opaque, so every interface also accepts and prints position names.

Voyager: per half 4 rows × 6 columns + 2 thumb keys = 26; 52 total.

```
<half>.<row>.<col>     half: L | R      row: num | top | home | bot      col: 0..5, counted from the OUTER edge
<half>.thumb.<n>       n: 1 = next to the main block, 2 = toward the centre (between the halves)
```

So `L.home.4` is F on QWERTY (left index finger), `R.home.4` is J, `L.home.0` is the outer pinky column.
Columns count from the outer edge on both halves, so the names are mirror-symmetric and match fingers:
0 = pinky outer, 1 = pinky, 2 = ring, 3 = middle, 4 = index, 5 = index inner.

**Moonlander** (not implemented): per half 5 rows (`num top home bot low`) × 7 columns (0–6 from the outer edge;
the 7th is the inner column) and a thumb cluster `thumb.1`–`thumb.4` (1 = the large red key). Rows and
columns that don't exist physically (the shorter bottom row) simply have no names.

**Geometry-generic.** Nothing in the format, engine or renderer assumes 52 keys; the key count, position
names, fingers and effort come from the geometry's data file (07). Supporting another ZSA board means adding
that data, not changing code.

The mapping index ↔ position lives in `keyloom::positions` and was verified by inspection. Thumb numbering comes from the Voyager's geometry (docs/oryx-encodings.md) and was checked on a real board (lit keys).

## Key selectors (input)

Everywhere a key is expected, accept:

| Form | Example | Meaning |
| --- | --- | --- |
| Position | `L.home.4` | physical key |
| Index | `#17` | Oryx index |
| Legend | `key:F`, `key:Esc` | the key that sends it (on tap, else in any slot) on the layer the edit or query names: that layer's own keys first, then base keys showing through transparent ones; the base layer when none is named. Error if ambiguous |

## Layer selectors

`0`, `3` (position), `"Sym+Num"` / `sym` (title, case-insensitive, unique prefix), `base` (position 0).

## Layout references

```
<ref>      := [<layoutId>][@<revision>] | draft:<name> | <file path ending in .json>
<revision> := <revisionId> | latest | oryx
```

- No `layoutId` → the active layout (`keyloom use <layoutId>`; the first import sets it), else the only one.
- No `@revision` → the newest stored revision.
- `@latest` → fetch from Oryx first, then use it.
- `@oryx` → the newest revision on Oryx, read with the user's login token, so an uncompiled one in their account
  counts. A compiled one is imported as usual; an uncompiled one is used as it is and never stored (it can still
  change), so it can't be a draft's base.
- Only compiled revisions are imported. Asked for an uncompiled revision id, Oryx answers anonymous requests with
  the newest compiled revision instead; keyloom notices, retries with the token if one is set, and otherwise says
  so. Private layouts are read with the token too.
- Draft names are unique across the workspace and can't be a layout id; a bare name that isn't a layout is taken as a draft (`wow` = `draft:wow`).

## Import sources

1. **Oryx GraphQL**: the layout query against `https://oryx.zsa.io/graphql`, with
   `LayoutRef::from_user_input` (accepts configure.zsa.io links). HTTP: `ureq` for all targets in
   `keyloom` (macOS included). Save the raw answer to `cache/oryx/`.
2. **Keymapp SQLite** (read-only): `~/Library/Application Support/.keymapp/keymapp.sqlite3`,
   `SELECT data FROM revision WHERE revisionId = ?`. Linux/Windows
   paths are unverified.
3. **File**: a `keyloom.layout/1` file, or a raw Oryx layout JSON (the API's `layout` object), detected by `schema`.

## Export

- `--format keyloom` (default): the layout file as is.
- `--format oryx`: the bare Oryx `layout` object (as Oryx's API returns it), for other
  tools. A draft's export is marked (`revision.keyloomDraft`, revision id `<base>-draft-<name>`) and
  can't be imported as a revision.
- Later: `--format qmk` (keymap.c), not in v1.
