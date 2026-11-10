//! Optional Godot connection: test a project-declared Godot MCP server over stdio and open the
//! editor on request. Servers are started only when the user asks, from the project's own
//! declaration; arguments, environment values and tokens are never returned or logged.
use serde::Serialize;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

const PROBE_LIMIT: Duration = Duration::from_secs(25);
const RESULT_CHARS: usize = 4000;
const CONNECT_WAIT: Duration = Duration::from_secs(12);

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct GodotConnection {
    pub server: String,
    /// The server process started and answered `initialize`.
    pub started: bool,
    /// Name and version the server reported.
    pub server_info: String,
    pub tool_count: usize,
    /// `get_project_info` answered without an error, so the editor is connected.
    pub editor_connected: bool,
    /// Bounded text of `get_project_info`, or the reason it failed.
    pub detail: String,
}

/// Replaces `${NAME}` and `${NAME:-default}` with environment values, as the CLIs do.
fn expand(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let Some(end) = rest[start..].find('}') else {
            out.push_str(&rest[start..]);
            return out;
        };
        let inner = &rest[start + 2..start + end];
        let (name, default) = inner.split_once(":-").unwrap_or((inner, ""));
        out.push_str(&std::env::var(name).unwrap_or_else(|_| default.to_string()));
        rest = &rest[start + end + 1..];
    }
    out.push_str(rest);
    out
}

/// Command, arguments and environment of a stdio server.
type Launch = (String, Vec<String>, Vec<(String, String)>);

/// A stdio server declared in `.mcp.json` or `.codex/config.toml`.
fn declared(root: &Path, name: &str) -> Option<Launch> {
    let from_json = std::fs::read_to_string(root.join(".mcp.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v["mcpServers"].get(name).cloned());
    let from_toml = || {
        std::fs::read_to_string(root.join(".codex/config.toml"))
            .ok()
            .and_then(|t| t.parse::<toml::Table>().ok())
            .and_then(|t| t.get("mcp_servers")?.get(name).cloned())
            .and_then(|v| serde_json::to_value(v).ok())
    };
    let server = from_json.or_else(from_toml)?;
    let command = expand(server["command"].as_str()?);
    let args = server["args"]
        .as_array()
        .map(|a| a.iter().filter_map(|x| x.as_str()).map(expand).collect())
        .unwrap_or_default();
    let env = server["env"]
        .as_object()
        .map(|o| {
            o.iter()
                .filter_map(|(k, v)| Some((k.clone(), expand(v.as_str()?))))
                .collect()
        })
        .unwrap_or_default();
    Some((command, args, env))
}

/// Godot-named stdio servers declared by the project, in declaration order.
pub fn godot_servers(root: &Path) -> Vec<String> {
    let mut names: Vec<String> = crate::projecttools::discover_tools(root)
        .into_iter()
        .filter(|e| e.kind == "mcp" && e.name.to_lowercase().contains("godot"))
        .map(|e| e.name)
        .collect();
    names.dedup();
    names
}

async fn request(
    stdin: &mut tokio::process::ChildStdin,
    lines: &mut tokio::io::Lines<BufReader<tokio::process::ChildStdout>>,
    id: u64,
    method: &str,
    params: Value,
) -> Result<Value, String> {
    let msg = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
    stdin
        .write_all(format!("{msg}\n").as_bytes())
        .await
        .map_err(|_| "The server closed its input")?;
    while let Some(line) = lines.next_line().await.map_err(|e| e.to_string())? {
        let Ok(v) = serde_json::from_str::<Value>(&line) else {
            continue; // log output on stdout is not part of the protocol
        };
        if v["id"].as_u64() != Some(id) {
            continue;
        }
        if let Some(e) = v.get("error") {
            return Err(e["message"].as_str().unwrap_or("Server error").to_string());
        }
        return Ok(v["result"].clone());
    }
    Err("The server exited before answering".into())
}

fn tool_text(result: &Value) -> String {
    result["content"]
        .as_array()
        .map(|c| {
            c.iter()
                .filter_map(|p| p["text"].as_str())
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
        .chars()
        .take(RESULT_CHARS)
        .collect()
}

async fn session(
    child: &mut tokio::process::Child,
    report: &mut GodotConnection,
) -> Result<(), String> {
    let mut stdin = child.stdin.take().ok_or("No input stream")?;
    let mut lines = BufReader::new(child.stdout.take().ok_or("No output stream")?).lines();
    let init = request(
        &mut stdin,
        &mut lines,
        1,
        "initialize",
        json!({"protocolVersion": "2025-06-18", "capabilities": {},
               "clientInfo": {"name": "lumen", "version": env!("CARGO_PKG_VERSION")}}),
    )
    .await?;
    report.started = true;
    report.server_info = format!(
        "{} {}",
        init["serverInfo"]["name"].as_str().unwrap_or("MCP server"),
        init["serverInfo"]["version"].as_str().unwrap_or("")
    )
    .trim()
    .to_string();
    let note = json!({"jsonrpc": "2.0", "method": "notifications/initialized"});
    let _ = stdin.write_all(format!("{note}\n").as_bytes()).await;
    let tools = request(&mut stdin, &mut lines, 2, "tools/list", json!({})).await?;
    let names: Vec<&str> = tools["tools"]
        .as_array()
        .map(|t| t.iter().filter_map(|x| x["name"].as_str()).collect())
        .unwrap_or_default();
    report.tool_count = names.len();
    if !names.contains(&"get_project_info") {
        report.detail = "The server does not offer get_project_info.".into();
        return Ok(());
    }
    // The editor addon connects to a freshly started server on its own reconnect timer, so a
    // "not connected" answer is retried for a few seconds before it is reported.
    let deadline = std::time::Instant::now() + CONNECT_WAIT;
    for id in 3.. {
        let info = request(
            &mut stdin,
            &mut lines,
            id,
            "tools/call",
            json!({"name": "get_project_info", "arguments": {}}),
        )
        .await?;
        report.detail = tool_text(&info);
        report.editor_connected = info["isError"] != true && !report.detail.is_empty();
        if report.editor_connected || std::time::Instant::now() >= deadline {
            break;
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    Ok(())
}

/// Starts the declared server, reads `get_project_info` (read-only), then stops it.
pub async fn probe(root: &Path, name: &str) -> Result<GodotConnection, String> {
    let (command, args, env) =
        declared(root, name).ok_or("This server is not declared as a local stdio server")?;
    let program = which::which(&command).unwrap_or_else(|_| PathBuf::from(&command));
    let mut c = crate::proc::command(program);
    c.args(&args)
        .envs(env)
        .current_dir(root)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);
    #[cfg(unix)]
    c.process_group(0);
    let mut child = c
        .spawn()
        .map_err(|e| format!("Could not start {command}: {e}"))?;
    let mut report = GodotConnection {
        server: name.to_string(),
        ..Default::default()
    };
    let outcome = tokio::time::timeout(PROBE_LIMIT, session(&mut child, &mut report)).await;
    crate::providers::terminate(&mut child).await;
    match outcome {
        Ok(Ok(())) => {}
        Ok(Err(e)) => report.detail = e,
        Err(_) => report.detail = "The server did not answer within 25 seconds.".into(),
    }
    if report.started && !report.editor_connected && report.detail.is_empty() {
        report.detail =
            "The editor did not answer. Open the project in Godot with the addon enabled.".into();
    }
    Ok(report)
}

/// Godot executables on PATH, then the usual per-user install folders.
pub fn find_executable() -> Option<String> {
    let names: &[&str] = if cfg!(windows) {
        &[
            "godot.exe",
            "godot4.exe",
            "godot-mono.exe",
            "godot4-mono.exe",
        ]
    } else {
        &["godot", "godot4", "godot-mono", "godot4-mono"]
    };
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            for name in names {
                if dir.join(name).is_file() {
                    return Some(dir.join(name).to_string_lossy().into_owned());
                }
            }
        }
    }
    let mut roots: Vec<PathBuf> = vec![];
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        roots.push(PathBuf::from(local).join("Programs/Godot"));
    }
    if cfg!(target_os = "macos") {
        roots.push(PathBuf::from("/Applications"));
    }
    let mut found = vec![];
    for root in roots {
        collect(&root, 0, &mut found);
    }
    // The newest version first; the console build is for terminals, not the editor.
    found.sort();
    found.pop()
}

fn collect(dir: &Path, depth: usize, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_lowercase();
        let editor = name.starts_with("godot")
            && !name.contains("console")
            && (name.ends_with(".exe") || name.ends_with(".app"));
        if editor {
            out.push(path.to_string_lossy().into_owned());
        } else if path.is_dir() && depth < 3 {
            collect(&path, depth + 1, out);
        }
    }
}

fn canonical_root(folder: &str) -> Result<PathBuf, String> {
    let root = dunce::canonicalize(folder).map_err(|_| "Choose an existing folder")?;
    if root.is_dir() {
        Ok(root)
    } else {
        Err("Choose an existing folder".into())
    }
}

#[derive(Clone, Default, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VisualControls {
    pub enabled: bool,
    pub server: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VisualResult {
    available: Vec<String>,
    detail: String,
    screenshot: Option<crate::screens::Screenshot>,
}
#[tauri::command]
pub fn godot_visual_settings(folder: String) -> Result<VisualControls, String> {
    let root = canonical_root(&folder)?;
    let path = root.join(".lumen/visual-controls.json");
    if !path.exists() {
        return Ok(VisualControls::default());
    }
    let path = crate::context::jail(&root, ".lumen/visual-controls.json")?;
    serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub fn set_godot_visual_settings(
    folder: String,
    value: VisualControls,
) -> Result<VisualControls, String> {
    let root = canonical_root(&folder)?;
    if value.enabled && !godot_servers(&root).contains(&value.server) {
        return Err("Choose a Godot server declared in this project".into());
    }
    let dir = root.join(".lumen");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let dir = dunce::canonicalize(dir).map_err(|e| e.to_string())?;
    if !dir.starts_with(&root) {
        return Err("Project settings folder is outside the project".into());
    }
    let file = dir.join(format!("visual-controls-{}.tmp", uuid::Uuid::new_v4()));
    std::fs::write(
        &file,
        serde_json::to_vec_pretty(&value).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    if let Err(e) = std::fs::rename(&file, dir.join("visual-controls.json")) {
        let _ = std::fs::remove_file(file);
        return Err(e.to_string());
    }
    Ok(value)
}
fn connected_path(result: &Value) -> Option<PathBuf> {
    let text = tool_text(result);
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    let value: Value = serde_json::from_str(&text[start..=end]).ok()?;
    let path = [
        value.get("project_path"),
        value.get("projectPath"),
        value.get("path"),
        value.pointer("/project/path"),
    ]
    .into_iter()
    .flatten()
    .find_map(Value::as_str)
    .and_then(|s| dunce::canonicalize(s).ok());
    path
}
fn visual_tool(action: &str) -> Option<&'static str> {
    match action {
        "play" => Some("play_scene"),
        "reload" => Some("reload_project"),
        "stop" => Some("stop_scene"),
        "capture" => Some("get_game_screenshot"),
        _ => None,
    }
}
async fn visual_session(
    root: &Path,
    child: &mut tokio::process::Child,
    action: &str,
    scene: &str,
) -> Result<VisualResult, String> {
    let mut stdin = child.stdin.take().ok_or("No Godot MCP input")?;
    let mut lines = BufReader::new(child.stdout.take().ok_or("No Godot MCP output")?).lines();
    request(&mut stdin,&mut lines,1,"initialize",json!({"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"lumen","version":env!("CARGO_PKG_VERSION")}})).await?;
    stdin
        .write_all(
            format!(
                "{}\n",
                json!({"jsonrpc":"2.0","method":"notifications/initialized"})
            )
            .as_bytes(),
        )
        .await
        .map_err(|e| e.to_string())?;
    let list = request(&mut stdin, &mut lines, 2, "tools/list", json!({})).await?;
    let tools = list["tools"]
        .as_array()
        .ok_or("Godot did not return a tool list")?;
    let available = ["play", "reload", "stop", "capture"]
        .iter()
        .filter(|a| tools.iter().any(|t| t["name"] == visual_tool(a).unwrap()))
        .map(|a| a.to_string())
        .collect();
    if !tools.iter().any(|t| t["name"] == "get_project_info") {
        return Err(
            "The server cannot verify its connected project; no visual action was sent".into(),
        );
    }
    let info = request(
        &mut stdin,
        &mut lines,
        3,
        "tools/call",
        json!({"name":"get_project_info","arguments":{}}),
    )
    .await?;
    let connected = connected_path(&info)
        .ok_or("The connected editor project could not be verified; no visual action was sent")?;
    if !connected.starts_with(root) {
        return Err("The editor is connected to a different project. Open this project's editor before using visual controls.".into());
    }
    if action == "discover" {
        return Ok(VisualResult {
            available,
            detail: "Connected editor matches this project".into(),
            screenshot: None,
        });
    }
    let name = visual_tool(action).ok_or("Unsupported Godot visual action")?;
    let tool = tools
        .iter()
        .find(|t| t["name"] == name)
        .ok_or("The installed Godot server does not offer this action")?;
    let mut arguments = json!({});
    let schema = &tool["inputSchema"];
    if action == "play" {
        if !scene.is_empty() {
            let rel = scene
                .strip_prefix("res://")
                .ok_or("Use a res:// scene path")?;
            let path = crate::context::jail(&connected, rel)?;
            if !path.extension().is_some_and(|e| e == "tscn" || e == "scn") {
                return Err("Choose a Godot scene".into());
            }
            let key = ["scene_path", "path", "scene"]
                .iter()
                .find(|key| schema["properties"].get(**key).is_some())
                .ok_or("This server does not accept a scene path")?;
            arguments[*key] = json!(scene);
        }
        if schema["properties"].get("mode").is_some() {
            let preferred = if scene.is_empty() {
                vec!["main"]
            } else {
                vec!["specific", "custom", "path"]
            };
            let mode = preferred
                .into_iter()
                .find(|m| {
                    schema["properties"]["mode"]["enum"]
                        .as_array()
                        .is_none_or(|items| items.iter().any(|i| i == m))
                })
                .ok_or("This server does not offer the selected play mode")?;
            arguments["mode"] = json!(mode);
        }
    }
    if let Some(required) = schema["required"].as_array() {
        for key in required.iter().filter_map(Value::as_str) {
            if arguments.get(key).is_none() {
                return Err(format!(
                    "The installed tool requires '{key}'; no action was sent"
                ));
            }
        }
    }
    let result = request(
        &mut stdin,
        &mut lines,
        4,
        "tools/call",
        json!({"name":name,"arguments":arguments}),
    )
    .await?;
    if result["isError"] == true {
        return Err(
            "Godot rejected the action. Check the editor connection and selected scene.".into(),
        );
    }
    let screenshot = if action == "capture" {
        let image = result["content"]
            .as_array()
            .and_then(|items| items.iter().find(|i| i["type"] == "image"))
            .ok_or("Godot returned no image")?;
        Some(crate::screens::save_mcp_image(
            root,
            image["data"]
                .as_str()
                .ok_or("Godot returned invalid image data")?,
            "Godot game viewport",
        )?)
    } else {
        None
    };
    Ok(VisualResult {
        available,
        detail: format!("Godot {action} completed"),
        screenshot,
    })
}
#[tauri::command]
pub async fn godot_visual_action(
    folder: String,
    action: String,
    scene: String,
) -> Result<VisualResult, String> {
    if action != "discover" && visual_tool(&action).is_none() {
        return Err("Unsupported Godot visual action".into());
    }
    let root = canonical_root(&folder)?;
    let settings = godot_visual_settings(folder)?;
    if !settings.enabled {
        return Err("Enable visual controls for this project first".into());
    }
    let (command, args, env) =
        declared(&root, &settings.server).ok_or("The selected Godot MCP server is not declared")?;
    let program = which::which(&command).unwrap_or_else(|_| PathBuf::from(command));
    let mut c = crate::proc::command(program);
    c.args(args)
        .envs(env)
        .current_dir(&root)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);
    #[cfg(unix)]
    c.process_group(0);
    let mut child = c.spawn().map_err(|e| e.to_string())?;
    let result = tokio::time::timeout(
        PROBE_LIMIT,
        visual_session(&root, &mut child, &action, &scene),
    )
    .await
    .map_err(|_| "Godot visual action timed out".to_string());
    crate::providers::terminate(&mut child).await;
    result?
}

#[tauri::command]
pub async fn godot_connection_test(
    folder: String,
    server: String,
) -> Result<GodotConnection, String> {
    let root = canonical_root(&folder)?;
    if !godot_servers(&root).contains(&server) {
        return Err("Choose a Godot server declared in this project".into());
    }
    probe(&root, &server).await
}

/// Opens a project's editor. `project` is a `project.godot` path relative to the folder.
#[tauri::command]
pub async fn open_godot_editor(folder: String, project: String) -> Result<(), String> {
    let root = canonical_root(&folder)?;
    let file = dunce::canonicalize(root.join(&project)).map_err(|_| "Project file not found")?;
    if !file.starts_with(&root) || file.file_name().is_none_or(|n| n != "project.godot") {
        return Err("Choose a project.godot inside this folder".into());
    }
    let exe = find_executable().ok_or("Godot is not installed where Lumen can find it")?;
    let dir = file.parent().ok_or("Project file not found")?;
    let program = if exe.ends_with(".app") {
        format!("{exe}/Contents/MacOS/Godot")
    } else {
        exe
    };
    std::process::Command::new(program)
        .args(["--editor", "--path"])
        .arg(dir)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("Could not open Godot: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_environment_references() {
        std::env::set_var("LUMEN_GODOT_TEST", "value");
        assert_eq!(expand("a ${LUMEN_GODOT_TEST} b"), "a value b");
        assert_eq!(expand("${LUMEN_MISSING_VAR:-fallback}"), "fallback");
        assert_eq!(expand("plain ${unclosed"), "plain ${unclosed");
    }

    #[test]
    fn reads_stdio_declarations_from_either_file() {
        let root = std::env::temp_dir().join(format!("lumen-godot-{}", std::process::id()));
        std::fs::create_dir_all(root.join(".codex")).unwrap();
        std::fs::write(
            root.join(".mcp.json"),
            r#"{"mcpServers":{"godot-pro":{"command":"node","args":["server.js"],"env":{"HOST":"127.0.0.1"}},"web":{"type":"http","url":"https://x"}}}"#,
        )
        .unwrap();
        std::fs::write(
            root.join(".codex/config.toml"),
            "[mcp_servers.godot-codex]\ncommand = \"node\"\nargs = [\"b.js\"]\n",
        )
        .unwrap();
        let (cmd, args, env) = declared(&root, "godot-pro").unwrap();
        assert_eq!(
            (cmd.as_str(), args, env.len()),
            ("node", vec!["server.js".to_string()], 1)
        );
        assert!(declared(&root, "godot-codex").is_some());
        assert!(
            declared(&root, "web").is_none(),
            "HTTP servers are not started"
        );
        assert_eq!(godot_servers(&root), vec!["godot-codex", "godot-pro"]);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Live check against the project's real server: `LUMEN_GODOT_PROJECT=<folder>`.
    #[tokio::test]
    #[ignore]
    async fn live_godot_project_info() {
        let folder = std::env::var("LUMEN_GODOT_PROJECT").expect("LUMEN_GODOT_PROJECT");
        let root = PathBuf::from(folder);
        let server = godot_servers(&root)
            .into_iter()
            .next()
            .expect("a Godot server");
        let report = probe(&root, &server).await.unwrap();
        println!("{report:?}");
        assert!(report.started, "{}", report.detail);
    }
}
