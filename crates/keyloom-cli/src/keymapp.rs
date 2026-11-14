//! Client for Keymapp's local gRPC API (`api.KeyboardService`, zsa/kontroll's keymapp.proto).
//!
//! Hand-rolled: HTTP/2 via `h2`, and the few protobuf messages encoded by hand, so building
//! keyloom doesn't need `protoc` (which the kontroll crate does).

use bytes::{Buf, Bytes};
use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum KeymappError {
    #[error("Keymapp's API isn't reachable ({0}). Start Keymapp and enable Settings → API.")]
    Unreachable(String),
    #[error("Keymapp API: {0}")]
    Protocol(String),
    /// gRPC status code and message (the message may be empty).
    #[error("Keymapp answered with gRPC status {0} {1}")]
    Status(String, String),
    #[error("Keymapp couldn't {0}. Is the keyboard connected in Keymapp, and does the flashed firmware have that layer/LED?")]
    Refused(String),
    #[error("{0}")]
    NoKeyboard(String),
}

type Result<T> = std::result::Result<T, KeymappError>;

// MARK: protobuf

fn varint(mut v: u64, out: &mut Vec<u8>) {
    while v >= 0x80 {
        out.push((v as u8) | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}

/// Encodes int32/bool fields (wire type 0). Negative int32 use 10-byte varints as in protobuf.
pub fn encode_ints(fields: &[(u32, i64)]) -> Vec<u8> {
    let mut out = Vec::new();
    for &(n, v) in fields {
        if v == 0 {
            continue; // proto3 default
        }
        varint((n as u64) << 3, &mut out);
        varint(v as u64, &mut out);
    }
    out
}

#[derive(Debug, Clone, PartialEq)]
pub enum Field {
    Int(u64),
    Bytes(Vec<u8>),
}

pub fn decode(mut buf: &[u8]) -> Result<Vec<(u32, Field)>> {
    let bad = || KeymappError::Protocol("malformed protobuf".into());
    fn read_varint(buf: &mut &[u8]) -> Option<u64> {
        let mut v = 0u64;
        for shift in (0..64).step_by(7) {
            let (&b, rest) = buf.split_first()?;
            *buf = rest;
            v |= ((b & 0x7f) as u64) << shift;
            if b < 0x80 {
                return Some(v);
            }
        }
        None
    }
    let mut out = Vec::new();
    while !buf.is_empty() {
        let tag = read_varint(&mut buf).ok_or_else(bad)?;
        let (n, wire) = ((tag >> 3) as u32, tag & 7);
        match wire {
            0 => out.push((n, Field::Int(read_varint(&mut buf).ok_or_else(bad)?))),
            2 => {
                let len = read_varint(&mut buf).ok_or_else(bad)? as usize;
                if buf.len() < len {
                    return Err(bad());
                }
                out.push((n, Field::Bytes(buf[..len].to_vec())));
                buf = &buf[len..];
            }
            5 => buf = buf.get(4..).ok_or_else(bad)?,
            1 => buf = buf.get(8..).ok_or_else(bad)?,
            _ => return Err(bad()),
        }
    }
    Ok(out)
}

fn int(fields: &[(u32, Field)], n: u32) -> i64 {
    fields
        .iter()
        .find_map(|(f, v)| {
            if *f == n {
                if let Field::Int(i) = v {
                    Some(*i as i64)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .unwrap_or(0)
}

fn string(fields: &[(u32, Field)], n: u32) -> String {
    fields
        .iter()
        .find_map(|(f, v)| {
            if *f == n {
                if let Field::Bytes(b) = v {
                    Some(String::from_utf8_lossy(b).to_string())
                } else {
                    None
                }
            } else {
                None
            }
        })
        .unwrap_or_default()
}

fn message(fields: &[(u32, Field)], n: u32) -> Option<Vec<(u32, Field)>> {
    fields.iter().find_map(|(f, v)| {
        if *f == n {
            if let Field::Bytes(b) = v {
                decode(b).ok()
            } else {
                None
            }
        } else {
            None
        }
    })
}

// MARK: messages

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub keymapp_version: String,
    pub keyboard: Option<ConnectedKeyboard>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectedKeyboard {
    pub friendly_name: String,
    pub firmware_version: String,
    pub current_layer: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Keyboard {
    pub id: i64,
    pub friendly_name: String,
    pub is_connected: bool,
}

// MARK: transport

/// Where Keymapp listens: a Unix socket on macOS/Linux, TCP on Windows.
#[derive(Debug, Clone)]
pub enum Endpoint {
    #[cfg(unix)]
    Unix(std::path::PathBuf),
    Tcp(String),
}

impl Endpoint {
    pub fn default_for_platform() -> Self {
        if let Ok(port) = std::env::var("KEYMAPP_PORT") {
            return Endpoint::Tcp(format!("127.0.0.1:{port}"));
        }
        #[cfg(unix)]
        {
            // Same override as ZSA's kontroll.
            if let Some(path) = std::env::var_os("KEYMAPP_SOCKET").filter(|p| !p.is_empty()) {
                return Endpoint::Unix(path.into());
            }
            let home = dirs::home_dir().unwrap_or_default();
            let base =
                if cfg!(target_os = "macos") { home.join("Library/Application Support") } else { dirs::config_dir().unwrap_or_default() };
            let default = base.join(".keymapp/keymapp.sock");
            // The Mac App Store build of Keymapp is sandboxed and keeps its socket in its container.
            let sandboxed = home.join("Library/Containers/io.zsa.keymapp/Data/Library/Application Support/.keymapp/keymapp.sock");
            if cfg!(target_os = "macos") && !default.exists() && sandboxed.exists() {
                return Endpoint::Unix(sandboxed);
            }
            Endpoint::Unix(default)
        }
        #[cfg(not(unix))]
        {
            Endpoint::Tcp("127.0.0.1:50051".into())
        }
    }
}

impl std::fmt::Display for Endpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            #[cfg(unix)]
            Endpoint::Unix(p) => write!(f, "{}", p.display()),
            Endpoint::Tcp(a) => write!(f, "{a}"),
        }
    }
}

pub struct Client {
    endpoint: Endpoint,
    timeout: Duration,
}

/// gRPC percent-encodes `grpc-message`.
fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() && s.is_char_boundary(i + 1) && s.is_char_boundary(i + 3) {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

impl Client {
    pub fn new(endpoint: Endpoint) -> Self {
        Self { endpoint, timeout: Duration::from_secs(3) }
    }

    pub fn default_for_platform() -> Self {
        Self::new(Endpoint::default_for_platform())
    }

    #[cfg(test)]
    pub fn endpoint(&self) -> &Endpoint {
        &self.endpoint
    }

    async fn call(&self, method: &str, body: Vec<u8>) -> Result<Vec<(u32, Field)>> {
        // Right after Keymapp connects to a keyboard it answers commands with "keyboard requires an
        // updated firmware" for a few tens of milliseconds (seen with Keymapp 1.3.7), until it has
        // talked to the firmware. Retry for a moment; a really outdated firmware still gets the error.
        let mut tries = 0;
        loop {
            let r = tokio::time::timeout(self.timeout, self.call_inner(method, body.clone()))
                .await
                .map_err(|_| KeymappError::Unreachable(format!("{}: timed out", self.endpoint)))?;
            match r {
                Err(KeymappError::Status(code, msg)) if code == "2" && msg.contains("updated firmware") && tries < 15 => {
                    tries += 1;
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                r => return r,
            }
        }
    }

    async fn call_inner(&self, method: &str, body: Vec<u8>) -> Result<Vec<(u32, Field)>> {
        let unreachable = |e: std::io::Error| KeymappError::Unreachable(format!("{}: {e}", self.endpoint));
        let proto = |e: h2::Error| KeymappError::Protocol(e.to_string());
        let send_request = match &self.endpoint {
            #[cfg(unix)]
            Endpoint::Unix(path) => {
                let io = tokio::net::UnixStream::connect(path).await.map_err(unreachable)?;
                let (send, conn) = h2::client::handshake(io).await.map_err(proto)?;
                tokio::spawn(async move {
                    let _ = conn.await;
                });
                send
            }
            Endpoint::Tcp(addr) => {
                let io = tokio::net::TcpStream::connect(addr).await.map_err(unreachable)?;
                let (send, conn) = h2::client::handshake(io).await.map_err(proto)?;
                tokio::spawn(async move {
                    let _ = conn.await;
                });
                send
            }
        };
        let mut send_request = send_request.ready().await.map_err(proto)?;
        let request = http::Request::builder()
            .method("POST")
            .uri(format!("http://localhost/api.KeyboardService/{method}"))
            .header("content-type", "application/grpc")
            .header("te", "trailers")
            .body(())
            .map_err(|e| KeymappError::Protocol(e.to_string()))?;
        let (response, mut stream) = send_request.send_request(request, false).map_err(proto)?;
        let mut framed = Vec::with_capacity(body.len() + 5);
        framed.push(0);
        framed.extend_from_slice(&(body.len() as u32).to_be_bytes());
        framed.extend_from_slice(&body);
        stream.send_data(Bytes::from(framed), true).map_err(proto)?;
        let response = response.await.map_err(proto)?;
        let (head, mut body) = response.into_parts();
        let mut data = Vec::new();
        while let Some(chunk) = body.data().await {
            let chunk = chunk.map_err(proto)?;
            let _ = body.flow_control().release_capacity(chunk.len());
            data.extend_from_slice(chunk.chunk());
        }
        let trailers = body.trailers().await.map_err(proto)?;
        if head.status != http::StatusCode::OK {
            return Err(KeymappError::Protocol(format!("HTTP {}", head.status)));
        }
        // The status is in the trailers, or in the headers for a "trailers-only" (error) reply.
        let meta = trailers.as_ref().filter(|t| t.contains_key("grpc-status")).unwrap_or(&head.headers);
        let Some(status) = meta.get("grpc-status").and_then(|v| v.to_str().ok()) else {
            return Err(KeymappError::Protocol("the reply isn't gRPC (no grpc-status)".into()));
        };
        if status != "0" {
            let msg = meta.get("grpc-message").and_then(|v| v.to_str().ok()).map(percent_decode).unwrap_or_default();
            return Err(KeymappError::Status(status.to_string(), msg));
        }
        if data.len() < 5 {
            return Ok(Vec::new());
        }
        decode(&data[5..])
    }

    fn success(fields: &[(u32, Field)]) -> bool {
        int(fields, 1) != 0
    }

    /// Keymapp answers `success = false` when it can't do something (no keyboard connected, a
    /// layer or LED the firmware doesn't have): that is an error, not a result to ignore.
    fn done(fields: &[(u32, Field)], what: &str) -> Result<()> {
        if Self::success(fields) {
            Ok(())
        } else {
            Err(KeymappError::Refused(what.to_string()))
        }
    }

    pub async fn status(&self) -> Result<Status> {
        let f = self.call("GetStatus", vec![]).await?;
        let keyboard = message(&f, 2).map(|k| ConnectedKeyboard {
            friendly_name: string(&k, 1),
            firmware_version: string(&k, 2),
            current_layer: int(&k, 3),
        });
        Ok(Status { keymapp_version: string(&f, 1), keyboard })
    }

    pub async fn keyboards(&self) -> Result<Vec<Keyboard>> {
        let f = self.call("GetKeyboards", vec![]).await?;
        Ok(f.iter()
            .filter(|(n, _)| *n == 1)
            .filter_map(|(_, v)| if let Field::Bytes(b) = v { decode(b).ok() } else { None })
            .map(|k| Keyboard { id: int(&k, 1), friendly_name: string(&k, 2), is_connected: int(&k, 3) != 0 })
            .collect())
    }

    pub async fn connect_any(&self) -> Result<bool> {
        Ok(Self::success(&self.call("ConnectAnyKeyboard", vec![]).await?))
    }

    pub async fn set_layer(&self, layer: i64) -> Result<()> {
        Self::done(&self.call("SetLayer", encode_ints(&[(1, layer)])).await?, &format!("switch to layer {layer}"))
    }

    pub async fn unset_layer(&self, layer: i64) -> Result<()> {
        Self::done(&self.call("UnsetLayer", encode_ints(&[(1, layer)])).await?, &format!("switch layer {layer} off"))
    }

    /// `sustain` in milliseconds; 0 keeps the color until restored.
    pub async fn set_rgb_led(&self, led: i64, rgb: (u8, u8, u8), sustain: i64) -> Result<()> {
        let body = encode_ints(&[(1, led), (2, rgb.0 as i64), (3, rgb.1 as i64), (4, rgb.2 as i64), (5, sustain)]);
        Self::done(&self.call("SetRGBLed", body).await?, &format!("light LED {led}"))
    }

    pub async fn set_rgb_all(&self, rgb: (u8, u8, u8), sustain: i64) -> Result<()> {
        let body = encode_ints(&[(1, rgb.0 as i64), (2, rgb.1 as i64), (3, rgb.2 as i64), (4, sustain)]);
        Self::done(&self.call("SetRGBAll", body).await?, "set the LEDs")
    }

    /// Keymapp has no restore RPC. Like kontroll's `restore-rgb-leds`: set all LEDs with a 1 ms
    /// sustain, after which the firmware's own colors come back.
    pub async fn restore_rgb(&self) -> Result<()> {
        self.set_rgb_all((0, 0, 0), 1).await
    }
}

pub fn parse_color(s: &str) -> Option<(u8, u8, u8)> {
    let h = s.trim().trim_start_matches('#');
    if h.len() != 6 || !h.is_ascii() {
        return None;
    }
    let p = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
    Some((p(0)?, p(2)?, p(4)?))
}

/// A fake Keymapp on a Unix socket, for tests: tracks the layer, records every call, and can
/// be told to refuse (answer `success = false`).
#[cfg(all(test, unix))]
pub mod fake {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    pub struct State {
        pub calls: Vec<(String, Vec<(u32, Field)>)>,
        pub layer: i64,
        pub connected: bool,
        pub refuse: bool,
        /// Refuse this many `ConnectAnyKeyboard` calls (as Keymapp does just after a disconnect).
        pub connect_refusals: u32,
        /// After connecting, answer this many commands with "keyboard requires an updated
        /// firmware" (as Keymapp 1.3.7 does for a moment after connecting).
        pub not_ready: u32,
    }

    pub struct Fake {
        pub client: Client,
        pub state: Arc<Mutex<State>>,
        _dir: tempfile::TempDir,
    }

    impl Fake {
        /// Method names called so far, with their first int field (layer or LED) if any.
        pub fn calls(&self) -> Vec<String> {
            self.state
                .lock()
                .unwrap()
                .calls
                .iter()
                .filter(|(m, _)| m != "GetStatus")
                .map(|(m, f)| match f.iter().find(|(n, _)| *n == 1) {
                    Some((_, Field::Int(v))) => format!("{m}({v})"),
                    _ => m.clone(),
                })
                .collect()
        }
    }

    fn status_reply(layer: i64, connected: bool) -> Vec<u8> {
        let mut m = vec![0x0a, 5];
        m.extend_from_slice(b"1.3.6");
        if connected {
            let mut kb = vec![0x0a, 7];
            kb.extend_from_slice(b"Voyager");
            kb.push(0x12);
            kb.push(12);
            kb.extend_from_slice(b"9qMJK/ZPjbXz");
            kb.extend_from_slice(&encode_ints(&[(3, layer)]));
            m.push(0x12);
            m.push(kb.len() as u8);
            m.extend_from_slice(&kb);
        }
        m
    }

    pub async fn start() -> Fake {
        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("keymapp.sock");
        let listener = tokio::net::UnixListener::bind(&sock).unwrap();
        let state = Arc::new(Mutex::new(State { connected: true, ..Default::default() }));
        let shared = state.clone();
        tokio::spawn(async move {
            loop {
                let Ok((io, _)) = listener.accept().await else { return };
                let shared = shared.clone();
                tokio::spawn(async move {
                    let mut conn = h2::server::handshake(io).await.unwrap();
                    while let Some(Ok((req, mut respond))) = conn.accept().await {
                        let method = req.uri().path().rsplit('/').next().unwrap_or("").to_string();
                        let mut body = req.into_body();
                        let mut data = Vec::new();
                        while let Some(Ok(c)) = body.data().await {
                            let _ = body.flow_control().release_capacity(c.len());
                            data.extend_from_slice(&c);
                        }
                        let fields = decode(&data[5..]).unwrap();
                        let mut grpc_status = "0";
                        let reply = {
                            let mut st = shared.lock().unwrap();
                            st.calls.push((method.clone(), fields.clone()));
                            let ok = !st.refuse && (st.connected || method == "ConnectAnyKeyboard");
                            match method.as_str() {
                                "ConnectAnyKeyboard" if st.connect_refusals > 0 => {
                                    st.connect_refusals -= 1;
                                    vec![]
                                }
                                "GetStatus" => status_reply(st.layer, st.connected),
                                "ConnectAnyKeyboard" if ok => {
                                    st.connected = true;
                                    vec![0x08, 1]
                                }
                                "ConnectAnyKeyboard" => vec![],
                                m if ok && m != "GetKeyboards" && st.not_ready > 0 => {
                                    st.not_ready -= 1;
                                    grpc_status = "2";
                                    vec![]
                                }
                                "SetLayer" if ok => {
                                    st.layer = int(&fields, 1);
                                    vec![0x08, 1]
                                }
                                "UnsetLayer" if ok => {
                                    st.layer = 0;
                                    vec![0x08, 1]
                                }
                                _ if ok => vec![0x08, 1],
                                _ => vec![],
                            }
                        };
                        let resp = http::Response::builder().status(200).header("content-type", "application/grpc").body(()).unwrap();
                        let mut stream = respond.send_response(resp, false).unwrap();
                        let mut framed = vec![0];
                        framed.extend_from_slice(&(reply.len() as u32).to_be_bytes());
                        framed.extend_from_slice(&reply);
                        stream.send_data(Bytes::from(framed), false).unwrap();
                        let mut trailers = http::HeaderMap::new();
                        trailers.insert("grpc-status", grpc_status.parse().unwrap());
                        if grpc_status != "0" {
                            trailers.insert("grpc-message", "keyboard requires an updated firmware".parse().unwrap());
                        }
                        stream.send_trailers(trailers).unwrap();
                    }
                });
            }
        });
        Fake { client: Client::new(Endpoint::Unix(sock)), state, _dir: dir }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protobuf_round_trip() {
        let enc = encode_ints(&[(1, 3), (2, 255), (5, 0)]);
        assert_eq!(enc, vec![0x08, 3, 0x10, 0xff, 0x01]);
        let dec = decode(&enc).unwrap();
        assert_eq!(int(&dec, 1), 3);
        assert_eq!(int(&dec, 2), 255);
        assert_eq!(int(&dec, 5), 0);
        assert_eq!(parse_color("#00ffaa"), Some((0, 255, 170)));
        assert_eq!(parse_color("#ééé"), None, "non-ASCII input must not panic");
        assert_eq!(percent_decode("no%20keyboard%2C sorry"), "no keyboard, sorry");
        assert_eq!(percent_decode("100%"), "100%");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn talks_grpc_to_a_fake_keymapp() {
        let fake = fake::start().await;
        fake.state.lock().unwrap().layer = 2;
        let st = fake.client.status().await.unwrap();
        assert_eq!(st.keymapp_version, "1.3.6");
        let kb = st.keyboard.unwrap();
        assert_eq!((kb.friendly_name.as_str(), kb.firmware_version.as_str(), kb.current_layer), ("Voyager", "9qMJK/ZPjbXz", 2));
        fake.client.set_rgb_led(17, (0, 255, 170), 0).await.unwrap();
        fake.client.restore_rgb().await.unwrap();
        let calls = fake.state.lock().unwrap().calls.clone();
        assert_eq!(calls[1].0, "SetRGBLed");
        assert_eq!(calls[1].1, vec![(1, Field::Int(17)), (3, Field::Int(255)), (4, Field::Int(170))]);
        assert_eq!(calls[2], ("SetRGBAll".to_string(), vec![(4, Field::Int(1))]), "restore = all black with 1 ms sustain");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_refused_request_is_an_error() {
        let fake = fake::start().await;
        fake.state.lock().unwrap().refuse = true;
        let e = fake.client.set_layer(9).await.unwrap_err().to_string();
        assert!(e.contains("couldn't switch to layer 9"), "{e}");
        assert!(fake.client.set_rgb_all((1, 2, 3), 0).await.is_err());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn unreachable_keymapp_is_a_clear_error() {
        let c = Client::new(Endpoint::Unix("/nonexistent/keymapp.sock".into()));
        let e = c.status().await.unwrap_err().to_string();
        assert!(e.contains("enable Settings → API") && e.contains("/nonexistent/keymapp.sock"), "{e}");
    }
}
