//! Model/effort discovery through official local CLI metadata protocols; never starts a turn.
use crate::{models::*, providers};
use serde_json::{json, Value};
use std::path::Path;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

pub fn parse_models(provider: &Provider, rows: &Value) -> Vec<ModelInfo> {
    rows.as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| {
            if v["hidden"] == true {
                return None;
            }
            let id = if *provider == Provider::Gpt
                || matches!(provider, Provider::Gemini | Provider::Antigravity)
            {
                v["model"].as_str().or_else(|| v["id"].as_str())
            } else {
                v["value"].as_str()
            }?;
            let efforts = if *provider == Provider::Gpt {
                v["supportedReasoningEfforts"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|e| e["reasoningEffort"].as_str().map(str::to_owned))
                    .collect()
            } else {
                v["supportedEffortLevels"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|e| e.as_str().map(str::to_owned))
                    .collect()
            };
            Some(ModelInfo {
                id: id.into(),
                name: v["displayName"].as_str().unwrap_or(id).into(),
                description: v["description"].as_str().unwrap_or("").into(),
                efforts,
                default_effort: v["defaultReasoningEffort"].as_str().unwrap_or("").into(),
                is_default: v["isDefault"].as_bool().unwrap_or(id == "default"),
            })
        })
        .collect()
}

pub async fn discover(
    account: &Account,
    dir: &Path,
    cancel: Option<tokio::sync::watch::Receiver<bool>>,
) -> Result<Vec<ModelInfo>, String> {
    if matches!(account.provider, Provider::Gemini | Provider::Antigravity) {
        let mut c = providers::command(&account.provider)?;
        providers::isolate(&mut c, &account.provider, dir);
        let args: &[&str] = if account.provider == Provider::Antigravity {
            &["models"]
        } else {
            &["--list-models"]
        };
        let output =
            tokio::time::timeout(std::time::Duration::from_secs(30), c.args(args).output())
                .await
                .map_err(|_| "Google model list timed out".to_string())?
                .map_err(|e| e.to_string())?;
        if !output.status.success() {
            return Err(format!(
                "{} could not list models. Complete official CLI login and try again.",
                account.provider.binary()
            ));
        }
        let text = String::from_utf8_lossy(&output.stdout);
        let parsed: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
        let rows = if parsed.is_array() {
            parsed.as_array().cloned().unwrap_or_default()
        } else if parsed["models"].is_array() {
            parsed["models"].as_array().cloned().unwrap_or_default()
        } else {
            Vec::new()
        };
        let mut models = parse_models(&account.provider, &Value::Array(rows));
        if models.is_empty() {
            // agy prints "id<TAB>display name" rows after a progress line.
            let tabbed = text.lines().any(|l| l.contains('\t'));
            models = text
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty() && !line.starts_with('#'))
                .filter(|line| !tabbed || line.contains('\t'))
                .map(|line| ModelInfo {
                    id: line
                        .split_whitespace()
                        .next()
                        .unwrap_or(line)
                        .trim_matches('*')
                        .to_string(),
                    name: line.split('\t').nth(1).unwrap_or(line).trim().to_string(),
                    description: String::new(),
                    efforts: vec![],
                    default_effort: String::new(),
                    is_default: false,
                })
                .collect();
        }
        if models.is_empty() {
            return Err(
                "Google CLI returned no selectable models. Update the CLI or complete sign-in."
                    .into(),
            );
        }
        models[0].is_default = true;
        return Ok(models);
    }
    let mut c = providers::command(&account.provider)?;
    providers::isolate(&mut c, &account.provider, dir);
    c.current_dir(dir)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    if account.provider == Provider::Gpt {
        c.args(["app-server", "--listen", "stdio://"]);
    } else {
        c.args([
            "--print",
            "--verbose",
            "--input-format",
            "stream-json",
            "--output-format",
            "stream-json",
            "--strict-mcp-config",
            "--tools",
            "",
            "--setting-sources",
            "",
        ]);
    }
    let mut child = c
        .spawn()
        .map_err(|e| format!("Cannot load the official model catalog: {e}"))?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or("Model catalog stdin unavailable")?;
    let mut lines = BufReader::new(
        child
            .stdout
            .take()
            .ok_or("Model catalog stdout unavailable")?,
    )
    .lines();
    let stderr = child
        .stderr
        .take()
        .ok_or("Model catalog stderr unavailable")?;
    let diagnostics = tokio::spawn(async move {
        let mut l = BufReader::new(stderr).lines();
        while let Ok(Some(_)) = l.next_line().await {}
    });
    let query = async {
        let init = if account.provider == Provider::Gpt {
            json!({"id":1,"method":"initialize","params":{"clientInfo":{"name":"lumen","title":"Lumen","version":env!("CARGO_PKG_VERSION")}}})
        } else {
            json!({"type":"control_request","request_id":"lumen-models","request":{"subtype":"initialize"}})
        };
        stdin
            .write_all(format!("{init}\n").as_bytes())
            .await
            .map_err(|e| e.to_string())?;
        let mut models = vec![];
        let mut page_id = 2_i64;
        loop {
            let line=lines.next_line().await.map_err(|e|e.to_string())?.ok_or("The CLI closed its model catalog. Update the official CLI and check the account login.")?;
            let Ok(v) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            if account.provider == Provider::Claude {
                if v["type"] == "control_response" && v["response"]["request_id"] == "lumen-models"
                {
                    if v["response"]["subtype"] == "error" {
                        return Err(
                            "Claude rejected model discovery. Check CLI version and login.".into(),
                        );
                    }
                    return Ok(parse_models(
                        &account.provider,
                        &v["response"]["response"]["models"],
                    ));
                }
            } else if v["id"] == 1 {
                if v.get("error").is_some() {
                    return Err(
                        "Codex rejected model catalog initialization. Update the CLI.".into(),
                    );
                }
                stdin.write_all(format!("{}\n{}\n",json!({"method":"initialized"}),json!({"id":page_id,"method":"model/list","params":{"limit":100,"includeHidden":false}})).as_bytes()).await.map_err(|e|e.to_string())?;
            } else if v["id"] == page_id {
                if v.get("error").is_some() {
                    return Err(
                        "Codex could not list models. Check CLI version and official login.".into(),
                    );
                }
                models.extend(parse_models(&account.provider, &v["result"]["data"]));
                if let Some(cursor) = v["result"]["nextCursor"].as_str() {
                    page_id += 1;
                    if page_id > 102 {
                        return Err("The CLI returned too many model catalog pages".into());
                    }
                    let request = json!({"id":page_id,"method":"model/list","params":{"limit":100,"cursor":cursor,"includeHidden":false}});
                    stdin
                        .write_all(format!("{request}\n").as_bytes())
                        .await
                        .map_err(|e| e.to_string())?;
                } else {
                    return Ok(models);
                }
            }
        }
    };
    let interrupted = async move {
        if let Some(mut rx) = cancel {
            if *rx.borrow() {
                return;
            }
            let _ = rx.changed().await;
        } else {
            std::future::pending::<()>().await
        }
    };
    let result = tokio::select! {biased;_=interrupted=>Err("Cancelled".into()),result=tokio::time::timeout(std::time::Duration::from_secs(45),query)=>result.map_err(|_|"Official model catalog timed out. Check CLI installation and login.".to_string()).and_then(|r|r)};
    drop(stdin);
    providers::terminate(&mut child).await;
    diagnostics.abort();
    let models = result?;
    if models.is_empty() {
        return Err("The official CLI returned no selectable models. Sign in or update the CLI, then refresh.".into());
    }
    Ok(models)
}

pub fn validate(models: &[ModelInfo], model: Option<&str>, effort: &str) -> Result<(), String> {
    let selected=model.filter(|m|!m.is_empty()).map(|id|models.iter().find(|m|m.id==id).ok_or("The selected model is no longer in this account's official catalog. Refresh models.")).transpose()?;
    if !effort.is_empty() {
        let selected = selected
            .or_else(|| models.iter().find(|m| m.is_default))
            .or_else(|| models.first())
            .ok_or("No model available")?;
        if !selected.efforts.iter().any(|e| e == effort) {
            return Err("This reasoning level is not supported by the selected model. Choose an official level or Provider default.".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn official_efforts_are_preserved() {
        let m = parse_models(
            &Provider::Gpt,
            &json!([{"model":"gpt-fixture","displayName":"GPT fixture","supportedReasoningEfforts":[{"reasoningEffort":"low"},{"reasoningEffort":"ultra"}],"defaultReasoningEffort":"low","isDefault":true}]),
        );
        assert_eq!(m[0].efforts, vec!["low", "ultra"]);
        assert!(validate(&m, Some("gpt-fixture"), "high").is_err());
        assert!(validate(&m, Some("gpt-fixture"), "ultra").is_ok());
    }
    #[test]
    fn claude_does_not_invent_levels() {
        let m = parse_models(
            &Provider::Claude,
            &json!([{"value":"opus[1m]","displayName":"Opus","supportedEffortLevels":["medium","max"]},{"value":"legacy","displayName":"Legacy"}]),
        );
        assert_eq!(m[0].efforts, vec!["medium", "max"]);
        assert!(m[1].efforts.is_empty());
        assert!(validate(&m, Some("missing"), "").is_err());
    }
}
