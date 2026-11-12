//! "Ask first" Claude turns. Claude runs with `--permission-prompt-tool`, so each action that
//! needs permission waits for a decision in the Gatekeeper panel. Codex: `approvals_codex`.
use crate::models::Activity;
use crate::providers::Invocation;
#[cfg(test)]
use crate::{models::Provider, providers};
use lumen_gatekeeper::Gatekeeper;
#[cfg(test)]
use lumen_gatekeeper::{mcp::ClaudeEndpoint, support, Availability, Provider as Engine};
use std::path::Path;
#[cfg(test)]
use std::time::Duration;
use tokio::sync::watch;

/// The app chat that owns a turn, and the journal that records its approvals.
/// Owned so turn futures stay `Send`; cloning the journal handle is cheap.
#[derive(Clone)]
pub struct Approvals {
    pub keeper: Gatekeeper,
    pub chat: String,
}

#[cfg(test)]
const PROBE_TIMEOUT: Duration = Duration::from_secs(30);
#[cfg(test)]
const VERSION_TIMEOUT: Duration = Duration::from_secs(20);

/// Runs one Claude turn in ask-first mode. Each turn gets a fresh provider session, approval
/// connection and MCP endpoint, so an approval can never outlive the turn that asked for it.
/// `exe` is the executable Claude starts as its MCP bridge: Lumen itself in the app.
pub async fn claude_turn(
    input: Invocation<'_>,
    approvals: Approvals,
    exe: &Path,
    cancel: watch::Receiver<bool>,
    emit: impl FnMut(Activity),
) -> Result<String, String> {
    let _ = (approvals, exe);
    crate::native_claude::execute(input, cancel, emit).await
}

/// Starts Claude with the approval bridge and waits until its MCP client has discovered the
/// permission tool. No prompt is sent, so no model turn runs. Failure means no turn starts.
#[cfg(test)]
async fn probe(
    approvals: &Approvals,
    exe: &Path,
    cwd: &Path,
    config_dir: &Path,
    version: &str,
) -> Result<(), String> {
    let session = uuid::Uuid::new_v4().to_string();
    let turn = uuid::Uuid::new_v4().to_string();
    let scope = approvals.keeper.start_session(
        &approvals.chat,
        Engine::Claude,
        &session,
        &turn,
        cwd,
        Availability::Available {
            version: version.into(),
            evidence: "Diagnostic bootstrap; no model turn".into(),
        },
    )?;
    let endpoint = ClaudeEndpoint::bind(approvals.keeper.clone(), scope.clone()).await?;
    let config = endpoint.config(exe, true)?;
    let mut ready = endpoint.readiness();
    let serve = tokio::spawn(endpoint.serve());

    let mut c = providers::command(&Provider::Claude)?;
    providers::isolate(&mut c, &Provider::Claude, config_dir);
    c.current_dir(cwd)
        .args(support::claude_args(&config, &session))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);
    let mut child = c
        .spawn()
        .map_err(|e| format!("unavailable: cannot start Claude: {e}"))?;
    // Reduced to a bool in one statement: the watch guard is not Send and must not span an await.
    let connected = matches!(
        tokio::time::timeout(PROBE_TIMEOUT, ready.wait_for(|r| *r)).await,
        Ok(Ok(_))
    );
    providers::terminate(&mut child).await;
    serve.abort();
    let _ = approvals.keeper.disconnect(&scope, "Diagnostic finished");
    if connected {
        Ok(())
    } else {
        Err("unavailable: Claude did not connect the approval bridge, so no turn was started. Use a turn without approvals or update Claude Code.".into())
    }
}

#[cfg(test)]
async fn cli_version(config_dir: &Path) -> Result<String, String> {
    let mut c = providers::command(&Provider::Claude)?;
    providers::isolate(&mut c, &Provider::Claude, config_dir);
    c.arg("--version")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);
    let out = tokio::time::timeout(VERSION_TIMEOUT, c.output())
        .await
        .map_err(|_| "unavailable: Claude --version timed out")?
        .map_err(|e| format!("unavailable: {e}"))?;
    let version = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if version.is_empty() {
        return Err("unavailable: Claude did not report a version".into());
    }
    Ok(version)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AccessMode, Account};
    use lumen_gatekeeper::{Action, Decision, RequestStatus};
    use std::{path::PathBuf, time::Instant};

    #[tokio::test]
    async fn claude_mcp_tools_are_reviewable_once_without_command_whitelists() {
        let keeper = Gatekeeper::in_memory(Duration::from_secs(30)).unwrap();
        let scope = keeper
            .start_session(
                "chat",
                Engine::Claude,
                "thread",
                "turn",
                &std::env::current_dir().unwrap(),
                Availability::Available {
                    version: "fixture".into(),
                    evidence: "offline".into(),
                },
            )
            .unwrap();
        let input = serde_json::json!({"repository":"fixture","title":"Review me"});
        let message = serde_json::json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"approve","arguments":{"tool_name":"mcp__github__create_issue","input":input}}});
        let pending = keeper.request_claude(&scope, &message).unwrap();
        let snapshot = keeper.snapshot().unwrap();
        assert!(snapshot[0].can_approve);
        assert!(!snapshot[0].whitelist_eligible);
        let request_id = pending.request_id.clone();
        keeper
            .decide(Decision {
                request_id: pending.request_id.clone(),
                scope: scope.clone(),
                action: Action::Approve,
                note: None,
            })
            .unwrap();
        let delivery = pending.wait().await.unwrap();
        let response: serde_json::Value = serde_json::from_str(
            delivery.response["result"]["content"][0]["text"]
                .as_str()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            response,
            serde_json::json!({"behavior":"allow","updatedInput":input})
        );
        assert!(keeper
            .decide(Decision {
                request_id,
                scope,
                action: Action::Approve,
                note: None
            })
            .is_err());
    }

    /// Runs the real Claude CLI against the real MCP bridge. Sends no prompt, so no model turn.
    /// Build the helper first, then point `LUMEN_GATEKEEPER_MCP_EXE` at it (an absolute path):
    /// `cargo build --manifest-path ../src/features/gatekeeper/Cargo.toml --bin lumen-gatekeeper-mcp`
    #[tokio::test]
    #[ignore]
    async fn probe_connects_to_the_installed_claude_cli() {
        let exe = std::env::var("LUMEN_GATEKEEPER_MCP_EXE").expect("set LUMEN_GATEKEEPER_MCP_EXE");
        let approvals = Approvals {
            keeper: Gatekeeper::in_memory(Duration::from_secs(300)).unwrap(),
            chat: "probe-chat".into(),
        };
        let config = std::env::temp_dir().join("lumen-probe-config");
        std::fs::create_dir_all(&config).unwrap();
        let cwd = std::env::temp_dir();
        let version = cli_version(&config).await.unwrap();
        probe(&approvals, Path::new(&exe), &cwd, &config, &version)
            .await
            .unwrap();
    }

    /// One ask-first turn with the real Claude CLI: an approval request must arrive and, once
    /// approved through the broker, the action runs. Costs one small Haiku turn. The account
    /// folder (already signed in) goes in LUMEN_TEST_CLAUDE_CONFIG; its files are never read here.
    #[tokio::test]
    #[ignore]
    async fn approved_action_runs_in_a_real_turn() {
        let exe = PathBuf::from(
            std::env::var("LUMEN_GATEKEEPER_MCP_EXE").expect("set LUMEN_GATEKEEPER_MCP_EXE"),
        );
        let config = PathBuf::from(
            std::env::var("LUMEN_TEST_CLAUDE_CONFIG").expect("set LUMEN_TEST_CLAUDE_CONFIG"),
        );
        let keeper = Gatekeeper::in_memory(Duration::from_secs(300)).unwrap();
        let approvals = Approvals {
            keeper: keeper.clone(),
            chat: "turn-chat".into(),
        };
        let workspace = std::env::temp_dir().join("lumen-ask-turn");
        std::fs::create_dir_all(&workspace).unwrap();
        let account = Account {
            id: "test".into(),
            name: "test".into(),
            provider: Provider::Claude,
        };

        // Stands in for the Gatekeeper panel: approve the first request this turn raises.
        let approver = keeper.clone();
        let decider = tokio::spawn(async move {
            let deadline = Instant::now() + Duration::from_secs(120);
            loop {
                let pending = approver
                    .snapshot()
                    .unwrap()
                    .into_iter()
                    .find(|r| matches!(r.status, RequestStatus::Pending));
                if let Some(request) = pending {
                    approver
                        .decide(Decision {
                            request_id: request.id.clone(),
                            scope: request.scope.clone(),
                            action: Action::Approve,
                            note: None,
                        })
                        .unwrap();
                    return request.tool;
                }
                if Instant::now() >= deadline {
                    return String::new();
                }
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        });

        let (_tx, rx) = watch::channel(false);
        let result = claude_turn(
            Invocation {
                browser: None,
                browser_scope: None,
                browser_artifacts: None,
                resume: None,
                account: &account,
                config: &config,
                workspace: &workspace,
                prompt: "Use the Write tool to create a file named gatekeeper-probe.txt in the current directory with the content ok. Then reply with only the word done.",
                model: Some("haiku"),
                effort: "",
                access: AccessMode::Ask,
                approvals: Some(approvals.clone()),
            },
            approvals,
            &exe,
            rx,
            |a| {
                eprintln!("activity {}: {}", a.kind, a.text.chars().take(300).collect::<String>());
            },
        )
        .await;
        eprintln!("result: {result:?}");
        let tool = decider.await.unwrap();
        eprintln!("approved tool: {tool:?}");
        assert!(result.is_ok(), "turn failed: {result:?}");
        assert_eq!(result.as_deref(), Ok("done"));
        assert_eq!(
            std::fs::read_to_string(workspace.join("gatekeeper-probe.txt"))
                .unwrap()
                .trim(),
            "ok"
        );
        assert!(!tool.is_empty(), "no approval request arrived");
    }
}
