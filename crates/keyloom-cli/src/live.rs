//! Live keyboard features (spec 06) through Keymapp: status, highlighting keys, showing layers,
//! and always restoring what we changed.
//!
//! Keymapp's LED index equals the Oryx key index (checked on a Voyager, 2026-11-14).

use crate::keymapp::{parse_color, Client, Keyboard, KeymappError, Status};
use keyloom::source::LayoutRef;
use serde::Serialize;
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyboardStatus {
    /// Keymapp's view, if its API is reachable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keymapp: Option<Status>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keymapp_error: Option<String>,
    /// Keyboards Keymapp sees (listed when none is connected).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub keyboards: Vec<Keyboard>,
    /// `<layoutId>/<revisionId>` of the flashed firmware, from Keymapp or the USB serial number.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flashed: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flashed_source: Option<String>,
}

/// USB serial numbers of ZSA devices (vendor 0x3297). Oryx firmware reports `<layout>/<revision>`.
/// `KEYLOOM_USB_SERIALS` (comma-separated, may be empty) replaces the lookup, for tests.
pub fn usb_serials() -> Vec<String> {
    if let Ok(v) = std::env::var("KEYLOOM_USB_SERIALS") {
        return v.split(',').map(str::trim).filter(|s| !s.is_empty()).map(String::from).collect();
    }
    if cfg!(target_os = "macos") {
        return Command::new("ioreg")
            .args(["-p", "IOUSB", "-l", "-w0"])
            .output()
            .map(|o| parse_ioreg(&String::from_utf8_lossy(&o.stdout)))
            .unwrap_or_default();
    }
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir("/sys/bus/usb/devices") {
        for e in rd.filter_map(|e| e.ok()) {
            let p = e.path();
            if std::fs::read_to_string(p.join("idVendor")).map(|v| v.trim() == "3297").unwrap_or(false) {
                if let Ok(s) = std::fs::read_to_string(p.join("serial")) {
                    out.push(s.trim().to_string());
                }
            }
        }
    }
    out
}

/// Serial numbers of devices with `idVendor` 0x3297 (12951) in `ioreg -p IOUSB -l` output.
pub fn parse_ioreg(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut vendor_ok = false;
    let mut serial: Option<String> = None;
    let mut flush = |vendor_ok: bool, serial: &mut Option<String>| {
        if let (true, Some(s)) = (vendor_ok, serial.take()) {
            out.push(s);
        }
    };
    for line in text.lines() {
        let l = line.trim_start_matches([' ', '|']);
        if l.starts_with("+-o ") {
            flush(vendor_ok, &mut serial);
            vendor_ok = false;
        } else if l.starts_with("\"idVendor\" = ") {
            vendor_ok = l.trim_end().ends_with("= 12951");
        } else if let Some(v) = l.strip_prefix("\"USB Serial Number\" = ") {
            serial = Some(v.trim().trim_matches('"').to_string());
        }
    }
    flush(vendor_ok, &mut serial);
    out
}

pub async fn status(client: &Client) -> KeyboardStatus {
    let (keymapp, keymapp_error) = match client.status().await {
        Ok(s) => (Some(s), None),
        Err(e) => (None, Some(e.to_string())),
    };
    let keyboards = match &keymapp {
        Some(s) if s.keyboard.is_none() => client.keyboards().await.unwrap_or_default(),
        _ => Vec::new(),
    };
    let from_keymapp = keymapp
        .as_ref()
        .and_then(|s| s.keyboard.as_ref())
        .map(|k| k.firmware_version.clone())
        .filter(|v| LayoutRef::from_serial(v, None).is_some());
    let (flashed, flashed_source) = match from_keymapp {
        Some(v) => (Some(v), Some("keymapp".to_string())),
        None => match usb_serials().into_iter().find(|s| LayoutRef::from_serial(s, None).is_some()) {
            Some(s) => (Some(s), Some("usb-serial".to_string())),
            None => (None, None),
        },
    };
    KeyboardStatus { keymapp, keymapp_error, keyboards, flashed, flashed_source }
}

/// What keyloom changed on the keyboard, so it can be undone. Kept in a small file in the
/// workspace cache, because the change and the reset are often different processes
/// (`keyloom find Esc --show`, later `keyloom live reset`, or the MCP server). The file is the
/// truth: it is re-read before every operation.
#[derive(Debug, Clone, Default, PartialEq, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct LiveState {
    /// The layer keyloom switched on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    layer_set: Option<i64>,
    /// The layer the keyboard was on before keyloom's first change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    start_layer: Option<i64>,
    /// keyloom switched the keyboard back to the base layer (from `start_layer`).
    #[serde(default)]
    went_base: bool,
    #[serde(default)]
    lights_changed: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetReport {
    pub lights_restored: bool,
    /// What happened to the layer, in words.
    pub layer: String,
}

pub struct Session {
    pub client: Client,
    state: LiveState,
    state_path: Option<PathBuf>,
    /// This session changed something on the keyboard.
    touched: bool,
    /// The last operation made Keymapp connect to a keyboard (it had none, e.g. after a replug).
    pub reconnected: bool,
}

impl Session {
    /// `state_path`: where to remember changes across processes (e.g. `<workspace>/cache/live.json`).
    pub fn new(client: Client, state_path: Option<PathBuf>) -> Self {
        Self { client, state: LiveState::default(), state_path, touched: false, reconnected: false }
    }

    /// Whether this session changed the keyboard (also when a later step then failed).
    pub fn changed(&self) -> bool {
        self.touched
    }

    /// Re-reads the shared state: another keyloom process may have changed or reset the keyboard.
    fn sync(&mut self) {
        if let Some(path) = &self.state_path {
            self.state = std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        }
    }

    fn save(&self) {
        let Some(path) = &self.state_path else { return };
        if self.state == LiveState::default() {
            let _ = std::fs::remove_file(path);
        } else if let Ok(text) = serde_json::to_string(&self.state) {
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(path, text);
        }
    }

    /// The connected keyboard's status; asks Keymapp to connect one if it has none.
    async fn connected(&mut self) -> Result<crate::keymapp::ConnectedKeyboard, KeymappError> {
        if let Some(kb) = self.client.status().await?.keyboard {
            return Ok(kb);
        }
        let no_keyboard = || {
            KeymappError::NoKeyboard(
                "Keymapp isn't connected to a keyboard and couldn't connect to one. Check the cable and that the keyboard \
                 shows up in Keymapp; if another app holds the keyboard (one that talks to it directly, not through Keymapp), \
                 quit it and retry."
                    .into(),
            )
        };
        // Just after a keyboard was disconnected (in Keymapp, or by unplugging it) Keymapp can
        // refuse to connect for a moment: ask again for up to about 2 s.
        for attempt in 0..10 {
            if attempt > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                if let Some(kb) = self.client.status().await?.keyboard {
                    self.reconnected = true;
                    return Ok(kb);
                }
            }
            if self.client.connect_any().await.unwrap_or(false) {
                self.reconnected = true;
                if let Some(kb) = self.client.status().await?.keyboard {
                    return Ok(kb);
                }
            }
        }
        Err(no_keyboard())
    }

    /// Switches the keyboard to `layer`; 0 means back to the base layer.
    pub async fn show_layer(&mut self, layer: i64) -> Result<(), KeymappError> {
        self.reconnected = false;
        self.sync();
        self.switch_to(layer).await
    }

    async fn switch_to(&mut self, layer: i64) -> Result<(), KeymappError> {
        let current = self.connected().await?.current_layer;
        if current == layer && self.state.layer_set != Some(layer) {
            return Ok(()); // already there, and not because of keyloom: nothing to do or undo
        }
        if self.state.layer_set.is_none() && !self.state.went_base {
            self.state.start_layer = Some(current);
        }
        if let Some(prev) = self.state.layer_set.take() {
            let _ = self.client.unset_layer(prev).await;
        }
        if layer == 0 {
            // Switch off whatever is on top until the base layer shows (layers can be stacked).
            for _ in 0..8 {
                let now = self.connected().await?.current_layer;
                if now == 0 {
                    break;
                }
                self.client.unset_layer(now).await?;
            }
            self.state.went_base = self.state.start_layer.is_some_and(|s| s != 0);
        } else {
            self.client.set_layer(layer).await?;
            self.state.layer_set = Some(layer);
        }
        self.touched = true;
        self.save();
        Ok(())
    }

    /// Dims the board and lights `keys` (Oryx key indices) in `color`, optionally on `layer`.
    pub async fn highlight(&mut self, keys: &[usize], layer: Option<i64>, color: &str) -> Result<(), KeymappError> {
        let rgb = parse_color(color).ok_or_else(|| KeymappError::Protocol(format!("'{color}' isn't a #rrggbb color")))?;
        if keys.is_empty() {
            return Err(KeymappError::Protocol("there are no keys to light".into()));
        }
        self.reconnected = false;
        self.sync();
        self.connected().await?;
        if let Some(l) = layer {
            self.switch_to(l).await?;
        }
        self.client.set_rgb_all((0, 0, 0), 0).await?;
        self.state.lights_changed = true;
        self.touched = true;
        self.save();
        for &k in keys {
            self.client.set_rgb_led(k as i64, rgb, 0).await?;
        }
        Ok(())
    }

    /// Undoes keyloom's changes: lights back to the firmware's own, the layer keyloom switched
    /// on switched off, and the layer the keyboard was on before restored. If the keyboard is
    /// no longer where keyloom left it (the user switched layers), the layer is left alone.
    ///
    /// `explicit`: the user asked for a reset (`keyloom live reset`, MCP `reset_live`), so the
    /// lights are restored in any case. Otherwise (a session ending) only this session's own
    /// changes are undone.
    pub async fn reset(&mut self, explicit: bool) -> Result<ResetReport, KeymappError> {
        if !explicit && !self.touched {
            return Ok(ResetReport { lights_restored: false, layer: "not changed by this session".into() });
        }
        self.sync();
        let nothing_recorded = self.state == LiveState::default();
        let mut lights = explicit || self.state.lights_changed;
        if lights && nothing_recorded && self.client.status().await?.keyboard.is_none() {
            // Nothing keyloom knows of to undo, and Keymapp holds no keyboard whose lights it
            // could have changed: don't connect one just to restore it.
            lights = false;
        } else if lights {
            self.connected().await?;
            self.client.restore_rgb().await?;
            self.state.lights_changed = false;
        }
        let start = self.state.start_layer.take().filter(|&s| s != 0);
        let went_base = std::mem::take(&mut self.state.went_base);
        let layer = match self.state.layer_set.take() {
            Some(l) => {
                let current = self.connected().await?.current_layer;
                if current != l {
                    format!("the keyboard is on layer {current}, not on layer {l} where keyloom left it: left as it is")
                } else if start == Some(l) {
                    format!("layer {l} is where the keyboard started: left on")
                } else {
                    self.client.unset_layer(l).await?;
                    match start.filter(|&s| s != l) {
                        Some(s) => {
                            self.client.set_layer(s).await?;
                            format!("layer {l} switched off, back on layer {s}")
                        }
                        None => format!("layer {l} switched off"),
                    }
                }
            }
            None => match start.filter(|_| went_base) {
                Some(s) => {
                    if self.connected().await?.current_layer == 0 {
                        self.client.set_layer(s).await?;
                        format!("back on layer {s}")
                    } else {
                        "the keyboard isn't on the base layer where keyloom left it: left as it is".to_string()
                    }
                }
                None => "not changed by keyloom, left as it is".to_string(),
            },
        };
        self.touched = false;
        self.save();
        Ok(ResetReport { lights_restored: lights, layer })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_zsa_serials_in_ioreg_output() {
        let text = r#"
+-o Root  <class IORegistryEntry, id 0x100000100, retain 23>
  +-o Preonic@01120000  <class IOUSBHostDevice>
  | {
  |   "USB Product Name" = "Preonic"
  |   "idVendor" = 1155
  |   "USB Serial Number" = "ABC"
  | }
  +-o Voyager@01130000  <class IOUSBHostDevice>
    {
      "USB Product Name" = "Voyager"
      "idVendor" = 12951
      "USB Serial Number" = "9qMJK/ZPjbXz"
    }
"#;
        assert_eq!(parse_ioreg(text), vec!["9qMJK/ZPjbXz".to_string()]);
    }

    #[cfg(unix)]
    mod with_fake_keymapp {
        use super::super::*;
        use crate::keymapp::fake;

        #[tokio::test]
        async fn reset_restores_lights_and_the_starting_layer() {
            let f = fake::start().await;
            f.state.lock().unwrap().layer = 3; // the user was on layer 3
            let dir = tempfile::tempdir().unwrap();
            let state = dir.path().join("live.json");
            let mut s = Session::new(Client::new(f.client.endpoint().clone()), Some(state.clone()));
            s.highlight(&[16, 17], Some(1), "#00ffaa").await.unwrap();
            assert_eq!(f.calls(), ["SetLayer(1)", "SetRGBAll", "SetRGBLed(16)", "SetRGBLed(17)"]);
            assert!(state.is_file(), "changes are remembered across processes");

            // A different process (e.g. `keyloom live reset`) undoes them.
            let mut other = Session::new(Client::new(f.client.endpoint().clone()), Some(state.clone()));
            let report = other.reset(true).await.unwrap();
            assert_eq!(f.calls()[4..], ["SetRGBAll", "UnsetLayer(1)", "SetLayer(3)"]);
            assert_eq!(report.layer, "layer 1 switched off, back on layer 3");
            assert!(!state.exists());
        }

        #[tokio::test]
        async fn reset_leaves_a_layer_the_user_chose_alone() {
            let f = fake::start().await;
            f.state.lock().unwrap().layer = 2;
            let mut s = Session::new(Client::new(f.client.endpoint().clone()), None);
            let report = s.reset(true).await.unwrap();
            assert_eq!(f.calls(), ["SetRGBAll"]);
            assert!(report.layer.contains("left as it is"));
            assert_eq!(f.state.lock().unwrap().layer, 2);
            // A session that ends without having changed anything does nothing at all.
            let mut s = Session::new(Client::new(f.client.endpoint().clone()), None);
            s.reset(false).await.unwrap();
            assert_eq!(f.calls(), ["SetRGBAll"]);
        }

        #[tokio::test]
        async fn layer_zero_goes_back_to_the_base_layer_and_reset_returns() {
            let f = fake::start().await;
            f.state.lock().unwrap().layer = 4;
            let mut s = Session::new(Client::new(f.client.endpoint().clone()), None);
            s.show_layer(0).await.unwrap();
            assert_eq!(f.calls(), ["UnsetLayer(4)"]);
            assert_eq!(f.state.lock().unwrap().layer, 0);
            let report = s.reset(true).await.unwrap();
            assert_eq!(report.layer, "back on layer 4");
            assert_eq!(f.state.lock().unwrap().layer, 4);
        }

        #[tokio::test]
        async fn reset_respects_what_the_user_did_in_between() {
            // Showing the layer the keyboard is already on changes and undoes nothing.
            let f = fake::start().await;
            f.state.lock().unwrap().layer = 2;
            let mut s = Session::new(Client::new(f.client.endpoint().clone()), None);
            s.show_layer(2).await.unwrap();
            s.reset(true).await.unwrap();
            assert_eq!(f.calls(), ["SetRGBAll"]);
            assert_eq!(f.state.lock().unwrap().layer, 2);

            // keyloom showed layer 1, then the user switched to layer 3: reset leaves the layer.
            let f = fake::start().await;
            let mut s = Session::new(Client::new(f.client.endpoint().clone()), None);
            s.show_layer(1).await.unwrap();
            f.state.lock().unwrap().layer = 3;
            let report = s.reset(true).await.unwrap();
            assert!(report.layer.contains("left as it is"), "{}", report.layer);
            assert_eq!(f.state.lock().unwrap().layer, 3);
        }

        #[tokio::test]
        async fn reset_after_returning_to_the_starting_layer_stays_there() {
            let f = fake::start().await;
            f.state.lock().unwrap().layer = 3;
            let mut s = Session::new(Client::new(f.client.endpoint().clone()), None);
            s.show_layer(1).await.unwrap();
            s.show_layer(3).await.unwrap();
            let report = s.reset(true).await.unwrap();
            assert!(report.layer.contains("where the keyboard started"), "{}", report.layer);
            assert_eq!(f.state.lock().unwrap().layer, 3);
        }

        #[tokio::test]
        async fn sessions_share_state_and_only_undo_their_own_changes_on_exit() {
            let f = fake::start().await;
            let dir = tempfile::tempdir().unwrap();
            let state = dir.path().join("cache/live.json");
            let client = || Client::new(f.client.endpoint().clone());
            // A long-lived session (the MCP server) that hasn't done anything …
            let mut server = Session::new(client(), Some(state.clone()));
            // … while the CLI lights a key and later resets it.
            let mut cli = Session::new(client(), Some(state.clone()));
            cli.highlight(&[3], Some(1), "#00ffaa").await.unwrap();
            // The server ending must not undo the CLI's change.
            server.reset(false).await.unwrap();
            assert_eq!(f.state.lock().unwrap().layer, 1);
            let mut cli2 = Session::new(client(), Some(state.clone()));
            cli2.reset(true).await.unwrap();
            assert_eq!(f.state.lock().unwrap().layer, 0);
            // The server then shows a layer: it sees the current state, not a stale copy.
            server.show_layer(2).await.unwrap();
            assert!(server.reset(false).await.unwrap().layer.contains("layer 2 switched off"));
            assert!(!state.exists());
            // Nothing to light is an error, not a dark keyboard.
            assert!(server.highlight(&[], None, "#00ffaa").await.is_err());
        }

        #[tokio::test]
        async fn refused_and_disconnected_are_errors() {
            let f = fake::start().await;
            f.state.lock().unwrap().refuse = true;
            let mut s = Session::new(Client::new(f.client.endpoint().clone()), None);
            assert!(s.show_layer(7).await.unwrap_err().to_string().contains("couldn't switch to layer 7"));

            let f = fake::start().await;
            f.state.lock().unwrap().connected = false;
            let mut s = Session::new(Client::new(f.client.endpoint().clone()), None);
            s.show_layer(1).await.unwrap();
            assert!(s.reconnected, "Keymapp was asked to connect, and that is reported");
            assert_eq!(f.calls(), ["ConnectAnyKeyboard", "SetLayer(1)"]);
        }

        /// Seen with Keymapp 1.3.7 after unplugging and replugging the keyboard: Keymapp first
        /// refuses to connect, then refuses commands for a moment ("requires an updated firmware").
        #[tokio::test]
        async fn waits_for_keymapp_after_reconnecting() {
            let f = fake::start().await;
            {
                let mut st = f.state.lock().unwrap();
                st.connected = false;
                st.connect_refusals = 2;
                st.not_ready = 3;
            }
            let mut s = Session::new(Client::new(f.client.endpoint().clone()), None);
            s.show_layer(1).await.unwrap();
            assert!(s.reconnected);
            assert_eq!(f.state.lock().unwrap().layer, 1);
        }

        #[tokio::test]
        async fn reset_with_nothing_to_undo_does_not_need_a_keyboard() {
            let f = fake::start().await;
            f.state.lock().unwrap().connected = false;
            let mut s = Session::new(Client::new(f.client.endpoint().clone()), None);
            let r = s.reset(true).await.unwrap();
            assert!(!r.lights_restored);
            assert!(f.calls().is_empty(), "no connect, no lights: {:?}", f.calls());
        }
    }
}
