use crate::models::*;
use serde_json::Value;
use std::path::{Path, PathBuf};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::Command,
    sync::watch,
};

pub fn executable(p: &Provider) -> Result<PathBuf, String> {
    which::which(p.binary()).map_err(|_| {
        format!(
            "{} CLI was not found. Install the official CLI and restart Lumen.",
            p.binary()
        )
    })
}
pub fn command(p: &Provider) -> Result<Command, String> {
    let exe = executable(p)?;
    // Rust's Windows process implementation handles .cmd/.bat quoting itself.
    // Manually prefixing cmd /c breaks launchers under paths containing spaces.
    #[allow(unused_mut)]
    let mut c = crate::proc::command(exe);
    #[cfg(unix)]
    c.process_group(0);
    Ok(c)
}
pub fn isolate(c: &mut Command, p: &Provider, dir: &Path) {
    if *p == Provider::Antigravity {
        c.env("HOME", dir).env("USERPROFILE", dir);
    } else if *p == Provider::Ollama {
        // Local models live in the user's own Ollama store; nothing to isolate.
    } else {
        c.env(p.env(), dir);
    }
    // Prevent inherited API credentials from silently selecting a different identity/billing path.
    for key in [
        "ANTHROPIC_API_KEY",
        "ANTHROPIC_AUTH_TOKEN",
        "CLAUDE_CODE_OAUTH_TOKEN",
        "OPENAI_API_KEY",
        "CODEX_API_KEY",
        "CODEX_ACCESS_TOKEN",
        "GOOGLE_API_KEY",
        "GEMINI_API_KEY",
    ] {
        c.env_remove(key);
    }
}
pub async fn terminate(child: &mut tokio::process::Child) {
    if let Some(pid) = child.id() {
        #[cfg(windows)]
        crate::proc::taskkill(pid).await;
        #[cfg(unix)]
        {
            // Graceful first, so CLIs can flush their session files.
            unsafe {
                libc::kill(-(pid as i32), libc::SIGTERM);
            }
            if tokio::time::timeout(std::time::Duration::from_millis(1500), child.wait())
                .await
                .is_err()
            {
                unsafe {
                    libc::kill(-(pid as i32), libc::SIGKILL);
                }
            }
        }
    }
    let _ = child.kill().await;
    let _ = child.wait().await;
}

/// Normalizes only provider-exposed summaries; never fabricates hidden reasoning.
pub fn normalize(p: &Provider, v: &Value) -> Vec<Activity> {
    let mut out = vec![];
    let mut push = |kind: &str, text: String| {
        if !text.is_empty() {
            out.push(Activity {
                kind: kind.into(),
                text,
            })
        }
    };
    if *p == Provider::Gpt {
        if v["type"] == "thread.started" {
            push("session", v["thread_id"].as_str().unwrap_or("").into());
        }
        if let Some(item) = v.get("item") {
            let typ = item["type"].as_str().unwrap_or("");
            let phase = v["type"].as_str().unwrap_or("");
            if phase == "item.completed" {
                match typ {
                    "agent_message" => push("text", item["text"].as_str().unwrap_or("").into()),
                    "reasoning" => push("thinking", item["text"].as_str().unwrap_or("").into()),
                    "command_execution" => push(
                        "tool",
                        format!(
                            "{}\n{}",
                            item["command"].as_str().unwrap_or("Command"),
                            item["aggregated_output"].as_str().unwrap_or("")
                        ),
                    ),
                    "file_change" => push("file", item["changes"].to_string()),
                    "mcp_tool_call" => push("tool", item["tool"].as_str().unwrap_or("Tool").into()),
                    _ => {}
                }
            }
        }
        if v["type"] == "error" || v["type"] == "turn.failed" {
            push(
                "error",
                v["message"]
                    .as_str()
                    .or_else(|| v["error"]["message"].as_str())
                    .unwrap_or("Codex turn failed")
                    .into(),
            );
        }
    } else if *p == Provider::Claude {
        if v["type"] == "system" && v["subtype"] == "init" {
            push("session", v["session_id"].as_str().unwrap_or("").into());
        }
        if v["type"] == "stream_event" {
            let delta = &v["event"]["delta"];
            if let Some(t) = delta["text"].as_str() {
                push("delta", t.into());
            }
            if let Some(t) = delta["thinking"].as_str() {
                push("thinking", t.into());
            }
        }
        if v["type"] == "assistant" {
            // Input tokens of this API request, counting cached reads and cache writes.
            let usage = &v["message"]["usage"];
            if let Some(input) = usage["input_tokens"].as_u64() {
                let total = input
                    + usage["cache_read_input_tokens"].as_u64().unwrap_or(0)
                    + usage["cache_creation_input_tokens"].as_u64().unwrap_or(0);
                push("usage", total.to_string());
            }
            if let Some(blocks) = v["message"]["content"].as_array() {
                for b in blocks {
                    if b["type"] == "tool_use" {
                        let name = b["name"].as_str().unwrap_or("Tool");
                        push(
                            if name == "Edit" || name == "Write" {
                                "file"
                            } else {
                                "tool"
                            },
                            format!("{}\n{}", name, b["input"]),
                        );
                    }
                }
            }
        }
        if v["type"] == "result" {
            // The CLI reports each model's real context window; the largest one bounds the chat.
            if let Some(models) = v["modelUsage"].as_object() {
                if let Some(window) = models
                    .values()
                    .filter_map(|m| m["contextWindow"].as_u64())
                    .max()
                {
                    push("window", window.to_string());
                }
            }
            if v["is_error"] == true {
                push(
                    "error",
                    v["result"].as_str().unwrap_or("Claude turn failed").into(),
                );
            } else if let Some(t) = v["result"].as_str() {
                push("final", t.into());
            }
        }
    } else {
        // Google CLIs. Antigravity print mode: step_update text deltas, then one result event.
        if v["event"] == "step_update" && v["step_update"]["step_type"] == "agent_response" {
            if let Some(t) = v["step_update"]["text_delta"].as_str() {
                push("delta", t.into());
            }
        }
        if v["event"] == "result" && v["result"]["status"] == "SUCCESS" {
            if let Some(t) = v["result"]["response"].as_str() {
                push("final", t.into());
            }
        }
        if v["event"] == "result" && v["result"]["status"] == "ERROR" {
            push(
                "error",
                v["result"]["error"]
                    .as_str()
                    .unwrap_or("Antigravity turn failed")
                    .into(),
            );
        }
        if v["type"] == "message" && v["role"] == "assistant" {
            if let Some(t) = v["content"].as_str() {
                push("delta", t.into());
            }
        }
        if v["type"] == "result" {
            if let Some(t) = v["response"].as_str() {
                push("final", t.into());
            }
        }
        if v["type"] == "event" && v["event"] == "result" {
            if let Some(t) = v["result"]["response"].as_str() {
                push("final", t.into());
            }
        }
        if v["type"] == "error" || v["event"] == "error" {
            push(
                "error",
                v["message"]
                    .as_str()
                    .or_else(|| v["error"].as_str())
                    .unwrap_or("Google CLI turn failed")
                    .into(),
            );
        }
    }
    out
}

pub struct Invocation<'a> {
    pub account: &'a Account,
    pub config: &'a Path,
    pub workspace: &'a Path,
    pub prompt: &'a str,
    pub model: Option<&'a str>,
    pub effort: &'a str,
    pub access: AccessMode,
    /// Required for `AccessMode::Ask`; other modes never consult it.
    pub approvals: Option<crate::approvals::Approvals>,
    /// Native CLI session to continue (Claude `--resume`, Codex `exec resume`). The prompt then
    /// carries only the new request: the CLI already holds the earlier turns, tool calls included.
    pub resume: Option<&'a str>,
}
pub async fn execute(
    input: Invocation<'_>,
    cancel: watch::Receiver<bool>,
    emit: impl FnMut(Activity),
) -> Result<String, String> {
    crate::models::share_global_instructions(&input.account.provider, input.config);
    if matches!(input.account.provider, Provider::Claude | Provider::Gpt) {
        // Lumen's shared skills become native skills, loaded only when a task needs them.
        if let Some(data) = input.config.parent().and_then(Path::parent) {
            for dir in crate::integrations::enabled_skills(data) {
                if let Some(name) = dir.file_name() {
                    crate::models::mirror_dir(&dir, &input.config.join("skills").join(name), 0);
                }
            }
        }
    }
    if input.access == AccessMode::Ask {
        if !matches!(input.account.provider, Provider::Claude | Provider::Gpt) {
            return Err(
                "Approve each action is available for Claude and Codex. Choose another permission mode."
                    .into(),
            );
        }
        let Some(approvals) = input.approvals.clone() else {
            return Err(
                "unavailable: The approval journal could not be opened, so Ask first is off."
                    .into(),
            );
        };
        if input.account.provider == Provider::Gpt {
            return crate::approvals_codex::codex_turn(input, approvals, cancel, emit).await;
        }
        let exe = std::env::current_exe().map_err(|e| format!("unavailable: {e}"))?;
        return crate::approvals::claude_turn(input, approvals, &exe, cancel, emit).await;
    }
    let p = &input.account.provider;
    let mut c = command(p)?;
    isolate(&mut c, p, input.config);
    // Windows batch launchers reject verbatim UNC-style drive prefixes as their cwd.
    let workspace = dunce::canonicalize(input.workspace)
        .map_err(|_| "The conversation folder is no longer available")?;
    c.current_dir(&workspace)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    let mut args = invocation_args(p, input.access, input.model, input.effort);
    if input.access != AccessMode::ReadOnly && matches!(p, Provider::Claude | Provider::Gpt) {
        let data = input
            .config
            .parent()
            .and_then(|p| p.parent())
            .ok_or("Invalid account directory")?;
        let (shared, names) = crate::integrations::args(data, p)?;
        if *p == Provider::Gpt {
            args.pop();
            args.extend(shared);
            args.push("-".into());
        } else {
            args.extend(shared);
            if input.access == AccessMode::FileEdits && !names.is_empty() {
                // MCP tools are opt-in per server; built-in tools stay restricted.
                if let Some(i) = args.iter().position(|a| a == "--allowedTools") {
                    let extra: Vec<String> = names.iter().map(|n| format!("mcp__{n}")).collect();
                    args[i + 1] = format!("{},{}", args[i + 1], extra.join(","));
                }
            }
        }
    }
    if let Some(id) = input.resume.filter(|id| !id.is_empty()) {
        match p {
            Provider::Claude => args.extend(["--resume".into(), id.into()]),
            Provider::Gpt => {
                // Exec flags stay before the subcommand; `-` still reads the prompt from stdin.
                if args.last().map(String::as_str) == Some("-") {
                    args.pop();
                }
                args.extend(["resume".into(), id.into(), "-".into()]);
            }
            _ => {}
        }
    }
    if matches!(p, Provider::Claude | Provider::Gpt) {
        if let Some(data) = input.config.parent().and_then(Path::parent) {
            if graph_enabled(data) {
                if let Some(extra) = crate::graph_mcp::cli_args(data, &workspace, p) {
                    add_graph(&mut args, extra, p);
                }
            }
        }
    }
    c.args(args);
    let prompt = if *p == Provider::Antigravity {
        format!(
            "{}\n",
            serde_json::json!({"event": "user", "message": {"content": input.prompt}})
        )
    } else {
        input.prompt.to_owned()
    };
    run_command(c, &prompt, p, cancel, emit).await
}

/// Commands that only inspect the repository: offered in every restricted Claude mode.
pub(crate) const READ_COMMANDS: [&str; 8] = [
    "Bash(git status:*)",
    "Bash(git diff:*)",
    "Bash(git log:*)",
    "Bash(git show:*)",
    "Bash(git blame:*)",
    "Bash(git grep:*)",
    "Bash(git branch:*)",
    "Bash(git ls-files:*)",
];

/// Build, test and lint commands, so File edits can verify a change the way Codex's
/// workspace-write sandbox already can. They run project code, which File edits already trusts.
pub(crate) const CHECK_COMMANDS: [&str; 18] = [
    "Bash(cargo check:*)",
    "Bash(cargo build:*)",
    "Bash(cargo test:*)",
    "Bash(cargo clippy:*)",
    "Bash(cargo fmt:*)",
    "Bash(npm test:*)",
    "Bash(npm run test:*)",
    "Bash(npm run build:*)",
    "Bash(npm run lint:*)",
    "Bash(npm run check:*)",
    "Bash(npm run typecheck:*)",
    "Bash(npx tsc:*)",
    "Bash(pnpm test:*)",
    "Bash(yarn test:*)",
    "Bash(pytest:*)",
    "Bash(python -m pytest:*)",
    "Bash(go test:*)",
    "Bash(dotnet test:*)",
];

/// The code graph setting ("repoContext"), on unless the user turned it off.
pub(crate) fn graph_enabled(data: &Path) -> bool {
    std::fs::read(data.join("settings.json"))
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .and_then(|v| v["repoContext"].as_bool())
        .unwrap_or(true)
}

/// Adds the code-graph MCP server and, in restricted Claude modes, allows its tools.
fn add_graph(args: &mut Vec<String>, extra: Vec<String>, p: &Provider) {
    let tools = format!("mcp__{}", crate::graph_mcp::SERVER);
    if let Some(i) = args.iter().position(|a| a == "--allowedTools") {
        args[i + 1] = format!("{},{tools}", args[i + 1]);
    }
    if *p == Provider::Gpt {
        // Config overrides go before the trailing `-` / `resume <id> -`.
        let at = args
            .iter()
            .position(|a| a == "resume")
            .unwrap_or_else(|| args.len().saturating_sub(1));
        for (k, e) in extra.into_iter().enumerate() {
            args.insert(at + k, e);
        }
    } else {
        args.extend(extra);
    }
}

pub(crate) const DENY: [&str; 9] = [
    "Read(**/.env)",
    "Read(**/.env.*)",
    "Read(**/package-lock.json)",
    "Read(**/Cargo.lock)",
    "Read(**/yarn.lock)",
    "Read(**/pnpm-lock.yaml)",
    "Read(~/.ssh/**)",
    "Read(~/.aws/**)",
    "Edit(**/.env)",
];

/// Permission profiles are mapped to explicit official CLI flags, never inferred from prompt text.
pub fn invocation_args(
    p: &Provider,
    access: AccessMode,
    model: Option<&str>,
    effort: &str,
) -> Vec<String> {
    let mut args: Vec<String> = match p {
        Provider::Gpt => vec![
            "exec",
            "--json",
            "--color",
            "never",
            "--skip-git-repo-check",
        ],
        Provider::Claude => vec![
            "--print",
            "--verbose",
            "--output-format",
            "stream-json",
            "--include-partial-messages",
        ],
        Provider::Gemini => vec!["--prompt", "", "--output-format", "stream-json"],
        // agy ignores stdin in plain print mode; stream-json input reads the prompt
        // as one NDJSON message, so long prompts never hit the command-line limit.
        Provider::Antigravity => vec![
            "--print=",
            "--input-format",
            "stream-json",
            "--output-format",
            "stream-json",
        ],
        // Prompt-only: Ollama reads stdin and prints plain text, never touching files.
        Provider::Ollama => vec!["run"],
    }
    .into_iter()
    .map(str::to_owned)
    .collect();
    match p {
        Provider::Gpt => {
            if access == AccessMode::Autopilot {
                args.push("--dangerously-bypass-approvals-and-sandbox".into());
            } else {
                args.extend(
                    [
                        "--sandbox",
                        if access == AccessMode::FileEdits {
                            "workspace-write"
                        } else {
                            "read-only"
                        },
                        "-c",
                        r#"approval_policy="never""#,
                    ]
                    .map(str::to_owned),
                );
                // Isolated CODEX_HOMEs have no [windows] sandbox setting, and without
                // one Codex blocks every write. The unelevated sandbox needs no admin prompt.
                if cfg!(windows) {
                    args.extend(["-c".into(), r#"windows.sandbox="unelevated""#.into()]);
                }
            }
            if !effort.is_empty() {
                args.extend(["-c".into(), format!(r#"model_reasoning_effort="{effort}""#)]);
            }
        }
        Provider::Claude => {
            if access == AccessMode::Autopilot {
                args.extend(
                    ["--dangerously-skip-permissions", "--tools", "default"].map(str::to_owned),
                );
            } else {
                // Planning, subagents and the read-only git/verification commands let the model
                // check its own work. Bash is offered, but dontAsk only lets the listed command
                // prefixes run; anything else is refused without a prompt.
                let (tools, allowed) = if access == AccessMode::FileEdits {
                    (
                        "Read,Glob,Grep,Edit,Write,NotebookEdit,TodoWrite,Task,Bash",
                        format!(
                            "Read,Glob,Grep,Edit,Write,NotebookEdit,TodoWrite,Task,{},{}",
                            READ_COMMANDS.join(","),
                            CHECK_COMMANDS.join(",")
                        ),
                    )
                } else {
                    (
                        "Read,Glob,Grep,TodoWrite,Task,Bash",
                        format!("Read,Glob,Grep,TodoWrite,Task,{}", READ_COMMANDS.join(",")),
                    )
                };
                args.extend(
                    [
                        "--permission-mode",
                        "dontAsk",
                        "--strict-mcp-config",
                        "--restricted",
                        "--tools",
                        tools,
                        "--allowedTools",
                        allowed.as_str(),
                    ]
                    .map(str::to_owned),
                );
            }
            // Secrets and lockfiles are denied even in Autopilot: they burn context or leak keys.
            args.extend(["--disallowedTools".into(), DENY.join(",")]);
            if !effort.is_empty() {
                args.extend(["--effort".into(), effort.into()]);
            }
        }
        Provider::Gemini | Provider::Antigravity => {
            if *p == Provider::Gemini {
                let permission = match access {
                    AccessMode::ReadOnly | AccessMode::Ask => "--approval-mode=plan",
                    AccessMode::FileEdits => "--approval-mode=auto_edit",
                    AccessMode::Autopilot => "--approval-mode=yolo",
                };
                args.push(permission.into());
            } else {
                let permission = match access {
                    AccessMode::ReadOnly | AccessMode::Ask => "--mode=plan",
                    AccessMode::FileEdits => "--mode=accept-edits",
                    AccessMode::Autopilot => "--dangerously-skip-permissions",
                };
                args.push(permission.into());
                // Headless agy denies every shell command, even `dir`, and its allow
                // rules only match command(*). In File edits it therefore gets all
                // commands; the interface warns about this when File edits is chosen.
                if access == AccessMode::FileEdits {
                    args.extend([
                        "--settings".into(),
                        r#"{"permissions":{"allow":["command(*)"]}}"#.into(),
                    ]);
                }
            }
            if *p == Provider::Antigravity && !effort.is_empty() {
                args.extend(["--effort".into(), effort.into()]);
            }
        }
        Provider::Ollama => {
            args.push(model.unwrap_or_default().into());
            args.extend(["--hidethinking".into(), "--nowordwrap".into()]);
            return args;
        }
    }
    if let Some(m) = model {
        args.extend(["--model".into(), m.into()]);
    }
    if *p == Provider::Gemini && args.len() > 1 {
        args[1] = "".into();
    }
    if *p == Provider::Gpt {
        args.push("-".into());
    }
    args
}

pub(crate) async fn run_command(
    mut c: Command,
    prompt: &str,
    p: &Provider,
    mut cancel: watch::Receiver<bool>,
    mut emit: impl FnMut(Activity),
) -> Result<String, String> {
    c.stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    if *cancel.borrow() {
        return Err("Cancelled".into());
    }
    let mut child = c
        .spawn()
        .map_err(|e| format!("Cannot start {}: {e}", p.binary()))?;
    // Callers that must outlive the app (Orchestra recovery) record the process.
    if let Some(pid) = child.id() {
        emit(Activity {
            kind: "pid".into(),
            text: pid.to_string(),
        });
    }
    let mut stdin = child.stdin.take().ok_or("CLI stdin unavailable")?;
    let prompt = prompt.to_owned();
    let writer = tokio::spawn(async move {
        let r = stdin.write_all(prompt.as_bytes()).await;
        drop(stdin);
        r
    });
    let stderr = child.stderr.take().ok_or("CLI stderr unavailable")?;
    // Drain diagnostics to prevent pipe deadlocks, but don't send raw auth logs to the UI.
    // Only a short tail is kept, in memory, to classify rate-limit failures.
    let diagnostics = tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        let mut tail: std::collections::VecDeque<String> = Default::default();
        while let Ok(Some(line)) = lines.next_line().await {
            tail.push_back(line.chars().take(400).collect());
            if tail.len() > 40 {
                tail.pop_front();
            }
        }
        tail.into_iter().collect::<Vec<_>>().join("\n")
    });
    let stdout = child.stdout.take().ok_or("CLI stdout unavailable")?;
    let mut lines = BufReader::new(stdout).lines();
    let mut text = String::new();
    let mut error = None;
    let read_result:Result<(),String>=async{
        loop{
            tokio::select!{
                biased;
                _=cancel.changed()=>{return Err("Cancelled".into());},
                line=lines.next_line()=>{
                    let line=line.map_err(|e|e.to_string())?;
                    let Some(line)=line else{break};
                    if *p==Provider::Ollama{let chunk=format!("{line}\n");text.push_str(&chunk);emit(Activity{kind:"delta".into(),text:chunk});continue;}
                    if let Ok(v)=serde_json::from_str::<Value>(&line){
                        if *p == Provider::Claude && v["type"] == "rate_limit_event" {
                            emit(Activity { kind: "quota".into(), text: v.to_string() });
                        }
                        for a in normalize(p,&v){
                        match a.kind.as_str(){
                            "delta"=>{text.push_str(&a.text);emit(a);},
                            "text"=>{if !text.is_empty(){text.push_str("\n\n");}text.push_str(&a.text);emit(Activity{kind:"replace".into(),text:text.clone()});},
                            "final"=>{if text.is_empty(){text=a.text;emit(Activity{kind:"replace".into(),text:text.clone()});}},
                            "error"=>{error=Some(a.text);},_=>emit(a)
                        }
                    }}
                }
            }
        }Ok(())
    }.await;
    if let Err(e) = read_result {
        terminate(&mut child).await;
        writer.abort();
        diagnostics.abort();
        return Err(e);
    }
    let status = tokio::select! { biased; _=cancel.changed()=>{terminate(&mut child).await;writer.abort();diagnostics.abort();return Err("Cancelled".into());}, r=child.wait()=>r.map_err(|e|e.to_string())? };
    let _ = writer.await;
    let stderr_tail = diagnostics.await.unwrap_or_default();
    if let Some(e) = error {
        if crate::limits::is_rate_limit(&e) {
            return Err(format!("{}{e}", crate::limits::RATE_LIMIT));
        }
        return Err(e);
    }
    if !status.success() && crate::limits::is_rate_limit(&stderr_tail) {
        let line = stderr_tail
            .lines()
            .rev()
            .find(|l| crate::limits::is_rate_limit(l))
            .unwrap_or("Rate limit reached");
        return Err(format!(
            "{}{} {line}",
            crate::limits::RATE_LIMIT,
            p.binary()
        ));
    }
    if !status.success() {
        return Err(format!("{} exited with {}. Check official CLI login, model access, quota and workspace permissions. No automatic account rotation is performed.",p.binary(),status));
    }
    if text.trim().is_empty() {
        return Err(
            "The CLI returned no assistant text. Verify CLI compatibility and login in Settings."
                .into(),
        );
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn graph_server_args_keep_codex_stdin_last() {
        let mut args: Vec<String> = ["exec", "--json", "resume", "t1", "-"]
            .map(String::from)
            .into();
        add_graph(
            &mut args,
            vec!["-c".into(), "mcp_servers.x.command=1".into()],
            &Provider::Gpt,
        );
        assert_eq!(
            args,
            [
                "exec",
                "--json",
                "-c",
                "mcp_servers.x.command=1",
                "resume",
                "t1",
                "-"
            ]
        );
        let mut claude: Vec<String> = ["--allowedTools", "Read"].map(String::from).into();
        add_graph(
            &mut claude,
            vec!["--mcp-config".into(), "g.json".into()],
            &Provider::Claude,
        );
        assert_eq!(claude[1], "Read,mcp__lumen_graph");
        assert_eq!(claude[2..], ["--mcp-config", "g.json"]);
    }
    #[test]
    fn native_session_ids_are_reported() {
        let c = normalize(
            &Provider::Claude,
            &json!({"type":"system","subtype":"init","session_id":"abc"}),
        );
        assert!(c.iter().any(|a| a.kind == "session" && a.text == "abc"));
        let g = normalize(
            &Provider::Gpt,
            &json!({"type":"thread.started","thread_id":"t1"}),
        );
        assert!(g.iter().any(|a| a.kind == "session" && a.text == "t1"));
    }
    #[test]
    fn restricted_claude_can_verify_but_not_run_anything() {
        let args = invocation_args(&Provider::Claude, AccessMode::FileEdits, None, "");
        let i = args.iter().position(|a| a == "--allowedTools").unwrap();
        assert!(args[i + 1].contains("Bash(cargo test:*)"));
        assert!(!args[i + 1].split(',').any(|t| t == "Bash"));
        let ro = invocation_args(&Provider::Claude, AccessMode::ReadOnly, None, "");
        let i = ro.iter().position(|a| a == "--allowedTools").unwrap();
        assert!(ro[i + 1].contains("Bash(git diff:*)"));
        assert!(!ro[i + 1].contains("Edit") && !ro[i + 1].contains("cargo"));
    }
    #[test]
    fn claude_reports_input_tokens_including_cache() {
        let v = json!({"type":"assistant","message":{"content":[],"usage":{
            "input_tokens":10,"cache_read_input_tokens":200,"cache_creation_input_tokens":30,"output_tokens":5}}});
        let acts = normalize(&Provider::Claude, &v);
        assert!(acts.iter().any(|a| a.kind == "usage" && a.text == "240"));
    }
    #[test]
    fn access_profiles_use_official_flags() {
        let auto = invocation_args(&Provider::Gpt, AccessMode::Autopilot, None, "");
        assert!(auto
            .iter()
            .any(|a| a == "--dangerously-bypass-approvals-and-sandbox"));
        assert!(!auto.iter().any(|a| a == "--sandbox"));
        assert!(!auto.iter().any(|a| a.contains("model_reasoning_effort")));
        let claude = invocation_args(
            &Provider::Claude,
            AccessMode::Autopilot,
            Some("opus"),
            "max",
        );
        assert!(claude.iter().any(|a| a == "--dangerously-skip-permissions"));
        assert!(!claude
            .iter()
            .any(|a| a == "--restricted" || a == "--strict-mcp-config"));
        assert!(claude.iter().any(|a| a == "max"));
        for p in [Provider::Claude, Provider::Gpt] {
            for access in [AccessMode::ReadOnly, AccessMode::FileEdits] {
                let args = invocation_args(&p, access, None, "");
                assert!(!args.iter().any(|a| a.starts_with("--dangerously")));
            }
        }
    }
    #[test]
    fn antigravity_reads_the_prompt_as_stream_json() {
        let args = invocation_args(
            &Provider::Antigravity,
            AccessMode::FileEdits,
            Some("gemini-3.8-flash-low"),
            "",
        );
        assert_eq!(
            args,
            [
                "--print=",
                "--input-format",
                "stream-json",
                "--output-format",
                "stream-json",
                "--mode=accept-edits",
                "--settings",
                r#"{"permissions":{"allow":["command(*)"]}}"#,
                "--model",
                "gemini-3.8-flash-low"
            ]
        );
        let delta = json!({"event":"step_update","step_update":{"step_type":"agent_response","text_delta":"hi"}});
        assert_eq!(normalize(&Provider::Antigravity, &delta)[0].text, "hi");
        let err = json!({"event":"result","result":{"status":"ERROR","error":"empty prompt"}});
        let a = normalize(&Provider::Antigravity, &err);
        assert_eq!(
            (a[0].kind.as_str(), a[0].text.as_str()),
            ("error", "empty prompt")
        );
    }
    #[test]
    fn ollama_is_prompt_only_with_its_model() {
        let args = invocation_args(
            &Provider::Ollama,
            AccessMode::Autopilot,
            Some("qwen:7b"),
            "high",
        );
        assert_eq!(args, ["run", "qwen:7b", "--hidethinking", "--nowordwrap"]);
    }
    #[test]
    fn protocol_fixtures() {
        let a = normalize(
            &Provider::Gpt,
            &json!({"type":"item.completed","item":{"type":"agent_message","text":"hello"}}),
        );
        assert_eq!(a[0].text, "hello");
        let b = normalize(
            &Provider::Claude,
            &json!({"type":"stream_event","event":{"delta":{"text":"hi"}}}),
        );
        assert_eq!(b[0].kind, "delta");
    }
    #[cfg(windows)]
    fn fixture(script: &str) -> Command {
        let mut c = crate::proc::command("powershell.exe");
        c.args(["-NoProfile", "-NonInteractive", "-Command", script]);
        c
    }
    #[cfg(windows)]
    #[tokio::test]
    async fn prompt_uses_stdin_and_stderr_is_drained() {
        let script="$p=[Console]::In.ReadToEnd(); for($i=0;$i -lt 3000;$i++){[Console]::Error.WriteLine('diagnostic data that must not leak')}; @{type='item.completed';item=@{type='agent_message';text=$p}} | ConvertTo-Json -Compress";
        let (tx, rx) = watch::channel(false);
        let mut events = vec![];
        let prompt = "literal $(not-a-command) & ; ' \" Unicode: à 🚀";
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(15),
            run_command(fixture(script), prompt, &Provider::Gpt, rx, |a| {
                events.push(a)
            }),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(result, prompt);
        // The process ID first, then the answer.
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].kind, "pid");
        drop(tx);
    }
    #[cfg(windows)]
    #[tokio::test]
    async fn cancellation_terminates_real_child() {
        let (tx, rx) = watch::channel(false);
        let signal = tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(350)).await;
            let _ = tx.send(true);
        });
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            run_command(
                fixture("Start-Sleep -Seconds 60"),
                "",
                &Provider::Gpt,
                rx,
                |_| {},
            ),
        )
        .await
        .unwrap();
        assert_eq!(result.unwrap_err(), "Cancelled");
        signal.await.unwrap();
    }
    #[cfg(windows)]
    #[tokio::test]
    async fn nonzero_exit_is_an_error() {
        let (_tx, rx) = watch::channel(false);
        let result = run_command(fixture("exit 3"), "", &Provider::Gpt, rx, |_| {}).await;
        assert!(result.unwrap_err().contains("exited"));
    }
}
