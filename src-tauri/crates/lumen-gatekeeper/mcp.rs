//! One-use, session-bound Claude MCP endpoint plus a small stdio proxy.
//! No HTTP, shell parsing, bearer tokens in command arguments, or remote hosts.
use crate::{Availability, Delivery, Gatekeeper, Provider, Scope};
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::io::{
    AsyncBufRead, AsyncBufReadExt, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader,
};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, watch};
use tokio::task::JoinSet;

const MAX_FRAME: usize = 2 * 1024 * 1024;
const PROTOCOLS: &[&str] = &["2024-11-05", "2025-03-26", "2025-06-18"];

pub struct ClaudeEndpoint {
    listener: TcpListener,
    token: String,
    keeper: Gatekeeper,
    scope: Scope,
    ready: watch::Sender<bool>,
}

impl ClaudeEndpoint {
    pub async fn bind(keeper: Gatekeeper, scope: Scope) -> Result<Self, String> {
        if scope.provider != Provider::Claude
            || !keeper.sessions()?.iter().any(|s| {
                s.scope == scope && matches!(s.availability, Availability::Available { .. })
            })
        {
            return Err("No supported Claude session for MCP endpoint".into());
        }
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|e| e.to_string())?;
        let (ready, _) = watch::channel(false);
        Ok(Self {
            listener,
            token: format!(
                "{}{}",
                uuid::Uuid::new_v4().simple(),
                uuid::Uuid::new_v4().simple()
            ),
            keeper,
            scope,
            ready,
        })
    }

    /// True after the actual MCP client initializes and discovers the tool.
    pub fn readiness(&self) -> watch::Receiver<bool> {
        self.ready.subscribe()
    }

    /// Pass this object via --mcp-config, keeping it out of logs and persistent
    /// settings. The executable is either the packaged helper or Lumen itself
    /// with an early main() branch calling proxy_stdio().
    pub fn config(&self, executable: &Path, embedded: bool) -> Result<Value, String> {
        if !executable.is_absolute() {
            return Err("MCP bridge executable must be absolute".into());
        }
        Ok(json!({"mcpServers":{"lumen_gatekeeper":{
            "type":"stdio", "command":executable.to_string_lossy(),
            "args":if embedded { vec!["--lumen-gatekeeper-mcp"] } else { vec![] },
            "env":{
                "LUMEN_GATEKEEPER_ADDR":self.listener.local_addr().map_err(|e|e.to_string())?.to_string(),
                "LUMEN_GATEKEEPER_TOKEN":self.token
            }
        }}}))
    }

    /// Each endpoint accepts one authenticated helper. The host must abort this
    /// task and stop the child on cancellation; disconnect also runs on errors.
    pub async fn serve(self) -> Result<(), String> {
        let result = self.accept_and_serve().await;
        let reason = match &result {
            Ok(()) => "Claude MCP connection closed",
            Err(_) => "Claude MCP transport failed; verify provider process state",
        };
        self.keeper.disconnect(&self.scope, reason)?;
        result
    }

    async fn accept_and_serve(&self) -> Result<(), String> {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
        let mut reader = loop {
            let (stream, address) = tokio::time::timeout_at(deadline, self.listener.accept())
                .await
                .map_err(|_| "Claude MCP bridge did not connect within 30 seconds")?
                .map_err(|e| e.to_string())?;
            if !address.ip().is_loopback() {
                continue;
            }
            let mut reader = BufReader::new(stream);
            let auth = tokio::time::timeout(Duration::from_secs(2), read_frame(&mut reader)).await;
            let valid = matches!(auth, Ok(Ok(Some(ref v))) if v.get("token").and_then(Value::as_str).is_some_and(|s| equal_secret(s, &self.token)));
            if valid {
                break reader;
            }
            if tokio::time::Instant::now() >= deadline {
                return Err("Claude MCP authentication timed out".into());
            }
        };
        // No more helper connections can attach to this capability.
        let stream = reader.get_mut();
        stream.set_nodelay(true).map_err(|e| e.to_string())?;
        // Preserve any bytes already buffered after the authentication frame.
        let (out_tx, mut out_rx) = mpsc::channel::<Delivery>(128);
        let mut jobs = JoinSet::new();
        let mut initialized = false;
        let mut ready = false;
        let mut incoming_buffer = Vec::new();
        loop {
            tokio::select! {
                outgoing = out_rx.recv() => {
                    if let Some(delivery) = outgoing {
                        if !self.keeper.delivery_is_current(&delivery)? { continue; }
                        let written = write_frame(reader.get_mut(), &delivery.response).await;
                        self.keeper.complete_delivery(&delivery, written.is_ok())?;
                        written?;
                    }
                }
                completed = jobs.join_next(), if !jobs.is_empty() => {
                    let result = completed.ok_or("MCP reply task disappeared")?.map_err(|e| e.to_string())?;
                    result?;
                }
                incoming = read_frame_into(&mut reader, &mut incoming_buffer) => {
                    let Some(message) = incoming? else { break; };
                    if message["jsonrpc"] != "2.0" { return Err("Invalid MCP JSON-RPC envelope".into()); }
                    let method = message.get("method").and_then(Value::as_str).unwrap_or("");
                    if method == "notifications/cancelled" {
                        if !ready { return Err("MCP cancellation before initialization".into()); }
                        let id = message["params"].get("requestId").ok_or("Missing MCP cancellation ID")?;
                        self.keeper.cancel_wire(&self.scope, id)?;
                        continue;
                    }
                    if method == "notifications/initialized" && initialized {
                        ready = true;
                        continue;
                    }
                    let id = crate::types::wire_id(&message)?;
                    let reply = match method {
                        "initialize" if !initialized => {
                            let offered = message["params"]["protocolVersion"].as_str().ok_or("Missing MCP protocol version")?;
                            let protocol = if PROTOCOLS.contains(&offered) { offered } else { "2025-06-18" };
                            initialized = true;
                            json!({"jsonrpc":"2.0","id":id,"result":{
                                "protocolVersion":protocol,"capabilities":{"tools":{}},
                                "serverInfo":{"name":"lumen-gatekeeper","version":env!("CARGO_PKG_VERSION")}
                            }})
                        }
                        "ping" if initialized => json!({"jsonrpc":"2.0","id":id,"result":{}}),
                        "tools/list" if ready => {
                            self.ready.send_replace(true);
                            json!({"jsonrpc":"2.0","id":id,"result":{"tools":[permission_tool(),list_screens_tool(),capture_screen_tool()]}})
                        }
                        "tools/call" if ready => {
                            let tool_name = message["params"]["name"].as_str().unwrap_or("");
                            if tool_name == "list_screens" {
                                let result = list_screens().await;
                                match result {
                                    Ok(text) => json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":text}]} }),
                                    Err(error) => json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":error}],"isError":true} }),
                                }
                            } else if tool_name == "capture_screen" {
                                let root = self.keeper.sessions()?.into_iter()
                                    .find(|s| s.scope == self.scope)
                                    .map(|s| PathBuf::from(s.cwd))
                                    .ok_or("Claude session is no longer active")?;
                                let title = message["params"]["arguments"]["window_title"].as_str().map(str::to_owned);
                                match capture_screen(root, title).await {
                                    Ok((path, data)) => json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":format!("Screenshot saved to {path}")},{"type":"image","data":data,"mimeType":"image/jpeg"}]} }),
                                    Err(error) => json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":error}],"isError":true} }),
                                }
                            } else {
                            match self.keeper.request_claude(&self.scope, &message) {
                                Ok(pending) => {
                                    let tx = out_tx.clone();
                                    jobs.spawn(async move {
                                        let Ok(delivery) = pending.wait().await else { return Ok::<_,String>(()); };
                                        tx.send(delivery).await.map_err(|_| "MCP connection closed".to_string())?;
                                        Ok(())
                                    });
                                    continue;
                                }
                                Err(error) => rpc_error(id, -32602, &error),
                            }
                            }
                        }
                        _ => rpc_error(id, -32601, "Unsupported MCP method or initialization incomplete"),
                    };
                    write_frame(reader.get_mut(), &reply).await?;
                }
            }
        }
        jobs.abort_all();
        Ok(())
    }
}

impl Drop for ClaudeEndpoint {
    fn drop(&mut self) {
        let _ = self
            .keeper
            .disconnect(&self.scope, "Claude MCP endpoint stopped");
    }
}

fn equal_secret(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.bytes()
        .zip(b.bytes())
        .fold(0_u8, |difference, (a, b)| difference | (a ^ b))
        == 0
}

fn rpc_error(id: Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}})
}

pub fn permission_tool() -> Value {
    json!({
        "name":"approve",
        "description":"Ask the Lumen user to approve or deny this exact pending tool operation",
        "inputSchema":{
            "type":"object",
            "properties":{
                "tool_name":{"type":"string"},
                "input":{"type":"object","additionalProperties":true},
                "tool_use_id":{"type":"string"}
            },
            "required":["tool_name","input"],"additionalProperties":false
        }
    })
}

fn list_screens_tool() -> Value {
    json!({"name":"list_screens","description":"List available monitors and visible application windows. Call before capture_screen when choosing a particular window.","inputSchema":{"type":"object","properties":{},"additionalProperties":false}})
}

fn capture_screen_tool() -> Value {
    json!({"name":"capture_screen","description":"Capture the desktop only when visual inspection is needed for the user's task. Returns the image to you and saves it in the current project. Omit window_title for the primary monitor, or pass a visible window title (or unique substring) to capture only that window. Ask the user before capturing if the screen may show unrelated private information.","inputSchema":{"type":"object","properties":{"window_title":{"type":"string","description":"Optional visible window title or unique substring. Omit to capture the primary monitor."}},"additionalProperties":false}})
}

async fn list_screens() -> Result<String, String> {
    tokio::task::spawn_blocking(|| {
        let monitors = xcap::Monitor::all().map_err(|e| e.to_string())?.into_iter().map(|m| {
            Ok(json!({"type":"monitor","name":m.name().map_err(|e|e.to_string())?,"primary":m.is_primary().unwrap_or(false),"width":m.width().unwrap_or(0),"height":m.height().unwrap_or(0)}))
        }).collect::<Result<Vec<_>,String>>()?;
        let windows = xcap::Window::all().map_err(|e| e.to_string())?.into_iter().filter_map(|w| {
            let title = w.title().ok()?;
            if title.trim().is_empty() || w.is_minimized().unwrap_or(true) { return None; }
            Some(json!({"type":"window","title":title,"app":w.app_name().unwrap_or_default(),"width":w.width().unwrap_or(0),"height":w.height().unwrap_or(0)}))
        }).collect::<Vec<_>>();
        Ok(serde_json::to_string(&json!({"screens":monitors,"windows":windows})).map_err(|e|e.to_string())?)
    }).await.map_err(|e|e.to_string())?
}

async fn capture_screen(root: PathBuf, title: Option<String>) -> Result<(String, String), String> {
    tokio::task::spawn_blocking(move || {
        let root = std::fs::canonicalize(root).map_err(|e| e.to_string())?;
        let source_title = title.clone().filter(|s| !s.trim().is_empty());
        let image = if let Some(title) = source_title.clone() {
            let matches = xcap::Window::all().map_err(|e| e.to_string())?.into_iter().filter(|w| {
                w.title().ok().is_some_and(|t| t.to_lowercase().contains(&title.to_lowercase()))
                    && !w.is_minimized().unwrap_or(true)
            }).collect::<Vec<_>>();
            if matches.len() != 1 { return Err(if matches.is_empty() { "No visible window matched that title".into() } else { "More than one window matched; use a more specific title".into() }); }
            matches.into_iter().next().unwrap().capture_image().map_err(|e|e.to_string())?
        } else {
            let monitor = xcap::Monitor::all().map_err(|e| e.to_string())?.into_iter().find(|m| m.is_primary().unwrap_or(false)).ok_or("No primary monitor is available")?;
            monitor.capture_image().map_err(|e|e.to_string())?
        };
        let dir = root.join(".lumen/screenshots");
        std::fs::create_dir_all(&dir).map_err(|e|e.to_string())?;
        let ignore = dir.join(".gitignore");
        if !ignore.exists() { std::fs::write(ignore, "# Lumen screenshots stay local\n*\n").map_err(|e|e.to_string())?; }
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis();
        let file = format!("shot-{stamp}.png");
        image.save(dir.join(&file)).map_err(|e|e.to_string())?;
        let path = format!(".lumen/screenshots/{file}");
        let sidecar = json!({"path":path,"label":"Captured at the assistant's request","width":image.width(),"height":image.height(),"source":source_title.unwrap_or_else(||"Primary monitor".into()),"captured":stamp});
        std::fs::write(dir.join(format!("shot-{stamp}.json")), serde_json::to_vec_pretty(&sidecar).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
        let thumbnail = image::DynamicImage::ImageRgba8(image).thumbnail(1600, 1200).to_rgb8();
        let mut encoded = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut encoded, 78).encode_image(&image::DynamicImage::ImageRgb8(thumbnail)).map_err(|e|e.to_string())?;
        use base64::Engine as _;
        Ok((path, base64::engine::general_purpose::STANDARD.encode(encoded)))
    }).await.map_err(|e|e.to_string())?
}

pub async fn read_frame<R: AsyncBufRead + Unpin>(reader: &mut R) -> Result<Option<Value>, String> {
    let mut buffer = Vec::new();
    read_frame_into(reader, &mut buffer).await
}

// The buffer survives select! cancellation. Partial input must not disappear
// when an outgoing GUI decision wins the race against an incoming frame.
async fn read_frame_into<R: AsyncBufRead + Unpin>(
    reader: &mut R,
    buffer: &mut Vec<u8>,
) -> Result<Option<Value>, String> {
    let remaining = (MAX_FRAME + 1).saturating_sub(buffer.len());
    let size = (&mut *reader)
        .take(remaining as u64)
        .read_until(b'\n', buffer)
        .await
        .map_err(|e| e.to_string())?;
    if size == 0 && buffer.is_empty() {
        return Ok(None);
    }
    if buffer.len() > MAX_FRAME || buffer.last() != Some(&b'\n') {
        return Err("Oversized or incomplete protocol frame".into());
    }
    let result = serde_json::from_slice(buffer)
        .map(Some)
        .map_err(|e| format!("Invalid protocol JSON: {e}"));
    buffer.clear();
    result
}

pub async fn write_frame<W: AsyncWrite + Unpin>(
    writer: &mut W,
    value: &Value,
) -> Result<(), String> {
    let mut bytes = serde_json::to_vec(value).map_err(|e| e.to_string())?;
    if bytes.len() >= MAX_FRAME {
        return Err("Protocol reply too large".into());
    }
    bytes.push(b'\n');
    writer.write_all(&bytes).await.map_err(|e| e.to_string())?;
    writer.flush().await.map_err(|e| e.to_string())
}

/// Early executable entry point. It only proxies bytes to the app's one-use
/// local capability; it cannot approve requests by itself.
pub async fn proxy_stdio() -> Result<(), String> {
    let address: SocketAddr = std::env::var("LUMEN_GATEKEEPER_ADDR")
        .map_err(|_| "Missing bridge endpoint")?
        .parse()
        .map_err(|_| "Invalid bridge endpoint")?;
    if !address.ip().is_loopback() {
        return Err("Gatekeeper bridge only permits loopback addresses".into());
    }
    let token = std::env::var("LUMEN_GATEKEEPER_TOKEN").map_err(|_| "Missing bridge capability")?;
    if token.len() != 64 || !token.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err("Invalid bridge capability".into());
    }
    let mut stream = tokio::time::timeout(Duration::from_secs(5), TcpStream::connect(address))
        .await
        .map_err(|_| "Bridge connection timed out")?
        .map_err(|e| e.to_string())?;
    write_frame(&mut stream, &json!({"token":token})).await?;
    let (mut read, mut write) = stream.into_split();
    let mut stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    tokio::select! {
        result = tokio::io::copy(&mut stdin, &mut write) => { result.map_err(|e|e.to_string())?; }
        result = tokio::io::copy(&mut read, &mut stdout) => { result.map_err(|e|e.to_string())?; }
    }
    Ok(())
}
