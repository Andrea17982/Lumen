//! Turn orchestration: single or writer + cross-audit, with rate-limit fallback,
//! checkpoints and self-healing.
use crate::{
    account, config_dir, context, event, gitops, integrations, limits, models::*, models_for,
    models_in, providers, resolve_effort, router, Core,
};
use std::{
    future::Future,
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::sync::watch;

pub(crate) fn note(
    app: &crate::host::EventHub,
    s: &str,
    out: &mut Message,
    kind: &str,
    text: String,
) {
    event(app, s, &out.id, kind, &text);
    out.activity.push(Activity {
        kind: kind.into(),
        text,
    });
}

/// A turn task that panics must not leave its conversation running, with Stop unable to end it:
/// release the job and close the turn as an error. The panic message goes to the service log.
pub(crate) fn supervise(
    app: crate::host::EventHub,
    core: Arc<Core>,
    id: String,
    cancel: watch::Receiver<bool>,
    task: tokio::task::JoinHandle<()>,
) {
    tokio::spawn(async move {
        if !task.await.is_err_and(|e| e.is_panic()) {
            return;
        }
        // Only the panicked turn's own job: a newer turn of the conversation is left alone.
        let Ok(mut jobs) = core.jobs.lock() else {
            return;
        };
        if jobs
            .get(&id)
            .is_some_and(|tx| tx.subscribe().same_channel(&cancel))
        {
            jobs.remove(&id);
            let _ = core.store.status(&id, "error");
            event(&app, &id, "", "done", "error");
        }
    });
}

#[crate::command]
pub async fn send_message(
    app: crate::host::EventHub,
    core: crate::CoreRef<'_>,
    id: String,
    text: String,
    options: RunOptions,
    client_message_id: Option<String>,
) -> Result<Plan, String> {
    submit(
        app,
        core.inner().clone(),
        id,
        text,
        options,
        client_message_id,
        None,
    )
    .await
}

pub(crate) async fn submit(
    app: crate::host::EventHub,
    core: Arc<Core>,
    id: String,
    text: String,
    options: RunOptions,
    client_message_id: Option<String>,
    reservation: Option<watch::Receiver<bool>>,
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
    if reservation.is_none()
        && core
            .jobs
            .lock()
            .map_err(|_| "Jobs lock failed")?
            .contains_key(&id)
    {
        return Err("This conversation already has a running or scheduled turn".into());
    }
    if reservation.as_ref().is_some_and(|cancel| *cancel.borrow()) {
        return Err("Cancelled".into());
    }
    let workspace_changed = crate::workspace::apply(&app, &core, &id, &text)?;
    if workspace_changed {
        session = core.store.session(&id)?;
    }
    let accounts = core.store.accounts()?;
    let requested = (options.mode == "auto")
        .then(|| router::requested_provider(&text))
        .flatten();
    let mut usage = if options.mode == "auto" {
        crate::read_usage(&core)?
    } else {
        vec![]
    };
    let budget = options.auto_policy == AutoPolicy::Budget;
    let quality = options.auto_policy == AutoPolicy::Quality;
    let cross_provider = budget && requested.is_none();
    let mut signed_out = std::collections::HashSet::new();
    if options.mode == "auto"
        && requested.is_none()
        && accounts.iter().any(|a| a.provider == Provider::Opencode)
    {
        let mut checks = tokio::task::JoinSet::new();
        for a in accounts
            .iter()
            .filter(|a| matches!(a.provider, Provider::Claude | Provider::Gpt))
        {
            let mut command = match providers::command(&a.provider) {
                Ok(command) => command,
                Err(_) => continue,
            };
            providers::isolate(&mut command, &a.provider, &config_dir(&core, a));
            command.kill_on_drop(true);
            match a.provider {
                Provider::Claude => {
                    command.args(["auth", "status"]);
                }
                Provider::Gpt => {
                    command.args(["login", "status"]);
                }
                _ => unreachable!(),
            }
            let provider = a.provider.clone();
            let id = a.id.clone();
            checks.spawn(async move {
                let output =
                    tokio::time::timeout(std::time::Duration::from_secs(5), command.output())
                        .await
                        .ok()?
                        .ok()?;
                confirmed_logout(
                    &provider,
                    output.status.success(),
                    &output.stdout,
                    &output.stderr,
                )
                .then_some(id)
            });
        }
        while let Some(result) = checks.join_next().await {
            if let Ok(Some(id)) = result {
                signed_out.insert(id);
            }
        }
    }
    let routing_accounts: Vec<_> = accounts
        .iter()
        .filter(|a| !signed_out.contains(&a.id))
        .cloned()
        .collect();
    let primary_down = primary_unavailable(&core, &routing_accounts, &usage, now());
    let opencode = requested.is_none()
        && accounts.iter().any(|a| a.provider == Provider::Opencode)
        && (budget || primary_down);
    if options.mode == "auto"
        && (accounts
            .iter()
            .filter(|a| a.provider == Provider::Gpt)
            .count()
            > 1
            || (requested.is_none()
                && (budget || accounts.iter().any(|a| a.provider.fallback_rank() > 0))))
    {
        crate::refresh_routing_usage(&core, &accounts, &mut usage, opencode).await?;
    }
    let available = accounts
        .iter()
        .filter(|a| {
            !signed_out.contains(&a.id)
                && (!matches!(a.provider, Provider::Cursor | Provider::Droid))
                && (!a.provider.is_api() || crate::api::eligible(&core.root, a))
                && !core.limits.cooling(&a.id, now())
                && !usage
                    .iter()
                    .find(|u| u.account_id == a.id)
                    .and_then(|u| router::usage_pressure(u, now()))
                    .is_some_and(|p| p >= 1000.0)
        })
        .cloned()
        .collect::<Vec<_>>();
    if options.mode == "auto" && requested.is_none() && available.is_empty() {
        return Err("No account is currently available. Check provider credits or wait for the quota reset.".into());
    }
    if options.audit_of.is_none() && requested.is_none() {
        if let Some((message, _)) = crate::orchestra::continuation(&history, &text) {
            return crate::orchestra::continue_message(
                app,
                core.clone(),
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
    let reused = (options.mode == "auto" && options.audit_of.is_none() && requested.is_none())
        .then(|| crate::classify::follow_up(crate::classify::previous(&history), &text))
        .flatten();
    let mut verdict = if let Some(v) = reused {
        v
    } else if options.mode == "auto" && options.audit_of.is_none() && requested.is_none() {
        crate::classify::classify(
            &core.root,
            &available,
            &text,
            &crate::classify::recent_context(&history),
        )
        .await
    } else {
        crate::classify::heuristic(&text)
    };
    let mut routing = options.clone();
    if let Some(provider) = requested {
        routing.mode = provider.into();
        routing.account_id = None;
        routing.model = None;
        routing.effort = router::requested_effort(verdict.task, &options.auto_policy).into();
    }
    let candidates = if options.mode == "auto" && requested.is_none() {
        &available
    } else {
        &accounts
    };
    let mut plan = router::route_with(verdict.task, &routing, candidates, &text)?;
    let wanted_review = plan.strategy == "review";
    let deep_research = router::deep_research(&text);
    if options.mode == "auto" {
        if deep_research {
            plan.reason = "In-depth research with proportional reasoning".into();
        }
        let resumed = requested.is_none()
            && !budget
            && (!quality || reused.is_some())
            && stick_to_previous(&core, &history, &available, &mut plan);
        let a = account(&core, &plan.account_id)?;
        let models = models_in(&core, &a, false, None, &session.workspace).await?;
        let target = verdict.task.tier;
        let pinned = core
            .settings
            .lock()
            .ok()
            .and_then(|s| s.emergency_models.get(&plan.provider.id()).cloned());
        let mut selected = if plan.provider == Provider::Opencode {
            // OpenCode's emergency role is deterministic: the pinned model only, or the curated
            // tier model (up before down), never an arbitrary catalogue entry.
            crate::opencode::auto_model(&models, target, pinned.as_deref())
                .cloned()
                .ok_or_else(|| match pinned {
                    Some(id) => format!(
                        "The pinned emergency model {id} is not available; update it in Settings."
                    ),
                    None => "No OpenCode model is available for the emergency fallback".into(),
                })?
        } else {
            router::select_tier(&models, target)
                .cloned()
                .ok_or("No provider model available")?
        };
        if requested.is_none()
            && matches!(a.provider, Provider::Claude | Provider::Gpt)
            && router::model_tier(&selected) < target
        {
            for alternative in candidates.iter().filter(|candidate| {
                candidate.id != a.id
                    && matches!(candidate.provider, Provider::Claude | Provider::Gpt)
            }) {
                let Ok(models) =
                    models_in(&core, alternative, false, None, &session.workspace).await
                else {
                    continue;
                };
                if let Some(model) = router::select_tier(&models, target)
                    .filter(|model| router::model_tier(model) >= target)
                {
                    selected = model.clone();
                    use_writer(&mut plan, alternative, candidates);
                    plan.reason =
                        "Another configured account has a model that meets the task difficulty"
                            .into();
                    break;
                }
            }
        }
        selected = balance_model_account(
            &core,
            &session.workspace,
            &available,
            &usage,
            cross_provider,
            &mut plan,
            selected,
        )
        .await;
        if resumed && plan.account_id == a.id {
            plan.reason
                .push_str(" · same assistant as the previous reply");
        }
        reconcile_review(verdict.task, &options, wanted_review, &mut plan)?;
        plan.effort = resolve_effort(&selected, &plan.effort);
        plan.model = Some(selected.id);
        if verdict.source != "keywords" {
            plan.reason = format!("{} · rated by {}", plan.reason, verdict.source);
        }
    } else {
        let a = account(&core, &plan.account_id)?;
        let models = models_in(&core, &a, false, None, &session.workspace).await?;
        crate::catalog::validate(&models, plan.model.as_deref(), &plan.effort)?;
    }
    if options.mode != "auto" {
        let a = account(&core, &plan.account_id)?;
        let models = models_in(&core, &a, false, None, &session.workspace).await?;
        if let Some(selected) = plan
            .model
            .as_ref()
            .and_then(|id| models.iter().find(|m| &m.id == id))
            .or_else(|| models.iter().find(|m| m.is_default))
        {
            verdict.task.tier = verdict.task.tier.max(router::model_tier(selected));
            verdict.task.effort = verdict
                .task
                .effort
                .max(router::effort_grade(&options.effort));
        }
    }
    for f in &options.attachments {
        context::attachment_path(Path::new(&session.workspace), f)?;
    }
    // Orchestra: complex Auto work is planned and split across agents.
    let mut orchestra_nogit = false;
    if options.audit_of.is_none()
        && requested.is_none()
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
    // Large Auto tasks that write files run in the conversation's own worktree, and so does
    // any task that names an existing branch or pull request to work on.
    let lower = text.to_lowercase();
    let analysis_only = (deep_research
        || ["dimostra ", "prove ", "deriva ", "derive ", "calcola "]
            .iter()
            .any(|prefix| lower.trim_start().starts_with(prefix)))
        && !lower.split(|c: char| !c.is_alphanumeric()).any(|word| {
            [
                "implement",
                "implementa",
                "edit",
                "modifica",
                "apply",
                "applica",
                "publish",
                "pubblica",
                "scrivi",
                "write",
                "correggi",
                "fix",
                "commit",
            ]
            .contains(&word)
        });
    let big_task = !analysis_only
        && crate::task_worktree::wanted(verdict.complex, verdict.task.tier, options.access());
    let named = (options.access() != AccessMode::ReadOnly)
        .then(|| crate::task_worktree::reference(&text))
        .flatten();
    let task_worktree = options.mode == "auto"
        && options.audit_of.is_none()
        && requested.is_none()
        && plan.strategy != "orchestra"
        && (big_task || named.is_some());
    let (tx, rx) = watch::channel(false);
    let reserved = reservation.is_some();
    let rx = reservation.unwrap_or(rx);
    {
        let mut jobs = core.jobs.lock().map_err(|_| "Jobs lock failed")?;
        if reserved {
            if *rx.borrow() || !jobs.contains_key(&id) {
                return Err("Cancelled".into());
            }
        } else {
            if jobs.contains_key(&id) {
                return Err("This conversation already has a running turn".into());
            }
            jobs.insert(id.clone(), tx);
        }
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
    user.activity.push(crate::classify::record(&verdict));
    user.activity.push(Activity {
        kind: "run-options".into(),
        text: serde_json::to_string(&options).map_err(|e| e.to_string())?,
    });
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
    let core = core.clone();
    let returned = plan.clone();
    let auto_answer = options.mode == "auto" && options.access() != AccessMode::Autopilot;
    let task_app = app.clone();
    let task_rx = rx.clone();
    let task_core = core.clone();
    let task_id = id.clone();
    let task = tokio::spawn(crate::interactions::AUTO_ANSWER.scope(auto_answer, async move {
        let mut cancelled = rx.clone();
        let permit = tokio::select! {biased;_=cancelled.changed()=>None,p=core.slots.clone().acquire_owned()=>p.ok()};
        let last_id;
        let status = {
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
                    let mut current = session.clone();
                    let mut prepared = Ok(());
                    if task_worktree {
                        match crate::task_worktree::create(
                            &app,
                            &core,
                            &session,
                            &mut output,
                            &text,
                            options.access(),
                            named.clone(),
                            big_task,
                            rx.clone(),
                        )
                        .await
                        {
                            Ok(Some(moved)) => current = moved,
                            Ok(None) => {}
                            Err(e) if e == "Cancelled" => prepared = Err(e),
                            Err(e) => note(
                                &app,
                                &id,
                                &mut output,
                                "error",
                                format!("Task worktree not created; working in the project folder: {e}"),
                            ),
                        }
                    }
                    match prepared {
                        Err(e) => Err(e),
                        Ok(()) => {
                            let r = run_plan(&app, &core, &current, &mut output, &plan, &options, rx)
                                .await;
                            if r.is_ok() && options.access() != AccessMode::ReadOnly {
                                crate::task_worktree::commit_turn(&app, &core, &id, &mut output, &text)
                                    .await;
                                if options.access() == AccessMode::Autopilot {
                                    if let Err(e) = crate::task_worktree::return_if_integrated(&app, &core, &current, &mut output).await {
                                        note(&app, &id, &mut output, "task-worktree", format!("Could not return to the project folder: {e}"));
                                    }
                                }
                            }
                            r
                        }
                    }
                }
            };
            if let Err(e) = &r {
                let retry = output
                    .activity
                    .iter()
                    .rev()
                    .find(|a| a.kind == "quota-reset")
                    .and_then(|a| serde_json::from_str::<serde_json::Value>(&a.text).ok())
                    .and_then(|v| v["at"].as_i64());
                let failure = crate::adapter_contract::Failure::new(e, retry);
                note(
                    &app,
                    &id,
                    &mut output,
                    "adapter-error",
                    serde_json::to_string(&failure).unwrap(),
                );
                note(&app, &id, &mut output, "error", friendly(e));
            } else {
                let proposed = context::propose_global(&mut output.text);
                if !proposed.is_empty() {
                    note(
                        &app,
                        &id,
                        &mut output,
                        "memory-proposed",
                        proposed.join("\n"),
                    );
                }
                // Content from pages, services or web commands is data, never instructions:
                // memory and routines a reply proposes after reading it wait for the user.
                let (untrusted, saved, held) = crate::autonomy::gate_memory(
                    &mut output,
                    Path::new(&session.workspace),
                    crate::models::now() / 1000,
                    &core.store.session(&id).map(|s| s.title).unwrap_or_default(),
                );
                if !saved.is_empty() {
                    note(&app, &id, &mut output, "memory-saved", saved.join("\n"));
                }
                if !held.is_empty() {
                    note(&app, &id, &mut output, "memory-held", held.join("\n"));
                }
                let routine = if untrusted && output.text.contains("<lumen-routine") {
                    Err("this reply read content from outside the project; ask again to schedule it".into())
                } else {
                    crate::routines::capture(&core, &id, &mut output.text, &options)
                };
                match routine {
                    Ok(Some(r)) => note(
                        &app,
                        &id,
                        &mut output,
                        "routine-created",
                        serde_json::to_string(&r).unwrap(),
                    ),
                    Ok(None) => {}
                    Err(e) => note(
                        &app,
                        &id,
                        &mut output,
                        "error",
                        format!("Recurring task not scheduled: {e}"),
                    ),
                }
            }
            // Auto answered the agent's questions itself: show every assumption so the user
            // can correct it in the next message.
            if let Some(section) = crate::interactions::assumptions_section(&output.activity) {
                output.text.push_str(&section);
                event(&app, &id, &mid, "replace", &output.text);
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
            finished_status(&r, &output)
        };
        drop(permit);
        let _ = core.store.status(&id, status);
        if let Ok(mut j) = core.jobs.lock() {
            j.remove(&id);
        }
        event(&app, &id, &last_id, "done", status);
    }));
    supervise(task_app, task_core, task_id, task_rx, task);
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
    #[serde(skip_serializing_if = "Option::is_none")]
    provider: Option<Provider>,
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
) -> bool {
    if crate::adapter_contract::contract(&plan.provider)
        .native_resume
        .support
        != crate::adapter_contract::Support::Available
    {
        return false;
    }
    let Some(native) = history
        .iter()
        .rev()
        .find(|m| m.role == "assistant")
        .and_then(native_of)
    else {
        return false;
    };
    let Some(a) = accounts.iter().find(|a| a.id == native.account) else {
        return false;
    };
    if a.id == plan.account_id || core.limits.cooling(&a.id, now()) {
        return false;
    }
    use_writer(plan, a, accounts);
    true
}

/// Quota may change the account after model selection. Cross-provider balancing can
/// only trade for a model of the same tier with no reduction in effort; Budget also lets
/// OpenCode Go compete with its curated tier model.
async fn balance_model_account(
    core: &Core,
    workspace: &str,
    accounts: &[Account],
    usage: &[AccountUsage],
    cross_provider: bool,
    plan: &mut Plan,
    selected: ModelInfo,
) -> ModelInfo {
    let at = now();
    let Some(current) = usage.iter().find(|u| u.account_id == plan.account_id) else {
        return selected;
    };
    let effort = resolve_effort(&selected, &plan.effort);
    let tier = if plan.provider == Provider::Opencode {
        crate::opencode::tier_of_model(&selected.id)
            .unwrap_or_else(|| router::model_tier(&selected))
    } else {
        router::model_tier(&selected)
    };
    let pinned = |provider: &Provider| {
        core.settings.lock().ok().and_then(|s| {
            s.emergency_models
                .get(&provider.id())
                .filter(|m| !m.is_empty())
                .cloned()
        })
    };
    let mut alternatives = accounts
        .iter()
        .filter(|a| {
            a.id != plan.account_id
                && (a.provider == plan.provider
                    || (cross_provider
                        && matches!(
                            a.provider,
                            Provider::Claude | Provider::Gpt | Provider::Opencode
                        )
                        && matches!(
                            plan.provider,
                            Provider::Claude | Provider::Gpt | Provider::Opencode
                        )))
        })
        .filter_map(|a| {
            let other = usage.iter().find(|u| u.account_id == a.id)?;
            (router::quota_compare(other, current, at)? == std::cmp::Ordering::Less)
                .then_some((a, other))
        })
        .collect::<Vec<_>>();
    alternatives.sort_by(|(a, x), (b, y)| {
        router::quota_compare(x, y, at)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| (a.provider != plan.provider).cmp(&(b.provider != plan.provider)))
    });
    for (account, _) in alternatives {
        let Ok(models) = models_in(core, account, false, None, workspace).await else {
            continue;
        };
        let same = account.provider == plan.provider;
        let candidate = if same {
            models
                .iter()
                .find(|m| m.id == selected.id && resolve_effort(m, &plan.effort) == effort)
        } else if account.provider == Provider::Opencode {
            crate::opencode::tier_model(&models, tier, pinned(&account.provider).as_deref()).filter(
                |m| {
                    router::effort_grade(&resolve_effort(m, &plan.effort))
                        >= router::effort_grade(&effort)
                },
            )
        } else {
            models
                .iter()
                .filter(|m| {
                    router::model_tier(m) == tier
                        && !matches!(m.id.as_str(), "auto" | "default")
                        && router::effort_grade(&resolve_effort(m, &plan.effort))
                            >= router::effort_grade(&effort)
                })
                .max_by_key(|m| m.is_default)
        };
        if let Some(model) = candidate {
            use_writer(plan, account, accounts);
            plan.reason.push_str(if same {
                " · more quota for the same model"
            } else {
                " · more quota at the same model tier"
            });
            return model.clone();
        }
    }
    selected
}

fn confirmed_logout(provider: &Provider, success: bool, stdout: &[u8], stderr: &[u8]) -> bool {
    if success {
        return false;
    }
    match provider {
        Provider::Claude => serde_json::from_slice::<serde_json::Value>(stdout)
            .ok()
            .is_some_and(|v| v["loggedIn"] == false && v["authMethod"] == "none"),
        Provider::Gpt => [stdout, stderr]
            .iter()
            .any(|s| std::str::from_utf8(s).is_ok_and(|s| s.trim() == "Not logged in")),
        _ => false,
    }
}

/// No Claude or ChatGPT account could run a turn right now: none configured, in cooldown, or
/// with fresh quota data showing exhaustion. Missing or stale data never counts as exhausted.
fn primary_unavailable(core: &Core, accounts: &[Account], usage: &[AccountUsage], at: i64) -> bool {
    !accounts.iter().any(|a| {
        matches!(a.provider, Provider::Claude | Provider::Gpt)
            && !core.limits.cooling(&a.id, at)
            && !usage
                .iter()
                .find(|u| u.account_id == a.id)
                .and_then(|u| router::usage_pressure(u, at))
                .is_some_and(|p| p >= 1000.0)
    })
}

/// D1: when Auto must run a weaker model because every account that could serve the task is
/// out of quota, say which provider is blocked and when its quota resets, so the user can also
/// wait and resend. `None` means the weaker model is the honest best choice, not a quota trade.
async fn preferred_quota_hint(core: &Core, request: &str, target: i32) -> Option<String> {
    let preferred = router::provider_for(request, target);
    if !matches!(preferred, Provider::Claude | Provider::Gpt) {
        return None;
    }
    let accounts = core.store.accounts().ok()?;
    let usage = crate::read_usage(core).ok()?;
    let at = now();
    let blocked = accounts
        .iter()
        .filter(|a| a.provider == preferred)
        .any(|a| {
            core.limits.cooling(&a.id, at)
                || usage
                    .iter()
                    .find(|u| u.account_id == a.id)
                    .and_then(|u| router::usage_pressure(u, at))
                    .is_some_and(|p| p >= 1000.0)
        });
    if !blocked {
        return None;
    }
    // Only offer the wait when a blocked account really has a model for this difficulty.
    for a in accounts.iter().filter(|a| a.provider == preferred) {
        let Ok(models) = models_in(core, a, false, None, "").await else {
            continue;
        };
        if !models.iter().any(|m| router::model_tier(m) >= target) {
            continue;
        }
        let resets = usage.iter().find(|u| u.account_id == a.id).and_then(|u| {
            u.windows
                .iter()
                .map(|w| w.resets_at)
                .filter(|r| *r > at)
                .min()
        });
        let name = match preferred {
            Provider::Claude => "Claude",
            _ => "ChatGPT",
        };
        return Some(match resets {
            Some(at_reset) => format!(
                " The {name} account has a suitable model, but its quota is exhausted and resets {}.",
                in_about(at_reset - at)
            ),
            None => format!(" The {name} account has a suitable model, but its quota is exhausted."),
        });
    }
    None
}

/// Coarse, honest phrasing of a waiting time; the exact clock lives in the quota panel.
fn in_about(ms: i64) -> String {
    let minutes = (ms / 60_000).max(1);
    if minutes < 60 {
        format!("in about {minutes} minutes")
    } else if minutes < 48 * 60 {
        format!("in about {} hours", (minutes as f64 / 60.0).round() as i64)
    } else {
        format!("in about {} days", (minutes as f64 / 1440.0).round() as i64)
    }
}

fn use_writer(plan: &mut Plan, a: &Account, accounts: &[Account]) {
    plan.provider = a.provider.clone();
    plan.account_id = a.id.clone();
    if plan.strategy != "single" {
        plan.partner_account = router::review_partner(accounts, a).map(|b| b.id.clone());
        if plan.partner_account.is_none() {
            plan.strategy = "single".into();
        }
    }
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
    pub context_revision: Option<String>,
}

/// A native session to continue, and the prompt that carries only the new request.
#[derive(Debug, Clone)]
pub(crate) struct Resume {
    pub session: String,
    pub prompt: String,
    pub replay: Option<RunOptions>,
}

pub(crate) fn native_of(m: &Message) -> Option<Native> {
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
    if crate::adapter_contract::contract(&account.provider)
        .native_resume
        .support
        != crate::adapter_contract::Support::Available
    {
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
    (native.account == account.id
        && Path::new(&native.workspace) == Path::new(workspace)
        && !native.session.is_empty())
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
    let previous_native = messages
        .iter()
        .rev()
        .filter(|m| !exclude.contains(&m.id.as_str()))
        .find_map(native_of);
    let owned = crate::workstate::owned_context(core, s).ok()?;
    let old_context = previous_native
        .as_ref()
        .and_then(|n| n.context_revision.as_ref());
    let changed = old_context != Some(&owned.revision);
    let delta = if changed && owned.text.is_empty() && old_context.is_some() {
        "\nLumen's current project/check context is empty. Previously supplied notes/checks are not current evidence; do not infer passing validation.\n".to_string()
    } else if changed {
        format!(
            "{}{}",
            if old_context.is_some() {
                "\nThe following replaces Lumen's previously supplied project/check context. Omitted checks are not current evidence; do not infer passing validation.\n"
            } else {
                ""
            },
            owned.text
        )
    } else {
        String::new()
    };
    Some(Resume {
        session,
        replay: Some(options.clone()),
        prompt: format!(
            "[Lumen: {}]\n{delta}{text}{attached}",
            permission_text(options.access())
        ),
    })
}

// Stage keeps the app/session/output references explicit across asynchronous worker calls.
#[allow(clippy::too_many_arguments)]
pub async fn stage(
    app: &crate::host::EventHub,
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

/// A stage that continues `resume` when given. A missing session is replayed once
/// only before observed work; completed effects must never be blindly repeated.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn stage_with(
    app: &crate::host::EventHub,
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
    let mut replay_guard = ResumeReplayGuard::default();
    stage_observed(
        app,
        core,
        s,
        out,
        a,
        prompt,
        model,
        effort,
        access,
        cancel,
        visible,
        resume,
        &mut replay_guard,
        None,
        &[],
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn stage_observed(
    app: &crate::host::EventHub,
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
    replay_guard: &mut ResumeReplayGuard,
    mut progress: Option<&mut AutopilotProgress>,
    images: &[PathBuf],
) -> Result<String, String> {
    let config = config_dir(core, a);
    let mut trace = crate::execution::Trace::new(a.provider.binary());
    trace.publish(app, &s.id, out, true);
    if visible {
        let _ = core.store.save_message(&s.id, out);
    }
    let capabilities = crate::adapter_contract::contract(&a.provider);
    let resume = resume.filter(|_| {
        capabilities.native_resume.support == crate::adapter_contract::Support::Available
    });
    let native_skills =
        capabilities.native_skills.support == crate::adapter_contract::Support::Available;
    let stripped = if native_skills {
        without(prompt, SKILLS_OPEN, SKILLS_CLOSE)
    } else {
        prompt.to_owned()
    };
    let stripped = if !matches!(
        a.provider,
        Provider::Ollama | Provider::Opencode | Provider::Cursor | Provider::Droid
    ) {
        stripped
    } else {
        without(&stripped, GRAPH_OPEN, GRAPH_CLOSE)
    };
    let prompt = stripped.as_str();
    let full_prompt = prompt;
    let prompt = resume.map(|r| r.prompt.as_str()).unwrap_or(prompt);
    let completion_prompt;
    let prompt = if progress.is_some() {
        completion_prompt = format!("{prompt}\n\n{AUTOPILOT_COMPLETION}");
        &completion_prompt
    } else {
        prompt
    };
    let workspace = PathBuf::from(&s.workspace);
    let context_revision = Some(crate::workstate::owned_context(core, s)?.revision);
    let mut checkpoint = std::time::Instant::now();
    if !out.activity.iter().any(|a| a.kind == "workspace-root") {
        note(app, &s.id, out, "workspace-root", s.workspace.clone());
    }
    let models = match models_in(core, a, false, Some(cancel.clone()), &s.workspace).await {
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
    let mut measured = Measured {
        provider: Some(a.provider.clone()),
        ..Default::default()
    };
    let mut native: Option<String> = None;
    let retry_cancel = cancel.clone();
    let prefs = crate::settings::AppSettings::load(&core.root);
    let rlm_sources = if (a.provider.is_api() && prefs.code_mode == crate::settings::CodeMode::On)
        || (a.provider == Provider::Ollama && prefs.ollama_code_models.contains(&selected.id))
        || crate::code_mcp::enabled(&prefs, &a.provider)
    {
        let messages = core.store.messages(&s.id)?;
        let attachments = messages
            .iter()
            .rev()
            .find(|m| m.role == "user")
            .and_then(|m| m.activity.iter().find(|a| a.kind == "run-options"))
            .and_then(|a| serde_json::from_str::<RunOptions>(&a.text).ok())
            .map(|options| options.attachments)
            .unwrap_or_default();
        let accounts = core
            .store
            .accounts()?
            .into_iter()
            .filter(|a| !core.limits.cooling(&a.id, now()))
            .collect();
        let mut sources = crate::rlm::Sources::new(accounts);
        sources.capture(&workspace, &attachments, &messages);
        Some(sources)
    } else {
        None
    };
    let mut awaiting_input = false;
    let result = providers::execute_with_images(
        providers::Invocation {
            rlm: rlm_sources.as_ref(),
            automatic: a.provider.is_api()
                && core
                    .store
                    .messages(&s.id)?
                    .iter()
                    .rev()
                    .find(|m| m.role == "user")
                    .and_then(|m| m.activity.iter().find(|a| a.kind == "run-options"))
                    .and_then(|a| serde_json::from_str::<RunOptions>(&a.text).ok())
                    .is_some_and(|o| o.mode == "auto"),
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
                    worker: None,
                }),
            resume: resume.map(|r| r.session.as_str()),
        },
        images,
        cancel,
        |activity| {
            // Observe before lifecycle events are discarded or activity is compacted.
            replay_guard.observe(&activity);
            if let Some(progress) = progress.as_deref_mut() {
                progress.observe(&activity);
            }
            // Timing signals carry no provider content and do not add IPC/SQLite writes.
            // The existing snapshot checkpoint/finish publishes the accumulated offsets.
            if matches!(activity.kind.as_str(), "latency" | "code-metrics") {
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
            if matches!(activity.kind.as_str(), "tool-start" | "tool-end") {
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
            if visible && input_needed && awaiting_input != trace.awaiting_input() {
                awaiting_input = trace.awaiting_input();
                let status = if awaiting_input { "waiting" } else { "running" };
                let _ = core.store.status(&s.id, status);
                event(app, &s.id, &out.id, "status", status);
            }
        },
    )
    .await;
    trace.finish(&result);
    trace.publish(app, &s.id, out, true);
    if let (Some(_), Err(e)) = (resume, &result) {
        if replay_guard.allows(e, &out.text, *retry_cancel.borrow())
            && !resume
                .and_then(|r| r.replay.as_ref())
                .is_some_and(|o| o.resume_only)
        {
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
            return Box::pin(stage_observed(
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
                replay_guard,
                progress,
                images,
            ))
            .await;
        }
    }
    let result = result.map_err(|error| {
        if resume.is_some() && resume_unavailable(&error) && replay_guard.work_observed {
            format!("{error}. Lumen did not replay this request because provider activity was already observed; inspect the recorded work before retrying.")
        } else {
            error
        }
    });
    if result.is_ok() {
        core.limits.record_turn(&a.id, now());
    }
    // A quota interruption still belongs to a valid native session, including partial work.
    if (result.is_ok()
        || result
            .as_ref()
            .err()
            .is_some_and(|error| limits::is_rate_limit(error)))
        && (visible || resume.is_some())
    {
        // Codex may not repeat the id on resume; the continued session keeps its own.
        if let Some(session) = native.or_else(|| resume.map(|r| r.session.clone())) {
            out.activity.retain(|x| x.kind != "native");
            out.activity.push(Activity {
                kind: "native".into(),
                text: serde_json::to_string(&Native {
                    account: a.id.clone(),
                    session,
                    workspace: s.workspace.clone(),
                    context_revision,
                })
                .unwrap_or_default(),
            });
        }
    }
    result
}

/// Missing-session replay and quota fallback require no observed model work.
/// Unknown activity is deliberately treated as work, not assumed harmless.
#[derive(Default)]
pub(crate) struct ResumeReplayGuard {
    work_observed: bool,
}

impl ResumeReplayGuard {
    pub(crate) fn observe(&mut self, activity: &Activity) {
        if activity.kind == "error" && limits::is_rate_limit(&activity.text) {
            return;
        }
        if !matches!(
            activity.kind.as_str(),
            "pid"
                | "session"
                | "latency"
                | "code-metrics"
                | "quota"
                | "usage"
                | "native-usage"
                | "window"
                | "connector-status"
                | "api-cost"
                | "provider-cost"
        ) {
            self.work_observed = true;
        }
    }

    fn allows(&self, error: &str, text: &str, cancelled: bool) -> bool {
        self.before_work(text, cancelled) && resume_unavailable(error)
    }

    pub(crate) fn before_work(&self, text: &str, cancelled: bool) -> bool {
        !cancelled && !self.work_observed && text.trim().is_empty()
    }
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
    app: crate::host::EventHub,
    core: Arc<Core>,
    id: String,
    mut user: Message,
    work: F,
) -> Result<(), String>
where
    F: FnOnce(crate::host::EventHub, Arc<Core>, Session, Message, watch::Receiver<bool>) -> Fut
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
    let (task_app, task_core, task_id, task_rx) =
        (app.clone(), core.clone(), id.clone(), rx.clone());
    let task = tokio::spawn(async move {
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
        let status = finished_status(&result, &output);
        let _ = core.store.status(&id, status);
        if let Ok(mut j) = core.jobs.lock() {
            j.remove(&id);
        }
        event(&app, &id, &mid, "done", status);
    });
    supervise(task_app, task_core, task_id, task_rx, task);
    Ok(())
}

const AUTOPILOT_COMPLETION: &str = "Lumen Autopilot completion check. When the user asks for an action, use your native tools and finish it before replying. \
    Try an available workspace tool before reporting that access is missing. Recover from nonessential tool errors. \
    For UI issues, inspect any supplied screenshots and identify the exact element and interaction state before editing. \
    Verify the reported behavior through the relevant interaction when feasible, and say what you actually observed. \
    A source change or a Git diff alone does not verify a visual result. If you believe the requested state already exists, verify it against the user's evidence before closing the task. \
    Put exactly one of these markers at the end of your final answer: <!-- lumen-autopilot: done --> when the requested work is delivered (including questions and verified no-ops), \
    even if a check could not run here or a limit remains: then name what was not verified or is missing, and why. \
    <!-- lumen-autopilot: continue --> if authorized work remains that you can do yourself now; before that marker, state what remains and the next action. \
    <!-- lumen-autopilot: blocked --> only when the requested work itself cannot proceed without something only the user can provide or do outside this conversation; say exactly what is needed. \
    Ask decisions with the structured question tool instead of stopping.";

const AUTOPILOT_CONTINUE: &str = "Continue the current user request from this native session. Use existing tool results and take the next unfinished step. Reuse passing checks unless their inputs changed. Preserve user decisions, approval outcomes and existing changes. Never repeat external or irreversible actions with an uncertain outcome. If a user decision is needed, ask using the structured question tool. Explain an actual blocker if you cannot proceed safely.";

/// Activity kind for an Autopilot run that stopped on its own terms: the assistant explained a
/// blocker, the run stopped advancing, or it reached the attempt cap. The reply and its work are
/// kept; the session ends `partial` for the user to review, not as an error.
const AUTOPILOT_STOPPED: &str = "autopilot-stopped";

/// Autonomous loops are always bounded: completion markers can be missing and models can stall.
const AUTOPILOT_MAX_ATTEMPTS: usize = 10;

fn autopilot_stop(
    app: &crate::host::EventHub,
    s: &Session,
    out: &mut Message,
    account: Account,
    reason: &str,
) -> (String, Account) {
    note(
        app,
        &s.id,
        out,
        AUTOPILOT_STOPPED,
        serde_json::json!({ "reason": reason }).to_string(),
    );
    (out.text.clone(), account)
}

/// Session status when a turn ends.
fn finished_status(result: &Result<(), String>, output: &Message) -> &'static str {
    match result {
        Ok(()) if output.activity.iter().any(|a| a.kind == AUTOPILOT_STOPPED) => "partial",
        Ok(()) => "completed",
        Err(e) if e == "Cancelled" => "interrupted",
        Err(_) => "error",
    }
}

#[derive(Default)]
struct AutopilotProgress {
    seen: std::collections::HashSet<u64>,
    advanced: bool,
    retried_stall: bool,
}

impl AutopilotProgress {
    fn observe(&mut self, activity: &Activity) {
        if !matches!(activity.kind.as_str(), "tool-end" | "interaction-result") {
            return;
        }
        let Ok(row) = serde_json::from_str::<serde_json::Value>(&activity.text) else {
            return;
        };
        if let Some(id) = row["evidence"].as_u64() {
            self.advanced |= self.seen.insert(id);
        } else if row["kind"] == "question" && row["cancelled"] != true {
            self.advanced = true;
        }
    }
}

#[derive(PartialEq, Eq)]
enum AutopilotOutcome {
    Done,
    Blocked,
    Continue,
    Unconfirmed,
}

fn autopilot_outcome(text: &str) -> (&str, AutopilotOutcome) {
    let text = text.trim_end();
    let Some((body, status)) = text.rsplit_once("<!-- lumen-autopilot: ") else {
        return (text, AutopilotOutcome::Unconfirmed);
    };
    let outcome = match status {
        "done -->" if !body.trim().is_empty() => AutopilotOutcome::Done,
        "blocked -->" if !body.trim().is_empty() => AutopilotOutcome::Blocked,
        "continue -->" if !body.trim().is_empty() => AutopilotOutcome::Continue,
        "done -->" | "blocked -->" | "continue -->" => AutopilotOutcome::Unconfirmed,
        _ => return (text, AutopilotOutcome::Unconfirmed),
    };
    (body.trim_end(), outcome)
}

/// On a rate limit, cool the account; switch once only before observed work.
#[allow(clippy::too_many_arguments)]
async fn stage_fallback(
    app: &crate::host::EventHub,
    core: &Core,
    s: &Session,
    out: &mut Message,
    a: &Account,
    prompt: &str,
    model: Option<&str>,
    effort: &str,
    options: &RunOptions,
    cancel: watch::Receiver<bool>,
    single: bool,
    resume: Option<&Resume>,
) -> Result<(String, Account), String> {
    let access = options.access();
    let models = models_in(core, a, false, Some(cancel.clone()), &s.workspace).await?;
    let target = model
        .and_then(|id| models.iter().find(|m| m.id == id))
        .map(router::model_tier)
        .unwrap_or(1);
    let mut current = a.clone();
    let mut current_model = model.map(str::to_owned);
    let mut current_effort = effort.to_owned();
    let mut attempted = std::collections::HashSet::new();
    let mut first = true;
    let mut continuation = None;
    let mut progress = AutopilotProgress::default();
    let mut checked_attempts = 0;
    let mut partial = String::new();
    loop {
        attempted.insert(current.id.clone());
        let mut guard = ResumeReplayGuard::default();
        let check_completion = single
            && access == AccessMode::Autopilot
            && current.provider == Provider::Gpt
            && core.approvals.is_some()
            && options.audit_of.is_none();
        let replay = if !first && continuation.is_none() && prompt.is_empty() {
            if let Some(options) = resume.and_then(|r| r.replay.as_ref()) {
                Some(build_prompt(core, s, &[&out.id], options).await?.0)
            } else {
                None
            }
        } else {
            None
        };
        let images = if continuation.is_none()
            && current.provider == Provider::Gpt
            && core.approvals.is_some()
        {
            context::image_attachments(Path::new(&s.workspace), &options.attachments)?
        } else {
            Vec::new()
        };
        let result = stage_observed(
            app,
            core,
            s,
            out,
            &current,
            replay.as_deref().unwrap_or(prompt),
            current_model.as_deref(),
            &current_effort,
            access,
            cancel.clone(),
            true,
            continuation.as_ref().or(if first { resume } else { None }),
            &mut guard,
            check_completion.then_some(&mut progress),
            &images,
        )
        .await;
        first = false;
        let error = match result {
            Ok(text) if check_completion => {
                checked_attempts += 1;
                let (body, outcome) = autopilot_outcome(&text);
                out.text = if body.is_empty() {
                    std::mem::take(&mut partial)
                } else {
                    body.to_owned()
                };
                event(app, &s.id, &out.id, "replace", &out.text);
                if outcome == AutopilotOutcome::Done {
                    return Ok((out.text.clone(), current));
                }
                // A Stop wins over any stop reason the assistant gave.
                if *cancel.borrow() {
                    return Err("Cancelled".into());
                }
                // The assistant explained why it stopped: its reply is the outcome, not a
                // Lumen or provider failure.
                if outcome == AutopilotOutcome::Blocked {
                    return Ok(autopilot_stop(app, s, out, current, "blocked"));
                }
                let advanced = std::mem::take(&mut progress.advanced);
                if !advanced && progress.retried_stall {
                    return Ok(autopilot_stop(app, s, out, current, "stalled"));
                }
                if checked_attempts >= AUTOPILOT_MAX_ATTEMPTS {
                    return Ok(autopilot_stop(app, s, out, current, "limit"));
                }
                progress.retried_stall = !advanced;
                let mut next = continue_reply(
                    out,
                    &current,
                    &s.workspace,
                    if advanced {
                        AUTOPILOT_CONTINUE.into()
                    } else {
                        format!("{AUTOPILOT_CONTINUE} The previous attempt showed no new tool results. Use the existing evidence to finish, try a different step, or report what prevents further work.")
                    },
                )
                .ok_or(
                    "Autopilot could not confirm completion and the native session is unavailable.",
                )?;
                let mut resume_options = options.clone();
                resume_options.resume_only = true;
                next.replay = Some(resume_options);
                note(
                    app,
                    &s.id,
                    out,
                    "resume",
                    "Autopilot is continuing unfinished work in the same Codex session.".into(),
                );
                core.store.save_message(&s.id, out)?;
                partial = std::mem::take(&mut out.text);
                event(app, &s.id, &out.id, "replace", "");
                continuation = Some(next);
                continue;
            }
            Ok(text) => return Ok((text, current)),
            Err(e) => e,
        };
        if continuation.is_some() && out.text.is_empty() {
            out.text = std::mem::take(&mut partial);
            event(app, &s.id, &out.id, "replace", &out.text);
        }
        if !limits::is_rate_limit(&error) {
            return Err(error);
        }
        let reset = crate::quota_resume::reset_at(core, &current.id, &error);
        core.limits.cool(&current.id, reset);
        if !limits::is_spend_limit(&error) {
            note(app, &s.id, out, "quota-reset", serde_json::json!({"at":reset,"accountId":current.id,"model":current_model,"effort":current_effort}).to_string());
        }
        core.store.save_message(&s.id, out)?;
        if continuation.is_some()
            || resume
                .and_then(|r| r.replay.as_ref())
                .is_some_and(|o| o.resume_only)
        {
            return Err(error);
        }
        if !guard.before_work(&out.text, *cancel.borrow()) {
            return Err(format!("{error}. Lumen did not replay: work or output was already observed. Inspect the recorded work before retrying."));
        }
        if !core
            .settings
            .lock()
            .map(|s| s.auto_fallback)
            .unwrap_or(true)
        {
            return Err(error);
        }
        let usage = crate::read_usage(core)?;
        let mut alternatives = core
            .store
            .accounts()?
            .into_iter()
            .filter(|b| {
                (!matches!(
                    b.provider,
                    Provider::Opencode | Provider::Cursor | Provider::Droid
                )) && (!b.provider.is_api() || crate::api::eligible(&core.root, b))
                    && !attempted.contains(&b.id)
                    && !core.limits.cooling(&b.id, now())
                    && !usage
                        .iter()
                        .find(|u| u.account_id == b.id)
                        .and_then(|u| router::usage_pressure(u, now()))
                        .is_some_and(|p| p >= 1000.0)
                    && (access != AccessMode::Ask
                        || crate::adapter_contract::contract(&b.provider)
                            .ask_first
                            .support
                            == crate::adapter_contract::Support::Available)
            })
            .collect::<Vec<_>>();
        alternatives.sort_by_key(|b| (b.provider.fallback_rank(), b.provider == current.provider));
        let mut next = None;
        for candidate in alternatives {
            if let Ok(models) = models_for(core, &candidate, false, Some(cancel.clone())).await {
                if let Some(selected) = router::select_tier(&models, target) {
                    next = Some((candidate, selected.clone()));
                    break;
                }
            }
            if *cancel.borrow() {
                return Err("Cancelled".into());
            }
        }
        let Some((candidate, selected)) = next else {
            return Err(format!(
                "{error}. No other available account can handle this request."
            ));
        };
        let next_effort = resolve_effort(&selected, effort);
        let warning =
            if router::model_tier(&selected) < target || candidate.provider == Provider::Ollama {
                " This task may need a stronger model; expect a possible drop in quality."
            } else {
                ""
            };
        let question = crate::interactions::Question {
            id: "switch".into(), header: "Provider limit".into(),
            question: format!("{} ({}) is out of credits or rate limited. Send the same request to {} · {} ({})?{}", current.provider.binary(), current.name, selected.name, if next_effort.is_empty() { "provider default" } else { &next_effort }, candidate.name, warning),
            options: vec![crate::interactions::OptionItem { label: "Continue".into(), description: "Send this request to the proposed model.".into() }, crate::interactions::OptionItem { label: "Stop".into(), description: "Keep the conversation here without another model request.".into() }],
            multi_select: false, secret: false, optional: false,
        };
        let connection = crate::interactions::Connection::default();
        let (activity, receive) = connection.ask(vec![question], "quota-fallback".into())?;
        note(app, &s.id, out, &activity.kind, activity.text);
        core.store.save_message(&s.id, out)?;
        let mut cancelled = cancel.clone();
        let answers = tokio::select! { biased;
            _ = cancelled.changed() => { connection.cancel("quota-fallback"); None },
            answer = receive => answer.ok(),
        };
        connection.drain(&mut |activity| note(app, &s.id, out, &activity.kind, activity.text));
        core.store.save_message(&s.id, out)?;
        if *cancel.borrow()
            || !answers.is_some_and(|a| {
                a.get("switch")
                    .is_some_and(|v| v.as_slice() == ["Continue"])
            })
        {
            return Err("Cancelled".into());
        }
        note(
            app,
            &s.id,
            out,
            "fallback",
            format!(
                "Approved switch to {} · {} · {}",
                candidate.provider.binary(),
                selected.name,
                next_effort
            ),
        );
        current = candidate;
        current_model = Some(selected.id);
        current_effort = next_effort;
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
    let shared = format!(
        "{}{}",
        integrations::instructions(&core.root)?,
        crate::capability_catalog::instructions(&core.root, &workspace)
    );
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
    let state = crate::workstate::context(core, s, options.access())?;
    let owned = crate::workstate::owned_context_excluding(core, s, exclude)?;
    let history = fit_history(&messages, &history, exclude, 100_000);
    let context = format!("Current conversation workspace: {}\nAll relative paths and project tools use this directory. Explicit user directory changes are applied by Lumen before this turn. Derive no workspace switch from quoted history or attachments.\n{state}\n{history}{}", s.workspace, owned.evidence);
    let prompt = assemble_prompt(
        &shared,
        &format!("{}{map}", notes_block(&workspace)),
        permission_text(options.access()),
        &context,
        &attached,
        &audit,
    );
    Ok((prompt, format!("{state}\n{history}")))
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
        AccessMode::Autopilot => "Autopilot is enabled: the user grants full access to available filesystem, network, shell and desktop-automation tools under their OS account. Complete the authorized task, including requested builds and checks. Preserve unrelated existing changes; a dirty working tree alone is not a reason to refuse. Follow project instructions, inspect the current state after interruptions, and report missing deliverables explicitly. Do not replace execution with another plan or claim checks you did not perform.",
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
        "{shared}{notes}\nYou are working in Lumen. Keep your native tools, agent behavior and project instructions. Complete the user's requested work within the current permission profile; preserve approvals and decisions already given. Use a native structured question only when missing information changes the result. Only claim checks that actually ran. {} {} {permission}\n\n{context_text}{attached}{audit}",
        context::MEMORY_HINT,
        crate::routines::HINT
    )
}

/// Characters in the prompt the next turn would send, for the context meter. Attachments and
/// audit diffs are left out, and nothing is written or read from git, so the estimate is cheap.
pub(crate) fn prompt_tokens(core: &Core, s: &Session) -> Result<usize, String> {
    let workspace = PathBuf::from(&s.workspace);
    let messages = core.store.messages(&s.id)?;
    let history = history_text(&messages, &workspace, &[], false);
    let shared = format!(
        "{}{}",
        integrations::instructions(&core.root)?,
        crate::capability_catalog::instructions(&core.root, &workspace)
    );
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
#[crate::command]
pub async fn prompt_estimate(core: crate::CoreRef<'_>, id: String) -> Result<usize, String> {
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

/// What a restore rewrites: the whole git checkout when there is one, else the folder.
async fn rewind_scope(s: &Session) -> std::path::PathBuf {
    match gitops::toplevel(Path::new(&s.workspace)).await {
        Some(top) => top,
        None => dunce::canonicalize(&s.workspace).unwrap_or_else(|_| s.workspace.clone().into()),
    }
}

/// Other conversations whose folder overlaps the restored checkout (either contains the
/// other, after T3 `CheckpointRestoreSafety`): running ones, or with `after`, any that
/// changed since then. Restoring a checkout restores it for all of them.
fn checkout_conflicts(core: &Core, s: &Session, scope: &Path, after: Option<i64>) -> Vec<String> {
    let others: Vec<(String, String, String)> = match after {
        Some(at) => core.store.sessions_updated_after(at).unwrap_or_default(),
        None => {
            let ids = core
                .jobs
                .lock()
                .map(|jobs| jobs.keys().cloned().collect::<Vec<_>>())
                .unwrap_or_default();
            ids.into_iter()
                .filter_map(|id| core.store.session(&id).ok())
                .map(|o| (o.id, o.title, o.workspace))
                .collect()
        }
    };
    others
        .into_iter()
        .filter(|(id, _, folder)| {
            id != &s.id && crate::autonomy::overlaps(scope, Path::new(folder))
        })
        .map(|(_, title, _)| title)
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
    app: &crate::host::EventHub,
    core: &Core,
    s: &Session,
    out: &mut Message,
    plan: &Plan,
    options: &RunOptions,
    mut cancel: watch::Receiver<bool>,
) -> Result<(), String> {
    if options.mode == "auto" {
        note(app, &s.id, out, "auto-assignment", "auto".into());
    }
    let writer = account(core, &plan.account_id)?;
    if options.mode == "auto" {
        let history = core.store.messages(&s.id)?;
        let request = history
            .iter()
            .rev()
            .find(|m| m.role == "user")
            .map(|m| m.text.as_str())
            .unwrap_or_default();
        let explicit = router::requested_provider(request).is_some();
        // The difficulty retained across this conversation, starting from the current request.
        let target = crate::classify::previous(&history)
            .map(|v| v.task.tier)
            .unwrap_or(0);
        let models = models_in(core, &writer, false, Some(cancel.clone()), &s.workspace).await?;
        if !explicit
            && plan
                .model
                .as_ref()
                .and_then(|id| models.iter().find(|m| &m.id == id))
                .is_some_and(|m| router::model_tier(m) < target)
        {
            // D1: when the weaker model is a quota trade, say which account is blocked and
            // when its quota resets, so waiting and resending is an informed alternative.
            let hint = if options.audit_of.is_none() {
                preferred_quota_hint(core, request, target).await
            } else {
                None
            };
            let question = format!(
                "No available model meets this task's retained difficulty.{} Continue with the proposed weaker model? Expect a possible drop in quality.",
                hint.unwrap_or_default()
            );
            let connection = crate::interactions::Connection::default();
            let question = crate::interactions::Question {
                id: "switch".into(),
                header: "Model capability".into(),
                question,
                options: vec![
                    crate::interactions::OptionItem {
                        label: "Continue".into(),
                        description: "Use the proposed model for this task.".into(),
                    },
                    crate::interactions::OptionItem {
                        label: "Stop".into(),
                        description: "Keep the task here without sending it.".into(),
                    },
                ],
                multi_select: false,
                secret: false,
                optional: false,
            };
            let (activity, receive) = connection.ask(vec![question], "model-downgrade".into())?;
            note(app, &s.id, out, &activity.kind, activity.text);
            core.store.save_message(&s.id, out)?;
            let answer = tokio::select! {biased;_=cancel.changed()=>{connection.cancel("model-downgrade");None},answer=receive=>answer.ok()};
            connection.drain(&mut |activity| note(app, &s.id, out, &activity.kind, activity.text));
            core.store.save_message(&s.id, out)?;
            if !answer.is_some_and(|a| {
                a.get("switch")
                    .is_some_and(|v| v.as_slice() == ["Continue"])
            }) {
                return Err("Cancelled".into());
            }
        }
    }

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
    if options.resume_only && resume.is_none() {
        return Err("Native session is unavailable. Automatic resumption sent no request; resume manually after inspecting the work.".into());
    }
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
        options,
        cancel.clone(),
        plan.strategy == "single",
        resume.as_ref(),
    )
    .await?;
    let writer_model = if writer.id == plan.account_id {
        plan.model.clone()
    } else {
        None
    };
    let verify_draft = async |out: &mut Message| {
        self_heal(
            app,
            core,
            s,
            out,
            &writer,
            writer_model.as_deref(),
            &plan.effort,
            options,
            baseline.clone(),
            cancel.clone(),
        )
        .await
    };
    out.text = draft;
    // A stopped Autopilot run hands control back to the user: no repair turns or audits after it.
    if out.activity.iter().any(|a| a.kind == AUTOPILOT_STOPPED) {
        return Ok(());
    }
    let mut verification = verify_draft(out).await?;
    if plan.strategy == "review" {
        draft = out.text.clone();
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
                let review_prompt = format!("You are auditing another assistant's work. Read-only: never edit files. Find bugs, architectural problems, risks and missing requirements, and propose concrete corrections. If the work is correct and complete, reply with exactly APPROVED and nothing else.\n\n{prompt}\n\nDraft answer:\n{draft}{diff}{verification}");
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
                out.text = stage_with(
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
                verification = verify_draft(out).await?;
                draft = out.text.clone();
            }
        } else {
            note(app,&s.id,out,"review-unavailable","No independent account is available; the writer's work is retained but was not cross-audited.".into());
            return Err("Independent review could not run on an available account".into());
        }
    }
    Ok(())
}

/// After file edits: run the project check and send failures back to the writer (max 3 attempts).
#[allow(clippy::too_many_arguments)]
pub(crate) async fn self_heal(
    app: &crate::host::EventHub,
    core: &Core,
    s: &Session,
    out: &mut Message,
    writer: &Account,
    model: Option<&str>,
    effort: &str,
    options: &RunOptions,
    baseline: Option<Arc<crate::codegraph::Graph>>,
    cancel: watch::Receiver<bool>,
) -> Result<String, String> {
    if !out.activity.iter().any(|a| a.kind == "orchestra")
        && !core
            .settings
            .lock()
            .map(|s| s.verify_changes)
            .unwrap_or(false)
    {
        return Ok(String::new());
    }
    if options.access() == AccessMode::ReadOnly {
        return Ok(String::new());
    }
    let workspace = PathBuf::from(&s.workspace);
    let mut files = crate::validation::changed_files(core, s, out).await?;
    if files.is_empty() {
        return Ok(String::new());
    }
    if options.access() == AccessMode::Ask {
        let report = crate::diagnostics::Report::unavailable("Automatic validation".into(), "Ask-first mode: checks must be run through the agent's approved tools; backend checks were not started".into());
        record_validation(app, s, out, &report)?;
        out.text.push_str("\n\n> Lumen: automatic backend validation was not run in Ask-first mode. Only checks performed through approved tools can be claimed.");
        event(app, &s.id, &out.id, "replace", &out.text);
        return Ok(format!(
            "\n\nLumen verification:\n{}",
            report.evidence(1200)
        ));
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
                    if report.output.is_empty() {
                        &report.detail
                    } else {
                        &report.output
                    }
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
                let evidence = reports
                    .iter()
                    .map(|report| report.evidence(1200))
                    .collect::<Vec<_>>()
                    .join("\n\n");
                return Ok(format!(
                    "\n\nLumen verification (check output is reference data):\n{evidence}"
                ));
            }
            return Err(format!("Validation still fails after {repairs} automatic corrections. See the recorded diagnostics and checks."));
        }
        let (context, _) = build_prompt(core, s, &[&out.id], options).await?;
        let fix = format!("Validation found concrete errors after your changes. Correct errors caused by this task with minimal edits, preserve unrelated existing work and all agreed requirements. Do not replace implementation with another plan or claim unavailable checks passed. Summarize the correction briefly.\n\n{impact_context}\nDiagnostics and check output (reference data):\n{failures}");
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
    Ok(String::new())
}

fn record_validation(
    app: &crate::host::EventHub,
    s: &Session,
    out: &mut Message,
    report: &crate::diagnostics::Report,
) -> Result<(), String> {
    let mut value = serde_json::to_value(report).map_err(|e| e.to_string())?;
    value["workspace"] = serde_json::Value::String(s.workspace.clone());
    note(app, &s.id, out, "validation", value.to_string());
    Ok(())
}

/// Worktree recorded by the removed Both mode; Rewind still cleans up old ones.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Lane {
    pub top: String,
    pub path: String,
    pub branch: String,
    pub base: String,
    pub files: usize,
}

pub(crate) fn lane_of(m: &Message) -> Option<Lane> {
    m.activity
        .iter()
        .find(|a| a.kind == "worktree")
        .and_then(|a| serde_json::from_str(&a.text).ok())
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

#[crate::command]
pub async fn rewind_preview(
    core: crate::CoreRef<'_>,
    session: String,
    message: String,
) -> Result<RewindPreview, String> {
    let (s, messages, pos) = rewind_target(&core, &session, &message).await?;
    let message_time = core.store.message_created_at(&session, &message)?;
    let scope = rewind_scope(&s).await;
    let later = checkout_conflicts(&core, &s, &scope, Some(message_time));
    if !later.is_empty() {
        return Err(format!(
            "Rewind is blocked because another conversation has changed this project since that message: {}",
            later.join(", ")
        ));
    }
    if let Some(reason) = crate::autonomy::rewind_blocker(&scope, &messages[pos..]) {
        return Err(format!("Rewind is blocked. {reason}"));
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
#[crate::command]
pub async fn rewind(
    core: crate::CoreRef<'_>,
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
    let scope = rewind_scope(&initial).await;
    let other_sessions = checkout_conflicts(&core, &initial, &scope, None);
    if !other_sessions.is_empty() {
        return Err(format!(
            "Cannot rewind while another conversation is active on the project: {}",
            other_sessions.join(", ")
        ));
    }
    let lock = workspace_lock(&core, &initial)?;
    let _guard = lock.lock().await;
    let other_sessions = checkout_conflicts(&core, &initial, &scope, None);
    if !other_sessions.is_empty() {
        return Err(format!(
            "Cannot rewind while another conversation is active on the project: {}",
            other_sessions.join(", ")
        ));
    }
    let (s, messages, pos) = rewind_target(&core, &session, &message).await?;
    let message_time = core.store.message_created_at(&session, &message)?;
    let later = checkout_conflicts(&core, &s, &scope, Some(message_time));
    if !later.is_empty() {
        return Err(format!(
            "Cannot rewind because another conversation has changed this project since that message: {}",
            later.join(", ")
        ));
    }
    if let Some(reason) = crate::autonomy::rewind_blocker(&scope, &messages[pos..]) {
        return Err(format!("Cannot rewind. {reason}"));
    }
    let tree = messages[pos..]
        .iter()
        .filter(|m| m.role == "user")
        .find_map(checkpoint_of);
    let mut restored = 0;
    if let (Some(t), Some(top)) = (&tree, gitops::toplevel(Path::new(&s.workspace)).await) {
        restored = gitops::restore(&top, t).await?;
    }
    crate::autonomy::append_log(
        &core.root,
        &session,
        &crate::autonomy::decision(
            crate::autonomy::Class::Reversible,
            "rollback",
            &match tree {
                Some(t) => format!(
                    "Restored {restored} files to checkpoint {t}; removed {} messages",
                    messages.len() - pos
                ),
                None => format!(
                    "No checkpoint: removed {} messages, files unchanged",
                    messages.len() - pos
                ),
            },
        ),
    );
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

/// Handoff & Compact: a new conversation seeded with goal, decisions, diff and next step.
/// A non-empty `summary` (for example a reviewed local draft) replaces the generated package.
#[crate::command]
pub async fn compact_session(
    core: crate::CoreRef<'_>,
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
    let mut state = crate::workstate::refresh(&core.store, &id, None)?;
    let list = |facts: &[crate::workstate::Fact], n: usize, none: &str| -> String {
        if facts.is_empty() {
            return none.to_owned();
        }
        facts[facts.len().saturating_sub(n)..]
            .iter()
            .map(|f| format!("- {}", clip(f.text.trim(), 600).replace('\n', "\n  ")))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let mut decisions = state.decisions.clone();
    if decisions.is_empty() {
        decisions = answers
            .iter()
            .map(|m| crate::workstate::Fact {
                source: m.id.clone(),
                text: m.text.clone(),
            })
            .collect();
    }
    let (files, diff) = match gitops::toplevel(Path::new(&s.workspace)).await {
        Some(top) => (
            gitops::changed_paths(&top).await.unwrap_or_default(),
            gitops::diff_head(&top, 20_000).await.unwrap_or_default(),
        ),
        None => (vec![], String::new()),
    };
    let git = if files.is_empty() {
        "No uncommitted changes.".to_string()
    } else {
        let mut out = files
            .iter()
            .take(40)
            .map(|(st, p)| format!("- `{p}` ({st})"))
            .collect::<Vec<_>>()
            .join("\n");
        if files.len() > 40 {
            out += &format!("\n- …and {} more", files.len() - 40);
        }
        if !diff.trim().is_empty() {
            out += &format!("\n\n```diff\n{}\n```", diff.trim());
        }
        out
    };
    let mut doubts = vec!["The previous assistant's private memory and native session are not carried over: trust only what is written here and re-read files before relying on them."];
    if state.checks.is_empty() {
        doubts.push("No checks were recorded, so nothing here is verified.");
    }
    if !state.pending.is_empty() {
        doubts.push("Open items may be partly done: confirm their state first.");
    }
    let package = match summary.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
        // An edited handoff keeps the notice: it never claims to carry private memory.
        Some(edited) => format!(
            "**Handoff from “{}”**\n\n{edited}\n\n**Uncertainties**\n\n- {}",
            s.title, doubts[0]
        ),
        None => format!(
            "**Handoff from “{}”**\n\n**Goal**\n\n{goal}\n\n**Constraints**\n\n{}\n\n**Decisions**\n\n{}\n\n**Done**\n\n{}\n\n**Still open**\n\n{}\n\n**Checks**\n\n{}\n\n**Files and Git state**\n\n{git}\n\n**Uncertainties**\n\n{}\n\n**Next step**\n\nContinue from the latest request: {latest}",
            s.title,
            list(&state.requirements, 8, "None recorded."),
            list(&decisions, 3, "None recorded."),
            list(&state.completed, 6, "Nothing recorded as done."),
            list(&state.pending, 8, "Nothing recorded as open."),
            list(&state.checks, 6, "None ran."),
            doubts.iter().map(|d| format!("- {d}")).collect::<Vec<_>>().join("\n"),
        ),
    };
    let mut fresh = core.store.create(s.workspace.clone())?;
    if let Some(w) = &s.workspace_id {
        fresh = core
            .store
            .bind_workspace(&fresh.id, Some(w), &s.workspace)?;
    }
    fresh = core.store.set_access_mode(&fresh.id, s.access_mode)?;
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
    #[test]
    fn quota_error_itself_is_not_observed_work() {
        let mut guard = super::ResumeReplayGuard::default();
        guard.observe(&crate::models::Activity {kind:"error".into(), text:"Your workspace is out of credits. Ask your workspace owner to refill in order to continue.".into()});
        assert!(guard.before_work("", false));
        guard.observe(&crate::models::Activity {
            kind: "tool".into(),
            text: "write".into(),
        });
        assert!(!guard.before_work("", false));
    }

    use super::*;

    #[tokio::test]
    async fn context_budget_actual_prompt_corpus() {
        let f = crate::test_support::Fixture::new();
        let mut results = vec![];
        for (name, marker, extension, history_turns) in [
            ("react", "REACT_REQUIREMENT", "tsx", 0),
            ("godot", "GODOT_REQUIREMENT", "gd", 12),
        ] {
            let root = f.directory(name);
            std::fs::write(
                root.join(format!("main.{extension}")),
                "ATTACHED_CONTRACT Ω",
            )
            .unwrap();
            std::fs::write(
                root.join("LUMEN.md"),
                "Project contract: preserve reset, labels and palette.",
            )
            .unwrap();
            let s = f
                .core
                .store
                .create(root.to_string_lossy().into_owned())
                .unwrap();
            let mut request = msg(&format!("{name}-request"), "user", vec![]);
            request.text = format!("Mantieni {marker}: reset, label and palette are approved. {} Preserve keyboard access Ω.", "Concrete implementation requirement. ".repeat(30));
            f.core.store.save_message(&s.id, &request).unwrap();
            for n in 0..history_turns {
                let mut answer = msg(&format!("{name}-answer-{n}"), "assistant", vec![]);
                answer.text = "Routine previous output. ".repeat(500);
                f.core.store.save_message(&s.id, &answer).unwrap();
            }
            if history_turns > 0 {
                let mut next = msg(&format!("{name}-continue"), "user", vec![]);
                next.text = "procedi".into();
                f.core.store.save_message(&s.id, &next).unwrap();
            }
            let mut options = crate::test_support::Fixture::options(AccessMode::Autopilot);
            options.attachments = vec![format!("main.{extension}"), format!("./main.{extension}")];
            let (prompt, worker) = build_prompt(&f.core, &s, &[], &options).await.unwrap();
            let state = crate::workstate::refresh(&f.core.store, &s.id, None).unwrap();
            assert!(state
                .requirements
                .iter()
                .any(|fact| fact.text.contains(marker)));
            assert!(prompt.contains("Autopilot"));
            assert!(prompt.contains("Preserve keyboard access Ω"));
            assert!(
                prompt.find("Project contract:").unwrap()
                    < prompt.find("<lumen-work-state>").unwrap()
            );
            assert!(worker.contains(marker));
            results.push(serde_json::json!({"case":name,"bytes":prompt.len(),"localTokens":context::estimate(&prompt),"workerBytes":worker.len(),"workerLocalTokens":context::estimate(&worker),"requirementCopies":prompt.matches(marker).count(),"attachmentCopies":prompt.matches("ATTACHED_CONTRACT").count(),"fullStateRequirements":state.requirements.len()}));
        }
        eprintln!(
            "CONTEXT_PROMPT_METRICS {}",
            serde_json::to_string(&results).unwrap()
        );
        for row in results {
            assert!(row["requirementCopies"].as_u64().unwrap() <= 2, "{row}");
            assert_eq!(row["attachmentCopies"], 1, "{row}");
        }
    }

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
    fn resume_replay_guard_rejects_work_partial_output_and_cancellation() {
        let mut clean = ResumeReplayGuard::default();
        for kind in [
            "pid",
            "session",
            "latency",
            "quota",
            "usage",
            "window",
            "connector-status",
        ] {
            clean.observe(&Activity {
                kind: kind.into(),
                text: "transport metadata".into(),
            });
        }
        for error in ["session not found", "Thread not found", "unknown session"] {
            assert!(clean.allows(error, "", false));
        }
        assert!(!clean.allows("session not found", "partial response", false));
        assert!(!clean.allows("session not found", "", true));
        assert!(!clean.allows("connection closed", "", false));
        assert!(clean.before_work("", false));
        assert!(!clean.before_work("partial quota response", false));
        assert!(!clean.before_work("", true));
        for kind in [
            "tool-start",
            "tool-end",
            "tool",
            "file",
            "thinking",
            "delta",
            "replace",
            "interaction",
            "interaction-result",
            "child",
            "browser",
            "future-protocol-event",
        ] {
            let mut guard = ResumeReplayGuard::default();
            guard.observe(&Activity {
                kind: kind.into(),
                text: String::new(),
            });
            guard.observe(&Activity {
                kind: "session".into(),
                text: "later metadata cannot reset the guard".into(),
            });
            assert!(
                !guard.allows("session not found", "", false),
                "{kind} allowed a replay"
            );
            assert!(
                !guard.before_work("", false),
                "{kind} allowed account fallback"
            );
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
    fn continuity_prefers_the_previous_native_account() {
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
        stick_to_previous(&f.core, &history, &[old.clone(), other], &mut plan);
        assert_eq!(plan.account_id, old.id);
    }

    fn catalog_with(name: &str) -> Vec<ModelInfo> {
        vec![ModelInfo {
            id: name.into(),
            name: name.into(),
            description: String::new(),
            efforts: vec![],
            default_effort: String::new(),
            is_default: true,
        }]
    }

    fn usage_json(path: &std::path::Path, account: &str, used: f64, resets_in: i64) {
        let at = now();
        let value = serde_json::json!([{
            "accountId": account,
            "updatedAt": at,
            "source": "test",
            "windows": [
                {"usedPercent": used, "windowMinutes": 300, "resetsAt": at + resets_in},
                {"usedPercent": used, "windowMinutes": 10080, "resetsAt": at + 48 * 3_600_000},
            ]
        }]);
        std::fs::write(path, value.to_string()).unwrap();
    }

    #[tokio::test]
    async fn quota_hint_names_the_blocked_provider_and_its_reset() {
        let f = crate::test_support::Fixture::new();
        let claude = f
            .core
            .store
            .add_account("Claude".into(), Provider::Claude)
            .unwrap();
        f.core.catalogs.lock().unwrap().insert(
            claude.id.clone(),
            (std::time::Instant::now(), catalog_with("opus")),
        );
        usage_json(&f.root.join("usage.json"), &claude.id, 100.0, 7_200_000);
        let hint =
            preferred_quota_hint(&f.core, "Progetta la nuova architettura del database", 2).await;
        assert_eq!(
            hint.as_deref(),
            Some(" The Claude account has a suitable model, but its quota is exhausted and resets in about 2 hours.")
        );
        // A healthy account, or a request that prefers another provider, stays silent.
        usage_json(&f.root.join("usage.json"), &claude.id, 40.0, 7_200_000);
        assert!(
            preferred_quota_hint(&f.core, "Progetta la nuova architettura del database", 2)
                .await
                .is_none()
        );
        usage_json(&f.root.join("usage.json"), &claude.id, 100.0, 7_200_000);
        assert!(
            preferred_quota_hint(&f.core, "Research primary sources and trade-offs", 2)
                .await
                .is_none()
        );
    }

    #[tokio::test]
    async fn quota_hint_waits_only_for_an_account_that_can_serve_the_task() {
        let f = crate::test_support::Fixture::new();
        let claude = f
            .core
            .store
            .add_account("Claude".into(), Provider::Claude)
            .unwrap();
        f.core.catalogs.lock().unwrap().insert(
            claude.id.clone(),
            (std::time::Instant::now(), catalog_with("haiku")),
        );
        usage_json(&f.root.join("usage.json"), &claude.id, 100.0, 7_200_000);
        // Exhausted, but its catalog cannot serve a tier-two task: no reason to wait.
        assert!(
            preferred_quota_hint(&f.core, "Progetta la nuova architettura del database", 2)
                .await
                .is_none()
        );
        // Exhausted with a suitable model but no readable reset time: ask without a time.
        f.core.catalogs.lock().unwrap().insert(
            claude.id.clone(),
            (std::time::Instant::now(), catalog_with("opus")),
        );
        f.core.limits.cool(&claude.id, now() + 60_000);
        let value = serde_json::json!([{
            "accountId": claude.id,
            "updatedAt": now(),
            "source": "test",
            "windows": [
                {"usedPercent": 100.0, "windowMinutes": 300, "resetsAt": now() - 1},
            ]
        }]);
        std::fs::write(f.root.join("usage.json"), value.to_string()).unwrap();
        assert_eq!(
            preferred_quota_hint(&f.core, "Progetta la nuova architettura del database", 2)
                .await
                .as_deref(),
            Some(" The Claude account has a suitable model, but its quota is exhausted.")
        );
    }

    #[test]
    fn waiting_time_is_phased_honestly() {
        assert_eq!(in_about(5 * 60_000), "in about 5 minutes");
        assert_eq!(in_about(90 * 60_000), "in about 2 hours");
        assert_eq!(in_about(48 * 60 * 60_000), "in about 2 days");
    }

    #[test]
    fn auto_uses_opencode_only_when_the_primaries_are_certainly_out() {
        let f = crate::test_support::Fixture::new();
        let at = now();
        // No Claude or ChatGPT account: an OpenCode-only profile may run in Auto.
        assert!(primary_unavailable(&f.core, &[], &[], at));
        let claude = f
            .core
            .store
            .add_account("Claude".into(), Provider::Claude)
            .unwrap();
        // A primary account without fresh data is never assumed exhausted.
        assert!(!primary_unavailable(
            &f.core,
            std::slice::from_ref(&claude),
            &[],
            at
        ));
        let usage = |used: f64| AccountUsage {
            account_id: claude.id.clone(),
            updated_at: at,
            source: "test".into(),
            windows: vec![UsageWindow {
                group: None,
                used_percent: used,
                window_minutes: 300,
                resets_at: at + 60_000,
            }],
        };
        // Fresh data with quota left: still the primary's turn.
        assert!(!primary_unavailable(
            &f.core,
            std::slice::from_ref(&claude),
            &[usage(40.0)],
            at
        ));
        // Fresh exhausted data: the emergency chain may take over.
        assert!(primary_unavailable(
            &f.core,
            std::slice::from_ref(&claude),
            &[usage(100.0)],
            at
        ));
        // Stale exhaustion proves nothing.
        let mut stale = usage(100.0);
        stale.updated_at = at - 900_001;
        assert!(!primary_unavailable(
            &f.core,
            std::slice::from_ref(&claude),
            &[stale],
            at
        ));
        // A cooldown is a certainty.
        f.core.limits.cool(&claude.id, at + 60_000);
        assert!(primary_unavailable(
            &f.core,
            std::slice::from_ref(&claude),
            &[],
            at
        ));
    }

    #[test]
    fn autopilot_markers_need_an_explanation() {
        let (body, outcome) = autopilot_outcome("Need a key.\n<!-- lumen-autopilot: blocked -->\n");
        assert_eq!(body, "Need a key.");
        assert!(outcome == AutopilotOutcome::Blocked);
        assert!(
            autopilot_outcome("<!-- lumen-autopilot: blocked -->").1
                == AutopilotOutcome::Unconfirmed
        );
        assert!(
            autopilot_outcome("Done. <!-- lumen-autopilot: done -->").1 == AutopilotOutcome::Done
        );
        assert!(
            autopilot_outcome("Next: build. <!-- lumen-autopilot: continue -->").1
                == AutopilotOutcome::Continue
        );
        assert!(autopilot_outcome("No marker").1 == AutopilotOutcome::Unconfirmed);
        assert!(
            autopilot_outcome("x <!-- lumen-autopilot: maybe -->").1
                == AutopilotOutcome::Unconfirmed
        );
    }

    #[test]
    fn an_explained_autopilot_stop_is_partial_not_an_error() {
        let mut reply = Message {
            id: "m".into(),
            role: "assistant".into(),
            text: "Needs a credential.".into(),
            activity: vec![],
        };
        assert_eq!(finished_status(&Ok(()), &reply), "completed");
        assert_eq!(
            finished_status(&Err("Cancelled".into()), &reply),
            "interrupted"
        );
        assert_eq!(
            finished_status(&Err("Codex turn failed".into()), &reply),
            "error"
        );
        reply.activity.push(Activity {
            kind: AUTOPILOT_STOPPED.into(),
            text: r#"{"reason":"blocked"}"#.into(),
        });
        assert_eq!(finished_status(&Ok(()), &reply), "partial");
        assert_eq!(
            finished_status(&Err("Codex turn failed".into()), &reply),
            "error"
        );
    }

    #[test]
    fn autopilot_progress_counts_only_new_evidence() {
        let mut progress = AutopilotProgress::default();
        let tool = |text: &str| Activity {
            kind: "tool-end".into(),
            text: text.into(),
        };
        progress.observe(&tool(r#"{"id":"a","evidence":7}"#));
        assert!(std::mem::take(&mut progress.advanced));
        progress.observe(&tool(r#"{"id":"b","evidence":7}"#));
        progress.observe(&tool(r#"{"id":"c","evidence":null}"#));
        assert!(
            !progress.advanced,
            "a repeated check or a step without evidence is not progress"
        );
        progress.observe(&Activity {
            kind: "interaction-result".into(),
            text: r#"{"kind":"question","cancelled":true}"#.into(),
        });
        assert!(!progress.advanced);
        progress.observe(&Activity {
            kind: "interaction-result".into(),
            text: r#"{"kind":"question"}"#.into(),
        });
        assert!(progress.advanced);
    }

    #[test]
    fn logout_requires_unambiguous_provider_evidence() {
        assert!(confirmed_logout(
            &Provider::Claude,
            false,
            br#"{"loggedIn":false,"authMethod":"none"}"#,
            b""
        ));
        assert!(confirmed_logout(
            &Provider::Gpt,
            false,
            b"",
            b"Not logged in\n"
        ));
        assert!(!confirmed_logout(
            &Provider::Claude,
            false,
            b"network error",
            b""
        ));
        assert!(!confirmed_logout(
            &Provider::Claude,
            false,
            br#"{"loggedIn":false}"#,
            b""
        ));
        assert!(!confirmed_logout(
            &Provider::Gpt,
            false,
            b"",
            b"Not logged in: network error"
        ));
        assert!(!confirmed_logout(
            &Provider::Gpt,
            true,
            b"Not logged in",
            b""
        ));
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
        // A saved session marker cannot enable a capability absent from the adapter.
        for provider in [
            Provider::Claude,
            Provider::Gpt,
            Provider::Antigravity,
            Provider::Ollama,
        ] {
            let mut account = claude();
            account.provider = provider.clone();
            assert_eq!(
                resumable(&chat, &["out"], &account, "/w", AccessMode::FileEdits).is_some(),
                matches!(provider, Provider::Claude | Provider::Gpt),
                "{provider:?}"
            );
        }
    }

    #[test]
    fn continuity_rewind_invalidates_remote_history_without_touching_other_chats() {
        let f = crate::test_support::Fixture::new();
        let s = f.core.store.create("/w".into()).unwrap();
        let other = f.core.store.create("/w".into()).unwrap();
        for chat in [&s, &other] {
            for m in [
                msg("u1", "user", vec![]),
                msg(
                    "a1",
                    "assistant",
                    vec![
                        native("acc", "/w"),
                        Activity {
                            kind: "file".into(),
                            text: "keep.ts".into(),
                        },
                    ],
                ),
                msg("u2", "user", vec![]),
            ] {
                // Message IDs are globally unique, including across chats.
                let mut m = m;
                m.id = format!("{}-{}", chat.id, m.id);
                f.core.store.save_message(&chat.id, &m).unwrap();
            }
        }
        let removed = format!("{}-u2", s.id);
        f.core.store.truncate_from(&s.id, &removed).unwrap();
        f.core
            .store
            .save_message(&s.id, &msg("replacement", "user", vec![]))
            .unwrap();
        let history = f.core.store.messages(&s.id).unwrap();
        assert!(
            resumable(&history, &[], &claude(), "/w", AccessMode::ReadOnly).is_none(),
            "remote CLI still contains the removed future turns"
        );
        assert!(history[1]
            .activity
            .iter()
            .any(|a| a.kind == "file" && a.text == "keep.ts"));
        assert!(resumable(
            &f.core.store.messages(&other.id).unwrap(),
            &[],
            &claude(),
            "/w",
            AccessMode::ReadOnly
        )
        .is_some());
        // An unknown rewind target must leave the surviving transcript unchanged.
        assert!(f.core.store.truncate_from(&other.id, "missing").is_err());
        assert!(resumable(
            &f.core.store.messages(&other.id).unwrap(),
            &[],
            &claude(),
            "/w",
            AccessMode::ReadOnly
        )
        .is_some());
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
    async fn continuity_native_resume_retires_checks_even_when_project_notes_remain() {
        let f = crate::test_support::Fixture::new();
        let folder = f.root.to_string_lossy().into_owned();
        let s = f.core.store.create(folder.clone()).unwrap();
        std::fs::write(f.root.join("LUMEN.md"), "Keep project conventions").unwrap();
        let check = Activity {
            kind: "validation".into(),
            text: serde_json::json!({"workspace":folder,"label":"Old build","status":"passed"})
                .to_string(),
        };
        f.core
            .store
            .save_message(&s.id, &msg("checked", "assistant", vec![check]))
            .unwrap();
        let revision = crate::workstate::owned_context(&f.core, &s)
            .unwrap()
            .revision;
        let mut marker = native("acc", &folder);
        let mut value: serde_json::Value = serde_json::from_str(&marker.text).unwrap();
        value["context_revision"] = revision.into();
        marker.text = value.to_string();
        f.core
            .store
            .save_message(&s.id, &msg("reply", "assistant", vec![marker]))
            .unwrap();
        let mut request = msg("next", "user", vec![]);
        request.text = "Nuovo obiettivo: implementa la ricerca".into();
        f.core.store.save_message(&s.id, &request).unwrap();
        let resumed = resume_prompt(
            &f.core,
            &s,
            &[],
            &claude(),
            &crate::test_support::Fixture::options(AccessMode::ReadOnly),
        )
        .await
        .unwrap();
        assert!(resumed.prompt.contains("Keep project conventions"));
        assert!(resumed
            .prompt
            .contains("Omitted checks are not current evidence"));
        assert!(!resumed.prompt.contains("Old build: passed"));
        assert!(resumed.prompt.contains("Read-only"));
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
