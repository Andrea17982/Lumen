//! Only structured, authenticated protocol messages reach these parsers.
use crate::types::*;
use serde_json::{json, Value};

pub(crate) fn validate_codex_scope(scope: &Scope, p: &Value) -> Result<(), String> {
    if string(p, "threadId")? != scope.provider_session_id || string(p, "turnId")? != scope.turn_id
    {
        return Err("Approval belongs to another Codex thread or turn".into());
    }
    Ok(())
}

fn diffs(changes: &Value) -> Vec<DiffPreview> {
    changes
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| {
            Some(DiffPreview {
                path: v.get("path")?.as_str()?.into(),
                diff: v.get("diff")?.as_str()?.into(),
            })
        })
        .collect()
}

pub(crate) fn codex(
    scope: &Scope,
    message: &Value,
    item: Option<&Value>,
    session_cwd: &str,
) -> Result<Draft, String> {
    let method = string(message, "method")?;
    let p = message.get("params").ok_or("Missing approval params")?;
    validate_codex_scope(scope, p)?;
    let item_id = string(p, "itemId")?;
    let mut d = Draft {
        wire_id: wire_id(message)?,
        item_id: Some(item_id.into()),
        method: method.into(),
        kind: RequestKind::Unsupported,
        tool: method.into(),
        command: None,
        cwd: session_cwd.into(),
        reason: p.get("reason").and_then(Value::as_str).map(str::to_owned),
        diffs: vec![],
        input: p.clone(),
        can_approve: false,
        unavailable_reason: None,
        whitelist_eligible: false,
    };
    match method {
        "item/commandExecution/requestApproval" => {
            let cached = item.filter(|v| v["type"] == "commandExecution");
            d.command = p
                .get("command")
                .and_then(Value::as_str)
                .or_else(|| cached.and_then(|v| v["command"].as_str()))
                .map(str::to_owned);
            d.cwd = p
                .get("cwd")
                .and_then(Value::as_str)
                .or_else(|| cached.and_then(|v| v["cwd"].as_str()))
                .unwrap_or("")
                .into();
            // Network, stdin, broad permission grants and future command kinds
            // need purpose-built views. A shell preview does not describe them.
            let specialized = p.get("kind").is_some_and(|v| v != "command")
                || ["networkApprovalContext", "additionalPermissions"]
                    .iter()
                    .any(|k| p.get(k).is_some_and(|v| !v.is_null()));
            if specialized {
                d.unavailable_reason = Some(
                    "This network, terminal-input or extended-permission approval is not supported"
                        .into(),
                );
            } else if d.command.as_ref().is_none_or(|s| s.is_empty()) || d.cwd.is_empty() {
                d.unavailable_reason =
                    Some("The provider did not supply a command and directory".into());
            } else {
                d.kind = RequestKind::Command;
                d.can_approve = true;
                d.whitelist_eligible = p.get("approvalId").is_none_or(Value::is_null)
                    && p.as_object().is_some_and(|o| {
                        o.keys().all(|key| {
                            [
                                "threadId",
                                "turnId",
                                "itemId",
                                "kind",
                                "startedAtMs",
                                "environmentId",
                                "reason",
                                "command",
                                "cwd",
                                "commandActions",
                                "approvalId",
                                "proposedExecpolicyAmendment",
                                "proposedNetworkPolicyAmendments",
                                "availableDecisions",
                                "networkApprovalContext",
                                "additionalPermissions",
                            ]
                            .contains(&key.as_str())
                        })
                    });
            }
            if p.get("availableDecisions").is_some_and(|v| {
                !v.as_array()
                    .is_some_and(|a| a.iter().any(|v| v == "accept"))
            }) {
                d.can_approve = false;
                d.whitelist_eligible = false;
                d.unavailable_reason = Some("The provider does not offer one-time approval".into());
            }
        }
        "item/fileChange/requestApproval" => {
            d.kind = RequestKind::FileChange;
            d.diffs = item
                .filter(|v| v["type"] == "fileChange")
                .map(|v| diffs(&v["changes"]))
                .unwrap_or_default();
            if p.get("grantRoot").is_some_and(|v| !v.is_null()) {
                d.unavailable_reason =
                    Some("A persistent filesystem grant is not a one-time file approval".into());
            } else if d.diffs.is_empty()
                || d.diffs
                    .iter()
                    .any(|d| d.path.is_empty() || d.diff.is_empty())
                || item
                    .and_then(|v| v["changes"].as_array())
                    .is_none_or(|a| a.len() != d.diffs.len())
            {
                d.unavailable_reason = Some("No file-change preview received for this item".into());
            } else {
                d.can_approve = true;
            }
        }
        _ => {
            d.unavailable_reason = Some("This Codex approval protocol is not supported".into());
        }
    }
    Ok(d)
}

pub(crate) fn claude(message: &Value, session_cwd: &str) -> Result<Draft, String> {
    if message.get("jsonrpc").and_then(Value::as_str) != Some("2.0")
        || string(message, "method")? != "tools/call"
        || string(&message["params"], "name")? != "approve"
    {
        return Err("Not a Gatekeeper MCP permission call".into());
    }
    let args = &message["params"]["arguments"];
    let tool = string(args, "tool_name")?;
    let input = args
        .get("input")
        .filter(|v| v.is_object())
        .ok_or("Missing tool input")?;
    let mut d = Draft {
        wire_id: wire_id(message)?,
        item_id: args
            .get("tool_use_id")
            .and_then(Value::as_str)
            .map(str::to_owned),
        method: "tools/call".into(),
        kind: RequestKind::Tool,
        tool: tool.into(),
        command: None,
        // Bash does not accept a cwd argument. Scope is bound by the native runner.
        cwd: session_cwd.into(),
        reason: input
            .get("description")
            .and_then(Value::as_str)
            .map(str::to_owned),
        diffs: vec![],
        input: input.clone(),
        can_approve: true,
        unavailable_reason: None,
        whitelist_eligible: false,
    };
    match tool {
        "Bash" => {
            d.kind = RequestKind::Command;
            d.command = Some(string(input, "command")?.into());
            // Unknown flags, cwd overrides, background execution and alternate
            // shells are never covered by an exact-command whitelist.
            d.whitelist_eligible = input.as_object().is_some_and(|o| {
                o.keys()
                    .all(|k| ["command", "description", "timeout"].contains(&k.as_str()))
            });
        }
        "Edit" => {
            d.kind = RequestKind::FileChange;
            d.diffs.push(DiffPreview {
                path: string(input, "file_path")?.into(),
                diff: format!(
                    "Old text:\n{}\n\nNew text:\n{}\n\nReplace all: {}",
                    input
                        .get("old_string")
                        .and_then(Value::as_str)
                        .ok_or("Missing old_string")?,
                    input
                        .get("new_string")
                        .and_then(Value::as_str)
                        .ok_or("Missing new_string")?,
                    input
                        .get("replace_all")
                        .and_then(Value::as_bool)
                        .unwrap_or(false)
                ),
            });
        }
        "Write" => {
            d.kind = RequestKind::FileChange;
            d.diffs.push(DiffPreview {
                path: string(input, "file_path")?.into(),
                diff: format!(
                    "Proposed full file content (replaces existing content):\n{}",
                    input
                        .get("content")
                        .and_then(Value::as_str)
                        .ok_or("Missing content")?
                ),
            });
        }
        "Read" | "Glob" | "Grep" | "WebFetch" | "WebSearch" => {}
        _ => {
            d.kind = RequestKind::Unsupported;
            d.can_approve = false;
            d.unavailable_reason =
                Some("This tool requires an adapter before it can be approved".into());
        }
    }
    Ok(d)
}

pub(crate) fn response(provider: &Provider, d: &Draft, action: &Action, note: &str) -> Value {
    match provider {
        Provider::Claude => {
            let payload = if *action == Action::Approve {
                json!({"behavior":"allow", "updatedInput":d.input})
            } else {
                json!({"behavior":"deny", "message":note})
            };
            json!({"jsonrpc":"2.0", "id":d.wire_id, "result":{
                "content":[{"type":"text", "text":payload.to_string()}], "isError":false
            }})
        }
        Provider::Codex => {
            if !matches!(
                d.method.as_str(),
                "item/commandExecution/requestApproval" | "item/fileChange/requestApproval"
            ) {
                return json!({"id":d.wire_id,"error":{"code":-32601,"message":"Unsupported approval; no permissions granted"}});
            }
            let decision = match action {
                Action::Approve => "accept",
                Action::Reject => "decline",
                Action::Cancel => "cancel",
            };
            json!({"id":d.wire_id,"result":{"decision":decision}})
        }
    }
}
