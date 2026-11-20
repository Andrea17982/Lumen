//! OpenCode v2 session policy API + official ACP. No global configuration is rewritten.
use crate::{
    models::*,
    providers::{self, Invocation},
};
use serde_json::{json, Value};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    sync::watch,
};
fn valid_session(s: &str) -> bool {
    s.starts_with("ses_")
        && s.len() <= 160
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}
async fn metadata(
    input: &Invocation<'_>,
    method: &str,
    path: &str,
    body: Option<Value>,
    cancel: watch::Receiver<bool>,
) -> Result<Value, String> {
    let mut c = providers::command(&Provider::Opencode)?;
    providers::isolate(&mut c, &Provider::Opencode, input.config);
    c.current_dir(input.workspace)
        .args(["api", "--standalone", method, path]);
    if let Some(body) = body {
        c.args(["--data", &body.to_string()]);
    }
    c.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());
    let text = crate::catalog::cli_output(c, Some(cancel)).await?;
    if text.trim().is_empty() {
        return Ok(Value::Null);
    }
    let value: Value =
        serde_json::from_str(&text).map_err(|_| "Invalid OpenCode policy API response")?;
    if value.get("error").is_some() {
        return Err("OpenCode session/policy API rejected the request".into());
    }
    Ok(value.get("data").cloned().unwrap_or(value))
}
fn rules(access: AccessMode, servers: &[Value]) -> Value {
    if access == AccessMode::Autopilot {
        return json!([]);
    }
    if access == AccessMode::Ask {
        return json!([{"action":"*","resource":"*","effect":"ask"},{"action":"execute","resource":"*","effect":"deny"},{"action":"lumen_browser_*","resource":"*","effect":"allow"},{"action":"lumen_graph_*","resource":"*","effect":"allow"},{"action":"subagent","resource":"*","effect":"deny"}]);
    }
    let mut rules = vec![json!({"action":"*","resource":"*","effect":"deny"})];
    for action in [
        "read",
        "glob",
        "grep",
        "skill",
        "question",
        "webfetch",
        "websearch",
    ] {
        rules.push(json!({"action":action,"resource":"*","effect":"allow"}));
    }
    if access == AccessMode::FileEdits {
        rules.push(json!({"action":"edit","resource":"*","effect":"allow"}));
    }
    // Source gateways filter their actual tools; native unknown namespaces stay denied.
    for s in servers {
        if let Some(name) = s["name"].as_str() {
            let name = name
                .chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() || c == '_' {
                        c
                    } else {
                        '_'
                    }
                })
                .collect::<String>();
            rules.push(json!({"action":format!("{name}_*"),"resource":"*","effect":"allow"}));
        }
    }
    rules.push(json!({"action":"execute","resource":"*","effect":"deny"}));
    // A custom subagent must never escape the parent turn's restricted profile.
    rules.push(json!({"action":"subagent","resource":"*","effect":"deny"}));
    rules.push(json!({"action":"external_directory","resource":"*","effect":"deny"}));
    json!(rules)
}
pub(crate) fn servers(input: &Invocation<'_>) -> Result<Vec<Value>, String> {
    let data = crate::api::root(input.config)?;
    let mut configs =
        crate::capability_catalog::shared_servers(data, Some(input.workspace), input.access);
    let exe = std::env::current_exe().map_err(|_| "Lumen executable unavailable")?;
    configs.insert(
        "lumen_browser".into(),
        json!({"command":exe,"args":["--lumen-browser-mcp"]}),
    );
    configs.insert(
        "lumen_graph".into(),
        json!({"command":exe,"args":[crate::graph_mcp::FLAG,data,input.workspace]}),
    );
    let mut inherited = crate::proc::command(&exe);
    let browser = input.browser.ok_or("Moli scope unavailable")?;
    browser.env(&mut inherited);
    if let Some(env) = browser.code_env() {
        configs.insert(
            crate::code_mcp::SERVER.into(),
            json!({
                "command":exe, "args":[crate::code_mcp::FLAG],
                "env":env.into_iter().collect::<std::collections::BTreeMap<_, _>>()
            }),
        );
    }
    let inherited = inherited
        .as_std()
        .get_envs()
        .filter(|(k, _)| *k == "LUMEN_BROWSER_URL" || *k == "LUMEN_BROWSER_CAP")
        .filter_map(|(k, v)| {
            v.map(|v| json!({"name":k.to_string_lossy(),"value":v.to_string_lossy()}))
        })
        .collect::<Vec<_>>();
    let mut servers = vec![];
    for (name, c) in configs {
        if let Some(command) = c["command"].as_str() {
            let mut env = if name == "lumen_browser" {
                inherited.clone()
            } else {
                vec![]
            };
            env.extend(
                c["env"]
                    .as_object()
                    .into_iter()
                    .flatten()
                    .filter_map(|(k, v)| v.as_str().map(|v| json!({"name":k,"value":v}))),
            );
            servers.push(json!({"name":name,"command":command,"args":c["args"].as_array().cloned().unwrap_or_default(),"env":env}));
        } else if let Some(url) = c["url"].as_str().or(c["serverUrl"].as_str()) {
            if c["type"] == "sse" {
                continue;
            }
            let headers = c["headers"]
                .as_object()
                .into_iter()
                .flatten()
                .filter_map(|(k, v)| v.as_str().map(|v| json!({"name":k,"value":v})))
                .collect::<Vec<_>>();
            servers.push(json!({"type":"http","name":name,"url":url,"headers":headers}));
        }
    }
    Ok(servers)
}
fn active_session(turn: bool, expected: Option<&str>, params: &Value) -> bool {
    turn && expected.is_some() && params["sessionId"].as_str() == expected
}
struct Client {
    input: tokio::process::ChildStdin,
    output: BufReader<tokio::process::ChildStdout>,
    next: u64,
}
impl Client {
    async fn send(&mut self, v: Value) -> Result<(), String> {
        self.input
            .write_all(format!("{}\n", v).as_bytes())
            .await
            .map_err(|_| "OpenCode ACP disconnected".to_owned())
    }
    async fn rpc(
        &mut self,
        method: &str,
        params: Value,
        access: AccessMode,
        emit: &mut impl FnMut(Activity),
        turn: bool,
        text: &mut String,
    ) -> Result<Value, String> {
        self.next += 1;
        let id = self.next;
        let session_id = params["sessionId"].as_str().map(str::to_owned);
        self.send(json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}))
            .await?;
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(30);
        loop {
            let mut line = vec![];
            let read = async {
                (&mut self.output)
                    .take(8_000_001)
                    .read_until(b'\n', &mut line)
                    .await
            };
            let n = if turn {
                read.await
            } else {
                tokio::time::timeout_at(deadline, read)
                    .await
                    .map_err(|_| "OpenCode ACP setup timed out; no prompt sent")?
            }
            .map_err(|_| "OpenCode ACP stream interrupted")?;
            if n == 0 || n > 8_000_000 || line.last() != Some(&b'\n') {
                return Err("OpenCode ACP stream is incomplete or oversized".into());
            }
            let v: Value =
                serde_json::from_slice(&line).map_err(|_| "Invalid OpenCode ACP frame")?;
            if v["method"] == "session/request_permission" {
                if !active_session(turn, session_id.as_deref(), &v["params"]) {
                    self.send(json!({"jsonrpc":"2.0","id":v["id"],"result":{"outcome":{"outcome":"cancelled"}}})).await?;
                    continue;
                }
                let tc = &v["params"]["toolCall"];
                let kind = tc["kind"].as_str().unwrap_or("other");
                let allowed = match access {
                    AccessMode::Autopilot => true,
                    AccessMode::Ask => {
                        crate::api_tools::permission(
                            "OpenCode action",
                            &format!(
                                "{}\n{}",
                                tc["title"].as_str().unwrap_or("Native tool"),
                                tc["rawInput"]
                            ),
                            emit,
                        )
                        .await?
                    }
                    _ => ["read", "search", "fetch"].contains(&kind),
                };
                let choice = v["params"]["options"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .find(|o| o["kind"] == "allow_once")
                    .and_then(|o| o["optionId"].as_str());
                let outcome = if allowed {
                    choice
                        .map(|s| json!({"outcome":"selected","optionId":s}))
                        .unwrap_or(json!({"outcome":"cancelled"}))
                } else {
                    json!({"outcome":"cancelled"})
                };
                self.send(json!({"jsonrpc":"2.0","id":v["id"],"result":{"outcome":outcome}}))
                    .await?;
                continue;
            }
            if v["method"].is_string() && v.get("id").is_some() {
                self.send(json!({"jsonrpc":"2.0","id":v["id"],"error":{"code":-32601,"message":"Unsupported client action; no permission granted"}})).await?;
                continue;
            }
            if v["id"] == id && v.get("method").is_none() {
                if v.get("error").is_some() {
                    let raw = v["error"].to_string();
                    if crate::limits::is_rate_limit(&raw) {
                        let secs = ((crate::limits::unlock_at(&raw, now()) - now()).max(1000)
                            / 1000)
                            .max(1);
                        return Err(format!(
                            "{}OpenCode quota exhausted. Try again in {secs} seconds.",
                            crate::limits::RATE_LIMIT
                        ));
                    }
                    if raw.to_lowercase().contains("not found") {
                        return Err("OpenCode session not found".into());
                    }
                    return Err(format!(
                        "OpenCode ACP {} failed; verify native session, model and permissions",
                        method
                    ));
                }
                return Ok(v["result"].clone());
            }
            if v["method"] == "session/update"
                && active_session(turn, session_id.as_deref(), &v["params"])
            {
                let u = &v["params"]["update"];
                match u["sessionUpdate"].as_str().unwrap_or("") {
                    "agent_message_chunk" => {
                        if let Some(t) = u["content"]["text"].as_str() {
                            text.push_str(t);
                            emit(Activity {
                                kind: "replace".into(),
                                text: text.clone(),
                            })
                        }
                    }
                    "agent_thought_chunk" => {
                        if let Some(t) = u["content"]["text"].as_str() {
                            emit(Activity {
                                kind: "thinking".into(),
                                text: t.into(),
                            })
                        }
                    }
                    "tool_call" | "tool_call_update" => {
                        let id = u["toolCallId"].as_str().unwrap_or("tool");
                        let done = ["completed", "failed"].iter().any(|s| u["status"] == *s);
                        emit(Activity {
                            kind: if done { "tool-end" } else { "tool-start" }.into(),
                            text: json!({"id":id}).to_string(),
                        });
                        if let Some(title) = u["title"].as_str() {
                            emit(Activity {
                                kind: "tool".into(),
                                text: title.into(),
                            })
                        }
                    }
                    "available_commands_update"
                    | "config_option_update"
                    | "current_mode_update" => {}
                    _ => emit(Activity {
                        kind: "provider-event".into(),
                        text: u["sessionUpdate"]
                            .as_str()
                            .unwrap_or("OpenCode event")
                            .into(),
                    }),
                }
            }
        }
    }
}
pub async fn execute(
    input: Invocation<'_>,
    mut cancel: watch::Receiver<bool>,
    mut emit: impl FnMut(Activity),
) -> Result<String, String> {
    let workspace = dunce::canonicalize(input.workspace).map_err(|_| "Project unavailable")?;
    let servers = servers(&input)?;
    let policy = rules(input.access, &servers);
    let id = if let Some(id) = input.resume {
        if !valid_session(id) {
            return Err("Invalid OpenCode native session".into());
        }
        let path = format!("/api/session/{id}");
        let old = metadata(&input, "GET", &path, None, cancel.clone()).await?;
        if old["location"]["directory"]
            .as_str()
            .and_then(|p| dunce::canonicalize(p).ok())
            .as_ref()
            != Some(&workspace)
        {
            return Err("OpenCode native session belongs to another workspace".into());
        }
        metadata(
            &input,
            "PATCH",
            &path,
            Some(json!({"permissions":policy})),
            cancel.clone(),
        )
        .await?;
        id.to_owned()
    } else {
        let s=metadata(&input,"POST","/api/session",Some(json!({"title":"Lumen session","location":{"directory":workspace},"permissions":policy})),cancel.clone()).await?;
        s["id"]
            .as_str()
            .filter(|s| valid_session(s))
            .ok_or("OpenCode returned no native session ID")?
            .to_owned()
    };
    let check = metadata(
        &input,
        "GET",
        &format!("/api/session/{id}"),
        None,
        cancel.clone(),
    )
    .await?;
    if check["permissions"] != policy {
        return Err(
            "OpenCode did not retain the requested session permissions; no prompt was sent".into(),
        );
    }
    let mut command = providers::command(&Provider::Opencode)?;
    providers::isolate(&mut command, &Provider::Opencode, input.config);
    command
        .current_dir(&workspace)
        .arg("acp")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());
    let mut child =
        crate::proc::ServiceChild::spawn(command).map_err(|_| "Cannot start OpenCode ACP")?;
    let mut client = Client {
        input: child.stdin.take().ok_or("Missing ACP input")?,
        output: BufReader::new(child.stdout.take().ok_or("Missing ACP output")?),
        next: 0,
    };
    let run = async {
        let mut ignored = String::new();
        let init=client.rpc("initialize",json!({"protocolVersion":1,"clientCapabilities":{"fs":{"readTextFile":false,"writeTextFile":false},"terminal":false},"clientInfo":{"name":"Lumen","version":env!("CARGO_PKG_VERSION")}}),input.access,&mut emit,false,&mut ignored).await?;
        if init["protocolVersion"] != 1 || init["agentCapabilities"]["loadSession"] != true {
            return Err("OpenCode ACP native session loading is unavailable".into());
        }
        client
            .rpc(
                "session/load",
                json!({"sessionId":id,"cwd":workspace,"mcpServers":servers}),
                input.access,
                &mut emit,
                false,
                &mut ignored,
            )
            .await?;
        for (option, value) in [
            ("model", input.model.unwrap_or("")),
            ("effort", input.effort),
        ] {
            if value.is_empty() {
                continue;
            }
            let selected = client
                .rpc(
                    "session/set_config_option",
                    json!({"sessionId":id,"configId":option,"value":value}),
                    input.access,
                    &mut emit,
                    false,
                    &mut ignored,
                )
                .await?;
            if !selected["configOptions"].as_array().is_some_and(|options| {
                options
                    .iter()
                    .any(|o| o["id"] == option && o["currentValue"] == value)
            }) {
                return Err(format!(
                    "OpenCode did not confirm the requested {option}; no prompt sent"
                ));
            }
        }
        emit(Activity {
            kind: "session".into(),
            text: id.clone(),
        });
        let mut text = String::new();
        let end = client
            .rpc(
                "session/prompt",
                json!({"sessionId":id,"prompt":crate::providers::wire_prompt(&Provider::Opencode, input.prompt)}),
                input.access,
                &mut emit,
                true,
                &mut text,
            )
            .await?;
        if end["stopReason"] == "cancelled" {
            return Err("Cancelled".into());
        }
        if end["stopReason"] != "end_turn" {
            return Err("OpenCode turn stopped before completion; partial work retained".into());
        }
        if text.trim().is_empty() {
            return Err("OpenCode returned no assistant text".into());
        }
        Ok(text)
    };
    let result = tokio::select! {biased;_=cancel.changed()=>Err("Cancelled".into()),r=run=>r};
    if result.as_ref().is_err_and(|e| e == "Cancelled") {
        let _ = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            client
                .send(json!({"jsonrpc":"2.0","method":"session/cancel","params":{"sessionId":id}})),
        )
        .await;
    }
    drop(client);
    child.finish(result.is_ok()).await?;
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn restricted_rules_deny_unknown_actions_and_subagents() {
        for m in [AccessMode::ReadOnly, AccessMode::FileEdits] {
            let r = rules(m, &[]);
            assert_eq!(r[0]["effect"], "deny");
            assert!(r
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v["action"] == "external_directory" && v["effect"] == "deny"));
            assert_eq!(
                r.as_array().unwrap().iter().any(|v| v["action"] == "edit"),
                m == AccessMode::FileEdits
            )
        }
        assert_eq!(rules(AccessMode::Ask, &[])[0]["effect"], "ask");
    }
    #[test]
    fn only_active_owned_session_can_emit_or_request_permission() {
        assert!(active_session(
            true,
            Some("ses_owned"),
            &json!({"sessionId":"ses_owned"})
        ));
        assert!(!active_session(
            false,
            Some("ses_owned"),
            &json!({"sessionId":"ses_owned"})
        ));
        assert!(!active_session(
            true,
            Some("ses_owned"),
            &json!({"sessionId":"ses_other"})
        ));
        assert!(!active_session(true, Some("ses_owned"), &json!({})));
        assert!(!active_session(true, None, &json!({})));
    }
}
