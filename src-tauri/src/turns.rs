//! Turn orchestration: single, writer + cross-audit, parallel perspectives and
//! side-by-side broadcast, with rate-limit fallback, checkpoints and self-healing.
use crate::{
    account, config_dir, context, event, gitops, integrations, limits, models::*, models_for,
    providers, resolve_effort, router, Core,
};
use std::{
    future::Future,
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::sync::watch;

pub(crate) fn note(app: &tauri::AppHandle, s: &str, out: &mut Message, kind: &str, text: String) {
    event(app, s, &out.id, kind, &text);
    out.activity.push(Activity {
        kind: kind.into(),
        text,
    });
}

#[tauri::command]
pub async fn send_message(
    app: tauri::AppHandle,
    core: tauri::State<'_, Arc<Core>>,
    id: String,
    text: String,
    options: RunOptions,
) -> Result<Plan, String> {
    if text.trim().is_empty() || text.len() > 100_000 {
        return Err("Enter a message of up to 100,000 bytes".into());
    }
    let mut session = core.store.session(&id)?;
    let accounts = core.store.accounts()?;
    let mut plan = router::route(&text, &options, &accounts)?;
    if options.mode == "auto" {
        crate::balance(&core, &options, &accounts, &mut plan).await?;
        // Skip an account that is cooling down after a rate limit.
        if core.limits.cooling(&plan.account_id, now()) {
            if let Some(b) = accounts.iter().find(|b| {
                b.id != plan.account_id
                    && matches!(b.provider, Provider::Claude | Provider::Gpt)
                    && !core.limits.cooling(&b.id, now())
            }) {
                plan.provider = b.provider.clone();
                plan.account_id = b.id.clone();
                plan.strategy = "single".into();
                plan.partner_account = None;
                plan.reason = "Preferred account is cooling down after a rate limit".into();
            }
        }
        let a = account(&core, &plan.account_id)?;
        let models = models_for(&core, &a, false, None).await?;
        let selected = router::select_model(&models, &text, &options.auto_policy)?;
        plan.model = Some(selected.id.clone());
        plan.effort = resolve_effort(selected, &plan.effort);
    } else if options.mode != "both" {
        let a = account(&core, &plan.account_id)?;
        let models = models_for(&core, &a, false, None).await?;
        crate::catalog::validate(&models, plan.model.as_deref(), &plan.effort)?;
    }
    for f in &options.attachments {
        context::attachment_path(Path::new(&session.workspace), f)?;
    }
    // Orchestra: complex Auto work is planned and split across agents.
    let mut orchestra_nogit = false;
    if options.mode == "auto"
        && options.audit_of.is_none()
        && plan.strategy != "broadcast"
        && core.settings.lock().map(|s| s.orchestra).unwrap_or(false)
        && router::complex(&text)
    {
        if options.access() != AccessMode::ReadOnly
            && gitops::toplevel(Path::new(&session.workspace))
                .await
                .is_none()
        {
            orchestra_nogit = true;
        } else {
            plan.strategy = "orchestra".into();
            plan.reason = "Complex task: Orchestra plans, splits and integrates the work".into();
        }
    }
    let (tx, rx) = watch::channel(false);
    {
        let mut jobs = core.jobs.lock().map_err(|_| "Jobs lock failed")?;
        if jobs.contains_key(&id) {
            return Err("This conversation already has a running turn".into());
        }
        jobs.insert(id.clone(), tx);
    }
    let release = |core: &Core| {
        if let Ok(mut j) = core.jobs.lock() {
            j.remove(&id);
        }
    };
    let mut user = Message {
        id: crate::models::id(),
        role: "user".into(),
        text: text.clone(),
        activity: vec![],
    };
    if !options.attachments.is_empty() {
        user.activity.push(Activity {
            kind: "attachments".into(),
            text: options.attachments.join("\n"),
        });
    }
    if let Err(e) = core.store.save_message(&id, &user) {
        release(&core);
        return Err(e);
    }
    crate::orchestra::supersede(&app, &core, &id);
    if session.title == "New conversation" {
        session.title = text
            .split_whitespace()
            .take(8)
            .collect::<Vec<_>>()
            .join(" ")
            .chars()
            .take(64)
            .collect();
    }
    session.status = "waiting".into();
    session.updated_at = now();
    if let Err(e) = core.store.update(&session) {
        release(&core);
        return Err(e);
    }
    let core = core.inner().clone();
    let returned = plan.clone();
    tauri::async_runtime::spawn(async move {
        let mut cancelled = rx.clone();
        let permit = tokio::select! {biased;_=cancelled.changed()=>None,p=core.slots.clone().acquire_owned()=>p.ok()};
        let last_id;
        let result = if plan.strategy == "broadcast" {
            last_id = String::new();
            if permit.is_none() {
                Err("Cancelled".to_string())
            } else {
                let _ = core.store.status(&id, "running");
                run_broadcast(&app, &core, &session, &plan, &options, rx, &user).await
            }
        } else {
            let mid = crate::models::id();
            last_id = mid.clone();
            let mut output = Message {
                id: mid.clone(),
                role: "assistant".into(),
                text: String::new(),
                activity: vec![Activity {
                    kind: "plan".into(),
                    text: format!(
                        "{} · {} reasoning · {}",
                        plan.strategy, plan.effort, plan.reason
                    ),
                }],
            };
            let _ = core.store.save_message(&id, &output);
            event(&app, &id, &mid, "start", "");
            let r = if permit.is_none() {
                Err("Cancelled".into())
            } else {
                let _ = core.store.status(&id, "running");
                event(&app, &id, &mid, "status", "running");
                if plan.strategy == "orchestra" {
                    crate::orchestra::ask(
                        &app,
                        &core,
                        &session,
                        &mut output,
                        &plan,
                        &options,
                        &text,
                        rx,
                    )
                    .await
                } else {
                    if orchestra_nogit {
                        note(
                            &app,
                            &id,
                            &mut output,
                            "orchestra-nogit",
                            "Orchestra needs a git repository in this folder.".into(),
                        );
                    }
                    run_plan(&app, &core, &session, &mut output, &plan, &options, rx).await
                }
            };
            if let Err(e) = &r {
                note(&app, &id, &mut output, "error", friendly(e));
            }
            if let Err(e) = core.store.save_message(&id, &output) {
                event(
                    &app,
                    &id,
                    &mid,
                    "error",
                    &format!("Could not save conversation: {e}"),
                );
            }
            r
        };
        drop(permit);
        let status = match &result {
            Ok(()) => "completed",
            Err(e) if e == "Cancelled" => "interrupted",
            Err(_) => "error",
        };
        let _ = core.store.status(&id, status);
        if let Ok(mut j) = core.jobs.lock() {
            j.remove(&id);
        }
        event(&app, &id, &last_id, "done", status);
    });
    Ok(returned)
}

/// Token usage a provider reported for a turn, stored on the reply for the context meter.
#[derive(serde::Serialize, Default)]
struct Measured {
    /// Input tokens of the latest request, cached reads and writes included.
    latest: u64,
    /// First request input minus the local prompt estimate: CLI instructions, tools and the
    /// estimate's own error.
    overhead: Option<u64>,
    /// Context window the CLI reported for the model.
    window: Option<u64>,
}

pub(crate) fn friendly(e: &str) -> String {
    e.strip_prefix(limits::RATE_LIMIT)
        .map(|m| format!("Rate limit reached: {m}"))
        .unwrap_or_else(|| e.to_string())
}

// Stage keeps the app/session/output references explicit across asynchronous worker calls.
#[allow(clippy::too_many_arguments)]
pub async fn stage(
    app: &tauri::AppHandle,
    core: &Core,
    s: &Session,
    out: &mut Message,
    a: &Account,
    prompt: &str,
    model: Option<&str>,
    effort: &str,
    access: AccessMode,
    cancel: watch::Receiver<bool>,
    visible: bool,
) -> Result<String, String> {
    let config = config_dir(core, a);
    let workspace = PathBuf::from(&s.workspace);
    let mut checkpoint = std::time::Instant::now();
    let models = models_for(core, a, false, Some(cancel.clone())).await?;
    let selected = model
        .and_then(|id| models.iter().find(|m| m.id == id))
        .or_else(|| models.iter().find(|m| m.is_default))
        .or_else(|| models.first())
        .ok_or("No provider model available")?;
    let resolved_effort = resolve_effort(selected, effort);
    let mut waiting = cancel.clone();
    let _permit = tokio::select! {
        biased;
        _ = waiting.changed() => return Err("Cancelled".into()),
        p = core.inference.clone().acquire_owned() => p.map_err(|e| e.to_string())?,
    };
    note(
        app,
        &s.id,
        out,
        "model",
        format!(
            "{} · {} · {}",
            a.provider.binary(),
            selected.name,
            if resolved_effort.is_empty() {
                "provider default reasoning"
            } else {
                &resolved_effort
            }
        ),
    );
    // Context measurement for the meter: the first request of the turn carries the prompt plus
    // the CLI's own instructions and tools, so the difference from the local estimate is the
    // overhead every later prompt will also pay.
    let prompt_tokens = (prompt.chars().count() as u64).div_ceil(4);
    let mut measured = Measured::default();
    let result = providers::execute(
        providers::Invocation {
            account: a,
            config: &config,
            workspace: &workspace,
            prompt,
            model: Some(&selected.id),
            effort: &resolved_effort,
            access,
            approvals: core
                .approvals
                .clone()
                .map(|keeper| crate::approvals::Approvals {
                    keeper,
                    chat: s.id.clone(),
                }),
        },
        cancel,
        |activity| {
            if activity.kind == "quota" {
                crate::record_claude_usage(core, app, &a.id, &activity.text);
                return;
            }
            if activity.kind == "pid" {
                return;
            }
            if activity.kind == "usage" || activity.kind == "window" {
                let Ok(n) = activity.text.parse::<u64>() else {
                    return;
                };
                if activity.kind == "window" {
                    measured.window = Some(n);
                } else {
                    measured
                        .overhead
                        .get_or_insert(n.saturating_sub(prompt_tokens));
                    measured.latest = n;
                }
                // Only the latest measurement matters: each request replaces the last one.
                out.activity.retain(|a| a.kind != "usage");
                out.activity.push(Activity {
                    kind: "usage".into(),
                    text: serde_json::to_string(&measured).unwrap_or_default(),
                });
                return;
            }
            if visible && ["delta", "replace"].contains(&activity.kind.as_str()) {
                if activity.kind == "delta" {
                    out.text.push_str(&activity.text);
                } else {
                    out.text = activity.text.clone();
                }
                event(app, &s.id, &out.id, &activity.kind, &activity.text);
            } else if !["delta", "replace"].contains(&activity.kind.as_str()) {
                event(app, &s.id, &out.id, &activity.kind, &activity.text);
                if out.activity.len() < 500 {
                    out.activity.push(Activity {
                        kind: activity.kind,
                        text: activity.text.chars().take(16000).collect(),
                    });
                }
            }
            if visible && checkpoint.elapsed().as_millis() > 750 {
                let _ = core.store.save_message(&s.id, out);
                checkpoint = std::time::Instant::now();
            }
        },
    )
    .await;
    if result.is_ok() {
        core.limits.record_turn(&a.id, now());
    }
    result
}

/// Run follow-up work (Orchestra steps) as this conversation's job: saves the
/// user message, creates the assistant message and tracks status like a turn.
pub(crate) async fn spawn_turn<F, Fut>(
    app: tauri::AppHandle,
    core: Arc<Core>,
    id: String,
    user: Message,
    work: F,
) -> Result<(), String>
where
    F: FnOnce(tauri::AppHandle, Arc<Core>, Session, Message, watch::Receiver<bool>) -> Fut
        + Send
        + 'static,
    Fut: Future<Output = (Message, Result<(), String>)> + Send + 'static,
{
    let session = core.store.session(&id)?;
    let (tx, rx) = watch::channel(false);
    {
        let mut jobs = core.jobs.lock().map_err(|_| "Jobs lock failed")?;
        if jobs.contains_key(&id) {
            return Err("This conversation already has a running turn".into());
        }
        jobs.insert(id.clone(), tx);
    }
    if let Err(e) = core.store.save_message(&id, &user) {
        if let Ok(mut j) = core.jobs.lock() {
            j.remove(&id);
        }
        return Err(e);
    }
    let _ = core.store.status(&id, "waiting");
    tauri::async_runtime::spawn(async move {
        let mut cancelled = rx.clone();
        let permit = tokio::select! {biased;_=cancelled.changed()=>None,p=core.slots.clone().acquire_owned()=>p.ok()};
        let mid = crate::models::id();
        let output = Message {
            id: mid.clone(),
            role: "assistant".into(),
            text: String::new(),
            activity: vec![],
        };
        let _ = core.store.save_message(&id, &output);
        event(&app, &id, &mid, "start", "");
        let (mut output, result) = if permit.is_none() {
            (output, Err("Cancelled".to_string()))
        } else {
            let _ = core.store.status(&id, "running");
            event(&app, &id, &mid, "status", "running");
            work(app.clone(), core.clone(), session, output, rx).await
        };
        drop(permit);
        if let Err(e) = &result {
            note(&app, &id, &mut output, "error", friendly(e));
        }
        let _ = core.store.save_message(&id, &output);
        let status = match &result {
            Ok(()) => "completed",
            Err(e) if e == "Cancelled" => "interrupted",
            Err(_) => "error",
        };
        let _ = core.store.status(&id, status);
        if let Ok(mut j) = core.jobs.lock() {
            j.remove(&id);
        }
        event(&app, &id, &mid, "done", status);
    });
    Ok(())
}

/// Runs a stage; on a rate limit, cools the account and continues on another one.
#[allow(clippy::too_many_arguments)]
async fn stage_fallback(
    app: &tauri::AppHandle,
    core: &Core,
    s: &Session,
    out: &mut Message,
    a: &Account,
    prompt: &str,
    model: Option<&str>,
    effort: &str,
    access: AccessMode,
    cancel: watch::Receiver<bool>,
    visible: bool,
) -> Result<(String, Account), String> {
    match stage(
        app,
        core,
        s,
        out,
        a,
        prompt,
        model,
        effort,
        access,
        cancel.clone(),
        visible,
    )
    .await
    {
        Ok(t) => Ok((t, a.clone())),
        Err(e) if e.starts_with(limits::RATE_LIMIT) => {
            let msg = e.trim_start_matches(limits::RATE_LIMIT).to_string();
            core.limits.cool(&a.id, limits::unlock_at(&msg, now()));
            let enabled = core
                .settings
                .lock()
                .map(|s| s.auto_fallback)
                .unwrap_or(true);
            let mut alternatives: Vec<Account> = core
                .store
                .accounts()?
                .into_iter()
                .filter(|b| b.id != a.id && !core.limits.cooling(&b.id, now()))
                .collect();
            // Other subscription first, then the emergency providers, local Ollama last.
            alternatives.sort_by_key(|b| (b.provider.fallback_rank(), b.provider == a.provider));
            let Some(b) = alternatives.into_iter().next().filter(|_| enabled) else {
                return Err(e);
            };
            note(
                app,
                &s.id,
                out,
                "fallback",
                format!(
                    "{} ({}) hit a rate limit. Continuing on {} ({}) with the same message.",
                    a.provider.binary(),
                    a.name,
                    b.provider.binary(),
                    b.name
                ),
            );
            out.text.clear();
            event(app, &s.id, &out.id, "replace", "");
            let model = if b.provider == a.provider {
                model
            } else {
                None
            };
            let t = stage(
                app, core, s, out, &b, prompt, model, effort, access, cancel, visible,
            )
            .await?;
            Ok((t, b))
        }
        Err(e) => Err(e),
    }
}

pub(crate) fn checkpoint_of(m: &Message) -> Option<String> {
    m.activity
        .iter()
        .find(|a| a.kind == "checkpoint")
        .map(|a| a.text.clone())
}

/// Shared prompt: skills, LUMEN.md, compact history with file-change notes, attachments, audit diff.
pub(crate) async fn build_prompt(
    core: &Core,
    s: &Session,
    exclude: &[&str],
    options: &RunOptions,
) -> Result<(String, String), String> {
    let workspace = PathBuf::from(&s.workspace);
    let messages = core.store.messages(&s.id)?;
    let history = history_text(&messages, &workspace, exclude, true);
    let shared = integrations::instructions(&core.root)?;
    let attached = if options.attachments.is_empty() {
        String::new()
    } else {
        format!(
            "\nFiles attached by the user:\n{}",
            context::attachments(&workspace, &options.attachments)?
        )
    };
    let mut audit = String::new();
    if let Some(target) = &options.audit_of {
        if let Some(pos) = messages.iter().position(|m| &m.id == target) {
            let tree = messages[..pos]
                .iter()
                .rev()
                .find(|m| m.role == "user")
                .and_then(checkpoint_of);
            if let (Some(tree), Some(top)) = (tree, gitops::toplevel(&workspace).await) {
                if let Ok(diff) = gitops::diff_since(&top, &tree, 60_000).await {
                    if !diff.trim().is_empty() {
                        audit =
                            format!("\nChanges made in the audited turn (git diff -U3):\n{diff}\n");
                    }
                }
            }
        }
    }
    let map = repo_references(core, s, &workspace, &messages).await;
    let prompt = assemble_prompt(
        &shared,
        &format!("{}{map}", notes_block(&workspace)),
        permission_text(options.access()),
        &tail_chars(&history, 100_000),
        &attached,
        &audit,
    );
    Ok((prompt, history))
}

/// Declarations of identifiers the latest request mentions, from the repository map, when the
/// setting is on. What was added is recorded on the request so the chat shows it.
async fn repo_references(
    core: &Core,
    s: &Session,
    workspace: &Path,
    messages: &[Message],
) -> String {
    if !core
        .settings
        .lock()
        .map(|x| x.repo_context)
        .unwrap_or(false)
    {
        return String::new();
    }
    let Some(mut request) = messages.iter().rev().find(|m| m.role == "user").cloned() else {
        return String::new();
    };
    let (data, root, text) = (
        core.root.clone(),
        workspace.to_path_buf(),
        request.text.clone(),
    );
    let Ok((block, used)) = tauri::async_runtime::spawn_blocking(move || {
        crate::repomap::context_for(&data, &root, &text)
    })
    .await
    else {
        return String::new();
    };
    if !used.is_empty() && !request.activity.iter().any(|a| a.kind == "references") {
        request.activity.push(Activity {
            kind: "references".into(),
            text: used
                .iter()
                .map(|u| format!("{} · {}:{}", u.name, u.file, u.line))
                .collect::<Vec<_>>()
                .join(
                    "
",
                ),
        });
        let _ = core.store.save_message(&s.id, &request);
    }
    block
}

/// Chat history as the CLI receives it. Estimates pass `write` false: long pasted logs are shown
/// as excerpts without saving the full copy to the project.
fn history_text(messages: &[Message], workspace: &Path, exclude: &[&str], write: bool) -> String {
    messages
        .iter()
        .filter(|m| !exclude.contains(&m.id.as_str()))
        .map(|m| {
            let text = if m.role == "user" && write {
                context::spill(workspace, &m.text).0
            } else if m.role == "user" {
                context::shown_text(&m.text)
            } else {
                m.text.clone()
            };
            let files: Vec<String> = m
                .activity
                .iter()
                .filter(|a| a.kind == "file")
                .take(12)
                .map(|a| {
                    a.text
                        .lines()
                        .next()
                        .unwrap_or("")
                        .chars()
                        .take(160)
                        .collect()
                })
                .collect();
            let files = if files.is_empty() {
                String::new()
            } else {
                format!("\n[files changed in this turn: {}]", files.join("; "))
            };
            format!("{}:\n{}{}", m.role, text, files)
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// The last `max` characters, where the most recent turns are.
fn tail_chars(text: &str, max: usize) -> String {
    text.chars()
        .rev()
        .take(max)
        .collect::<String>()
        .chars()
        .rev()
        .collect()
}

fn permission_text(access: AccessMode) -> &'static str {
    match access {
        AccessMode::ReadOnly => "Read-only turn. Do not modify files.",
        AccessMode::FileEdits => "File edits are enabled in the chosen workspace.",
        AccessMode::Autopilot => "Autopilot is enabled: the user grants full access to available filesystem, network, shell and desktop-automation tools under their OS account. Work autonomously on the requested task.",
        AccessMode::Ask => "Ask first: each action that needs permission waits for the user to approve or reject it in Lumen. If an action is rejected, do not retry it another way.",
    }
}

fn notes_block(workspace: &Path) -> String {
    let notes = context::read_context(workspace);
    if notes.trim().is_empty() {
        String::new()
    } else {
        format!(
            "\nProject notes ({}, shared by every assistant in Lumen):\n{}\n",
            context::CONTEXT_FILE,
            notes.trim()
        )
    }
}

fn assemble_prompt(
    shared: &str,
    notes: &str,
    permission: &str,
    context_text: &str,
    attached: &str,
    audit: &str,
) -> String {
    format!(
        "{shared}{notes}\nYou are working in Lumen, alongside another assistant that may have produced earlier turns. Never read lockfiles, .env files or credentials. Respond to the final user request. {permission}\n\n{context_text}{attached}{audit}"
    )
}

/// Characters in the prompt the next turn would send, for the context meter. Attachments and
/// audit diffs are left out, and nothing is written or read from git, so the estimate is cheap.
pub(crate) fn prompt_chars(core: &Core, s: &Session) -> Result<usize, String> {
    let workspace = PathBuf::from(&s.workspace);
    let messages = core.store.messages(&s.id)?;
    let history = history_text(&messages, &workspace, &[], false);
    let shared = integrations::instructions(&core.root)?;
    let prompt = assemble_prompt(
        &shared,
        &notes_block(&workspace),
        permission_text(s.access_mode),
        &tail_chars(&history, 100_000),
        "",
        "",
    );
    Ok(prompt.chars().count())
}

/// Estimated prompt size of a chat, for the context meter.
#[tauri::command]
pub async fn prompt_estimate(
    core: tauri::State<'_, Arc<Core>>,
    id: String,
) -> Result<usize, String> {
    let s = core.store.session(&id)?;
    prompt_chars(&core, &s)
}

pub(crate) fn workspace_lock(
    core: &Core,
    s: &Session,
) -> Result<Arc<tokio::sync::Mutex<()>>, String> {
    let mut locks = core
        .workspace_locks
        .lock()
        .map_err(|_| "Workspace lock failed")?;
    let canonical = dunce::canonicalize(&s.workspace)
        .map_err(|_| "The conversation folder is no longer available")?
        .to_string_lossy()
        .into_owned();
    let key = if cfg!(windows) {
        canonical.to_lowercase()
    } else {
        canonical
    };
    Ok(locks
        .entry(key)
        .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
        .clone())
}

/// Other active Lumen conversations that target the same canonical project folder.
pub(crate) fn conversations_on_workspace(core: &Core, s: &Session) -> Vec<String> {
    let Ok(current) = dunce::canonicalize(&s.workspace) else {
        return Vec::new();
    };
    let current_key = if cfg!(windows) {
        current.to_string_lossy().to_lowercase()
    } else {
        current.to_string_lossy().into_owned()
    };
    let ids = core
        .jobs
        .lock()
        .map(|jobs| {
            jobs.keys()
                .filter(|id| *id != &s.id)
                .cloned()
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    ids.into_iter()
        .filter_map(|id| core.store.session(&id).ok())
        .filter(|other| {
            dunce::canonicalize(&other.workspace)
                .ok()
                .map(|path| {
                    let key = if cfg!(windows) {
                        path.to_string_lossy().to_lowercase()
                    } else {
                        path.to_string_lossy().into_owned()
                    };
                    key == current_key
                })
                .unwrap_or(false)
        })
        .map(|other| other.title)
        .collect()
}

fn later_project_conversations(core: &Core, s: &Session, after: i64) -> Vec<String> {
    let Ok(current) = dunce::canonicalize(&s.workspace) else {
        return Vec::new();
    };
    core.store
        .sessions_updated_after(after)
        .unwrap_or_default()
        .into_iter()
        .filter(|(id, _, _)| id != &s.id)
        .filter_map(|(_, title, folder)| {
            let folder = dunce::canonicalize(folder).ok()?;
            (folder == current).then_some(title)
        })
        .collect()
}

/// Take a turn's rewind/base snapshot only after its project write lock is held.
pub(crate) async fn checkpoint_message(
    core: &Core,
    s: &Session,
    message_id: &str,
) -> Result<Option<String>, String> {
    let mut messages = core.store.messages(&s.id)?;
    let Some(message) = messages
        .iter_mut()
        .find(|m| m.id == message_id && m.role == "user")
    else {
        return Ok(None);
    };
    if let Some(tree) = checkpoint_of(message) {
        return Ok(Some(tree));
    }
    let Some(top) = gitops::toplevel(Path::new(&s.workspace)).await else {
        return Ok(None);
    };
    let tree = gitops::checkpoint(&top, message_id).await?;
    message.activity.push(Activity {
        kind: "checkpoint".into(),
        text: tree.clone(),
    });
    core.store.save_message(&s.id, message)?;
    Ok(Some(tree))
}

pub(crate) fn approved(review: &str) -> bool {
    review.trim_start().lines().next().is_some_and(|l| {
        l.trim()
            .trim_matches('*')
            .to_uppercase()
            .starts_with("APPROVED")
    })
}

#[allow(clippy::too_many_arguments)]
async fn run_plan(
    app: &tauri::AppHandle,
    core: &Core,
    s: &Session,
    out: &mut Message,
    plan: &Plan,
    options: &RunOptions,
    mut cancel: watch::Receiver<bool>,
) -> Result<(), String> {
    let writer = account(core, &plan.account_id)?;
    let lock = workspace_lock(core, s)?;
    let other_sessions = conversations_on_workspace(core, s);
    if !other_sessions.is_empty() && options.access() != AccessMode::ReadOnly {
        note(
            app,
            &s.id,
            out,
            "coordination",
            format!(
                "Another Lumen conversation is active on this project ({}). Write turns are serialized; this turn reads the project after the other writer releases its lock.",
                other_sessions.join(", ")
            ),
        );
    }
    let _guard = if options.access() != AccessMode::ReadOnly {
        Some(
            tokio::select! {biased;_=cancel.changed()=>return Err("Cancelled".into()),g=lock.lock()=>g},
        )
    } else {
        None
    };
    if options.access() != AccessMode::ReadOnly {
        if let Some(user) = core
            .store
            .messages(&s.id)?
            .into_iter()
            .rev()
            .find(|m| m.role == "user")
        {
            checkpoint_message(core, s, &user.id).await?;
        }
    }
    let (prompt, history) = build_prompt(core, s, &[&out.id], options).await?;
    let partner_model = if let Some(id) = &plan.partner_account {
        let partner = account(core, id)?;
        let models = models_for(core, &partner, false, Some(cancel.clone())).await?;
        let task = history.rsplit("user:\n").next().unwrap_or(&history);
        Some(
            router::select_model(&models, task, &options.auto_policy)?
                .id
                .clone(),
        )
    } else {
        None
    };
    if plan.strategy == "parallel" {
        let partner = account(
            core,
            plan.partner_account.as_deref().ok_or("Missing partner")?,
        )?;
        let mut other = Message {
            id: out.id.clone(),
            role: "assistant".into(),
            text: String::new(),
            activity: vec![],
        };
        let (a, b) = tokio::join!(
            stage(
                app,
                core,
                s,
                out,
                &writer,
                &prompt,
                plan.model.as_deref(),
                &plan.effort,
                AccessMode::ReadOnly,
                cancel.clone(),
                false
            ),
            stage(
                app,
                core,
                s,
                &mut other,
                &partner,
                &prompt,
                partner_model.as_deref(),
                &plan.effort,
                AccessMode::ReadOnly,
                cancel.clone(),
                false
            )
        );
        out.activity.extend(other.activity);
        let a = a?;
        let b = b?;
        out.activity.push(Activity {
            kind: "perspectives".into(),
            text: format!("Perspective A:\n{a}\n\nPerspective B:\n{b}"),
        });
        let synth = format!("Synthesize these independent perspectives. Resolve disagreements and answer the request.\nRequest and context:\n{prompt}\n\nA:\n{a}\n\nB:\n{b}");
        out.text.clear();
        event(app, &s.id, &out.id, "replace", "");
        let (text, _) = stage_fallback(
            app,
            core,
            s,
            out,
            &writer,
            &synth,
            plan.model.as_deref(),
            &plan.effort,
            options.access(),
            cancel,
            true,
        )
        .await?;
        out.text = text;
        return Ok(());
    }
    let (mut draft, writer) = stage_fallback(
        app,
        core,
        s,
        out,
        &writer,
        &prompt,
        plan.model.as_deref(),
        &plan.effort,
        options.access(),
        cancel.clone(),
        true,
    )
    .await?;
    let writer_model = if writer.id == plan.account_id {
        plan.model.clone()
    } else {
        None
    };
    if plan.strategy == "review" {
        let reviewer = plan
            .partner_account
            .as_deref()
            .map(|id| account(core, id))
            .transpose()?
            .filter(|r| r.provider != writer.provider)
            .or_else(|| {
                core.store.accounts().ok()?.into_iter().find(|b| {
                    b.provider != writer.provider
                        && matches!(b.provider, Provider::Claude | Provider::Gpt)
                })
            });
        if let Some(reviewer) = reviewer {
            let rounds = core
                .settings
                .lock()
                .map(|s| s.review_rounds)
                .unwrap_or(1)
                .clamp(1, 3);
            let top = gitops::toplevel(Path::new(&s.workspace)).await;
            let tree = core
                .store
                .messages(&s.id)?
                .iter()
                .rev()
                .find(|m| m.role == "user")
                .and_then(checkpoint_of);
            for round in 1..=rounds {
                // Reviewers see only the unified diff of the turn, never whole files.
                let diff = match (&top, &tree) {
                    (Some(top), Some(tree)) => gitops::diff_since(top, tree, 60_000)
                        .await
                        .unwrap_or_default(),
                    _ => String::new(),
                };
                let diff = if diff.trim().is_empty() {
                    String::new()
                } else {
                    format!("\n\nFile changes so far (git diff -U3):\n{diff}")
                };
                let review_prompt = format!("You are auditing another assistant's work. Read-only: never edit files. Find bugs, architectural problems, risks and missing requirements, and propose concrete corrections. If the work is correct and complete, reply with the single word APPROVED on the first line.\n\n{prompt}\n\nDraft answer:\n{draft}{diff}");
                let mut review_out = Message {
                    id: out.id.clone(),
                    role: "assistant".into(),
                    text: String::new(),
                    activity: vec![],
                };
                let review_result = stage(
                    app,
                    core,
                    s,
                    &mut review_out,
                    &reviewer,
                    &review_prompt,
                    partner_model
                        .as_deref()
                        .filter(|_| Some(&reviewer.id) == plan.partner_account.as_ref()),
                    &plan.effort,
                    AccessMode::ReadOnly,
                    cancel.clone(),
                    false,
                )
                .await;
                out.activity.extend(review_out.activity);
                let review = review_result?;
                note(
                    app,
                    &s.id,
                    out,
                    "review",
                    format!(
                        "Audit by {} · round {round}/{rounds}\n\n{review}",
                        reviewer.provider.binary()
                    ),
                );
                if approved(&review) {
                    break;
                }
                let revise = format!("Another assistant audited your answer. Address every valid point (ignore incorrect ones) and give the final answer. Do not claim to have run checks unless you actually did.\n{prompt}\n\nYour draft:\n{draft}\n\nAudit:\n{review}");
                out.text.clear();
                event(app, &s.id, &out.id, "replace", "");
                draft = stage(
                    app,
                    core,
                    s,
                    out,
                    &writer,
                    &revise,
                    writer_model.as_deref(),
                    &plan.effort,
                    options.access(),
                    cancel.clone(),
                    true,
                )
                .await?;
            }
        }
    }
    out.text = draft;
    self_heal(
        app,
        core,
        s,
        out,
        &writer,
        writer_model.as_deref(),
        &plan.effort,
        options,
        cancel,
    )
    .await
}

/// After file edits: run the project check and send failures back to the writer (max 3 attempts).
#[allow(clippy::too_many_arguments)]
pub(crate) async fn self_heal(
    app: &tauri::AppHandle,
    core: &Core,
    s: &Session,
    out: &mut Message,
    writer: &Account,
    model: Option<&str>,
    effort: &str,
    options: &RunOptions,
    cancel: watch::Receiver<bool>,
) -> Result<(), String> {
    let enabled = core.settings.lock().map(|s| s.self_heal).unwrap_or(true);
    if !enabled
        || options.access() == AccessMode::ReadOnly
        || !out.activity.iter().any(|a| a.kind == "file")
    {
        return Ok(());
    }
    let workspace = PathBuf::from(&s.workspace);
    let Some(cmd) = context::verify_command(&workspace) else {
        return Ok(());
    };
    for attempt in 1..=3 {
        let (ok, log) = context::run_shell(&workspace, &cmd, cancel.clone()).await?;
        note(
            app,
            &s.id,
            out,
            "verify",
            if ok {
                format!("{cmd} · passed")
            } else {
                format!("{cmd} · failed\n{log}")
            },
        );
        if ok {
            return Ok(());
        }
        if attempt == 3 {
            out.text.push_str(&format!(
                "\n\n> Lumen: `{cmd}` still fails after 3 automatic fixes. Review the Verify activity."
            ));
            event(app, &s.id, &out.id, "replace", &out.text);
            return Ok(());
        }
        let fix = format!("The project check `{cmd}` failed after your changes. Fix the root cause with minimal edits, then summarize what you changed in one short paragraph.\n\nCheck output:\n{log}");
        let text = stage(
            app,
            core,
            s,
            out,
            writer,
            &fix,
            model,
            effort,
            options.access(),
            cancel.clone(),
            false,
        )
        .await?;
        out.text.push_str(&format!(
            "\n\n---\n**Self-healing · attempt {attempt}**\n\n{text}"
        ));
        event(app, &s.id, &out.id, "replace", &out.text);
        let _ = core.store.save_message(&s.id, out);
    }
    Ok(())
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Lane {
    top: String,
    path: String,
    branch: String,
    base: String,
    files: usize,
}

/// Same prompt to Claude and ChatGPT in parallel, each in its own message.
/// With edits enabled, each provider works in its own git worktree.
async fn run_broadcast(
    app: &tauri::AppHandle,
    core: &Arc<Core>,
    s: &Session,
    plan: &Plan,
    options: &RunOptions,
    cancel: watch::Receiver<bool>,
    user: &Message,
) -> Result<(), String> {
    let first = account(core, &plan.account_id)?;
    let second = account(
        core,
        plan.partner_account
            .as_deref()
            .ok_or("Missing ChatGPT account")?,
    )?;
    let workspace = PathBuf::from(&s.workspace);
    let mut access = options.access();
    let write_lock = workspace_lock(core, s)?;
    let _write_guard = if access == AccessMode::ReadOnly {
        None
    } else {
        Some(write_lock.lock().await)
    };
    let checkpoint = if access == AccessMode::ReadOnly {
        None
    } else {
        checkpoint_message(core, s, &user.id).await?
    };
    let mut outs: Vec<Message> = [&first, &second]
        .iter()
        .map(|a| Message {
            id: crate::models::id(),
            role: "assistant".into(),
            text: String::new(),
            activity: vec![Activity {
                kind: "lane".into(),
                text: a.provider.binary().into(),
            }],
        })
        .collect();
    for o in &outs {
        let _ = core.store.save_message(&s.id, o);
        event(app, &s.id, &o.id, "start", "");
    }
    let mut lanes: Vec<Option<(PathBuf, String, PathBuf)>> = vec![None, None];
    let mut sessions = [s.clone(), s.clone()];
    let mut base = String::new();
    if access != AccessMode::ReadOnly {
        match gitops::toplevel(&workspace).await {
            Some(top) => {
                base = match checkpoint.clone() {
                    Some(t) => t,
                    None => gitops::write_tree(&top).await?,
                };
                let rel = workspace
                    .strip_prefix(&top)
                    .unwrap_or(Path::new(""))
                    .to_path_buf();
                let short: String = user.id.chars().take(8).collect();
                for (i, a) in [&first, &second].iter().enumerate() {
                    let name = if a.provider == Provider::Claude {
                        "claude"
                    } else {
                        "chatgpt"
                    };
                    let path = core.root.join("worktrees").join(&short).join(name);
                    let branch = format!("lumen/{name}-{short}");
                    gitops::add_lane(&top, &base, &path, &branch).await?;
                    sessions[i].workspace = path.join(&rel).to_string_lossy().into_owned();
                    lanes[i] = Some((path, branch, top.clone()));
                }
            }
            None => {
                access = AccessMode::ReadOnly;
                for o in outs.iter_mut() {
                    note(app, &s.id, o, "lane-note", "Edits in Both mode need a git repository, so both assistants ran read-only.".into());
                }
            }
        }
    }
    let exclude: Vec<&str> = outs.iter().map(|o| o.id.as_str()).collect();
    let (prompt, _) = build_prompt(core, s, &exclude, options).await?;
    let mut picked = vec![];
    for a in [&first, &second] {
        let models = models_for(core, a, false, None).await?;
        picked.push(
            router::select_model(&models, &user.text, &options.auto_policy)?
                .id
                .clone(),
        );
    }
    let (m1, m2) = (picked[0].clone(), picked[1].clone());
    let (o1, rest) = outs.split_at_mut(1);
    let (r1, r2) = tokio::join!(
        stage(
            app,
            core,
            &sessions[0],
            &mut o1[0],
            &first,
            &prompt,
            Some(&m1),
            &plan.effort,
            access,
            cancel.clone(),
            true
        ),
        stage(
            app,
            core,
            &sessions[1],
            &mut rest[0],
            &second,
            &prompt,
            Some(&m2),
            &plan.effort,
            access,
            cancel.clone(),
            true
        )
    );
    let mut failures = vec![];
    for (i, r) in [r1, r2].into_iter().enumerate() {
        let o = &mut outs[i];
        match r {
            Ok(t) => o.text = t,
            Err(e) => {
                note(app, &s.id, o, "error", friendly(&e));
                failures.push(e);
            }
        }
        if let Some((path, branch, top)) = &lanes[i] {
            let files = gitops::lane_patch(path, &base)
                .await
                .map(|p| p.matches("diff --git").count())
                .unwrap_or(0);
            if files == 0 {
                gitops::remove_lane(top, path, branch).await;
            } else {
                let lane = Lane {
                    top: top.to_string_lossy().into(),
                    path: path.to_string_lossy().into(),
                    branch: branch.clone(),
                    base: base.clone(),
                    files,
                };
                note(
                    app,
                    &s.id,
                    o,
                    "worktree",
                    serde_json::to_string(&lane).unwrap_or_default(),
                );
            }
        }
        let _ = core.store.save_message(&s.id, o);
    }
    match failures.len() {
        2 => Err(failures.remove(0)),
        _ => Ok(()),
    }
}

fn lane_of(m: &Message) -> Option<Lane> {
    m.activity
        .iter()
        .find(|a| a.kind == "worktree")
        .and_then(|a| serde_json::from_str(&a.text).ok())
}

/// Applies one provider's version from its worktree, then removes both lanes of that turn.
#[tauri::command]
pub async fn resolve_lanes(
    core: tauri::State<'_, Arc<Core>>,
    session: String,
    message: String,
    apply: bool,
) -> Result<(), String> {
    let mut messages = core.store.messages(&session)?;
    let target = messages
        .iter()
        .find(|m| m.id == message)
        .and_then(lane_of)
        .ok_or("This version is no longer available")?;
    if apply {
        let s = core.store.session(&session)?;
        let other_sessions = conversations_on_workspace(&core, &s);
        if !other_sessions.is_empty() {
            return Err(format!(
                "Cannot apply this version while another conversation is active on the project: {}",
                other_sessions.join(", ")
            ));
        }
        let lock = workspace_lock(&core, &s)?;
        let _guard = lock.lock().await;
        let other_sessions = conversations_on_workspace(&core, &s);
        if !other_sessions.is_empty() {
            return Err(format!(
                "Cannot apply this version while another conversation is active on the project: {}",
                other_sessions.join(", ")
            ));
        }
        let patch = gitops::lane_patch(Path::new(&target.path), &target.base).await?;
        gitops::apply_patch(Path::new(&target.top), &patch).await?;
    }
    let group = Path::new(&target.path).parent().map(Path::to_path_buf);
    for m in messages.iter_mut() {
        if let Some(l) = lane_of(m) {
            if Path::new(&l.path).parent().map(Path::to_path_buf) == group {
                gitops::remove_lane(Path::new(&l.top), Path::new(&l.path), &l.branch).await;
                for a in m.activity.iter_mut().filter(|a| a.kind == "worktree") {
                    a.kind = if apply && m.id == message {
                        "applied"
                    } else {
                        "discarded"
                    }
                    .into();
                    a.text = format!("{} files", l.files);
                }
                core.store.save_message(&session, m)?;
            }
        }
    }
    Ok(())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RewindPreview {
    files: Vec<String>,
    restorable: bool,
    later_messages: usize,
}

async fn rewind_target(
    core: &Core,
    session: &str,
    message: &str,
) -> Result<(Session, Vec<Message>, usize), String> {
    let s = core.store.session(session)?;
    let messages = core.store.messages(session)?;
    let pos = messages
        .iter()
        .position(|m| m.id == message && m.role == "user")
        .ok_or("Choose one of your messages to rewind to")?;
    Ok((s, messages, pos))
}

#[tauri::command]
pub async fn rewind_preview(
    core: tauri::State<'_, Arc<Core>>,
    session: String,
    message: String,
) -> Result<RewindPreview, String> {
    let (s, messages, pos) = rewind_target(&core, &session, &message).await?;
    let message_time = core.store.message_created_at(&session, &message)?;
    let later = later_project_conversations(&core, &s, message_time);
    if !later.is_empty() {
        return Err(format!(
            "Rewind is blocked because another conversation has changed this project since that message: {}",
            later.join(", ")
        ));
    }
    // The earliest checkpoint at or after this message is the state before this turn.
    let tree = messages[pos..]
        .iter()
        .filter(|m| m.role == "user")
        .find_map(checkpoint_of);
    let files = match (tree, gitops::toplevel(Path::new(&s.workspace)).await) {
        (Some(t), Some(top)) => gitops::preview(&top, &t).await?,
        _ => vec![],
    };
    Ok(RewindPreview {
        restorable: !files.is_empty(),
        files,
        later_messages: messages.len() - pos,
    })
}

/// Restores files to the state before `message` and truncates the conversation there.
/// Returns the original prompt so the composer can offer it again.
#[tauri::command]
pub async fn rewind(
    core: tauri::State<'_, Arc<Core>>,
    session: String,
    message: String,
) -> Result<String, String> {
    if core
        .jobs
        .lock()
        .map_err(|_| "Jobs lock failed")?
        .contains_key(&session)
    {
        return Err("Stop this conversation before rewinding".into());
    }
    let initial = core.store.session(&session)?;
    let other_sessions = conversations_on_workspace(&core, &initial);
    if !other_sessions.is_empty() {
        return Err(format!(
            "Cannot rewind while another conversation is active on the project: {}",
            other_sessions.join(", ")
        ));
    }
    let lock = workspace_lock(&core, &initial)?;
    let _guard = lock.lock().await;
    let other_sessions = conversations_on_workspace(&core, &initial);
    if !other_sessions.is_empty() {
        return Err(format!(
            "Cannot rewind while another conversation is active on the project: {}",
            other_sessions.join(", ")
        ));
    }
    let (s, messages, pos) = rewind_target(&core, &session, &message).await?;
    let message_time = core.store.message_created_at(&session, &message)?;
    let later = later_project_conversations(&core, &s, message_time);
    if !later.is_empty() {
        return Err(format!(
            "Cannot rewind because another conversation has changed this project since that message: {}",
            later.join(", ")
        ));
    }
    let tree = messages[pos..]
        .iter()
        .filter(|m| m.role == "user")
        .find_map(checkpoint_of);
    if let (Some(t), Some(top)) = (tree, gitops::toplevel(Path::new(&s.workspace)).await) {
        gitops::restore(&top, &t).await?;
    }
    for m in &messages[pos..] {
        if let Some(l) = lane_of(m) {
            gitops::remove_lane(Path::new(&l.top), Path::new(&l.path), &l.branch).await;
        }
    }
    core.store.truncate_from(&session, &message)?;
    core.store.status(&session, "idle")?;
    if crate::orchestra::reopen(&core, &session) {
        return Ok(String::new());
    }
    Ok(messages[pos].text.clone())
}

/// Uncommitted changes in the session's project, for a commit-message draft. Empty when none.
#[tauri::command]
pub async fn working_diff(core: tauri::State<'_, Arc<Core>>, id: String) -> Result<String, String> {
    let s = core.store.session(&id)?;
    match gitops::toplevel(Path::new(&s.workspace)).await {
        Some(top) => gitops::diff_head(&top, 20_000).await,
        None => Ok(String::new()),
    }
}

/// Handoff & Compact: a new conversation seeded with goal, decisions, diff and next step.
/// A non-empty `summary` (for example a reviewed local draft) replaces the generated package.
#[tauri::command]
pub async fn compact_session(
    core: tauri::State<'_, Arc<Core>>,
    id: String,
    summary: Option<String>,
) -> Result<Session, String> {
    let s = core.store.session(&id)?;
    let messages = core.store.messages(&id)?;
    let users: Vec<&Message> = messages.iter().filter(|m| m.role == "user").collect();
    let answers: Vec<&Message> = messages
        .iter()
        .filter(|m| m.role == "assistant" && !m.text.trim().is_empty())
        .collect();
    let clip = |t: &str, n: usize| -> String {
        let c: String = t.chars().take(n).collect();
        if c.len() < t.len() {
            format!("{c}…")
        } else {
            c
        }
    };
    let goal = users
        .first()
        .map(|m| clip(&m.text, 2000))
        .unwrap_or_default();
    let latest = users
        .last()
        .map(|m| clip(&m.text, 2000))
        .unwrap_or_default();
    let decisions = answers
        .iter()
        .rev()
        .take(3)
        .rev()
        .map(|m| format!("- {}", clip(m.text.trim(), 900).replace('\n', "\n  ")))
        .collect::<Vec<_>>()
        .join("\n");
    let diff = match gitops::toplevel(Path::new(&s.workspace)).await {
        Some(top) => gitops::diff_head(&top, 20_000).await.unwrap_or_default(),
        None => String::new(),
    };
    let diff = if diff.trim().is_empty() {
        "No uncommitted changes.".to_string()
    } else {
        format!("```diff\n{}\n```", diff.trim())
    };
    let package = match summary.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
        Some(edited) => format!("**Handoff from “{}”**\n\n{edited}", s.title),
        None => format!(
            "**Handoff from “{}”**\n\n**Goal**\n\n{goal}\n\n**Recent decisions and results**\n\n{decisions}\n\n**Uncommitted changes**\n\n{diff}\n\n**Next step**\n\nContinue from the latest request: {latest}",
            s.title
        ),
    };
    let mut fresh = core.store.create(s.workspace.clone())?;
    if let Some(w) = &s.workspace_id {
        fresh = core
            .store
            .bind_workspace(&fresh.id, Some(w), &s.workspace)?;
    }
    fresh = core.store.set_access_mode(&fresh.id, s.access_mode)?;
    fresh.title = clip(&format!("{} · continued", s.title), 100);
    core.store.update(&fresh)?;
    core.store.save_message(
        &fresh.id,
        &Message {
            id: crate::models::id(),
            role: "assistant".into(),
            text: package,
            activity: vec![Activity {
                kind: "handoff".into(),
                text: format!("Compacted {} messages", messages.len()),
            }],
        },
    )?;
    core.store.session(&fresh.id)
}
