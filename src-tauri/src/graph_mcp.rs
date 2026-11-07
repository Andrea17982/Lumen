//! The code graph as an MCP server. Claude Code and Codex start Lumen itself with
//! `--lumen-graph-mcp <data> <root>`; it answers JSON-RPC over stdio and never writes to the
//! project. Every tool is read-only.
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

pub const SERVER: &str = "lumen_graph";
pub const FLAG: &str = "--lumen-graph-mcp";

fn tools() -> Value {
    let read_only = json!({"readOnlyHint": true, "idempotentHint": true, "openWorldHint": false});
    let tool = |name: &str, description: &str, properties: Value, required: Value| {
        json!({
            "name": name,
            "description": description,
            "inputSchema": {"type": "object", "properties": properties, "required": required},
            "annotations": read_only.clone(),
        })
    };
    json!([
        tool(
            "overview",
            "Map of the repository: languages, main areas and the most depended-on files with their key symbols. Call first in an unfamiliar codebase instead of listing or reading many files.",
            json!({"limit": {"type": "integer", "description": "Central files to list (default 25)."}}),
            json!([]),
        ),
        tool(
            "find_symbol",
            "Find where functions, types, methods, constants, tables or scenes are defined, by full or partial name. Returns each definition's file and exact line range plus its signature. Use instead of grepping for a definition.",
            json!({
                "query": {"type": "string", "description": "Name or part of a name."},
                "kind": {"type": "string", "description": "Optional kind filter, e.g. fn, class, method, struct, interface, table."},
                "limit": {"type": "integer", "description": "Maximum results (default 20)."}
            }),
            json!(["query"]),
        ),
        tool(
            "symbol",
            "Everything about one definition: location and line range, enclosing type, the definitions its body uses, and every place that uses it grouped by the calling function. Set body=true to include its source. Use before changing or deleting a symbol.",
            json!({
                "name": {"type": "string", "description": "Symbol name; Type::method and Type.method also work."},
                "file": {"type": "string", "description": "Optional file, when the name is defined in several places."},
                "body": {"type": "boolean", "description": "Include the definition's source (up to 150 lines)."}
            }),
            json!(["name"]),
        ),
        tool(
            "file_outline",
            "Structure of one file without reading it: nested symbols with line ranges and signatures, its imports, the files importing it and the files whose symbols it uses. Then Read only the line ranges you need.",
            json!({"path": {"type": "string", "description": "File path, relative to the project or absolute."}}),
            json!(["path"]),
        ),
        tool(
            "related_files",
            "Files most related to one file: imports in both directions, shared symbols and files that git history shows changing together. Use to find what else to look at or update.",
            json!({
                "path": {"type": "string", "description": "File path."},
                "limit": {"type": "integer", "description": "Maximum results (default 15)."}
            }),
            json!(["path"]),
        ),
        tool(
            "impact",
            "What a change can break: files that depend on the given files or symbols, directly and transitively, ordered by distance, plus the tests among them.",
            json!({
                "targets": {"type": "array", "items": {"type": "string"}, "description": "Files and/or symbol names that change."},
                "depth": {"type": "integer", "description": "How many dependency steps to follow (default 2, max 4)."}
            }),
            json!(["targets"]),
        ),
    ])
}

fn int(v: &Value, key: &str, default: usize, max: usize) -> usize {
    v[key]
        .as_u64()
        .map_or(default, |n| (n as usize).clamp(1, max))
}

/// Runs one tool. Unknown tools and bad arguments come back as tool errors, never panics.
pub fn call(data: &Path, root: &Path, name: &str, args: &Value) -> Result<String, String> {
    let text = |key: &str| -> Result<String, String> {
        args[key]
            .as_str()
            .filter(|s| !s.trim().is_empty())
            .map(str::to_string)
            .ok_or_else(|| format!("`{key}` is required"))
    };
    let g = crate::codegraph::graph(data, root);
    Ok(match name {
        "overview" => g.overview(int(args, "limit", 25, 80)),
        "find_symbol" => g.find_symbol(
            &text("query")?,
            args["kind"].as_str(),
            int(args, "limit", 20, 100),
        ),
        "symbol" => g.symbol(
            &text("name")?,
            args["file"].as_str(),
            args["body"].as_bool().unwrap_or(false),
        ),
        "file_outline" => g.file_outline(&text("path")?),
        "related_files" => g.related(&text("path")?, int(args, "limit", 15, 60)),
        "impact" => {
            let targets: Vec<String> = args["targets"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .take(20)
                .collect();
            if targets.is_empty() {
                return Err("`targets` needs at least one file or symbol".into());
            }
            g.impact(&targets, int(args, "depth", 2, 4))
        }
        other => return Err(format!("Unknown tool `{other}`")),
    })
}

/// Answer to one JSON-RPC message; `None` for notifications.
pub fn handle(data: &Path, root: &Path, msg: &Value) -> Option<Value> {
    let id = msg.get("id")?.clone();
    let method = msg["method"].as_str().unwrap_or("");
    let result = match method {
        "initialize" => json!({
            "protocolVersion": msg["params"]["protocolVersion"].as_str().unwrap_or("2025-06-18"),
            "capabilities": {"tools": {}},
            "serverInfo": {"name": SERVER, "version": env!("CARGO_PKG_VERSION")},
            "instructions": "Lumen's code graph of this project. Prefer these tools over broad Grep or whole-file Reads to find definitions, callers, file structure and the impact of a change; then Read only the line ranges they return.",
        }),
        "ping" => json!({}),
        "tools/list" => json!({"tools": tools()}),
        "tools/call" => {
            let name = msg["params"]["name"].as_str().unwrap_or("");
            let args = &msg["params"]["arguments"];
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                call(data, root, name, args)
            }))
            .unwrap_or_else(|_| Err("The code graph failed on this request".into()));
            match r {
                Ok(text) => json!({"content": [{"type": "text", "text": text}]}),
                Err(e) => json!({"content": [{"type": "text", "text": e}], "isError": true}),
            }
        }
        _ => {
            return Some(json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": {"code": -32601, "message": format!("Method not found: {method}")}
            }))
        }
    };
    Some(json!({"jsonrpc": "2.0", "id": id, "result": result}))
}

/// Serves stdio until the client closes it.
pub fn serve(data: PathBuf, root: PathBuf) -> std::io::Result<()> {
    let root = dunce::canonicalize(&root).unwrap_or(root);
    let stdin = std::io::stdin();
    let mut out = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(&line) {
            Ok(msg) => handle(&data, &root, &msg),
            Err(_) => Some(
                json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": "Parse error"}}),
            ),
        };
        if let Some(reply) = reply {
            writeln!(out, "{reply}")?;
            out.flush()?;
        }
    }
    Ok(())
}

/// Arguments that give Claude (`--mcp-config <file>`) or Codex (`-c mcp_servers…`) this server.
pub fn cli_args(
    data: &Path,
    root: &Path,
    provider: &crate::models::Provider,
) -> Option<Vec<String>> {
    let exe = std::env::current_exe().ok()?;
    let (exe, data, root) = (
        exe.to_string_lossy().into_owned(),
        data.to_string_lossy().into_owned(),
        root.to_string_lossy().into_owned(),
    );
    match provider {
        crate::models::Provider::Claude => {
            let config = json!({"mcpServers": {SERVER: {"type": "stdio", "command": exe, "args": [FLAG, data, root]}}});
            use std::hash::{DefaultHasher, Hash, Hasher};
            let mut h = DefaultHasher::new();
            root.to_lowercase().hash(&mut h);
            let dir = Path::new(&data).join("repomap");
            std::fs::create_dir_all(&dir).ok()?;
            let path = dir.join(format!("{:x}.mcp.json", h.finish()));
            std::fs::write(&path, serde_json::to_vec_pretty(&config).ok()?).ok()?;
            Some(vec![
                "--mcp-config".into(),
                path.to_string_lossy().into_owned(),
            ])
        }
        crate::models::Provider::Gpt => {
            // JSON strings are valid TOML basic strings, Windows backslashes included.
            let q = |s: &str| serde_json::to_string(s).unwrap_or_default();
            Some(vec![
                "-c".into(),
                format!("mcp_servers.{SERVER}.command={}", q(&exe)),
                "-c".into(),
                format!(
                    "mcp_servers.{SERVER}.args=[{},{},{}]",
                    q(FLAG),
                    q(&data),
                    q(&root)
                ),
            ])
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speaks_mcp() {
        let root = std::env::temp_dir().join(format!("lumen-mcp-{}", std::process::id()));
        let src = root.join("p");
        std::fs::create_dir_all(src.join("src")).unwrap();
        std::fs::write(
            src.join("src/lib.rs"),
            "pub fn answer() -> u8 {\n    42\n}\n",
        )
        .unwrap();
        let data = root.join("d");
        let init = handle(&data, &src, &json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26"}})).unwrap();
        assert_eq!(init["result"]["protocolVersion"], "2025-03-26");
        assert!(handle(
            &data,
            &src,
            &json!({"jsonrpc":"2.0","method":"notifications/initialized"})
        )
        .is_none());
        let list = handle(
            &data,
            &src,
            &json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}),
        )
        .unwrap();
        assert_eq!(list["result"]["tools"].as_array().unwrap().len(), 6);
        let hit = handle(&data, &src, &json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"find_symbol","arguments":{"query":"answer"}}})).unwrap();
        let text = hit["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.starts_with("answer (fn) src/lib.rs:1-3"), "{text}");
        let bad = handle(&data, &src, &json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"symbol","arguments":{}}})).unwrap();
        assert_eq!(bad["result"]["isError"], true);
        let codex = cli_args(&data, &src, &crate::models::Provider::Gpt).unwrap();
        assert!(codex[3].starts_with("mcp_servers.lumen_graph.args=[\"--lumen-graph-mcp\""));
        let _ = std::fs::remove_dir_all(&root);
    }
}
