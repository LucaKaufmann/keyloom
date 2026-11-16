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

`keyloom push <draft> --to <layout>` (with the user's token, sent as `Authorization: Bearer`):

1. Reads the layout's latest revision as its owner.
2. If that revision is compiled: refuses unless it is the draft's base (or `--overwrite`), then calls
   `createLayout` with `parentHashId` set to the layout itself. Oryx answers with the same layout id and a new
   uncompiled revision. If the id differs (not the user's layout, or a bad token), keyloom stops.
3. If that revision is uncompiled: refuses unless its layers equal the draft's base or the draft itself (or
   `--overwrite`), then writes into it.
4. Writes, reads back and diffs as above. Doesn't compile.

`keyloom push <draft> --to new` (with the token): a new layout in the user's account.

1. Picks the source as the no-login push does, and makes an anonymous copy of it, sent without the token.
2. With the token, calls `createLayout` on that anonymous copy. Its source isn't the user's, so Oryx makes a
   new layout owned by the user. keyloom checks the new layout has an owner before writing.
3. Writes, reads back and diffs as above. Doesn't compile. The anonymous copy stays on Oryx.

## What was tried with a login

On a throwaway layout in the owner's account, with the `jwtToken` from the web app's local storage:

- `updateLayer`, `createLayer` and `deleteLayer` on the uncompiled revision worked; the same calls without the
  token answered `Unauthorized`.
- `compileRevision` worked (about 10 s). After it, `updateLayer` answered `Unauthorized` even for the owner.
- `createLayout` with `parentHashId` = the layout itself returned the same layout id and a new uncompiled
  revision. Called again while that revision existed, it returned the same revision.
- `createLayout` from ZSA's default layout, with the token, made a new layout owned by the user.
- `createLayout` from a revision of the user's own layout always resolves to that layout, whatever
  `parentHashId` says: with another layout as parent it still returned the own layout and its uncompiled
  revision, and with no parent it answered HTTP 500. So a layout can't be copied to a second own layout
  directly.
- `createLayout` from an uncompiled anonymous layout, with the token, made a new layout owned by the user,
  with the same navigators, config and combos.
- Without the token, `latest` keeps returning the newest compiled revision; the uncompiled one is only visible
  to the owner.
- The token is a JWT with `uid` and `exp`; the one seen was valid for about three months.

## Oryx's editor rewrites keys

Read from the editor's code (configure.zsa.io, November 2026) and seen on a real layout. A key on layer n
is "preceded" when a lower layer has a layer key to n at the same position. That covers keycodes whose
layer parameter is `forwardLayers`, except `OSL`: at least `TG`, `TT`, `MO` and `LM`. `MO`, `LM` and `TT`
don't count when the revision's `config.ambidexLT` is on. Whether `TO` and `DF` count wasn't checked.
The editor shows preceded keys as disabled ("needed to return you to your original layer"), and when it
mounts one that isn't transparent it calls `clear()`, which saves straight away. So opening a layer in
Oryx, which happens before every compile, wipes such keys. The API itself stores them, so a read-back
right after a push matches.

This bit a pushed gaming layer: `TG(Gaming)` on a held symbol layer, and `TG(Gaming)` at the same
position on Gaming to leave again. After the layout was opened in Oryx, the second key was gone. The
transparent key then fell through to the base layer, so the layer had no way out. keyloom treats these
keys as transparent when it works out exits, and reports them as `oryx-clears-key`.

## Open questions and risks

- **Not tried:** combos (`upsertCombo`), `updateRevisionConfig`, swatch, reordering layers, macros,
  `loginWithEmail`, `forkRevision`, deleting a revision.
- **Access control on uncompiled revisions:** the anonymous test layout could be edited with only its hash
  ids. An uncompiled revision in a user's account is protected: the same call without the token was refused.
- **Undocumented:** ZSA can change or close these calls at any time, and hasn't said automated writes are
  welcome. Rate limits are unknown. Their terms of service haven't been checked. Worth asking ZSA before
  shipping this.
- **Credentials:** keyloom takes a token copied from the browser (environment variable or keychain), which
  avoids handling the password and two-factor flow. The token is long-lived and gives full account access.
- **Leftovers:** anonymous layouts can't be deleted without owning them, so each push leaves a public
  layout on Oryx.
