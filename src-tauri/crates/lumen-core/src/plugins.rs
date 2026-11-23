//! Plugins v1: a folder with `lumen-plugin.json`. Everything a plugin can do is declared in the
//! manifest, shown before install, off until the user enables it, and checked again at every use.
//! Themes are tokens (no code), slash commands are prompt text, skills reuse shared skills, MCP
//! servers start through Lumen's launcher with a scrubbed environment, and panels run in a
//! sandboxed iframe whose only door is the narrow broker below.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

pub const MANIFEST: &str = "lumen-plugin.json";
const RESERVED_COMMANDS: [&str; 2] = ["workspace", "compact"];
/// Interpreters that may run a script shipped inside the plugin without the shell permission.
const RUNTIMES: [&str; 5] = ["node", "python", "python3", "deno", "bun"];
/// Variables a child needs to start at all; everything else in Lumen's environment is dropped.
const BASE_ENV: [&str; 12] = [
    "PATH",
    "PATHEXT",
    "SYSTEMROOT",
    "SystemRoot",
    "WINDIR",
    "TEMP",
    "TMP",
    "TMPDIR",
    "HOME",
    "USERPROFILE",
    "LANG",
    "LC_ALL",
];

#[derive(Deserialize, Serialize, Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum FsAccess {
    #[default]
    None,
    Read,
    Write,
}

/// Nothing by default: a plugin without this block gets no project files, network, shell or secrets.
#[derive(Deserialize, Serialize, Clone, Debug, Default, PartialEq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Permissions {
    #[serde(default)]
    pub fs_project: FsAccess,
    #[serde(default)]
    pub network: Vec<String>,
    #[serde(default)]
    pub shell: bool,
    #[serde(default)]
    pub secrets: Vec<String>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct Theme {
    pub id: String,
    pub name: String,
    pub file: String,
}
#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct SlashCommand {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub prompt: String,
}
#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct Server {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
}
#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct Panel {
    pub id: String,
    pub title: String,
    pub entry: String,
}
#[derive(Deserialize, Serialize, Clone, Debug, Default)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Contributions {
    #[serde(default)]
    pub themes: Vec<Theme>,
    #[serde(default)]
    pub skills: Vec<String>,
    #[serde(default)]
    pub slash_commands: Vec<SlashCommand>,
    #[serde(default)]
    pub mcp_servers: BTreeMap<String, Server>,
    #[serde(default)]
    pub panels: Vec<Panel>,
}
#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    /// Compatible Lumen versions, e.g. `">=0.10.0 <0.12.0"`.
    pub lumen: String,
    #[serde(default)]
    pub contributes: Contributions,
    #[serde(default)]
    pub permissions: Permissions,
}

fn version(text: &str) -> Option<(u64, u64, u64)> {
    let mut parts = text.trim().split('.').map(|p| p.parse::<u64>().ok());
    let v = (
        parts.next()??,
        parts.next()??,
        parts.next().unwrap_or(Some(0))?,
    );
    parts.next().is_none().then_some(v)
}
/// Space-separated comparators, all of which must hold.
pub fn compatible(range: &str, current: &str) -> Result<bool, String> {
    let current = version(current).ok_or("Invalid Lumen version")?;
    let mut any = false;
    for part in range.split_whitespace() {
        any = true;
        let (op, rest) = ["<=", ">=", "<", ">", "="]
            .iter()
            .find_map(|op| part.strip_prefix(op).map(|rest| (*op, rest)))
            .unwrap_or(("=", part));
        let want = version(rest).ok_or_else(|| format!("Invalid Lumen version range: {range}"))?;
        let ok = match op {
            "<=" => current <= want,
            ">=" => current >= want,
            "<" => current < want,
            ">" => current > want,
            _ => current == want,
        };
        if !ok {
            return Ok(false);
        }
    }
    if !any {
        return Err("The manifest must declare compatible Lumen versions".into());
    }
    Ok(true)
}

fn slug(text: &str, max: usize) -> bool {
    let mut chars = text.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && text.len() <= max
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}
fn host_pattern(host: &str) -> bool {
    let bare = host.strip_prefix("*.").unwrap_or(host);
    !bare.is_empty()
        && bare.len() <= 253
        && bare.contains('.')
        && bare
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '-')
}
/// A relative path that stays inside the plugin folder, symlinks included.
fn inside(dir: &Path, rel: &str) -> Result<PathBuf, String> {
    let path = Path::new(rel);
    if rel.is_empty()
        || path
            .components()
            .any(|c| !matches!(c, Component::Normal(_) | Component::CurDir))
    {
        return Err(format!("{rel}: paths must stay inside the plugin folder"));
    }
    let base = dunce::canonicalize(dir).map_err(|e| e.to_string())?;
    let full = dunce::canonicalize(base.join(path)).map_err(|_| format!("{rel} is missing"))?;
    if !full.starts_with(&base) {
        return Err(format!("{rel}: paths must stay inside the plugin folder"));
    }
    Ok(full)
}

/// Theme files are CSS custom properties under `:root` (optionally per light/dark) and nothing else.
pub fn parse_theme(css: &str) -> Result<BTreeMap<String, BTreeMap<String, String>>, String> {
    let refuse = || "Themes may only set CSS tokens (--name: value) under :root".to_string();
    let mut text = css.to_string();
    while let Some(start) = text.find("/*") {
        let end = text[start..].find("*/").ok_or_else(refuse)? + start + 2;
        text.replace_range(start..end, "");
    }
    let mut blocks = BTreeMap::<String, BTreeMap<String, String>>::new();
    let mut rest = text.as_str();
    while let Some(open) = rest.find('{') {
        let selector: String = rest[..open]
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>()
            .replace('"', "'");
        let scheme = match selector.as_str() {
            ":root" => "any",
            ":root[data-theme='dark']" => "dark",
            ":root[data-theme='light']" => "light",
            _ => return Err(refuse()),
        };
        let close = rest[open..].find('}').ok_or_else(refuse)? + open;
        let tokens = blocks.entry(scheme.into()).or_default();
        for decl in rest[open + 1..close].split(';').map(str::trim) {
            if decl.is_empty() {
                continue;
            }
            let (name, value) = decl.split_once(':').ok_or_else(refuse)?;
            let (name, value) = (name.trim(), value.trim());
            let lower = value.to_ascii_lowercase();
            let valid_name = name.strip_prefix("--").is_some_and(|n| slug(n, 64));
            if !valid_name
                || value.is_empty()
                || value.len() > 200
                || value.contains(['{', '}', '<', '>', '\\', '@', '"', '\''])
                || ["url(", "expression", "image-set", "src("]
                    .iter()
                    .any(|bad| lower.contains(bad))
            {
                return Err(refuse());
            }
            tokens.insert(name.into(), value.into());
        }
        rest = &rest[close + 1..];
    }
    if !rest.trim().is_empty() || blocks.values().all(BTreeMap::is_empty) {
        return Err(refuse());
    }
    Ok(blocks)
}

/// Reads and checks a plugin folder. Nothing in it runs here.
pub fn validate(dir: &Path) -> Result<Manifest, String> {
    let file = dir.join(MANIFEST);
    let meta = std::fs::metadata(&file).map_err(|_| format!("{MANIFEST} is missing"))?;
    if meta.len() > 256_000 {
        return Err(format!("{MANIFEST} exceeds 256 KB"));
    }
    let text = std::fs::read_to_string(&file).map_err(|e| e.to_string())?;
    let m: Manifest =
        serde_json::from_str(&text).map_err(|e| format!("Invalid {MANIFEST}: {e}"))?;
    if !slug(&m.id, 64) || m.id.len() < 2 {
        return Err("Plugin id must be 2-64 lowercase letters, digits or dashes".into());
    }
    if m.name.trim().is_empty() || m.name.len() > 64 || m.description.len() > 500 {
        return Err("Plugin name (1-64) or description (up to 500) has the wrong length".into());
    }
    if version(&m.version).is_none() {
        return Err("Plugin version must look like 1.2.3".into());
    }
    if !compatible(&m.lumen, env!("CARGO_PKG_VERSION"))? {
        return Err(format!(
            "This plugin needs Lumen {}; this is {}",
            m.lumen,
            env!("CARGO_PKG_VERSION")
        ));
    }
    let p = &m.permissions;
    if let Some(host) = p.network.iter().find(|h| !host_pattern(h)) {
        return Err(format!(
            "Invalid network host: {host} (use example.com or *.example.com)"
        ));
    }
    if let Some(name) = p.secrets.iter().find(|s| {
        !s.chars().next().is_some_and(|c| c.is_ascii_uppercase())
            || s.len() > 64
            || !s
                .chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
    }) {
        return Err(format!(
            "Invalid secret name: {name} (use UPPER_SNAKE_CASE)"
        ));
    }
    let c = &m.contributes;
    let mut seen = std::collections::BTreeSet::new();
    for theme in &c.themes {
        if !slug(&theme.id, 32) || !seen.insert(format!("theme:{}", theme.id)) {
            return Err(format!("Invalid or duplicate theme id: {}", theme.id));
        }
        let path = inside(dir, &theme.file)?;
        if path.extension().is_none_or(|e| e != "css") {
            return Err(format!("{}: themes must be .css files", theme.file));
        }
        parse_theme(&std::fs::read_to_string(path).map_err(|e| e.to_string())?)
            .map_err(|e| format!("{}: {e}", theme.file))?;
    }
    for skill in &c.skills {
        if !inside(dir, skill)?.join("SKILL.md").is_file() {
            return Err(format!("{skill}: a skill folder needs SKILL.md"));
        }
    }
    for command in &c.slash_commands {
        if !slug(&command.name, 32)
            || !command.name.starts_with(|c: char| c.is_ascii_lowercase())
            || RESERVED_COMMANDS.contains(&command.name.as_str())
            || !seen.insert(format!("slash:{}", command.name))
        {
            return Err(format!(
                "Invalid, reserved or duplicate command: /{}",
                command.name
            ));
        }
        if command.prompt.trim().is_empty() || command.prompt.len() > 8_000 {
            return Err(format!(
                "/{}: the prompt must be 1-8000 characters",
                command.name
            ));
        }
    }
    for (name, server) in &c.mcp_servers {
        if !slug(name, 32) {
            return Err(format!("Invalid MCP server name: {name}"));
        }
        program(dir, server, p)?;
    }
    for panel in &c.panels {
        if !slug(&panel.id, 32) || !seen.insert(format!("panel:{}", panel.id)) {
            return Err(format!("Invalid or duplicate panel id: {}", panel.id));
        }
        let path = inside(dir, &panel.entry)?;
        if path.extension().is_none_or(|e| e != "html")
            || path.metadata().is_ok_and(|m| m.len() > 1_000_000)
        {
            return Err(format!(
                "{}: panels are one .html file up to 1 MB",
                panel.entry
            ));
        }
    }
    Ok(m)
}

/// What the user approves, in words, so updates can show only what is new.
pub fn grants(p: &Permissions) -> Vec<String> {
    let mut out = vec![];
    if p.fs_project >= FsAccess::Read {
        out.push("Read files in the current project".to_string());
    }
    if p.fs_project == FsAccess::Write {
        out.push("Change files in the current project".into());
    }
    out.extend(p.network.iter().map(|h| format!("Connect to {h}")));
    if p.shell {
        out.push("Run any program on this computer".into());
    }
    out.extend(p.secrets.iter().map(|s| format!("Use the secret {s}")));
    out
}
pub fn added(old: &Permissions, new: &Permissions) -> Vec<String> {
    let before = grants(old);
    grants(new)
        .into_iter()
        .filter(|g| !before.contains(g))
        .collect()
}

/// Resolves an MCP server's program. Without `shell`, only a file shipped in the plugin may run,
/// directly or through a known interpreter.
fn program(dir: &Path, server: &Server, p: &Permissions) -> Result<(PathBuf, Vec<String>), String> {
    let refuse = || {
        format!(
            "MCP command `{}` is outside the plugin; it needs the shell permission",
            server.command
        )
    };
    if RUNTIMES.contains(&server.command.as_str()) {
        if let Some(script) = server.args.first() {
            if let Ok(script) = inside(dir, script) {
                let exe =
                    which::which(&server.command).unwrap_or_else(|_| server.command.clone().into());
                let mut args = vec![script.to_string_lossy().into_owned()];
                args.extend(server.args[1..].iter().cloned());
                return Ok((exe, args));
            }
        }
        return if p.shell {
            Ok((server.command.clone().into(), server.args.clone()))
        } else {
            Err(refuse())
        };
    }
    if let Some(rel) = server.command.strip_prefix("./") {
        return Ok((inside(dir, rel)?, server.args.clone()));
    }
    if p.shell {
        return Ok((server.command.clone().into(), server.args.clone()));
    }
    Err(refuse())
}

pub struct Launch {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub env: BTreeMap<String, String>,
}
/// The process a plugin MCP server gets: its own folder, a scrubbed environment, the project
/// path only with `fsProject`, and only the secrets it declared.
pub fn launch(
    dir: &Path,
    m: &Manifest,
    server: &str,
    workspace: &Path,
    secret: &dyn Fn(&str) -> Option<String>,
) -> Result<Launch, String> {
    let config = m
        .contributes
        .mcp_servers
        .get(server)
        .ok_or("This plugin does not declare that MCP server")?;
    let p = &m.permissions;
    let (program, args) = program(dir, config, p)?;
    let mut env: BTreeMap<String, String> = BASE_ENV
        .iter()
        .filter_map(|k| std::env::var(k).ok().map(|v| (k.to_string(), v)))
        .collect();
    env.insert("LUMEN_PLUGIN_ID".into(), m.id.clone());
    env.insert("LUMEN_PLUGIN_DIR".into(), dir.to_string_lossy().into());
    if p.fs_project != FsAccess::None {
        env.insert(
            "LUMEN_PROJECT_DIR".into(),
            workspace.to_string_lossy().into(),
        );
        env.insert(
            "LUMEN_PROJECT_ACCESS".into(),
            if p.fs_project == FsAccess::Write {
                "write"
            } else {
                "read"
            }
            .into(),
        );
    }
    if !p.network.is_empty() {
        env.insert("LUMEN_NETWORK_HOSTS".into(), p.network.join(","));
    }
    for name in &p.secrets {
        if let Some(value) = secret(name) {
            env.insert(name.clone(), value);
        }
    }
    Ok(Launch {
        program,
        args,
        cwd: dunce::canonicalize(dir).map_err(|e| e.to_string())?,
        env,
    })
}

/// The broker's network rule: HTTPS to a host the plugin declared, nothing else.
pub fn allowed_url(p: &Permissions, url: &str) -> Result<reqwest::Url, String> {
    let url = reqwest::Url::parse(url).map_err(|_| "Invalid URL")?;
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    let declared = p.network.iter().any(|h| match h.strip_prefix("*.") {
        Some(base) => host.ends_with(&format!(".{base}")),
        None => *h == host,
    });
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() {
        return Err("Plugins may only fetch https:// URLs".into());
    }
    if !declared {
        return Err(format!(
            "This plugin did not declare network access to {host}"
        ));
    }
    Ok(url)
}

// --- Storage ---------------------------------------------------------------------------------

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub enabled: bool,
    pub source: String,
    /// The permissions the user agreed to when enabling; growth needs a new approval.
    pub approved: Permissions,
}
fn root(data: &Path) -> PathBuf {
    data.join("plugins")
}
fn installed(data: &Path, id: &str) -> PathBuf {
    root(data).join("installed").join(id)
}
fn state(data: &Path) -> BTreeMap<String, Entry> {
    std::fs::read(root(data).join("state.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}
fn save(data: &Path, s: &BTreeMap<String, Entry>) -> Result<(), String> {
    std::fs::create_dir_all(root(data)).map_err(|e| e.to_string())?;
    let tmp = root(data).join(format!(".state-{}.json", crate::models::id()));
    std::fs::write(
        &tmp,
        serde_json::to_vec_pretty(s).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    std::fs::rename(tmp, root(data).join("state.json")).map_err(|e| e.to_string())
}
fn copy_tree(source: &Path, dest: &Path, depth: usize, budget: &mut u64) -> Result<(), String> {
    if depth > 16 {
        return Err("Plugin folder is too deep".into());
    }
    std::fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    for item in std::fs::read_dir(source).map_err(|e| e.to_string())? {
        let item = item.map_err(|e| e.to_string())?;
        let ty = item.file_type().map_err(|e| e.to_string())?;
        let name = item.file_name();
        if ty.is_symlink() {
            return Err("Plugins cannot contain symbolic links".into());
        }
        if name == ".git" || name == "node_modules" {
            continue;
        }
        if ty.is_dir() {
            copy_tree(&item.path(), &dest.join(&name), depth + 1, budget)?;
        } else if ty.is_file() {
            *budget = budget
                .checked_sub(item.metadata().map_err(|e| e.to_string())?.len())
                .ok_or("Plugin exceeds 50 MB")?;
            std::fs::copy(item.path(), dest.join(&name)).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// The installed, enabled plugins whose files still match what the user approved.
pub fn active(data: &Path) -> Vec<(PathBuf, Manifest)> {
    state(data)
        .into_iter()
        .filter(|(_, e)| e.enabled)
        .filter_map(|(id, e)| {
            let dir = installed(data, &id);
            let m = validate(&dir).ok()?;
            (m.id == id && added(&e.approved, &m.permissions).is_empty()).then_some((dir, m))
        })
        .collect()
}
fn active_one(data: &Path, id: &str) -> Result<(PathBuf, Manifest), String> {
    active(data)
        .into_iter()
        .find(|(_, m)| m.id == id)
        .ok_or_else(|| "This plugin is not enabled".into())
}

/// Copies a local folder or clones an https git URL into staging and describes it for approval.
pub async fn inspect(data: &Path, source: &str) -> Result<Value, String> {
    let source = source.trim();
    let staging = root(data).join("staging");
    let _ = std::fs::remove_dir_all(&staging);
    let token = crate::models::id();
    let dest = staging.join(&token);
    std::fs::create_dir_all(&staging).map_err(|e| e.to_string())?;
    let result = async {
        if source.starts_with("https://") {
            let status = tokio::time::timeout(
                std::time::Duration::from_secs(120),
                crate::proc::command("git")
                    .args(["clone", "--depth", "1", "--quiet", "--", source])
                    .arg(&dest)
                    .env("GIT_TERMINAL_PROMPT", "0")
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status(),
            )
            .await
            .map_err(|_| "git clone timed out")?
            .map_err(|_| "git is not installed")?;
            if !status.success() {
                return Err("git clone failed; check the URL".to_string());
            }
            let _ = std::fs::remove_dir_all(dest.join(".git"));
        } else if source.contains("://") || source.starts_with("git@") {
            return Err("Only local folders and https:// git URLs are supported".into());
        } else {
            let from = dunce::canonicalize(source).map_err(|_| "Folder not found")?;
            if from.starts_with(dunce::canonicalize(data).map_err(|e| e.to_string())?) {
                return Err("Choose a folder outside Lumen's data folder".into());
            }
            copy_tree(&from, &dest, 0, &mut 50_000_000)?;
        }
        validate(&dest)
    }
    .await;
    let m = match result {
        Ok(m) => m,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&dest);
            return Err(e);
        }
    };
    std::fs::write(staging.join(format!("{token}.source")), source).map_err(|e| e.to_string())?;
    let previous = state(data).get(&m.id).cloned();
    let old = previous
        .as_ref()
        .map(|e| e.approved.clone())
        .unwrap_or_default();
    let installed_version = validate(&installed(data, &m.id)).ok().map(|m| m.version);
    Ok(json!({
        "token": token,
        "manifest": m,
        "grants": grants(&m.permissions),
        "added": added(&old, &m.permissions),
        "update": previous.is_some(),
        "installedVersion": installed_version,
    }))
}

/// Moves a staged plugin into place. New plugins start disabled; an update stays enabled only
/// when it asks for nothing beyond what was approved.
pub fn install(data: &Path, token: &str) -> Result<Value, String> {
    if !slug(token, 64) {
        return Err("Invalid install token".into());
    }
    let staging = root(data).join("staging");
    let dir = staging.join(token);
    let m = validate(&dir)?;
    let source =
        std::fs::read_to_string(staging.join(format!("{token}.source"))).unwrap_or_default();
    let dest = installed(data, &m.id);
    std::fs::create_dir_all(dest.parent().unwrap()).map_err(|e| e.to_string())?;
    if dest.exists() {
        let old = root(data).join(format!(".removed-{}", crate::models::id()));
        std::fs::rename(&dest, &old).map_err(|e| e.to_string())?;
        let _ = std::fs::remove_dir_all(old);
    }
    std::fs::rename(&dir, &dest).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_dir_all(&staging);
    let mut s = state(data);
    let entry = s.entry(m.id.clone()).or_default();
    entry.source = source;
    entry.enabled = entry.enabled && added(&entry.approved, &m.permissions).is_empty();
    let enabled = entry.enabled;
    save(data, &s)?;
    Ok(json!({"id": m.id, "enabled": enabled}))
}

pub fn set_enabled(data: &Path, id: &str, on: bool) -> Result<(), String> {
    let mut s = state(data);
    let entry = s.get_mut(id).ok_or("Plugin is not installed")?;
    if on {
        // Enabling is the approval of exactly what the manifest on disk declares.
        entry.approved = validate(&installed(data, id))?.permissions;
    }
    entry.enabled = on;
    save(data, &s)
}

pub fn remove(data: &Path, id: &str) -> Result<(), String> {
    let mut s = state(data);
    let entry = s.remove(id).ok_or("Plugin is not installed")?;
    save(data, &s)?;
    for name in entry.approved.secrets {
        let _ = forget_secret(data, id, &name);
    }
    std::fs::remove_dir_all(installed(data, id)).or_else(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            Ok(())
        } else {
            Err(e.to_string())
        }
    })
}

/// Re-reads the plugin from where it was installed. Same permissions apply at once; anything new
/// comes back for approval instead.
pub async fn reload(data: &Path, id: &str) -> Result<Value, String> {
    let source = state(data)
        .get(id)
        .ok_or("Plugin is not installed")?
        .source
        .clone();
    if source.is_empty() {
        return Err("This plugin has no source to reload from".into());
    }
    let mut review = inspect(data, &source).await?;
    if review["manifest"]["id"] != id {
        return Err("The source now contains a different plugin".into());
    }
    if review["added"].as_array().is_some_and(Vec::is_empty) {
        install(data, review["token"].as_str().unwrap_or_default())?;
        review["applied"] = json!(true);
    } else {
        review["applied"] = json!(false);
    }
    Ok(review)
}

pub fn list(data: &Path) -> Vec<Value> {
    state(data)
        .into_iter()
        .map(|(id, e)| match validate(&installed(data, &id)) {
            Ok(m) => {
                let pending = added(&e.approved, &m.permissions);
                json!({"id": id, "manifest": m, "enabled": e.enabled && pending.is_empty(),
                    "source": e.source, "grants": grants(&m.permissions), "pending": if e.enabled { pending } else { vec![] }})
            }
            Err(error) => json!({"id": id, "enabled": false, "source": e.source, "error": error}),
        })
        .collect()
}

/// What the interface needs from enabled plugins: validated theme tokens, prompt commands, panels.
pub fn contributions(data: &Path) -> Value {
    let (mut themes, mut commands, mut panels) = (vec![], vec![], vec![]);
    for (dir, m) in active(data) {
        for t in &m.contributes.themes {
            if let Ok(blocks) = inside(&dir, &t.file)
                .and_then(|p| std::fs::read_to_string(p).map_err(|e| e.to_string()))
                .and_then(|css| parse_theme(&css))
            {
                themes.push(json!({"plugin": m.id, "id": t.id, "name": t.name, "tokens": blocks}));
            }
        }
        for c in &m.contributes.slash_commands {
            commands.push(json!({"plugin": m.id, "name": c.name, "description": c.description, "prompt": c.prompt}));
        }
        for p in &m.contributes.panels {
            panels.push(json!({"plugin": m.id, "id": p.id, "title": p.title, "network": m.permissions.network}));
        }
    }
    json!({"themes": themes, "slashCommands": commands, "panels": panels})
}

pub fn panel_html(data: &Path, id: &str, panel: &str) -> Result<String, String> {
    let (dir, m) = active_one(data, id)?;
    let p = m
        .contributes
        .panels
        .iter()
        .find(|p| p.id == panel)
        .ok_or("Unknown panel")?;
    std::fs::read_to_string(inside(&dir, &p.entry)?).map_err(|e| e.to_string())
}

pub async fn panel_fetch(data: &Path, id: &str, url: &str) -> Result<Value, String> {
    let (_, m) = active_one(data, id)?;
    let url = allowed_url(&m.permissions, url)?;
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;
    let response = client.get(url).send().await.map_err(|_| "Request failed")?;
    let status = response.status().as_u16();
    let kind = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let bytes = response.bytes().await.map_err(|_| "Request failed")?;
    if bytes.len() > 1_000_000 {
        return Err("Response exceeds 1 MB".into());
    }
    Ok(json!({"status": status, "contentType": kind, "body": String::from_utf8_lossy(&bytes)}))
}

/// Shared MCP entries for enabled plugins; each starts through Lumen's launcher.
pub fn mcp_servers(data: &Path) -> serde_json::Map<String, Value> {
    let exe = std::env::current_exe().unwrap_or_default();
    active(data)
        .into_iter()
        .flat_map(|(_, m)| {
            let exe = exe.clone();
            m.contributes.mcp_servers.into_keys().map(move |server| {
                (
                    format!("plugin-{}-{server}", m.id),
                    json!({"command": exe, "args": ["--lumen-plugin-mcp", data, m.id, server]}),
                )
            })
        })
        .collect()
}

pub fn skill_folders(data: &Path) -> Vec<PathBuf> {
    active(data)
        .into_iter()
        .flat_map(|(dir, m)| {
            m.contributes
                .skills
                .into_iter()
                .filter_map(move |s| inside(&dir, &s).ok())
        })
        .collect()
}

// --- Secrets: OS keychain, a file only for isolated test data --------------------------------

fn test_secrets(data: &Path) -> Option<PathBuf> {
    (cfg!(test) || std::env::var_os("LUMEN_TEST_DATA_DIR").is_some())
        .then(|| root(data).join("test-secrets.json"))
}
fn secret_key(id: &str, name: &str) -> String {
    format!("{id}:{name}")
}
pub fn set_secret(data: &Path, id: &str, name: &str, value: &str) -> Result<(), String> {
    let m = validate(&installed(data, id))?;
    if !m.permissions.secrets.iter().any(|s| s == name) {
        return Err(format!("This plugin did not declare the secret {name}"));
    }
    if let Some(file) = test_secrets(data) {
        let mut all: BTreeMap<String, String> = std::fs::read(&file)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        all.insert(secret_key(id, name), value.into());
        std::fs::create_dir_all(root(data)).map_err(|e| e.to_string())?;
        return std::fs::write(file, serde_json::to_vec(&all).unwrap()).map_err(|e| e.to_string());
    }
    keyring::Entry::new("lumen-plugin", &secret_key(id, name))
        .and_then(|e| e.set_password(value))
        .map_err(|_| "OS keychain unavailable".into())
}
fn secret(data: &Path, id: &str, name: &str) -> Option<String> {
    if let Some(file) = test_secrets(data) {
        let all: BTreeMap<String, String> =
            serde_json::from_slice(&std::fs::read(file).ok()?).ok()?;
        return all.get(&secret_key(id, name)).cloned();
    }
    keyring::Entry::new("lumen-plugin", &secret_key(id, name))
        .ok()?
        .get_password()
        .ok()
}
fn forget_secret(data: &Path, id: &str, name: &str) -> Result<(), String> {
    if test_secrets(data).is_some() {
        return Ok(());
    }
    keyring::Entry::new("lumen-plugin", &secret_key(id, name))
        .and_then(|e| e.delete_credential())
        .map_err(|e| e.to_string())
}

/// `--lumen-plugin-mcp`: re-checks the plugin at launch, then runs the server on this stdio.
pub fn serve_mcp(data: &Path, id: &str, server: &str) -> Result<(), String> {
    let workspace = std::env::current_dir().map_err(|e| e.to_string())?;
    let (dir, m) = active_one(data, id)?;
    let plan = launch(&dir, &m, server, &workspace, &|name| secret(data, id, name))?;
    let status = crate::proc::std_command(&plan.program)
        .args(&plan.args)
        .env_clear()
        .envs(&plan.env)
        .current_dir(&plan.cwd)
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|_| "Cannot start the plugin MCP server")?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("Plugin MCP server exited with {status}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(manifest: Value) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("lumen-plugin-{}", crate::models::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(MANIFEST), manifest.to_string()).unwrap();
        dir
    }
    fn base(extra: Value) -> Value {
        let mut m = json!({"id":"demo","name":"Demo","version":"1.0.0","lumen":">=0.10.0"});
        m.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        m
    }

    #[test]
    fn manifest_validation() {
        assert!(validate(&folder(base(json!({})))).is_ok());
        for bad in [
            json!({"id":"Bad Id"}),
            json!({"version":"one"}),
            json!({"lumen":">=99.0.0"}),
            json!({"lumen":""}),
            json!({"unknown":true}),
            json!({"permissions":{"fsProject":"everything"}}),
            json!({"permissions":{"network":["https://x.com"]}}),
            json!({"permissions":{"secrets":["lower"]}}),
            json!({"contributes":{"slashCommands":[{"name":"compact","prompt":"x"}]}}),
            json!({"contributes":{"slashCommands":[{"name":"ok","prompt":" "}]}}),
            json!({"contributes":{"skills":["../outside"]}}),
            json!({"contributes":{"panels":[{"id":"p","title":"P","entry":"missing.html"}]}}),
        ] {
            assert!(validate(&folder(base(bad.clone()))).is_err(), "{bad}");
        }
        assert!(compatible(">=0.10.0 <0.11", "0.10.4").unwrap());
        assert!(!compatible(">=0.10.0 <0.11", "0.11.0").unwrap());
    }

    #[test]
    fn themes_are_tokens_only() {
        assert!(parse_theme(
            ":root { --accent: #123456; }\n:root[data-theme=\"dark\"] { --text: #fff }"
        )
        .is_ok());
        for bad in [
            "body { --accent: red }",
            ":root { color: red }",
            ":root { --bg: url(https://evil.test/x.png) }",
            ":root { --x: red } @import 'x';",
            ":root { --x: expression(alert(1)) }",
            "",
        ] {
            assert!(parse_theme(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn undeclared_capabilities_are_refused() {
        let workspace = std::env::temp_dir();
        // Shell: a command outside the plugin needs `shell`.
        let dir = folder(base(
            json!({"contributes":{"mcpServers":{"x":{"command":"bash","args":["-c","id"]}}}}),
        ));
        assert!(validate(&dir).unwrap_err().contains("shell permission"));
        let dir = folder(base(
            json!({"contributes":{"mcpServers":{"x":{"command":"node","args":["/etc/passwd"]}}}}),
        ));
        assert!(validate(&dir).is_err());
        // A shipped script runs; the project, network and secrets appear only when declared.
        let dir = folder(base(
            json!({"contributes":{"mcpServers":{"x":{"command":"node","args":["server.mjs"]}}},
            "permissions":{"secrets":["TOKEN"]}}),
        ));
        std::fs::write(dir.join("server.mjs"), "").unwrap();
        let m = validate(&dir).unwrap();
        std::env::set_var("LUMEN_PLUGIN_TEST_LEAK", "1");
        let plan = launch(&dir, &m, "x", &workspace, &|name| {
            Some(format!("value-{name}"))
        })
        .unwrap();
        assert!(!plan.env.contains_key("LUMEN_PROJECT_DIR"));
        assert!(!plan.env.contains_key("LUMEN_NETWORK_HOSTS"));
        assert!(!plan.env.contains_key("LUMEN_PLUGIN_TEST_LEAK"));
        assert_eq!(plan.env["TOKEN"], "value-TOKEN");
        assert!(launch(&dir, &m, "undeclared", &workspace, &|_| None).is_err());
        // Network: only declared https hosts.
        let p = Permissions {
            network: vec!["api.example.com".into(), "*.cdn.example.org".into()],
            ..Default::default()
        };
        assert!(allowed_url(&p, "https://api.example.com/v1").is_ok());
        assert!(allowed_url(&p, "https://a.cdn.example.org/x").is_ok());
        assert!(allowed_url(&p, "https://evil.test/")
            .unwrap_err()
            .contains("did not declare"));
        assert!(allowed_url(&p, "http://api.example.com/").is_err());
        assert!(allowed_url(&Permissions::default(), "https://api.example.com/").is_err());
    }

    #[tokio::test]
    async fn lifecycle_starts_disabled_and_updates_show_new_permissions() {
        let f = crate::test_support::Fixture::new();
        let src = folder(base(
            json!({"contributes":{"slashCommands":[{"name":"hello","prompt":"Say hi"}]}}),
        ));
        let review = inspect(&f.root, src.to_str().unwrap()).await.unwrap();
        assert_eq!(review["added"], json!([]));
        install(&f.root, review["token"].as_str().unwrap()).unwrap();
        assert_eq!(contributions(&f.root)["slashCommands"], json!([]));
        set_enabled(&f.root, "demo", true).unwrap();
        assert_eq!(
            contributions(&f.root)["slashCommands"][0]["prompt"],
            "Say hi"
        );
        // Same permissions: reload applies at once.
        assert_eq!(reload(&f.root, "demo").await.unwrap()["applied"], true);
        // More permissions: the diff comes back and the plugin stays as approved.
        std::fs::write(
            src.join(MANIFEST),
            base(json!({"version":"1.1.0","permissions":{"network":["api.example.com"]}}))
                .to_string(),
        )
        .unwrap();
        let review = reload(&f.root, "demo").await.unwrap();
        assert_eq!(review["applied"], false);
        assert_eq!(review["added"], json!(["Connect to api.example.com"]));
        install(&f.root, review["token"].as_str().unwrap()).unwrap();
        assert!(active(&f.root).is_empty());
        assert!(panel_fetch(&f.root, "demo", "https://api.example.com/")
            .await
            .is_err());
        // Undeclared secrets cannot be stored.
        assert!(set_secret(&f.root, "demo", "OTHER", "x").is_err());
        remove(&f.root, "demo").unwrap();
        assert!(list(&f.root).is_empty());
    }

    #[test]
    fn shipped_examples_are_valid_and_the_mcp_example_runs_scrubbed() {
        use std::io::{BufRead, Write};
        let examples = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../plugins/examples");
        for id in ["dusk-theme", "review-commands", "word-count"] {
            let m = validate(&examples.join(id)).unwrap_or_else(|e| panic!("{id}: {e}"));
            assert_eq!(m.id, id);
            assert!(
                grants(&m.permissions).is_empty(),
                "{id} should need no permissions"
            );
        }
        let godot = examples.join("../godot");
        let m = validate(&godot).unwrap_or_else(|e| panic!("godot: {e}"));
        assert_eq!(m.id, "godot");
        assert!(!grants(&m.permissions).is_empty());
        let dir = examples.join("word-count");
        let m = validate(&dir).unwrap();
        let plan = launch(&dir, &m, "words", &std::env::temp_dir(), &|_| None).unwrap();
        let mut child = std::process::Command::new(&plan.program)
            .args(&plan.args)
            .env_clear()
            .envs(&plan.env)
            .current_dir(&plan.cwd)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let mut input = child.stdin.take().unwrap();
        writeln!(
            input,
            r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{}}}}"#
        )
        .unwrap();
        writeln!(input, r#"{{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{{"name":"count_words","arguments":{{"text":"one two\nthree"}}}}}}"#).unwrap();
        drop(input);
        let lines: Vec<Value> = std::io::BufReader::new(child.stdout.take().unwrap())
            .lines()
            .map(|l| serde_json::from_str(&l.unwrap()).unwrap())
            .collect();
        child.wait().unwrap();
        assert_eq!(lines[0]["result"]["serverInfo"]["name"], "word-count");
        let counts: Value =
            serde_json::from_str(lines[1]["result"]["content"][0]["text"].as_str().unwrap())
                .unwrap();
        assert_eq!(counts, json!({"words": 3, "lines": 2, "characters": 13}));
    }
}
