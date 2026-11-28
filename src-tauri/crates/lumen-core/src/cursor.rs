//! Official Cursor headless CLI, using the user's existing native profile.
use crate::{models::*, providers};
use serde_json::Value;
use tokio::sync::watch;

pub async fn models() -> Result<Vec<ModelInfo>, String> {
    let text =
        crate::catalog::headless_models_output(&Provider::Cursor, &["--list-models"]).await?;
    let rows = parse_models(&text);
    if rows.is_empty() {
        return Err("Cursor returned no supported model identifiers. Complete official CLI login and check model access; no default catalog is invented.".into());
    }
    Ok(rows)
}

fn parse_models(text: &str) -> Vec<ModelInfo> {
    text.lines()
        .filter_map(|line| {
            let (id, name) = line.trim().split_once(" - ")?;
            if id.is_empty()
                || id.len() > 100
                || !id
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "-._:/[]".contains(c))
            {
                return None;
            }
            Some(ModelInfo {
                resolved_id: None,
                id: id.into(),
                name: name.trim().into(),
                description: "Cursor native profile · Autopilot only".into(),
                efforts: vec![],
                default_effort: String::new(),
                is_default: false,
            })
        })
        .collect()
}

pub async fn execute(
    input: providers::Invocation<'_>,
    cancel: watch::Receiver<bool>,
    emit: impl FnMut(Activity),
) -> Result<String, String> {
    // shortcut: headless approvals are not bridged; add restricted modes after enforcement tests.
    if input.access != AccessMode::Autopilot {
        return Err("Cursor currently supports Autopilot only. Restricted permissions and Ask first are not integrated; no request was sent.".into());
    }
    let workspace =
        dunce::canonicalize(input.workspace).map_err(|_| "Conversation folder unavailable")?;
    let mut command = providers::command(&Provider::Cursor)?;
    providers::isolate(&mut command, &Provider::Cursor, input.config);
    command.current_dir(&workspace).args([
        "--print",
        "--output-format",
        "stream-json",
        "--force",
        "--trust",
    ]);
    if let Some(model) = input.model {
        command.args(["--model", model]);
    }
    if let Some(session) = input.resume {
        command.args(["--resume", session]);
    }
    providers::run_owned_json(command, input.prompt, &Provider::Cursor, cancel, emit).await
}

pub fn normalize(v: &Value) -> Vec<Activity> {
    let mut rows = vec![];
    let mut push = |kind: &str, text: &str| {
        if !text.is_empty() {
            rows.push(Activity {
                kind: kind.into(),
                text: text.into(),
            });
        }
    };
    if let Some(session) = v["session_id"]
        .as_str()
        .filter(|id| uuid::Uuid::parse_str(id).is_ok())
    {
        push("session", session);
    }
    match v["type"].as_str().unwrap_or("") {
        "system" if v["subtype"] == "init" => {}
        "user" => {}
        "assistant" => {
            // Partial output is not requested: each event contains one complete message segment.
            for part in v["message"]["content"].as_array().into_iter().flatten() {
                if part["type"] == "text" {
                    push("text", part["text"].as_str().unwrap_or(""));
                }
            }
        }
        "tool_call" => {
            if let Some(id) = v["call_id"].as_str() {
                push(
                    if v["subtype"] == "completed" {
                        "tool-end"
                    } else {
                        "tool-start"
                    },
                    &serde_json::json!({"id":id}).to_string(),
                );
            }
            let tool = v["tool_call"]
                .as_object()
                .and_then(|o| o.keys().next())
                .map(String::as_str)
                .unwrap_or("Cursor tool");
            push("tool", tool);
        }
        "result" if v["is_error"] == false && v["subtype"] == "success" => {
            push("final", v["result"].as_str().unwrap_or(""));
            push("completed", "success");
        }
        "result" | "error" => push(
            "error",
            v["error"]["message"]
                .as_str()
                .or_else(|| v["message"].as_str())
                .or_else(|| v["result"].as_str())
                .unwrap_or("Cursor turn failed"),
        ),
        _ => push(
            "provider-event",
            v["type"].as_str().unwrap_or("Unknown Cursor event"),
        ),
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn catalog_is_discovered_and_unknown_output_does_not_invent_models() {
        let rows = parse_models("Available models\ncomposer-2.5 - Composer 2.5\ngpt-6.1-sol - GPT 6.1 Sol\n../../bad model - Unsafe\n");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].id, "composer-2.5");
        assert!(parse_models("No models available for this account.").is_empty());
    }
    #[test]
    fn official_events_keep_session_work_and_terminal_outcomes_distinct() {
        let init = normalize(
            &json!({"type":"system","subtype":"init","session_id":"c6b62c6f-7ead-4fd6-9922-e952131177ff"}),
        );
        assert_eq!(init[0].kind, "session");
        assert!(normalize(&json!({"type":"user","session_id":"bad","message":{"content":[{"type":"text","text":"secret prompt"}]}})).is_empty());
        assert_eq!(normalize(&json!({"type":"tool_call","subtype":"started","call_id":"w1","tool_call":{"writeToolCall":{"args":{"fileText":"private"}}}}))[1].text,"writeToolCall");
        let result = normalize(
            &json!({"type":"result","subtype":"success","is_error":false,"result":"Done"}),
        );
        assert_eq!(result[0].kind, "final");
        assert_eq!(result[1].kind, "completed");
        assert_eq!(
            normalize(&json!({"type":"result","is_error":true,"result":"Quota exhausted"}))[0].kind,
            "error"
        );
        assert_eq!(
            normalize(&json!({"type":"future-work"}))[0].kind,
            "provider-event"
        );
    }
}
