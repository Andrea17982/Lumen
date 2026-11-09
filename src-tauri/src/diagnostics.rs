//! Bounded LSP diagnostics for changed documents. No edits, installations or
//! model calls. Missing servers and missing responses are explicit, never passes.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    net::SocketAddr,
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader},
    sync::watch,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    pub file: String,
    pub line: u64,
    pub column: u64,
    pub severity: u64,
    pub message: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Report {
    pub label: String,
    pub status: String,
    pub detail: String,
    pub diagnostics: Vec<Diagnostic>,
}

impl Report {
    pub fn unavailable(label: String, detail: String) -> Self {
        Self {
            label,
            status: "unavailable".into(),
            detail,
            diagnostics: vec![],
        }
    }
    pub fn errors(&self) -> String {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == 1)
            .take(60)
            .map(|d| format!("{}:{}:{}: {}", d.file, d.line, d.column, d.message))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Server {
    name: String,
    command: String,
    args: Vec<String>,
    address: Option<SocketAddr>,
    extensions: Vec<String>,
    root: Option<String>,
    initialization_options: Value,
    timeout_seconds: u64,
}

impl Default for Server {
    fn default() -> Self {
        Self {
            name: String::new(),
            command: String::new(),
            args: vec![],
            address: None,
            extensions: vec![],
            root: None,
            initialization_options: Value::Null,
            timeout_seconds: 15,
        }
    }
}

fn servers(root: &Path) -> Result<Vec<Server>, String> {
    let path = root.join(".lumen/diagnostics.json");
    if path.exists() {
        #[derive(Deserialize)]
        struct Config {
            servers: Vec<Server>,
        }
        let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        if text.len() > 64 * 1024 {
            return Err("Diagnostics configuration exceeds 64 KB".into());
        }
        let config = serde_json::from_str::<Config>(&text)
            .map_err(|e| format!("Invalid diagnostics configuration: {e}"))?;
        if config.servers.len() > 8 {
            return Err("Configure at most 8 language servers".into());
        }
        return Ok(config.servers);
    }
    Ok(vec![
        Server {
            name: "Rust".into(),
            command: "rust-analyzer".into(),
            extensions: vec!["rs".into()],
            initialization_options: json!({"checkOnSave":false}),
            ..Default::default()
        },
        Server {
            name: "TypeScript/JavaScript".into(),
            command: "typescript-language-server".into(),
            args: vec!["--stdio".into()],
            extensions: ["ts", "tsx", "js", "jsx", "mts", "cts", "mjs", "cjs"]
                .map(str::to_string)
                .into(),
            ..Default::default()
        },
        Server {
            name: "C#".into(),
            command: "csharp-ls".into(),
            extensions: vec!["cs".into()],
            ..Default::default()
        },
        Server {
            name: "GDScript".into(),
            extensions: vec!["gd".into()],
            ..Default::default()
        },
    ])
}

fn language(path: &Path) -> &'static str {
    match path.extension().and_then(|s| s.to_str()).unwrap_or("") {
        "rs" => "rust",
        "ts" | "mts" | "cts" => "typescript",
        "tsx" => "typescriptreact",
        "js" | "mjs" | "cjs" => "javascript",
        "jsx" => "javascriptreact",
        "cs" => "csharp",
        "gd" => "gdscript",
        _ => "plaintext",
    }
}

fn project_root(root: &Path, file: &Path) -> PathBuf {
    for dir in file.ancestors().skip(1).take_while(|p| p.starts_with(root)) {
        if [
            "Cargo.toml",
            "tsconfig.json",
            "package.json",
            "project.godot",
        ]
        .iter()
        .any(|n| dir.join(n).is_file())
            || std::fs::read_dir(dir).ok().is_some_and(|entries| {
                entries.flatten().any(|e| {
                    e.path()
                        .extension()
                        .is_some_and(|x| x == "csproj" || x == "sln")
                })
            })
        {
            return dir.into();
        }
    }
    root.into()
}

type Input = BufReader<Box<dyn AsyncRead + Send + Unpin>>;
type Output = Box<dyn AsyncWrite + Send + Unpin>;

async fn write(output: &mut Output, value: Value) -> Result<(), String> {
    let body = serde_json::to_vec(&value).map_err(|e| e.to_string())?;
    output
        .write_all(format!("Content-Length: {}\r\n\r\n", body.len()).as_bytes())
        .await
        .map_err(|e| e.to_string())?;
    output.write_all(&body).await.map_err(|e| e.to_string())?;
    output.flush().await.map_err(|e| e.to_string())
}

async fn read(input: &mut Input) -> Result<Value, String> {
    let mut length = None;
    let mut header_bytes = 0;
    loop {
        let mut line = String::new();
        // Bound each header before allocating an unbounded read_line buffer.
        let n = input
            .take(8193)
            .read_line(&mut line)
            .await
            .map_err(|e| e.to_string())?;
        if n == 0 {
            return Err("Language server closed the connection".into());
        }
        header_bytes += n;
        if header_bytes > 8192 {
            return Err("LSP headers exceed 8 KB".into());
        }
        if line == "\r\n" || line == "\n" {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            if name.eq_ignore_ascii_case("Content-Length") {
                length = Some(
                    value
                        .trim()
                        .parse::<usize>()
                        .map_err(|_| "Invalid LSP Content-Length")?,
                );
            }
        }
    }
    let length = length
        .filter(|n| *n <= 2 * 1024 * 1024)
        .ok_or("Missing or oversized LSP Content-Length")?;
    let mut body = vec![0; length];
    input
        .read_exact(&mut body)
        .await
        .map_err(|e| e.to_string())?;
    serde_json::from_slice(&body).map_err(|e| e.to_string())
}

fn diagnostic_items(file: &str, items: &Value) -> Vec<Diagnostic> {
    items
        .as_array()
        .into_iter()
        .flatten()
        .take(100)
        .filter_map(|d| {
            Some(Diagnostic {
                file: file.into(),
                line: d["range"]["start"]["line"].as_u64()?.saturating_add(1),
                column: d["range"]["start"]["character"].as_u64()?.saturating_add(1),
                severity: d["severity"].as_u64().unwrap_or(1),
                message: d["message"].as_str()?.chars().take(2000).collect(),
            })
        })
        .collect()
}

async fn callback(output: &mut Output, message: &Value, options: &Value) -> Result<(), String> {
    let Some(id) = message.get("id") else {
        return Ok(());
    };
    let method = message["method"].as_str().unwrap_or("");
    let result = match method {
        "workspace/configuration" => Value::Array(message["params"]["items"].as_array().into_iter().flatten().map(|item| {
            item["section"].as_str().map(|section| section.split('.').fold(options, |v, part| &v[part]).clone()).unwrap_or_else(|| options.clone())
        }).collect()),
        "workspace/applyEdit" => json!({"applied":false,"failureReason":"Lumen's diagnostics client is read-only"}),
        "workspace/workspaceFolders" => Value::Null,
        "window/workDoneProgress/create" | "workspace/diagnostic/refresh" => Value::Null,
        _ => return write(output, json!({"jsonrpc":"2.0","id":id,"error":{"code":-32601,"message":"Unsupported diagnostic-client request"}})).await,
    };
    write(output, json!({"jsonrpc":"2.0","id":id,"result":result})).await
}

async fn collect(
    root: &Path,
    project: &Path,
    server: &Server,
    files: &[String],
    mut input: Input,
    mut output: Output,
) -> Result<Report, String> {
    let deadline =
        tokio::time::Instant::now() + Duration::from_secs(server.timeout_seconds.clamp(1, 30));
    let uri = tauri::Url::from_directory_path(project)
        .map_err(|_| "Invalid project URI")?
        .to_string();
    write(&mut output, json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "processId":std::process::id(), "rootUri":uri,"workspaceFolders":[{"uri":uri,"name":"Lumen project"}],
        "capabilities":{"window":{"workDoneProgress":true},"workspace":{"configuration":true,"applyEdit":false},"textDocument":{"publishDiagnostics":{"versionSupport":true},"diagnostic":{"dynamicRegistration":false}}},
        "initializationOptions":server.initialization_options
    }})).await?;
    let initialized = loop {
        let message = tokio::time::timeout_at(deadline, read(&mut input))
            .await
            .map_err(|_| "Diagnostics timed out during initialization")??;
        if message["id"] == 1 && message.get("method").is_none() {
            if message.get("error").is_some() {
                return Err("Language server rejected initialization".into());
            }
            break message;
        }
        if message.get("method").is_some() {
            callback(&mut output, &message, &server.initialization_options).await?;
        }
    };
    write(
        &mut output,
        json!({"jsonrpc":"2.0","method":"initialized","params":{}}),
    )
    .await?;
    let pull = initialized["result"]["capabilities"]
        .get("diagnosticProvider")
        .is_some_and(|v| v != &Value::Bool(false) && !v.is_null());
    let mut documents = BTreeMap::new();
    let mut requests = BTreeMap::new();
    for file in files.iter().take(24) {
        let path = crate::context::jail(root, file)?;
        if !path.is_file()
            || std::fs::metadata(&path).map_err(|e| e.to_string())?.len() > 512 * 1024
        {
            continue;
        }
        let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let uri = tauri::Url::from_file_path(&path)
            .map_err(|_| "Invalid document URI")?
            .to_string();
        documents.insert(uri.clone(), file.clone());
        write(&mut output, json!({"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":uri,"languageId":language(&path),"version":1,"text":text}}})).await?;
        if pull {
            let id = requests.len() as u64 + 10;
            requests.insert(id, file.clone());
            write(&mut output, json!({"jsonrpc":"2.0","id":id,"method":"textDocument/diagnostic","params":{"textDocument":{"uri":uri}}})).await?;
        }
    }
    if documents.is_empty() {
        return Err("No readable changed documents for this language server".into());
    }
    let mut observed: BTreeMap<String, Vec<Diagnostic>> = BTreeMap::new();
    let mut progress = BTreeSet::new();
    let mut incomplete = false;
    loop {
        let done = observed.len() == documents.len() && requests.is_empty() && progress.is_empty();
        let message = if done {
            match tokio::time::timeout(Duration::from_millis(500), read(&mut input)).await {
                Ok(Ok(message)) => message,
                Ok(Err(_)) => {
                    incomplete = true;
                    break;
                }
                Err(_) => break,
            }
        } else {
            match tokio::time::timeout_at(deadline, read(&mut input)).await {
                Ok(Ok(message)) => message,
                _ => {
                    incomplete = true;
                    break;
                }
            }
        };
        if message["method"] == "textDocument/publishDiagnostics" {
            let params = &message["params"];
            if params["version"].as_u64().is_some_and(|v| v != 1) {
                continue;
            }
            if let Some(file) = params["uri"].as_str().and_then(|u| documents.get(u)) {
                observed.insert(file.clone(), diagnostic_items(file, &params["diagnostics"]));
            }
        } else if message["method"] == "$/progress" {
            let token = message["params"]["token"].to_string();
            match message["params"]["value"]["kind"].as_str() {
                Some("begin") => {
                    progress.insert(token);
                }
                Some("end") => {
                    progress.remove(&token);
                }
                _ => {}
            }
        } else if let Some(id) = message["id"]
            .as_u64()
            .filter(|_| message.get("method").is_none())
        {
            if let Some(file) = requests.remove(&id) {
                if message.get("error").is_some() {
                    incomplete = true;
                    break;
                }
                if message["result"]["kind"] != "full" {
                    incomplete = true;
                    break;
                }
                observed.insert(
                    file.clone(),
                    diagnostic_items(&file, &message["result"]["items"]),
                );
            }
        } else if message.get("method").is_some()
            && callback(&mut output, &message, &server.initialization_options)
                .await
                .is_err()
        {
            incomplete = true;
            break;
        }
    }
    for uri in documents.keys() {
        let _ = write(&mut output, json!({"jsonrpc":"2.0","method":"textDocument/didClose","params":{"textDocument":{"uri":uri}}})).await;
    }
    if server.address.is_none() {
        let _ = write(
            &mut output,
            json!({"jsonrpc":"2.0","id":9999,"method":"shutdown","params":null}),
        )
        .await;
        // Give the server its shutdown response before exit; a process timeout is still enforced outside.
        let _ = tokio::time::timeout(Duration::from_millis(300), async {
            loop {
                let value = read(&mut input).await?;
                if value["id"] == 9999 {
                    break Ok::<(), String>(());
                }
            }
        })
        .await;
        let _ = write(&mut output, json!({"jsonrpc":"2.0","method":"exit"})).await;
    }
    let observed_count = observed.len();
    let diagnostics: Vec<Diagnostic> = observed.into_values().flatten().collect();
    let status = if diagnostics.iter().any(|d| d.severity == 1) {
        "failed"
    } else if incomplete || observed_count < files.len() {
        "incomplete"
    } else {
        "passed"
    };
    Ok(Report {
        label: format!(
            "LSP {} ({})",
            server.name,
            project.strip_prefix(root).unwrap_or(project).display()
        ),
        status: status.into(),
        detail: format!(
            "Fresh diagnostics received for {} of {} selected documents; this is not a full-project build",
            observed_count, files.len()
        ),
        diagnostics,
    })
}

async fn run_server(
    root: &Path,
    project: &Path,
    server: &Server,
    files: &[String],
    mut cancel: watch::Receiver<bool>,
) -> Result<Report, String> {
    if *cancel.borrow() {
        return Err("Cancelled".into());
    }
    let mut child = None;
    let connection: Result<(Input, Output), String> = if let Some(address) = server.address {
        if !address.ip().is_loopback() {
            return Err("Diagnostics TCP servers must use a loopback address".into());
        }
        match tokio::time::timeout(
            Duration::from_secs(2),
            tokio::net::TcpStream::connect(address),
        )
        .await
        {
            Ok(Ok(stream)) => {
                let (read, write) = stream.into_split();
                Ok((BufReader::new(Box::new(read)), Box::new(write)))
            }
            _ => Err("Configured diagnostics server is not reachable".into()),
        }
    } else {
        let exe = which::which(&server.command)
            .map_err(|_| "Language server is not installed; no installation was attempted")?;
        let mut command = crate::proc::command(exe);
        command
            .args(&server.args)
            .current_dir(project)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true);
        #[cfg(unix)]
        command.process_group(0);
        let mut process = command.spawn().map_err(|e| e.to_string())?;
        let input = process.stdout.take().ok_or("No LSP stdout")?;
        let output = process.stdin.take().ok_or("No LSP stdin")?;
        child = Some(process);
        Ok((BufReader::new(Box::new(input)), Box::new(output)))
    };
    let (input, output) = connection?;
    let result = tokio::select! {
        biased;
        _ = cancel.changed() => Err("Cancelled".into()),
        result = tokio::time::timeout(Duration::from_secs(server.timeout_seconds.clamp(1, 30)+2), collect(root, project, server, files, input, output)) => result.unwrap_or_else(|_| Err("Diagnostics timed out before every document returned fresh results".into())),
    };
    if let Some(mut process) = child {
        crate::providers::terminate(&mut process).await;
    }
    result
}

pub async fn run(
    root: &Path,
    files: &[String],
    cancel: watch::Receiver<bool>,
) -> Result<Vec<Report>, String> {
    if *cancel.borrow() {
        return Err("Cancelled".into());
    }
    let root = dunce::canonicalize(root).map_err(|e| e.to_string())?;
    let servers = match servers(&root) {
        Ok(s) => s,
        Err(e) => return Ok(vec![Report::unavailable("LSP configuration".into(), e)]),
    };
    let mut reports = vec![];
    if files.len() > 80 {
        reports.push(Report::unavailable(
            "LSP coverage".into(),
            "Only the first 80 changed or affected paths were considered for diagnostics".into(),
        ));
    }
    for server in servers {
        let mut projects: BTreeMap<PathBuf, Vec<String>> = BTreeMap::new();
        for file in files.iter().take(80) {
            let Ok(path) = crate::context::jail(&root, file) else {
                continue;
            };
            if !server
                .extensions
                .iter()
                .any(|e| path.extension().is_some_and(|x| x == e.as_str()))
            {
                continue;
            }
            let project = if let Some(configured) = &server.root {
                crate::context::jail(&root, configured)?
            } else {
                project_root(&root, &path)
            };
            if !path.starts_with(&project) {
                continue;
            }
            projects.entry(project).or_default().push(file.clone());
        }
        if projects.len() > 4 {
            reports.push(Report::unavailable(
                format!("LSP {} coverage", server.name),
                "Only the first 4 affected projects were considered".into(),
            ));
        }
        for (project, files) in projects.into_iter().take(4) {
            let label = format!(
                "LSP {} ({})",
                server.name,
                project.strip_prefix(&root).unwrap_or(&project).display()
            );
            if server.command.is_empty() && server.address.is_none() {
                reports.push(Report::unavailable(
                    label,
                    "Configure this language server in .lumen/diagnostics.json".into(),
                ));
                continue;
            }
            match run_server(&root, &project, &server, &files, cancel.clone()).await {
                Ok(report) => reports.push(report),
                Err(e) if e == "Cancelled" => return Err(e),
                Err(e) => reports.push(Report::unavailable(label, e)),
            }
        }
    }
    Ok(reports)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Fixture;

    fn wire(stream: tokio::io::DuplexStream) -> (Input, Output) {
        let (r, w) = tokio::io::split(stream);
        (BufReader::new(Box::new(r)), Box::new(w))
    }

    #[tokio::test]
    async fn partial_timeout_preserves_known_errors_instead_of_losing_the_evidence() {
        let fixture = Fixture::new();
        for name in ["one.ts", "two.ts"] {
            std::fs::write(fixture.root.join(name), "export const value = 1;\n").unwrap();
        }
        let (client, server) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            let (mut input, mut output) = wire(server);
            read(&mut input).await.unwrap();
            write(
                &mut output,
                json!({"jsonrpc":"2.0","id":1,"result":{"capabilities":{}}}),
            )
            .await
            .unwrap();
            read(&mut input).await.unwrap();
            let opened = read(&mut input).await.unwrap();
            write(&mut output,json!({"jsonrpc":"2.0","method":"textDocument/publishDiagnostics","params":{"uri":opened["params"]["textDocument"]["uri"],"version":1,"diagnostics":[{"range":{"start":{"line":0,"character":0}},"severity":1,"message":"Concrete type failure"}]}})).await.unwrap();
            tokio::time::sleep(Duration::from_secs(10)).await;
        });
        let (input, output) = wire(client);
        let server = Server {
            name: "partial".into(),
            address: Some("127.0.0.1:1".parse().unwrap()),
            timeout_seconds: 1,
            ..Default::default()
        };
        let report = collect(
            &fixture.root,
            &fixture.root,
            &server,
            &["one.ts".into(), "two.ts".into()],
            input,
            output,
        )
        .await
        .unwrap();
        peer.abort();
        assert_eq!(report.status, "failed");
        assert!(report.errors().contains("Concrete type failure"));
        assert!(report.detail.contains("1 of 2"));
    }

    async fn fake(stream: tokio::io::DuplexStream, pull: bool) {
        let (mut input, mut output) = wire(stream);
        let init = read(&mut input).await.unwrap();
        assert_eq!(init["method"], "initialize");
        assert_eq!(
            init["params"]["capabilities"]["workspace"]["applyEdit"],
            false
        );
        write(&mut output, json!({"jsonrpc":"2.0","id":1,"result":{"capabilities":if pull {json!({"diagnosticProvider":{"interFileDependencies":true,"workspaceDiagnostics":false}})} else {json!({})}}})).await.unwrap();
        assert_eq!(read(&mut input).await.unwrap()["method"], "initialized");
        loop {
            let message = read(&mut input).await.unwrap();
            match message["method"].as_str() {
                Some("textDocument/didOpen") if !pull => {
                    let uri = &message["params"]["textDocument"]["uri"];
                    // An unrelated file and a stale document version must not become evidence.
                    for (target, version) in [
                        (json!("file:///unrelated/secret.rs"), 1),
                        (uri.clone(), 0),
                        (uri.clone(), 1),
                    ] {
                        write(&mut output,json!({"jsonrpc":"2.0","method":"textDocument/publishDiagnostics","params":{"uri":target,"version":version,"diagnostics":[{"range":{"start":{"line":3,"character":6},"end":{"line":3,"character":7}},"severity":1,"message":"Type mismatch à 🐭"}]}})).await.unwrap();
                    }
                    write(&mut output,json!({"jsonrpc":"2.0","id":42,"method":"workspace/applyEdit","params":{"edit":{}}})).await.unwrap();
                }
                Some("textDocument/diagnostic") => {
                    write(&mut output,json!({"jsonrpc":"2.0","id":message["id"],"result":{"kind":"full","items":[]}})).await.unwrap();
                }
                Some("shutdown") => {
                    write(
                        &mut output,
                        json!({"jsonrpc":"2.0","id":9999,"result":null}),
                    )
                    .await
                    .unwrap();
                }
                Some("exit") => break,
                _ if message["id"] == 42 => assert_eq!(message["result"]["applied"], false),
                _ => {}
            }
        }
    }

    #[tokio::test]
    async fn push_diagnostics_are_fresh_scoped_and_never_apply_server_edits() {
        let fixture = Fixture::new();
        std::fs::write(fixture.root.join("code.rs"), "pub fn example() {}\n").unwrap();
        let (client, server) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(fake(server, false));
        let (input, output) = wire(client);
        let report = collect(
            &fixture.root,
            &fixture.root,
            &Server {
                name: "test".into(),
                ..Default::default()
            },
            &["code.rs".into()],
            input,
            output,
        )
        .await
        .unwrap();
        peer.await.unwrap();
        assert_eq!(report.status, "failed");
        assert_eq!(report.diagnostics.len(), 1);
        assert_eq!(report.diagnostics[0].line, 4);
        assert_eq!(report.diagnostics[0].column, 7);
        assert_eq!(report.diagnostics[0].file, "code.rs");
        assert!(report.errors().contains("Type mismatch à 🐭"));
    }

    #[tokio::test]
    async fn pull_diagnostics_require_an_actual_full_response() {
        let fixture = Fixture::new();
        std::fs::write(fixture.root.join("code.ts"), "export const answer = 42;\n").unwrap();
        let (client, server) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(fake(server, true));
        let (input, output) = wire(client);
        let report = collect(
            &fixture.root,
            &fixture.root,
            &Server {
                name: "test".into(),
                ..Default::default()
            },
            &["code.ts".into()],
            input,
            output,
        )
        .await
        .unwrap();
        peer.await.unwrap();
        assert_eq!(report.status, "passed");
        assert!(report.diagnostics.is_empty());
    }

    #[tokio::test]
    async fn missing_servers_and_tcp_timeout_are_not_successes() {
        let fixture = Fixture::new();
        std::fs::write(fixture.root.join("code.gd"), "extends Node\n").unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let peer = tokio::spawn(async move {
            let (_stream, _) = listener.accept().await.unwrap();
            tokio::time::sleep(Duration::from_secs(10)).await;
        });
        std::fs::create_dir_all(fixture.root.join(".lumen")).unwrap();
        let config = fixture.root.join(".lumen/diagnostics.json");
        std::fs::write(&config,json!({"servers":[{"name":"GDScript","extensions":["gd"],"address":address,"timeoutSeconds":1}]}).to_string()).unwrap();
        let (_tx, rx) = watch::channel(false);
        let reports = run(&fixture.root, &["code.gd".into()], rx.clone())
            .await
            .unwrap();
        assert_eq!(reports[0].status, "unavailable");
        assert!(reports[0].detail.contains("timed out"));
        peer.abort();
        std::fs::write(&config,json!({"servers":[{"name":"Missing","extensions":["gd"],"command":"lumen-nonexistent-language-server-934761"}]}).to_string()).unwrap();
        let reports = run(&fixture.root, &["code.gd".into()], rx).await.unwrap();
        assert_eq!(reports[0].status, "unavailable");
        assert!(reports[0].detail.contains("not installed"));
    }

    #[tokio::test]
    async fn malformed_or_unbounded_frames_are_rejected() {
        let (client, mut server) = tokio::io::duplex(64 * 1024);
        let peer = tokio::spawn(async move {
            server
                .write_all(b"Content-Length: 99999999\r\n\r\n")
                .await
                .unwrap();
        });
        let (mut input, _) = wire(client);
        assert!(read(&mut input).await.unwrap_err().contains("oversized"));
        peer.await.unwrap();
    }
}
