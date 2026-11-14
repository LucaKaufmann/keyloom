//! Operations shared by the CLI and the MCP server: resolving references, importing, editing
//! drafts with validation, guides and verification.

use anyhow::{bail, Context as _, Result};
use keyloom::diff::{diff, Diff, DiffOptions};
use keyloom::guide::{self, Guide, Verification};
use keyloom::model::{Kind, Layout};
use keyloom::ops::{apply_ops, Op};
use keyloom::source::{self, ImportInput, LayoutRef};
use keyloom::validate::{validate, Context, Finding, Severity};
use keyloom::workspace::{Stored, Workspace};
use serde::Serialize;

pub fn open() -> Result<Workspace> {
    Workspace::open_default().context("opening the keyloom workspace")
}

/// `[layoutId][@revisionId]`, `draft:<name>` (or a bare draft name), or a path to a layout
/// `.json` file. `@latest` imports from Oryx first.
pub fn resolve(ws: &Workspace, reference: Option<&str>) -> Result<Layout> {
    if let Some(r) = reference.map(str::trim).filter(|r| !r.is_empty()) {
        if let Some(layout_id) = r.strip_suffix("@latest") {
            let layout_id = if layout_id.is_empty() { ws.default_layout()? } else { layout_id.to_string() };
            let (layout, _) = import_oryx(ws, &LayoutRef::new(&layout_id, "latest", None), true)?;
            return Ok(layout);
        }
        if r.ends_with(".json") {
            return Ok(source::read_file(std::path::Path::new(r))?);
        }
    }
    Ok(ws.resolve(reference)?)
}

/// Imports from Oryx; when Oryx can't be reached, optionally falls back to Keymapp's copy.
pub fn import_oryx(ws: &Workspace, r: &LayoutRef, keymapp_fallback: bool) -> Result<(Layout, Stored)> {
    match source::fetch_oryx(r) {
        Ok((raw, layout)) => {
            let stored = ws.store_revision(&layout, Some(&raw))?;
            // What is stored is the truth (it may have settings the fetch didn't get this time).
            let layout = ws.load_revision(&layout.layout_id, &layout.revision_id)?;
            Ok((layout, stored))
        }
        Err(e @ source::SourceError::Network(_)) if keymapp_fallback => {
            let db =
                source::keymapp_db_path().filter(|p| p.is_file()).with_context(|| format!("{e}, and Keymapp's database wasn't found"))?;
            let mut candidates: Vec<Layout> = source::read_keymapp(&db)?
                .into_iter()
                .filter(|l| l.layout_id == r.hash_id && (r.is_latest() || l.revision_id == r.revision_id))
                .collect();
            candidates.sort_by(|a, b| a.created_at.cmp(&b.created_at));
            let layout = candidates.pop().with_context(|| format!("{e}, and Keymapp has no copy of {r}"))?;
            eprintln!("Oryx unreachable ({e}); using Keymapp's copy {}", layout.reference());
            let stored = ws.store_revision(&layout, None)?;
            let layout = ws.load_revision(&layout.layout_id, &layout.revision_id)?;
            Ok((layout, stored))
        }
        Err(e) => Err(e.into()),
    }
}

pub fn import(ws: &Workspace, input: &str, keymapp_fallback: bool) -> Result<Vec<(Layout, Stored)>> {
    let input = ImportInput::parse(input).map_err(anyhow::Error::msg)?;
    Ok(match input {
        ImportInput::Oryx(r) => vec![import_oryx(ws, &r, keymapp_fallback)?],
        ImportInput::File(path) => {
            let layout = source::read_file(&path)?;
            if layout.kind == Kind::Draft {
                bail!(
                    "{} is a keyloom draft, not a revision, so it can't be imported. You can look at it (`keyloom show {0}`); to carry a draft to another machine, copy the whole workspace folder",
                    path.display()
                );
            }
            let stored = ws.store_revision(&layout, None)?;
            // Report what is stored (an already imported revision keeps its own content).
            let layout = ws.load_revision(&layout.layout_id, &layout.revision_id)?;
            vec![(layout, stored)]
        }
        ImportInput::Keymapp => {
            let db = source::keymapp_db_path().filter(|p| p.is_file()).context("Keymapp's database wasn't found")?;
            // Oldest first, so the result doesn't depend on the database's row order. With no
            // active layout yet, the most recently created revision's layout becomes active.
            let mut layouts = source::read_keymapp(&db)?;
            layouts.sort_by(|a, b| a.created_at.cmp(&b.created_at));
            let had_active = ws.config()?.active_layout.is_some();
            let mut out = Vec::new();
            for layout in layouts {
                let stored = ws.store_revision(&layout, None)?;
                let layout = ws.load_revision(&layout.layout_id, &layout.revision_id)?;
                out.push((layout, stored));
            }
            if let (false, Some((newest, _))) = (had_active, out.last()) {
                ws.set_active(&newest.layout_id)?;
            }
            out
        }
    })
}

/// JSON summary of one imported revision (CLI `--json` and the MCP import tool).
pub fn import_summary(l: &Layout, stored: &Stored) -> serde_json::Value {
    serde_json::json!({
        "reference": l.reference(),
        "title": l.title,
        "createdAt": l.created_at,
        "layers": l.layers.iter().map(|x| x.title.as_str()).collect::<Vec<_>>(),
        "new": *stored == Stored::New,
        "source": l.source.kind,
        "autoMouseLayer": l.settings.auto_mouse_layer,
    })
}

/// Layouts, revisions and drafts in the workspace.
pub fn list_summary(ws: &Workspace) -> Result<serde_json::Value> {
    let layouts = ws.layouts()?;
    Ok(serde_json::json!({
        "workspace": ws.root(),
        "active": ws.config()?.active_layout,
        "layouts": layouts.iter().map(|m| serde_json::json!({
            "layoutId": m.layout_id,
            "title": m.title,
            "geometry": m.geometry,
            "newest": m.newest().map(|r| &r.id),
            "revisions": m.revisions.iter().map(|r| serde_json::json!({"id": r.id, "createdAt": r.created_at, "importedAt": r.imported_at, "source": r.source})).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "drafts": ws.drafts()?,
    }))
}

pub fn validation_context(ws: &Workspace, layout: &Layout) -> Context {
    let newest = ws.meta(&layout.layout_id).ok().and_then(|m| m.newest().map(|r| r.id.clone()));
    let disabled = ws.config().map(|c| c.validate.disabled_rules).unwrap_or_default();
    Context { newest_revision: newest, disabled }
}

pub fn validate_ref(ws: &Workspace, layout: &Layout) -> Vec<Finding> {
    validate(layout, &validation_context(ws, layout))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditResult {
    pub draft: String,
    /// What this batch changed (the first changes; `summary` counts all of them).
    pub diff: Diff,
    /// Changes not listed in `diff` (the `diff` tool shows everything).
    #[serde(skip_serializing_if = "is_zero")]
    pub more_changes: usize,
    pub warnings: Vec<String>,
    /// Errors and warnings for the whole draft after the change (info-level notes are left out;
    /// `validate` lists them).
    pub findings: Vec<Finding>,
    /// The findings this batch introduced.
    pub new_findings: Vec<Finding>,
}

/// How many changes an edit result lists in full.
const MAX_CHANGES: usize = 30;

fn result(ws: &Workspace, before: &Layout, after: &Layout, warnings: Vec<String>) -> EditResult {
    // A finding is the same one across the edit if it is about the same layer (layers are
    // matched as the diff does, so renames and moves don't make everything look new).
    let (pairs, _, _) = keyloom::diff::match_layers(before, after);
    let before_layer = |after_pos: i64| pairs.iter().find(|&&(_, pb)| pb == after_pos).map(|&(pa, _)| pa);
    let old: Vec<_> = validate_ref(ws, before).iter().map(|f| (f.rule, f.layer, f.key.clone())).collect();
    let findings: Vec<Finding> = validate_ref(ws, after).into_iter().filter(|f| f.severity != Severity::Info).collect();
    let is_old = |f: &Finding| match f.layer {
        Some(l) => before_layer(l).is_some_and(|pa| old.contains(&(f.rule, Some(pa), f.key.clone()))),
        None => old.contains(&(f.rule, None, f.key.clone())),
    };
    let new_findings = findings.iter().filter(|f| !is_old(f)).cloned().collect();
    let mut diff = diff(before, after, DiffOptions::default());
    // If the list has to be cut, cut the filler: keys merely set to KC_NO go last.
    let filler =
        |c: &keyloom::diff::Change| matches!(c, keyloom::diff::Change::KeyChanged { from: None, to: Some(to), .. } if to == "KC_NO");
    diff.changes.sort_by_key(filler);
    let more_changes = diff.changes.len().saturating_sub(MAX_CHANGES);
    diff.changes.truncate(MAX_CHANGES);
    EditResult { draft: after.reference(), diff, more_changes, warnings, findings, new_findings }
}

fn is_zero(n: &usize) -> bool {
    *n == 0
}

/// Counts and findings, as `keyloom validate --json` and the MCP `validate` tool return them.
pub fn validation_report(ws: &Workspace, layout: &Layout) -> (serde_json::Value, Vec<Finding>) {
    let findings = validate_ref(ws, layout);
    let count = |s: Severity| findings.iter().filter(|f| f.severity == s).count();
    let report = serde_json::json!({
        "reference": layout.reference(),
        "errors": count(Severity::Error),
        "warnings": count(Severity::Warn),
        "info": count(Severity::Info),
        "findings": findings,
    });
    (report, findings)
}

/// Applies ops to a draft atomically, saves it, and reports the change and validation.
/// The workspace stays locked from reading the draft to saving it.
pub fn edit(ws: &Workspace, draft_name: &str, ops: &[Op]) -> Result<EditResult> {
    let _guard = ws.lock()?;
    let mut draft = ws.load_draft(draft_name)?;
    let before = draft.clone();
    let applied = apply_ops(&mut draft, ops)?;
    ws.save_draft(&mut draft, ops)?;
    Ok(result(ws, &before, &draft, applied.warnings))
}

/// Undoes the last batch of edits; the diff shows what was reverted.
pub fn undo(ws: &Workspace, draft_name: &str) -> Result<EditResult> {
    let _guard = ws.lock()?;
    let before = ws.load_draft(draft_name)?;
    let after = ws.undo_draft(draft_name)?;
    Ok(result(ws, &before, &after, vec![]))
}

/// The revision that is on the keyboard, as far as keyloom can tell: the flashed one (from
/// the keyboard's USB serial) if it belongs to this layout and is imported, otherwise the
/// newest imported revision (the same assumption the guide makes).
pub fn keyboard_revision(ws: &Workspace, layout_id: &str, flashed: Option<&str>) -> Result<Layout> {
    if let Some((l, r)) = flashed.and_then(|f| f.split_once('/')) {
        if l == layout_id {
            if let Ok(rev) = ws.load_revision(l, r) {
                return Ok(rev);
            }
        }
    }
    Ok(ws.resolve(Some(layout_id))?)
}

/// The layer of the flashed firmware that corresponds to `layer` of `layout`.
pub fn keyboard_layer(ws: &Workspace, layout: &Layout, layer: i64) -> Result<i64> {
    let flashed = crate::live::usb_serials().into_iter().find(|s| LayoutRef::from_serial(s, None).is_some());
    keyboard_layer_on(ws, layout, layer, flashed.as_deref())
}

/// A draft may have added, moved or removed layers, and an older revision may be numbered
/// differently from the flashed one, so the layer is looked up in the revision on the keyboard.
/// An error if that revision doesn't have the layer.
pub fn keyboard_layer_on(ws: &Workspace, layout: &Layout, layer: i64, flashed: Option<&str>) -> Result<i64> {
    let on_keyboard = keyboard_revision(ws, &layout.layout_id, flashed)?;
    if layout.draft.is_none() && layout.revision_id == on_keyboard.revision_id {
        return Ok(layer);
    }
    let (pairs, _, _) = keyloom::diff::match_layers(&on_keyboard, layout);
    pairs.iter().find(|&&(_, here)| here == layer).map(|&(there, _)| there).with_context(|| {
        format!(
            "layer {layer} \"{}\" of {} isn't in {} (the revision on the keyboard), so the keyboard can't show it yet. After flashing, import the new revision",
            layout.layer_title(layer),
            layout.reference(),
            on_keyboard.reference()
        )
    })
}

/// Validation errors that should be fixed before a draft's guide is followed.
pub fn guide_blockers(ws: &Workspace, draft: &Layout) -> Vec<Finding> {
    validate_ref(ws, draft).into_iter().filter(|f| f.severity == Severity::Error).collect()
}

/// A warning block for the top of a guide, or nothing when the draft has no errors.
pub fn blockers_text(draft: &Layout, blockers: &[Finding]) -> String {
    if blockers.is_empty() {
        return String::new();
    }
    let mut out = format!("NOT READY TO FLASH: this draft has {} validation error(s). Fix them first:\n", blockers.len());
    for f in blockers {
        let at = f.layer.map(|l| format!("{}: ", draft.layer_title(l))).unwrap_or_default();
        out.push_str(&format!("  [{}] {at}{} → {}\n", f.rule, f.message, f.hint));
    }
    out.push('\n');
    out
}

pub fn base_of(ws: &Workspace, draft: &Layout) -> Result<Layout> {
    Ok(ws.load_revision(&draft.layout_id, &draft.revision_id)?)
}

/// The steps to make in Oryx for a draft. If a newer revision than the draft's base has been
/// imported (the draft was applied and flashed, then edited further), only what that revision
/// doesn't have yet is listed.
pub fn guide_for(ws: &Workspace, draft_name: &str) -> Result<(Guide, Layout)> {
    let draft = ws.load_draft(draft_name)?;
    let base = base_of(ws, &draft)?;
    let newest = ws.resolve(Some(&draft.layout_id))?;
    if newest.revision_id == draft.revision_id {
        return Ok((guide::guide(&base, &draft), draft));
    }
    // Oryx is at the newest revision now, so the steps always start from there (steps from the
    // draft's old base would repeat what is already done).
    let seen = guide::verify(&base, &draft, &newest);
    let mut g = guide::guide(&newest, &draft);
    g.notes.insert(
        0,
        format!(
            "{} already has part of this draft (it was applied after {}): these are only the steps still to do.",
            newest.reference(),
            base.reference()
        ),
    );
    if !seen.unexpected.is_empty() {
        // Some of the steps undo things that were changed in Oryx but aren't in the draft.
        g.notes.insert(
            1,
            format!(
                "{} of these steps undo changes that are in {} but that this draft didn't ask for (see `keyloom verify` / verify_draft). \
                 If those changes were made on purpose in Oryx, don't follow these steps: start a new draft from {} instead.",
                seen.unexpected.len(),
                newest.reference(),
                newest.reference()
            ),
        );
    }
    if g.sections.is_empty() {
        g.notes.clear();
    }
    Ok((g, draft))
}

/// Where a push goes.
#[derive(Debug, Clone, Default)]
pub struct PushTarget {
    /// One of the user's own Oryx layouts (id or link), or "new" for a new layout in their account.
    /// None: a new anonymous layout.
    pub to: Option<String>,
    /// With `to`: write even if that loses changes on Oryx the draft doesn't have.
    pub overwrite: bool,
}

/// The environment variable holding the user's Oryx login token (set but empty: no token, don't look further).
pub const ORYX_TOKEN_ENV: &str = "KEYLOOM_ORYX_TOKEN";
/// The macOS keychain item holding it.
pub const ORYX_KEYCHAIN_SERVICE: &str = "keyloom-oryx";

/// The user's Oryx login token: `KEYLOOM_ORYX_TOKEN`, or on macOS the keychain item `keyloom-oryx`.
/// keyloom never stores it itself.
pub fn oryx_token() -> Result<String> {
    let clean = |t: &str| t.trim().trim_matches(|c| c == '"' || c == '\'').to_string();
    let found = match std::env::var(ORYX_TOKEN_ENV) {
        Ok(t) => clean(&t),
        Err(_) if cfg!(target_os = "macos") => std::process::Command::new("/usr/bin/security")
            .args(["find-generic-password", "-s", ORYX_KEYCHAIN_SERVICE, "-w"])
            .stderr(std::process::Stdio::null())
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| clean(&String::from_utf8_lossy(&o.stdout)))
            .unwrap_or_default(),
        Err(_) => String::new(),
    };
    if found.is_empty() {
        anyhow::bail!(
            "Pushing into your own Oryx account needs your login token, and none is set. Nothing was sent to Oryx.\n\
             Get it: sign in at https://configure.zsa.io, open the browser console and run localStorage.getItem(\"jwtToken\").\n\
             Store it: export {ORYX_TOKEN_ENV}=…{}\n\
             Without a token, `keyloom push <draft>` (no --to) makes a new layout instead.",
            if cfg!(target_os = "macos") {
                format!(", or in the keychain: security add-generic-password -U -a oryx -s {ORYX_KEYCHAIN_SERVICE} -w")
            } else {
                String::new()
            }
        );
    }
    Ok(found)
}

/// The layout id in `https://configure.zsa.io/voyager/layouts/<id>/…`, or the id itself.
pub fn oryx_layout_id(link_or_id: &str) -> String {
    let s = link_or_id.trim();
    match s.split("/layouts/").nth(1) {
        Some(rest) => rest.split(['/', '?', '#']).next().unwrap_or_default().to_string(),
        None => s.to_string(),
    }
}

/// Pushes with the real Oryx: no login for a new layout, the user's token for `to`.
pub fn push_live(ws: &Workspace, draft_name: &str, force: bool, target: &PushTarget) -> Result<keyloom::push::Pushed> {
    // Check the draft before asking for a token, so a refused draft never needs one.
    let mut oryx = match &target.to {
        Some(_) => {
            push_check(ws, draft_name, force)?;
            keyloom::push::OryxHttp::with_token(&oryx_token()?)
        }
        None => keyloom::push::OryxHttp::new(),
    };
    push(ws, draft_name, force, target, &mut oryx)
}

fn push_check(ws: &Workspace, draft_name: &str, force: bool) -> Result<Layout> {
    let draft = ws.load_draft(draft_name)?;
    let blockers = guide_blockers(ws, &draft);
    if !blockers.is_empty() && !force {
        anyhow::bail!(
            "{}Nothing was sent to Oryx. Fix the errors, or push anyway with --force (MCP: force=true).",
            blockers_text(&draft, &blockers)
        );
    }
    Ok(draft)
}

/// Writes a draft to Oryx: into a new anonymous layout (nothing in the user's account changes), or with
/// `target.to` into an uncompiled revision of the user's own layout (`oryx` must carry their login).
/// Refuses a draft with validation errors unless `force`. Never compiles.
pub fn push(
    ws: &Workspace,
    draft_name: &str,
    force: bool,
    target: &PushTarget,
    oryx: &mut dyn keyloom::push::Transport,
) -> Result<keyloom::push::Pushed> {
    let draft = push_check(ws, draft_name, force)?;
    let base = base_of(ws, &draft).ok();
    Ok(match &target.to {
        Some(to) if to.trim().eq_ignore_ascii_case("new") => keyloom::push::push_new_owned(oryx, &draft, base.as_ref())?,
        Some(to) => keyloom::push::push_into(oryx, &draft, base.as_ref(), &oryx_layout_id(to), target.overwrite)?,
        None => keyloom::push::push_new(oryx, &draft, base.as_ref())?,
    })
}

/// The push result as text.
pub fn push_text(p: &keyloom::push::Pushed) -> String {
    let mut out = if p.new_layout && p.in_account {
        format!("Created a new layout in your Oryx account from {}: {}\n", p.draft, p.url)
    } else if p.new_layout {
        format!("Created a new layout on Oryx from {}: {}\n", p.draft, p.url)
    } else {
        format!("Wrote {} into your layout {} on Oryx as an uncompiled revision: {}\n", p.draft, p.layout_id, p.url)
    };
    out.push_str(&format!(
        "  {} {}, then wrote {} layer(s), added {}, removed {}\n",
        if p.new_layout { "copied" } else { "started from" },
        p.copied_from,
        p.layers_written,
        p.layers_added,
        p.layers_removed
    ));
    if p.verified {
        out.push_str("  ✓ read back from Oryx: layers and keys match the draft\n");
    } else {
        out.push_str(&format!("  ✗ read back from Oryx: {} difference(s) from the draft; check the layout in Oryx\n", p.differences.len()));
    }
    for w in &p.warnings {
        out.push_str(&format!("  ! {w}\n"));
    }
    out.push_str(if p.new_layout && p.in_account {
        "Next: open the link, check the layout, then compile and flash in Oryx. It is a new layout in your account; \
         your other layouts are unchanged.\n"
    } else if p.new_layout {
        "Next: open the link, check the layout, then compile and flash in Oryx. It is a new public layout that belongs to no account; \
         your own layouts are unchanged.\n"
    } else {
        "Next: open the link, check the changes, then compile and flash in Oryx. Nothing was compiled, and the revisions you \
         compiled before are unchanged.\n"
    });
    out
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyResult {
    #[serde(flatten)]
    pub verification: Verification,
    /// Set when there is nothing new to compare against yet.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// Compares a draft with the newest imported revision (or `against`).
pub fn verify(ws: &Workspace, draft_name: &str, against: Option<&str>) -> Result<VerifyResult> {
    let draft = ws.load_draft(draft_name)?;
    let base = base_of(ws, &draft)?;
    let imported = match against {
        Some(r) => resolve(ws, Some(r))?,
        None => ws.resolve(Some(&draft.layout_id))?,
    };
    let verification = guide::verify(&base, &draft, &imported);
    let done = verification.matches.then(|| {
        format!(
            "Done: {} has everything in this draft. Archive it (`keyloom draft archive`); for further changes create a new draft, which starts from {}.",
            imported.reference(),
            imported.reference()
        )
    });
    let note = (imported.revision_id == draft.revision_id && !verification.matches).then(|| {
        format!(
            "Nothing new was imported: {} is the revision the draft started from, so everything is still to do. \
             After applying the guide in Oryx and flashing, import the new revision (`keyloom import {}`, or the import_layout tool) and verify again.",
            imported.reference(),
            draft.layout_id
        )
    });
    Ok(VerifyResult { verification, note: note.or(done) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use keyloom::ops::Op;

    fn fixture() -> Layout {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../keyloom/tests/fixtures/oryx-9qMJK-ZPjbXz.json");
        source::read_file(&path).unwrap()
    }

    #[test]
    fn draft_layers_map_to_the_revision_on_the_keyboard() {
        let dir = tempfile::tempdir().unwrap();
        let ws = Workspace::open(dir.path()).unwrap();
        ws.store_revision(&fixture(), None).unwrap();
        let mut draft = ws.create_draft("nav", &ws.resolve(None).unwrap()).unwrap();
        let ops: Vec<Op> = serde_json::from_value(serde_json::json!([{"op": "add_layer", "title": "Nav", "after": 0}])).unwrap();
        apply_ops(&mut draft, &ops).unwrap();
        ws.save_draft(&mut draft, &ops).unwrap();
        // Not flashed yet: Sym+Num (2 in the draft) is layer 1 on the keyboard, Nav isn't there.
        assert_eq!(keyboard_layer_on(&ws, &draft, 2, None).unwrap(), 1);
        assert!(keyboard_layer_on(&ws, &draft, 1, None).unwrap_err().to_string().contains("isn't in 9qMJK@ZPjbXz"));
        // Applied, flashed and re-imported: the draft's numbers are the keyboard's.
        let mut flashed = draft.clone();
        flashed.kind = Kind::Revision;
        flashed.draft = None;
        flashed.revision_id = "NEWrev".into();
        flashed.created_at = Some("2030-01-01 00:00:00 UTC".into());
        ws.store_revision(&flashed, None).unwrap();
        assert_eq!(keyboard_layer_on(&ws, &draft, 1, None).unwrap(), 1);
        assert_eq!(keyboard_layer_on(&ws, &draft, 2, None).unwrap(), 2);
        // Imported but the keyboard still reports the old revision: the old numbering applies,
        // also for the new revision itself.
        assert_eq!(keyboard_layer_on(&ws, &draft, 2, Some("9qMJK/ZPjbXz")).unwrap(), 1);
        assert_eq!(keyboard_layer_on(&ws, &ws.resolve(None).unwrap(), 2, Some("9qMJK/ZPjbXz")).unwrap(), 1);
        assert!(keyboard_layer_on(&ws, &draft, 1, Some("9qMJK/ZPjbXz")).is_err());
        // A serial of another layout or an unknown revision is ignored.
        assert_eq!(keyboard_layer_on(&ws, &draft, 2, Some("Other/Rev")).unwrap(), 2);
    }
}
