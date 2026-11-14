//! `keyloom mcp`: the MCP server (spec 05) on stdio. Same engine as the CLI; tool results are the
//! CLI's `--json` objects (plus ASCII boards where a picture helps).

use crate::app;
use crate::keymapp::Client;
use crate::live::{self, Session};
use keyloom::explore;
use keyloom::knowledge;
use keyloom::ops::{self, Op};
use keyloom::positions::PositionMap;
use keyloom::render::{self, AsciiOptions, Style};
use rmcp::handler::server::router::{prompt::PromptRouter, tool::ToolRouter};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    CallToolResult, ContentBlock, Implementation, ListResourcesResult, PaginatedRequestParams, PromptMessage, ReadResourceRequestParams,
    ReadResourceResponse, ReadResourceResult, Resource, ResourceContents, Role, ServerCapabilities, ServerConfig,
};
use rmcp::service::RequestContext;
use rmcp::{prompt, prompt_handler, prompt_router, tool, tool_handler, tool_router, ErrorData, RoleServer, ServerHandler, ServiceExt};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::Mutex;

const INSTRUCTIONS: &str = "keyloom knows the user's ZSA keyboard layouts (imported from Oryx). \
Keys are named by position: `L.home.4` = left half, home row, column 4 counted from the OUTER edge \
(0 outer pinky … 5 inner index); rows num, top, home, bot; thumbs `L.thumb.1` (next to the main block) and \
`L.thumb.2` (toward the centre). Layers by number or title. Start with layout_summary, then render_layer; nothing \
imported yet: ask for the user's Oryx link, then import_layout. Never guess keycodes: lookup_keycode. Before designing \
a layer read keyloom://guides/layer-design. Revisions are read-only: create_draft, then edit_draft; each edit returns \
its diff and findings, fix errors. keyloom can't compile or flash: finish with apply_guide (steps for the user \
in Oryx, or with push=true and the user's OK a new Oryx layout holding the draft; into their own layout only with `to`, when they ask); after they flashed, import_layout again and verify_draft. The keyboard tool changes the physical \
keyboard's lights/layer; use action=reset when done.";

#[derive(Clone)]
pub struct KeyloomServer {
    tool_router: ToolRouter<Self>,
    prompt_router: PromptRouter<Self>,
    live: Arc<Mutex<Session>>,
}

// MARK: parameters

/// A layer or key given as a number or as text.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Sel {
    Number(i64),
    Text(String),
}

impl Sel {
    fn value(&self) -> Value {
        match self {
            Sel::Number(n) => json!(n),
            Sel::Text(s) => json!(s),
        }
    }
    fn text(&self) -> String {
        match self {
            Sel::Number(n) => n.to_string(),
            Sel::Text(s) => s.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum StyleArg {
    /// Readable labels.
    Legend,
    /// QMK names: tap on line 1, hold on line 2.
    Codes,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LayoutArg {
    /// `layoutId`, `layoutId@revisionId`, a draft (`draft:wow` or `wow`), or omit for the active layout's newest revision.
    #[serde(default)]
    pub layout: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ImportArgs {
    /// An Oryx link (`https://configure.zsa.io/voyager/layouts/<id>/latest/0`), a layout id (`9qMJK`,
    /// `9qMJK/ZPjbXz`), a path to a .json layout file, or `keymapp` for the revisions Keymapp has stored locally.
    pub source: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RenderArgs {
    /// Layout or draft; default the active layout.
    #[serde(default)]
    pub layout: Option<String>,
    /// Layer number or title; default the base layer.
    #[serde(default)]
    pub layer: Option<Sel>,
    /// Default `legend`.
    #[serde(default)]
    pub style: Option<StyleArg>,
    /// Also return every key's position, QMK expression and slots as JSON.
    #[serde(default)]
    pub json: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct KeyArgs {
    /// Layout or draft; default the active layout.
    #[serde(default)]
    pub layout: Option<String>,
    /// Position like `L.home.4`, key index like 17 or `#17`, or `key:Esc` (the base-layer key sending Esc).
    pub key: Sel,
    /// Limit to one layer (number or title).
    #[serde(default)]
    pub layer: Option<Sel>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FindArgs {
    /// Layout or draft; default the active layout.
    #[serde(default)]
    pub layout: Option<String>,
    /// Keycode or alias (`Esc`, `KC_ESCAPE`), layer action (`MO(1)`, `TG(Gaming)`) or label text.
    pub query: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LayerArgs {
    /// Layout or draft; default the active layout.
    #[serde(default)]
    pub layout: Option<String>,
    /// Layer number or title.
    pub layer: Sel,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DiffArgs {
    /// Layout or draft to compare from (e.g. `9qMJK@ZPjbXz`).
    pub from: String,
    /// Layout or draft to compare to (e.g. `draft:wow`).
    pub to: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LookupArgs {
    /// Keycode, alias, symbol or words (`Esc`, `volume up`, `!`, `right shift`).
    pub query: String,
    /// Maximum results; default 10.
    #[serde(default)]
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateDraftArgs {
    /// Draft name: letters, digits, `-`, `_` (e.g. `wow`).
    pub name: String,
    /// What to copy: a revision or another draft; default the newest revision of the active layout.
    #[serde(default)]
    pub from: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EditArgs {
    /// Draft name.
    pub draft: String,
    /// Edit operations, applied in order, all or nothing. Each is an object with "op" and its fields
    /// (layers by number or title, keys by position like `L.home.4`):
    /// set_key{layer,key,action} · set_slot{layer,key,slot:tap|hold|doubleTap|tapHold,action|null} ·
    /// clear_key{layer,key,to:trans|none} · set_label{layer,key,label} · set_color{layer,key,color} ·
    /// add_layer{title,after?,copy_from?,fill:trans|none,tags[],color?} · move_layer{layer,to} ·
    /// remove_layer{layer,force?} (force also clears keys elsewhere that switch to it) ·
    /// rename_layer{layer,title} · tag_layer{layer,tags[]} (tags: base, gaming, nav, symbols, numbers, mouse,
    /// function, media, auto) ·
    /// color_layer{layer,color} · add_combo{layer,keys[],action,name?} · remove_combo{layer,keys[]} ·
    /// swap_keys{layer|"all",a,b}. Actions: `KC_A`, `Esc`, `LCTL(KC_C)`, `MO(3)`, `TG(Gaming)`,
    /// `TO(0)`, `OSL(sym)`, `TT(2)`, `DF(0)`, `LT(3, KC_SPC)`, `LCTL_T(KC_A)`, `MEH_T(KC_BSPC)`,
    /// `TAP_HOLD(KC_EQL, KC_ESC)`, `RGB(#ff0000)`, `TRNS` (falls through), `NO` (does nothing).
    /// fill none = empty keys do nothing (gaming layers). Later ops can use a layer added earlier in the list.
    pub ops: Vec<Value>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DraftArg {
    /// Draft name.
    pub draft: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GuideArgs {
    /// Draft name.
    pub draft: String,
    /// Instead of steps, create a NEW public layout on Oryx holding the draft and return its link (no login; the
    /// user's own layouts never change). Ask the user before pushing.
    #[serde(default)]
    pub push: bool,
    /// With `push`: write into this layout of the user's own (id or link) as an uncompiled revision instead of
    /// a new layout. Needs the login token the user stored for keyloom. Only when the user asked for it.
    #[serde(default)]
    pub to: Option<String>,
    /// With `to`: replace changes on Oryx the draft doesn't have. Only when the user said so.
    #[serde(default)]
    pub overwrite: bool,
    /// With `push`: push even if the draft has validation errors.
    #[serde(default)]
    pub force: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VerifyArgs {
    /// Draft name.
    pub draft: String,
    /// Revision to compare against; default the newest imported one.
    #[serde(default)]
    pub against: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum KeyboardAction {
    /// Keymapp reachable? Connected keyboard, current layer, flashed layout.
    Status,
    /// Switch to `layer` (0 = base). It must exist in the flashed firmware.
    ShowLayer,
    /// Light `keys` (the rest goes dark); optional `layer` first.
    Highlight,
    /// Undo keyloom's changes: lights back to normal, its layer off.
    Reset,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct KeyboardArgs {
    pub action: KeyboardAction,
    /// Layout or draft that `layer` and `keys` refer to; default the active layout.
    #[serde(default)]
    pub layout: Option<String>,
    /// Layer number or title.
    #[serde(default)]
    pub layer: Option<Sel>,
    /// Positions like `L.home.4` (highlight).
    #[serde(default)]
    pub keys: Vec<Sel>,
    /// `#rrggbb` (highlight); default from the user's config.
    #[serde(default)]
    pub color: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SummaryArgs {
    /// Layout or draft; default the active layout.
    #[serde(default)]
    pub layout: Option<String>,
    /// Also return the layer graph: every layer key/combo as an edge, reachability, exits.
    #[serde(default)]
    pub graph: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DesignArgs {
    /// What the layer is for, e.g. "gaming", "World of Warcraft", "navigation".
    pub purpose: String,
    /// Layout to work on; default the active layout.
    #[serde(default)]
    pub layout: Option<String>,
    /// Anything the user already said about it.
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GameArgs {
    /// The game, e.g. "World of Warcraft".
    pub game: String,
    /// Layout to work on; default the active layout.
    #[serde(default)]
    pub layout: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LayoutPromptArgs {
    /// Layout to look at; default the active layout.
    #[serde(default)]
    pub layout: Option<String>,
}

// MARK: helpers

fn text_result(text: String) -> CallToolResult {
    CallToolResult::success(vec![ContentBlock::text(text)])
}

fn error_result(e: impl std::fmt::Display) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(e.to_string())])
}

fn json_result<T: Serialize>(v: &T) -> CallToolResult {
    text_result(serde_json::to_string_pretty(v).unwrap_or_default())
}

fn run<T: Serialize>(f: impl FnOnce() -> anyhow::Result<T>) -> Result<CallToolResult, ErrorData> {
    // A bug must come back as an error the agent can read, not as a call that never answers.
    Ok(match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
        Ok(Ok(v)) => json_result(&v),
        Ok(Err(e)) => error_result(format!("{e:#}")),
        Err(_) => error_result(
            "keyloom hit an internal error on this call (a bug). Nothing was saved; `validate` may show what is unusual about this layout",
        ),
    })
}

fn layer_of(layout: &keyloom::model::Layout, sel: Option<&Sel>) -> anyhow::Result<i64> {
    match sel {
        Some(s) => Ok(ops::resolve_layer(layout, &s.value())?),
        None => Ok(layout.layers.first().map(|l| l.position).unwrap_or(0)),
    }
}

fn edit(draft: &str, op_list: Vec<Op>) -> anyhow::Result<app::EditResult> {
    let ws = app::open()?;
    app::edit(&ws, draft, &op_list).map_err(|e| anyhow::anyhow!("{e:#} (nothing was changed)"))
}

/// ASCII chart of Voyager position names (resource `keyloom://reference/voyager`).
pub fn voyager_reference() -> String {
    let m = PositionMap::voyager();
    let mut out = String::from(
        "# Voyager positions\n\n52 keys: per half 4 rows (num, top, home, bot) x 6 columns + 2 thumb keys.\n\
Columns count from the OUTER edge on both halves: 0 outer pinky, 1 pinky, 2 ring, 3 middle, 4 index, 5 inner index.\n\
Thumbs: thumb.1 next to the main block, thumb.2 toward the centre.\n\nAs seen on the desk (left half | right half):\n\n",
    );
    for row in ["num", "top", "home", "bot"] {
        let left: Vec<String> = (0..6).map(|c| format!("L.{row}.{c}")).collect();
        let right: Vec<String> = (0..6).rev().map(|c| format!("R.{row}.{c}")).collect();
        out.push_str(&format!("{:<60} | {}\n", left.join(" "), right.join(" ")));
    }
    out.push_str(&format!("{:>60} | {}\n\n", "L.thumb.1 L.thumb.2", "R.thumb.2 R.thumb.1"));
    out.push_str("index  position   finger  effort (0 easy … 1 hard)\n");
    for p in &m.positions {
        out.push_str(&format!("{:>5}  {:<10} {:<7} {:.2}\n", p.index, p.name, format!("{:?}", p.finger).to_lowercase(), p.effort));
    }
    out
}

// MARK: tools

#[tool_router(router = tool_router)]
impl KeyloomServer {
    pub fn new() -> Self {
        let state = app::open().ok().map(|ws| ws.root().join("cache/live.json"));
        Self {
            tool_router: Self::tool_router(),
            prompt_router: Self::prompt_router(),
            live: Arc::new(Mutex::new(Session::new(Client::default_for_platform(), state))),
        }
    }

    #[tool(
        description = "Imported layouts, revisions and drafts, and the active layout. Empty: ask the user for their Oryx link, then import_layout."
    )]
    async fn list_layouts(&self) -> Result<CallToolResult, ErrorData> {
        run(|| app::list_summary(&app::open()?))
    }

    #[tool(
        description = "Import from Oryx (link or id), a .json file, or `keymapp` (its local copies). Fetches the newest revision; call again after the user flashed, before verify_draft."
    )]
    async fn import_layout(&self, Parameters(a): Parameters<ImportArgs>) -> Result<CallToolResult, ErrorData> {
        // Network and disk work: keep it off the async threads.
        let r = tokio::task::spawn_blocking(move || -> anyhow::Result<Value> {
            let ws = app::open()?;
            let results = app::import(&ws, &a.source, true)?;
            Ok(json!({"imported": results.iter().map(|(l, s)| app::import_summary(l, s)).collect::<Vec<_>>(), "active": ws.config()?.active_layout}))
        })
        .await;
        Ok(match r {
            Ok(Ok(v)) => json_result(&v),
            Ok(Err(e)) => error_result(format!("{e:#}")),
            Err(e) => error_result(e),
        })
    }

    #[tool(
        description = "Start here. Layers (title, tags, key counts, how each is reached and left), combos, auto-mouse layer. graph=true adds every layer key as an edge."
    )]
    async fn layout_summary(&self, Parameters(a): Parameters<SummaryArgs>) -> Result<CallToolResult, ErrorData> {
        run(|| {
            let layout = app::resolve(&app::open()?, a.layout.as_deref())?;
            let summary = serde_json::to_value(explore::summary(&layout))?;
            Ok(if a.graph { json!({"summary": summary, "graph": explore::layer_graph(&layout)}) } else { summary })
        })
    }

    #[tool(
        description = "One layer as ASCII (per key: tap, then hold; ▽ = transparent, showing what it falls through to). json=true adds each key's position, QMK expression and slots."
    )]
    async fn render_layer(&self, Parameters(a): Parameters<RenderArgs>) -> Result<CallToolResult, ErrorData> {
        let result = (|| -> anyhow::Result<CallToolResult> {
            let layout = app::resolve(&app::open()?, a.layout.as_deref())?;
            let layer = layer_of(&layout, a.layer.as_ref())?;
            let view = render::layer_view(&layout, layer).ok_or_else(|| anyhow::anyhow!("no layer {layer}"))?;
            let style = if matches!(a.style, Some(StyleArg::Codes)) { Style::Codes } else { Style::Legend };
            let mut blocks = vec![ContentBlock::text(render::ascii(&view, &AsciiOptions { style, ..Default::default() }))];
            if a.json {
                blocks.push(ContentBlock::text(serde_json::to_string_pretty(&view)?));
            }
            Ok(CallToolResult::success(blocks))
        })();
        Ok(result.unwrap_or_else(|e| error_result(format!("{e:#}"))))
    }

    #[tool(description = "One physical key: finger, effort, and its binding on every layer.")]
    async fn key_info(&self, Parameters(a): Parameters<KeyArgs>) -> Result<CallToolResult, ErrorData> {
        run(|| {
            let layout = app::resolve(&app::open()?, a.layout.as_deref())?;
            let index = ops::resolve_key(&layout, &a.key.value())?;
            let layer = a.layer.as_ref().map(|l| ops::resolve_layer(&layout, &l.value())).transpose()?;
            Ok(explore::key_info(&layout, index, layer))
        })
    }

    #[tool(
        description = "Where a keycode, layer action or label is on the layout (layer, position, slot) and how to reach it. Empty = not on the layout."
    )]
    async fn find_keycode(&self, Parameters(a): Parameters<FindArgs>) -> Result<CallToolResult, ErrorData> {
        run(|| Ok(explore::find(&app::resolve(&app::open()?, a.layout.as_deref())?, &a.query)))
    }

    #[tool(description = "Free keys on a layer (transparent or KC_NO), easiest first, with finger and effort (0 easy … 1 hard).")]
    async fn free_keys(&self, Parameters(a): Parameters<LayerArgs>) -> Result<CallToolResult, ErrorData> {
        run(|| {
            let layout = app::resolve(&app::open()?, a.layout.as_deref())?;
            Ok(explore::free_keys(&layout, ops::resolve_layer(&layout, &a.layer.value())?))
        })
    }

    #[tool(description = "Search the keycode reference (code, aliases, description). Use it instead of guessing keycodes.")]
    async fn lookup_keycode(&self, Parameters(a): Parameters<LookupArgs>) -> Result<CallToolResult, ErrorData> {
        run(|| Ok(knowledge::search(&a.query, a.limit.unwrap_or(10))))
    }

    #[tool(
        description = "Audit a layout or draft: unreachable layers, layers you can't leave, unknown keycodes, combo conflicts, gaming-layer pitfalls. Fix errors before apply_guide."
    )]
    async fn validate(&self, Parameters(a): Parameters<LayoutArg>) -> Result<CallToolResult, ErrorData> {
        run(|| {
            let ws = app::open()?;
            let layout = app::resolve(&ws, a.layout.as_deref())?;
            Ok(app::validation_report(&ws, &layout).0)
        })
    }

    #[tool(description = "Differences between two layouts or drafts: layers, key slots, combos.")]
    async fn diff(&self, Parameters(a): Parameters<DiffArgs>) -> Result<CallToolResult, ErrorData> {
        run(|| {
            let ws = app::open()?;
            let (x, y) = (app::resolve(&ws, Some(&a.from))?, app::resolve(&ws, Some(&a.to))?);
            Ok(keyloom::diff::diff(&x, &y, Default::default()))
        })
    }

    #[tool(description = "Create a draft (an editable copy) of a revision or another draft. All edits happen in drafts.")]
    async fn create_draft(&self, Parameters(a): Parameters<CreateDraftArgs>) -> Result<CallToolResult, ErrorData> {
        run(|| {
            let ws = app::open()?;
            let from = app::resolve(&ws, a.from.as_deref())?;
            let d = ws.create_draft(&a.name, &from)?;
            Ok(json!({"draft": d.reference(), "basedOn": from.reference(), "summary": explore::summary(&d)}))
        })
    }

    #[tool(
        description = "Edit a draft: a list of operations, all or nothing. Returns the diff and the draft's findings (newFindings = caused by this edit). On error nothing changes."
    )]
    async fn edit_draft(&self, Parameters(a): Parameters<EditArgs>) -> Result<CallToolResult, ErrorData> {
        run(|| {
            let parsed: Vec<Op> = a
                .ops
                .iter()
                .enumerate()
                .map(|(i, v)| {
                    serde_json::from_value(v.clone()).map_err(|e| anyhow::anyhow!("op {}: {e} in {v} (nothing was changed)", i + 1))
                })
                .collect::<anyhow::Result<_>>()?;
            edit(&a.draft, parsed)
        })
    }

    #[tool(description = "Undo the last edit_draft call on a draft.")]
    async fn undo(&self, Parameters(a): Parameters<DraftArg>) -> Result<CallToolResult, ErrorData> {
        run(|| app::undo(&app::open()?, &a.draft))
    }

    #[tool(
        description = "Get a draft into Oryx. Default: step-by-step instructions for the user to make in Oryx; show them. push=true (ask the user first): create a new public Oryx layout holding the draft and return its link, for the user to check, compile and flash; the user's own layouts don't change. With to=<their layout> (only when they ask; needs their stored login token): an uncompiled revision of that layout instead. keyloom never compiles or flashes."
    )]
    async fn apply_guide(&self, Parameters(a): Parameters<GuideArgs>) -> Result<CallToolResult, ErrorData> {
        if a.push {
            // Blocking network calls: keep them off the async runtime.
            let (draft, force) = (a.draft.clone(), a.force);
            let target = app::PushTarget { to: a.to.clone(), overwrite: a.overwrite };
            let pushed = tokio::task::spawn_blocking(move || app::push_live(&app::open()?, &draft, force, &target)).await;
            return Ok(match pushed {
                Ok(Ok(p)) => json_result(&p),
                Ok(Err(e)) => error_result(format!("{e:#}")),
                Err(e) => error_result(e),
            });
        }
        let r = (|| -> anyhow::Result<String> {
            let ws = app::open()?;
            let (g, d) = app::guide_for(&ws, &a.draft)?;
            Ok(format!("{}{}", app::blockers_text(&d, &app::guide_blockers(&ws, &d)), keyloom::guide::render(&g)))
        })();
        Ok(match r {
            Ok(text) => text_result(text),
            Err(e) => error_result(format!("{e:#}")),
        })
    }

    #[tool(
        description = "After the user applied the guide and flashed, and you re-imported: does the newest revision match the draft? Lists what is still to do and unexpected differences (`from` = Oryx now, `to` = draft)."
    )]
    async fn verify_draft(&self, Parameters(a): Parameters<VerifyArgs>) -> Result<CallToolResult, ErrorData> {
        run(|| app::verify(&app::open()?, &a.draft, a.against.as_deref()))
    }

    #[tool(
        description = "The physical keyboard, through Keymapp (needs Keymapp running with Settings → API on): status, show_layer, highlight keys, reset. Call reset when done showing."
    )]
    async fn keyboard(&self, Parameters(a): Parameters<KeyboardArgs>) -> Result<CallToolResult, ErrorData> {
        match a.action {
            KeyboardAction::Status => Ok(json_result(&live::status(&self.live.lock().await.client).await)),
            KeyboardAction::Reset => Ok(match self.live.lock().await.reset(true).await {
                Ok(report) => json_result(&report),
                Err(e) => error_result(e),
            }),
            KeyboardAction::ShowLayer | KeyboardAction::Highlight => {
                let highlight = matches!(a.action, KeyboardAction::Highlight);
                let prep = (|| -> anyhow::Result<(Vec<usize>, Option<i64>, String)> {
                    let ws = app::open()?;
                    let layout = app::resolve(&ws, a.layout.as_deref())?;
                    if highlight && a.keys.is_empty() {
                        anyhow::bail!("highlight needs `keys`");
                    }
                    if !highlight && a.layer.is_none() {
                        anyhow::bail!("show_layer needs `layer`");
                    }
                    let keys = a.keys.iter().map(|k| ops::resolve_key(&layout, &k.value())).collect::<Result<Vec<_>, _>>()?;
                    let layer = a.layer.as_ref().map(|l| ops::resolve_layer(&layout, &l.value())).transpose()?;
                    // A draft's layer numbers aren't the keyboard's.
                    let layer = layer.map(|l| app::keyboard_layer(&ws, &layout, l)).transpose()?;
                    let color = a.color.clone().unwrap_or(ws.config()?.live.highlight_color);
                    Ok((keys, layer, color))
                })();
                let (keys, layer, color) = match prep {
                    Ok(x) => x,
                    Err(e) => return Ok(error_result(format!("{e:#}"))),
                };
                let mut s = self.live.lock().await;
                let r = if highlight { s.highlight(&keys, layer, &color).await } else { s.show_layer(layer.unwrap_or(0)).await };
                Ok(match r {
                    Ok(()) if highlight => json_result(&json!({
                        "highlighted": a.keys.iter().map(Sel::text).collect::<Vec<_>>(),
                        "layer": layer,
                        "keymappReconnected": s.reconnected,
                    })),
                    Ok(()) => json_result(&json!({"layer": layer, "keymappReconnected": s.reconnected})),
                    Err(e) => error_result(e),
                })
            }
        }
    }
}

impl Default for KeyloomServer {
    fn default() -> Self {
        Self::new()
    }
}

// MARK: prompts

fn layout_hint(layout: &Option<String>) -> String {
    layout.as_deref().map(|l| format!(" (layout `{l}`)")).unwrap_or_default()
}

const FINISH: &str = "When the draft is done: show the user render_layer for every changed layer and the diff, run validate \
and fix all errors, then give them apply_guide's steps. If keyboard(status) shows Keymapp is reachable, offer \
keyboard(highlight) on the changed keys (and keyboard(reset) after). After they flashed, call import_layout with the layout id, then verify_draft.";

#[prompt_router]
impl KeyloomServer {
    #[prompt(
        name = "design_layer",
        description = "Design a new layer for a purpose (gaming, nav, symbols, …) together with the user, then build it in a draft."
    )]
    async fn design_layer(&self, Parameters(a): Parameters<DesignArgs>) -> Vec<PromptMessage> {
        let notes = a.notes.as_deref().map(|n| format!("\nUser notes: {n}")).unwrap_or_default();
        vec![PromptMessage::new_text(
            Role::User,
            format!(
                "Help me design a new layer for: {purpose}{layout}.{notes}\n\n\
1. Call layout_summary and render_layer for the base layer to learn my layout; read keyloom://guides/layer-design and the guide that fits the purpose (gaming, mmo, nav, symbols, numbers, mouse).\n\
2. Ask me the questions the guide suggests before deciding anything (which hand is on the mouse, which keys matter, how I want to enter and leave the layer).\n\
3. Use free_keys and layout_summary(graph=true) to pick where the layer key goes; propose the layer as a small table of positions → keys and wait for my OK.\n\
4. create_draft, then edit_draft in one or a few batches (tag the layer). Never guess keycodes: lookup_keycode.\n\
5. {FINISH}",
                purpose = a.purpose,
                layout = layout_hint(&a.layout)
            ),
        )]
    }

    #[prompt(
        name = "add_gaming_layer",
        description = "Add a gaming layer for a specific game: plain keys, no tap-hold, a toggle in and out."
    )]
    async fn add_gaming_layer(&self, Parameters(a): Parameters<GameArgs>) -> Vec<PromptMessage> {
        vec![PromptMessage::new_text(
            Role::User,
            format!(
                "Add a gaming layer for {game}{layout}.\n\n\
Read keyloom://guides/gaming (and keyloom://guides/mmo for MMOs like World of Warcraft) first. Check layout_summary: \
if there is already an unused or unreachable gaming layer, offer to reuse it. Ask me which hand is on the mouse, \
which game keys I use (movement, abilities/action bars, modifiers, chat, push-to-talk), and how I want to enter and leave the layer. \
Use plain keys only (no hold/tap-hold), bind every key the game needs explicitly so base-layer mod-taps don't fall through \
(a new layer with fill=none, or NO on unused keys), tag the layer `gaming`, enter it with TG(layer) and leave it with the same key. \
Propose the layout as a table first.\n\n{FINISH}",
                game = a.game,
                layout = layout_hint(&a.layout)
            ),
        )]
    }

    #[prompt(name = "review_layout", description = "Review a layout: validation, ergonomics and suggestions. No edits.")]
    async fn review_layout(&self, Parameters(a): Parameters<LayoutPromptArgs>) -> Vec<PromptMessage> {
        vec![PromptMessage::new_text(
            Role::User,
            format!(
                "Review my layout{}. Call layout_summary (graph=true) and validate, render each layer, and read keyloom://guides/layer-design. \
Report: problems (validation errors and warnings, explained), ergonomic issues (frequent keys on high-effort positions, \
same-finger strain, layer keys off the thumbs), and 3–5 concrete suggestions with positions. Don't edit anything.",
                layout_hint(&a.layout)
            ),
        )]
    }

    #[prompt(name = "explain_layout", description = "Teach the user their own layout, layer by layer.")]
    async fn explain_layout(&self, Parameters(a): Parameters<LayoutPromptArgs>) -> Vec<PromptMessage> {
        vec![PromptMessage::new_text(
            Role::User,
            format!(
                "Explain my layout{} to me. Start with layout_summary, then go layer by layer with render_layer: what the layer is for, \
how I get there and back, and the keys worth remembering. Point out hold actions and combos. Keep it short per layer.",
                layout_hint(&a.layout)
            ),
        )]
    }
}

// MARK: resources

const MARKDOWN: &str = "text/markdown";
const JSON: &str = "application/json";

fn resources() -> Vec<(String, String, &'static str)> {
    let mut out = vec![
        ("keyloom://reference/keycodes".to_string(), "Keycode reference".to_string(), JSON),
        ("keyloom://reference/voyager".into(), "Voyager positions, fingers and effort".into(), MARKDOWN),
        ("keyloom://reference/layer-actions".into(), "Layer actions (MO, LT, TG, TO, TT, OSL, DF)".into(), MARKDOWN),
    ];
    for (name, _) in knowledge::guides() {
        out.push((format!("keyloom://guides/{name}"), format!("Guide: {name}"), MARKDOWN));
    }
    if let Ok(ws) = app::open() {
        for m in ws.layouts().unwrap_or_default() {
            out.push((format!("keyloom://layouts/{}/summary", m.layout_id), format!("Layout {} \"{}\"", m.layout_id, m.title), JSON));
        }
    }
    out
}

/// (text, mime type)
fn read(uri: &str) -> anyhow::Result<(String, &'static str)> {
    if uri == "keyloom://reference/keycodes" {
        return Ok((serde_json::to_string_pretty(knowledge::keycodes())?, JSON));
    }
    if uri == "keyloom://reference/voyager" {
        return Ok((voyager_reference(), MARKDOWN));
    }
    if uri == "keyloom://reference/layer-actions" {
        return Ok((knowledge::LAYER_ACTIONS.to_string(), MARKDOWN));
    }
    if let Some(name) = uri.strip_prefix("keyloom://guides/") {
        let names = || knowledge::guides().iter().map(|(n, _)| *n).collect::<Vec<_>>().join(", ");
        return knowledge::guide(name)
            .map(|t| (t.to_string(), MARKDOWN))
            .ok_or_else(|| anyhow::anyhow!("no guide '{name}' (guides: {})", names()));
    }
    if let Some(id) = uri.strip_prefix("keyloom://layouts/").and_then(|r| r.strip_suffix("/summary")) {
        let ws = app::open()?;
        return Ok((serde_json::to_string_pretty(&explore::summary(&ws.resolve(Some(id))?))?, JSON));
    }
    anyhow::bail!("unknown resource {uri}")
}

#[tool_handler(router = self.tool_router)]
#[prompt_handler(router = self.prompt_router)]
impl ServerHandler for KeyloomServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().enable_prompts().enable_resources().build())
            .with_server_info(Implementation::new("keyloom", env!("CARGO_PKG_VERSION")))
            .with_instructions(INSTRUCTIONS)
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, ErrorData> {
        let items = resources().into_iter().map(|(uri, name, mime)| Resource::new(uri, name).with_mime_type(mime)).collect();
        Ok(ListResourcesResult::with_all_items(items))
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, ErrorData> {
        match read(&request.uri) {
            Ok((text, mime)) => Ok(ReadResourceResult::new(vec![ResourceContents::text(text, request.uri).with_mime_type(mime)]).into()),
            Err(e) => Err(ErrorData::resource_not_found(e.to_string(), None)),
        }
    }
}

/// Runs the server on stdio until the client disconnects; restores the keyboard on the way out
/// if this session changed it.
pub async fn serve() -> anyhow::Result<()> {
    let server = KeyloomServer::new();
    let live = server.live.clone();
    let service = server.serve(rmcp::transport::stdio()).await?;
    service.waiting().await?;
    let _ = live.lock().await.reset(false).await;
    Ok(())
}
