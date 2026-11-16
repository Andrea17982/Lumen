//! OpenCode v2 headless adapter. The existing native profile owns credentials and configuration.
use crate::models::*;

pub async fn models(
    workspace: Option<&std::path::Path>,
    cancel: Option<tokio::sync::watch::Receiver<bool>>,
) -> Result<Vec<ModelInfo>, String> {
    let mut command = crate::providers::command(&Provider::Opencode)?;
    crate::providers::isolate(&mut command, &Provider::Opencode, std::path::Path::new(""));
    if let Some(workspace) = workspace {
        command.current_dir(workspace);
    }
    command
        .args(["models", "--standalone"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());
    let text = crate::catalog::cli_output(command, cancel).await?;
    parse_models(&text)
}
fn parse_models(text: &str) -> Result<Vec<ModelInfo>, String> {
    let rows = text
        .lines()
        .map(str::trim)
        .filter(|s| {
            s.contains('/')
                && s.len() <= 100
                && s.chars()
                    .all(|c| c.is_ascii_alphanumeric() || "-._:/[]".contains(c))
        })
        .map(|id| ModelInfo {
            id: id.into(),
            name: id.into(),
            description: "OpenCode native profile · session-scoped permissions".into(),
            efforts: vec![],
            default_effort: String::new(),
            is_default: false,
        })
        .collect::<Vec<_>>();
    if rows.is_empty() {
        return Err("OpenCode returned no supported model identifiers".into());
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn model_catalog_requires_actual_identifiers() {
        let rows = parse_models("Progress...\nopenai/gpt-6.1-sol\nlocal/fixture-model\n").unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].id, "openai/gpt-6.1-sol");
        assert!(rows.iter().all(|r| r.efforts.is_empty()));
        assert!(parse_models("sign in required").is_err());
    }
}
