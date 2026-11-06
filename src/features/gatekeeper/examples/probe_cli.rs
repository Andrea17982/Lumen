//! Read-only protocol smoke: no model turn, shell tool or inference request.
//! Args: absolute codex exe, absolute claude exe, absolute MCP helper, output dir.
use lumen_gatekeeper::{mcp, support, Availability, Gatekeeper, Provider};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncReadExt, BufReader};
use tokio::process::{Child, Command};

#[tokio::main]
async fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if args.len() != 4 || args.iter().any(|p| !p.is_absolute()) {
        return Err(
            "Expected absolute paths: codex.exe claude.exe helper.exe output-directory".into(),
        );
    }
    let codex = &args[0];
    let claude = &args[1];
    let bridge = &args[2];
    let output = &args[3];
    std::fs::create_dir_all(output).map_err(|e| e.to_string())?;
    let codex_version = version(codex).await?;
    let claude_version = version(claude).await?;
    let schema_dir = output.join("schema");
    let generated = Command::new(codex)
        .args(["app-server", "generate-json-schema", "--out"])
        .arg(&schema_dir)
        .output()
        .await
        .map_err(|e| e.to_string())?;
    if !generated.status.success() {
        return Err("Codex schema generation failed".into());
    }
    let mut codex_child = spawn(codex, &support::codex_args(), output)?;
    let mut codex_input = codex_child.stdin.take().ok_or("Codex stdin missing")?;
    let mut codex_output = BufReader::new(codex_child.stdout.take().ok_or("Codex stdout missing")?);
    let errors = drain(&mut codex_child);
    mcp::write_frame(&mut codex_input,&json!({"id":1,"method":"initialize","params":{"clientInfo":{"name":"lumen-gatekeeper-probe","version":"0.1.0"}}})).await?;
    let init = tokio::time::timeout(Duration::from_secs(20), async {
        for _ in 0..100 {
            let value = mcp::read_frame(&mut codex_output)
                .await?
                .ok_or("Codex closed before initialization")?;
            if value["id"] == 1 {
                return Ok::<_, String>(value);
            }
        }
        Err("Codex initialization produced too many messages".into())
    })
    .await
    .map_err(|_| "Codex initialize timeout")??;
    if init.get("error").is_none() {
        mcp::write_frame(&mut codex_input, &json!({"method":"initialized"})).await?;
    }
    let codex_support = support::codex(
        &codex_version,
        &read_json(&schema_dir.join("ServerRequest.json"))?,
        &read_json(&schema_dir.join("CommandExecutionRequestApprovalResponse.json"))?,
        &read_json(&schema_dir.join("FileChangeRequestApprovalResponse.json"))?,
        &init,
    );
    let _ = codex_child.kill().await;
    let _ = codex_child.wait().await;
    errors.abort();

    let keeper = Gatekeeper::in_memory(Duration::from_secs(10))?;
    let provider_session = uuid::Uuid::new_v4().to_string();
    // This temporary diagnostic session never receives a user/model turn.
    let scope = keeper.start_session(
        "probe",
        Provider::Claude,
        &provider_session,
        "probe",
        output,
        Availability::Available {
            version: claude_version.clone(),
            evidence: "Diagnostic endpoint; no turns allowed".into(),
        },
    )?;
    let endpoint = mcp::ClaudeEndpoint::bind(keeper.clone(), scope).await?;
    let mut ready = endpoint.readiness();
    let config = endpoint.config(bridge, false)?;
    let endpoint_task = tokio::spawn(endpoint.serve());
    let mut claude_args = support::claude_args(&config, &provider_session);
    claude_args.extend(["--tools".to_string(), "".to_string()]);
    let mut claude_child = spawn(claude, &claude_args, output)?;
    let mut claude_input = claude_child.stdin.take().ok_or("Claude stdin missing")?;
    let mut claude_output =
        BufReader::new(claude_child.stdout.take().ok_or("Claude stdout missing")?);
    let errors = drain(&mut claude_child);
    mcp::write_frame(&mut claude_input,&json!({"type":"control_request","request_id":"lumen-gatekeeper-probe","request":{"subtype":"initialize"}})).await?;
    let initialized = tokio::time::timeout(Duration::from_secs(20), async {
        for _ in 0..100 {
            let value = mcp::read_frame(&mut claude_output)
                .await?
                .ok_or("Claude closed before initialization")?;
            if value["type"] == "control_response"
                && value["response"]["request_id"] == "lumen-gatekeeper-probe"
            {
                return Ok::<_, String>(value["response"]["subtype"] == "success");
            }
        }
        Err("Claude initialization produced too many messages".into())
    })
    .await
    .map_err(|_| "Claude initialize timeout")??;
    let discovered = if *ready.borrow() {
        true
    } else {
        tokio::time::timeout(Duration::from_secs(15), ready.wait_for(|value| *value))
            .await
            .is_ok_and(|r| r.is_ok())
    };
    let claude_support = support::claude(&claude_version, initialized, discovered);
    let _ = claude_child.kill().await;
    let _ = claude_child.wait().await;
    errors.abort();
    endpoint_task.abort();
    let _ = endpoint_task.await;
    let report = json!({"codex":codex_support,"claude":claude_support,"modelTurns":0,"permissionCallbacksFromModels":0,"note":"Real CLI initialization, generated schemas and MCP discovery only; adapter decisions are exercised by fixtures and real local MCP transport tests"});
    let report_text = serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?;
    std::fs::write(output.join("probe-report.json"), &report_text).map_err(|e| e.to_string())?;
    println!("{report_text}");
    Ok(())
}

fn read_json(path: &Path) -> Result<Value, String> {
    serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}

async fn version(path: &Path) -> Result<String, String> {
    let output = tokio::time::timeout(
        Duration::from_secs(10),
        Command::new(path).arg("--version").output(),
    )
    .await
    .map_err(|_| "Version command timed out")?
    .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err("CLI version command failed".into());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn spawn(path: &Path, args: &[String], cwd: &Path) -> Result<Child, String> {
    let mut command = Command::new(path);
    command
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(0x08000000);
    command.spawn().map_err(|e| e.to_string())
}

fn drain(child: &mut Child) -> tokio::task::JoinHandle<()> {
    let stderr = child.stderr.take();
    tokio::spawn(async move {
        if let Some(mut stderr) = stderr {
            let mut buffer = [0_u8; 4096];
            while matches!(stderr.read(&mut buffer).await,Ok(n) if n>0) {}
        }
    })
}
