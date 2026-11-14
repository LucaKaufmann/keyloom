//! End to end through the real binary: CLI commands and an MCP session over stdio.
//! No network: layouts come from the engine's fixtures.

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_keyloom")
}

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../keyloom/tests/fixtures/oryx-9qMJK-ZPjbXz.json")
}

fn keyloom(home: &std::path::Path, args: &[&str]) -> (i32, String, String) {
    // No Keymapp in tests: point the client at a socket that doesn't exist.
    let o = Command::new(bin())
        .args(args)
        .env("KEYLOOM_HOME", home)
        .env("NO_COLOR", "1")
        .env("KEYMAPP_SOCKET", home.join("no-keymapp.sock"))
        .env("KEYLOOM_USB_SERIALS", "")
        .env("KEYLOOM_ORYX_TOKEN", "")
        .env_remove("KEYMAPP_PORT")
        .output()
        .unwrap();
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into(), String::from_utf8_lossy(&o.stderr).into())
}

#[test]
fn cli_flow() {
    let home = tempfile::tempdir().unwrap();
    let h = home.path();
    let (code, out, err) = keyloom(h, &["import", fixture().to_str().unwrap()]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("Imported 9qMJK@ZPjbXz"), "{out}");
    assert!(keyloom(h, &["show", "-l", "sym"]).1.contains("layer 1 \"Sym+Num\""));
    assert!(keyloom(h, &["find", "Esc"]).1.contains("hold L.thumb.2 (MO(1))"));
    assert_eq!(keyloom(h, &["validate"]).0, 0, "warnings only");
    assert_eq!(keyloom(h, &["draft", "new", "wow"]).0, 0);
    assert_eq!(keyloom(h, &["set", "wow", "Main", "L.thumb.1", "TG(Gaming)"]).0, 0);
    let (_, out, _) = keyloom(h, &["set", "wow", "Gaming", "L.thumb.1", "KC_LALT"]);
    assert!(out.contains("layer-no-exit"), "errors are shown right after the edit: {out}");
    assert_eq!(keyloom(h, &["validate", "draft:wow"]).0, 3, "exit code 3 on validation errors");
    let ops = h.join("ops.json");
    std::fs::write(&ops, r#"[{"op":"set_key","layer":"Gaming","key":"L.thumb.1","action":"TG(Gaming)"},{"op":"tag_layer","layer":"Gaming","tags":["gaming"]}]"#).unwrap();
    assert_eq!(keyloom(h, &["apply", "wow", ops.to_str().unwrap()]).0, 0);
    let (code, out, _) = keyloom(h, &["validate", "draft:wow"]);
    assert_eq!(code, 0, "{out}");
    let (_, guide, _) = keyloom(h, &["guide", "wow"]);
    assert!(guide.contains("Tap: KC_LEFT_GUI → TG(3 Gaming)") && guide.contains("Oryx: Layer → Toggle (TG)"), "{guide}");
    let (code, json, _) = keyloom(h, &["--json", "diff", "9qMJK", "draft:wow"]);
    assert_eq!(code, 0);
    let d: Value = serde_json::from_str(&json).unwrap();
    let key_change = d["changes"].as_array().unwrap().iter().find(|c| c["change"] == "key_changed").unwrap();
    assert_eq!(key_change["layerTitle"], "Main", "fields are camelCase");
    assert_eq!(keyloom(h, &["verify", "wow"]).0, 3, "not applied yet");
    assert_eq!(keyloom(h, &["draft", "undo", "wow"]).0, 0);
    let (_, list, _) = keyloom(h, &["list"]);
    assert!(list.contains("draft:wow") && list.contains("2 edit(s)"), "{list}");
    let (_, exported, _) = keyloom(h, &["export", "--format", "oryx"]);
    assert!(serde_json::from_str::<Value>(&exported).unwrap()["revision"]["layers"].as_array().unwrap().len() == 5);
}

struct Mcp {
    child: std::process::Child,
    out: BufReader<std::process::ChildStdout>,
    id: i64,
}

impl Mcp {
    fn start(home: &std::path::Path) -> Self {
        let mut child = Command::new(bin())
            .arg("mcp")
            .env("KEYLOOM_HOME", home)
            .env("KEYMAPP_SOCKET", home.join("no-keymapp.sock"))
            .env("KEYLOOM_USB_SERIALS", "")
            .env_remove("KEYMAPP_PORT")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let out = BufReader::new(child.stdout.take().unwrap());
        let mut m = Self { child, out, id: 0 };
        let init = m.call(
            "initialize",
            json!({"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "test", "version": "0"}}),
        );
        assert_eq!(init["serverInfo"]["name"], "keyloom");
        assert!(init["instructions"].as_str().unwrap().contains("L.home.4"));
        m.send(json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
        m
    }
    fn send(&mut self, v: Value) {
        let stdin = self.child.stdin.as_mut().unwrap();
        writeln!(stdin, "{v}").unwrap();
        stdin.flush().unwrap();
    }
    fn call(&mut self, method: &str, params: Value) -> Value {
        self.id += 1;
        self.send(json!({"jsonrpc": "2.0", "id": self.id, "method": method, "params": params}));
        loop {
            let mut line = String::new();
            assert!(self.out.read_line(&mut line).unwrap() > 0, "server closed");
            let v: Value = serde_json::from_str(&line).unwrap();
            if v["id"] == self.id {
                return v["result"].clone();
            }
        }
    }
    /// (is_error, first text block)
    fn tool(&mut self, name: &str, args: Value) -> (bool, String) {
        let r = self.call("tools/call", json!({"name": name, "arguments": args}));
        (r["isError"] == true, r["content"][0]["text"].as_str().unwrap_or_default().to_string())
    }
}

impl Drop for Mcp {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

#[test]
fn mcp_session() {
    let home = tempfile::tempdir().unwrap();
    assert_eq!(keyloom(home.path(), &["import", fixture().to_str().unwrap()]).0, 0);
    let mut m = Mcp::start(home.path());

    let tools: Vec<String> =
        m.call("tools/list", json!({}))["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap().to_string()).collect();
    for t in [
        "layout_summary",
        "render_layer",
        "key_info",
        "find_keycode",
        "free_keys",
        "validate",
        "diff",
        "lookup_keycode",
        "create_draft",
        "edit_draft",
        "undo",
        "apply_guide",
        "verify_draft",
        "keyboard",
        "list_layouts",
        "import_layout",
    ] {
        assert!(tools.contains(&t.to_string()), "missing tool {t}");
    }
    assert_eq!(tools.len(), 16, "{tools:?}");
    let prompts = m.call("prompts/list", json!({}));
    assert!(prompts["prompts"].as_array().unwrap().iter().any(|p| p["name"] == "add_gaming_layer"));
    let res = m.call("resources/read", json!({"uri": "keyloom://reference/voyager"}));
    assert!(res["contents"][0]["text"].as_str().unwrap().contains("L.home.4"));

    let (err, summary) = m.tool("layout_summary", json!({}));
    assert!(!err && summary.contains("\"Gaming\""), "{summary}");
    let (err, free) = m.tool("free_keys", json!({"layer": "Gaming"}));
    assert!(!err && free.contains("L.home.4"));
    assert!(!m.tool("create_draft", json!({"name": "wow"})).0);
    let (err, text) = m.tool(
        "edit_draft",
        json!({"draft": "wow", "ops": [
            {"op": "rename_layer", "layer": "Gaming", "title": "WoW"},
            {"op": "tag_layer", "layer": "WoW", "tags": ["gaming"]},
            {"op": "set_key", "layer": "Main", "key": "L.thumb.1", "action": "TG(WoW)"},
            {"op": "set_key", "layer": "WoW", "key": "L.thumb.1", "action": "TG(WoW)"},
            {"op": "set_key", "layer": "WoW", "key": "L.home.1", "action": "A"}
        ]}),
    );
    assert!(!err, "{text}");
    let r: Value = serde_json::from_str(&text).unwrap();
    assert!(r["findings"].as_array().unwrap().iter().all(|f| f["severity"] != "error"), "{text}");
    let (err, text) =
        m.tool("edit_draft", json!({"draft": "wow", "ops": [{"op": "set_key", "layer": "WoW", "key": "L.home.2", "action": "OSM(LSFT)"}]}));
    assert!(err && text.contains("isn't supported yet"), "{text}");
    let (err, guide) = m.tool("apply_guide", json!({"draft": "wow"}));
    assert!(!err && guide.contains("TG(3 WoW)"), "{guide}");
    let (err, text) = m.tool("render_layer", json!({"layout": "draft:wow", "layer": "WoW"}));
    assert!(!err && text.contains("layer 3 \"WoW\" [gaming]"), "{text}");
    // Revisions stay read-only through MCP too.
    let (err, text) =
        m.tool("edit_draft", json!({"draft": "9qMJK", "ops": [{"op": "set_key", "layer": "Main", "key": "L.home.1", "action": "B"}]}));
    assert!(err, "{text}");
    // The layer graph comes with the summary on request.
    let (err, text) = m.tool("layout_summary", json!({"graph": true}));
    assert!(!err && text.contains("\"graph\"") && text.contains("\"edges\""), "{text}");
}

/// Sets up a workspace with the fixture and a draft `wow` that adds a reachable gaming layer.
fn workspace_with_draft() -> tempfile::TempDir {
    let home = tempfile::tempdir().unwrap();
    let h = home.path();
    assert_eq!(keyloom(h, &["import", fixture().to_str().unwrap()]).0, 0);
    assert_eq!(keyloom(h, &["draft", "new", "wow"]).0, 0);
    for args in [
        vec!["layer", "add", "wow", "WoW", "--tag", "gaming", "--fill", "none"],
        vec!["set", "wow", "Main", "L.thumb.1", "TG(WoW)"],
        vec!["set", "wow", "WoW", "L.thumb.1", "TG(WoW)"],
        vec!["set", "wow", "WoW", "L.home.2", "A"],
    ] {
        let (code, out, err) = keyloom(h, &args);
        assert_eq!(code, 0, "{args:?}: {out}{err}");
    }
    home
}

#[test]
fn references_and_arguments_are_consistent() {
    let home = workspace_with_draft();
    let h = home.path();
    // A draft works bare, with `draft:`, positionally and with --in, in explore commands too.
    for args in [
        vec!["show", "wow", "-l", "WoW"],
        vec!["show", "draft:wow", "-l", "5"],
        vec!["key", "L.home.2", "wow"],
        vec!["key", "L.home.2", "--in", "draft:wow"],
        vec!["find", "KC_A", "wow"],
        vec!["free", "wow", "-l", "WoW"],
        vec!["layers", "wow"],
        vec!["validate", "wow"],
        vec!["key", "13"],
    ] {
        let (code, out, err) = keyloom(h, &args);
        assert_eq!(code, 0, "{args:?}: {out}{err}");
    }
    let (code, _, err) = keyloom(h, &["show", "nope"]);
    assert!(code == 1 && err.contains("no layout or draft called 'nope'"), "{err}");
    // A layout file works as a reference.
    let file = h.join("x.json");
    assert_eq!(keyloom(h, &["export", "wow", "-o", file.to_str().unwrap()]).0, 0);
    assert!(keyloom(h, &["layers", file.to_str().unwrap()]).1.contains("WoW"));
    // Errors say what is wrong.
    let (code, _, err) = keyloom(h, &["set", "wow", "WoW", "L.home.3", "MO"]);
    assert!(code == 1 && err.contains("needs") && err.contains("MO(<layer>)"), "{err}");
    let (_, _, err) = keyloom(h, &["set", "wow", "WoW", "L.nope.3", "A"]);
    assert!(err.contains("unknown key 'L.nope.3'"), "{err}");
    let (_, _, err) = keyloom(h, &["set", "wow", "WoW", "L.home.3", "blah"]);
    assert!(err.contains("unknown keycode 'blah'"), "{err}");
    let (_, _, err) = keyloom(h, &["draft", "new", "wow"]);
    assert!(err.contains("keyloom: a draft called 'wow' already exists"), "no workspace path in front: {err}");
    // Every subcommand documents itself.
    for sub in
        [vec!["draft", "--help"], vec!["layer", "add", "--help"], vec!["combo", "--help"], vec!["apply", "--help"], vec!["live", "--help"]]
    {
        let (_, out, _) = keyloom(h, &sub);
        assert!(out.lines().count() > 6, "{sub:?}: {out}");
    }
    assert!(keyloom(h, &["apply", "--help"]).1.contains("add_layer{title"));
}

#[test]
fn json_output_is_json_for_every_command() {
    let home = workspace_with_draft();
    let h = home.path();
    for args in [
        vec!["--json", "use", "9qMJK"],
        vec!["--json", "list"],
        vec!["--json", "guides"],
        vec!["--json", "guides", "gaming"],
        vec!["--json", "ref", "esc"],
        vec!["--json", "layers", "--graph"],
        vec!["--json", "draft", "show", "wow"],
        vec!["--json", "guide", "wow"],
        vec!["--json", "draft", "undo", "wow"],
        vec!["--json", "status"],
        vec!["--json", "draft", "archive", "wow"],
    ] {
        let (code, out, err) = keyloom(h, &args);
        assert_eq!(code, 0, "{args:?}: {err}");
        serde_json::from_str::<Value>(&out).unwrap_or_else(|e| panic!("{args:?} printed non-JSON ({e}): {out}"));
    }
}

#[test]
fn new_layer_guide_and_verify_success() {
    let home = workspace_with_draft();
    let h = home.path();
    let (_, guide, _) = keyloom(h, &["guide", "wow"]);
    assert!(guide.contains("Add a layer \"WoW\""), "{guide}");
    assert!(
        guide.contains("Layer 5 \"WoW\"") && guide.contains("→ KC_A") && guide.contains("Set the other 50 keys to None (KC_NO)"),
        "the new layer's keys are listed: {guide}"
    );
    let (code, out, _) = keyloom(h, &["verify", "wow"]);
    assert!(code == 3 && out.contains("Nothing new was imported"), "{out}");
    // "Apply in Oryx": the draft, exported in Oryx's shape, comes back as a new revision.
    let (_, exported, _) = keyloom(h, &["export", "wow", "--format", "oryx"]);
    let mut oryx: Value = serde_json::from_str(&exported).unwrap();
    // An export of a draft says so and can't be imported as if Oryx had made it.
    let draft_export = h.join("draft-export.json");
    std::fs::write(&draft_export, &exported).unwrap();
    let (code, _, err) = keyloom(h, &["import", draft_export.to_str().unwrap()]);
    assert!(code == 1 && err.contains("export of the keyloom draft 'wow'"), "{err}");
    assert_eq!(keyloom(h, &["verify", "wow"]).0, 3, "still nothing to verify against");
    oryx["revision"].as_object_mut().unwrap().remove("keyloomDraft");
    oryx["revision"]["hashId"] = json!("FLASHED");
    oryx["revision"]["createdAt"] = json!("2030-01-01 00:00:00 UTC");
    let file = h.join("flashed.json");
    std::fs::write(&file, oryx.to_string()).unwrap();
    assert_eq!(keyloom(h, &["import", file.to_str().unwrap()]).0, 0);
    let (code, out, err) = keyloom(h, &["verify", "wow"]);
    assert!(code == 0 && out.contains("matches draft:wow"), "{out}{err}");
    let (code, json, _) = keyloom(h, &["--json", "verify", "wow"]);
    assert!(code == 0 && serde_json::from_str::<Value>(&json).unwrap()["matches"] == true);
}

#[test]
fn oryx_export_of_a_draft_is_an_oryx_layout() {
    let home = workspace_with_draft();
    let h = home.path();
    let (code, out, _) = keyloom(h, &["export", "wow", "--format", "oryx"]);
    assert_eq!(code, 0);
    let v: Value = serde_json::from_str(&out).unwrap();
    // Oryx's shape: hashId, title, revision.hashId, layers with keys.
    assert!(v["hashId"].is_string() && v["revision"]["hashId"].is_string() && v["title"].is_string());
    assert_eq!(v["revision"]["layers"].as_array().unwrap().len(), 6);
    assert_eq!(v["revision"]["layers"][5]["keys"].as_array().unwrap().len(), 52);
    assert!(v["revision"]["layers"][5].get("keyloom").is_none(), "keyloom-only metadata stays out");
}

#[test]
fn live_commands_fail_properly_without_keymapp() {
    let home = workspace_with_draft();
    let h = home.path();
    for args in [vec!["live", "layer", "1"], vec!["live", "highlight", "L.home.4"], vec!["live", "reset"]] {
        let (code, out, err) = keyloom(h, &args);
        assert!(
            code == 1 && out.is_empty() && err.contains("Keymapp's API isn't reachable"),
            "{args:?}: code {code} out {out:?} err {err}"
        );
    }
    assert!(keyloom(h, &["live", "highlight", "L.home.4", "--color", "ééé"]).2.contains("Keymapp"), "no panic on odd colors");
    // --show degrades: the command still works and doesn't claim the keyboard lit up.
    let (code, out, err) = keyloom(h, &["find", "Esc", "--show"]);
    assert_eq!(code, 0);
    assert!(out.contains("L.num.0") && err.contains("--show skipped") && !err.contains("shown on the keyboard"), "{err}");
    let (code, out, _) = keyloom(h, &["status"]);
    assert!(code == 0 && out.contains("Keymapp:") && out.contains("Flashed:"), "{out}");
}

#[test]
fn concurrent_edits_are_all_kept() {
    let home = workspace_with_draft();
    let h = home.path().to_path_buf();
    let keys = ["L.top.1", "L.top.2", "L.top.3", "L.top.4", "L.bot.1", "L.bot.2", "L.bot.3", "L.bot.4"];
    let handles: Vec<_> = keys
        .iter()
        .map(|k| {
            let (h, k) = (h.clone(), k.to_string());
            std::thread::spawn(move || keyloom(&h, &["set", "wow", "WoW", &k, "KC_F"]))
        })
        .collect();
    for t in handles {
        let (code, out, err) = t.join().unwrap();
        assert_eq!(code, 0, "{out}{err}");
    }
    let (_, json, _) = keyloom(&h, &["--json", "find", "KC_F", "wow"]);
    let hits: Vec<Value> = serde_json::from_str(&json).unwrap();
    assert_eq!(hits.iter().filter(|x| x["title"] == "WoW").count(), keys.len(), "every edit survived");
    let (_, list, _) = keyloom(&h, &["--json", "draft", "list"]);
    assert_eq!(serde_json::from_str::<Value>(&list).unwrap()[0]["ops"], 4 + keys.len(), "the op log matches the draft");
    assert!(std::fs::read_dir(h.join("layouts/9qMJK/drafts")).unwrap().all(|e| !e.unwrap().file_name().to_string_lossy().contains(".tmp")));
}

#[test]
fn mcp_agent_adds_a_layer() {
    let home = tempfile::tempdir().unwrap();
    let mut m = Mcp::start(home.path());
    // Nothing imported yet: the agent can import by itself.
    let (err, text) = m.tool("layout_summary", json!({}));
    assert!(err && text.contains("no layouts yet"), "{text}");
    let (err, text) = m.tool("import_layout", json!({"source": fixture().to_str().unwrap()}));
    assert!(!err && text.contains("9qMJK@ZPjbXz"), "{text}");
    assert!(!m.tool("create_draft", json!({"name": "wow"})).0);
    // Layers and keys as numbers, the way an agent echoes them back.
    let edit = |m: &mut Mcp, op: Value| m.tool("edit_draft", json!({"draft": "wow", "ops": [op]}));
    let (err, text) = edit(&mut m, json!({"op": "add_layer", "title": "WoW", "after": 3, "fill": "none", "tags": ["Gaming"]}));
    assert!(!err, "{text}");
    let (err, text) = edit(&mut m, json!({"op": "add_layer", "title": "X", "fill": "transparent"}));
    assert!(err && text.contains("unknown variant"), "an unknown fill is rejected, not silently treated as trans: {text}");
    let (err, text) = edit(&mut m, json!({"op": "set_key", "layer": 4, "key": 14, "action": "KC_S"}));
    assert!(!err, "{text}");
    let (err, text) = m.tool(
        "edit_draft",
        json!({"draft": "wow", "ops": [
            {"op": "set_key", "layer": 0, "key": "L.thumb.1", "action": "TG(WoW)"},
            {"op": "set_key", "layer": "WoW", "key": "L.thumb.1", "action": "TG(WoW)"}
        ]}),
    );
    assert!(!err, "{text}");
    let r: Value = serde_json::from_str(&text).unwrap();
    assert!(r["findings"].as_array().unwrap().iter().all(|f| f["severity"] != "info"), "info-level notes aren't repeated on every edit");
    let (err, text) = m.tool("key_info", json!({"layout": "wow", "key": 14, "layer": 4}));
    assert!(!err && text.contains("KC_S"), "{text}");
    let (err, text) = m.tool("validate", json!({"layout": "wow"}));
    assert!(!err && serde_json::from_str::<Value>(&text).unwrap()["errors"] == 0, "{text}");
    let (err, guide) = m.tool("apply_guide", json!({"draft": "wow"}));
    assert!(
        !err && guide.contains("Add a layer \"WoW\"") && guide.contains("right after \"Gaming\"") && guide.contains("→ KC_S"),
        "{guide}"
    );
    let (err, text) = m.tool("verify_draft", json!({"draft": "wow"}));
    assert!(!err && text.contains("Nothing new was imported") && text.contains("import_layout"), "{text}");
    let (err, text) = m.tool("undo", json!({"draft": "wow"}));
    assert!(!err && text.contains("\"diff\"") && text.contains("TG(4 WoW)"), "undo shows what was reverted: {text}");
    // Live tools fail clearly without Keymapp; reset doesn't claim success.
    let (err, text) = m.tool("keyboard", json!({"action": "reset"}));
    assert!(err && text.contains("Keymapp's API isn't reachable"), "{text}");
    let (err, text) = m.tool("keyboard", json!({"action": "highlight", "keys": ["L.home.4"]}));
    assert!(err && text.contains("Keymapp"), "{text}");
    let (err, text) = m.tool("keyboard", json!({"action": "highlight"}));
    assert!(err && text.contains("needs `keys`"), "{text}");
    let (err, text) = m.tool("keyboard", json!({"action": "status"}));
    assert!(!err && text.contains("keymappError"), "status reports an unreachable Keymapp instead of failing: {text}");
    // Resources carry their real type.
    let res = m.call("resources/read", json!({"uri": "keyloom://guides/gaming"}));
    assert_eq!(res["contents"][0]["mimeType"], "text/markdown");
    // Schemas: layers accept numbers, and parameters are described.
    let tools = m.call("tools/list", json!({}));
    let tool = |name: &str| tools["tools"].as_array().unwrap().iter().find(|t| t["name"] == name).unwrap().clone();
    let schema = tool("free_keys")["inputSchema"].to_string();
    assert!(schema.contains("integer") && schema.contains("Layer number or title"), "{schema}");
    let schema = tool("keyboard")["inputSchema"].to_string();
    assert!(["status", "show_layer", "highlight", "reset"].iter().all(|a| schema.contains(a)), "{schema}");
}

#[test]
fn round_two_cli_fixes() {
    let home = workspace_with_draft();
    let h = home.path();
    // Op files: unknown or missing fields are errors, never a different edit.
    let apply = |ops: &str| {
        let f = h.join("ops.json");
        std::fs::write(&f, ops).unwrap();
        keyloom(h, &["apply", "wow", f.to_str().unwrap()])
    };
    let (code, _, err) = apply(r#"[{"op":"set_slot","layer":0,"key":"L.thumb.2","slot":"hold","value":"MO(2)"}]"#);
    assert!(code == 1 && err.contains("unknown field `value`"), "{err}");
    let (code, _, err) = apply(r#"[{"op":"set_slot","layer":0,"key":"L.thumb.2","slot":"hold"}]"#);
    assert!(code == 1 && err.contains("missing field `action`"), "{err}");
    let (code, _, err) = apply(r#"[{"op":"add_layer","title":"X","copyFrom":"Gaming"}]"#);
    assert!(code == 1 && err.contains("copyFrom"), "{err}");
    let (code, out, err) = apply(r#"[{"op":"clear_key","layer":0,"key":"L.num.1","fill":"none"}]"#);
    assert!(code == 0 && out.contains("→ KC_NO"), "`fill` works as another name for `to`: {out}{err}");
    assert!(keyloom(h, &["key", "L.thumb.2", "wow", "-l", "0"]).1.contains("LT(1, KC_SPACE)"), "the failed batches changed nothing");
    // Colors are checked.
    let (code, _, err) = keyloom(h, &["layer", "color", "wow", "WoW", "red"]);
    assert!(code == 1 && err.contains("#rrggbb"), "{err}");
    assert_eq!(keyloom(h, &["layer", "color", "wow", "WoW", "#FF8800"]).0, 0);
    // `-` is the minus key, also in a slot; `none` empties a slot.
    let (code, out, err) = keyloom(h, &["set", "wow", "WoW", "L.home.3", "-", "--slot", "tap"]);
    assert!(code == 0 && out.contains("KC_MINUS"), "{out}{err}");
    assert!(keyloom(h, &["set", "wow", "WoW", "L.home.3", "none", "--slot", "tap"]).1.contains("KC_MINUS → —"));
    // Meh is a hold action only, whichever way it is written.
    let (code, _, err) = keyloom(h, &["set", "wow", "WoW", "L.home.3", "Hyper", "--slot", "tap"]);
    assert!(code == 1 && err.contains("hold action"), "{err}");
    assert_eq!(keyloom(h, &["set", "wow", "WoW", "L.home.3", "Hyper", "--slot", "hold"]).0, 0);
    // The `auto` tag is known.
    let (_, out, _) = keyloom(h, &["layer", "tag", "wow", "Mouse", "mouse", "auto"]);
    assert!(!out.contains("has no meaning"), "{out}");
    // validate --json has the same shape as the MCP tool.
    let (_, json, _) = keyloom(h, &["--json", "validate", "wow"]);
    let v: Value = serde_json::from_str(&json).unwrap();
    assert!(v["errors"].is_number() && v["findings"].is_array(), "{json}");
    // A draft can only be based on an imported revision.
    let foreign = h.join("foreign.json");
    let (_, exported, _) = keyloom(h, &["export", "--format", "oryx"]);
    std::fs::write(&foreign, exported.replace("\"9qMJK\"", "\"OTHER\"")).unwrap();
    let (code, _, err) = keyloom(h, &["draft", "new", "x", "--from", foreign.to_str().unwrap()]);
    assert!(code == 1 && err.contains("isn't imported"), "{err}");
    assert!(!h.join("layouts/OTHER").exists());
    // A bare id is an id even if a file of that name is in the current directory.
    let (code, _, err) = keyloom(h, &["import", "Cargo"]);
    assert!(code == 1 && !err.contains("not JSON"), "{err}");
    assert_eq!(keyloom(h, &["set", "wow", "WoW", "L.top.3", "KC_W"]).0, 0);
    // Stopping the step-by-step guide early doesn't claim the guide is done (and needs no Keymapp).
    let mut child = Command::new(bin())
        .args(["guide", "wow", "--step"])
        .env("KEYLOOM_HOME", h)
        .env("KEYMAPP_SOCKET", h.join("no-keymapp.sock"))
        .env("KEYLOOM_USB_SERIALS", "")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"\nq\n").unwrap();
    let o = child.wait_with_output().unwrap();
    let text = String::from_utf8_lossy(&o.stdout);
    assert!(o.status.success() && text.contains("Stopped at step 2") && !text.contains("Compile the layout"), "{text}");
    // Output to a closed pipe ends quietly.
    let mut child = Command::new(bin())
        .args(["show", "--all"])
        .env("KEYLOOM_HOME", h)
        .env("KEYLOOM_USB_SERIALS", "")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdout.take());
    let o = child.wait_with_output().unwrap();
    assert!(!String::from_utf8_lossy(&o.stderr).contains("panicked"), "{}", String::from_utf8_lossy(&o.stderr));
}

#[test]
fn mcp_rejects_misspelt_arguments() {
    let home = workspace_with_draft();
    let mut m = Mcp::start(home.path());
    let r = m.call("tools/call", json!({"name": "edit_draft", "arguments": {"draft": "wow", "ops": [], "opz": []}}));
    assert!(r.is_null() || r["isError"] == true, "an unknown argument must not be ignored: {r}");
    let (err, text) = m.tool("edit_draft", json!({"draft": "wow", "ops": [{"op": "add_layer", "title": "X", "copyFrom": "Main"}]}));
    assert!(err && text.contains("nothing was changed"), "{text}");
    let (err, text) =
        m.tool("edit_draft", json!({"draft": "wow", "ops": [{"op": "set_key", "layer": 0, "key": "L.home.1", "acton": "B"}]}));
    assert!(err && text.contains("nothing was changed"), "{text}");
    // A big edit comes back short.
    let (err, text) = m.tool("edit_draft", json!({"draft": "wow", "ops": [{"op": "add_layer", "title": "Big", "fill": "none"}]}));
    assert!(!err && text.len() < 9000 && text.contains("moreChanges"), "{} bytes", text.len());
    // Renaming a layer doesn't make its old findings look new.
    let (err, text) = m.tool("edit_draft", json!({"draft": "wow", "ops": [{"op": "rename_layer", "layer": "Gaming", "title": "Games"}]}));
    let r: Value = serde_json::from_str(&text).unwrap();
    assert!(!err && r["newFindings"].as_array().unwrap().is_empty(), "{text}");
    let (err, text) = m.tool("find_keycode", json!({"layout": "wow", "query": "LT"}));
    assert!(!err && text.contains("LT(1, KC_SPACE)"), "{text}");
    let (_, text) = m.tool("lookup_keycode", json!({"query": "LT"}));
    assert!(text.contains("Layer-tap"), "{text}");
}

#[test]
fn round_five_fixes() {
    let home = workspace_with_draft();
    let h = home.path();
    // A draft exported in Oryx's shape can't be imported as a revision.
    let exported = h.join("draft-export.json");
    assert_eq!(keyloom(h, &["export", "wow", "--format", "oryx", "-o", exported.to_str().unwrap()]).0, 0);
    let (code, _, err) = keyloom(h, &["import", exported.to_str().unwrap()]);
    assert!(code == 1 && err.contains("keyloom draft"), "{err}");
    // A guide for a draft with errors says so first.
    assert_eq!(keyloom(h, &["set", "wow", "WoW", "L.thumb.1", "KC_LALT"]).0, 0);
    let (_, guide, _) = keyloom(h, &["guide", "wow"]);
    assert!(guide.starts_with("NOT READY TO FLASH: this draft has 1 validation error(s)") && guide.contains("[layer-no-exit]"), "{guide}");
    // …and it isn't pushed to Oryx (refused before any network call).
    let (code, _, err) = keyloom(h, &["push", "wow"]);
    assert!(code == 1 && err.contains("Nothing was sent to Oryx") && err.contains("[layer-no-exit]"), "{err}");
    assert_eq!(keyloom(h, &["draft", "undo", "wow"]).0, 0);
    // Into the user's own layout needs their login token; without one nothing is sent.
    let (code, _, err) = keyloom(h, &["push", "wow", "--to", "https://configure.zsa.io/voyager/layouts/9qMJK/latest/0"]);
    assert!(code == 1 && err.contains("needs your login token") && err.contains("Nothing was sent to Oryx"), "{err}");
    assert_eq!(keyloom(h, &["push", "wow", "--overwrite"]).0, 2, "--overwrite only with --to");
    assert!(!keyloom(h, &["guide", "wow"]).1.contains("NOT READY"));
    // A draft-only layer can't be shown on the keyboard; an existing one maps to the firmware's number.
    let (code, _, err) = keyloom(h, &["live", "layer", "WoW", "wow"]);
    assert!(code == 1 && err.contains("isn't in 9qMJK@ZPjbXz (the revision on the keyboard)"), "{err}");
    let (_, _, err) = keyloom(h, &["show", "wow", "-l", "WoW", "--show"]);
    assert!(err.contains("--show skipped") && err.contains("isn't in 9qMJK@ZPjbXz"), "{err}");
    // Labels and titles are one line.
    let ops = h.join("label.json");
    std::fs::write(&ops, r#"[{"op":"set_label","layer":0,"key":"L.home.1","label":"two\nlines"}]"#).unwrap();
    let (code, _, err) = keyloom(h, &["apply", "wow", ops.to_str().unwrap()]);
    assert!(code == 1 && err.contains("one line"), "{err}");
    // A modifier on nothing is refused.
    let (code, _, err) = keyloom(h, &["set", "wow", "WoW", "L.home.3", "LCTL(TRNS)"]);
    assert!(code == 1 && err.contains("needs a key to modify"), "{err}");
    let (code, _, err) = keyloom(h, &["set", "wow", "WoW", "L.home.3", "MEH_T(KC_A)", "--slot", "hold"]);
    assert!(code == 1 && err.contains("whole key") && !err.contains("write MEH_T(KC_x)"), "{err}");
}

#[test]
fn a_draft_edited_after_flashing_only_lists_what_is_left() {
    let home = workspace_with_draft();
    let h = home.path();
    // Apply the draft "in Oryx" and import the result.
    let (_, exported, _) = keyloom(h, &["export", "wow", "--format", "oryx"]);
    let mut oryx: Value = serde_json::from_str(&exported).unwrap();
    oryx["revision"].as_object_mut().unwrap().remove("keyloomDraft");
    oryx["revision"]["hashId"] = json!("FLASHED");
    oryx["revision"]["createdAt"] = json!("2030-01-01 00:00:00 UTC");
    let file = h.join("flashed.json");
    std::fs::write(&file, oryx.to_string()).unwrap();
    assert_eq!(keyloom(h, &["import", file.to_str().unwrap()]).0, 0);
    let (code, out, _) = keyloom(h, &["verify", "wow"]);
    assert!(code == 0 && out.contains("start a new draft"), "{out}");
    // The draft's new layer is now in the newest revision: the keyboard can be asked for it
    // (the only thing missing here is Keymapp).
    let (code, _, err) = keyloom(h, &["live", "layer", "WoW", "wow"]);
    assert!(code == 1 && err.contains("Keymapp's API isn't reachable") && !err.contains("isn't in"), "{err}");
    let (_, guide, _) = keyloom(h, &["guide", "wow"]);
    assert!(guide.contains("Nothing to do") && !guide.contains("Add a layer"), "{guide}");
    // One more edit to the same draft: the guide has that step only, from the flashed revision.
    assert_eq!(keyloom(h, &["set", "wow", "WoW", "L.top.3", "KC_W"]).0, 0);
    let (_, guide, _) = keyloom(h, &["guide", "wow"]);
    assert!(guide.contains("based on 9qMJK@FLASHED") && guide.contains("already has part of this draft"), "{guide}");
    assert!(guide.contains("→ KC_W") && !guide.contains("Add a layer \"WoW\""), "{guide}");
    assert_eq!(guide.matches("Tap: ").count(), 1, "{guide}");
    // If Oryx was also changed by hand in between, the guide says so instead of guessing.
    oryx["revision"]["hashId"] = json!("HANDEDIT");
    oryx["revision"]["createdAt"] = json!("2031-01-01 00:00:00 UTC");
    oryx["revision"]["layers"][1]["keys"][0] = json!({"tap": {"code": "KC_F24"}});
    std::fs::write(&file, oryx.to_string()).unwrap();
    assert_eq!(keyloom(h, &["import", file.to_str().unwrap()]).0, 0);
    let (_, guide, _) = keyloom(h, &["guide", "wow"]);
    assert!(guide.contains("based on 9qMJK@HANDEDIT") && guide.contains("1 of these steps undo changes"), "{guide}");
    assert!(!guide.contains("Add a layer \"WoW\""), "never the steps that are already done: {guide}");
}
