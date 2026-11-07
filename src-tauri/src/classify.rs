//! Auto routing verdict from a small model instead of keywords. The local helper (Ollama) is
//! used when the user enabled it; otherwise the cheapest Claude model at low effort rates the
//! request. Any failure or timeout falls back to the keyword profile, so routing never blocks.
use crate::models::{Account, Message, Provider};
use crate::router::TaskProfile;
use serde_json::Value;
use std::path::Path;
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(15);

/// What the turn needs: model class, reasoning, an independent audit, and whether it is big
/// enough for Orchestra to split.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Verdict {
    pub task: TaskProfile,
    pub complex: bool,
    /// Where the verdict came from: "ollama", "claude" or "keywords".
    pub source: &'static str,
}

pub fn heuristic(prompt: &str) -> Verdict {
    Verdict {
        task: crate::router::profile(prompt),
        complex: crate::router::complex(prompt),
        source: "keywords",
    }
}

/// The previous exchange, shortened, so a follow-up like "procedi" is rated by its real task.
pub fn recent_context(messages: &[Message]) -> String {
    let mut parts: Vec<String> = messages
        .iter()
        .rev()
        .skip_while(|m| m.role != "assistant")
        .take(2)
        .map(|m| {
            let text: String = m.text.chars().take(1_200).collect();
            format!("{}: {}", m.role, text)
        })
        .collect();
    parts.reverse();
    parts.join("\n")
}

fn prompt(request: &str, context: &str) -> String {
    let request: String = request.chars().take(6_000).collect();
    let context = if context.trim().is_empty() {
        "(new conversation)".to_string()
    } else {
        context.to_string()
    };
    format!(
        r#"You route requests in an app that runs coding agents (Claude, Codex). Rate the user's request. Reply with only one JSON object and nothing else:
{{"tier":0,"effort":0,"review":false,"complex":false}}

tier: 0 = small talk, quick factual questions, short writing or translation, trivial one-line edits. 1 = ordinary coding, debugging, refactoring, explaining code, substantial writing. 2 = hard work: architecture, security, concurrency, subtle or intermittent bugs, large refactors or migrations, math, deep analysis.
effort: 0 = answer directly, 1 = needs some thought, 2 = needs long careful reasoning.
review: true only when a mistake would be costly (security, data loss, production systems) and an independent second check is worth paying for.
complex: true only when the work is large enough to split into several independent subtasks for parallel agents.
A short follow-up ("go on", "fix it", "procedi", "ok fallo") continues the previous task: rate that task, not the words.

Previous exchange:
{context}

Request:
{request}"#
    )
}

/// Reads the first JSON object in a model's answer.
pub fn parse(text: &str) -> Option<(TaskProfile, bool)> {
    let start = text.find('{')?;
    let end = text[start..].find('}')? + start;
    let v: Value = serde_json::from_str(&text[start..=end]).ok()?;
    let num = |k: &str| v[k].as_i64().map(|n| n.clamp(0, 2) as i32);
    Some((
        TaskProfile {
            tier: num("tier")?,
            effort: num("effort").unwrap_or(0),
            review: v["review"].as_bool().unwrap_or(false),
        },
        v["complex"].as_bool().unwrap_or(false),
    ))
}

/// Rates `request` with a small model. `accounts` and `data` locate a Claude login; `local`
/// is the enabled local helper model, if any.
pub async fn classify(
    data: &Path,
    accounts: &[Account],
    local: Option<(String, u64)>,
    request: &str,
    context: &str,
) -> Verdict {
    let fallback = heuristic(request);
    let ask = prompt(request, context);
    if let Some((model, timeout_ms)) = local.filter(|(m, _)| !m.is_empty()) {
        let wait = timeout_ms.min(TIMEOUT.as_millis() as u64);
        if let Ok(text) = crate::ollama::generate(&model, &ask, wait).await {
            if let Some((task, complex)) = parse(&text) {
                return Verdict {
                    task,
                    complex,
                    source: "ollama",
                };
            }
        }
    }
    let Some(account) = accounts.iter().find(|a| a.provider == Provider::Claude) else {
        return fallback;
    };
    let config = data.join("accounts").join(&account.id);
    match tokio::time::timeout(TIMEOUT, claude(&config, &ask)).await {
        Ok(Some(text)) => match parse(&text) {
            Some((task, complex)) => Verdict {
                task,
                complex,
                source: "claude",
            },
            None => fallback,
        },
        _ => fallback,
    }
}

/// One tool-less, unsaved Haiku answer at low effort.
async fn claude(config: &Path, ask: &str) -> Option<String> {
    let mut c = crate::providers::command(&Provider::Claude).ok()?;
    crate::providers::isolate(&mut c, &Provider::Claude, config);
    c.args([
        "--print",
        "--output-format",
        "text",
        "--model",
        "haiku",
        "--effort",
        "low",
        "--tools",
        "",
        "--strict-mcp-config",
        "--no-session-persistence",
        "--permission-mode",
        "dontAsk",
    ])
    .current_dir(std::env::temp_dir())
    .stdin(std::process::Stdio::piped())
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::null())
    .kill_on_drop(true);
    let mut child = c.spawn().ok()?;
    {
        use tokio::io::AsyncWriteExt;
        let mut stdin = child.stdin.take()?;
        stdin.write_all(ask.as_bytes()).await.ok()?;
    }
    let out = child.wait_with_output().await.ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_verdict_inside_chatter() {
        let (t, complex) = parse(
            "Sure:\n```json\n{\"tier\":2,\"effort\":2,\"review\":true,\"complex\":true}\n```",
        )
        .unwrap();
        assert_eq!((t.tier, t.effort, t.review, complex), (2, 2, true, true));
        assert!(parse("no json here").is_none());
        assert_eq!(parse("{\"tier\":9}").unwrap().0.tier, 2);
    }

    #[test]
    fn context_is_the_previous_exchange() {
        let m = |role: &str, text: &str| Message {
            id: text.into(),
            role: role.into(),
            text: text.into(),
            activity: vec![],
        };
        let chat = vec![
            m("user", "old"),
            m("assistant", "old reply"),
            m("user", "fix the race"),
            m("assistant", "plan"),
            m("user", "procedi"),
        ];
        assert_eq!(recent_context(&chat), "user: fix the race\nassistant: plan");
    }
}
