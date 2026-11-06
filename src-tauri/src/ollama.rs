//! Ollama: a local provider reached only through the official `ollama` CLI.
//!
//! It never edits files (prompt in, text out), so it serves as the last
//! emergency fallback and as an Orchestra worker for trivially simple text
//! subtasks. Lumen picks the largest installed model unless one is pinned.
use crate::models::{ModelInfo, Provider};
use crate::providers;
use serde::Serialize;

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

pub async fn list() -> Result<Vec<LocalModel>, String> {
    let mut c = providers::command(&Provider::Ollama)?;
    c.arg("list").kill_on_drop(true);
    let out = tokio::time::timeout(std::time::Duration::from_secs(20), c.output())
        .await
        .map_err(|_| "Ollama did not answer. Open the Ollama app and try again.".to_string())?
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(
            "Ollama is installed but not running. Open the Ollama app and try again.".into(),
        );
    }
    Ok(parse_list(&String::from_utf8_lossy(&out.stdout)))
}

pub async fn status(pinned: &str) -> OllamaStatus {
    if providers::executable(&Provider::Ollama).is_err() {
        return OllamaStatus {
            installed: false,
            running: false,
            models: vec![],
            selected: None,
        };
    }
    match list().await {
        Ok(models) => OllamaStatus {
            installed: true,
            running: true,
            selected: choose(&models, pinned).map(|m| m.name.clone()),
            models,
        },
        Err(_) => OllamaStatus {
            installed: true,
            running: false,
            models: vec![],
            selected: None,
        },
    }
}

/// Catalog for routing: only the model Lumen will actually use.
pub async fn catalog(pinned: &str) -> Result<Vec<ModelInfo>, String> {
    let models = list().await?;
    let chosen = choose(&models, pinned)
        .ok_or("Ollama has no models yet. Run `ollama pull llama3.2` and try again.")?;
    Ok(vec![ModelInfo {
        id: chosen.name.clone(),
        name: chosen.name.clone(),
        description: format!("Local · {}", chosen.size),
        efforts: vec![],
        default_effort: String::new(),
        is_default: true,
    }])
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
