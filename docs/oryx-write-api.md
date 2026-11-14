# Writing to Oryx: analysis and proof of concept

Notes from 2026-11-14. Question: could keyloom put a draft into Oryx itself, instead of handing the user a
list of changes to click through?

Short answer: yes, and the no-login route is built: `keyloom push <draft>` (`crates/keyloom/src/push.rs`). Oryx's web app saves edits through GraphQL mutations on the same endpoint keyloom reads
from (`https://oryx.zsa.io/graphql`). A proof of concept wrote a six-layer keyloom draft into a new Oryx
layout, read it back with no key differences, and compiled it. Nothing here is documented or supported by
ZSA.

## How it was found

Introspection is off on the endpoint. The operations below were read out of Oryx's public web app bundle
(`configure.zsa.io/assets/index-*.js`), which contains every query and mutation as plain text, and the
call sites show how the app uses them.

## The model

- A **layout** has **revisions**. A revision has **layers**; a layer's `keys` is a JSON array (52 entries
  on a Voyager) in the same shape the read query returns.
- A revision is **editable until it is compiled** (`qmkVersion == null`). After `compileRevision` it is
  frozen: an `updateKey` on it answers `Unauthorized`.
- To change a compiled layout you start a new revision: `forkRevision` (owner, needs a login) or
  `createLayout` from the compiled revision.
- Layers, revisions and layouts are addressed by hash ids. Layer hash ids change with every new revision.

## Mutations that matter

| Mutation | Arguments | Use |
| --- | --- | --- |
| `createLayout` | `title, revisionHashId, geometry, parentHashId?, mcuAlternate?` | New editable copy of a revision. Returns the new layout and revision ids |
| `forkRevision` | `hashId` | New editable revision in the same layout (needs a login) |
| `updateLayer` | `hashId, newKeys?, title?, position?` | Replace all keys of a layer, or rename it |
| `updateKey` | `hashId (layer), keyData, position` | Replace one key |
| `createLayer` | `newKeys, revisionHashId, position, title?, automouse?` | Add a layer |
| `deleteLayer`, `restoreLayer`, `reorderLayers` | layer `hashId` (`direction` for reorder) | Layer housekeeping |
| `updateLayerColor` | `hashId, color` | Layer colour |
| `upsertCombo`, `deleteCombo`, `reorderCombos` | `revisionHashId, comboIdx, name, layerIdx, keyIndices, trigger, …` | Combos |
| `updateNavigators` | `revisionHash, navigators` | Trackball settings |
| `updateRevisionAutomouse` | `revisionHash, layerPosition, automouse` | Auto-mouse layer |
| `updateRevisionConfig`, `updateRevisionSwatch`, `updateRevisionTitle` | `hashId, …` | Revision settings |
| `compileRevision` | `hashId` | Build the firmware. Returns `md5`, `buildErrorLog` |
| `deleteLayout`, `deleteRevision`, `updateLayoutPrivacy`, `updateLayoutTitle` | | Account housekeeping (not tried) |

Login: `loginWithEmail(email, password)` returns a JWT that the app sends as `Authorization: Bearer …`.
Accounts with two-factor login need `otpChallenge` as well.

## What was tried

By hand, with no login:

1. `createLayout` from ZSA's default Voyager layout: a new anonymous layout with an editable revision.
2. `updateLayer` for the existing layers (keys, then title) and `createLayer` for more, from a keyloom draft.
3. Read back and compared with `keyloom diff`: no key or layer differences (6 layers × 52 keys, including
   tap-hold keys, layer keys, modifiers and RGB keys).
4. `compileRevision`: built in about 9 seconds with no errors. The firmware was not downloaded or flashed.
5. `updateNavigators`, `updateRevisionAutomouse` (Oryx keeps the flag to one layer by itself),
   `updateLayerColor` and `deleteLayer` (Oryx renumbers the layers after it): all work.

Also seen: after compiling, `updateKey` is refused; `forkRevision` without a login is refused; `createLayout`
from someone's compiled public revision, sent without a login, makes a separate anonymous layout whose
`parent` is the original and leaves the original untouched. Oryx answered one call with HTTP 503 once;
repeating it worked.

## What keyloom does

`keyloom push <draft>` (no login):

1. Asks Oryx for a copy of the draft's base revision (`createLayout`). The copy carries trackball settings,
   config, swatch and combos. If Oryx doesn't have that revision (private, or imported from a file), it
   copies ZSA's default layout instead and says so.
2. Refuses to go on if Oryx answers with an existing layout id instead of a new one.
3. Writes only the layers that differ, adds and deletes layers, sets the auto-mouse layer.
4. Reads the copy back and diffs it against the draft; the result says whether it matches.
5. Doesn't compile. The user opens the link, compiles and flashes in Oryx.

Not built: writing into the user's own layout with a login. From the web app's code, `createLayout` with
`parentHashId` set to one's own layout, sent with a login, adds a revision to that layout rather than making
a copy. That hasn't been tried.

## Open questions and risks

- **Not tried:** combos (`upsertCombo`), `updateRevisionConfig`, swatch, reordering layers, macros, anything
  with a login.
- **Access control on uncompiled revisions:** the anonymous test layout could be edited with only its hash
  ids. Whether an uncompiled revision in a user's account is protected from such calls was deliberately
  not tested.
- **Undocumented:** ZSA can change or close these calls at any time, and hasn't said automated writes are
  welcome. Rate limits are unknown. Their terms of service haven't been checked. Worth asking ZSA before
  shipping this.
- **Credentials:** the login route means handling a password or a token. A token pasted from the browser
  avoids the password and two-factor flow.
- **Leftovers:** anonymous layouts can't be deleted without owning them, so each push leaves a public
  layout on Oryx.
