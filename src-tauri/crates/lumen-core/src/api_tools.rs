//! Host tools shared by direct APIs. Every call checks the immutable turn access profile.
use crate::{models::*, providers::Invocation};
use serde_json::{json, Value};
use std::path::{Component, Path, PathBuf};
use tokio::io::{AsyncRead, AsyncReadExt};

pub(crate) async fn dispatch(
    input: &Invocation<'_>,
    browser: &crate::browser::Registration,
    hub: &tokio::sync::Mutex<crate::shared_tools::Hub>,
    name: &str,
    args: &Value,
    emit: &mut impl FnMut(Activity),
) -> Value {
    if input.account.provider != Provider::Ollama && is_builtin(name) {
        call(input, name, args, emit).await
    } else if crate::browser::tools()
        .as_array()
        .unwrap()
        .iter()
        .any(|tool| tool["name"] == name)
    {
        browser.call(name, args).await
    } else {
        hub.lock()
            .await
            .call(name, args, input.access, browser)
            .await
    }
}
pub fn definitions(access: AccessMode) -> Vec<Value> {
    let tool = |name: &str, description: &str, properties: Value, required: Vec<&str>| json!({"name":name,"description":description,"inputSchema":{"type":"object","properties":properties,"required":required,"additionalProperties":false}});
    let mut tools = vec![
        tool(
            "lumen_read_file",
            "Read a UTF-8 project file, maximum 2 MB.",
            json!({"path":{"type":"string"}}),
            vec!["path"],
        ),
        tool(
            "lumen_list_files",
            "List project files, respecting ignore files, maximum 500.",
            json!({"path":{"type":"string"}}),
            vec!["path"],
        ),
        tool(
            "lumen_ask_user",
            "Ask the user a task clarification; this cannot grant permissions.",
            json!({"question":{"type":"string"}}),
            vec!["question"],
        ),
    ];
    if access != AccessMode::ReadOnly {
        tools.push(tool(
            "lumen_write_file",
            "Write a UTF-8 file inside the project; Git internals are protected.",
            json!({"path":{"type":"string"},"content":{"type":"string"}}),
            vec!["path", "content"],
        ))
    }
    if matches!(access, AccessMode::Autopilot | AccessMode::Ask) {
        tools.push(tool("lumen_run_command","Run a shell command in the project. Stop kills the owned process tree. Output/time are bounded.",json!({"command":{"type":"string"}}),vec!["command"]))
    }
    for mut t in crate::graph_mcp::tools().as_array().unwrap().clone() {
        t["name"] = json!(format!("lumen_graph_{}", t["name"].as_str().unwrap()));
        tools.push(t)
    }
    tools
}
pub fn is_builtin(name: &str) -> bool {
    [
        "lumen_read_file",
        "lumen_list_files",
        "lumen_write_file",
        "lumen_run_command",
        "lumen_ask_user",
    ]
    .contains(&name)
        || name.starts_with("lumen_graph_")
}
pub(crate) async fn permission(
    title: &str,
    detail: &str,
    emit: &mut impl FnMut(Activity),
) -> Result<bool, String> {
    use crate::interactions::{Connection, OptionItem, Question};
    let connection = Connection::default();
    let wire = crate::models::id();
    let (activity, receive) = connection.ask_permission(
        vec![Question {
            id: "action".into(),
            header: title.into(),
            question: detail.chars().take(8000).collect(),
            options: vec![
                OptionItem {
                    label: "Allow once".into(),
                    description: "Allow only this pending action.".into(),
                },
                OptionItem {
                    label: "Deny".into(),
                    description: "Do not execute this action.".into(),
                },
            ],
            multi_select: false,
            secret: false,
            optional: false,
        }],
        wire,
    )?;
    emit(activity);
    let answer = receive.await.map_err(|_| "Permission request expired")?;
    connection.drain(emit);
    Ok(answer
        .get("action")
        .is_some_and(|v| v.as_slice() == ["Allow once"]))
}
fn scoped(root: &Path, raw: &str) -> Result<PathBuf, String> {
    if raw.is_empty()
        || raw.len() > 4096
        || Path::new(raw)
            .components()
            .any(|x| matches!(x, Component::ParentDir))
    {
        return Err("Invalid project path".into());
    }
    #[cfg(windows)]
    if Path::new(raw)
        .components()
        .any(|c| matches!(c,Component::Normal(s) if s.to_string_lossy().ends_with(['.', ' '])))
    {
        return Err("Non-canonical Windows project path".into());
    }
    let root = dunce::canonicalize(root).map_err(|_| "Project unavailable")?;
    let target = if Path::new(raw).is_absolute() {
        PathBuf::from(raw)
    } else {
        root.join(raw)
    };
    if target.components().any(
        |x| matches!(x,Component::Normal(s) if s.to_string_lossy().eq_ignore_ascii_case(".git") || s.to_string_lossy().contains(':')),
    ) {
        return Err("Git internals are protected".into());
    }
    let mut existing = target.as_path();
    while !existing.exists() {
        existing = existing.parent().ok_or("Project path unavailable")?;
    }
    let existing = dunce::canonicalize(existing).map_err(|_| "Project path unavailable")?;
    if !existing.starts_with(&root) {
        return Err("Path leaves the project".into());
    }
    Ok(target)
}
async fn output(mut reader: impl AsyncRead + Unpin) -> Result<Vec<u8>, String> {
    let mut bytes = vec![];
    let mut chunk = [0u8; 8192];
    loop {
        let n = reader
            .read(&mut chunk)
            .await
            .map_err(|_| "Command output interrupted")?;
        if n == 0 {
            return Ok(bytes);
        }
        if bytes.len() + n > 256_000 {
            return Err("Command output exceeds 256 KB".into());
        }
        bytes.extend_from_slice(&chunk[..n]);
    }
}
async fn command(input: &Invocation<'_>, text: &str) -> Result<String, String> {
    if text.is_empty() || text.len() > 16000 {
        return Err("Invalid shell command".into());
    }
    let mut c = crate::proc::shell(text, input.workspace);
    crate::providers::isolate(&mut c, &Provider::Ollama, input.config);
    c.stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = crate::proc::ServiceChild::spawn(c).map_err(|_| "Cannot start command")?;
    let stdout = child.stdout.take().ok_or("Missing command output")?;
    let stderr = child.stderr.take().ok_or("Missing command diagnostics")?;
    let run = async {
        let (out, err, status) = tokio::try_join!(output(stdout), output(stderr), child.wait())?;
        let text = format!(
            "Exit: {}\n{}\n{}",
            status,
            String::from_utf8_lossy(&out),
            String::from_utf8_lossy(&err)
        );
        if status.success() {
            Ok(text)
        } else {
            Err(text)
        }
    };
    let result = tokio::time::timeout(std::time::Duration::from_secs(120), run)
        .await
        .map_err(|_| "Command exceeded 120 seconds".to_owned())
        .and_then(|x| x);
    child.finish(result.is_ok()).await?;
    result
}
pub async fn call(
    input: &Invocation<'_>,
    name: &str,
    args: &Value,
    emit: &mut impl FnMut(Activity),
) -> Value {
    let run = async {
        if name == "lumen_write_file" && input.access == AccessMode::ReadOnly {
            return Err("Read only denies file writes".into());
        }
        if name == "lumen_run_command"
            && !matches!(input.access, AccessMode::Autopilot | AccessMode::Ask)
        {
            return Err("Shell commands require Autopilot or per-action approval".into());
        }
        use crate::autonomy::{decision, Class};
        let class = crate::autonomy::tool(name, args);
        // Autopilot runs reversible work freely; an action a checkpoint cannot undo
        // always waits for one explicit answer, whatever the profile.
        let confirm =
            (input.access == AccessMode::Ask && name != "lumen_ask_user") || class == Class::Risky;
        if confirm {
            let title = match class {
                Class::Risky => "Irreversible action",
                Class::External => "External action (not undone by rollback)",
                Class::Reversible => name,
            };
            let allowed = permission(title, &args.to_string(), emit).await?;
            if class != Class::Reversible {
                emit(decision(
                    class,
                    if allowed { "confirmed" } else { "denied" },
                    &args.to_string(),
                ));
            }
            if !allowed {
                return Err("Action denied".into());
            }
        } else if class == Class::External {
            emit(decision(class, "ran", &args.to_string()));
        }
        let path = || {
            scoped(
                input.workspace,
                args["path"].as_str().ok_or("Missing path")?,
            )
        };
        match name {
            "lumen_read_file" => {
                let p = path()?;
                let size = std::fs::metadata(&p).map_err(|_| "File unavailable")?.len();
                if size > 2_000_000 {
                    return Err("File exceeds 2 MB".into());
                }
                std::fs::read_to_string(p).map_err(|_| "File is unavailable or not UTF-8".into())
            }
            "lumen_list_files" => {
                let p = path()?;
                let rows = ignore::WalkBuilder::new(&p)
                    .follow_links(false)
                    .build()
                    .filter_map(Result::ok)
                    .filter(|e| e.file_type().is_some_and(|t| t.is_file()))
                    .take(500)
                    .filter_map(|e| {
                        e.path()
                            .strip_prefix(input.workspace)
                            .ok()
                            .map(|p| p.to_string_lossy().into_owned())
                    })
                    .collect::<Vec<_>>();
                Ok(rows.join("\n"))
            }
            "lumen_write_file" => {
                let p = path()?;
                let text = args["content"].as_str().ok_or("Missing content")?;
                if text.len() > 2_000_000 {
                    return Err("Write exceeds 2 MB".into());
                }
                let parent = p.parent().ok_or("Missing parent")?;
                std::fs::create_dir_all(parent).map_err(|_| "Cannot create project directory")?;
                let _ = scoped(input.workspace, &p.to_string_lossy())?;
                let temp = parent.join(format!(".lumen-write-{}", crate::models::id()));
                std::fs::write(&temp, text).map_err(|_| "Cannot stage file")?;
                if let Ok(m) = std::fs::metadata(&p) {
                    std::fs::set_permissions(&temp, m.permissions())
                        .map_err(|_| "Cannot preserve file permissions")?;
                }
                if let Err(e) = std::fs::rename(&temp, &p) {
                    let _ = std::fs::remove_file(&temp);
                    return Err(format!("Cannot replace project file: {e}"));
                }
                Ok("File written".into())
            }
            "lumen_run_command" => {
                command(input, args["command"].as_str().ok_or("Missing command")?).await
            }
            "lumen_ask_user" => {
                use crate::interactions::{Connection, Question};
                let q = args["question"]
                    .as_str()
                    .filter(|s| s.len() <= 8000)
                    .ok_or("Invalid question")?;
                let connection = Connection::default();
                let (a, r) = connection.ask(
                    vec![Question {
                        id: "answer".into(),
                        header: "Task question".into(),
                        question: q.into(),
                        options: vec![],
                        multi_select: false,
                        secret: false,
                        optional: false,
                    }],
                    crate::models::id(),
                )?;
                emit(a);
                let answer = r.await.map_err(|_| "Question expired")?;
                connection.drain(emit);
                Ok(serde_json::to_string(&answer).unwrap_or_default())
            }
            n if n.starts_with("lumen_graph_") => crate::graph_mcp::call(
                crate::api::root(input.config)?,
                input.workspace,
                &n[12..],
                args,
            ),
            _ => Err("Unknown project tool".into()),
        }
    };
    match run.await {
        Ok(text) => json!({"content":[{"type":"text","text":text}]}),
        Err(error) => json!({"isError":true,"content":[{"type":"text","text":error}]}),
    }
}
/// Fixtures from T3 Code's top permission bugs, in both directions: no question for an
/// action the profile already allows, and no action that should have asked. Each row:
/// profile, tool, arguments, prompts expected, and whether the action runs once the
/// user allows every prompt.
#[cfg(test)]
pub(crate) fn permission_matrix() -> Vec<(AccessMode, &'static str, Value, usize, bool)> {
    let destroy = if cfg!(windows) {
        "rmdir /s /q build"
    } else {
        "rm -rf build"
    };
    let actions = [
        ("lumen_read_file", json!({"path": "a.txt"})),
        ("lumen_write_file", json!({"path": "b.txt", "content": "x"})),
        ("lumen_run_command", json!({"command": "echo ok"})),
        ("lumen_run_command", json!({"command": destroy})),
        // Not a repository, so the external command fails harmlessly after classification.
        ("lumen_run_command", json!({"command": "git push"})),
    ];
    // (asks, runs) per action, in the order above.
    let expected = [
        (
            AccessMode::ReadOnly,
            [(0, true), (0, false), (0, false), (0, false), (0, false)],
        ),
        (
            AccessMode::FileEdits,
            [(0, true), (0, true), (0, false), (0, false), (0, false)],
        ),
        (
            AccessMode::Ask,
            [(1, true), (1, true), (1, true), (1, true), (1, false)],
        ),
        (
            AccessMode::Autopilot,
            [(0, true), (0, true), (0, true), (1, true), (0, false)],
        ),
    ];
    expected
        .into_iter()
        .flat_map(|(access, outcomes)| {
            actions
                .clone()
                .into_iter()
                .zip(outcomes)
                .map(move |((tool, args), (asks, runs))| (access, tool, args, asks, runs))
        })
        .collect()
}

#[cfg(test)]
pub(crate) fn matrix_fixture(root: &std::path::Path) {
    std::fs::write(root.join("a.txt"), "a").unwrap();
    std::fs::create_dir_all(root.join("build")).unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn permission_matrix_has_no_superfluous_or_missing_prompts() {
        for (access, tool, args, asks, runs) in permission_matrix() {
            let f = crate::test_support::Fixture::new();
            let workspace = f.directory("project");
            let config = f.directory("accounts/test");
            matrix_fixture(&workspace);
            let account = Account {
                id: "test".into(),
                name: "fixture".into(),
                provider: Provider::Ollama,
            };
            let input = Invocation {
                rlm: None,
                automatic: false,
                account: &account,
                config: &config,
                workspace: &workspace,
                prompt: "",
                model: None,
                effort: "",
                access,
                approvals: None,
                resume: None,
                browser_scope: None,
                browser_artifacts: None,
                browser: None,
            };
            let mut asked = 0;
            let out = call(&input, tool, &args, &mut |a| {
                if a.kind == "interaction" && a.text.contains("\"permission\"") {
                    asked += 1;
                    let v: Value = serde_json::from_str(&a.text).unwrap();
                    crate::interactions::answer_question(
                        v["id"].as_str().unwrap().into(),
                        [("action".to_string(), vec!["Allow once".to_string()])].into(),
                    )
                    .unwrap();
                }
            })
            .await;
            let case = format!("{access:?} {tool} {args}: {out}");
            assert_eq!(asked, asks, "prompts for {case}");
            assert_eq!(out["isError"] != true, runs, "outcome for {case}");
        }
    }

    #[test]
    fn traversal_and_git_internals_are_rejected() {
        let f = crate::test_support::Fixture::new();
        assert!(scoped(&f.root, "../outside").is_err());
        assert!(scoped(&f.root, ".git/hooks/pre-commit").is_err());
        assert!(scoped(&f.root, "src/new.ts").is_ok());
        #[cfg(windows)]
        {
            assert!(scoped(&f.root, ".git./hooks/pre-commit").is_err());
            assert!(scoped(&f.root, "file.txt:stream").is_err());
        }
        assert!(!definitions(AccessMode::ReadOnly)
            .iter()
            .any(|t| t["name"] == "lumen_write_file"));
    }

    /// Autopilot still stops at an irreversible command: denying it runs nothing and the
    /// decision lands in the audit log; confirming it runs it once.
    #[tokio::test]
    async fn autopilot_confirms_destructive_commands() {
        let f = crate::test_support::Fixture::new();
        let workspace = f.directory("project");
        let config = f.directory("accounts/test");
        std::fs::create_dir_all(workspace.join("build")).unwrap();
        std::fs::write(workspace.join("build/out.txt"), "x").unwrap();
        let account = Account {
            id: "test".into(),
            name: "fixture".into(),
            provider: Provider::Ollama,
        };
        let input = Invocation {
            rlm: None,
            automatic: false,
            account: &account,
            config: &config,
            workspace: &workspace,
            prompt: "",
            model: None,
            effort: "",
            access: AccessMode::Autopilot,
            approvals: None,
            resume: None,
            browser_scope: None,
            browser_artifacts: None,
            browser: None,
        };
        let remove = if cfg!(windows) {
            "rmdir /s /q build"
        } else {
            "rm -rf build"
        };
        for (choice, ran) in [("Deny", false), ("Allow once", true)] {
            let mut events = vec![];
            let mut asked = 0;
            let out = call(
                &input,
                "lumen_run_command",
                &json!({"command": remove}),
                &mut |a| {
                    if a.kind == "interaction" && a.text.contains("\"permission\"") {
                        asked += 1;
                        let v: Value = serde_json::from_str(&a.text).unwrap();
                        let answers = [("action".to_string(), vec![choice.to_string()])].into();
                        crate::interactions::answer_question(
                            v["id"].as_str().unwrap().into(),
                            answers,
                        )
                        .unwrap();
                    }
                    events.push(a);
                },
            )
            .await;
            assert_eq!(asked, 1, "{choice}");
            assert_eq!(out["isError"] == true, !ran, "{choice}: {out}");
            assert_eq!(workspace.join("build").exists(), !ran, "{choice}");
            let log = crate::autonomy::audit(&[Message {
                id: "m".into(),
                role: "assistant".into(),
                text: String::new(),
                activity: events,
            }]);
            assert_eq!(log.len(), 1);
            assert_eq!(log[0].class, crate::autonomy::Class::Risky);
            assert_eq!(log[0].action, if ran { "confirmed" } else { "denied" });
        }
        // Reversible work in Autopilot never asks.
        let out = call(
            &input,
            "lumen_list_files",
            &json!({"path": "."}),
            &mut |a| assert!(!a.text.contains("\"permission\"")),
        )
        .await;
        assert!(out["isError"] != true);
    }

    /// Auto answers the agent's clarifying question itself and records the assumption,
    /// while a destructive command in the same loop still waits for the user.
    #[tokio::test]
    async fn auto_answers_questions_but_escalates_destructive_commands() {
        let f = crate::test_support::Fixture::new();
        let workspace = f.directory("project");
        let config = f.directory("accounts/test");
        std::fs::create_dir_all(workspace.join("build")).unwrap();
        let account = Account {
            id: "test".into(),
            name: "fixture".into(),
            provider: Provider::Ollama,
        };
        let input = Invocation {
            rlm: None,
            automatic: true,
            account: &account,
            config: &config,
            workspace: &workspace,
            prompt: "",
            model: None,
            effort: "",
            access: AccessMode::Autopilot,
            approvals: None,
            resume: None,
            browser_scope: None,
            browser_artifacts: None,
            browser: None,
        };
        crate::interactions::AUTO_ANSWER
            .scope(true, async {
                let mut events = vec![];
                let out = call(
                    &input,
                    "lumen_ask_user",
                    &json!({"question": "Which test runner should I use?"}),
                    &mut |a| events.push(a),
                )
                .await;
                assert!(out["isError"] != true, "{out}");
                assert!(out.to_string().contains("Lumen Auto"), "{out}");
                assert!(events.iter().all(|a| a.kind != "interaction"));
                let section = crate::interactions::assumptions_section(&events).unwrap();
                assert!(section.contains("Which test runner should I use?"));

                let remove = if cfg!(windows) {
                    "rmdir /s /q build"
                } else {
                    "rm -rf build"
                };
                let mut asked = 0;
                let out = call(
                    &input,
                    "lumen_run_command",
                    &json!({"command": remove}),
                    &mut |a| {
                        if a.kind == "interaction" && a.text.contains("\"permission\"") {
                            asked += 1;
                            let v: Value = serde_json::from_str(&a.text).unwrap();
                            crate::interactions::answer_question(
                                v["id"].as_str().unwrap().into(),
                                [("action".to_string(), vec!["Deny".to_string()])].into(),
                            )
                            .unwrap();
                        }
                    },
                )
                .await;
                assert_eq!(asked, 1);
                assert!(out["isError"] == true);
                assert!(workspace.join("build").exists());
            })
            .await;
    }
}
