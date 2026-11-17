//! One provider-neutral installation directory. Provider-specific hooks are not portable.
use serde_json::{json, Value};
use std::path::Path;
fn root(data: &Path) -> std::path::PathBuf {
    data.join("shared")
}
pub fn status(data: &Path) -> Result<Value, String> {
    let base = root(data);
    std::fs::create_dir_all(base.join("skills")).map_err(|e| e.to_string())?;
    let mut skills = vec![];
    for entry in std::fs::read_dir(base.join("skills")).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        if entry.path().join("SKILL.md").is_file() {
            skills.push(entry.file_name().to_string_lossy().to_string());
        }
    }
    skills.sort();
    let path = base.join("mcp.json");
    let mcp = if path.exists() {
        std::fs::read_to_string(path).map_err(|e| e.to_string())?
    } else {
        "{\"mcpServers\": {}}".into()
    };
    Ok(json!({"path":base.to_string_lossy(), "skills":skills, "mcp":mcp}))
}
pub fn install_skill(data: &Path, source: &Path) -> Result<(), String> {
    let source = dunce::canonicalize(source).map_err(|e| e.to_string())?;
    if !source.join("SKILL.md").is_file() {
        return Err("Choose a skill folder containing SKILL.md".into());
    }
    let name = source.file_name().ok_or("Invalid skill folder")?;
    let base = root(data).join("skills");
    std::fs::create_dir_all(&base).map_err(|e| e.to_string())?;
    let base = dunce::canonicalize(base).map_err(|e| e.to_string())?;
    if source.starts_with(&base) || base.starts_with(&source) {
        return Err("Choose a source outside the shared installation folder".into());
    }
    let dest = base.join(name);
    if dest.exists() {
        return Err("A skill with this folder name is already installed".into());
    }
    fn copy(source: &Path, dest: &Path, depth: usize, budget: &mut u64) -> Result<(), String> {
        if depth > 32 {
            return Err("Skill directory is too deep".into());
        }
        std::fs::create_dir(dest).map_err(|e| e.to_string())?;
        for item in std::fs::read_dir(source).map_err(|e| e.to_string())? {
            let item = item.map_err(|e| e.to_string())?;
            let ty = item.file_type().map_err(|e| e.to_string())?;
            if ty.is_symlink() {
                return Err("Skill imports cannot contain symbolic links".into());
            }
            if ty.is_dir() {
                copy(
                    &item.path(),
                    &dest.join(item.file_name()),
                    depth + 1,
                    budget,
                )?;
            } else if ty.is_file() {
                *budget = budget
                    .checked_sub(item.metadata().map_err(|e| e.to_string())?.len())
                    .ok_or("Skill exceeds 100 MB")?;
                std::fs::copy(item.path(), dest.join(item.file_name()))
                    .map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }
    // Staging keeps failed imports out of the active skill inventory.
    let staging = base.join(format!(".install-{}", crate::models::id()));
    if let Err(e) = copy(&source, &staging, 0, &mut 100_000_000) {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(e);
    }
    std::fs::rename(&staging, &dest).map_err(|e| e.to_string())
}
pub fn save_mcp(data: &Path, text: &str) -> Result<(), String> {
    if text.len() > 1_000_000 {
        return Err("MCP configuration is too large".into());
    }
    let v: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let servers = v["mcpServers"]
        .as_object()
        .ok_or("Expected an mcpServers object")?;
    for (name, server) in servers {
        if name.is_empty()
            || !name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            return Err("Use simple MCP server names".into());
        }
        if server["command"].as_str().is_none() && server["url"].as_str().is_none() {
            return Err("Each MCP server needs command or url".into());
        }
        if server["type"] == "sse" || server["type"] == "ws" {
            return Err("Cross-provider MCP supports stdio and HTTP; SSE/WebSocket need a provider-specific adapter".into());
        }
    }
    std::fs::create_dir_all(root(data)).map_err(|e| e.to_string())?;
    std::fs::write(
        root(data).join("mcp.json"),
        serde_json::to_vec_pretty(&v).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}
/// Turns a request that worked into a shared skill every assistant can reuse.
/// Returns the folder name; an existing recipe is never overwritten.
pub fn save_recipe(data: &Path, request: &str, outcome: &str) -> Result<String, String> {
    let request = request.trim();
    if request.is_empty() {
        return Err("This reply has no request to save".into());
    }
    let words: Vec<String> = request
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .take(6)
        .map(str::to_lowercase)
        .collect();
    let slug = format!("recipe-{}", words.join("-"))
        .chars()
        .take(60)
        .collect::<String>();
    let base = root(data).join("skills");
    let (mut name, mut n) = (slug.clone(), 1);
    while base.join(&name).exists() {
        n += 1;
        name = format!("{slug}-{n}");
    }
    let clip = |s: &str, max: usize| s.trim().chars().take(max).collect::<String>();
    let description = clip(
        &request.split_whitespace().collect::<Vec<_>>().join(" "),
        200,
    );
    let body = format!(
        "---
name: {name}
description: Saved recipe: {description}
---

# Recipe

A task the user saved after it worked. When asked to repeat it, redo it for the current project state instead of copying old results, keep the same scope and checks, and report what differs.

## Request

{}

## Result that was accepted

{}
",
        clip(request, 4000),
        clip(outcome, 4000)
    );
    std::fs::create_dir_all(base.join(&name)).map_err(|e| e.to_string())?;
    std::fs::write(base.join(&name).join("SKILL.md"), body).map_err(|e| e.to_string())?;
    Ok(name)
}

/// Folders of the shared skills that are switched on.
pub fn enabled_skills(data: &Path) -> Vec<std::path::PathBuf> {
    let Ok(state) = status(data) else {
        return vec![];
    };
    state["skills"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(|name| root(data).join("skills").join(name))
        .filter(|p| p.join("SKILL.md").is_file())
        .collect()
}

/// Personal preferences the user confirmed for every project.
pub fn global_memory(data: &Path) -> std::path::PathBuf {
    root(data).join("MEMORY.md")
}

/// Appends a confirmed global fact (deduplicated, secret-filtered, dated).
pub fn remember_global(data: &Path, fact: &str, now_secs: i64) -> Result<(), String> {
    let fact = fact.split_whitespace().collect::<Vec<_>>().join(" ");
    if fact.is_empty() || fact.chars().count() > 300 || crate::context::secret().is_match(&fact) {
        return Err("This fact cannot be saved".into());
    }
    let path = global_memory(data);
    let mut text = std::fs::read_to_string(&path).unwrap_or_default();
    if text.contains(&fact) {
        return Ok(());
    }
    if text.trim().is_empty() {
        text = "# Lumen global memory\n\nConfirmed preferences shared by every assistant in every project. Edit or delete freely.\n\n".into();
    }
    let date = chrono::DateTime::from_timestamp(now_secs, 0)
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_default();
    if !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str(&format!("- {fact} _({date})_\n"));
    std::fs::create_dir_all(root(data)).map_err(|e| e.to_string())?;
    std::fs::write(path, text).map_err(|e| e.to_string())
}

pub fn instructions(data: &Path) -> Result<String, String> {
    let memory = std::fs::read_to_string(global_memory(data)).unwrap_or_default();
    let memory: String = memory.chars().take(20_000).collect();
    let mut result = if memory.lines().any(|l| l.starts_with("- ")) {
        format!(
            "User's global memory ({}, confirmed preferences for every project; project notes win on conflict):\n{}\n",
            global_memory(data).display(),
            memory.trim()
        )
    } else {
        String::new()
    };
    let state = status(data)?;
    if state["skills"].as_array().is_none_or(|a| a.is_empty()) {
        return Ok(result);
    }
    result.push_str("Shared skills available in Lumen. When a skill applies, read its SKILL.md before acting and resolve resources relative to its folder:\n");
    for name in state["skills"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
    {
        let path = root(data).join("skills").join(name).join("SKILL.md");
        let body = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let description = body
            .lines()
            .take(80)
            .find_map(|line| line.trim().strip_prefix("description:"))
            .unwrap_or("");
        result.push_str(&format!(
            "- {name}: {} ({})\n",
            description.trim().chars().take(1000).collect::<String>(),
            path.display()
        ));
    }
    Ok(result)
}
fn load(data: &Path) -> Result<Value, String> {
    let path = root(data).join("mcp.json");
    if !path.exists() {
        return Ok(json!({"mcpServers":{}}));
    }
    serde_json::from_slice(&std::fs::read(&path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}
/// Enabled servers only; `"disabled": true` is Lumen's ON/OFF switch and is never passed on.
pub fn enabled(data: &Path) -> Result<serde_json::Map<String, Value>, String> {
    Ok(load(data)?["mcpServers"]
        .as_object()
        .ok_or("Invalid MCP config")?
        .iter()
        .filter(|(_, s)| s["disabled"] != true)
        .map(|(k, s)| {
            let mut s = s.clone();
            if let Some(o) = s.as_object_mut() {
                o.remove("disabled");
            }
            (k.clone(), s)
        })
        .collect())
}
/// Health: stdio servers need a resolvable command; HTTP servers need a URL.
pub fn health(data: &Path) -> Result<Value, String> {
    let v = load(data)?;
    let mut out = vec![];
    for (name, s) in v["mcpServers"].as_object().into_iter().flatten() {
        let healthy = match s["command"].as_str() {
            Some(cmd) => which::which(cmd).is_ok() || Path::new(cmd).is_file(),
            None => s["url"].as_str().is_some(),
        };
        out.push(json!({
            "name": name,
            "enabled": s["disabled"] != true,
            "healthy": healthy,
            "target": s["command"].as_str().or(s["url"].as_str()).unwrap_or(""),
        }));
    }
    Ok(Value::Array(out))
}
pub fn toggle(data: &Path, name: &str, on: bool) -> Result<(), String> {
    let mut v = load(data)?;
    let server = v["mcpServers"]
        .get_mut(name)
        .and_then(Value::as_object_mut)
        .ok_or("MCP server not found")?;
    if on {
        server.remove("disabled");
    } else {
        server.insert("disabled".into(), Value::Bool(true));
    }
    save_mcp(data, &v.to_string())
}
pub fn add_server(data: &Path, name: &str, target: &str) -> Result<(), String> {
    let mut v = load(data)?;
    let target = target.trim();
    let server = if target.starts_with("http://") || target.starts_with("https://") {
        json!({"type":"http","url":target})
    } else {
        let mut parts = shell_words(target).into_iter();
        let command = parts.next().ok_or("Enter a command or URL")?;
        json!({"command":command,"args":parts.collect::<Vec<_>>()})
    };
    let servers = v["mcpServers"]
        .as_object_mut()
        .ok_or("Invalid MCP config")?;
    if servers.contains_key(name) {
        return Err("A server with this name already exists".into());
    }
    servers.insert(name.into(), server);
    save_mcp(data, &v.to_string())
}
pub fn remove_server(data: &Path, name: &str) -> Result<(), String> {
    let mut v = load(data)?;
    v["mcpServers"]
        .as_object_mut()
        .and_then(|s| s.remove(name))
        .ok_or("MCP server not found")?;
    save_mcp(data, &v.to_string())
}
fn shell_words(s: &str) -> Vec<String> {
    let mut out = vec![];
    let mut cur = String::new();
    let mut quote = None;
    for c in s.chars() {
        match (quote, c) {
            (None, '"' | '\'') => quote = Some(c),
            (Some(q), c) if c == q => quote = None,
            (None, c) if c.is_whitespace() => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            _ => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}
#[cfg(test)]
pub fn args(
    data: &Path,
    provider: &crate::models::Provider,
) -> Result<(Vec<String>, Vec<String>), String> {
    let (args, names, _config) =
        args_for(data, provider, None, crate::models::AccessMode::Autopilot)?;
    Ok((args, names))
}
pub struct ActiveConfig(std::path::PathBuf);
impl Drop for ActiveConfig {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
pub type Arguments = (Vec<String>, Vec<String>, Option<ActiveConfig>);
pub fn allow_unfiltered_mcp(access: crate::models::AccessMode) -> bool {
    matches!(
        access,
        crate::models::AccessMode::Ask | crate::models::AccessMode::Autopilot
    )
}
pub fn read_only_tool(tool: &Value) -> bool {
    tool["annotations"]["readOnlyHint"] == true && tool["annotations"]["destructiveHint"] != true
}
pub fn args_for(
    data: &Path,
    provider: &crate::models::Provider,
    workspace: Option<&Path>,
    access: crate::models::AccessMode,
) -> Result<Arguments, String> {
    let mut servers = crate::capability_catalog::shared_servers(data, workspace, access);
    let manual = enabled(data)?;
    if allow_unfiltered_mcp(access) {
        servers.extend(manual);
    }
    if servers.is_empty() {
        return Ok((vec![], vec![], None));
    }
    let names: Vec<String> = servers.keys().cloned().collect();
    if *provider == crate::models::Provider::Claude {
        let dir = root(data).join("active");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let path = dir.join(format!("mcp-{}.json", crate::models::id()));
        let guard = ActiveConfig(path.clone());
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        use std::io::Write;
        options
            .open(&path)
            .and_then(|mut file| {
                file.write_all(
                    &serde_json::to_vec_pretty(&json!({"mcpServers":servers}))
                        .map_err(std::io::Error::other)?,
                )
            })
            .map_err(|e| e.to_string())?;
        return Ok((
            vec!["--mcp-config".into(), path.to_string_lossy().into()],
            names,
            Some(guard),
        ));
    }
    let mut args = vec![];
    for (name, server) in &servers {
        let mut converted = server.clone();
        if let Some(o) = converted.as_object_mut() {
            o.remove("type");
            if server["args"][0] == "--lumen-native-account-mcp" {
                o.insert(
                    "env_vars".into(),
                    json!(["LUMEN_BROWSER_URL", "LUMEN_BROWSER_CAP"]),
                );
            }
            if let Some(headers) = o.remove("headers") {
                o.insert("http_headers".into(), headers);
            }
        }
        let table: toml::Value = serde_json::from_value(converted).map_err(|e| e.to_string())?;
        // Serialize as an inline TOML table, not JSON (the CLI expects TOML).
        fn inline(v: &toml::Value) -> String {
            match v {
                toml::Value::Table(t) => format!(
                    "{{{}}}",
                    t.iter()
                        .map(|(k, v)| format!(
                            "{} = {}",
                            serde_json::to_string(k).unwrap(),
                            inline(v)
                        ))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                toml::Value::Array(a) => {
                    format!("[{}]", a.iter().map(inline).collect::<Vec<_>>().join(", "))
                }
                other => other.to_string(),
            }
        }
        args.extend([
            "-c".into(),
            format!("mcp_servers.{name}={}", inline(&table)),
        ]);
    }
    Ok((args, names, None))
}

#[cfg(test)]
mod tests {
    #[test]
    fn recipes_become_unique_shared_skills() {
        let dir = std::env::temp_dir().join(format!("lumen-recipe-{}", crate::models::id()));
        let a = super::save_recipe(&dir, "Build the release, then run tests!", "Built OK").unwrap();
        let b = super::save_recipe(&dir, "Build the release, then run tests!", "Again").unwrap();
        assert_eq!(a, "recipe-build-the-release-then-run-tests");
        assert_eq!(b, format!("{a}-2"));
        assert!(super::instructions(&dir)
            .unwrap()
            .contains("Saved recipe: Build the release"));
        assert!(super::save_recipe(&dir, "  ", "x").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
    use super::*;
    #[test]
    fn shared_skill_resources_and_mcp_adapters() {
        let temp = std::env::temp_dir().join(format!("lumen-integrations-{}", crate::models::id()));
        let source = temp.join("source").join("example");
        let data = temp.join("data");
        std::fs::create_dir_all(source.join("scripts")).unwrap();
        std::fs::write(
            source.join("SKILL.md"),
            "---\nname: example\ndescription: Example skill\n---\nRead resources.",
        )
        .unwrap();
        std::fs::write(source.join("scripts/helper.txt"), "resource").unwrap();
        install_skill(&data, &source).unwrap();
        assert_eq!(
            std::fs::read_to_string(root(&data).join("skills/example/scripts/helper.txt")).unwrap(),
            "resource"
        );
        assert!(instructions(&data).unwrap().contains("Example skill"));
        assert!(install_skill(&data, &source).is_err());
        save_mcp(&data, r#"{"mcpServers":{"local":{"command":"node","args":["path with spaces/server.js"],"env":{"TEST":"value"}},"remote":{"type":"http","url":"https://example.test/mcp","headers":{"X-Test":"value"}}}}"#).unwrap();
        let gpt = args(&data, &crate::models::Provider::Gpt).unwrap().0;
        for pair in gpt.chunks(2) {
            assert_eq!(pair[0], "-c");
            let config: toml::Value = pair[1].parse().unwrap();
            assert!(config.get("mcp_servers").is_some());
        }
        assert!(gpt.iter().any(|s| s.contains("http_headers")));
        let claude = args(&data, &crate::models::Provider::Claude).unwrap().0;
        assert_eq!(claude[0], "--mcp-config");
        toggle(&data, "local", false).unwrap();
        assert_eq!(
            args(&data, &crate::models::Provider::Claude).unwrap().1,
            vec!["remote"]
        );
        add_server(&data, "files", "npx -y \"@scope/server\" ./dir").unwrap();
        assert_eq!(health(&data).unwrap().as_array().unwrap().len(), 3);
        remove_server(&data, "files").unwrap();
        assert!(save_mcp(&data, "{}").is_err());
        assert!(save_mcp(
            &data,
            r#"{"mcpServers":{"bad":{"type":"sse","url":"https://example.test"}}}"#
        )
        .is_err());
        // Cleanup is confined to this unique test directory under the system temp folder.
        std::fs::remove_dir_all(temp).unwrap();
    }
}

#[cfg(test)]
mod global_memory_tests {
    #[test]
    fn confirmed_global_facts_are_stored_once_and_injected() {
        let f = crate::test_support::Fixture::new();
        let dir = f.directory("global-memory");
        assert!(!super::instructions(&dir).unwrap().contains("global memory"));
        super::remember_global(&dir, "Reply in Italian", 0).unwrap();
        super::remember_global(&dir, "Reply  in Italian", 0).unwrap();
        assert!(super::remember_global(&dir, "my password is x", 0).is_err());
        let text = std::fs::read_to_string(super::global_memory(&dir)).unwrap();
        assert_eq!(text.matches("Reply in Italian").count(), 1);
        assert!(super::instructions(&dir)
            .unwrap()
            .contains("- Reply in Italian _(1970-01-01)_"));
    }
}
