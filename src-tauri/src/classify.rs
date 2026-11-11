//! Auto routing verdict from a small model instead of keywords. The local helper (Ollama) is
//! used when the user enabled it; otherwise the cheapest Claude model at low effort rates the
//! request. Any failure or timeout falls back to the keyword profile, so routing never blocks.
//! Recognized short continuations ("procedi", "fix it") reuse the latest request's verdict.
//! Unrelated short tasks and newer unrated manual requests do not inherit an old route.
use crate::models::{Account, Activity, Message, Provider};
use crate::router::TaskProfile;
use serde_json::Value;
use std::path::Path;
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(12);
/// Requests up to this many characters on one line may be follow-ups.
const FOLLOW_UP_CHARS: usize = 80;

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

/// The verdict stored on a request, so the next turn can reuse it.
pub fn record(v: &Verdict) -> Activity {
    Activity {
        kind: "route".into(),
        text: serde_json::json!({
            "tier": v.task.tier,
            "effort": v.task.effort,
            "review": v.task.review,
            "complex": v.complex,
            "source": v.source,
            "audit": v.task.audit,
        })
        .to_string(),
    }
}

/// The verdict of the latest earlier request in this chat.
pub fn previous(messages: &[Message]) -> Option<Verdict> {
    let a = messages
        .iter()
        .rev()
        .find(|m| m.role == "user")?
        .activity
        .iter()
        .find(|a| a.kind == "route")?;
    let (task, complex) = parse(&a.text)?;
    Some(Verdict {
        task,
        complex,
        source: "previous request",
    })
}

/// A short one-line message after an earlier rated request continues that task. It keeps the
/// earlier verdict, raised if its own words ask for more.
pub fn follow_up(previous: Option<Verdict>, request: &str) -> Option<Verdict> {
    let prev = previous?;
    let text = request.trim();
    if text.chars().count() > FOLLOW_UP_CHARS || text.contains('\n') || text.contains("```") {
        return None;
    }
    let command = text.trim_end_matches(['.', '!']).to_lowercase();
    let continuation = [
        "procedi",
        "continua",
        "proceed",
        "continue",
        "go on",
        "go ahead",
        "ok fallo",
        "fallo",
        "fix it",
        "correggilo",
        "implement the approved plan",
        "applica il piano",
        "procedi con il piano approvato",
        "procedi secondo il piano approvato",
    ]
    .contains(&command.as_str())
        || [
            "procedi con il punto ",
            "continua dal punto ",
            "continue with step ",
        ]
        .iter()
        .any(|s| command.starts_with(s));
    if !continuation {
        return None;
    }
    let own = crate::router::profile(text);
    Some(Verdict {
        task: TaskProfile {
            tier: prev.task.tier.max(own.tier),
            effort: prev.task.effort.max(own.effort),
            review: prev.task.review || own.review,
            audit: if own.audit == crate::router::AuditIntent::Auto {
                prev.task.audit
            } else {
                own.audit
            },
        },
        complex: prev.complex,
        source: "previous request",
    })
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
review: true only when a mistake would be costly (security, data loss, money, production systems) and an independent second check is worth paying for. Most requests are false.
complex: true only when the request asks for several separate deliverables, or one large feature spanning many files or areas, that independent agents could build in parallel. A single bug, question, review, explanation or change to one area is never complex, however hard. Most requests are false.
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
            audit: v
                .get("audit")
                .cloned()
                .and_then(|v| serde_json::from_value(v).ok())
                .unwrap_or_default(),
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
            if let Some((mut task, complex)) = parse(&text) {
                task.audit = crate::router::audit_intent(request);
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
            Some((mut task, complex)) => {
                task.audit = crate::router::audit_intent(request);
                Verdict {
                    task,
                    complex,
                    source: "claude",
                }
            }
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
    fn quality_unrelated_short_requests_do_not_inherit_old_complexity() {
        let prev = Verdict {
            task: TaskProfile {
                tier: 2,
                effort: 2,
                review: true,
                audit: Default::default(),
            },
            complex: true,
            source: "claude",
        };
        for request in [
            "Nuovo obiettivo: traduci ciao",
            "Traduci questa email",
            "Correggi il colore CSS",
            "What does this variable mean?",
        ] {
            assert!(follow_up(Some(prev), request).is_none(), "{request}");
        }
        assert!(follow_up(Some(prev), "procedi").is_some());
    }
    #[test]
    fn quality_previous_does_not_skip_a_newer_unrated_manual_task() {
        let verdict = heuristic("architecture security production");
        let chat = vec![
            Message {
                id: "old".into(),
                role: "user".into(),
                text: "old".into(),
                activity: vec![record(&verdict)],
            },
            Message {
                id: "new".into(),
                role: "user".into(),
                text: "New task: translate this".into(),
                activity: vec![],
            },
        ];
        assert!(previous(&chat).is_none());
    }
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
    fn short_follow_ups_reuse_the_previous_verdict() {
        let hard = Verdict {
            task: TaskProfile {
                tier: 2,
                effort: 2,
                review: false,
                audit: Default::default(),
            },
            complex: false,
            source: "claude",
        };
        let chat = vec![
            Message {
                id: "u1".into(),
                role: "user".into(),
                text: "fix the race".into(),
                activity: vec![record(&hard)],
            },
            Message {
                id: "a1".into(),
                role: "assistant".into(),
                text: "plan".into(),
                activity: vec![],
            },
        ];
        let prev = previous(&chat).unwrap();
        assert_eq!((prev.task.tier, prev.task.effort), (2, 2));
        let v = follow_up(Some(prev), "procedi").unwrap();
        assert_eq!((v.task.tier, v.source), (2, "previous request"));
        assert!(follow_up(Some(prev), &"x".repeat(81)).is_none());
        assert!(follow_up(Some(prev), "line one\nline two").is_none());
        assert!(follow_up(None, "procedi").is_none());
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
