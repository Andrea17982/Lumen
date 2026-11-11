//! Turn orchestration: single, writer + cross-audit and
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
    client_message_id: Option<String>,
) -> Result<Plan, String> {
    let preparation_started = std::time::Instant::now();
    let message_id = match client_message_id {
        Some(id) => uuid::Uuid::parse_str(&id)
            .map_err(|_| "Invalid client message ID")?
            .to_string(),
        None => crate::models::id(),
    };
    if text.trim().is_empty() || text.len() > 100_000 {
        return Err("Enter a message of up to 100,000 bytes".into());
    }
    if context::pasted_lockfile(&text) {
        return Err("This message contains a complete lockfile. Send the relevant entries or describe the dependency change instead.".into());
    }
    let mut session = core.store.session(&id)?;
    let history = core.store.messages(&id)?;
    // Reject an already accepted ID before applying even an explicit folder change.
    if history.iter().any(|message| message.id == message_id) {
        return Err("This message was already accepted; reload the conversation".into());
    }
    let workspace_changed = crate::workspace::apply(&app, &core, &id, &text)?;
    if workspace_changed {
        session = core.store.session(&id)?;
    }
    let accounts = core.store.accounts()?;
    if options.audit_of.is_none() {
        if let Some((message, _)) = crate::orchestra::continuation(&history, &text) {
            return crate::orchestra::continue_message(
                app,
                core.inner().clone(),
                id,
                message,
                text,
                options,
                workspace_changed,
                message_id,
            )
            .await;
        }
    }
    let reused = (options.mode == "auto" && options.audit_of.is_none())
        .then(|| crate::classify::follow_up(crate::classify::previous(&history), &text))
        .flatten();
    let verdict = if let Some(v) = reused {
        v
    } else if options.mode == "auto" && options.audit_of.is_none() {
        let local = core.settings.lock().ok().and_then(|s| {
            let l = &s.local_assistant;
            (l.enabled && !l.model.is_empty()).then(|| (l.model.clone(), l.timeout_ms))
        });
        crate::classify::classify(
            &core.root,
            &accounts,
            local,
            &text,
            &crate::classify::recent_context(&history),
        )
        .await
    } else {
        crate::classify::heuristic(&text)
    };
    let mut plan = router::route_with(verdict.task, &options, &accounts)?;
    let wanted_review = plan.strategy == "review";
    if options.mode == "auto" {
        let balanced = crate::balance(&core, &options, &accounts, &mut plan).await?;
        stick_to_previous(&core, &history, &accounts, &mut plan, balanced);
        // Skip an account that is cooling down after a rate limit.
        if core.limits.cooling(&plan.account_id, now()) {
            if let Some(b) = accounts.iter().find(|b| {
                b.id != plan.account_id
                    && matches!(b.provider, Provider::Claude | Provider::Gpt)
                    && !core.limits.cooling(&b.id, now())
            }) {
                plan.provider = b.provider.clone();
                plan.account_id = b.id.clone();
                let available = accounts
                    .iter()
                    .filter(|a| !core.limits.cooling(&a.id, now()))
                    .cloned()
                    .collect::<Vec<_>>();
                plan.partner_account = if wanted_review {
                    router::review_partner(&available, b).map(|a| a.id.clone())
                } else {
                    None
                };
                if plan.partner_account.is_none() {
                    plan.strategy = "single".into();
                }
                plan.reason = "Preferred account is cooling down after a rate limit".into();
            }
        }
        reconcile_review(verdict.task, &options, wanted_review, &mut plan)?;
        let a = account(&core, &plan.account_id)?;
        let models = models_for(&core, &a, false, None).await?;
        let selected =
            router::select_tier(&models, verdict.task.tier).ok_or("No provider model available")?;
        plan.model = Some(selected.id.clone());
        plan.effort = resolve_effort(selected, &plan.effort);
        if verdict.source != "keywords" {
            plan.reason = format!("{} · rated by {}", plan.reason, verdict.source);
        }
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
    if options.audit_of.is_none()
        && plan.strategy != "broadcast"
        && core.settings.lock().map(|s| s.orchestra).unwrap_or(false)
        && verdict.complex
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
        id: message_id,
        role: "user".into(),
        text: text.clone(),
        activity: vec![],
    };
    if options.mode == "auto" {
        user.activity.push(crate::classify::record(&verdict));
    }
    if !options.attachments.is_empty() {
        user.activity.push(Activity {
            kind: "attachments".into(),
            text: options.attachments.join("\n"),
        });
    }
    crate::execution::queued(&mut user);
    if let Some(row) = user
        .activity
        .iter_mut()
        .find(|a| a.kind == "execution-phase")
    {
        if let Ok(mut value) = serde_json::from_str::<serde_json::Value>(&row.text) {
            value["preparationMs"] =
                serde_json::json!(preparation_started.elapsed().as_millis() as u64);
            row.text = value.to_string();
        }
    }
    if let Err(e) = core.store.insert_message(&id, &user) {
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
            crate::execution::phase(
                &app,
                &id,
                &mut output,
                match &r {
                    Ok(()) => "turn-returned",
                    Err(e) if e == "Cancelled" => "turn-interrupted",
                    Err(_) => "turn-error",
                },
            );
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

/// Quota/cooldown routing must not silently discard an explicitly requested audit.
fn reconcile_review(
    task: router::TaskProfile,
    options: &RunOptions,
    wanted: bool,
    plan: &mut Plan,
) -> Result<(), String> {
    if wanted && plan.strategy != "review" {
        if task.audit == router::AuditIntent::Requested || options.auto_review {
            return Err("Independent review is unavailable under current account quota/cooldown; no work was started.".into());
        }
        plan.reason
            .push_str(" · independent review unavailable under current account quota/cooldown");
    }
    Ok(())
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

/// Auto keeps a chat on the account that answered last, so the next turn can continue that
/// CLI's own session instead of starting over on another provider. A cooling account, the
/// Google emergency fallback and Both are left as routed.
fn stick_to_previous(
    core: &Core,
    history: &[Message],
    accounts: &[Account],
    plan: &mut Plan,
    balanced: bool,
) {
    if balanced
        || !matches!(plan.provider, Provider::Claude | Provider::Gpt)
        || plan.strategy == "broadcast"
    {
        return;
    }
    let Some(native) = history
        .iter()
        .rev()
        .find(|m| m.role == "assistant")
        .and_then(native_of)
    else {
        return;
    };
    let Some(a) = accounts.iter().find(|a| a.id == native.account) else {
        return;
    };
    if a.id == plan.account_id || core.limits.cooling(&a.id, now()) {
        return;
    }
    plan.provider = a.provider.clone();
    plan.account_id = a.id.clone();
    if plan.strategy != "single" {
        plan.partner_account = router::review_partner(accounts, a).map(|b| b.id.clone());
        if plan.partner_account.is_none() {
            plan.strategy = "single".into();
        }
    }
    plan.reason
        .push_str(" · same assistant as the previous reply");
}

/// The native session of `out` on `account`, continued with `prompt`.
fn continue_reply(
    out: &Message,
    account: &Account,
    workspace: &str,
    prompt: String,
) -> Option<Resume> {
    native_of(out)
        .filter(|n| n.account == account.id && n.workspace == workspace)
        .map(|n| Resume {
            session: n.session,
            prompt,
            replay: None,
        })
}

/// Native CLI session a visible reply left behind. The next turn on the same account and
/// folder continues it instead of replaying the chat as text.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub(crate) struct Native {
    pub account: String,
    pub session: String,
    pub workspace: String,
    #[serde(default)]
    pub layout_revision: Option<String>,
    #[serde(default)]
    pub context_revision: Option<String>,
}

/// A native session to continue, and the prompt that carries only the new request.
#[derive(Debug, Clone)]
pub(crate) struct Resume {
    pub session: String,
    pub prompt: String,
    pub replay: Option<RunOptions>,
}

fn native_of(m: &Message) -> Option<Native> {
    m.activity
        .iter()
        .rev()
        .find(|a| a.kind == "native")
        .and_then(|a| serde_json::from_str(&a.text).ok())
}

/// The native session to continue, when the reply right before the latest request came from
/// the same account in the same folder. Any other shape (a provider switch, Both, an edited
/// history, Ask first) falls back to the full replay.
pub(crate) fn resumable(
    messages: &[Message],
    exclude: &[&str],
    account: &Account,
    workspace: &str,
    _access: AccessMode,
) -> Option<(String, Message)> {
    if !matches!(account.provider, Provider::Claude | Provider::Gpt) {
        return None;
    }
    let shown: Vec<&Message> = messages
        .iter()
        .filter(|m| !exclude.contains(&m.id.as_str()))
        .collect();
    let (request, rest) = shown.split_last()?;
    let previous = rest.last()?;
    if request.role != "user" || previous.role != "assistant" {
        return None;
    }
    let native = native_of(previous)?;
    (native.account == account.id && native.workspace == workspace && !native.session.is_empty())
        .then(|| (native.session, (*request).clone()))
}

/// Prompt for a resumed native session: only what is new this turn.
pub(crate) async fn resume_prompt(
    core: &Core,
    s: &Session,
    exclude: &[&str],
    account: &Account,
    options: &RunOptions,
) -> Option<Resume> {
    if options.audit_of.is_some() {
        return None;
    }
    let messages = core.store.messages(&s.id).ok()?;
    let (session, request) =
        resumable(&messages, exclude, account, &s.workspace, options.access())?;
    let workspace = PathBuf::from(&s.workspace);
    let attached = if options.attachments.is_empty() {
        String::new()
    } else {
        format!(
            "\nFiles attached by the user:\n{}",
            context::attachments(&workspace, &options.attachments).ok()?
        )
    };
    let text = context::spill(&workspace, &request.text).0;
    let (revision, mut layout) = crate::layout::context(&workspace);
    let previous_native = messages
        .iter()
        .rev()
        .filter(|m| !exclude.contains(&m.id.as_str()))
        .find_map(native_of);
    let previous = previous_native
        .as_ref()
        .and_then(|n| n.layout_revision.clone());
    if revision == previous {
        layout.clear();
    } else if revision.is_none() {
        layout = "\nThe user removed the saved layout constraints. No previous canvas constraint remains active.\n".into();
    }
    let owned = crate::workstate::owned_context(core, s).ok()?;
    let old_context = previous_native
        .as_ref()
        .and_then(|n| n.context_revision.as_ref());
    let changed = old_context != Some(&owned.revision);
    let delta = if changed && owned.text.is_empty() && old_context.is_some() {
        "\nLumen's current project/check context is empty. Previously supplied notes/checks are not current evidence; do not infer passing validation.\n".to_string()
    } else if changed {
        owned.text
    } else {
        String::new()
    };
    Some(Resume {
        session,
        replay: Some(options.clone()),
        prompt: format!(
            "[Lumen: {}]\n{layout}{delta}{text}{attached}",
            permission_text(options.access())
        ),
    })
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
    stage_with(
        app, core, s, out, a, prompt, model, effort, access, cancel, visible, None,
    )
    .await
}

/// A stage that continues `resume` when given. A resume the CLI rejects (a deleted or expired
/// session) is retried once as a full replay, so a stale record never fails a turn.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn stage_with(
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
    resume: Option<&Resume>,
) -> Result<String, String> {
    let config = config_dir(core, a);
    let mut trace = crate::execution::Trace::new(a.provider.binary());
    trace.publish(app, &s.id, out, true);
    if visible {
        let _ = core.store.save_message(&s.id, out);
    }
    let resume = resume.filter(|_| matches!(a.provider, Provider::Claude | Provider::Gpt));
    let native_skills = matches!(a.provider, Provider::Claude | Provider::Gpt);
    let stripped = if native_skills {
        without(prompt, SKILLS_OPEN, SKILLS_CLOSE)
    } else {
        prompt.to_owned()
    };
    let stripped = if !matches!(a.provider, Provider::Ollama) {
        stripped
    } else {
        without(&stripped, GRAPH_OPEN, GRAPH_CLOSE)
    };
    let prompt = stripped.as_str();
    let full_prompt = prompt;
    let prompt = resume.map(|r| r.prompt.as_str()).unwrap_or(prompt);
    let workspace = PathBuf::from(&s.workspace);
    let layout_revision = crate::layout::context(&workspace).0;
    let context_revision = Some(crate::workstate::owned_context(core, s)?.revision);
    let mut checkpoint = std::time::Instant::now();
    if !out.activity.iter().any(|a| a.kind == "workspace-root") {
        note(app, &s.id, out, "workspace-root", s.workspace.clone());
    }
    let models = match models_for(core, a, false, Some(cancel.clone())).await {
        Ok(models) => models,
        Err(e) => {
            trace.finish(&Err(e.clone()));
            trace.publish(app, &s.id, out, true);
            return Err(e);
        }
    };
    let Some(selected) = model
        .and_then(|id| models.iter().find(|m| m.id == id))
        .or_else(|| models.iter().find(|m| m.is_default))
        .or_else(|| models.first())
    else {
        let error = "No provider model available".to_string();
        trace.finish(&Err(error.clone()));
        trace.publish(app, &s.id, out, true);
        return Err(error);
    };
    let resolved_effort = resolve_effort(selected, effort);
    trace.phase("capacity");
    trace.publish(app, &s.id, out, true);
    if visible {
        let _ = core.store.save_message(&s.id, out);
    }
    let mut waiting = cancel.clone();
    let permit = tokio::select! {
        biased;
        _ = waiting.changed() => Err("Cancelled".to_string()),
        p = core.inference.clone().acquire_owned() => p.map_err(|e| e.to_string()),
    };
    let permit = match permit {
        Ok(permit) => permit,
        Err(e) => {
            trace.finish(&Err(e.clone()));
            trace.publish(app, &s.id, out, true);
            return Err(e);
        }
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
    let prompt_tokens = context::estimate(prompt);
    trace.phase("starting-provider");
    trace.publish(app, &s.id, out, true);
    if visible {
        let _ = core.store.save_message(&s.id, out);
    }
    let mut measured = Measured::default();
    let mut native: Option<String> = None;
    let retry_cancel = cancel.clone();
    let result = providers::execute(
        providers::Invocation {
            browser: None,
            browser_scope: Some(&s.id),
            browser_artifacts: Some(std::path::Path::new(&s.workspace)),
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
            resume: resume.map(|r| r.session.as_str()),
        },
        cancel,
        |activity| {
            // Timing signals carry no provider content and do not add IPC/SQLite writes.
            // The existing snapshot checkpoint/finish publishes the accumulated offsets.
            if activity.kind == "latency" {
                trace.observe(&activity);
                return;
            }
            let changed = trace.observe(&activity);
            trace.publish(app, &s.id, out, changed);
            if visible && changed {
                let _ = core.store.save_message(&s.id, out);
                checkpoint = std::time::Instant::now();
            }
            // Lifecycle pairs have already updated the bounded trace. Do not let hidden
            // per-tool rows evict native sessions, questions or checkpoints from activity.
            if ["tool-start", "tool-end", "latency"].contains(&activity.kind.as_str()) {
                return;
            }
            let input_needed =
                activity.kind == "interaction" || activity.kind == "interaction-result";
            if activity.kind == "quota" {
                crate::record_claude_usage(core, app, &a.id, &activity.text);
                return;
            }
            if activity.kind == "pid" {
                return;
            }
            if activity.kind == "session" {
                native = Some(activity.text);
                return;
            }
            if activity.kind == "usage" || activity.kind == "window" {
                let Ok(n) = activity.text.parse::<u64>() else {
                    return;
                };
                if activity.kind == "window" {
                    measured.window = Some(n);
                } else {
                    if a.provider == Provider::Claude && resume.is_none() {
                        measured
                            .overhead
                            .get_or_insert(n.saturating_sub(prompt_tokens));
                    }
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
                crate::activity::record(&mut out.activity, activity);
            }
            if visible && (input_needed || checkpoint.elapsed().as_millis() > 750) {
                let _ = core.store.save_message(&s.id, out);
                checkpoint = std::time::Instant::now();
            }
        },
    )
    .await;
    trace.finish(&result);
    trace.publish(app, &s.id, out, true);
    if let (Some(_), Err(e)) = (resume, &result) {
        if resume_unavailable(e) && out.text.trim().is_empty() {
            drop(permit);
            out.activity.retain(|x| x.kind != "native");
            let replay = if full_prompt.is_empty() {
                if let Some(options) = resume.and_then(|r| r.replay.as_ref()) {
                    Some(build_prompt(core, s, &[&out.id], options).await?.0)
                } else {
                    None
                }
            } else {
                None
            };
            return Box::pin(stage_with(
                app,
                core,
                s,
                out,
                a,
                replay.as_deref().unwrap_or(full_prompt),
                model,
                effort,
                access,
                retry_cancel,
                visible,
                None,
            ))
            .await;
        }
    }
    if result.is_ok() {
        core.limits.record_turn(&a.id, now());
        if visible || resume.is_some() {
            // Codex may not repeat the id on resume; the continued session keeps its own.
            if let Some(session) = native.or_else(|| resume.map(|r| r.session.clone())) {
                out.activity.retain(|x| x.kind != "native");
                out.activity.push(Activity {
                    kind: "native".into(),
                    text: serde_json::to_string(&Native {
                        account: a.id.clone(),
                        session,
                        workspace: s.workspace.clone(),
                        layout_revision,
                        context_revision,
                    })
                    .unwrap_or_default(),
                });
            }
        }
    }
    result
}

fn resume_unavailable(error: &str) -> bool {
    let error = error.to_lowercase();
    [
        "session not found",
        "thread not found",
        "no conversation found",
        "could not find thread",
        "unknown session",
        "unknown thread",
    ]
    .iter()
    .any(|s| error.contains(s))
}

/// Run follow-up work (Orchestra steps) as this conversation's job: saves the
/// user message, creates the assistant message and tracks status like a turn.
pub(crate) async fn spawn_turn<F, Fut>(
    app: tauri::AppHandle,
    core: Arc<Core>,
    id: String,
    mut user: Message,
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
    crate::execution::queued(&mut user);
    if let Err(e) = core.store.insert_message(&id, &user) {
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
        crate::execution::phase(
            &app,
            &id,
            &mut output,
            match &result {
                Ok(()) => "turn-returned",
                Err(e) if e == "Cancelled" => "turn-interrupted",
                Err(_) => "turn-error",
            },
        );
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
    resume: Option<&Resume>,
) -> Result<(String, Account), String> {
    match stage_with(
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
        resume,
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
            let replay = if prompt.is_empty() {
                if let Some(options) = resume.and_then(|r| r.replay.as_ref()) {
                    Some(build_prompt(core, s, &[&out.id], options).await?.0)
                } else {
                    None
                }
            } else {
                None
            };
            let t = stage(
                app,
                core,
                s,
                out,
                &b,
                replay.as_deref().unwrap_or(prompt),
                model,
                effort,
                access,
                cancel,
                visible,
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
        &format!(
            "{}{map}{}",
            notes_block(&workspace),
            crate::layout::context(&workspace).1
        ),
        permission_text(options.access()),
        &fit_history(&messages, &history, exclude, 100_000),
        &attached,
        &audit,
    );
    let state = crate::workstate::context(core, s, options.access())?;
    let owned = crate::workstate::owned_context(core, s)?;
    let prompt = format!("Current conversation workspace: {}\nAll relative paths and project tools use this directory. Explicit user directory changes are applied by Lumen before this turn; do not infer a switch from paths in quoted history or attachments.\n{state}\n{prompt}{}", s.workspace,owned.evidence);
    Ok((
        prompt,
        format!(
            "{state}\n{}",
            fit_history(&messages, &history, exclude, 100_000)
        ),
    ))
}

/// Points Claude and Codex at the code-graph tools when the setting is on. The CLIs query the
/// graph themselves, so nothing is guessed from the request's words.
async fn repo_references(
    core: &Core,
    _s: &Session,
    _workspace: &Path,
    _messages: &[Message],
) -> String {
    if !core
        .settings
        .lock()
        .map(|x| x.repo_context)
        .unwrap_or(false)
    {
        return String::new();
    }
    format!(
        "{GRAPH_OPEN}Code graph: the `{}` tools (overview, find_symbol, symbol, file_outline, related_files, impact) know where code is defined, who uses it and what a change affects, with exact line ranges. Use them before broad Grep or whole-file Reads, then Read only the lines you need.\n{GRAPH_CLOSE}",
        crate::graph_mcp::SERVER
    )
}

const GRAPH_OPEN: &str = "<lumen-graph>\n";
const GRAPH_CLOSE: &str = "</lumen-graph>\n";

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
            let answers = crate::interactions::answer_context(&m.activity).join("\n");
            format!(
                "{}:\n{}{}{}",
                m.role,
                text,
                files,
                if answers.is_empty() {
                    String::new()
                } else {
                    format!("\n{answers}")
                }
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// The history the prompt can hold: the latest turns, plus the chat's first request when it
/// would otherwise be cut, so the goal of a long chat is never lost.
fn fit_history(messages: &[Message], history: &str, exclude: &[&str], max: usize) -> String {
    if history.chars().count() <= max {
        return history.to_owned();
    }
    let first: String = messages
        .iter()
        .find(|m| m.role == "user" && !exclude.contains(&m.id.as_str()))
        .map(|m| context::shown_text(&m.text).chars().take(4_000).collect())
        .unwrap_or_default();
    let head = format!(
        "[Lumen: earlier turns were omitted. This chat began with this request:]
user:
{first}

[...]

"
    );
    let room = max.saturating_sub(head.chars().count());
    format!("{head}{}", tail_chars(history, room))
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

const SKILLS_OPEN: &str = "<lumen-skills>\n";
const SKILLS_CLOSE: &str = "</lumen-skills>\n";

/// The prompt without its first `open`…`close` section: Lumen's skill listing for CLIs that load
/// skills natively, the code-graph hint for CLIs that do not get the graph server.
fn without(prompt: &str, open: &str, close: &str) -> String {
    match (prompt.find(open), prompt.find(close)) {
        (Some(a), Some(b)) if b > a => format!("{}{}", &prompt[..a], &prompt[b + close.len()..]),
        _ => prompt.to_owned(),
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
    let shared = if shared.trim().is_empty() {
        String::new()
    } else {
        format!("{SKILLS_OPEN}{shared}{SKILLS_CLOSE}")
    };
    format!(
        "{shared}{notes}\nYou are working in Lumen. Keep your native tools, agent behavior and project instructions. Complete the user's requested work within the current permission profile; preserve approvals and decisions already given. Use a native structured question only when missing information changes the result. Only claim checks that actually ran. {permission}\n\n{context_text}{attached}{audit}"
    )
}

/// Characters in the prompt the next turn would send, for the context meter. Attachments and
/// audit diffs are left out, and nothing is written or read from git, so the estimate is cheap.
pub(crate) fn prompt_tokens(core: &Core, s: &Session) -> Result<usize, String> {
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
    let state = core
        .store
        .work_state(&s.id)?
        .map(|s| s.prompt())
        .unwrap_or_default();
    Ok(context::estimate(&format!("{prompt}{state}")) as usize)
}

/// Estimated prompt size of a chat, for the context meter.
#[tauri::command]
pub async fn prompt_estimate(
    core: tauri::State<'_, Arc<Core>>,
    id: String,
) -> Result<usize, String> {
    let s = core.store.session(&id)?;
    prompt_tokens(&core, &s)
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
    review
        .trim()
        .trim_matches('*')
        .trim()
        .eq_ignore_ascii_case("APPROVED")
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
    crate::execution::phase(app, &s.id, out, "workspace");
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
    crate::execution::phase(app, &s.id, out, "context");
    let verify = core
        .settings
        .lock()
        .map(|s| s.verify_changes)
        .unwrap_or(false);
    let baseline = if verify {
        crate::impact::snapshot(core, Path::new(&s.workspace), options.access()).await?
    } else {
        None
    };
    let resume = resume_prompt(core, s, &[&out.id], &writer, options).await;
    let (prompt, history) = if resume.is_some() && plan.strategy == "single" {
        (String::new(), String::new())
    } else {
        build_prompt(core, s, &[&out.id], options).await?
    };
    if resume.is_some() {
        note(
            app,
            &s.id,
            out,
            "resume",
            "Continuing the native session with the new request and current permissions.".into(),
        );
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
        resume.as_ref(),
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
            .filter(|r| r.id != writer.id && !core.limits.cooling(&r.id, now()))
            .or_else(|| {
                let available = core
                    .store
                    .accounts()
                    .ok()?
                    .into_iter()
                    .filter(|a| !core.limits.cooling(&a.id, now()))
                    .collect::<Vec<_>>();
                router::review_partner(&available, &writer).cloned()
            });
        if let Some(reviewer) = reviewer {
            // Resolve against the actual independent account after writer fallback, not
            // a model ID belonging to the originally planned reviewer.
            let models = models_for(core, &reviewer, false, Some(cancel.clone())).await?;
            let task = history.rsplit("user:\n").next().unwrap_or(&history);
            let target = core
                .store
                .messages(&s.id)
                .ok()
                .and_then(|messages| crate::classify::previous(&messages))
                .map(|v| v.task.tier)
                .unwrap_or_else(|| router::task_tier(task));
            let reviewer_model = router::select_tier(&models, target)
                .ok_or("No reviewer model available")?
                .id
                .clone();
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
                let review_prompt = format!("You are auditing another assistant's work. Read-only: never edit files. Find bugs, architectural problems, risks and missing requirements, and propose concrete corrections. If the work is correct and complete, reply with exactly APPROVED and nothing else.\n\n{prompt}\n\nDraft answer:\n{draft}{diff}");
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
                    Some(&reviewer_model),
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
                        "Audit by {} ({}) · round {round}/{rounds}\n\n{review}",
                        reviewer.provider.binary(),
                        reviewer.name
                    ),
                );
                if approved(&review) {
                    break;
                }
                let revise = format!("Another assistant audited your answer. Address every valid point (ignore incorrect ones) and give the final answer. Do not claim to have run checks unless you actually did.\n{prompt}\n\nYour draft:\n{draft}\n\nAudit:\n{review}");
                out.text.clear();
                event(app, &s.id, &out.id, "replace", "");
                let resume = continue_reply(
                    out,
                    &writer,
                    &s.workspace,
                    format!("Another assistant audited your answer. Address every valid point (ignore incorrect ones) and give the final answer. Do not claim to have run checks unless you actually did.\n\nAudit:\n{review}"),
                );
                draft = stage_with(
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
                    resume.as_ref(),
                )
                .await?;
            }
        } else {
            note(app,&s.id,out,"review-unavailable","No independent account is available; the writer's work is retained but was not cross-audited.".into());
            return Err("Independent review could not run on an available account".into());
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
        baseline,
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
    baseline: Option<Arc<crate::codegraph::Graph>>,
    cancel: watch::Receiver<bool>,
) -> Result<(), String> {
    if !out.activity.iter().any(|a| a.kind == "orchestra")
        && !core
            .settings
            .lock()
            .map(|s| s.verify_changes)
            .unwrap_or(false)
    {
        return Ok(());
    }
    if options.access() == AccessMode::ReadOnly {
        return Ok(());
    }
    let workspace = PathBuf::from(&s.workspace);
    let mut files = crate::validation::changed_files(core, s, out).await?;
    if files.is_empty() {
        return Ok(());
    }
    if options.access() == AccessMode::Ask {
        let report = crate::diagnostics::Report::unavailable("Automatic validation".into(), "Ask-first mode: checks must be run through the agent's approved tools; backend checks were not started".into());
        record_validation(app, s, out, &report)?;
        out.text.push_str("\n\n> Lumen: automatic backend validation was not run in Ask-first mode. Only checks performed through approved tools can be claimed.");
        event(app, &s.id, &out.id, "replace", &out.text);
        return Ok(());
    }
    let repairs = if core.settings.lock().map(|s| s.self_heal).unwrap_or(true) {
        3
    } else {
        0
    };
    for attempt in 0..=repairs {
        let mut verification_files = files.clone();
        let impact = crate::impact::analyse(core, &workspace, &files, baseline.clone()).await?;
        let impact_context = if let Some(report) = &impact {
            for file in &report.files {
                if !verification_files.contains(&file.path) {
                    verification_files.push(file.path.clone());
                }
            }
            note(app, &s.id, out, "impact", report.text());
            crate::impact::handoff(&workspace, report)
        } else {
            String::new()
        };
        note(
            app,
            &s.id,
            out,
            "tool",
            "Running diagnostics and relevant project checks.".into(),
        );
        crate::execution::phase(app, &s.id, out, "verification");
        let reports =
            crate::validation::run(&workspace, &verification_files, cancel.clone()).await?;
        for report in &reports {
            record_validation(app, s, out, report)?;
            note(
                app,
                &s.id,
                out,
                "verify",
                format!(
                    "{} · {}\n{}\n{}",
                    report.label,
                    report.status,
                    report.errors(),
                    report.detail
                ),
            );
        }
        core.store.save_message(&s.id, out)?;
        let failures = crate::validation::failure_text(&reports);
        if failures.is_empty() || attempt == repairs {
            out.text.push_str(&format!(
                "\n\n**Verification by Lumen**\n{}",
                reports
                    .iter()
                    .map(|r| format!("- {}: {}", r.label, r.status))
                    .collect::<Vec<_>>()
                    .join("\n")
            ));
            event(app, &s.id, &out.id, "replace", &out.text);
            if failures.is_empty() {
                return Ok(());
            }
            return Err(format!("Validation still fails after {repairs} automatic corrections. See the recorded diagnostics and checks."));
        }
        let (context, _) = build_prompt(core, s, &[&out.id], options).await?;
        let fix = format!("Validation found concrete errors after your changes. Correct errors caused by this task with minimal edits, preserve unrelated existing work and all agreed requirements. Do not replace implementation with another plan or claim unavailable checks passed. Summarize the correction briefly.\n\n{impact_context}\nDiagnostics and check output:\n{failures}");
        let resume = continue_reply(out, writer, &s.workspace, fix.clone());
        let text = stage_with(
            app,
            core,
            s,
            out,
            writer,
            &format!("{context}\n\n{fix}"),
            model,
            effort,
            options.access(),
            cancel.clone(),
            false,
            resume.as_ref(),
        )
        .await?;
        out.text.push_str(&format!(
            "\n\n---\n**Self-healing · attempt {}**\n\n{text}",
            attempt + 1
        ));
        event(app, &s.id, &out.id, "replace", &out.text);
        core.store.save_message(&s.id, out)?;
        files = crate::validation::changed_files(core, s, out).await?;
    }
    Ok(())
}

fn record_validation(
    app: &tauri::AppHandle,
    s: &Session,
    out: &mut Message,
    report: &crate::diagnostics::Report,
) -> Result<(), String> {
    let mut value = serde_json::to_value(report).map_err(|e| e.to_string())?;
    value["workspace"] = serde_json::Value::String(s.workspace.clone());
    note(app, &s.id, out, "validation", value.to_string());
    Ok(())
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Lane {
    pub top: String,
    pub path: String,
    pub branch: String,
    pub base: String,
    pub files: usize,
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

pub(crate) fn lane_of(m: &Message) -> Option<Lane> {
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
    let mut state = crate::workstate::refresh(&core.store, &id, None)?;
    let handoff_id = crate::models::id();
    if let Some(edited) = summary.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
        state.decisions.push(crate::workstate::Fact { source: handoff_id.clone(), text: format!("User-supplied handoff; supersedes conflicting earlier task facts, but not live permissions:\n{edited}") });
    }
    core.store.seed_work_state(&fresh.id, &state)?;
    fresh.title = clip(&format!("{} · continued", s.title), 100);
    core.store.update(&fresh)?;
    core.store.save_message(
        &fresh.id,
        &Message {
            id: handoff_id,
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

#[cfg(test)]
mod resume_tests {
    use super::*;

    fn msg(id: &str, role: &str, activity: Vec<Activity>) -> Message {
        Message {
            id: id.into(),
            role: role.into(),
            text: format!("text {id}"),
            activity,
        }
    }
    fn native(account: &str, workspace: &str) -> Activity {
        Activity {
            kind: "native".into(),
            text: serde_json::to_string(&Native {
                account: account.into(),
                session: "sid".into(),
                workspace: workspace.into(),
                layout_revision: None,
                context_revision: None,
            })
            .unwrap(),
        }
    }
    fn claude() -> Account {
        Account {
            id: "acc".into(),
            name: "A".into(),
            provider: Provider::Claude,
        }
    }
    #[test]
    fn quality_qualified_audit_is_not_approval() {
        assert!(approved("APPROVED"));
        assert!(approved("**APPROVED**"));
        for text in [
            "APPROVED with blocking issues",
            "APPROVED? No, fix data loss",
            "APPROVED\nBut tests still fail",
            "NOT APPROVED",
        ] {
            assert!(!approved(text), "{text}");
        }
    }
    #[test]
    fn quality_quota_downgrade_cannot_silently_discard_a_requested_audit() {
        let mut options = crate::test_support::Fixture::options(AccessMode::Autopilot);
        options.mode = "auto".into();
        let mut plan = Plan {
            provider: Provider::Claude,
            account_id: "writer".into(),
            model: None,
            effort: "low".into(),
            strategy: "single".into(),
            reason: "quota selection".into(),
            partner_account: None,
        };
        let requested = router::profile("Fai un cross-audit di questo fix");
        assert!(reconcile_review(requested, &options, true, &mut plan).is_err());
        let optional = router::profile("Implement security authentication");
        assert!(reconcile_review(optional, &options, true, &mut plan).is_ok());
        assert!(plan.reason.contains("review unavailable"));
        options.auto_review = true;
        assert!(reconcile_review(optional, &options, true, &mut plan).is_err());
        // No previously selected review means no audit promise was made.
        assert!(reconcile_review(optional, &options, false, &mut plan).is_ok());
    }
    #[test]
    fn quality_continuity_does_not_undo_balanced_account_selection() {
        let f = crate::test_support::Fixture::new();
        let old = claude();
        let other = Account {
            id: "balanced".into(),
            name: "B".into(),
            provider: Provider::Gpt,
        };
        let history = vec![msg("a", "assistant", vec![native(&old.id, "/w")])];
        let mut plan = Plan {
            provider: other.provider.clone(),
            account_id: other.id.clone(),
            model: None,
            effort: "medium".into(),
            strategy: "single".into(),
            reason: "quota selection".into(),
            partner_account: None,
        };
        stick_to_previous(&f.core, &history, &[old, other.clone()], &mut plan, true);
        assert_eq!(plan.account_id, other.id);
    }

    #[test]
    fn resumes_only_the_same_account_and_folder() {
        let chat = vec![
            msg("u1", "user", vec![]),
            msg("a1", "assistant", vec![native("acc", "/w")]),
            msg("u2", "user", vec![]),
            msg("out", "assistant", vec![]),
        ];
        let (sid, req) =
            resumable(&chat, &["out"], &claude(), "/w", AccessMode::FileEdits).unwrap();
        assert_eq!((sid.as_str(), req.id.as_str()), ("sid", "u2"));
        assert!(resumable(&chat, &["out"], &claude(), "/other", AccessMode::FileEdits).is_none());
        assert!(resumable(&chat, &["out"], &claude(), "/w", AccessMode::Ask).is_some());
        let mut other = claude();
        other.id = "b".into();
        assert!(resumable(&chat, &["out"], &other, "/w", AccessMode::FileEdits).is_none());
    }

    #[tokio::test]
    async fn quality_native_resume_receives_lumen_owned_check_failures() {
        let f = crate::test_support::Fixture::new();
        let folder = f.root.to_string_lossy().into_owned();
        let s = f.core.store.create(folder.clone()).unwrap();
        let mut reply = msg("a", "assistant", vec![native("acc", &folder)]);
        reply.activity.push(Activity{kind:"validation".into(),text:serde_json::json!({"workspace":folder,"label":"Independent counter check","status":"failed"}).to_string()});
        f.core.store.save_message(&s.id, &reply).unwrap();
        f.core
            .store
            .save_message(&s.id, &msg("u", "user", vec![]))
            .unwrap();
        let options = crate::test_support::Fixture::options(AccessMode::Autopilot);
        let resumed = resume_prompt(&f.core, &s, &[], &claude(), &options)
            .await
            .unwrap();
        assert!(resumed.prompt.contains("Independent counter check: failed"));
    }
    #[tokio::test]
    async fn native_resume_sends_only_changed_or_removed_layout_constraints() {
        use crate::test_support::Fixture;
        let f = Fixture::new();
        let folder = dunce::canonicalize(&f.root)
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let s = f.core.store.create(folder.clone()).unwrap();
        let request = msg("u2", "user", vec![]);
        let save = |revision: Option<String>| {
            let mut reply = msg("a1", "assistant", vec![]);
            reply.activity.push(Activity {
                kind: "native".into(),
                text: serde_json::to_string(&Native {
                    account: "acc".into(),
                    session: "sid".into(),
                    workspace: folder.clone(),
                    layout_revision: revision,
                    context_revision: None,
                })
                .unwrap(),
            });
            f.core.store.save_message(&s.id, &reply).unwrap();
            f.core.store.save_message(&s.id, &request).unwrap();
        };
        let options = Fixture::options(AccessMode::Autopilot);
        save(None);
        let p = resume_prompt(&f.core, &s, &[], &claude(), &options)
            .await
            .unwrap();
        assert!(!p.prompt.contains("layout constraint"));
        let spec=serde_json::from_value(serde_json::json!({"version":1,"width":800,"height":600,"blocks":[{"id":"nav","label":"Navigation","role":"navigation","x":0,"y":0,"width":200,"height":600,"notes":"Keep visible"}]})).unwrap();
        crate::layout::save_layout(folder.clone(), spec).unwrap();
        let p = resume_prompt(&f.core, &s, &[], &claude(), &options)
            .await
            .unwrap();
        assert!(p.prompt.contains("Keep visible"));
        assert!(!p.prompt.contains("text a1"));
        save(crate::layout::context(&f.root).0);
        let p = resume_prompt(&f.core, &s, &[], &claude(), &options)
            .await
            .unwrap();
        assert!(!p.prompt.contains("Keep visible"));
        std::fs::remove_file(f.root.join(".lumen/layout.json")).unwrap();
        let p = resume_prompt(&f.core, &s, &[], &claude(), &options)
            .await
            .unwrap();
        assert!(p.prompt.contains("removed the saved layout"));
    }

    #[test]
    fn replays_when_the_previous_reply_has_no_session() {
        let chat = vec![
            msg("u1", "user", vec![]),
            msg("a1", "assistant", vec![native("acc", "/w")]),
            msg("u2", "user", vec![]),
            msg("a2", "assistant", vec![]),
            msg("u3", "user", vec![]),
        ];
        assert!(resumable(&chat, &[], &claude(), "/w", AccessMode::FileEdits).is_none());
    }

    #[test]
    fn long_history_keeps_the_first_request() {
        let chat = vec![msg("u1", "user", vec![])];
        let history = format!("user:\ntext u1\n\n{}", "x".repeat(500));
        let fitted = fit_history(&chat, &history, &[], 300);
        assert!(fitted.contains("text u1"));
        assert!(fitted.chars().count() <= 300);
        assert!(fitted.ends_with('x'));
    }
}
