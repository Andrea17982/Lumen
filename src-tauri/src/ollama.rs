//! Ollama uses its official loopback API for discovery, helpers and agent turns.
//! `ollama_agent` adds Moli tool execution without remote services or API keys.
use crate::models::{ModelInfo, Provider};
use crate::providers;
use serde::Serialize;
use serde_json::{json, Value};

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LocalModel {
    pub name: String,
    pub size: String,
    pub bytes: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OllamaStatus {
    pub installed: bool,
    pub running: bool,
    pub models: Vec<LocalModel>,
    pub selected: Option<String>,
}

#[cfg(test)]
fn size_bytes(number: &str, unit: &str) -> u64 {
    let n: f64 = number.parse().unwrap_or(0.0);
    let scale = match unit.to_ascii_uppercase().as_str() {
        "KB" => 1e3,
        "MB" => 1e6,
        "GB" => 1e9,
        "TB" => 1e12,
        _ => 1.0,
    };
    (n * scale) as u64
}

/// Parse `ollama list`: `NAME  ID  SIZE  MODIFIED`, size as "4.7 GB".
#[cfg(test)]
pub fn parse_list(text: &str) -> Vec<LocalModel> {
    text.lines()
        .skip_while(|l| !l.trim_start().starts_with("NAME"))
        .skip(1)
        .filter_map(|line| {
            let cols: Vec<&str> = line.split_whitespace().collect();
            if cols.len() < 4 {
                return None;
            }
            Some(LocalModel {
                name: cols[0].into(),
                size: format!("{} {}", cols[2], cols[3]),
                bytes: size_bytes(cols[2], cols[3]),
            })
        })
        .collect()
}

/// The pinned model when installed, otherwise the largest one.
pub fn choose<'a>(models: &'a [LocalModel], pinned: &str) -> Option<&'a LocalModel> {
    models
        .iter()
        .find(|m| !pinned.is_empty() && m.name == pinned)
        .or_else(|| models.iter().max_by_key(|m| m.bytes))
}

fn local_http() -> Result<(reqwest::Client, reqwest::Url), String> {
    let address = std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://127.0.0.1:11434".into());
    let endpoint = crate::ollama_agent::endpoint(&address)?;
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| e.to_string())?;
    Ok((client, endpoint))
}
fn models_from_tags(value: &Value) -> Result<Vec<LocalModel>, String> {
    let rows = value["models"]
        .as_array()
        .ok_or("Ollama returned an invalid model list")?;
    Ok(rows
        .iter()
        .filter_map(|row| {
            let name = row["name"].as_str()?;
            if !valid_model(name) || crate::ollama_agent::local_model(name, row).is_err() {
                return None;
            }
            if row["capabilities"]
                .as_array()
                .is_some_and(|caps| !caps.iter().any(|c| c == "completion"))
            {
                return None;
            }
            let bytes = row["size"].as_u64().unwrap_or(0);
            Some(LocalModel {
                name: name.into(),
                size: format!("{:.1} GB", bytes as f64 / 1e9),
                bytes,
            })
        })
        .collect())
}
pub async fn list() -> Result<Vec<LocalModel>, String> {
    let (http, base) = local_http()?;
    let value = http
        .get(base.join("api/tags").map_err(|e| e.to_string())?)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
        .map_err(|_| "Open the Ollama app to start its local API")?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json::<Value>()
        .await
        .map_err(|e| e.to_string())?;
    models_from_tags(&value)
}

pub async fn status(pinned: &str) -> OllamaStatus {
    match list().await {
        Ok(models) => OllamaStatus {
            installed: true,
            running: true,
            selected: choose(&models, pinned).map(|m| m.name.clone()),
            models,
        },
        Err(_) => OllamaStatus {
            installed: providers::executable(&Provider::Ollama).is_ok(),
            running: false,
            models: vec![],
            selected: None,
        },
    }
}

/// Every installed conversational model is selectable; the pin/largest model is the default.
pub async fn catalog(pinned: &str) -> Result<Vec<ModelInfo>, String> {
    let models = list().await?;
    let chosen = choose(&models, pinned)
        .ok_or("Ollama has no local models yet. Install one in Ollama and try again.")?;
    Ok(models
        .iter()
        .map(|model| ModelInfo {
            id: model.name.clone(),
            name: model.name.clone(),
            description: format!("Local · {}", model.size),
            efforts: vec![],
            default_effort: String::new(),
            is_default: model.name == chosen.name,
        })
        .collect())
}

/// Model names are passed as CLI arguments, so anything that could read as a flag is refused.
pub fn valid_model(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 120
        && !name.starts_with('-')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "._:/-".contains(c))
}

/// Drops terminal control sequences (cursor moves, colors) from CLI output.
#[cfg(test)]
pub fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            if chars.as_str().starts_with('[') {
                chars.next();
                for d in chars.by_ref() {
                    if d.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// A short local text helper. HTTP bodies keep prompts out of process arguments; never pulls.
pub async fn generate(model: &str, prompt: &str, timeout_ms: u64) -> Result<String, String> {
    if !valid_model(model) {
        return Err("unavailable: Choose a valid local model in Settings.".into());
    }
    let installed = list().await.map_err(|e| format!("unavailable: {e}"))?;
    if !installed
        .iter()
        .any(|m| m.name == model || m.name == format!("{model}:latest"))
    {
        return Err("model-missing: The selected model is not installed.".into());
    }
    let (http, base) = local_http()?;
    let run = async {
        let show = http
            .post(base.join("api/show").unwrap())
            .json(&json!({"model":model}))
            .send()
            .await
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?
            .json::<Value>()
            .await
            .map_err(|e| e.to_string())?;
        crate::ollama_agent::local_model(model, &show)?;
        let value = http.post(base.join("api/chat").unwrap()).json(&json!({"model":model,"stream":false,"messages":[{"role":"user","content":prompt}]}))
            .send().await.map_err(|e|e.to_string())?.error_for_status().map_err(|e|e.to_string())?.json::<Value>().await.map_err(|e|e.to_string())?;
        crate::ollama_agent::local_model(model, &value)?;
        value["message"]["content"]
            .as_str()
            .map(|s| s.trim().to_owned())
            .ok_or_else(|| "Ollama returned no text".to_string())
    };
    tokio::time::timeout(std::time::Duration::from_millis(timeout_ms), run)
        .await
        .map_err(|_| "unavailable: Ollama took too long.".to_string())?
        .map_err(|e| format!("unavailable: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_catalog_excludes_cloud_aliases_and_embedding_only_models() {
        let rows = models_from_tags(&json!({"models":[
            {"name":"local:8b","size":42,"capabilities":["completion","tools"]},
            {"name":"alias:latest","remote_host":"https://ollama.com","size":0},
            {"name":"large:cloud","size":0},
            {"name":"embedding:latest","capabilities":["embedding"],"size":10}
        ]}))
        .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "local:8b");
    }
    const LIST: &str = "NAME               ID              SIZE      MODIFIED\n\
llama3.2:latest    a80c4f17acd5    2.0 GB    3 weeks ago\n\
qwen2.5:7b         845dbda0ea48    4.7 GB    2 days ago\n\
tinyllama:latest   2644915ede35    637 MB    5 months ago\n";
    #[test]
    fn parses_list_and_picks_largest_or_pinned() {
        let models = parse_list(LIST);
        assert_eq!(models.len(), 3);
        assert_eq!(models[2].size, "637 MB");
        assert_eq!(choose(&models, "").unwrap().name, "qwen2.5:7b");
        assert_eq!(
            choose(&models, "tinyllama:latest").unwrap().name,
            "tinyllama:latest"
        );
        assert_eq!(choose(&models, "missing").unwrap().name, "qwen2.5:7b");
        assert!(parse_list("NAME    ID    SIZE    MODIFIED \n").is_empty());
    }
    #[test]
    fn refuses_flag_like_and_odd_model_names() {
        assert!(valid_model("qwen2.5:7b"));
        assert!(valid_model("library/llama3.2"));
        assert!(!valid_model("--insecure"));
        assert!(!valid_model("llama 3; rm"));
        assert!(!valid_model(""));
    }
    /// Needs a running Ollama with an installed model: `cargo test ollama -- --ignored`.
    #[tokio::test]
    #[ignore]
    async fn answers_from_the_local_daemon_and_never_pulls() {
        let models = list().await.expect("Ollama is running");
        let model = models.first().expect("one model installed").name.clone();
        let answer = generate(&model, "Reply with the single word: pong", 120_000)
            .await
            .expect("local answer");
        assert!(!answer.is_empty());
        let missing = generate("lumen-test-missing-model", "hi", 5_000).await;
        assert!(missing.unwrap_err().starts_with("model-missing:"));
    }
    #[test]
    fn strips_terminal_sequences() {
        assert_eq!(strip_ansi("\u{1b}[?25lHello\u{1b}[0m world"), "Hello world");
    }
}
