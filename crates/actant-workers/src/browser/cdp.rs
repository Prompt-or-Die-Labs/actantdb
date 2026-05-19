//! Real Chrome DevTools Protocol driver.
//!
//! Implements the [`Driver`](super::Driver) trait against a headless
//! Chrome / Chromium subprocess that this module spawns and lifecycles.
//!
//! The driver speaks raw CDP over a WebSocket — no `chromiumoxide` /
//! `fantoccini` dependency. Transitive surface is `tokio-tungstenite` +
//! `reqwest` + `futures-util` + `url`, all already used elsewhere in the
//! workspace; chromiumoxide would pull ~150 crates including `tower-http`
//! and `rustix` we don't otherwise need.
//!
//! ## Lifecycle
//!
//! 1. [`CdpDriver::launch_headless`] resolves a Chrome binary (env
//!    `CHROME_PATH`, then PATH, then well-known macOS bundles).
//! 2. Spawns it with `--headless=new --remote-debugging-port=0 --no-sandbox
//!    --disable-gpu --disable-dev-shm-usage --user-data-dir=<tmp>`.
//! 3. Parses the `DevTools listening on ws://HOST:PORT/devtools/browser/…`
//!    line from stderr (Chrome writes this on bind).
//! 4. Opens the browser-level WebSocket, creates a page target via
//!    `Target.createTarget`, attaches via `Target.attachToTarget`
//!    (`flatten: true`) — subsequent messages carry a `sessionId`.
//! 5. Enables `Page` + `Runtime` domains.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::browser::{Action, Driver};
use actant_core::ActantError;
use async_trait::async_trait;
use futures_util::stream::SplitSink;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{oneshot, Mutex};
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

type WsStream = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;
type WsSink = SplitSink<WsStream, Message>;
type PendingMap = Mutex<HashMap<u64, oneshot::Sender<Value>>>;

/// How long to wait for the `DevTools listening on …` line to appear on
/// stderr before giving up on the launch.
const PORT_PARSE_TIMEOUT: Duration = Duration::from_secs(10);

/// How long to wait for the response to a single CDP RPC.
const RPC_TIMEOUT: Duration = Duration::from_secs(30);

/// Real Chrome / Chromium driver speaking raw CDP.
///
/// One `CdpDriver` owns one Chrome process plus one page target. The driver
/// is `Clone`able via interior `Arc` so it slots into
/// [`crate::browser::BrowserHandler`] the same way
/// [`crate::browser::EmulatorDriver`] does.
#[derive(Clone)]
pub struct CdpDriver {
    inner: Arc<Inner>,
}

struct Inner {
    /// Headless Chrome child. Wrapped in `Mutex` so `close()` can `take()`
    /// and await `wait()` from `&self`.
    child: Mutex<Option<Child>>,
    /// CDP session id attached to the page target.
    session_id: String,
    /// CDP target id (page) — used so `close` can `Target.closeTarget` it.
    target_id: String,
    /// Sink for outbound JSON RPC frames.
    ws_send: Mutex<Option<WsSink>>,
    /// Pending RPC responses keyed by request id. Shared with the receive
    /// pump task; the pump removes entries and resolves their oneshots.
    pending: Arc<PendingMap>,
    /// Monotonic CDP message id.
    next_id: AtomicU64,
    /// Temp dir used as `--user-data-dir`; deleted on `close()` (and best-
    /// effort on `Drop`).
    user_data_dir: PathBuf,
    /// Receive pump handle; aborted on close.
    pump: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl std::fmt::Debug for CdpDriver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CdpDriver")
            .field("session_id", &self.inner.session_id)
            .field("target_id", &self.inner.target_id)
            .finish_non_exhaustive()
    }
}

impl CdpDriver {
    /// Spawn a headless Chrome and attach a fresh page target.
    pub async fn launch_headless() -> Result<Self, ActantError> {
        let binary = resolve_chrome_binary().ok_or_else(|| {
            ActantError::Internal(
                "no Chrome/Chromium binary found; set CHROME_PATH or install Chrome".into(),
            )
        })?;

        let tmp = std::env::temp_dir().join(format!(
            "actantdb-cdp-{}-{}",
            std::process::id(),
            timestamp_suffix()
        ));
        std::fs::create_dir_all(&tmp)
            .map_err(|e| ActantError::Internal(format!("create user-data-dir: {e}")))?;

        let mut cmd = Command::new(&binary);
        cmd.arg("--headless=new")
            .arg("--remote-debugging-port=0")
            .arg("--no-sandbox")
            .arg("--disable-gpu")
            .arg("--disable-dev-shm-usage")
            .arg("--no-first-run")
            .arg("--no-default-browser-check")
            .arg("--disable-background-networking")
            .arg("--disable-sync")
            .arg("--disable-translate")
            .arg(format!("--user-data-dir={}", tmp.display()))
            .arg("about:blank")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        let mut child = cmd
            .spawn()
            .map_err(|e| ActantError::Internal(format!("spawn chrome: {e}")))?;

        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| ActantError::Internal("chrome stderr unavailable".into()))?;

        let ws_url = match timeout(PORT_PARSE_TIMEOUT, parse_devtools_url(stderr)).await {
            Ok(Ok(u)) => u,
            Ok(Err(e)) => {
                let _ = child.kill().await;
                let _ = std::fs::remove_dir_all(&tmp);
                return Err(e);
            }
            Err(_) => {
                let _ = child.kill().await;
                let _ = std::fs::remove_dir_all(&tmp);
                return Err(ActantError::Internal(format!(
                    "timed out waiting {PORT_PARSE_TIMEOUT:?} for DevTools listening line"
                )));
            }
        };

        let (ws, _) = connect_async(&ws_url)
            .await
            .map_err(|e| ActantError::Internal(format!("ws connect {ws_url}: {e}")))?;

        let (sink, mut stream) = ws.split();
        let pending: Arc<PendingMap> = Arc::new(Mutex::new(HashMap::new()));
        let next_id = Arc::new(AtomicU64::new(1));

        // Receive pump: dispatch inbound frames to the pending map. Stays
        // alive for the lifetime of the driver; aborted in `close()`.
        let pump_pending = pending.clone();
        let pump = tokio::spawn(async move {
            while let Some(msg) = stream.next().await {
                let msg = match msg {
                    Ok(m) => m,
                    Err(e) => {
                        tracing::debug!(target: "actant_workers::browser::cdp", error = %e, "ws recv error");
                        break;
                    }
                };
                let text = match msg {
                    Message::Text(t) => t,
                    Message::Binary(b) => match String::from_utf8(b) {
                        Ok(t) => t,
                        Err(_) => continue,
                    },
                    Message::Close(_) => break,
                    _ => continue,
                };
                let Ok(val) = serde_json::from_str::<Value>(&text) else {
                    continue;
                };
                if let Some(id) = val.get("id").and_then(|v| v.as_u64()) {
                    let mut p = pump_pending.lock().await;
                    if let Some(tx) = p.remove(&id) {
                        let _ = tx.send(val);
                    }
                }
                // Events (no `id`) ignored — we use synchronous waits.
            }
        });

        // Wrap the send sink in a Mutex so the bootstrap helper can grab
        // it once and the final Inner can adopt the same Mutex afterwards.
        let ws_send: Arc<Mutex<Option<WsSink>>> = Arc::new(Mutex::new(Some(sink)));

        // Helper: on any bootstrap error, tear down what we just spawned so
        // we don't leak a Chrome process / pump task / temp dir.
        macro_rules! bootstrap_try {
            ($e:expr) => {{
                match $e {
                    Ok(v) => v,
                    Err(e) => {
                        pump.abort();
                        let _ = pump.await;
                        let _ = child.kill().await;
                        let _ = child.wait().await;
                        let _ = std::fs::remove_dir_all(&tmp);
                        return Err(e);
                    }
                }
            }};
        }

        // 1. Create page target.
        let res = bootstrap_try!(
            boot_rpc(
                &ws_send,
                &pending,
                &next_id,
                "Target.createTarget",
                json!({"url": "about:blank"}),
                None,
            )
            .await
        );
        let target_id = bootstrap_try!(res
            .get("targetId")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ActantError::Internal("Target.createTarget: missing targetId".into()))
            .map(|s| s.to_string()));

        // 2. Attach to it.
        let res = bootstrap_try!(
            boot_rpc(
                &ws_send,
                &pending,
                &next_id,
                "Target.attachToTarget",
                json!({"targetId": target_id, "flatten": true}),
                None,
            )
            .await
        );
        let session_id = bootstrap_try!(res
            .get("sessionId")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ActantError::Internal(
                "Target.attachToTarget: missing sessionId".into()
            ))
            .map(|s| s.to_string()));

        // 3. Enable Page / Runtime in that session.
        bootstrap_try!(
            boot_rpc(
                &ws_send,
                &pending,
                &next_id,
                "Page.enable",
                json!({}),
                Some(&session_id),
            )
            .await
        );
        bootstrap_try!(
            boot_rpc(
                &ws_send,
                &pending,
                &next_id,
                "Runtime.enable",
                json!({}),
                Some(&session_id),
            )
            .await
        );

        // Unwrap the ws_send Arc — by this point all `boot_rpc` borrows
        // have returned, so the local binding is the only strong ref.
        let ws_send_mutex = Arc::try_unwrap(ws_send).map_err(|_| {
            ActantError::Internal("ws_send arc still shared after bootstrap".into())
        })?;

        let inner = Arc::new(Inner {
            child: Mutex::new(Some(child)),
            session_id,
            target_id,
            ws_send: ws_send_mutex,
            pending,
            next_id: AtomicU64::new(next_id.load(Ordering::SeqCst)),
            user_data_dir: tmp,
            pump: Mutex::new(Some(pump)),
        });

        Ok(CdpDriver { inner })
    }

    /// Cleanly shut down: close the page target, terminate Chrome, await its
    /// exit, then remove the temporary user-data-dir.
    pub async fn close(&self) -> Result<(), ActantError> {
        // Best-effort target close (ignore errors — the browser may already
        // be on its way down).
        let _ = self
            .rpc(
                "Target.closeTarget",
                json!({"targetId": self.inner.target_id}),
                None,
            )
            .await;

        // Drop the send half so the pump sees EOF.
        {
            let mut guard = self.inner.ws_send.lock().await;
            guard.take();
        }
        if let Some(handle) = self.inner.pump.lock().await.take() {
            handle.abort();
            let _ = handle.await;
        }
        {
            let mut guard = self.inner.child.lock().await;
            if let Some(mut child) = guard.take() {
                let _ = child.kill().await;
                let _ = child.wait().await;
            }
        }
        let _ = std::fs::remove_dir_all(&self.inner.user_data_dir);
        Ok(())
    }

    async fn rpc(
        &self,
        method: &str,
        params: Value,
        session_id: Option<&str>,
    ) -> Result<Value, ActantError> {
        let id = self.inner.next_id.fetch_add(1, Ordering::SeqCst);
        let mut frame = json!({"id": id, "method": method, "params": params});
        if let Some(s) = session_id {
            frame["sessionId"] = Value::String(s.to_string());
        }
        let (tx, rx) = oneshot::channel();
        self.inner.pending.lock().await.insert(id, tx);
        {
            let mut guard = self.inner.ws_send.lock().await;
            let sink = guard
                .as_mut()
                .ok_or_else(|| ActantError::Internal("cdp ws send half closed".into()))?;
            sink.send(Message::Text(frame.to_string()))
                .await
                .map_err(|e| ActantError::Internal(format!("cdp ws send: {e}")))?;
        }
        let reply = timeout(RPC_TIMEOUT, rx)
            .await
            .map_err(|_| ActantError::Internal(format!("cdp rpc {method} timed out")))?
            .map_err(|_| ActantError::Internal(format!("cdp rpc {method} dropped")))?;
        if let Some(err) = reply.get("error") {
            return Err(ActantError::Internal(format!(
                "cdp rpc {method} error: {err}"
            )));
        }
        Ok(reply.get("result").cloned().unwrap_or(Value::Null))
    }

    async fn navigate(&self, url: &str) -> Result<Value, ActantError> {
        let session = self.inner.session_id.clone();
        self.rpc("Page.navigate", json!({"url": url}), Some(&session))
            .await?;
        // Poll readyState until "complete" or 20s deadline.
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        loop {
            let r = self
                .rpc(
                    "Runtime.evaluate",
                    json!({
                        "expression": "document.readyState",
                        "returnByValue": true,
                    }),
                    Some(&session),
                )
                .await?;
            let state = r
                .get("result")
                .and_then(|v| v.get("value"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if state == "complete" {
                break;
            }
            if std::time::Instant::now() >= deadline {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        let t = self
            .rpc(
                "Runtime.evaluate",
                json!({"expression": "document.title", "returnByValue": true}),
                Some(&session),
            )
            .await?;
        let title = t
            .get("result")
            .and_then(|v| v.get("value"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        Ok(json!({"title": title, "url": url}))
    }

    async fn click(&self, selector: &str) -> Result<Value, ActantError> {
        let session = self.inner.session_id.clone();
        let expr = format!(
            "(() => {{ const el = document.querySelector({}); \
              if (!el) {{ throw new Error('selector not found'); }} \
              el.click(); return true; }})()",
            json_string(selector),
        );
        let r = self
            .rpc(
                "Runtime.evaluate",
                json!({"expression": expr, "returnByValue": true}),
                Some(&session),
            )
            .await?;
        if let Some(exc) = r.get("exceptionDetails") {
            return Err(ActantError::Internal(format!("click failed: {exc}")));
        }
        Ok(json!({"clicked": selector}))
    }

    async fn type_text(&self, selector: &str, text: &str) -> Result<Value, ActantError> {
        let session = self.inner.session_id.clone();
        let expr = format!(
            "(() => {{ const el = document.querySelector({}); \
              if (!el) {{ throw new Error('selector not found'); }} \
              el.focus(); el.value = {}; \
              el.dispatchEvent(new Event('input', {{ bubbles: true }})); \
              el.dispatchEvent(new Event('change', {{ bubbles: true }})); \
              return true; }})()",
            json_string(selector),
            json_string(text),
        );
        let r = self
            .rpc(
                "Runtime.evaluate",
                json!({"expression": expr, "returnByValue": true}),
                Some(&session),
            )
            .await?;
        if let Some(exc) = r.get("exceptionDetails") {
            return Err(ActantError::Internal(format!("type failed: {exc}")));
        }
        Ok(json!({"typed": text, "into": selector}))
    }

    async fn screenshot(&self) -> Result<Value, ActantError> {
        let session = self.inner.session_id.clone();
        let r = self
            .rpc(
                "Page.captureScreenshot",
                json!({"format": "png"}),
                Some(&session),
            )
            .await?;
        let b64 = r
            .get("data")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ActantError::Internal("captureScreenshot: missing data".into()))?;
        // Bytes = base64-decoded length.
        let pad = b64.bytes().rev().take_while(|&b| b == b'=').count();
        let bytes = if b64.is_empty() {
            0
        } else {
            (b64.len() / 4) * 3 - pad
        };
        // Page dimensions for structural comparison (used by parity tests).
        let dims = self
            .rpc(
                "Runtime.evaluate",
                json!({
                    "expression": "JSON.stringify({w: window.innerWidth, h: window.innerHeight})",
                    "returnByValue": true,
                }),
                Some(&session),
            )
            .await?;
        let (w, h) = dims
            .get("result")
            .and_then(|v| v.get("value"))
            .and_then(|v| v.as_str())
            .and_then(|s| serde_json::from_str::<Value>(s).ok())
            .map(|v| {
                (
                    v.get("w").and_then(|x| x.as_u64()).unwrap_or(0),
                    v.get("h").and_then(|x| x.as_u64()).unwrap_or(0),
                )
            })
            .unwrap_or((0, 0));
        Ok(json!({
            "bytes": bytes,
            "width": w,
            "height": h,
            "format": "png",
        }))
    }
}

#[async_trait]
impl Driver for CdpDriver {
    async fn run(&self, action: Action) -> Result<Value, ActantError> {
        match action {
            Action::Navigate(url) => self.navigate(&url).await,
            Action::Click(sel) => self.click(&sel).await,
            Action::Type(sel, text) => self.type_text(&sel, &text).await,
            Action::Screenshot => self.screenshot().await,
        }
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        // Best-effort cleanup if `close()` was not called. `kill_on_drop` on
        // the `Command` handles the child; we just remove the temp dir.
        let _ = std::fs::remove_dir_all(&self.user_data_dir);
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Send one CDP RPC frame during bootstrap (before we have a `CdpDriver`).
async fn boot_rpc(
    ws_send: &Arc<Mutex<Option<WsSink>>>,
    pending: &Arc<PendingMap>,
    next_id: &Arc<AtomicU64>,
    method: &str,
    params: Value,
    session_id: Option<&str>,
) -> Result<Value, ActantError> {
    let id = next_id.fetch_add(1, Ordering::SeqCst);
    let mut frame = json!({"id": id, "method": method, "params": params});
    if let Some(s) = session_id {
        frame["sessionId"] = Value::String(s.to_string());
    }
    let (tx, rx) = oneshot::channel();
    pending.lock().await.insert(id, tx);
    {
        let mut g = ws_send.lock().await;
        let sink = g
            .as_mut()
            .ok_or_else(|| ActantError::Internal("cdp ws send half closed".into()))?;
        sink.send(Message::Text(frame.to_string()))
            .await
            .map_err(|e| ActantError::Internal(format!("cdp ws send: {e}")))?;
    }
    let reply = timeout(RPC_TIMEOUT, rx)
        .await
        .map_err(|_| ActantError::Internal(format!("cdp rpc {method} timed out")))?
        .map_err(|_| ActantError::Internal(format!("cdp rpc {method} dropped")))?;
    if let Some(err) = reply.get("error") {
        return Err(ActantError::Internal(format!(
            "cdp rpc {method} error: {err}"
        )));
    }
    Ok(reply.get("result").cloned().unwrap_or(Value::Null))
}

/// Resolve Chrome / Chromium binary via env var, then PATH, then well-known
/// macOS bundle locations.
fn resolve_chrome_binary() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("CHROME_PATH") {
        if !p.is_empty() {
            let pb = PathBuf::from(p);
            if pb.exists() {
                return Some(pb);
            }
        }
    }
    for name in [
        "google-chrome",
        "google-chrome-stable",
        "chromium",
        "chromium-browser",
        "chrome",
    ] {
        if let Some(p) = which_on_path(name) {
            return Some(p);
        }
    }
    for bundle in [
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        "/Applications/Chromium.app/Contents/MacOS/Chromium",
        "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
    ] {
        let p = PathBuf::from(bundle);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

fn which_on_path(prog: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(prog);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// Read stderr line by line until we see `DevTools listening on ws://...`.
async fn parse_devtools_url(stderr: tokio::process::ChildStderr) -> Result<String, ActantError> {
    let mut reader = BufReader::new(stderr).lines();
    loop {
        match reader.next_line().await {
            Ok(Some(line)) => {
                tracing::trace!(target: "actant_workers::browser::cdp", line = %line, "chrome-stderr");
                if let Some(idx) = line.find("ws://") {
                    let rest = &line[idx..];
                    let end = rest
                        .find(|c: char| c.is_whitespace())
                        .unwrap_or(rest.len());
                    return Ok(rest[..end].to_string());
                }
            }
            Ok(None) => {
                return Err(ActantError::Internal(
                    "chrome stderr closed before DevTools URL".into(),
                ));
            }
            Err(e) => {
                return Err(ActantError::Internal(format!("chrome stderr read: {e}")));
            }
        }
    }
}

/// JSON-encode a string as a JS literal — delegate to `serde_json` for
/// escape correctness.
fn json_string(s: &str) -> String {
    Value::String(s.to_string()).to_string()
}

/// Monotonic, unique-enough suffix for temp dirs without pulling in `uuid`.
fn timestamp_suffix() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{nanos:032x}")
}
