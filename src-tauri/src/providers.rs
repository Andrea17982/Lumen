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
    let mut c = Command::new(exe);
    #[cfg(windows)]
    c.creation_flags(0x08000000);
    #[cfg(unix)]
    c.process_group(0);
    Ok(c)
}
pub fn isolate(c: &mut Command, p: &Provider, dir: &Path) {
    c.env(p.env(), dir);
    // Prevent inherited API credentials from silently selecting a different identity/billing path.
    for key in [
        "ANTHROPIC_API_KEY",
        "ANTHROPIC_AUTH_TOKEN",
        "CLAUDE_CODE_OAUTH_TOKEN",
        "OPENAI_API_KEY",
        "CODEX_API_KEY",
        "CODEX_ACCESS_TOKEN",
    ] {
        c.env_remove(key);
    }
}
pub async fn terminate(child: &mut tokio::process::Child) {
    if let Some(pid) = child.id() {
        #[cfg(windows)]
        {
            let mut c = Command::new("taskkill.exe");
            c.args(["/PID", &pid.to_string(), "/T", "/F"])
                .creation_flags(0x08000000);
            let _ = c.output().await;
        }
        #[cfg(unix)]
        unsafe {
            libc::kill(-(pid as i32), libc::SIGKILL);
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
    } else {
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
            if v["is_error"] == true {
                push(
                    "error",
                    v["result"].as_str().unwrap_or("Claude turn failed").into(),
                );
            } else if let Some(t) = v["result"].as_str() {
                push("final", t.into());
            }
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
    pub write: bool,
}
pub async fn execute(
    input: Invocation<'_>,
    cancel: watch::Receiver<bool>,
    emit: impl FnMut(Activity),
) -> Result<String, String> {
    let p = &input.account.provider;
    let mut c = command(p)?;
    isolate(&mut c, p, input.config);
    c.current_dir(input.workspace)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    match p {
        Provider::Gpt => {
            c.args([
                "exec",
                "--json",
                "--color",
                "never",
                "--skip-git-repo-check",
                "--sandbox",
                if input.write {
                    "workspace-write"
                } else {
                    "read-only"
                },
                "-c",
                "approval_policy=\"never\"",
                "-c",
                &format!("model_reasoning_effort=\"{}\"", input.effort),
            ]);
            if let Some(m) = input.model {
                c.args(["--model", m]);
            }
            c.arg("-");
        }
        Provider::Claude => {
            c.args([
                "--print",
                "--verbose",
                "--output-format",
                "stream-json",
                "--include-partial-messages",
                "--effort",
                input.effort,
                "--permission-mode",
                "dontAsk",
                "--strict-mcp-config",
                "--restricted",
            ]);
            let tools = if input.write {
                "Read,Glob,Grep,Edit,Write"
            } else {
                "Read,Glob,Grep"
            };
            c.args(["--tools", tools, "--allowedTools", tools]);
            if let Some(m) = input.model {
                c.args(["--model", m]);
            }
        }
    }
    run_command(c, input.prompt, p, cancel, emit).await
}

async fn run_command(
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
    let mut stdin = child.stdin.take().ok_or("CLI stdin unavailable")?;
    let prompt = prompt.to_owned();
    let writer = tokio::spawn(async move {
        let r = stdin.write_all(prompt.as_bytes()).await;
        drop(stdin);
        r
    });
    let stderr = child.stderr.take().ok_or("CLI stderr unavailable")?;
    // Drain diagnostics to prevent pipe deadlocks, but don't send raw auth logs to the UI.
    let diagnostics = tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(_)) = lines.next_line().await {}
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
                    if let Ok(v)=serde_json::from_str::<Value>(&line){for a in normalize(p,&v){
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
    let _ = diagnostics.await;
    if let Some(e) = error {
        return Err(e);
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
        let mut c = Command::new("powershell.exe");
        c.args(["-NoProfile", "-NonInteractive", "-Command", script])
            .creation_flags(0x08000000);
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
        assert_eq!(events.len(), 1);
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
