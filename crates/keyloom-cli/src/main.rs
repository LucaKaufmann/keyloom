//! `keyloom`: see, audit and improve ZSA keyboard layouts from the terminal (docs/specs/04-cli.md).

mod app;
mod keymapp;
mod live;
mod mcp;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use keyloom::diff::Change;
use keyloom::explore;
use keyloom::knowledge;
use keyloom::model::{Layout, Slot};
use keyloom::ops::{self, Fill, Op};
use keyloom::render::{self, AsciiOptions, Style};
use keyloom::validate::{has_errors, Finding, Severity};
use keyloom::workspace::{Stored, Workspace};
use serde::Serialize;
use serde_json::{json, Value};
use std::io::IsTerminal;
use std::path::PathBuf;

const REF_HELP: &str = "Layout: `layoutId`, `layoutId@revisionId`, `@revisionId`, a draft (`draft:wow` or just `wow`), or a .json file. `layoutId@latest` fetches from Oryx first. Default: the active layout's newest revision";
const LAYER_HELP: &str = "Layer number or title (case-insensitive, a unique prefix is enough); `base` is layer 0";
const KEY_HELP: &str = "Key: a position like `L.home.4` (half.row.column, columns 0-5 from the OUTER edge; thumbs `L.thumb.1`, `L.thumb.2`), an index like `#17`, or `key:Esc` (the base-layer key that sends Esc)";
const ACTION_HELP: &str = "Action: `KC_A` or `A`, `Esc`, `LCTL(KC_C)`, `MO(3)`, `TG(Gaming)`, `TO(0)`, `OSL(sym)`, `TT(2)`, `DF(0)`, `LT(3, KC_SPC)`, `LCTL_T(KC_A)`, `MEH_T(KC_BSPC)`, `TAP_HOLD(KC_EQL, KC_ESC)`, `RGB(#ff0000)`, `TRNS`, `NO`. Quote it in the shell";
const AFTER_HELP: &str = "\
Key positions: L.home.4 = left half, home row, column 4 counted from the OUTER edge
(0 outer pinky, 1 pinky, 2 ring, 3 middle, 4 index, 5 inner index). Rows: num, top, home, bot.
Thumbs: L.thumb.1 (next to the main block), L.thumb.2 (toward the centre).

Typical flow:
  keyloom import <oryx link>       keyloom show --all        keyloom validate
  keyloom draft new wow            keyloom set wow Gaming L.home.1 A
  keyloom guide wow                (apply in Oryx, flash)    keyloom import <id> && keyloom verify wow

Agents: every command takes --json. `keyloom mcp` runs the MCP server (`claude mcp add keyloom -- keyloom mcp`).
Docs: https://github.com/LucaKaufmann/keyloom; `keyloom guides` lists the layer-design guides.";

#[derive(Parser)]
#[command(name = "keyloom", version, about = "See, audit and improve ZSA keyboard layouts (CLI and MCP server for AI agents)", after_help = AFTER_HELP)]
struct Cli {
    /// Machine-readable output (stable JSON, the same objects the MCP server returns).
    #[arg(long, global = true)]
    json: bool,
    /// No ANSI colors.
    #[arg(long, global = true)]
    no_color: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Import a layout: an Oryx link or id, a .json file, or `keymapp` for everything Keymapp has stored.
    Import {
        /// e.g. https://configure.zsa.io/voyager/layouts/9qMJK/latest/0, `9qMJK`, `9qMJK/ZPjbXz`, a file, or `keymapp`
        input: String,
        /// Don't fall back to Keymapp's local copy when Oryx can't be reached.
        #[arg(long)]
        no_keymapp: bool,
    },
    /// List imported layouts, revisions and drafts.
    List,
    /// Set the layout commands use when none is named.
    Use {
        /// A layout id from `keyloom list`
        layout_id: String,
    },
    /// Draw a layer (default: the base layer). `--show` also switches the keyboard to it.
    Show {
        #[arg(help = REF_HELP)]
        reference: Option<String>,
        #[arg(short, long, help = LAYER_HELP)]
        layer: Option<String>,
        /// Every layer.
        #[arg(short, long, conflicts_with = "layer")]
        all: bool,
        /// `legend`: readable labels; `codes`: QMK names (tap on line 1, hold on line 2).
        #[arg(long, value_enum, default_value_t = StyleArg::Legend)]
        style: StyleArg,
        /// Also switch the physical keyboard to the layer (needs Keymapp's API).
        #[arg(long)]
        show: bool,
    },
    /// Layers, how they're reached and left, key counts.
    Layers {
        #[arg(help = REF_HELP)]
        reference: Option<String>,
        /// Every layer key as an edge.
        #[arg(long)]
        graph: bool,
    },
    /// Short overview of a layout.
    Summary {
        #[arg(help = REF_HELP)]
        reference: Option<String>,
    },
    /// One key on every layer.
    Key {
        #[arg(help = KEY_HELP)]
        key: String,
        #[arg(help = REF_HELP)]
        reference: Option<String>,
        #[arg(short, long, help = LAYER_HELP)]
        layer: Option<String>,
        #[arg(long = "in", hide = true)]
        in_ref: Option<String>,
        /// Light the key up on the keyboard (needs Keymapp's API).
        #[arg(long)]
        show: bool,
    },
    /// Where a keycode, layer action (`MO(1)`) or label lives.
    Find {
        /// A keycode or alias (`Esc`, `KC_ESCAPE`), a layer action (`MO(1)`, `TG(Gaming)`), or label text
        query: String,
        #[arg(help = REF_HELP)]
        reference: Option<String>,
        #[arg(long = "in", hide = true)]
        in_ref: Option<String>,
        /// Light the keys up on the keyboard (needs Keymapp's API).
        #[arg(long)]
        show: bool,
    },
    /// Free keys on a layer (transparent or KC_NO), easiest to reach first.
    Free {
        #[arg(help = REF_HELP)]
        reference: Option<String>,
        #[arg(short, long, default_value = "0", help = LAYER_HELP)]
        layer: String,
        #[arg(long = "in", hide = true)]
        in_ref: Option<String>,
        /// Light the keys up on the keyboard (needs Keymapp's API).
        #[arg(long)]
        show: bool,
    },
    /// Search the keycode reference.
    Ref {
        /// A code, alias, symbol or words: `Esc`, `volume`, `!`, `right shift`
        query: String,
    },
    /// Show a layer-design guide (layer-design, gaming, mmo, nav, …); no name lists them.
    Guides {
        /// Guide name (omit to list them)
        name: Option<String>,
    },
    /// Check a layout or draft. Exit code 3 if there are errors.
    Validate {
        #[arg(help = REF_HELP)]
        reference: Option<String>,
    },
    /// Drafts: editable copies of a layout.
    #[command(subcommand)]
    Draft(DraftCmd),
    /// Set a key on a draft: `keyloom set wow Gaming L.home.1 KC_A`.
    Set {
        /// Draft name
        draft: String,
        #[arg(help = LAYER_HELP)]
        layer: String,
        #[arg(help = KEY_HELP)]
        key: String,
        #[arg(help = ACTION_HELP)]
        action: String,
        /// Only change one slot: the action is then a single key or layer action, and `none` empties the slot.
        #[arg(long, value_enum)]
        slot: Option<SlotArg>,
    },
    /// Clear a key on a draft: transparent (falls through), or KC_NO with --none.
    Clear {
        /// Draft name
        draft: String,
        #[arg(help = LAYER_HELP)]
        layer: String,
        #[arg(help = KEY_HELP)]
        key: String,
        /// KC_NO (does nothing) instead of transparent.
        #[arg(long)]
        none: bool,
    },
    /// Layer operations on a draft.
    #[command(subcommand)]
    Layer(LayerCmd),
    /// Combos on a draft.
    #[command(subcommand)]
    Combo(ComboCmd),
    /// Apply edit operations from a JSON file (or - for stdin) to a draft, all or nothing.
    #[command(after_help = "\
The file is a JSON array of operations, e.g.
  [{\"op\": \"add_layer\", \"title\": \"WoW\", \"tags\": [\"gaming\"], \"fill\": \"none\"},
   {\"op\": \"set_key\", \"layer\": \"WoW\", \"key\": \"L.home.1\", \"action\": \"KC_A\"},
   {\"op\": \"set_key\", \"layer\": \"Main\", \"key\": \"L.thumb.1\", \"action\": \"TG(WoW)\"}]

Operations: set_key{layer,key,action} · set_slot{layer,key,slot,action|null} · clear_key{layer,key,to:trans|none} ·
set_label{layer,key,label} · set_color{layer,key,color} · add_layer{title,after?,copy_from?,fill?,tags?,color?} ·
move_layer{layer,to} · remove_layer{layer,force?} · rename_layer{layer,title} · tag_layer{layer,tags} ·
color_layer{layer,color} · add_combo{layer,keys,action,name?} · remove_combo{layer,keys} · swap_keys{layer|\"all\",a,b}")]
    Apply {
        /// Draft name
        draft: String,
        /// JSON file with an array of operations, or `-` for stdin
        file: PathBuf,
    },
    /// Differences between two layouts or drafts.
    Diff {
        #[arg(help = REF_HELP)]
        from: String,
        /// The other layout or draft
        to: String,
    },
    /// Step-by-step changes to make in Oryx for a draft. `--step` walks them with the keyboard lit.
    Guide {
        /// Draft name
        draft: String,
        /// One step at a time, lighting each key on the keyboard (needs Keymapp's API).
        #[arg(long)]
        step: bool,
    },
    /// Put a draft on Oryx: a NEW layout (no login, your own layouts never change), or with --to your own layout.
    ///
    /// Never compiles or flashes. --to needs your Oryx login token in KEYLOOM_ORYX_TOKEN (or, on macOS, in the
    /// keychain item "keyloom-oryx"); it writes an uncompiled revision for you to check and compile in Oryx.
    Push {
        /// Draft name
        draft: String,
        /// Write into this layout of yours (id or Oryx link), or "new" for a new layout in your account
        #[arg(long, value_name = "LAYOUT")]
        to: Option<String>,
        /// With --to: write even if that replaces changes on Oryx the draft doesn't have
        #[arg(long, requires = "to")]
        overwrite: bool,
        /// Push even if the draft has validation errors
        #[arg(long)]
        force: bool,
    },
    /// After applying a draft in Oryx, flashing and re-importing: check it matches. Exit code 3 if not.
    Verify {
        /// Draft name
        draft: String,
        /// Revision to compare with (default: the newest imported one)
        #[arg(long)]
        against: Option<String>,
    },
    /// Keymapp, the connected keyboard, the flashed layout.
    Status,
    /// Control the keyboard through Keymapp (its API must be enabled: Keymapp → Settings → API).
    #[command(subcommand)]
    Live(LiveCmd),
    /// Write a layout to a file or stdout.
    Export {
        #[arg(help = REF_HELP)]
        reference: Option<String>,
        /// Write to this file instead of stdout
        #[arg(short, long)]
        output: Option<PathBuf>,
        #[arg(long, value_enum, default_value_t = FormatArg::Keyloom)]
        format: FormatArg,
    },
    /// Run the MCP server on stdio (for AI agents).
    Mcp,
}

#[derive(Subcommand)]
enum DraftCmd {
    /// New draft from a revision (default: newest of the active layout) or another draft.
    New {
        /// Letters, digits, `-`, `_`
        name: String,
        #[arg(long, help = REF_HELP)]
        from: Option<String>,
    },
    /// All drafts.
    List,
    /// Draw a draft's layer (default: the first changed one) and list its changes so far.
    Show {
        /// Draft name
        name: String,
        #[arg(short, long, help = LAYER_HELP)]
        layer: Option<String>,
    },
    /// Undo the last edit (one command or one batch).
    Undo {
        /// Draft name
        name: String,
    },
    /// Move a finished draft to the archive.
    Archive {
        /// Draft name
        name: String,
    },
    /// Delete a draft.
    Rm {
        /// Draft name
        name: String,
    },
}

#[derive(Subcommand)]
enum LayerCmd {
    /// Add a layer (at the end unless --after).
    Add {
        /// Draft name
        draft: String,
        /// The new layer's title (unique, not a number)
        title: String,
        /// Insert after this layer
        #[arg(long, help = LAYER_HELP)]
        after: Option<String>,
        /// Start as a copy of this layer
        #[arg(long, help = LAYER_HELP)]
        copy_from: Option<String>,
        /// `trans`: empty keys fall through to the layer below; `none`: KC_NO, they do nothing.
        #[arg(long, value_enum, default_value_t = FillArg::Trans)]
        fill: FillArg,
        /// e.g. --tag gaming (known: base, gaming, nav, symbols, numbers, mouse, function, media, auto)
        #[arg(long = "tag")]
        tags: Vec<String>,
        /// Layer color, #rrggbb
        #[arg(long)]
        color: Option<String>,
    },
    /// Move a layer to another position; layer keys and combos follow.
    Move {
        /// Draft name
        draft: String,
        #[arg(help = LAYER_HELP)]
        layer: String,
        /// New position (0 = base)
        to: i64,
    },
    /// Rename a layer.
    Rename {
        /// Draft name
        draft: String,
        #[arg(help = LAYER_HELP)]
        layer: String,
        /// The new title
        title: String,
    },
    /// Remove a layer. Fails while keys elsewhere switch to it, unless --force.
    Rm {
        /// Draft name
        draft: String,
        #[arg(help = LAYER_HELP)]
        layer: String,
        /// Also clear the keys and combos that switch to it.
        #[arg(long)]
        force: bool,
    },
    /// Set a layer's tags (replaces them; none clears). Tags drive validation, e.g. `gaming`.
    Tag {
        /// Draft name
        draft: String,
        #[arg(help = LAYER_HELP)]
        layer: String,
        /// Known: base, gaming, nav, symbols, numbers, mouse, function, media, auto
        tags: Vec<String>,
    },
    /// Set (or with no color, clear) a layer's color.
    Color {
        /// Draft name
        draft: String,
        #[arg(help = LAYER_HELP)]
        layer: String,
        /// #rrggbb
        color: Option<String>,
    },
}

#[derive(Subcommand)]
enum ComboCmd {
    /// Add a combo: `keyloom combo add wow 0 L.home.3+L.home.4 KC_ESC`
    Add {
        /// Draft name
        draft: String,
        #[arg(help = LAYER_HELP)]
        layer: String,
        /// Two or more keys joined with `+`
        keys: String,
        /// What the combo sends: a key (`KC_ESC`, `LCTL(KC_C)`) or a layer action (`TG(Gaming)`). Quote it in the shell
        action: String,
        /// A name for the combo
        #[arg(long)]
        name: Option<String>,
    },
    /// Remove the combo with these keys.
    Rm {
        /// Draft name
        draft: String,
        #[arg(help = LAYER_HELP)]
        layer: String,
        /// The combo's keys joined with `+`
        keys: String,
    },
}

#[derive(Subcommand)]
enum LiveCmd {
    /// Switch the keyboard to a layer (0 or `base` goes back to the base layer).
    Layer {
        #[arg(help = LAYER_HELP)]
        layer: String,
        #[arg(help = REF_HELP)]
        reference: Option<String>,
        #[arg(long = "in", hide = true)]
        in_ref: Option<String>,
    },
    /// Light keys up (the rest of the board goes dark until `live reset`).
    Highlight {
        #[arg(required = true, help = KEY_HELP)]
        keys: Vec<String>,
        /// Switch to this layer first
        #[arg(short, long, help = LAYER_HELP)]
        layer: Option<String>,
        /// #rrggbb (default from config.toml)
        #[arg(long)]
        color: Option<String>,
        #[arg(long = "in", help = REF_HELP)]
        reference: Option<String>,
    },
    /// Undo what keyloom changed: lights back to normal, its layer switched off.
    Reset,
}

#[derive(Clone, Copy, ValueEnum)]
enum StyleArg {
    Legend,
    Codes,
}

#[derive(Clone, Copy, ValueEnum)]
enum FillArg {
    Trans,
    None,
}

#[derive(Clone, Copy, ValueEnum)]
enum SlotArg {
    Tap,
    Hold,
    DoubleTap,
    TapHold,
}

#[derive(Clone, Copy, ValueEnum)]
enum FormatArg {
    /// keyloom's layout file.
    Keyloom,
    /// The bare Oryx layout object, as Oryx's API returns it.
    Oryx,
}

struct Out {
    json: bool,
    color: bool,
}

impl Out {
    fn json<T: Serialize>(&self, v: &T) -> Result<()> {
        println!("{}", serde_json::to_string_pretty(v)?);
        Ok(())
    }

    /// A one-line confirmation: text, or the given object with `--json`.
    fn done(&self, text: String, value: Value) -> Result<()> {
        if self.json {
            self.json(&value)
        } else {
            println!("{text}");
            Ok(())
        }
    }
}

fn main() {
    let cli = Cli::parse();
    // `keyloom show --all | head` must end quietly, not panic on the closed pipe.
    #[cfg(unix)]
    if !matches!(cli.command, Command::Mcp) {
        // SAFETY: restoring the default action for SIGPIPE at startup, before any other thread exists.
        unsafe {
            libc::signal(libc::SIGPIPE, libc::SIG_DFL);
        }
    }
    let color = !cli.no_color && std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none();
    let out = Out { json: cli.json, color };
    let result = match cli.command {
        Command::Mcp => tokio_rt().and_then(|rt| rt.block_on(mcp::serve())),
        cmd => run(cmd, &out),
    };
    match result {
        Ok(()) => {}
        Err(e) if e.downcast_ref::<CheckFailed>().is_some() => std::process::exit(3),
        Err(e) => {
            eprintln!("keyloom: {e:#}");
            std::process::exit(1);
        }
    }
}

/// The command worked but its check didn't pass (validation errors, verify mismatch): exit 3.
#[derive(Debug)]
struct CheckFailed;
impl std::fmt::Display for CheckFailed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "check failed")
    }
}
impl std::error::Error for CheckFailed {}

fn tokio_rt() -> Result<tokio::runtime::Runtime> {
    Ok(tokio::runtime::Builder::new_multi_thread().enable_all().build()?)
}

fn v(s: &str) -> Value {
    Value::String(s.to_string())
}

fn session(ws: &Workspace) -> live::Session {
    live::Session::new(keymapp::Client::default_for_platform(), Some(ws.root().join("cache/live.json")))
}

type LiveFuture<'a> = std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), keymapp::KeymappError>> + 'a>>;

/// Runs one live action. The change stays on the keyboard until `keyloom live reset`.
fn live_do(ws: &Workspace, f: impl for<'a> FnOnce(&'a mut live::Session) -> LiveFuture<'a>) -> Result<()> {
    let rt = tokio_rt()?;
    let mut s = session(ws);
    let r = rt.block_on(f(&mut s));
    Ok(r?)
}

/// `--show`: best effort. Says why when the keyboard couldn't be reached; returns whether it worked.
fn show_on_keyboard(ws: &Workspace, f: impl for<'a> FnOnce(&'a mut live::Session) -> LiveFuture<'a>) -> bool {
    let Ok(rt) = tokio_rt() else { return false };
    let mut s = session(ws);
    let r = rt.block_on(f(&mut s));
    match r {
        Ok(()) => {
            eprintln!("(shown on the keyboard; `keyloom live reset` restores it)");
            true
        }
        // Something was already changed before it failed: say how to undo it.
        Err(e) if s.changed() => {
            eprintln!("--show failed part-way: {e:#} (`keyloom live reset` restores the keyboard)");
            false
        }
        Err(e) => {
            eprintln!("--show skipped: {e:#}");
            false
        }
    }
}

fn highlight(ws: &Workspace, layout: &Layout, keys: Vec<usize>, layer: Option<i64>) -> Result<bool> {
    let color = ws.config()?.live.highlight_color;
    // The keyboard's layer number for it (a draft's numbers can differ from the firmware's).
    let layer = match layer.map(|l| app::keyboard_layer(ws, layout, l)).transpose() {
        Ok(l) => l,
        Err(e) => {
            eprintln!("--show skipped: {e:#}");
            return Ok(false);
        }
    };
    Ok(show_on_keyboard(ws, |s| Box::pin(async move { s.highlight(&keys, layer, &color).await })))
}

fn run(cmd: Command, out: &Out) -> Result<()> {
    let ws = app::open()?;
    match cmd {
        Command::Import { input, no_keymapp } => import(&ws, &input, !no_keymapp, out),
        Command::List => list(&ws, out),
        Command::Use { layout_id } => {
            ws.set_active(&layout_id)?;
            out.done(format!("Active layout: {layout_id}"), json!({"active": layout_id}))
        }
        Command::Show { reference, layer, all, style, show } => {
            let layout = app::resolve(&ws, reference.as_deref())?;
            let positions: Vec<i64> = if all {
                layout.layers.iter().map(|l| l.position).collect()
            } else {
                vec![match &layer {
                    Some(sel) => ops::resolve_layer_str(&layout, sel)?,
                    None => layout.layers.first().map(|l| l.position).unwrap_or(0),
                }]
            };
            let views: Vec<_> = positions.iter().filter_map(|p| render::layer_view(&layout, *p)).collect();
            if views.is_empty() {
                anyhow::bail!("{} has no layers", layout.reference());
            }
            if show && all {
                eprintln!("--show skipped: it shows one layer; use --layer");
            } else if show {
                match app::keyboard_layer(&ws, &layout, positions[0]) {
                    Ok(n) => {
                        show_on_keyboard(&ws, |s| Box::pin(async move { s.show_layer(n).await }));
                    }
                    Err(e) => eprintln!("--show skipped: {e:#}"),
                }
            }
            if out.json {
                return if all { out.json(&views) } else { out.json(&views[0]) };
            }
            let style = match style {
                StyleArg::Legend => Style::Legend,
                StyleArg::Codes => Style::Codes,
            };
            let opts = AsciiOptions { style, color: out.color, ..Default::default() };
            let text: Vec<String> = views.iter().map(|v| render::ascii(v, &opts)).collect();
            print!("{}", text.join("\n"));
            Ok(())
        }
        Command::Layers { reference, graph } => {
            let layout = app::resolve(&ws, reference.as_deref())?;
            if graph {
                let g = explore::layer_graph(&layout);
                if out.json {
                    return out.json(&g);
                }
                if g.edges.is_empty() {
                    println!("No layer keys.");
                }
                for e in &g.edges {
                    let via = if e.via_transparent { " (via transparent key)" } else { "" };
                    let what = if e.combo { "combo".to_string() } else { format!("{:?}", e.slot).to_lowercase() };
                    println!(
                        "  {} → {}  {}({}) on {} ({what}){via}",
                        layout.layer_title(e.from),
                        layout.layer_title(e.to),
                        e.action,
                        e.to,
                        e.position
                    );
                }
                return Ok(());
            }
            let s = explore::summary(&layout);
            if out.json {
                return out.json(&s);
            }
            println!("{} · {}", layout.title, layout.reference());
            for l in &s.layers {
                let reached = if l.position == 0 {
                    "base layer".to_string()
                } else {
                    l.reached_by.clone().unwrap_or_else(|| "no key leads here".into())
                };
                let exits = if l.exits.is_empty() { String::new() } else { format!("; leave: {}", l.exits.join(", ")) };
                let tags = if l.tags.is_empty() { String::new() } else { format!(" [{}]", l.tags.join(",")) };
                println!(
                    "  {:>2}  {:<12} {:>2} bound, {:>2} transparent   {reached}{exits}{tags}",
                    l.position, l.title, l.bound, l.transparent
                );
            }
            Ok(())
        }
        Command::Summary { reference } => {
            let s = explore::summary(&app::resolve(&ws, reference.as_deref())?);
            if out.json {
                return out.json(&s);
            }
            println!(
                "{} · {} · {} · {} keys · {} layers · {} combos",
                s.title,
                s.reference,
                s.geometry,
                s.key_count,
                s.layers.len(),
                s.combos
            );
            if let Some(c) = &s.created_at {
                println!("created {c}");
            }
            for l in &s.layers {
                let how = l.reached_by.clone().unwrap_or_else(|| if l.position == 0 { "base".into() } else { "unreachable".into() });
                println!("  {:>2} {:<12} {how}", l.position, l.title);
            }
            Ok(())
        }
        Command::Key { key, reference, layer, in_ref, show } => {
            let layout = app::resolve(&ws, reference.or(in_ref).as_deref())?;
            let index = ops::resolve_key(&layout, &v(&key))?;
            let layer = layer.as_deref().map(|l| ops::resolve_layer_str(&layout, l)).transpose()?;
            let info = explore::key_info(&layout, index, layer);
            if show {
                highlight(&ws, &layout, vec![index], layer)?;
            }
            if out.json {
                return out.json(&info);
            }
            println!("{} (#{}){}", info.position, info.index, info.description.as_deref().map(|d| format!(": {d}")).unwrap_or_default());
            if let (Some(f), Some(e)) = (&info.finger, info.effort) {
                println!("  finger {}, effort {e:.2} (0 easy … 1 hard)", format!("{f:?}").to_lowercase());
            }
            for l in &info.layers {
                let resolved = match (&l.resolved_qmk, l.resolved_from) {
                    (Some(q), Some(from)) => format!("  ▽ falls through to {q} on {}", layout.layer_title(from)),
                    _ => String::new(),
                };
                println!("  {:>2} {:<12} {:<32} {}{resolved}", l.layer, l.title, l.qmk, if l.transparent { "" } else { &l.legend });
            }
            Ok(())
        }
        Command::Find { query, reference, in_ref, show } => {
            let layout = app::resolve(&ws, reference.or(in_ref).as_deref())?;
            let hits = explore::find(&layout, &query);
            if show {
                match hits.first() {
                    Some(first) => {
                        let keys: Vec<usize> =
                            hits.iter().filter(|h| h.layer == first.layer && h.slot != "combo").map(|h| h.index).collect();
                        if highlight(&ws, &layout, keys, Some(first.layer))? && hits.iter().any(|h| h.layer != first.layer) {
                            eprintln!("(only the matches on \"{}\" are lit)", first.title);
                        }
                    }
                    None => eprintln!("--show skipped: nothing found"),
                }
            }
            if out.json {
                return out.json(&hits);
            }
            if hits.is_empty() {
                println!("'{query}' isn't on any layer of {}. (`keyloom ref {query}` searches the keycode reference.)", layout.reference());
            }
            for h in &hits {
                println!("  {:<12} {:<18} {:<9} {:<30} {}", h.title, h.position, h.slot, h.qmk, h.reach.as_deref().unwrap_or(""));
            }
            Ok(())
        }
        Command::Free { reference, layer, in_ref, show } => {
            let layout = app::resolve(&ws, reference.or(in_ref).as_deref())?;
            let l = ops::resolve_layer_str(&layout, &layer)?;
            let free = explore::free_keys(&layout, l);
            if show {
                highlight(&ws, &layout, free.iter().map(|f| f.index).collect(), Some(l))?;
            }
            if out.json {
                return out.json(&free);
            }
            println!("{} free keys on \"{}\" (easiest first):", free.len(), layout.layer_title(l));
            for f in &free {
                let falls = f.falls_through_to.as_deref().map(|q| format!("  (now: {q})")).unwrap_or_default();
                let finger = f.finger.map(|x| format!("{x:?}").to_lowercase()).unwrap_or_default();
                println!("  {:<10} {finger:<7} effort {:.2}  {}{falls}", f.position, f.effort.unwrap_or(0.0), f.kind);
            }
            Ok(())
        }
        Command::Ref { query } => {
            let hits = knowledge::search(&query, 20);
            if out.json {
                return out.json(&hits);
            }
            if hits.is_empty() {
                println!("No keycode matches '{query}'. Try fewer or other words (e.g. `keyloom ref volume`).");
            }
            for k in hits {
                let unconfirmed = if k.verified { "" } else { "  (name not confirmed in a real Oryx layout)" };
                println!(
                    "  {:<24} {:<10} {:<9} {}  [{}]{unconfirmed}",
                    k.code,
                    k.legend.as_deref().unwrap_or(""),
                    k.category,
                    k.description,
                    k.aliases.join(", ")
                );
            }
            Ok(())
        }
        Command::Guides { name } => {
            match name {
                Some(n) => {
                    let text = knowledge::guide(&n).with_context(|| {
                        format!("no guide '{n}' (guides: {})", knowledge::guides().iter().map(|(n, _)| *n).collect::<Vec<_>>().join(", "))
                    })?;
                    if out.json {
                        return out.json(&json!({"name": n, "text": text}));
                    }
                    print!("{text}");
                }
                None => {
                    let title =
                        |text: &str| text.lines().find(|l| l.starts_with('#')).unwrap_or("").trim_start_matches('#').trim().to_string();
                    if out.json {
                        return out
                            .json(&knowledge::guides().iter().map(|(n, t)| json!({"name": n, "title": title(t)})).collect::<Vec<_>>());
                    }
                    for (n, text) in knowledge::guides() {
                        println!("  {n:<16} {}", title(text));
                    }
                    println!("Read one with: keyloom guides <name>");
                }
            }
            Ok(())
        }
        Command::Validate { reference } => {
            let layout = app::resolve(&ws, reference.as_deref())?;
            let (report, findings) = app::validation_report(&ws, &layout);
            if out.json {
                out.json(&report)?;
            } else {
                print_findings(&layout, &findings);
            }
            if has_errors(&findings) {
                return Err(CheckFailed.into());
            }
            Ok(())
        }
        Command::Draft(d) => draft_cmd(&ws, d, out),
        Command::Set { draft, layer, key, action, slot } => {
            let op = match slot {
                None => Op::SetKey { layer: v(&layer), key: v(&key), action },
                Some(s) => {
                    let slot = match s {
                        SlotArg::Tap => Slot::Tap,
                        SlotArg::Hold => Slot::Hold,
                        SlotArg::DoubleTap => Slot::DoubleTap,
                        SlotArg::TapHold => Slot::TapHold,
                    };
                    let action = (action != "none").then_some(action);
                    Op::SetSlot { layer: v(&layer), key: v(&key), slot, action }
                }
            };
            edit(&ws, &draft, vec![op], out)
        }
        Command::Clear { draft, layer, key, none } => {
            edit(&ws, &draft, vec![Op::ClearKey { layer: v(&layer), key: v(&key), to: if none { Fill::None } else { Fill::Trans } }], out)
        }
        Command::Layer(l) => {
            let (draft, op) = match l {
                LayerCmd::Add { draft, title, after, copy_from, fill, tags, color } => (
                    draft,
                    Op::AddLayer {
                        title,
                        after: after.as_deref().map(v),
                        copy_from: copy_from.as_deref().map(v),
                        fill: match fill {
                            FillArg::Trans => Fill::Trans,
                            FillArg::None => Fill::None,
                        },
                        tags,
                        color,
                    },
                ),
                LayerCmd::Move { draft, layer, to } => (draft, Op::MoveLayer { layer: v(&layer), to }),
                LayerCmd::Rename { draft, layer, title } => (draft, Op::RenameLayer { layer: v(&layer), title }),
                LayerCmd::Rm { draft, layer, force } => (draft, Op::RemoveLayer { layer: v(&layer), force }),
                LayerCmd::Tag { draft, layer, tags } => (draft, Op::TagLayer { layer: v(&layer), tags }),
                LayerCmd::Color { draft, layer, color } => (draft, Op::ColorLayer { layer: v(&layer), color }),
            };
            edit(&ws, &draft, vec![op], out)
        }
        Command::Combo(c) => {
            let keys = |s: &str| s.split('+').map(|k| v(k.trim())).collect::<Vec<_>>();
            let (draft, op) = match c {
                ComboCmd::Add { draft, layer, keys: k, action, name } => {
                    (draft, Op::AddCombo { layer: v(&layer), keys: keys(&k), action, name })
                }
                ComboCmd::Rm { draft, layer, keys: k } => (draft, Op::RemoveCombo { layer: v(&layer), keys: keys(&k) }),
            };
            edit(&ws, &draft, vec![op], out)
        }
        Command::Apply { draft, file } => {
            let text = if file.as_os_str() == "-" {
                std::io::read_to_string(std::io::stdin())?
            } else {
                std::fs::read_to_string(&file).with_context(|| format!("reading {}", file.display()))?
            };
            let ops: Vec<Op> = serde_json::from_str(&text).context("expected a JSON array of operations (see `keyloom apply --help`)")?;
            edit(&ws, &draft, ops, out)
        }
        Command::Diff { from, to } => {
            let (a, b) = (app::resolve(&ws, Some(&from))?, app::resolve(&ws, Some(&to))?);
            let d = keyloom::diff::diff(&a, &b, Default::default());
            if out.json {
                return out.json(&d);
            }
            println!("{} → {}: {}", d.from, d.to, d.summary);
            for c in &d.changes {
                println!("  {}", describe_change(c));
            }
            Ok(())
        }
        Command::Guide { draft, step } => {
            let (g, d) = app::guide_for(&ws, &draft)?;
            let blockers = app::guide_blockers(&ws, &d);
            if out.json {
                return out.json(&json!({"guide": g, "errors": blockers}));
            }
            print!("{}", app::blockers_text(&d, &blockers));
            if step {
                return guide_step(&ws, &g);
            }
            print!("{}", keyloom::guide::render(&g));
            Ok(())
        }
        Command::Push { draft, to, overwrite, force } => {
            let p = app::push_live(&ws, &draft, force, &app::PushTarget { to, overwrite })?;
            if out.json {
                out.json(&p)?;
            } else {
                print!("{}", app::push_text(&p));
            }
            if !p.verified {
                std::process::exit(3);
            }
            Ok(())
        }
        Command::Verify { draft, against } => {
            let r = app::verify(&ws, &draft, against.as_deref())?;
            let name = draft.trim_start_matches("draft:");
            if out.json {
                out.json(&r)?;
            } else if r.verification.matches {
                println!(
                    "✓ {} matches {}. Archive the draft with: keyloom draft archive {name}",
                    r.verification.against, r.verification.draft
                );
                println!("  For further changes start a new draft: it begins from {}.", r.verification.against);
            } else if let Some(note) = &r.note {
                println!("{note}");
            } else {
                println!("{} doesn't match {} yet.", r.verification.against, r.verification.draft);
                if !r.verification.still_to_do.is_empty() {
                    println!("\nStill to do in Oryx:");
                    for c in &r.verification.still_to_do {
                        println!("  {}", describe_change(c));
                    }
                }
                if !r.verification.unexpected.is_empty() {
                    println!("\nDifferent in Oryx although the draft didn't ask for it (shown as: in Oryx now → in the draft):");
                    for c in &r.verification.unexpected {
                        println!("  {}", describe_change(c));
                    }
                }
            }
            if !r.verification.matches {
                return Err(CheckFailed.into());
            }
            Ok(())
        }
        Command::Status => {
            let rt = tokio_rt()?;
            let st = rt.block_on(live::status(&keymapp::Client::default_for_platform()));
            if out.json {
                return out.json(&st);
            }
            match &st.keymapp {
                Some(k) => {
                    println!("Keymapp {}: API reachable", k.keymapp_version);
                    match &k.keyboard {
                        Some(kb) => println!(
                            "Keyboard: {} (firmware {}), current layer {}",
                            kb.friendly_name, kb.firmware_version, kb.current_layer
                        ),
                        None => {
                            println!("Keyboard: none connected in Keymapp");
                            for k in &st.keyboards {
                                println!(
                                    "  available: {} (id {}){}",
                                    k.friendly_name,
                                    k.id,
                                    if k.is_connected { ", connected" } else { "" }
                                );
                            }
                        }
                    }
                }
                None => println!("Keymapp: {}", st.keymapp_error.as_deref().unwrap_or("not reachable")),
            }
            match (&st.flashed, &st.flashed_source) {
                (Some(f), Some(src)) => {
                    let known = f.split_once('/').and_then(|(l, r)| ws.load_revision(l, r).ok()).is_some();
                    let note = if known { "imported".to_string() } else { format!("not imported yet: keyloom import {f}") };
                    println!("Flashed: {f} (from {src}; {note})");
                }
                _ => println!("Flashed: unknown (no ZSA keyboard with Oryx firmware found over USB)"),
            }
            Ok(())
        }
        Command::Live(l) => match l {
            LiveCmd::Layer { layer, reference, in_ref } => {
                let layout = app::resolve(&ws, reference.or(in_ref).as_deref())?;
                let wanted = ops::resolve_layer_str(&layout, &layer)?;
                let n = app::keyboard_layer(&ws, &layout, wanted)?;
                live_do(&ws, |s| Box::pin(async move { s.show_layer(n).await }))?;
                let hint = if n == 0 { "" } else { " `keyloom live reset` goes back." };
                out.done(format!("Keyboard on layer {n} \"{}\".{hint}", layout.layer_title(wanted)), json!({"layer": n}))
            }
            LiveCmd::Highlight { keys, layer, color, reference } => {
                let layout = app::resolve(&ws, reference.as_deref())?;
                let idx = keys.iter().map(|k| ops::resolve_key(&layout, &v(k))).collect::<Result<Vec<_>, _>>()?;
                let layer = layer.as_deref().map(|l| ops::resolve_layer_str(&layout, l)).transpose()?;
                let layer = layer.map(|l| app::keyboard_layer(&ws, &layout, l)).transpose()?;
                let color = color.unwrap_or(ws.config()?.live.highlight_color);
                live_do(&ws, |s| Box::pin(async move { s.highlight(&idx, layer, &color).await }))?;
                out.done(
                    format!("Lit {} key(s). `keyloom live reset` restores the lights.", keys.len()),
                    json!({"highlighted": keys, "layer": layer}),
                )
            }
            LiveCmd::Reset => {
                let rt = tokio_rt()?;
                let mut s = session(&ws);
                let report = rt.block_on(s.reset(true))?;
                let lights = if report.lights_restored {
                    "Lights restored"
                } else {
                    "Lights: nothing to restore (no keyboard connected in Keymapp)"
                };
                out.done(format!("{lights}. Layer: {}.", report.layer), serde_json::to_value(&report)?)
            }
        },
        Command::Export { reference, output, format } => {
            let layout = app::resolve(&ws, reference.as_deref())?;
            let text = match format {
                FormatArg::Keyloom => keyloom::workspace::pretty(&layout),
                FormatArg::Oryx => serde_json::to_string_pretty(&layout.to_oryx())? + "\n",
            };
            match output {
                Some(path) => {
                    std::fs::write(&path, text).with_context(|| format!("writing {}", path.display()))?;
                    eprintln!("Wrote {} to {}", layout.reference(), path.display());
                }
                None => print!("{text}"),
            }
            Ok(())
        }
        Command::Mcp => unreachable!("handled in main"),
    }
}

enum Input {
    Line(String),
    Eof,
    Interrupt,
}

/// Walks the guide one step at a time with the key lit. Ctrl-C and `q` restore the keyboard.
fn guide_step(ws: &Workspace, g: &keyloom::guide::Guide) -> Result<()> {
    let rt = tokio_rt()?;
    let mut session = session(ws);
    let color = ws.config()?.live.highlight_color;
    let steps: Vec<(&str, &keyloom::guide::Step)> =
        g.sections.iter().flat_map(|s| s.steps.iter().map(move |st| (s.heading.as_str(), st))).collect();
    if steps.is_empty() {
        println!("Nothing to do: {} already has everything in {}.", g.base, g.draft);
        return Ok(());
    }
    for n in &g.notes {
        println!("Note: {n}");
    }
    println!("{} steps. Enter = next, b = back, q = quit.\n", steps.len());
    // stdin is read on its own thread so Ctrl-C can be noticed while waiting for a line.
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    std::thread::spawn(move || {
        for line in std::io::stdin().lines() {
            let Ok(l) = line else { break };
            if tx.send(l).is_err() {
                break;
            }
        }
    });
    let mut i = 0;
    let mut live_ok = true;
    let mut interrupted = false;
    while i < steps.len() {
        let (heading, st) = steps[i];
        println!("[{}/{}] {heading}: {} {}", i + 1, steps.len(), st.position.as_deref().unwrap_or(""), st.what);
        if let Some(l) = &st.location {
            println!("        {l}");
        }
        if let Some(h) = &st.oryx_hint {
            println!("        {h}");
        }
        // Only the lights: the draft's layer numbers aren't the flashed firmware's, so the
        // keyboard's layer is left alone.
        let lit: Vec<usize> = st.index.into_iter().chain(st.keys.iter().copied()).collect();
        if !lit.is_empty() && live_ok {
            if let Err(e) = rt.block_on(session.highlight(&lit, None, &color)) {
                eprintln!("        (keyboard not lit: {e})");
                live_ok = false;
            }
        }
        let input = rt.block_on(async {
            tokio::select! {
                l = rx.recv() => l.map(Input::Line).unwrap_or(Input::Eof),
                _ = tokio::signal::ctrl_c() => Input::Interrupt,
            }
        });
        match input {
            Input::Line(l) => match l.trim() {
                "q" => break,
                "b" => i = i.saturating_sub(1),
                _ => i += 1,
            },
            Input::Eof => break,
            Input::Interrupt => {
                interrupted = true;
                break;
            }
        }
    }
    if let Err(e) = rt.block_on(session.reset(false)) {
        eprintln!("(couldn't restore the keyboard: {e}; try `keyloom live reset`)");
    }
    if interrupted {
        std::process::exit(130);
    }
    if i >= steps.len() {
        for f in &g.finally {
            println!("→ {f}");
        }
    } else {
        println!("Stopped at step {} of {}. `keyloom guide <draft>` prints the whole list.", i + 1, steps.len());
    }
    Ok(())
}

fn describe_change(c: &Change) -> String {
    let show = |x: &Option<String>| x.clone().unwrap_or_else(|| "—".into());
    match c {
        Change::LayerAdded { position, title, .. } => format!("+ layer {position} \"{title}\""),
        Change::LayerRemoved { position, title } => format!("- layer {position} \"{title}\""),
        Change::LayerMoved { title, from, to, .. } => format!("~ layer \"{title}\" moved {from} → {to}"),
        Change::LayerChanged { title, field, from, to, .. } => format!("~ layer \"{title}\" {field}: {} → {}", show(from), show(to)),
        Change::KeyChanged { layer_title, position, slot, from, to, .. } => {
            format!("~ {layer_title:<10} {position:<10} {slot:<9} {} → {}", show(from), show(to))
        }
        Change::ComboAdded { layer_title, keys, trigger, .. } => format!("+ combo {} on \"{layer_title}\" → {trigger}", keys.join("+")),
        Change::ComboRemoved { layer_title, keys, trigger, .. } => format!("- combo {} on \"{layer_title}\" ({trigger})", keys.join("+")),
        Change::ComboChanged { layer_title, keys, from, to, .. } => {
            format!("~ combo {} on \"{layer_title}\": {from} → {to}", keys.join("+"))
        }
    }
}

/// Prints changes, at most `limit` lines; `unlisted` more exist beyond `changes`.
fn print_changes(changes: &[Change], limit: usize, unlisted: usize, more_hint: &str) {
    for c in changes.iter().take(limit) {
        println!("  {}", describe_change(c));
    }
    let more = changes.len().saturating_sub(limit) + unlisted;
    if more > 0 {
        println!("  … and {more} more ({more_hint})");
    }
}

fn print_findings(layout: &Layout, findings: &[Finding]) {
    if findings.is_empty() {
        println!("✓ {}: no problems found", layout.reference());
        return;
    }
    for f in findings {
        let sev = match f.severity {
            Severity::Error => "error",
            Severity::Warn => "warn ",
            Severity::Info => "info ",
        };
        let at = match (f.layer, &f.key) {
            (Some(l), Some(k)) => format!("{} {k}: ", layout.layer_title(l)),
            (Some(l), None) => format!("{}: ", layout.layer_title(l)),
            (None, Some(k)) => format!("{k}: "),
            _ => String::new(),
        };
        println!("{sev} [{}] {at}{}", f.rule, f.message);
        println!("       → {}", f.hint);
    }
}

fn print_edit(ws: &Workspace, draft: &str, r: &app::EditResult) -> Result<()> {
    let name = draft.trim_start_matches("draft:");
    println!("{}: {}", r.draft, r.diff.summary);
    print_changes(&r.diff.changes, 12, r.more_changes, &format!("`keyloom draft show {name}` lists all"));
    for w in &r.warnings {
        println!("warn: {w}");
    }
    // Errors always; warnings only when this edit introduced them.
    let is_new = |f: &Finding| r.new_findings.iter().any(|n| n.rule == f.rule && n.key == f.key && n.layer == f.layer);
    let show: Vec<Finding> = r.findings.iter().filter(|f| f.severity == Severity::Error || is_new(f)).cloned().collect();
    if !show.is_empty() {
        println!();
        print_findings(&ws.load_draft(draft)?, &show);
    }
    let other = r.findings.len() - show.len();
    if other > 0 {
        println!("({other} earlier warning(s); `keyloom validate {name}`)");
    }
    Ok(())
}

fn edit(ws: &Workspace, draft: &str, ops: Vec<Op>, out: &Out) -> Result<()> {
    let r = app::edit(ws, draft, &ops)?;
    if out.json {
        return out.json(&r);
    }
    print_edit(ws, draft, &r)
}

fn draft_cmd(ws: &Workspace, cmd: DraftCmd, out: &Out) -> Result<()> {
    match cmd {
        DraftCmd::New { name, from } => {
            let base = app::resolve(ws, from.as_deref())?;
            let d = ws.create_draft(&name, &base)?;
            out.done(
                format!(
                    "Created {} from {}. Edit it with `keyloom set {name} <layer> <key> <action>`, `keyloom layer add {name} <title>`, or an agent via `keyloom mcp`.",
                    d.reference(),
                    base.reference()
                ),
                json!({"draft": d.reference(), "basedOn": base.reference()}),
            )
        }
        DraftCmd::List => {
            let drafts = ws.drafts()?;
            if out.json {
                return out.json(&drafts);
            }
            if drafts.is_empty() {
                println!("No drafts. Create one: keyloom draft new <name>");
            }
            for d in drafts {
                println!("  draft:{:<16} {}@{}  {} edit(s), updated {}", d.name, d.layout_id, d.based_on, d.ops, d.updated);
            }
            Ok(())
        }
        DraftCmd::Show { name, layer } => {
            let d = ws.load_draft(&name)?;
            let base = app::base_of(ws, &d)?;
            let dd = keyloom::diff::diff(&base, &d, Default::default());
            let l = match layer {
                Some(s) => ops::resolve_layer_str(&d, &s)?,
                None => {
                    dd.changes.iter().find_map(|c| if let Change::KeyChanged { layer, .. } = c { Some(*layer) } else { None }).unwrap_or(0)
                }
            };
            if out.json {
                return out.json(&json!({"diff": dd, "layer": render::layer_view(&d, l)}));
            }
            if let Some(view) = render::layer_view(&d, l) {
                print!("{}", render::ascii(&view, &AsciiOptions { color: out.color, ..Default::default() }));
            }
            println!("\nChanges from {}: {}", dd.from, dd.summary);
            print_changes(&dd.changes, usize::MAX, 0, "");
            Ok(())
        }
        DraftCmd::Undo { name } => {
            let r = app::undo(ws, &name)?;
            if out.json {
                return out.json(&r);
            }
            println!("Undid the last edit of {}. Reverted: {}", r.draft, r.diff.summary);
            print_changes(&r.diff.changes, 12, r.more_changes, "`keyloom draft show` lists the draft's changes");
            Ok(())
        }
        DraftCmd::Archive { name } => {
            let p = ws.archive_draft(&name)?;
            out.done(format!("Archived draft {name} to {}", p.display()), json!({"archived": name, "path": p}))
        }
        DraftCmd::Rm { name } => {
            ws.remove_draft(&name)?;
            out.done(format!("Removed draft {name}."), json!({"removed": name}))
        }
    }
}

fn import(ws: &Workspace, input: &str, keymapp_fallback: bool, out: &Out) -> Result<()> {
    let results = app::import(ws, input, keymapp_fallback)?;
    if out.json {
        let items: Vec<_> = results.iter().map(|(l, s)| app::import_summary(l, s)).collect();
        return out.json(&items);
    }
    if results.is_empty() {
        println!("Nothing to import.");
    }
    for (l, s) in &results {
        let what = if *s == Stored::New { "Imported" } else { "Already have" };
        let created = l.created_at.as_deref().map(|c| format!(", {c}")).unwrap_or_default();
        let titles: Vec<_> = l.layers.iter().map(|x| x.title.as_str()).collect();
        let auto = l.settings.auto_mouse_layer.map(|a| format!("; auto-mouse layer {a}")).unwrap_or_default();
        println!(
            "{what} {} \"{}\" from {}{created}: {} layers ({}){auto}",
            l.reference(),
            l.title,
            l.source.kind,
            l.layers.len(),
            titles.join(", ")
        );
    }
    Ok(())
}

fn list(ws: &Workspace, out: &Out) -> Result<()> {
    let layouts = ws.layouts()?;
    let active = ws.config()?.active_layout;
    let drafts = ws.drafts()?;
    if out.json {
        return out.json(&app::list_summary(ws)?);
    }
    if layouts.is_empty() {
        println!("No layouts yet. Import one: keyloom import <oryx link>");
        println!("(workspace: {})", ws.root().display());
        return Ok(());
    }
    for m in &layouts {
        let mark = if active.as_deref() == Some(m.layout_id.as_str()) { "*" } else { " " };
        println!("{mark} {} \"{}\" ({})", m.layout_id, m.title, m.geometry);
        let newest = m.newest().map(|r| r.id.clone());
        let mut revs = m.revisions.clone();
        revs.sort_by_key(|r| std::cmp::Reverse(r.sort_time()));
        for r in revs {
            let tag = if Some(&r.id) == newest.as_ref() { " (newest)" } else { "" };
            println!("    @{}  {}  from {}{tag}", r.id, r.created_at.as_deref().unwrap_or("unknown date"), r.source);
        }
        for d in drafts.iter().filter(|d| d.layout_id == m.layout_id) {
            println!("    draft:{}  based on @{}, {} edit(s)", d.name, d.based_on, d.ops);
        }
    }
    println!("(* = active layout; workspace: {})", ws.root().display());
    Ok(())
}
