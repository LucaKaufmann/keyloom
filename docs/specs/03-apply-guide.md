# 03: Apply guide and verify

Since keyloom doesn't write to Oryx or flash, a draft becomes real by you making the changes in Oryx.
keyloom makes that fast and checks the result afterwards.

## `guide(draft)`

Input: a draft. Output: an ordered list of steps, grouped, worded in **Oryx's UI terms**, derived from
`diff(base revision, draft)` (02).

Order (so everything a step refers to already exists):
1. **Layers**: renames first (later steps use the new titles; a rename names the layer by its current
   Oryx number, which is still the base's at that point); then adding and dragging layers, top to
   bottom in their final order, each placed "right after" the layer above it (Oryx adds new layers at the
   end); then other layer settings (color).
2. **Per layer**, in key order (left half then right, top to bottom): key changes, slot by slot. A layer
   filled with `KC_NO` gets one step for all those keys, not one per key.
3. **Combos**, named by layer number and title. A combo on a layer that is deleted has no step.
4. **Remove layers** last.
5. "Compile and flash in Oryx/Keymapp", then `keyloom import <id>` and `keyloom verify <draft>`.

If the draft has validation errors, the guide starts with "NOT READY TO FLASH" and lists them.

If a newer revision than the draft's base is imported (the draft was applied, or partly applied, and
flashed), the guide is computed against that revision, because that is where Oryx is now: it lists only
what is still to do. If that revision also has changes the draft didn't ask for, a note says how many of
the steps would undo them and suggests a new draft from that revision instead.

When layers are added, moved or removed, the guide starts with a note: layer numbers are the final ones;
while working in Oryx go by the titles.

Each step: `{n, layer, index, position, location, what, oryxHint, keys}`, e.g.

```
Apply draft:wow (based on 9qMJK@ZPjbXz): 1 layer(s) added, 9 key(s) changed
Note: Layer numbers are the final ones. While you work, Oryx's numbers can differ (deleted layers go last): go by the layer titles.

Layers
  L.1   Add a layer "WoW", then drag it right after "Gaming" (Oryx adds new layers at the end)
        Oryx: "+" next to the layer list

Layer 0 "Main"
  0.1   L.thumb.1  (left thumb key next to the main block)  Tap: KC_LEFT_GUI → TG(4 WoW)
        Oryx: Layer → Toggle (TG)

Layer 4 "WoW"
  4.1   L.home.2   (left home row, 3rd from the outer edge)  Tap: empty (transparent) → KC_S
  4.2   Set the other 50 keys to None (KC_NO), so nothing falls through from the base layer: L.num.0, …
```

`oryxHint` maps keyloom's concepts to Oryx's wording (which menu, which action type). The wording is a best
guess until checked against Oryx's UI (PLAN, Status).

## `guide --step` (needs Keymapp, see 06)

Walks the steps one at a time and lights the step's key(s). Only the lights are used: the keyboard's layer
is left alone, because the draft's layer numbers aren't the flashed firmware's. Enter = next, `b` = back,
`q` = quit. The lights are restored at the end, on `q` and on Ctrl-C.

## `verify(draft, against = newest revision)`

After you've applied the guide and flashed, import the new revision (`keyloom import <id>`, MCP
`import_layout`). `verify` compares the draft with it, ignoring keyloom-only metadata (tags):

- `matches`: done; archive the draft (`keyloom draft archive`).
- `stillToDo`: changes the draft asked for that aren't in the revision (yet, or not correctly).
- `unexpected`: differences the draft didn't ask for (changed in Oryx by hand, or a mistake). The title and
  color of a layer the draft added, and the layer order when the draft adds or moves a layer, always count
  as still to do.
- `note`: set when nothing new was imported (the newest revision is still the draft's base).

Exit code 3 when it doesn't match.
