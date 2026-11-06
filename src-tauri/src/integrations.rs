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
pub fn instructions(data: &Path) -> Result<String, String> {
    let state = status(data)?;
    let mut result=String::from("Shared skills available in Lumen. When a skill applies, read its SKILL.md before acting and resolve resources relative to its folder:\n");
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
pub fn args(
    data: &Path,
    provider: &crate::models::Provider,
) -> Result<(Vec<String>, Vec<String>), String> {
    let servers = enabled(data)?;
    if servers.is_empty() {
        return Ok((vec![], vec![]));
    }
    let names: Vec<String> = servers.keys().cloned().collect();
    if *provider == crate::models::Provider::Claude {
        let path = root(data).join("mcp.active.json");
        std::fs::write(
            &path,
            serde_json::to_vec_pretty(&json!({ "mcpServers": servers }))
                .map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        return Ok((
            vec!["--mcp-config".into(), path.to_string_lossy().into()],
            names,
        ));
    }
    let mut args = vec![];
    for (name, server) in &servers {
        let mut converted = server.clone();
        if let Some(o) = converted.as_object_mut() {
            o.remove("type");
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
    Ok((args, names))
}

#[cfg(test)]
mod tests {
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
