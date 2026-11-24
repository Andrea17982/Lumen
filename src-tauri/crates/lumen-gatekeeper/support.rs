//! Evidence validators, not optimistic version comparisons. The runner collects
//! metadata from the selected installed binary, under its isolated account env.
use crate::Availability;
use serde_json::Value;

pub fn codex(
    version: &str,
    request_schema: &Value,
    command_response_schema: &Value,
    file_response_schema: &Value,
    initialize_response: &Value,
) -> Availability {
    let init_ok = initialize_response.get("error").is_none()
        && initialize_response
            .get("result")
            .is_some_and(Value::is_object);
    let schema_ok = [
        (
            "item/commandExecution/requestApproval",
            "CommandExecutionRequestApprovalParams",
        ),
        (
            "item/fileChange/requestApproval",
            "FileChangeRequestApprovalParams",
        ),
    ]
    .iter()
    .all(|(method, definition)| {
        request_schema["oneOf"].as_array().is_some_and(|variants| {
            variants.iter().any(|v| {
                v["properties"]["method"]["enum"]
                    .as_array()
                    .is_some_and(|a| a.iter().any(|v| v == method))
                    && v["properties"]["params"]["$ref"] == format!("#/definitions/{definition}")
            })
        }) && ["threadId", "turnId", "itemId"].iter().all(|field| {
            request_schema["definitions"][definition]["required"]
                .as_array()
                .is_some_and(|a| a.iter().any(|v| v == field))
        })
    });
    let decisions_ok = [command_response_schema, file_response_schema]
        .iter()
        .all(|schema| {
            let decision = &schema["properties"]["decision"];
            let decision = decision
                .get("$ref")
                .and_then(Value::as_str)
                .and_then(|r| r.strip_prefix("#/definitions/"))
                .map_or(decision, |name| &schema["definitions"][name]);
            ["accept", "decline", "cancel"]
                .iter()
                .all(|expected| has_enum(decision, expected))
        });
    if !version.trim().is_empty() && init_ok && schema_ok && decisions_ok {
        Availability::Available {
            version: version.trim().into(),
            evidence: "Installed CLI generated matching command/file approval schemas and completed app-server initialize".into(),
        }
    } else {
        Availability::Unavailable { reason: "Codex app-server did not provide the required approval schemas and initialization; update or select a supported CLI".into() }
    }
}

fn has_enum(value: &Value, expected: &str) -> bool {
    match value {
        Value::Object(o) => {
            o.get("enum")
                .and_then(Value::as_array)
                .is_some_and(|a| a.iter().any(|v| v == expected))
                || ["oneOf", "anyOf"]
                    .iter()
                    .any(|key| o.get(*key).is_some_and(|v| has_enum(v, expected)))
        }
        Value::Array(a) => a.iter().any(|v| has_enum(v, expected)),
        _ => false,
    }
}

/// The hidden --permission-prompt-tool flag must have been accepted by the
/// real child; readiness must come from this run's authenticated MCP endpoint.
pub fn claude(version: &str, flag_accepted: bool, mcp_tool_discovered: bool) -> Availability {
    if !version.trim().is_empty() && flag_accepted && mcp_tool_discovered {
        Availability::Available {
            version: version.trim().into(),
            evidence: "Installed CLI accepted permission-prompt-tool and discovered the permission tool over authenticated MCP".into(),
        }
    } else {
        Availability::Unavailable { reason: "Claude permission-prompt-tool or the dedicated MCP bridge is unavailable; no automatic authorization".into() }
    }
}

/// Build args independently from existing headless/autopilot runners. In
/// particular, do not append any skip-permissions or approval_policy=never flag.
pub fn claude_args(mcp_config: &Value, provider_session_id: &str) -> Vec<String> {
    [
        "--print",
        "--verbose",
        "--input-format",
        "stream-json",
        "--output-format",
        "stream-json",
        "--permission-mode",
        "default",
        "--permission-prompt-tool",
        "mcp__lumen_gatekeeper__approve",
        "--strict-mcp-config",
        "--mcp-config",
        &mcp_config.to_string(),
        "--session-id",
        provider_session_id,
        "--setting-sources",
        "",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

pub fn codex_args() -> Vec<String> {
    [
        "app-server",
        "--listen",
        "stdio://",
        "-c",
        "approval_policy=\"on-request\"",
        "-c",
        "sandbox_mode=\"read-only\"",
        "-c",
        "approvals_reviewer=\"user\"",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

pub fn codex_thread_params(cwd: &str) -> Value {
    serde_json::json!({"cwd":cwd,"approvalPolicy":"on-request","approvalsReviewer":"user","sandbox":"read-only"})
}
