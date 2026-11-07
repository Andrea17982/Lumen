//! Project-scoped helpers: read-only discovery of skills and MCP declarations, and project-defined
//! checks that run only when the user asks. Nothing here starts a server or reads a secret value.
use serde::Serialize;
use std::path::{Path, PathBuf};

const SKILL_DIRS: &[(&str, &str)] = &[
    (".claude/skills", "claude"),
    (".agents/skills", "gpt"),
    (".codex/skills", "gpt"),
    (".gemini/skills", "gemini"),
];
const SOURCE_EXTENSIONS: &[&str] = &[
    "cs", "gd", "ts", "tsx", "js", "rs", "py", "sql", "tscn", "tres", "go", "godot", "gdshader",
    "cfg",
];
const MAX_DEPTH: usize = 3;

#[derive(Serialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ToolEntry {
    /// `skill` or `mcp`.
    pub kind: String,
    pub name: String,
    pub provider: String,
    /// File that declares it, relative to the project.
    pub origin: String,
    /// Non-secret summary: a skill description, a server's executable or host.
    pub detail: String,
    pub trust: String,
    /// The same server name is declared elsewhere with a different target.
    pub conflict: bool,
    /// Hash of the whole declaration (arguments, env, headers included). Only compared in
    /// memory, so differences are detected without exposing or storing their values.
    #[serde(skip)]
    pub fingerprint: String,
}

fn fingerprint(server: &serde_json::Value) -> String {
    use std::hash::{DefaultHasher, Hash, Hasher};
    // serde_json keeps object keys sorted, so equal declarations serialize identically.
    let mut hasher = DefaultHasher::new();
    server.to_string().hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

fn trust_note(provider: &str) -> &'static str {
    match provider {
        "claude" => "Not started. Claude Code asks for approval before using project servers.",
        "gpt" => "Not started. Codex applies its own project trust settings.",
        _ => "Not started. The provider CLI decides whether to trust this folder.",
    }
}

fn rel(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn skill_description(text: &str) -> String {
    let mut lines = text.lines();
    if lines.next().map(str::trim) != Some("---") {
        return String::new();
    }
    for line in lines {
        if line.trim() == "---" {
            break;
        }
        if let Some(value) = line.trim().strip_prefix("description:") {
            return value
                .trim()
                .trim_matches(['"', '\''])
                .chars()
                .take(160)
                .collect();
        }
    }
    String::new()
}

/// Executable name only: arguments and environment can carry tokens.
fn server_summary(command: Option<&str>, url: Option<&str>) -> String {
    if let Some(url) = url {
        let rest = url.split_once("://").map(|(_, r)| r).unwrap_or(url);
        let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
        let host = authority.rsplit('@').next().unwrap_or(authority);
        return format!("remote · {host}");
    }
    match command {
        Some(c) => {
            let name = c.rsplit(['/', '\\']).next().unwrap_or(c);
            format!("local · {name}")
        }
        None => "undeclared target".into(),
    }
}

fn mcp_from_json(root: &Path, file: &Path, provider: &str, out: &mut Vec<ToolEntry>) {
    let Ok(text) = std::fs::read_to_string(file) else {
        return;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return;
    };
    let Some(servers) = value.get("mcpServers").and_then(|v| v.as_object()) else {
        return;
    };
    for (name, server) in servers {
        out.push(ToolEntry {
            kind: "mcp".into(),
            name: name.clone(),
            provider: provider.into(),
            origin: rel(root, file),
            detail: server_summary(
                server.get("command").and_then(|v| v.as_str()),
                server
                    .get("url")
                    .or_else(|| server.get("httpUrl"))
                    .and_then(|v| v.as_str()),
            ),
            trust: trust_note(provider).into(),
            conflict: false,
            fingerprint: fingerprint(server),
        });
    }
}

fn mcp_from_toml(root: &Path, file: &Path, out: &mut Vec<ToolEntry>) {
    let Ok(text) = std::fs::read_to_string(file) else {
        return;
    };
    let Ok(value) = text.parse::<toml::Table>() else {
        return;
    };
    let Some(servers) = value.get("mcp_servers").and_then(|v| v.as_table()) else {
        return;
    };
    for (name, server) in servers {
        out.push(ToolEntry {
            kind: "mcp".into(),
            name: name.clone(),
            provider: "gpt".into(),
            origin: rel(root, file),
            detail: server_summary(
                server.get("command").and_then(|v| v.as_str()),
                server.get("url").and_then(|v| v.as_str()),
            ),
            trust: trust_note("gpt").into(),
            conflict: false,
            fingerprint: serde_json::to_value(server)
                .map(|v| fingerprint(&v))
                .unwrap_or_default(),
        });
    }
}

pub fn discover_tools(root: &Path) -> Vec<ToolEntry> {
    let mut out = vec![];
    for (dir, provider) in SKILL_DIRS {
        let Ok(entries) = std::fs::read_dir(root.join(dir)) else {
            continue;
        };
        for entry in entries.flatten() {
            let skill = entry.path().join("SKILL.md");
            let Ok(text) = std::fs::read_to_string(&skill) else {
                continue;
            };
            out.push(ToolEntry {
                kind: "skill".into(),
                name: entry.file_name().to_string_lossy().into_owned(),
                provider: (*provider).into(),
                origin: rel(root, &skill),
                detail: skill_description(&text),
                trust: "Instructions only; nothing is executed by Lumen.".into(),
                conflict: false,
                fingerprint: String::new(),
            });
        }
    }
    mcp_from_json(root, &root.join(".mcp.json"), "claude", &mut out);
    mcp_from_json(
        root,
        &root.join(".gemini/settings.json"),
        "gemini",
        &mut out,
    );
    mcp_from_toml(root, &root.join(".codex/config.toml"), &mut out);
    let snapshot = out.clone();
    for entry in out.iter_mut().filter(|e| e.kind == "mcp") {
        entry.conflict = snapshot
            .iter()
            .any(|o| o.kind == "mcp" && o.name == entry.name && o.fingerprint != entry.fingerprint);
    }
    out.sort_by(|a, b| (&a.kind, &a.name, &a.origin).cmp(&(&b.kind, &b.name, &b.origin)));
    out
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CheckEntry {
    pub id: String,
    pub label: String,
    pub command: String,
    /// Working directory relative to the project (`.` for the root).
    pub cwd: String,
    pub source: String,
    /// Needs someone to play or look at the game; everything else runs headless.
    pub playtest: bool,
    /// Change-review layers this check covers, from `.lumen/checks.json`.
    pub layers: Vec<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GodotDiagnostics {
    /// Executable on PATH or in the usual install folders; nothing is started by the scan.
    pub executable: Option<String>,
    /// Godot project files nested in this selected folder.
    pub projects: Vec<String>,
    /// MCP declaration names/providers only. Arguments, environment and headers are never returned.
    pub mcp_servers: Vec<String>,
    /// Godot server names a connection test can start.
    pub servers: Vec<String>,
    /// The scan itself never contacts the editor; see `godot::godot_connection_test`.
    pub editor_connection: String,
}

pub fn godot_diagnostics(root: &Path) -> GodotDiagnostics {
    let mut dirs = Vec::new();
    find_dirs(root, root, 0, &mut dirs);
    let mut projects: Vec<String> = dirs
        .into_iter()
        .filter(|dir| dir.join("project.godot").is_file())
        .map(|dir| rel(root, &dir.join("project.godot")))
        .collect();
    projects.sort();
    let mcp_servers = discover_tools(root)
        .into_iter()
        .filter(|entry| entry.kind == "mcp" && entry.name.to_lowercase().contains("godot"))
        .map(|entry| format!("{} ({})", entry.name, entry.provider))
        .collect();
    GodotDiagnostics {
        executable: crate::godot::find_executable(),
        projects,
        mcp_servers,
        servers: crate::godot::godot_servers(root),
        editor_connection:
            "Not checked yet. Test connection starts the declared server and asks the editor for its project info.".into(),
    }
}

#[tauri::command]
pub async fn project_godot_diagnostics(folder: String) -> Result<GodotDiagnostics, String> {
    let root = dunce::canonicalize(&folder).map_err(|_| "Choose an existing folder")?;
    if !root.is_dir() {
        return Err("Choose an existing folder".into());
    }
    Ok(godot_diagnostics(&root))
}

fn script_name_ok(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, ':' | '_' | '.' | '-'))
}

fn skip_dir(name: &str) -> bool {
    name.starts_with('.')
        || [
            "node_modules",
            "target",
            "dist",
            "build",
            "bin",
            "obj",
            "addons",
        ]
        .contains(&name)
}

fn find_dirs(root: &Path, dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    out.push(dir.to_path_buf());
    if depth >= MAX_DEPTH {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut dirs: Vec<PathBuf> = entries
        .flatten()
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .filter(|e| !skip_dir(&e.file_name().to_string_lossy()))
        .map(|e| e.path())
        .collect();
    dirs.sort();
    for d in dirs {
        if d.starts_with(root) {
            find_dirs(root, &d, depth + 1, out);
        }
    }
}

/// Folders holding a `project.godot`, relative to the project (`""` for the root itself).
pub fn godot_project_dirs(root: &Path) -> Vec<String> {
    let mut dirs = vec![];
    find_dirs(root, root, 0, &mut dirs);
    dirs.into_iter()
        .filter(|d| d.join("project.godot").is_file())
        .map(|d| {
            if d == root {
                String::new()
            } else {
                rel(root, &d)
            }
        })
        .collect()
}

pub fn list_checks(root: &Path) -> Vec<CheckEntry> {
    let mut out = vec![];
    if let Some(cmd) = crate::context::verify_command(root) {
        out.push(CheckEntry {
            id: "verify".into(),
            label: "Project verify".into(),
            command: cmd,
            cwd: ".".into(),
            source: "LUMEN.md or detected".into(),
            playtest: false,
            layers: vec![],
        });
    }
    let mut dirs = vec![];
    find_dirs(root, root, 0, &mut dirs);
    for dir in dirs {
        let cwd = if dir == root {
            ".".to_string()
        } else {
            rel(root, &dir)
        };
        if let Ok(text) = std::fs::read_to_string(dir.join("package.json")) {
            if let Some(scripts) = serde_json::from_str::<serde_json::Value>(&text)
                .ok()
                .and_then(|v| v.get("scripts").and_then(|s| s.as_object()).cloned())
            {
                for name in scripts.keys().filter(|n| script_name_ok(n)).take(40) {
                    out.push(CheckEntry {
                        id: format!("npm:{cwd}:{name}"),
                        label: format!("npm run {name}"),
                        command: format!("npm run {name}"),
                        cwd: cwd.clone(),
                        source: rel(root, &dir.join("package.json")),
                        playtest: false,
                        layers: vec![],
                    });
                }
            }
        }
        if dir.join("project.godot").is_file() {
            out.push(CheckEntry {
                id: format!("godot-assets:{cwd}"),
                label: "Godot res:// references".into(),
                command: crate::assets::COMMAND.into(),
                cwd: cwd.clone(),
                source: rel(root, &dir.join("project.godot")),
                playtest: false,
                layers: vec![],
            });
            let has_dotnet = std::fs::read_dir(&dir)
                .map(|it| {
                    it.flatten().any(|e| {
                        let n = e.file_name().to_string_lossy().to_lowercase();
                        n.ends_with(".sln") || n.ends_with(".csproj")
                    })
                })
                .unwrap_or(false);
            if has_dotnet {
                out.push(CheckEntry {
                    id: format!("godot-dotnet:{cwd}"),
                    label: "Godot .NET build (headless)".into(),
                    command: "dotnet build".into(),
                    cwd: cwd.clone(),
                    source: rel(root, &dir.join("project.godot")),
                    playtest: false,
                    layers: vec![],
                });
            }
        }
    }
    // User-defined checks: .lumen/checks.json = [{"name", "command", "cwd"?, "playtest"?}]
    if let Ok(text) = std::fs::read_to_string(root.join(".lumen/checks.json")) {
        if let Ok(items) = serde_json::from_str::<Vec<serde_json::Value>>(&text) {
            for item in items.iter().take(40) {
                let (Some(name), Some(command)) = (
                    item.get("name").and_then(|v| v.as_str()),
                    item.get("command").and_then(|v| v.as_str()),
                ) else {
                    continue;
                };
                let cwd = item.get("cwd").and_then(|v| v.as_str()).unwrap_or(".");
                out.push(CheckEntry {
                    id: format!("custom:{cwd}:{name}"),
                    label: name.into(),
                    command: command.into(),
                    cwd: cwd.into(),
                    source: ".lumen/checks.json".into(),
                    playtest: item
                        .get("playtest")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
                    layers: item
                        .get("layers")
                        .and_then(|v| v.as_array())
                        .map(|a| {
                            a.iter()
                                .filter_map(|x| x.as_str().map(String::from))
                                .collect()
                        })
                        .unwrap_or_default(),
                });
            }
        }
    }
    out
}

#[derive(Serialize, Clone, PartialEq, Debug)]
pub struct SourceLink {
    pub file: String,
    pub line: u32,
}

/// Source locations (`path.ext:12` or `path.ext(12,3)`) mentioned in output, checked against the
/// project so only files that exist inside it are linked.
pub fn source_links(root: &Path, cwd: &Path, output: &str) -> Vec<SourceLink> {
    let mut found: Vec<SourceLink> = vec![];
    for line in output.lines() {
        for ext in SOURCE_EXTENSIONS {
            let needle = format!(".{ext}");
            let mut from = 0;
            while let Some(pos) = line[from..].find(&needle) {
                let end = from + pos + needle.len();
                from = end;
                let tail = &line[end..];
                let digits: String = tail
                    .strip_prefix([':', '('])
                    .map(|t| t.chars().take_while(|c| c.is_ascii_digit()).collect())
                    .unwrap_or_default();
                let Ok(number) = digits.parse::<u32>() else {
                    continue;
                };
                let start = line[..end - needle.len()]
                    .rfind(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | '(' | '[' | '='))
                    .map(|i| i + 1)
                    .unwrap_or(0);
                let candidate = &line[start..end];
                let Some(path) = [cwd.join(candidate), root.join(candidate)]
                    .into_iter()
                    .find_map(|p| dunce::canonicalize(p).ok())
                else {
                    continue;
                };
                let rel_path = rel(root, &path);
                if path.starts_with(root) && path.is_file() && !crate::context::is_secret(&rel_path)
                {
                    let link = SourceLink {
                        file: rel_path,
                        line: number,
                    };
                    if !found.contains(&link) {
                        found.push(link);
                    }
                }
            }
        }
        if found.len() >= 10 {
            break;
        }
    }
    found.truncate(10);
    found
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckResult {
    pub ok: bool,
    pub exit_code: Option<i32>,
    pub output: String,
    pub truncated: bool,
    pub millis: u64,
    pub links: Vec<SourceLink>,
}

const MAX_LINES: usize = 120;
const MAX_CHARS: usize = 12_000;

/// Keeps the tail, where failures usually are, and says when anything was dropped.
pub fn bound_output(text: &str) -> (String, bool) {
    let lines: Vec<&str> = text.lines().collect();
    let mut start = lines.len().saturating_sub(MAX_LINES);
    let mut kept = lines[start..].join("\n");
    while kept.chars().count() > MAX_CHARS && start < lines.len() - 1 {
        start += 1;
        kept = lines[start..].join("\n");
    }
    if kept.chars().count() > MAX_CHARS {
        let skip = kept.chars().count() - MAX_CHARS;
        kept = kept.chars().skip(skip).collect();
    }
    // A single over-long line is cut by character count, which is also a truncation.
    let char_cut = text
        .lines()
        .skip(start)
        .collect::<Vec<_>>()
        .join("\n")
        .chars()
        .count()
        > MAX_CHARS;
    (kept, start > 0 || char_cut)
}

#[tauri::command]
pub async fn project_tools(folder: String) -> Result<Vec<ToolEntry>, String> {
    let dir = dunce::canonicalize(&folder).map_err(|_| "Choose an existing folder")?;
    tauri::async_runtime::spawn_blocking(move || discover_tools(&dir))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn project_checks(folder: String) -> Result<Vec<CheckEntry>, String> {
    let dir = dunce::canonicalize(&folder).map_err(|_| "Choose an existing folder")?;
    tauri::async_runtime::spawn_blocking(move || list_checks(&dir))
        .await
        .map_err(|e| e.to_string())
}

/// Resolves a source location from a check's output to a file inside the project. Files outside
/// the folder, missing, or secret are refused; the location is only used to open the file.
fn resolve_source(root: &Path, file: &str, line: u32) -> Result<PathBuf, String> {
    if line == 0 {
        return Err("Line numbers start at 1".into());
    }
    let path = dunce::canonicalize(root.join(file)).map_err(|_| "That file is not available")?;
    let rel_path = rel(root, &path);
    if !path.starts_with(root) || !path.is_file() || crate::context::is_secret(&rel_path) {
        return Err("That file is outside the project or not allowed".into());
    }
    Ok(path)
}

/// Editor command-line tools that accept a file and line, tried in this order. Each entry is the
/// program and whether it takes `--goto file:line` (VS Code family) or a bare `file:line`.
const LINE_EDITORS: [(&str, bool); 4] = [
    ("code", true),
    ("cursor", true),
    ("zed", false),
    ("subl", false),
];

/// Opens the file at the line in the first installed editor. Returns false when none can open it,
/// so the caller can fall back to the system's default application without a line.
fn open_at_line(path: &Path, line: u32) -> bool {
    LINE_EDITORS.iter().any(|(name, goto)| {
        let program = if cfg!(windows) && matches!(*name, "code" | "cursor") {
            format!("{name}.cmd")
        } else {
            name.to_string()
        };
        let location = format!("{}:{line}", path.display());
        let mut command = std::process::Command::new(program);
        if *goto {
            command.arg("--goto");
        }
        command
            .arg(location)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt as _;
            command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        command.status().map(|s| s.success()).unwrap_or(false)
    })
}

/// Opens a file reported by a check at its line when a supported editor is installed; otherwise the
/// file opens in the default application and the caller shows the line itself.
#[tauri::command]
pub async fn open_source_location(
    app: tauri::AppHandle,
    folder: String,
    file: String,
    line: u32,
) -> Result<bool, String> {
    use tauri_plugin_opener::OpenerExt as _;
    let root = dunce::canonicalize(&folder).map_err(|_| "Choose an existing folder")?;
    let path = resolve_source(&root, &file, line)?;
    let open_path = path.clone();
    let at_line = tauri::async_runtime::spawn_blocking(move || open_at_line(&open_path, line))
        .await
        .map_err(|e| e.to_string())?;
    if !at_line {
        app.opener()
            .open_path(path.to_string_lossy(), None::<&str>)
            .map_err(|e| e.to_string())?;
    }
    Ok(at_line)
}

/// Runs one listed check. The command comes from the listing, never from the caller.
#[tauri::command]
pub async fn run_project_check(
    folder: String,
    id: String,
    command: String,
    cwd: String,
) -> Result<CheckResult, String> {
    let root = dunce::canonicalize(&folder).map_err(|_| "Choose an existing folder")?;
    let list_root = root.clone();
    let check = tauri::async_runtime::spawn_blocking(move || list_checks(&list_root))
        .await
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|c| c.id == id)
        .ok_or("That check is no longer defined in the project")?;
    if check.command != command || check.cwd != cwd {
        return Err(
            "This check changed since it was listed. Review the new command and run it again."
                .into(),
        );
    }
    let cwd = dunce::canonicalize(root.join(&check.cwd))
        .map_err(|_| format!("{} is not available", check.cwd))?;
    if !cwd.starts_with(&root) {
        return Err(format!("{} is outside the project folder", check.cwd));
    }
    let started = std::time::Instant::now();
    if check.id.starts_with("godot-assets:") {
        let (base, project) = (root.clone(), cwd.clone());
        let (ok, text) =
            tauri::async_runtime::spawn_blocking(move || crate::assets::run(&base, &project))
                .await
                .map_err(|e| e.to_string())?;
        let links = source_links(&root, &root, &text);
        let (output, truncated) = bound_output(&text);
        return Ok(CheckResult {
            ok,
            exit_code: Some(if ok { 0 } else { 1 }),
            output,
            truncated,
            millis: started.elapsed().as_millis() as u64,
            links,
        });
    }
    let (status, text, cut) = run_bounded(&cwd, &check.command, CHECK_TIMEOUT).await?;
    let links = source_links(&root, &cwd, &text);
    let (output, truncated) = bound_output(&text);
    Ok(CheckResult {
        ok: status.success(),
        exit_code: status.code(),
        output,
        truncated: truncated || cut,
        millis: started.elapsed().as_millis() as u64,
        links,
    })
}

const CHECK_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(600);

/// Bytes kept per stream while a check runs; older output is dropped as it arrives.
const STREAM_TAIL: usize = 64 * 1024;

async fn tail_of<R: tokio::io::AsyncRead + Unpin>(mut stream: R) -> (Vec<u8>, bool) {
    use tokio::io::AsyncReadExt;
    let mut kept: std::collections::VecDeque<u8> = std::collections::VecDeque::new();
    let mut cut = false;
    let mut buf = [0u8; 8192];
    while let Ok(n) = stream.read(&mut buf).await {
        if n == 0 {
            break;
        }
        kept.extend(&buf[..n]);
        if kept.len() > STREAM_TAIL {
            kept.drain(..kept.len() - STREAM_TAIL);
            cut = true;
        }
    }
    (kept.into_iter().collect(), cut)
}

/// Runs the command with piped streams read incrementally, so verbose output cannot exhaust memory.
async fn run_bounded(
    cwd: &Path,
    cmd: &str,
    limit: std::time::Duration,
) -> Result<(std::process::ExitStatus, String, bool), String> {
    #[cfg(windows)]
    let mut c = {
        let mut c = tokio::process::Command::new("cmd.exe");
        c.args(["/D", "/S", "/C", cmd]).creation_flags(0x08000000);
        c
    };
    #[cfg(not(windows))]
    let mut c = {
        let mut c = tokio::process::Command::new("sh");
        c.args(["-c", cmd]);
        c
    };
    c.current_dir(cwd)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    // A group of its own lets a timeout stop the shell and everything it started.
    #[cfg(unix)]
    c.process_group(0);
    let mut child = c.spawn().map_err(|e| e.to_string())?;
    let out = tokio::spawn(tail_of(child.stdout.take().ok_or("No output stream")?));
    let err = tokio::spawn(tail_of(child.stderr.take().ok_or("No error stream")?));
    let status = match tokio::time::timeout(limit, child.wait()).await {
        Ok(r) => r.map_err(|e| e.to_string())?,
        Err(_) => {
            crate::providers::terminate(&mut child).await;
            return Err(format!(
                "Check timed out after {} minutes",
                limit.as_secs() / 60
            ));
        }
    };
    let (out, out_cut) = out.await.map_err(|e| e.to_string())?;
    let (err, err_cut) = err.await.map_err(|e| e.to_string())?;
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&err),
        String::from_utf8_lossy(&out)
    );
    Ok((status, text, out_cut || err_cut))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("lumen-pt-{name}-{}", crate::models::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dunce::canonicalize(dir).unwrap()
    }
    fn write(root: &Path, rel: &str, text: &str) {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    #[test]
    fn source_locations_must_be_project_files_and_lines() {
        let root = fixture("source");
        write(&root, "game/player.gd", "extends Node\n");
        write(&root, ".env", "TOKEN=SECRET\n");
        let outside = fixture("outside");
        write(&outside, "other.gd", "x\n");

        assert!(resolve_source(&root, "game/player.gd", 1).is_ok());
        assert!(resolve_source(&root, "game/player.gd", 0).is_err());
        assert!(resolve_source(&root, ".env", 1).is_err());
        assert!(resolve_source(&root, "game/missing.gd", 1).is_err());
        let escape = format!(
            "../{}/other.gd",
            outside.file_name().unwrap().to_string_lossy()
        );
        assert!(resolve_source(&root, &escape, 1).is_err());
    }

    #[test]
    fn discovers_skills_and_servers_without_secrets_and_flags_conflicts() {
        let root = fixture("my project");
        write(
            &root,
            ".claude/skills/godot/SKILL.md",
            "---\nname: godot\ndescription: \"Inspect scenes\"\n---\nbody",
        );
        write(
            &root,
            ".mcp.json",
            r#"{"mcpServers":{"godot":{"command":"C:\\tools\\godot-mcp.exe","args":["--token","SECRET123"],"env":{"K":"SECRET456"}},
                "remote":{"url":"https://user:SECRET789@example.com/path?key=SECRET0"}}}"#,
        );
        write(
            &root,
            ".codex/config.toml",
            "[mcp_servers.godot]\ncommand = \"node\"\nargs = [\"x\"]\n",
        );
        let tools = discover_tools(&root);
        let json = serde_json::to_string(&tools).unwrap();
        assert!(!json.contains("SECRET"));
        let skill = tools.iter().find(|t| t.kind == "skill").unwrap();
        assert_eq!(skill.detail, "Inspect scenes");
        assert_eq!(skill.provider, "claude");
        let godot: Vec<_> = tools
            .iter()
            .filter(|t| t.name == "godot" && t.kind == "mcp")
            .collect();
        assert_eq!(godot.len(), 2);
        assert!(godot.iter().all(|t| t.conflict));
        assert!(tools.iter().any(|t| t.detail == "remote · example.com"));
        assert!(!tools.iter().find(|t| t.name == "remote").unwrap().conflict);
        // A separate project sees none of it, and its own declarations stay its own.
        let other = fixture("other project");
        write(
            &other,
            ".mcp.json",
            r#"{"mcpServers":{"godot":{"command":"other"}}}"#,
        );
        let theirs = discover_tools(&other);
        assert_eq!(theirs.len(), 1);
        assert!(!theirs[0].conflict && theirs[0].detail == "local · other");
    }

    #[test]
    fn compares_full_declarations_not_only_the_executable() {
        let root = fixture("same executable");
        let same = r#"{"mcpServers":{"db":{"command":"node","args":["a.js","--token","S1"]}}}"#;
        write(&root, ".mcp.json", same);
        write(&root, ".gemini/settings.json", same);
        let tools = discover_tools(&root);
        assert!(tools.iter().all(|t| t.name != "db" || !t.conflict));
        // Same executable, different script: the declarations disagree.
        write(
            &root,
            ".codex/config.toml",
            "[mcp_servers.db]\ncommand = \"node\"\nargs = [\"b.js\"]\n",
        );
        let tools = discover_tools(&root);
        let db: Vec<_> = tools.iter().filter(|t| t.name == "db").collect();
        assert_eq!(db.len(), 3);
        assert!(db.iter().all(|t| t.conflict));
        assert!(!serde_json::to_string(&tools)
            .unwrap()
            .contains("fingerprint"));
    }

    #[test]
    fn lists_scripts_in_nested_projects_and_custom_checks() {
        let root = fixture("checks");
        write(
            &root,
            "package.json",
            r#"{"scripts":{"test":"vitest","bad name":"x"}}"#,
        );
        write(
            &root,
            "backend/package.json",
            r#"{"scripts":{"sim":"node sim.js"}}"#,
        );
        write(&root, "game/project.godot", "config_version=5");
        write(&root, "game/Game.csproj", "<Project/>");
        write(
            &root,
            "node_modules/x/package.json",
            r#"{"scripts":{"nope":"x"}}"#,
        );
        write(
            &root,
            ".lumen/checks.json",
            r#"[{"name":"Parity","command":"node parity.js","cwd":"backend"}]"#,
        );
        let checks = list_checks(&root);
        let labels: Vec<_> = checks.iter().map(|c| c.label.as_str()).collect();
        assert!(labels.contains(&"npm run test"));
        assert!(labels.contains(&"npm run sim"));
        assert!(labels.contains(&"Godot .NET build (headless)"));
        assert!(labels.contains(&"Parity"));
        assert!(!labels
            .iter()
            .any(|l| l.contains("bad name") || l.contains("nope")));
        let sim = checks.iter().find(|c| c.label == "npm run sim").unwrap();
        assert_eq!(sim.cwd, "backend");
    }

    #[test]
    fn godot_diagnostics_are_project_scoped_and_do_not_expose_server_arguments() {
        let root = fixture("godot diagnostics");
        write(&root, "game/project.godot", "config_version=5");
        write(
            &root,
            ".mcp.json",
            r#"{"mcpServers":{"godot":{"command":"node","args":["--token","PRIVATE"]}}}"#,
        );
        let report = godot_diagnostics(&root);
        assert_eq!(report.projects, vec!["game/project.godot"]);
        assert_eq!(report.mcp_servers, vec!["godot (claude)"]);
        assert!(report.editor_connection.contains("Not checked"));
        let json = serde_json::to_string(&report).unwrap();
        assert!(!json.contains("PRIVATE"));
        let other = fixture("unrelated");
        assert!(godot_diagnostics(&other).projects.is_empty());
        assert!(godot_diagnostics(&other).mcp_servers.is_empty());
    }

    #[test]
    fn links_errors_to_existing_project_files_only() {
        let root = fixture("links");
        write(&root, "game/Player.cs", "class P {}");
        write(&root, "src/api.ts", "export {}");
        write(&root, ".env.ts", "secret");
        let output = "game/Player.cs(12,5): error CS1002: ; expected\n\
                      src/api.ts:40:2 - error TS2304\n\
                      missing/File.cs(3,1): error\n\
                      ../outside.cs(1,1): error\n";
        let links = source_links(&root, &root, output);
        assert_eq!(
            links,
            vec![
                SourceLink {
                    file: "game/Player.cs".into(),
                    line: 12
                },
                SourceLink {
                    file: "src/api.ts".into(),
                    line: 40
                },
            ]
        );
    }

    #[test]
    fn bounds_output_to_the_tail() {
        let text: String = (0..500).map(|i| format!("line {i}\n")).collect();
        let (out, truncated) = bound_output(&text);
        assert!(truncated);
        assert!(out.ends_with("line 499"));
        assert!(out.lines().count() <= MAX_LINES);
        let (short, cut) = bound_output("a\nb");
        assert_eq!((short.as_str(), cut), ("a\nb", false));
    }

    #[tokio::test]
    async fn runs_listed_checks_with_status_and_rejects_unknown_ids() {
        let root = fixture("run");
        write(
            &root,
            ".lumen/checks.json",
            r#"[{"name":"Pass","command":"echo hello"},{"name":"Fail","command":"echo broken 1>&2 && exit 3"}]"#,
        );
        let folder = root.to_string_lossy().into_owned();
        let pass = run_project_check(
            folder.clone(),
            "custom:.:Pass".into(),
            "echo hello".into(),
            ".".into(),
        )
        .await
        .unwrap();
        assert!(pass.ok && pass.output.contains("hello"));
        let fail = run_project_check(
            folder.clone(),
            "custom:.:Fail".into(),
            "echo broken 1>&2 && exit 3".into(),
            ".".into(),
        )
        .await
        .unwrap();
        assert!(!fail.ok);
        assert_eq!(fail.exit_code, Some(3));
        assert!(fail.output.contains("broken"));
        assert!(run_project_check(
            folder.clone(),
            "custom:.:rm -rf".into(),
            "rm -rf".into(),
            ".".into()
        )
        .await
        .is_err());
        // A command that differs from the one the user was shown is refused.
        let changed = run_project_check(
            folder,
            "custom:.:Pass".into(),
            "echo other".into(),
            ".".into(),
        )
        .await;
        assert!(changed.err().unwrap().contains("changed"));
    }

    #[tokio::test]
    async fn stops_a_check_and_its_children_at_the_timeout() {
        let root = fixture("timeout");
        #[cfg(windows)]
        let cmd = "ping -n 60 127.0.0.1 > nul";
        #[cfg(not(windows))]
        let cmd = "sleep 60 & sleep 60; wait";
        let started = std::time::Instant::now();
        let result = run_bounded(&root, cmd, std::time::Duration::from_millis(500)).await;
        assert!(result.err().unwrap().contains("timed out"));
        // The shell would otherwise keep the pipes open for a minute.
        assert!(started.elapsed() < std::time::Duration::from_secs(20));
    }

    /// Runs listed checks of a real project: `LUMEN_CHECK_PROJECT=<folder>`,
    /// `LUMEN_CHECK_IDS=<id>,<id>` (all ids are printed when it is empty).
    #[tokio::test]
    #[ignore]
    async fn live_project_checks() {
        let folder = std::env::var("LUMEN_CHECK_PROJECT").expect("LUMEN_CHECK_PROJECT");
        let checks = list_checks(&dunce::canonicalize(&folder).unwrap());
        for c in &checks {
            println!(
                "listed {} | {} | {} | playtest {}",
                c.id, c.command, c.cwd, c.playtest
            );
        }
        let wanted = std::env::var("LUMEN_CHECK_IDS").unwrap_or_default();
        for id in wanted.split(',').filter(|s| !s.is_empty()) {
            let c = checks.iter().find(|c| c.id == id).expect(id);
            let r = run_project_check(
                folder.clone(),
                c.id.clone(),
                c.command.clone(),
                c.cwd.clone(),
            )
            .await
            .unwrap();
            let tail: String = r
                .output
                .lines()
                .rev()
                .take(6)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect::<Vec<_>>()
                .join(" / ");
            println!(
                "ran {id}: ok {} exit {:?} {}ms links {:?}
  {tail}",
                r.ok, r.exit_code, r.millis, r.links
            );
        }
    }
}
