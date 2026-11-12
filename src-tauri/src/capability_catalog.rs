//! Read-through discovery: keep native installations and their provenance, never copy login stores.
use serde::Serialize;
use serde_json::{json, Value};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub id: String,
    pub name: String,
    pub source: String,
    pub kind: String,
    pub status: String,
    pub reason: String,
    #[serde(skip)]
    pub path: PathBuf,
    #[serde(skip)]
    pub config: Value,
}
#[derive(Default)]
pub struct Catalog {
    pub entries: Vec<Entry>,
    pub warnings: Vec<String>,
}
fn id(path: &Path, name: &str) -> String {
    // Stable, deterministic namespaces prevent equal names in two accounts from overwriting.
    let mut hash = 0xcbf29ce484222325u64;
    let canonical = dunce::canonicalize(path).unwrap_or_else(|_| path.to_owned());
    for byte in canonical.to_string_lossy().bytes().chain(name.bytes()) {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("imported_{hash:016x}")
}
fn json_file(path: &Path) -> Result<Value, ()> {
    let metadata = std::fs::metadata(path).map_err(|_| ())?;
    if metadata.len() > 2_000_000 {
        return Err(());
    }
    let raw = std::fs::read_to_string(path).map_err(|_| ())?;
    if path.extension().is_some_and(|e| e == "toml") {
        let value: toml::Value = toml::from_str(&raw).map_err(|_| ())?;
        serde_json::to_value(value).map_err(|_| ())
    } else {
        serde_json::from_str(&raw).map_err(|_| ())
    }
}
impl Catalog {
    fn config(&mut self, path: &Path, source: &str, plugin: Option<&Path>) {
        if !path.is_file() {
            return;
        }
        let Ok(value) = json_file(path) else {
            self.warnings.push(format!(
                "{source}: configuration could not be read; source was not changed"
            ));
            return;
        };
        let servers = value.get("mcpServers").or_else(|| value.get("mcp_servers"));
        for (name, raw) in servers.and_then(Value::as_object).into_iter().flatten() {
            if name.starts_with("lumen_") || name.starts_with("imported_") {
                continue;
            }
            let gemini_sse = path.file_name().is_some_and(|name| name == "settings.json")
                && path
                    .parent()
                    .and_then(Path::file_name)
                    .is_some_and(|name| name == ".gemini")
                && raw["url"].is_string()
                && raw["httpUrl"].is_null()
                && raw["type"].is_null();
            let mut server = raw.clone();
            let Some(fields) = server.as_object_mut() else {
                continue;
            };
            if let Some(url) = fields
                .remove("serverUrl")
                .or_else(|| fields.remove("httpUrl"))
            {
                fields.insert("url".into(), url);
            }
            if let Some(headers) = fields.remove("http_headers") {
                fields.insert("headers".into(), headers);
            }
            let disabled = fields.remove("disabled") == Some(json!(true))
                || fields.remove("enabled") == Some(json!(false));
            fields.remove("trust");
            // Policies cannot be discarded when adapting an imported server.
            let unsupported = fields.keys().any(|key| {
                !matches!(
                    key.as_str(),
                    "type" | "command" | "args" | "env" | "url" | "headers" | "cwd"
                )
            });
            let transport = fields.get("type").and_then(Value::as_str).unwrap_or(
                if fields.contains_key("command") {
                    "stdio"
                } else {
                    "http"
                },
            );
            let unsupported = unsupported
                || gemini_sse
                || !matches!(transport, "stdio" | "http")
                || (fields.get("command").and_then(Value::as_str).is_none()
                    && fields.get("url").and_then(Value::as_str).is_none())
                || fields.get("args").is_some_and(|args| {
                    args.as_array()
                        .is_none_or(|args| args.iter().any(|v| !v.is_string()))
                })
                || ["env", "headers"].iter().any(|key| {
                    fields.get(*key).is_some_and(|v| {
                        v.as_object()
                            .is_none_or(|o| o.values().any(|v| !v.is_string()))
                    })
                });
            let unsupported = unsupported
                || if transport == "stdio" {
                    fields
                        .get("command")
                        .and_then(Value::as_str)
                        .is_none_or(str::is_empty)
                        || fields.contains_key("url")
                } else {
                    fields.contains_key("command")
                        || fields
                            .get("url")
                            .and_then(Value::as_str)
                            .and_then(|url| tauri::Url::parse(url).ok())
                            .is_none_or(|url| {
                                !matches!(url.scheme(), "https" | "http")
                                    || !url.username().is_empty()
                                    || url.password().is_some()
                            })
                };
            if fields.contains_key("url") && !fields.contains_key("type") {
                fields.insert("type".into(), json!("http"));
            }
            if let Some(plugin) = plugin {
                let root = plugin.to_string_lossy();
                replace_root(&mut server, &root);
            }
            let status = if disabled {
                "disabled"
            } else if unsupported {
                "needs-adapter"
            } else {
                "available"
            };
            self.entries.push(Entry {id:id(path,name),name:name.clone(),source:source.into(),kind:"mcp".into(),status:status.into(),reason:if disabled {"Disabled in its source configuration".into()}else if unsupported {"Native policy, transport or authentication options need an owner-runtime adapter; not silently removed".into()}else{String::new()},path:path.into(),config:server});
        }
    }
    fn skills(&mut self, folder: &Path, source: &str, depth: usize) {
        if depth > 3 {
            return;
        }
        let Ok(folder) = dunce::canonicalize(folder) else {
            return;
        };
        if folder.join("SKILL.md").is_file() {
            self.entries.push(Entry {
                id: id(&folder, "skill"),
                name: folder
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into(),
                source: source.into(),
                kind: "skill".into(),
                status: "available".into(),
                reason: String::new(),
                path: folder,
                config: Value::Null,
            });
            return;
        }
        if let Ok(children) = std::fs::read_dir(&folder) {
            for child in children.flatten().take(500) {
                if !child.file_name().to_string_lossy().starts_with('.') && child.path().is_dir() {
                    self.skills(&child.path(), source, depth + 1);
                }
            }
        }
    }
    fn plugins(&mut self, home: &Path, source: &str) {
        let index = home.join("plugins/installed_plugins.json");
        let Ok(value) = json_file(&index) else {
            return;
        };
        let settings = json_file(&home.join("settings.json")).unwrap_or(Value::Null);
        for (name, versions) in value["plugins"].as_object().into_iter().flatten() {
            if settings["enabledPlugins"][name] == false {
                continue;
            }
            for version in versions.as_array().into_iter().flatten() {
                // Project/local plugin installations are loaded only through their workspace.
                if version["scope"].as_str().is_some_and(|s| s != "user") {
                    continue;
                }
                let Some(path) = version["installPath"].as_str() else {
                    continue;
                };
                let Ok(root) = dunce::canonicalize(path) else {
                    continue;
                };
                let label = format!("{source} · {name}");
                self.skills(&root.join("skills"), &label, 0);
                self.config(&root.join(".mcp.json"), &label, Some(&root));
                self.config(&root.join("mcp_config.json"), &label, Some(&root));
                let manifest =
                    json_file(&root.join(".claude-plugin/plugin.json")).unwrap_or(Value::Null);
                if manifest["mcpServers"].is_object() {
                    self.config(
                        &root.join(".claude-plugin/plugin.json"),
                        &label,
                        Some(&root),
                    );
                }
                if root.join("hooks/hooks.json").exists() || manifest.get("hooks").is_some() {
                    self.entries.push(Entry {id:id(&root,"hooks"),name:name.clone(),source:source.into(),kind:"native-hooks".into(),status:"owner-runtime".into(),reason:"Lifecycle hooks require the source provider's runtime; skills and portable MCP components are discovered separately".into(),path:root,config:Value::Null});
                }
            }
        }
    }
    fn profile(&mut self, home: &Path, source: &str) {
        // Existence only: provider login credentials are never read or copied by discovery.
        if home.join("auth.json").is_file()
            && (home.join("config.toml").is_file()
                || home.file_name().is_some_and(|name| name == ".codex"))
        {
            self.entries.push(Entry {
                id: id(home, "native-account-tools"),
                name: "ChatGPT account tools".into(),
                source: source.into(),
                kind: "native-mcp".into(),
                status: "available".into(),
                reason: String::new(),
                path: home.into(),
                config: json!({"lumenNativeProfile":home}),
            });
        }
        for file in [
            "config.toml",
            ".claude.json",
            "settings.json",
            ".gemini/settings.json",
            ".gemini/config/mcp_config.json",
            ".gemini/antigravity/mcp_config.json",
        ] {
            self.config(&home.join(file), source, None);
        }
        self.skills(&home.join("skills"), source, 0);
        self.plugins(home, source);
    }
    pub fn servers(&self) -> serde_json::Map<String, Value> {
        self.entries
            .iter()
            .filter(|e| matches!(e.kind.as_str(), "mcp" | "native-mcp") && e.status == "available")
            .map(|e| (e.id.clone(), e.config.clone()))
            .collect()
    }
}
fn replace_root(value: &mut Value, root: &str) {
    match value {
        Value::String(text) => {
            *text = text
                .replace("${CLAUDE_PLUGIN_ROOT}", root)
                .replace("${CODEX_PLUGIN_ROOT}", root)
        }
        Value::Array(values) => {
            for value in values {
                replace_root(value, root);
            }
        }
        Value::Object(values) => {
            for value in values.values_mut() {
                replace_root(value, root);
            }
        }
        _ => {}
    }
}
pub fn discover(data: &Path, workspace: Option<&Path>) -> Catalog {
    let home = if cfg!(test) || std::env::var_os("LUMEN_TEST_DATA_DIR").is_some() {
        None
    } else {
        std::env::var_os("LUMEN_DISCOVERY_HOME")
            .or_else(|| std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }))
            .map(PathBuf::from)
    };
    let mut catalog = discover_at(data, workspace, home.as_deref());
    if home.is_some() {
        for (fixed, native, label) in [
            ("LUMEN_DISCOVERY_CLAUDE_HOME", "CLAUDE_CONFIG_DIR", "Claude"),
            (
                "LUMEN_DISCOVERY_CODEX_HOME",
                "CODEX_HOME",
                "ChatGPT / Codex",
            ),
        ] {
            if let Some(path) = std::env::var_os(fixed).or_else(|| std::env::var_os(native)) {
                catalog.profile(&PathBuf::from(path), label);
            }
        }
        finalize(data, &mut catalog);
    }
    catalog
}
pub fn discover_at(data: &Path, workspace: Option<&Path>, home: Option<&Path>) -> Catalog {
    let mut out = Catalog::default();
    if let Some(home) = home {
        for (folder, label) in [
            (home.join(".claude"), "Claude"),
            (home.join(".codex"), "ChatGPT / Codex"),
        ] {
            out.profile(&folder, label);
        }
        out.config(&home.join(".claude.json"), "Claude", None);
        out.skills(&home.join(".agents/skills"), "Agent skills", 0);
        for file in [
            ".gemini/settings.json",
            ".gemini/config/mcp_config.json",
            ".gemini/antigravity/mcp_config.json",
        ] {
            out.config(&home.join(file), "Google", None);
        }
        out.skills(&home.join(".gemini/skills"), "Google", 0);
        #[cfg(target_os = "macos")]
        out.config(
            &home.join("Library/Application Support/Claude/claude_desktop_config.json"),
            "Claude Desktop",
            None,
        );
        #[cfg(windows)]
        {
            let appdata = if cfg!(test) {
                home.join("AppData/Roaming")
            } else {
                std::env::var_os("APPDATA")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| home.join("AppData/Roaming"))
            };
            out.config(
                &appdata.join("Claude/claude_desktop_config.json"),
                "Claude Desktop",
                None,
            );
        }
    }
    if let Ok(accounts) = std::fs::read_dir(data.join("accounts")) {
        for account in accounts.flatten().take(100) {
            if account.path().is_dir() {
                out.profile(
                    &account.path(),
                    &format!("Lumen account {}", account.file_name().to_string_lossy()),
                );
            }
        }
    }
    if let Some(workspace) = workspace {
        for file in [
            ".mcp.json",
            ".codex/config.toml",
            ".gemini/settings.json",
            ".agents/mcp_config.json",
        ] {
            out.config(&workspace.join(file), "Current project", None);
        }
        for folder in [
            ".agents/skills",
            ".claude/skills",
            ".codex/skills",
            ".gemini/skills",
        ] {
            out.skills(&workspace.join(folder), "Current project", 0);
        }
    }
    finalize(data, &mut out);
    out
}
fn finalize(data: &Path, out: &mut Catalog) {
    let disabled = json_file(&data.join("shared/discovery.json")).unwrap_or(Value::Null);
    let mut seen = BTreeSet::new();
    out.entries.retain(|e| seen.insert(e.id.clone()));
    for entry in &mut out.entries {
        if disabled["disabled"]
            .as_array()
            .is_some_and(|ids| ids.contains(&json!(entry.id)))
        {
            entry.status = "disabled".into();
        }
    }
    out.entries
        .sort_by(|a, b| (&a.source, &a.name, &a.id).cmp(&(&b.source, &b.name, &b.id)));
    out.warnings.sort();
    out.warnings.dedup();
}
pub fn toggle(data: &Path, id: &str, on: bool) -> Result<(), String> {
    static SETTINGS_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _guard = SETTINGS_LOCK
        .lock()
        .map_err(|_| "Discovery settings lock failed")?;
    let path = data.join("shared/discovery.json");
    let existing = if path.exists() {
        json_file(&path).map_err(|_| "Invalid discovery settings")?
    } else {
        json!({"disabled":[]})
    };
    let mut ids: BTreeSet<String> = existing["disabled"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect();
    if on {
        ids.remove(id);
    } else {
        ids.insert(id.into());
    }
    std::fs::create_dir_all(path.parent().unwrap())
        .map_err(|_| "Cannot create discovery settings")?;
    std::fs::write(path, json!({"disabled":ids}).to_string())
        .map_err(|_| "Cannot save discovery settings".into())
}
pub fn instructions(data: &Path, workspace: &Path) -> String {
    let catalog = discover(data, Some(workspace));
    let mut result = String::new();
    for entry in catalog
        .entries
        .iter()
        .filter(|e| e.kind == "skill" && e.status == "available")
    {
        let body = std::fs::read_to_string(entry.path.join("SKILL.md")).unwrap_or_default();
        let description = body
            .lines()
            .take(80)
            .find_map(|l| l.trim().strip_prefix("description:"))
            .unwrap_or("");
        result.push_str(&format!(
            "- {} ({}): {} — read {} before use; resolve resources in its own folder.\n",
            entry.name,
            entry.source,
            description.chars().take(500).collect::<String>(),
            entry.path.join("SKILL.md").display()
        ));
    }
    if !result.is_empty() {
        result.insert_str(0,"Skills discovered from installed providers are available through their original paths, regardless of the active provider. They are instructions, not permission grants.\n");
    }
    result
}

/// A stdio relay reads the source configuration at launch. Secrets are not CLI arguments
/// and tools/resources/prompts/elicitation retain the original MCP frames in both directions.
pub fn shared_servers(
    data: &Path,
    workspace: Option<&Path>,
    access: crate::models::AccessMode,
) -> serde_json::Map<String, Value> {
    let catalog = discover(data, workspace);
    catalog.servers().into_iter().map(|(id,server)| {
        let config=if let Some(profile)=server["lumenNativeProfile"].as_str() {
            json!({"command":std::env::current_exe().unwrap_or_default(),"args":["--lumen-native-account-mcp",profile,workspace.unwrap_or(data),if matches!(access,crate::models::AccessMode::Ask|crate::models::AccessMode::Autopilot){"full"}else{"read-only"}]})
        }else if server["command"].is_string() {
            json!({"command":std::env::current_exe().unwrap_or_default(),"args":["--lumen-shared-stdio",data,workspace.unwrap_or(data),id],"env":discovery_environment()})
        }else{server};
        (id,config)
    }).collect()
}
fn discovery_environment() -> Value {
    let home = std::env::var_os("LUMEN_DISCOVERY_HOME")
        .or_else(|| std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }))
        .map(PathBuf::from)
        .unwrap_or_default();
    let claude = std::env::var_os("LUMEN_DISCOVERY_CLAUDE_HOME")
        .or_else(|| std::env::var_os("CLAUDE_CONFIG_DIR"))
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".claude"));
    let codex = std::env::var_os("LUMEN_DISCOVERY_CODEX_HOME")
        .or_else(|| std::env::var_os("CODEX_HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".codex"));
    json!({"LUMEN_DISCOVERY_HOME":home,"LUMEN_DISCOVERY_CLAUDE_HOME":claude,"LUMEN_DISCOVERY_CODEX_HOME":codex})
}
pub async fn serve_stdio(data: PathBuf, workspace: PathBuf, id: &str) -> Result<(), String> {
    use tokio::io::AsyncWriteExt;
    let catalog = discover(&data, Some(&workspace));
    let server = catalog
        .entries
        .iter()
        .find(|entry| entry.id == id && entry.kind == "mcp" && entry.status == "available")
        .ok_or("Integration is disabled, removed or unsupported")?;
    let command = server.config["command"]
        .as_str()
        .ok_or("Not a stdio integration")?;
    let mut process = crate::proc::command(expand(command)?);
    #[cfg(unix)]
    process.process_group(0);
    for arg in server.config["args"].as_array().into_iter().flatten() {
        process.arg(expand(arg.as_str().ok_or("Invalid MCP argument")?)?);
    }
    for (key, value) in server.config["env"].as_object().into_iter().flatten() {
        process.env(
            key,
            expand(value.as_str().ok_or("Invalid MCP environment")?)?,
        );
    }
    let cwd = server.config["cwd"]
        .as_str()
        .map(PathBuf::from)
        .map(|p| {
            if p.is_absolute() {
                p
            } else {
                workspace.join(p)
            }
        })
        .unwrap_or(workspace);
    process
        .current_dir(cwd)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);
    let mut child = crate::proc::ServiceChild::spawn(process)
        .map_err(|_| "Could not start imported MCP server")?;
    let mut input = child.stdin.take().ok_or("MCP input unavailable")?;
    let mut output = child.stdout.take().ok_or("MCP output unavailable")?;
    let mut stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let result = tokio::select! {
        result=tokio::io::copy(&mut stdin,&mut input)=>result.map(|_|()),
        result=async {let mut block=[0u8;8192];use tokio::io::AsyncReadExt;loop {let n=output.read(&mut block).await?;if n==0 {return Ok::<(),std::io::Error>(());}stdout.write_all(&block[..n]).await?;stdout.flush().await?;}}=>result,
    };
    drop(input);
    child.finish(result.is_ok()).await?;
    result.map_err(|_| "Imported MCP connection closed".into())
}
fn expand(text: &str) -> Result<String, String> {
    let regex = regex::Regex::new(r"\$\{([A-Za-z_][A-Za-z0-9_]*)(?::-([^}]*))?\}").unwrap();
    let mut missing = false;
    let result = regex
        .replace_all(text, |captures: &regex::Captures| {
            std::env::var(&captures[1])
                .ok()
                .or_else(|| captures.get(2).map(|m| m.as_str().into()))
                .unwrap_or_else(|| {
                    missing = true;
                    String::new()
                })
        })
        .to_string();
    if missing || result.contains("${") {
        Err("Imported MCP requires an unavailable environment or native plugin setting".into())
    } else {
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn write(path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    #[test]
    fn concurrent_claude_configs_keep_their_workspace_and_are_removed_on_drop() {
        let f = crate::test_support::Fixture::new();
        let a = f.directory("a");
        let b = f.directory("b");
        write(
            &a.join(".mcp.json"),
            r#"{"mcpServers":{"one":{"command":"one"}}}"#,
        );
        write(
            &b.join(".mcp.json"),
            r#"{"mcpServers":{"two":{"command":"two"}}}"#,
        );
        let (first, one, guard_a) = crate::integrations::args_for(
            &f.root,
            &crate::models::Provider::Claude,
            Some(&a),
            crate::models::AccessMode::Ask,
        )
        .unwrap();
        let (second, two, guard_b) = crate::integrations::args_for(
            &f.root,
            &crate::models::Provider::Claude,
            Some(&b),
            crate::models::AccessMode::Ask,
        )
        .unwrap();
        assert_ne!(first[1], second[1]);
        let content: Value =
            serde_json::from_str(&std::fs::read_to_string(&first[1]).unwrap()).unwrap();
        assert!(content["mcpServers"].get(&one[0]).is_some());
        assert!(content["mcpServers"].get(&two[0]).is_none());
        drop(guard_a);
        assert!(!Path::new(&first[1]).exists());
        assert!(Path::new(&second[1]).exists());
        drop(guard_b);
        assert!(!Path::new(&second[1]).exists());
    }
    #[test]
    fn discovers_existing_accounts_and_preserves_credentials_policies_and_sources() {
        let f = crate::test_support::Fixture::new();
        let data = f.directory("data");
        let home = f.directory("home");
        write(
            &home.join(".claude.json"),
            r#"{"mcpServers":{"github":{"command":"fixture","env":{"PRIVATE":"secret-value"}},"blocked":{"url":"https://example.test","oauth":{"clientId":"native"}},"off":{"command":"fixture","disabled":true}}}"#,
        );
        write(
            &data.join("accounts/a/config.toml"),
            "[mcp_servers.github]\ncommand = 'second-owner'\n",
        );
        write(
            &data.join("accounts/b/.claude.json"),
            r#"{"mcpServers":{"github":{"command":"third-owner"}}}"#,
        );
        let before = std::fs::read(home.join(".claude.json")).unwrap();
        let catalog = discover_at(&data, None, Some(&home));
        let github: Vec<_> = catalog
            .entries
            .iter()
            .filter(|e| e.name == "github")
            .collect();
        assert_eq!(github.len(), 3);
        assert_eq!(
            github.iter().map(|e| &e.id).collect::<BTreeSet<_>>().len(),
            3
        );
        assert!(
            catalog
                .entries
                .iter()
                .find(|e| e.name == "blocked")
                .unwrap()
                .status
                == "needs-adapter"
        );
        assert!(
            catalog
                .entries
                .iter()
                .find(|e| e.name == "off")
                .unwrap()
                .status
                == "disabled"
        );
        assert!(!serde_json::to_string(&catalog.entries)
            .unwrap()
            .contains("secret-value"));
        assert_eq!(std::fs::read(home.join(".claude.json")).unwrap(), before);
        let owner = github
            .iter()
            .find(|e| e.source.contains("account a"))
            .unwrap();
        toggle(&data, &owner.id, false).unwrap();
        assert!(!discover_at(&data, None, Some(&home))
            .servers()
            .contains_key(&owner.id));
        toggle(&data, &owner.id, true).unwrap();
        assert!(discover_at(&data, None, Some(&home))
            .servers()
            .contains_key(&owner.id));
    }
    #[test]
    fn project_components_never_leak_to_another_workspace() {
        let f = crate::test_support::Fixture::new();
        let a = f.directory("a");
        let b = f.directory("b");
        write(
            &a.join(".mcp.json"),
            r#"{"mcpServers":{"project":{"command":"project-tool"}}}"#,
        );
        write(
            &a.join(".claude/skills/design/SKILL.md"),
            "---\ndescription: scoped skill\n---\nRead resources/file.txt.",
        );
        write(
            &a.join(".claude/skills/design/resources/file.txt"),
            "resource preserved",
        );
        assert_eq!(discover_at(&f.root, Some(&a), None).servers().len(), 1);
        assert!(discover_at(&f.root, Some(&b), None).entries.is_empty());
        assert!(instructions(&f.root, &a).contains("scoped skill"));
        assert!(!instructions(&f.root, &b).contains("scoped skill"));
        assert_eq!(
            std::fs::read_to_string(a.join(".claude/skills/design/resources/file.txt")).unwrap(),
            "resource preserved"
        );
    }
    #[test]
    fn installed_plugins_use_original_resources_and_report_native_hooks() {
        let f = crate::test_support::Fixture::new();
        let home = f.directory("home");
        let plugin = f.directory("plugin");
        write(
            &plugin.join("skills/review/SKILL.md"),
            "---\ndescription: plugin review\n---\n",
        );
        write(
            &plugin.join(".mcp.json"),
            r#"{"mcpServers":{"plugin-tool":{"command":"node","args":["${CLAUDE_PLUGIN_ROOT}/server.js"]}}}"#,
        );
        write(&plugin.join("hooks/hooks.json"), "{}");
        write(
            &home.join(".claude/plugins/installed_plugins.json"),
            &json!({"plugins":{"review@market":[{"scope":"user","installPath":plugin}]}})
                .to_string(),
        );
        let result = discover_at(&f.root, None, Some(&home));
        assert!(result
            .entries
            .iter()
            .any(|e| e.kind == "skill" && e.path == plugin.join("skills/review")));
        let server = result.servers().into_values().next().unwrap();
        assert!(!server.to_string().contains("${CLAUDE_PLUGIN_ROOT}"));
        assert!(result
            .entries
            .iter()
            .any(|e| e.kind == "native-hooks" && e.status == "owner-runtime"));
        write(
            &home.join(".claude/settings.json"),
            r#"{"enabledPlugins":{"review@market":false}}"#,
        );
        assert!(discover_at(&f.root, None, Some(&home)).entries.is_empty());
    }
    #[test]
    fn adapters_use_references_and_refresh_google_without_changing_native_settings() {
        let f = crate::test_support::Fixture::new();
        let project = f.directory("project");
        write(
            &f.root.join("accounts/owner/.claude.json"),
            r#"{"mcpServers":{"service":{"command":"fixture","env":{"PRIVATE":"secret-value"}}}}"#,
        );
        let (args, names, _guard) = crate::integrations::args_for(
            &f.root,
            &crate::models::Provider::Gpt,
            Some(&project),
            crate::models::AccessMode::Autopilot,
        )
        .unwrap();
        assert_eq!(names.len(), 1);
        assert!(args.iter().any(|a| a.contains("--lumen-shared-stdio")));
        assert!(!args.join(" ").contains("secret-value"));
        for provider in [
            crate::models::Provider::Gemini,
            crate::models::Provider::Antigravity,
        ] {
            let config = f.directory(provider.binary());
            crate::graph_mcp::configure_google_for(
                &config,
                &provider,
                &f.root,
                false,
                Some(&project),
                crate::models::AccessMode::Autopilot,
            )
            .unwrap();
            let relative = if provider == crate::models::Provider::Gemini {
                ".gemini/settings.json"
            } else {
                ".gemini/config/mcp_config.json"
            };
            let file = config.join(relative);
            let first = std::fs::read(&file).unwrap();
            let value: Value = serde_json::from_slice(&first).unwrap();
            assert!(value["mcpServers"].get(&names[0]).is_some());
            assert!(!String::from_utf8(first.clone())
                .unwrap()
                .contains("secret-value"));
            crate::graph_mcp::configure_google_for(
                &config,
                &provider,
                &f.root,
                false,
                Some(&project),
                crate::models::AccessMode::Autopilot,
            )
            .unwrap();
            assert_eq!(std::fs::read(&file).unwrap(), first);
        }
    }
}
