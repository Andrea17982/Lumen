//! Claude's bidirectional stream-json protocol. Stdin stays open for native questions and
//! permission callbacks; no synthetic system prompt replaces Claude Code's own harness.
use crate::{
    models::{AccessMode, Activity, Provider},
    providers::{self, Invocation},
};
use lumen_gatekeeper::{Availability, Provider as Engine};
use serde_json::{json, Value};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    sync::{mpsc, oneshot, watch},
};

fn response(id: &Value, result: Value) -> Value {
    json!({"type":"control_response","response":{"subtype":"success","request_id":id,"response":result}})
}
/// The empty successful result a resumed session sends when it closes queued work, such as
/// the notice for a background task the previous process left running (Claude Code 2.1.327).
fn stale_result(value: &Value) -> bool {
    value["type"] == "result"
        && value["is_error"] != true
        && value["result"].as_str().unwrap_or("").trim().is_empty()
}
#[cfg(test)]
mod stale_tests {
    use super::stale_result;
    use serde_json::json;
    #[test]
    fn only_an_empty_success_is_queued_work() {
        assert!(stale_result(
            &json!({"type":"result","subtype":"success","is_error":false,"num_turns":0,"result":""})
        ));
        assert!(!stale_result(
            &json!({"type":"result","is_error":true,"result":"You've hit your usage limit"})
        ));
        assert!(!stale_result(
            &json!({"type":"result","is_error":false,"result":"The answer"})
        ));
        assert!(!stale_result(
            &json!({"type":"assistant","message":{"content":[]}})
        ));
    }
}
fn connector_status(value: &Value) -> Vec<Activity> {
    if value["type"] != "system" || value["subtype"] != "init" {
        return vec![];
    }
    value["mcp_servers"].as_array().into_iter().flatten().take(100).map(|server| {
        let name: String = server["name"].as_str().unwrap_or("connector").chars().filter(|c| c.is_ascii_alphanumeric() || " _-.".contains(*c)).take(100).collect();
        let state=match server["status"].as_str() {Some("connected")=>"connected",Some("pending")=>"pending",Some("needs-auth")=>"needs authentication",Some("failed")=>"failed",Some("disabled")=>"disabled",_=>"status unknown"};
        Activity {kind:"connector-status".into(),text:format!("Claude connector {name}: {state}. Connections belong to the selected account and permission profile.")}
    }).collect()
}
type Frame = (Value, Option<oneshot::Sender<bool>>);
#[derive(Clone)]
struct Transport(mpsc::Sender<Frame>);
impl Transport {
    async fn send(&self, value: Value) -> Result<(), String> {
        self.0
            .send((value, None))
            .await
            .map_err(|_| "Claude closed its input".into())
    }
    async fn deliver(&self, value: Value) -> bool {
        let (sent, receive) = oneshot::channel();
        self.0.send((value, Some(sent))).await.is_ok() && receive.await.unwrap_or(false)
    }
}
struct SessionGuard {
    keeper: lumen_gatekeeper::Gatekeeper,
    scope: lumen_gatekeeper::Scope,
    bridge: tokio::task::JoinHandle<Result<(), String>>,
}
impl Drop for SessionGuard {
    fn drop(&mut self) {
        self.bridge.abort();
        let _ = self
            .keeper
            .disconnect(&self.scope, "Claude connection closed");
    }
}
pub async fn execute(
    input: Invocation<'_>,
    mut cancel: watch::Receiver<bool>,
    mut emit: impl FnMut(Activity),
) -> Result<String, String> {
    crate::execution::signal(&mut emit, "adapter-start");
    if *cancel.borrow() {
        return Err("Cancelled".into());
    }
    let cwd = dunce::canonicalize(input.workspace)
        .map_err(|_| "The conversation folder is unavailable")?;
    let mut c = providers::command(&Provider::Claude)?;
    providers::isolate(&mut c, &Provider::Claude, input.config);
    let mut args = if input.access == AccessMode::Ask {
        [
            "--print",
            "--verbose",
            "--output-format",
            "stream-json",
            "--include-partial-messages",
            "--permission-mode",
            "default",
            "--disallowedTools",
            "Agent,Task",
        ]
        .map(String::from)
        .to_vec()
    } else {
        providers::invocation_args(&Provider::Claude, input.access, input.model, input.effort)
    };
    args.extend([
        "--input-format".into(),
        "stream-json".into(),
        "--permission-prompt-tool".into(),
        "stdio".into(),
        // The CLI echoes the prompt when it starts it: the turn's result is the one after it.
        "--replay-user-messages".into(),
    ]);
    if input.access == AccessMode::Ask {
        if let Some(model) = input.model {
            args.extend(["--model".into(), model.into()]);
        }
        if !input.effort.is_empty() {
            args.extend(["--effort".into(), input.effort.into()]);
        }
    }
    let native_id = input
        .resume
        .map(str::to_owned)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    args.extend([
        if input.resume.is_some() {
            "--resume"
        } else {
            "--session-id"
        }
        .into(),
        native_id.clone(),
    ]);
    let mut _shared_config = None;
    if let Some(data) = input.config.parent().and_then(std::path::Path::parent) {
        let (extra, names, guard) = crate::integrations::args_for_turn(
            data,
            &Provider::Claude,
            Some(&cwd),
            input.access,
            input.browser,
        )?;
        _shared_config = guard;
        args.extend(extra);
        if let Some(i) = args.iter().position(|a| a == "--allowedTools") {
            for name in names {
                args[i + 1].push_str(&format!(",mcp__{name}"));
            }
        }
        if providers::graph_enabled(data) {
            if let Some(extra) = crate::graph_mcp::cli_args(data, &cwd, &Provider::Claude) {
                providers::add_graph(&mut args, extra, &Provider::Claude);
            }
        }
    }
    let mut scope = None;
    let mut session_guard = None;
    if let Some(approvals) = &input.approvals {
        let owned = approvals.keeper.start_worker_session(&approvals.chat, Engine::Claude, &native_id, &uuid::Uuid::new_v4().to_string(), &cwd, Availability::Available { version:"native stdio".into(), evidence:"Local connection; model execution requires the installed CLI initialization acknowledgement".into() }, approvals.worker.clone())?;
        let endpoint =
            lumen_gatekeeper::mcp::ClaudeEndpoint::bind(approvals.keeper.clone(), owned.clone())
                .await?
                .screens_only();
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        args.extend([
            "--mcp-config".into(),
            endpoint.config(&exe, true)?.to_string(),
        ]);
        scope = Some(owned.clone());
        session_guard = Some(SessionGuard {
            keeper: approvals.keeper.clone(),
            scope: owned,
            bridge: tokio::spawn(endpoint.serve()),
        });
    } else if input.access == AccessMode::Ask {
        return Err("The approval journal is unavailable".into());
    }
    if let Some(browser) = input.browser {
        browser.env(&mut c);
        browser.args(&mut args, &Provider::Claude)?;
    }
    c.current_dir(&cwd)
        .args(args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    crate::execution::signal(&mut emit, "spawn-start");
    let mut child = c.spawn().map_err(|e| format!("Cannot start Claude: {e}"))?;
    crate::execution::signal(&mut emit, "process-started");
    if let Some(pid) = child.id() {
        emit(Activity {
            kind: "pid".into(),
            text: pid.to_string(),
        });
    }
    let mut stdin = child.stdin.take().ok_or("No Claude input")?;
    let (tx, mut rx) = mpsc::channel::<Frame>(64);
    let tx = Transport(tx);
    let (failed_tx, mut failed) = watch::channel(false);
    let writing = tokio::spawn(async move {
        while let Some((value, delivered)) = rx.recv().await {
            let ok = stdin
                .write_all(format!("{value}\n").as_bytes())
                .await
                .is_ok()
                && stdin.flush().await.is_ok();
            if let Some(delivered) = delivered {
                let _ = delivered.send(ok);
            }
            if !ok {
                let _ = failed_tx.send(true);
                break;
            }
        }
    });
    let stderr = child.stderr.take().ok_or("No Claude diagnostics")?;
    // Diagnostics are never exposed (they can describe authentication); only a missing
    // session is recognized, so Lumen can replay the request in a new one.
    let missing = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let seen = missing.clone();
    let diagnostics = tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if line.contains("No conversation found") {
                seen.store(true, std::sync::atomic::Ordering::SeqCst);
            }
        }
    });
    let mut lines = BufReader::new(child.stdout.take().ok_or("No Claude output")?).lines();
    let questions = crate::interactions::Connection::default();
    let init_id = uuid::Uuid::new_v4().to_string();
    tx.send(json!({"type":"control_request","request_id":init_id,"request":{"subtype":"initialize","forwardSubagentText":true}})).await.map_err(|_| "Claude closed its input")?;
    let mut jobs = tokio::task::JoinSet::new();
    let delivering = std::sync::Arc::new(tokio::sync::RwLock::new(()));
    let result = async {
        // Verify the installed CLI acknowledges the control protocol before starting inference.
        tokio::time::timeout(std::time::Duration::from_secs(60), async {
            loop {
                let line = tokio::select! { biased; _=cancel.changed()=>return Err("Cancelled".to_string()), r=lines.next_line()=>r.map_err(|e|e.to_string())? };
                let Some(line) = line else {
                    // stderr may still be draining when stdout closes.
                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                    return Err(if missing.load(std::sync::atomic::Ordering::SeqCst) {
                        "Claude session not found: no conversation found to resume".to_string()
                    } else {
                        "Claude exited during initialization".to_string()
                    });
                };
                let value: Value = serde_json::from_str(&line).map_err(|e|e.to_string())?;
                if value["type"] == "control_response" && value["response"]["request_id"] == init_id {
                    if value["response"]["subtype"] == "error" { return Err("Installed Claude does not support the interactive connection".into()); }
                    return Ok(());
                }
            }
        }).await.map_err(|_|"Claude did not initialize in time".to_string())??;
        crate::execution::signal(&mut emit, "protocol-ready");
        let prompt_id = uuid::Uuid::new_v4().to_string();
        let mut prompt = crate::providers::wire_prompt(&crate::models::Provider::Claude, input.prompt);
        prompt["uuid"] = json!(prompt_id);
        tx.send(prompt).await.map_err(|_|"Claude closed its input")?;
        crate::execution::signal(&mut emit, "request-enqueued");
        let mut text = String::new();
        let mut initialized = false;
        // A resumed session can first finish work queued before this request, such as the
        // notice for a background task the previous process left running. Its empty result
        // is not this turn's answer.
        let mut started = false;
        loop {
            let line = tokio::select! { biased; _=cancel.changed()=>return Err("Cancelled".into()), _=failed.changed()=>return Err("Claude input delivery failed; the turn was stopped".into()), r=lines.next_line()=>r.map_err(|e|e.to_string())? };
            let Some(line) = line else { return Err("Claude exited before completing the turn".into()); };
            if line.len() > 16*1024*1024 { return Err("Claude sent an oversized protocol message".into()); }
            let Ok(value) = serde_json::from_str::<Value>(&line) else { continue };
            questions.drain(&mut emit);
            let request = &value["request"];
            let child = ["parent_tool_use_id", "parent_agent_id", "agent_id"]
                .iter()
                .any(|field| !value[*field].is_null() || !request[*field].is_null());
            let foreign = [value["session_id"].as_str(), request["session_id"].as_str()]
                .into_iter()
                .flatten()
                .any(|id| id != native_id);
            let init = value["type"] == "system" && value["subtype"] == "init";
            if child || foreign {
                if value["type"] == "control_request" {
                    let decision = if request["subtype"] == "elicitation" {
                        json!({"action":"cancel","content":null})
                    } else {
                        json!({"behavior":"deny","message":"Lumen does not approve requests from sub-agents"})
                    };
                    tokio::select! { biased;
                        _ = cancel.changed() => return Err("Cancelled".into()),
                        sent = tx.send(response(&value["request_id"], decision)) => sent.map_err(|_| "Claude closed its input")?,
                    }
                } else if value["parent_tool_use_id"].is_string() {
                    for activity in providers::normalize(&Provider::Claude, &value) {
                        emit(activity);
                    }
                }
                continue;
            }
            if value["type"] == "user" && value["uuid"] == prompt_id.as_str() && !started {
                started = true;
                // Output of queued earlier work is not part of this answer.
                text.clear();
            }
            if init {
                if value["session_id"] != native_id { return Err("Claude started a different session than requested".into()); }
                initialized = true;
            }
            if !initialized {
                if value["type"] == "result" {
                    return Err("Claude returned a result before confirming the requested session".into());
                }
                continue;
            }
            // Before Claude starts this request, only an empty success closes queued work.
            // An error still ends the turn, and a CLI that never echoes the prompt still
            // finishes with its answer.
            if !started && stale_result(&value) {
                continue;
            }
            for status in connector_status(&value) {emit(status);}
            if value["type"] == "control_request" {
                let id = value["request_id"].clone();
                if request["subtype"] == "elicitation" {
                    let normalized=json!({"serverName":request["mcp_server_name"],"message":request["message"],"mode":request["mode"].as_str().unwrap_or("form"),"url":request["url"],"elicitationId":request["elicitation_id"],"requestedSchema":request["requested_schema"]});
                    let pending=crate::connectors::Request::parse(&normalized).and_then(|parsed|questions.ask_connector(parsed.clone(),id.to_string()).map(|(activity,receive)|(parsed,activity,receive)));
                    match pending {
                        Ok((parsed,activity,receive))=>{
                            emit(activity);let tx=tx.clone();
                            jobs.spawn(async move {if let Ok(answers)=receive.await {let decision=parsed.response(&answers).unwrap_or(json!({"action":"cancel","content":null}));let _=tx.send(response(&id,decision)).await;}});
                        }
                        Err(reason)=>{emit(Activity {kind:"connector-status".into(),text:format!("Claude connector request cancelled: {reason}")});tx.send(response(&id,json!({"action":"cancel","content":null}))).await.map_err(|_|"Claude closed its input")?;}
                    }
                    continue;
                }
                if request["subtype"] == "can_use_tool" && request["tool_name"] == "AskUserQuestion" {
                    let (activity, receive) = questions.ask(crate::interactions::claude_questions(&request["input"])?,id.to_string())?;
                    emit(activity);
                    let tx = tx.clone();
                    let mut updated = request["input"].clone();
                    jobs.spawn(async move {
                        if let Ok(answers) = receive.await {
                            updated["answers"] = json!(answers.into_iter().map(|(id, a)| (id,a.join(", "))).collect::<std::collections::HashMap<_,_>>());
                            let _ = tx.send(response(&id,json!({"behavior":"allow","updatedInput":updated}))).await;
                        }
                    });
                } else if request["subtype"] == "can_use_tool" && input.access == AccessMode::Ask {
                    let approvals = input.approvals.as_ref().ok_or("The approval journal is unavailable")?.clone();
                    let scope = scope.as_ref().ok_or("Claude requested permission before starting its session")?.clone();
                    let message = json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":"approve","arguments":request}});
                    let pending = approvals.keeper.request_claude(&scope,&message)?;
                    let tx = tx.clone();
                    let delivering = delivering.clone();
                    jobs.spawn(async move {
                        if let Ok(delivery) = pending.wait().await {
                            // Held until the outcome is recorded, so the end of the turn cannot cut in between.
                            let _delivering = delivering.read_owned().await;
                            if !approvals.keeper.delivery_is_current(&delivery).unwrap_or(false) { return; }
                            let decision = delivery.response["result"]["content"][0]["text"].as_str().and_then(|s|serde_json::from_str::<Value>(s).ok()).unwrap_or(json!({"behavior":"deny","message":"Invalid approval response"}));
                            let sent = tx.deliver(response(&id,decision)).await;
                            let _ = approvals.keeper.complete_delivery(&delivery,sent);
                        }
                    });
                } else {
                    let decision = if request["subtype"] == "can_use_tool" && input.access == AccessMode::Autopilot { json!({"behavior":"allow","updatedInput":request["input"]}) } else { json!({"behavior":"deny","message":"This action is outside the current permission profile"}) };
                    tx.send(response(&id,decision)).await.map_err(|_|"Claude closed its input")?;
                }
                continue;
            }
            if value["type"] == "control_cancel_request" {
                questions.cancel(&value["request_id"].to_string());
                questions.drain(&mut emit);
                if let (Some(approvals), Some(scope)) = (&input.approvals, &scope) {
                    let _ = approvals.keeper.cancel_wire(scope, &value["request_id"]);
                }
            }
            if value["type"] == "rate_limit_event" { emit(Activity { kind:"quota".into(), text:value.to_string() }); }
            for activity in providers::normalize(&Provider::Claude,&value) {
                match activity.kind.as_str() {
                    "delta" => { text.push_str(&activity.text); emit(activity); },
                    "final" => { if text.trim().is_empty() { text=activity.text; emit(Activity { kind:"replace".into(),text:text.clone() }); } },
                    "error" => return Err(if crate::limits::is_rate_limit(&activity.text) { format!("{}{}",crate::limits::RATE_LIMIT,activity.text) } else { activity.text }),
                    _ => emit(activity),
                }
            }
            if value["type"] == "result" {
                if text.trim().is_empty() { return Err("Claude returned no assistant text".into()); }
                return Ok(text);
            }
        }
    }.await;
    // A decision already being written finishes recording its outcome; a later one stays undelivered.
    let _closed = tokio::time::timeout(std::time::Duration::from_secs(2), delivering.write()).await;
    jobs.abort_all();
    if let (Some(approvals), Some(scope)) = (&input.approvals, &scope) {
        let _ = approvals.keeper.disconnect(scope, "Claude turn finished");
    }
    providers::terminate(&mut child).await;
    writing.abort();
    diagnostics.abort();
    drop(session_guard);
    result
}
