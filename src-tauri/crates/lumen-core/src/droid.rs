//! Official Factory Droid headless CLI (`droid exec`), using the user's existing native profile.
use crate::{models::*, providers};
use serde_json::Value;
use tokio::sync::watch;

/// `droid exec` has no model listing command. Lumen offers the profile default plus the models
/// the user chose in Droid's own settings (default model and favorites), never an invented
/// catalog; Droid validates the model itself.
pub const DEFAULT_MODEL: &str = "default";

pub fn models() -> Vec<ModelInfo> {
    let settings = dirs::home_dir()
        .and_then(|h| std::fs::read_to_string(h.join(".factory").join("settings.json")).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or(Value::Null);
    from_settings(&settings)
}

fn from_settings(settings: &Value) -> Vec<ModelInfo> {
    let configured = settings["sessionDefaultSettings"]["model"].as_str();
    let mut rows = vec![ModelInfo {
        resolved_id: None,
        id: DEFAULT_MODEL.into(),
        name: match configured {
            Some(m) if valid(m) => format!("Droid default ({m})"),
            _ => "Droid default".into(),
        },
        description: "Model configured in the Droid profile".into(),
        efforts: vec![],
        default_effort: String::new(),
        is_default: true,
    }];
    let favorites = settings["modelFavorites"].as_array().into_iter().flatten();
    for id in configured
        .into_iter()
        .chain(favorites.filter_map(Value::as_str))
    {
        if valid(id) && !rows.iter().any(|m| m.id == id) {
            rows.push(ModelInfo {
                resolved_id: None,
                id: id.into(),
                name: id.into(),
                description: "From your Droid settings".into(),
                efforts: vec![],
                default_effort: String::new(),
                is_default: false,
            });
        }
    }
    rows
}

fn valid(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 100
        && id != DEFAULT_MODEL
        && !id.starts_with('-')
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-._:/[]".contains(c))
}

/// Signed in through `/login` (stored by Droid under `~/.factory`). `FACTORY_API_KEY` is stripped
/// like other inherited keys, so the billing identity never changes silently. File presence only:
/// an expired token still shows as connected until a turn fails.
pub fn signed_in() -> bool {
    dirs::home_dir()
        .and_then(|h| std::fs::read_dir(h.join(".factory")).ok())
        .is_some_and(|mut d| {
            d.any(|e| e.is_ok_and(|e| e.file_name().to_string_lossy().starts_with("auth")))
        })
}

/// Lumen access profile → `droid exec` autonomy. Droid stops with a non-zero exit at any action
/// above the level, so restricted profiles are enforced by the official CLI.
fn autonomy(access: AccessMode) -> Result<Option<&'static str>, String> {
    match access {
        AccessMode::ReadOnly => Ok(None),
        AccessMode::FileEdits => Ok(Some("low")),
        AccessMode::Autopilot => Ok(Some("high")),
        AccessMode::Ask => Err("Droid does not support per-action approvals in headless mode. Choose Read only, File edits or Autopilot; no request was sent.".into()),
    }
}

pub async fn execute(
    input: providers::Invocation<'_>,
    cancel: watch::Receiver<bool>,
    emit: impl FnMut(Activity),
) -> Result<String, String> {
    let level = autonomy(input.access)?;
    if input
        .resume
        .is_some_and(|s| uuid::Uuid::parse_str(s).is_err())
    {
        return Err("Stored Droid session id is invalid; no request was sent.".into());
    }
    let workspace =
        dunce::canonicalize(input.workspace).map_err(|_| "Conversation folder unavailable")?;
    let mut command = providers::command(&Provider::Droid)?;
    // The prompt goes through a Lumen-owned file: it stays out of the process list and argv limits.
    // Files left by a crashed run are removed first; the guard removes this one on every exit.
    std::fs::create_dir_all(input.config).map_err(|e| e.to_string())?;
    for stale in std::fs::read_dir(input.config)
        .into_iter()
        .flatten()
        .flatten()
    {
        let name = stale.file_name();
        let name = name.to_string_lossy();
        // Only files older than a day: a concurrent turn's prompt must not be removed.
        let old = stale
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|age| age.as_secs() > 86_400);
        if old && name.starts_with("prompt-") && name.ends_with(".txt") {
            let _ = std::fs::remove_file(stale.path());
        }
    }
    let prompt_file = PromptFile(
        input
            .config
            .join(format!("prompt-{}.txt", uuid::Uuid::new_v4())),
    );
    write_private(&prompt_file.0, input.prompt).map_err(|e| e.to_string())?;
    providers::isolate(&mut command, &Provider::Droid, input.config);
    command
        .current_dir(&workspace)
        .args(["exec", "--output-format", "stream-json", "--cwd"])
        .arg(&workspace)
        .arg("--file")
        .arg(&prompt_file.0);
    if let Some(level) = level {
        command.args(["--auto", level]);
    }
    if let Some(model) = input.model.filter(|m| *m != DEFAULT_MODEL) {
        command.args(["--model", model]);
    }
    if let Some(session) = input.resume {
        command.args(["--session-id", session]);
    }
    providers::run_owned_json(command, "", &Provider::Droid, cancel, emit).await
}

struct PromptFile(std::path::PathBuf);
impl Drop for PromptFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn write_private(path: &std::path::Path, text: &str) -> std::io::Result<()> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(path)?.write_all(text.as_bytes())
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
        "system" => {}
        // The user's own prompt is echoed back; never re-render it as assistant output.
        "message" if v["role"] == "assistant" => push("text", v["text"].as_str().unwrap_or("")),
        "message" => {}
        "tool_call" => {
            if let Some(id) = v["id"].as_str() {
                push("tool-start", &serde_json::json!({"id":id}).to_string());
            }
            push("tool", v["toolName"].as_str().unwrap_or("Droid tool"));
        }
        "tool_result" => {
            if let Some(id) = v["id"].as_str() {
                push("tool-end", &serde_json::json!({"id":id}).to_string());
            }
        }
        "completion" => {
            push("final", v["finalText"].as_str().unwrap_or(""));
            push("completed", "success");
        }
        "error" => push(
            "error",
            v["message"]
                .as_str()
                .or_else(|| v["error"]["message"].as_str())
                .or_else(|| v["error"].as_str())
                .unwrap_or("Droid turn failed"),
        ),
        other => push(
            "provider-event",
            if other.is_empty() {
                "Unknown Droid event"
            } else {
                other
            },
        ),
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Recorded from `droid exec -o stream-json` 0.233.0 (paths and tool output trimmed).
    const FIXTURE: &str = r#"{"type":"system","subtype":"init","cwd":"C:\\w","session_id":"e06ce7a6-45ce-492b-8935-db36c26035ab","tools":["Read","LS"]}
{"type":"message","role":"user","id":"e53634a9-ee9f-4a53-a42c-89ab9a95c2f8","text":"secret prompt","timestamp":1795620102826,"session_id":"e06ce7a6-45ce-492b-8935-db36c26035ab"}
{"type":"tool_call","id":"call_969bdd005e5449fbad9a9a25ac1fef12","messageId":"10b279ed-42a5-47dc-81fd-afce3edbc0a9","toolId":"LS","toolName":"LS","parameters":{"directory_path":"C:\\w"}}
{"type":"tool_result","id":"call_969bdd005e5449fbad9a9a25ac1fef12","messageId":"510b6fb2-02c1-4d56-a75e-a0c3d8ff541b","toolId":"LS","isError":false,"value":"a.txt"}
{"type":"message","role":"assistant","id":"4211b633-6be9-4d5f-bcbe-055afeef0c6a","text":"done","timestamp":1795620122811,"session_id":"e06ce7a6-45ce-492b-8935-db36c26035ab"}
{"type":"completion","finalText":"done","numTurns":2,"durationMs":22177,"session_id":"e06ce7a6-45ce-492b-8935-db36c26035ab","timestamp":1795620122921,"usage":{"input_tokens":14431,"output_tokens":80}}"#;

    fn all() -> Vec<Activity> {
        FIXTURE
            .lines()
            .flat_map(|l| normalize(&serde_json::from_str(l).unwrap()))
            .collect()
    }

    #[test]
    fn recorded_stream_maps_session_tools_text_and_completion() {
        let rows = all();
        let kinds: Vec<_> = rows
            .iter()
            .filter(|a| a.kind != "session")
            .map(|a| a.kind.as_str())
            .collect();
        assert_eq!(
            kinds,
            [
                "tool-start",
                "tool",
                "tool-end",
                "text",
                "final",
                "completed"
            ]
        );
        assert!(rows
            .iter()
            .any(|a| a.kind == "session" && a.text == "e06ce7a6-45ce-492b-8935-db36c26035ab"));
        assert!(rows.iter().all(|a| !a.text.contains("secret prompt")));
    }

    #[test]
    fn errors_and_unknown_events_are_not_success() {
        let err = normalize(&serde_json::json!({"type":"error","message":"Blocked by hook"}));
        assert_eq!(
            (err[0].kind.as_str(), err[0].text.as_str()),
            ("error", "Blocked by hook")
        );
        assert_eq!(
            normalize(&serde_json::json!({"type":"error"}))[0].text,
            "Droid turn failed"
        );
        assert_eq!(
            normalize(&serde_json::json!({"type":"x"}))[0].kind,
            "provider-event"
        );
        assert!(normalize(&serde_json::json!({"session_id":"../bad","type":"system"})).is_empty());
    }

    #[test]
    fn models_come_from_droid_settings_only() {
        let rows = from_settings(&serde_json::json!({
            "sessionDefaultSettings": {"model": "gpt-6.1-sol"},
            "modelFavorites": ["claude-opus-5-5", "gpt-6.1-sol", "--bad", "a b"]
        }));
        let ids: Vec<_> = rows.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, ["default", "gpt-6.1-sol", "claude-opus-5-5"]);
        assert_eq!(rows[0].name, "Droid default (gpt-6.1-sol)");
        assert_eq!(from_settings(&Value::Null).len(), 1);
    }

    #[test]
    fn access_profiles_map_to_official_autonomy_levels() {
        assert_eq!(autonomy(AccessMode::ReadOnly).unwrap(), None);
        assert_eq!(autonomy(AccessMode::FileEdits).unwrap(), Some("low"));
        assert_eq!(autonomy(AccessMode::Autopilot).unwrap(), Some("high"));
        assert!(autonomy(AccessMode::Ask).is_err());
    }
}
