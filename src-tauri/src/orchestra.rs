//! Orchestra: for complex Auto tasks, one orchestrator model asks clarifying
//! questions, splits the work into subtasks, shows the plan for approval, runs
//! the subtasks in parallel git worktrees across every available provider and
//! integrates the result.
//!
//! The whole flow lives in assistant messages: each carries one `orchestra`
//! activity with the JSON [`State`], replaced in place as the run advances, so
//! the interface can redraw the card and the history survives restarts.
use crate::{
    account, config_dir, event, gitops, models::*, models_for, providers, resolve_effort, router,
    turns, Core,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::sync::watch;

/// Subtasks that run at the same time; the rest queue.
const PARALLEL: usize = 6;
const MAX_SUBTASKS: usize = 12;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Question {
    pub q: String,
    pub recommended: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Subtask {
    pub id: String,
    pub title: String,
    pub detail: String,
    pub files: Vec<String>,
    /// "trivial", "normal" or "complex".
    pub difficulty: String,
    pub writes: bool,
    pub depends_on: Vec<String>,
    pub account_id: String,
    pub agent: String,
    pub model: String,
    pub effort: String,
    /// pending · running · done · failed
    pub status: String,
    pub output: String,
    pub started_at: i64,
    pub ended_at: i64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Step {
    pub name: String,
    /// running · done · failed · skipped
    pub status: String,
    pub detail: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct State {
    /// questions · answered · plan · approved · cancelled · running · done · failed · kept · discarded
    pub stage: String,
    pub task: String,
    pub questions: Vec<Question>,
    pub confirm: String,
    pub answers: String,
    pub orchestrator: String,
    pub orchestrator_agent: String,
    pub orchestrator_model: String,
    pub effort: String,
    pub summary: String,
    pub subtasks: Vec<Subtask>,
    pub plan_review: String,
    pub read_only: bool,
    pub steps: Vec<Step>,
    /// Worktrees kept after a failed integration: (top, path, branch).
    pub lanes: Vec<(String, String, String)>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Edit {
    pub id: String,
    #[serde(default)]
    pub account_id: Option<String>,
    #[serde(default)]
    pub remove: bool,
}

pub fn display(p: &Provider) -> &'static str {
    match p {
        Provider::Claude => "Claude",
        Provider::Gpt => "ChatGPT",
        Provider::Gemini => "Gemini CLI",
        Provider::Antigravity => "Antigravity",
        Provider::Ollama => "Ollama",
    }
}

pub fn state_of(m: &Message) -> Option<State> {
    m.activity
        .iter()
        .find(|a| a.kind == "orchestra")
        .and_then(|a| serde_json::from_str(&a.text).ok())
}

fn publish(app: &tauri::AppHandle, core: &Core, s: &Session, out: &mut Message, st: &State) {
    let text = serde_json::to_string(st).unwrap_or_default();
    match out.activity.iter_mut().find(|a| a.kind == "orchestra") {
        Some(a) => a.text = text.clone(),
        None => out.activity.insert(
            0,
            Activity {
                kind: "orchestra".into(),
                text: text.clone(),
            },
        ),
    }
    event(app, &s.id, &out.id, "orchestra", &text);
    let _ = core.store.save_message(&s.id, out);
}

/// First JSON object in a model reply, tolerating code fences and prose around it.
pub fn extract_json(text: &str) -> Option<Value> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    if end <= start {
        return None;
    }
    serde_json::from_str(&text[start..=end]).ok()
}

pub fn parse_questions(text: &str) -> (Vec<Question>, String) {
    let Some(v) = extract_json(text) else {
        return (vec![], String::new());
    };
    let questions: Vec<Question> =
        serde_json::from_value(v["questions"].clone()).unwrap_or_default();
    let questions = questions
        .into_iter()
        .filter(|q| !q.q.trim().is_empty())
        .take(8)
        .collect();
    (
        questions,
        v["confirm"].as_str().unwrap_or("").trim().to_string(),
    )
}

/// Parse and sanitize a plan: unique ids, at most 12 subtasks, dependencies
/// only on earlier subtasks (so the graph can never contain a cycle).
pub fn parse_plan(text: &str) -> Option<(String, Vec<Subtask>)> {
    let v = extract_json(text)?;
    let raw: Vec<Subtask> = serde_json::from_value(v["subtasks"].clone()).ok()?;
    let mut seen: Vec<String> = vec![];
    let mut renamed: HashMap<String, String> = HashMap::new();
    let mut out = vec![];
    for (i, mut t) in raw.into_iter().take(MAX_SUBTASKS).enumerate() {
        if t.title.trim().is_empty() && t.detail.trim().is_empty() {
            continue;
        }
        let id = format!("s{}", i + 1);
        renamed.insert(t.id.clone(), id.clone());
        t.depends_on = t
            .depends_on
            .iter()
            .filter_map(|d| renamed.get(d).cloned().or_else(|| Some(d.clone())))
            .filter(|d| seen.contains(d))
            .collect();
        t.depends_on.dedup();
        t.id = id.clone();
        if !["trivial", "normal", "complex"].contains(&t.difficulty.as_str()) {
            t.difficulty = "normal".into();
        }
        if t.title.trim().is_empty() {
            t.title = t.detail.chars().take(60).collect();
        }
        t.status = "pending".into();
        t.account_id.clear();
        t.agent.clear();
        t.model.clear();
        t.output.clear();
        seen.push(id);
        out.push(t);
    }
    if out.is_empty() {
        return None;
    }
    Some((v["summary"].as_str().unwrap_or("").trim().to_string(), out))
}

fn tier_of(difficulty: &str) -> i32 {
    match difficulty {
        "trivial" => 0,
        "complex" => 2,
        _ => 1,
    }
}

/// Pick an agent for every subtask that has none: trivial work may go to the
/// emergency providers (Ollama only when nothing is written), the rest is
/// spread across Claude and ChatGPT. Models follow the same tiers as Auto.
pub fn choose_accounts(subtasks: &mut [Subtask], accounts: &[Account], orchestrator: &Provider) {
    let mut primary: Vec<&Account> = vec![];
    for p in [Provider::Claude, Provider::Gpt] {
        if let Some(a) = accounts.iter().find(|a| a.provider == p) {
            primary.push(a);
        }
    }
    // Start with the provider that is not orchestrating, to spread the load.
    if primary.len() == 2 && primary[0].provider == *orchestrator {
        primary.swap(0, 1);
    }
    let mut emergency: Vec<&Account> = accounts
        .iter()
        .filter(|a| a.provider.fallback_rank() > 0)
        .collect();
    emergency.sort_by_key(|a| a.provider.fallback_rank());
    let (mut rr, mut er) = (0, 0);
    for t in subtasks.iter_mut() {
        if accounts.iter().any(|a| a.id == t.account_id) {
            continue;
        }
        let light: Vec<&&Account> = emergency
            .iter()
            .filter(|a| !(t.writes && a.provider == Provider::Ollama))
            .collect();
        let pick = if t.difficulty == "trivial" && !light.is_empty() {
            er += 1;
            Some(*light[(er - 1) % light.len()])
        } else if !primary.is_empty() {
            rr += 1;
            Some(primary[(rr - 1) % primary.len()])
        } else {
            accounts
                .iter()
                .find(|a| !(t.writes && a.provider == Provider::Ollama))
        };
        if let Some(a) = pick {
            t.account_id = a.id.clone();
            t.model.clear();
        }
    }
}

pub fn effort_for(tier: i32, policy: &AutoPolicy) -> &'static str {
    let base = i32::from(tier == 2);
    let adj = match policy {
        AutoPolicy::Budget => -1,
        AutoPolicy::Quality => 1,
        AutoPolicy::Default => 0,
    };
    ["low", "medium", "high"][(base + adj).clamp(0, 2) as usize]
}

/// Orchestrator model tier: Budget/Default follow the task with a Sonnet/Terra
/// floor (planning on the smallest models is the weak link); Quality uses the strongest.
pub fn orchestrator_tier(policy: &AutoPolicy, task_tier: i32) -> i32 {
    match policy {
        AutoPolicy::Quality => 2,
        _ => task_tier.max(1),
    }
}

async fn assign(core: &Core, st: &mut State, policy: &AutoPolicy) -> Result<(), String> {
    let accounts: Vec<Account> = core
        .store
        .accounts()?
        .into_iter()
        .filter(|a| !core.limits.cooling(&a.id, now()))
        .collect();
    if accounts.is_empty() {
        return Err("Every account is cooling down after a rate limit. Try again later.".into());
    }
    let orchestrator = account(core, &st.orchestrator)?.provider;
    choose_accounts(&mut st.subtasks, &accounts, &orchestrator);
    for t in st.subtasks.iter_mut() {
        let Some(a) = accounts.iter().find(|a| a.id == t.account_id) else {
            continue;
        };
        let tier = tier_of(&t.difficulty);
        if t.model.is_empty() {
            if let Ok(models) = models_for(core, a, false, None).await {
                if let Some(m) = router::select_tier(&models, tier) {
                    t.model = m.id.clone();
                    t.agent = format!("{} · {}", display(&a.provider), m.name);
                }
            }
        }
        if t.agent.is_empty() || !t.agent.starts_with(display(&a.provider)) {
            t.agent = display(&a.provider).into();
        }
        t.effort = effort_for(tier, policy).into();
    }
    Ok(())
}

fn plan_markdown(st: &State) -> String {
    let mut s = format!("**Orchestra plan** · {}\n\n", st.summary);
    for t in &st.subtasks {
        s.push_str(&format!(
            "- **{}** — {} ({})\n",
            t.title, t.agent, t.difficulty
        ));
    }
    s
}

#[allow(clippy::too_many_arguments)]
async fn hidden(
    app: &tauri::AppHandle,
    core: &Core,
    s: &Session,
    out: &mut Message,
    a: &Account,
    prompt: &str,
    model: Option<&str>,
    effort: &str,
    cancel: watch::Receiver<bool>,
) -> Result<String, String> {
    let mut scratch = Message {
        id: out.id.clone(),
        role: "assistant".into(),
        text: String::new(),
        activity: vec![],
    };
    let r = turns::stage(
        app,
        core,
        s,
        &mut scratch,
        a,
        prompt,
        model,
        effort,
        AccessMode::ReadOnly,
        cancel,
        false,
    )
    .await;
    out.activity.extend(
        scratch
            .activity
            .into_iter()
            .filter(|a| a.kind == "model")
            .take(4),
    );
    r
}

/// Another provider than `writer`, for audits.
fn reviewer_for(core: &Core, writer: &Provider) -> Option<Account> {
    core.store.accounts().ok()?.into_iter().find(|b| {
        b.provider != *writer
            && matches!(b.provider, Provider::Claude | Provider::Gpt)
            && !core.limits.cooling(&b.id, now())
    })
}

// ---------------------------------------------------------------- questions

/// Stage 1: the orchestrator asks what it needs to know before planning.
#[allow(clippy::too_many_arguments)]
pub async fn ask(
    app: &tauri::AppHandle,
    core: &Core,
    s: &Session,
    out: &mut Message,
    plan: &Plan,
    options: &RunOptions,
    task: &str,
    cancel: watch::Receiver<bool>,
) -> Result<(), String> {
    let orch = account(core, &plan.account_id)?;
    let models = models_for(core, &orch, false, Some(cancel.clone())).await?;
    let tier = orchestrator_tier(&options.auto_policy, router::task_tier(task));
    let model = router::select_tier(&models, tier).ok_or("No orchestrator model available")?;
    let mut st = State {
        stage: "asking".into(),
        task: task.into(),
        orchestrator: orch.id.clone(),
        orchestrator_agent: format!("{} · {}", display(&orch.provider), model.name),
        orchestrator_model: model.id.clone(),
        effort: plan.effort.clone(),
        read_only: options.access() == AccessMode::ReadOnly,
        ..Default::default()
    };
    publish(app, core, s, out, &st);
    let (context, _) = turns::build_prompt(core, s, &[&out.id], options).await?;
    let prompt = format!(
        "{context}\n\n---\nYou are the orchestrator of a team of AI agents that will work on the request above in parallel. Before planning, decide what you must clarify with the user. You may read the project files; do not edit anything.\nReply with JSON only, in one of these two shapes:\n{{\"questions\":[{{\"q\":\"a question that changes the result\",\"recommended\":\"your recommended answer\"}}]}}  (at most 6 questions, each with a recommendation)\n{{\"confirm\":\"I'd do X, Y and Z.\"}}  (when the request is already clear)"
    );
    let reply = hidden(
        app,
        core,
        s,
        out,
        &orch,
        &prompt,
        Some(&model.id),
        &plan.effort,
        cancel,
    )
    .await?;
    let (questions, confirm) = parse_questions(&reply);
    st.questions = questions;
    st.confirm = if st.questions.is_empty() && confirm.is_empty() {
        "I'll plan this task, split it into subtasks and assign each to the best available agent."
            .into()
    } else {
        confirm
    };
    st.stage = "questions".into();
    out.text = if st.questions.is_empty() {
        format!("**Orchestra** · {}\n\nAgree?", st.confirm)
    } else {
        let mut t = "**Orchestra** · a few questions before planning:\n\n".to_string();
        for (i, q) in st.questions.iter().enumerate() {
            t.push_str(&format!(
                "{}. {}\n   *Recommended:* {}\n",
                i + 1,
                q.q,
                q.recommended
            ));
        }
        t
    };
    event(app, &s.id, &out.id, "replace", &out.text);
    publish(app, core, s, out, &st);
    Ok(())
}

pub fn answers_text(st: &State, answers: &[String], note: &str, skip: bool) -> String {
    if skip {
        return "Skip the questions: use your recommendations.".into();
    }
    if st.questions.is_empty() {
        return if note.trim().is_empty() {
            "Agreed.".into()
        } else {
            format!("Change: {}", note.trim())
        };
    }
    let mut t = String::new();
    for (i, q) in st.questions.iter().enumerate() {
        let a = answers
            .get(i)
            .map(|a| a.trim())
            .filter(|a| !a.is_empty())
            .unwrap_or(&q.recommended);
        t.push_str(&format!("{}. {}\n→ {}\n", i + 1, q.q, a));
    }
    if !note.trim().is_empty() {
        t.push_str(&format!("\n{}", note.trim()));
    }
    t
}

// ---------------------------------------------------------------- planning

/// Stage 2: split into subtasks, assign agents, optionally audit the plan.
#[allow(clippy::too_many_arguments)]
pub async fn plan(
    app: &tauri::AppHandle,
    core: &Core,
    s: &Session,
    out: &mut Message,
    mut st: State,
    options: &RunOptions,
    cancel: watch::Receiver<bool>,
) -> Result<(), String> {
    let orch = account(core, &st.orchestrator)?;
    st.stage = "planning".into();
    st.subtasks.clear();
    publish(app, core, s, out, &st);
    let schema = "Reply with JSON only:\n{\"summary\":\"one sentence\",\"subtasks\":[{\"id\":\"s1\",\"title\":\"short title\",\"detail\":\"precise, self-contained instructions\",\"files\":[\"paths this subtask owns\"],\"difficulty\":\"trivial|normal|complex\",\"writes\":true,\"dependsOn\":[]}]}";
    let rules = "Rules: 2 to 8 subtasks that can run in parallel, each in its own copy of the repository. Give each subtask clear file ownership and never let two subtasks edit the same file. Use dependsOn only when a subtask truly needs another one's result. difficulty is trivial (messages, text, renames, tiny edits), normal or complex. writes is false for research or text-only work. You will integrate the results yourself at the end, so do not add an integration subtask.";
    let prompt = format!(
        "You are the orchestrator of a team of AI agents. Plan the work below. You may read the project files; do not edit anything.\n\nRequest:\n{}\n\nClarifications from the user:\n{}\n\n{rules}\n{schema}",
        st.task, st.answers
    );
    let model = Some(st.orchestrator_model.clone()).filter(|m| !m.is_empty());
    let reply = hidden(
        app,
        core,
        s,
        out,
        &orch,
        &prompt,
        model.as_deref(),
        &st.effort,
        cancel.clone(),
    )
    .await?;
    let (summary, subtasks) =
        parse_plan(&reply).ok_or("The orchestrator did not return a usable plan. Try again.")?;
    st.summary = summary;
    st.subtasks = subtasks;
    assign(core, &mut st, &options.auto_policy).await?;
    // Default and Quality audit the plan too; Budget audits only the integration.
    if options.auto_review && options.auto_policy != AutoPolicy::Budget {
        if let Some(reviewer) = reviewer_for(core, &orch.provider) {
            st.steps = vec![Step {
                name: format!("Plan audit by {}", display(&reviewer.provider)),
                status: "running".into(),
                detail: String::new(),
            }];
            publish(app, core, s, out, &st);
            let plan_json = serde_json::to_string_pretty(&st.subtasks).unwrap_or_default();
            let review_prompt = format!(
                "You are auditing a work plan written by another AI orchestrator. Read-only: never edit files. Check that the split is complete, that file ownership does not overlap, that dependencies are right and that each assignment fits the difficulty. If the plan is sound, reply with the single word APPROVED on the first line; otherwise list concrete corrections.\n\nRequest:\n{}\n\nClarifications:\n{}\n\nPlan:\n{plan_json}",
                st.task, st.answers
            );
            let review = hidden(
                app,
                core,
                s,
                out,
                &reviewer,
                &review_prompt,
                None,
                &st.effort,
                cancel.clone(),
            )
            .await?;
            let ok = turns::approved(&review);
            st.plan_review = review.clone();
            st.steps[0].status = "done".into();
            st.steps[0].detail = if ok {
                "Approved".into()
            } else {
                "Revised once with the auditor's notes".into()
            };
            if !ok {
                let revise = format!(
                    "{prompt}\n\nYour previous plan:\n{plan_json}\n\nAn auditor reviewed it. Address every valid point (ignore incorrect ones) and return the revised plan.\n\nAudit:\n{review}"
                );
                let reply = hidden(
                    app,
                    core,
                    s,
                    out,
                    &orch,
                    &revise,
                    model.as_deref(),
                    &st.effort,
                    cancel.clone(),
                )
                .await?;
                if let Some((summary, subtasks)) = parse_plan(&reply) {
                    st.summary = summary;
                    st.subtasks = subtasks;
                    assign(core, &mut st, &options.auto_policy).await?;
                }
            }
        }
    }
    st.stage = "plan".into();
    out.text = plan_markdown(&st);
    event(app, &s.id, &out.id, "replace", &out.text);
    publish(app, core, s, out, &st);
    Ok(())
}

/// Apply the user's edits from the plan card (reassign or remove subtasks).
pub fn apply_edits(st: &mut State, edits: &[Edit], accounts: &[Account]) -> Result<(), String> {
    for e in edits {
        if e.remove {
            st.subtasks.retain(|t| t.id != e.id);
            continue;
        }
        if let Some(id) = &e.account_id {
            let a = accounts
                .iter()
                .find(|a| &a.id == id)
                .ok_or("Account not found")?;
            if let Some(t) = st.subtasks.iter_mut().find(|t| t.id == e.id) {
                if t.writes && a.provider == Provider::Ollama {
                    return Err(format!(
                        "Ollama can't edit files, so it can't take \"{}\". Choose another agent.",
                        t.title
                    ));
                }
                if t.account_id != a.id {
                    t.account_id = a.id.clone();
                    t.model.clear();
                    t.agent.clear();
                }
            }
        }
    }
    let ids: Vec<String> = st.subtasks.iter().map(|t| t.id.clone()).collect();
    for t in st.subtasks.iter_mut() {
        t.depends_on.retain(|d| ids.contains(d));
    }
    if st.subtasks.is_empty() {
        return Err("The plan has no subtasks left.".into());
    }
    Ok(())
}

// ---------------------------------------------------------------- execution

fn manifest(st: &State) -> String {
    st.subtasks
        .iter()
        .map(|t| {
            format!(
                "- {} · {} · {} · owns: {}",
                t.id,
                t.title,
                t.agent,
                if t.files.is_empty() {
                    "(no files)".to_string()
                } else {
                    t.files.join(", ")
                }
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn subtask_prompt(st: &State, t: &Subtask, deps: &str, text_only: bool) -> String {
    let rules = if text_only {
        "Reply with the requested text only."
    } else if st.read_only {
        "This run is read-only: analyse and describe the changes precisely, but do not modify files."
    } else {
        "Edit only the files you own. If another subtask's file must change, describe the change in your final message instead of editing it. Finish with a short summary of what you changed."
    };
    format!(
        "You are one agent in a team working in parallel on a shared task, coordinated by an orchestrator. Do ONLY your subtask.\n\nOverall request:\n{}\n\nClarifications:\n{}\n\nTeam manifest (who owns which files):\n{}\n{deps}\nYour subtask ({}): {}\n{}\nFiles you own: {}\n\n{rules}",
        st.task,
        st.answers,
        manifest(st),
        t.id,
        t.title,
        t.detail,
        if t.files.is_empty() {
            "(none listed)".to_string()
        } else {
            t.files.join(", ")
        }
    )
}

struct Job {
    app: tauri::AppHandle,
    core: Arc<Core>,
    session: String,
    message: String,
    id: String,
    account: Account,
    model: String,
    effort: String,
    prompt: String,
    workspace: PathBuf,
    access: AccessMode,
    cancel: watch::Receiver<bool>,
}

async fn attempt(job: &Job, a: &Account, model: Option<&str>) -> Result<String, String> {
    let core = &job.core;
    let models = models_for(core, a, false, Some(job.cancel.clone())).await?;
    let selected = model
        .and_then(|id| models.iter().find(|m| m.id == id))
        .or_else(|| models.iter().find(|m| m.is_default))
        .or_else(|| models.first())
        .ok_or("No provider model available")?;
    let effort = resolve_effort(selected, &job.effort);
    let config = config_dir(core, a);
    let (app, session, message, id) = (&job.app, &job.session, &job.message, &job.id);
    let r = providers::execute(
        providers::Invocation {
            account: a,
            config: &config,
            workspace: &job.workspace,
            prompt: &job.prompt,
            model: Some(&selected.id),
            effort: &effort,
            access: job.access,
        },
        job.cancel.clone(),
        |activity| {
            if activity.kind == "delta" || activity.kind == "replace" {
                let payload =
                    serde_json::json!({"id": id, "kind": activity.kind, "text": activity.text});
                event(
                    app,
                    session,
                    message,
                    "orchestra-delta",
                    &payload.to_string(),
                );
            }
        },
    )
    .await;
    if r.is_ok() {
        core.limits.record_turn(&a.id, now());
    }
    r
}

/// Run one subtask; on any failure retry once on another provider.
async fn run_job(job: Job) -> (String, Result<(String, String), String>) {
    let first = attempt(
        &job,
        &job.account,
        Some(job.model.as_str()).filter(|m| !m.is_empty()),
    )
    .await;
    let err = match first {
        Ok(t) => return (job.id.clone(), Ok((t, String::new()))),
        Err(e) if e == "Cancelled" => return (job.id.clone(), Err(e)),
        Err(e) => e,
    };
    if err.starts_with(crate::limits::RATE_LIMIT) {
        let msg = err.trim_start_matches(crate::limits::RATE_LIMIT);
        job.core
            .limits
            .cool(&job.account.id, crate::limits::unlock_at(msg, now()));
    }
    let writes = job.access != AccessMode::ReadOnly;
    let mut alternatives: Vec<Account> = job
        .core
        .store
        .accounts()
        .unwrap_or_default()
        .into_iter()
        .filter(|b| {
            b.id != job.account.id
                && !job.core.limits.cooling(&b.id, now())
                && !(writes && b.provider == Provider::Ollama)
        })
        .collect();
    alternatives.sort_by_key(|b| {
        (
            b.provider == job.account.provider,
            b.provider.fallback_rank(),
        )
    });
    let Some(b) = alternatives.into_iter().next() else {
        return (job.id.clone(), Err(err));
    };
    let note = format!(
        "{} failed ({}). Retried on {}.",
        display(&job.account.provider),
        err.chars().take(160).collect::<String>(),
        display(&b.provider)
    );
    match attempt(&job, &b, None).await {
        Ok(t) => (job.id.clone(), Ok((t, note))),
        Err(e) => (job.id.clone(), Err(format!("{note} That also failed: {e}"))),
    }
}

struct Lane {
    path: PathBuf,
    branch: String,
    start: String,
}

/// Stage 3: run the approved plan, integrate, audit, verify.
#[allow(clippy::too_many_arguments)]
pub async fn execute(
    app: &tauri::AppHandle,
    core: &Arc<Core>,
    s: &Session,
    out: &mut Message,
    mut st: State,
    options: &RunOptions,
    checkpoint: Option<String>,
    mut cancel: watch::Receiver<bool>,
) -> Result<(), String> {
    let orch = account(core, &st.orchestrator)?;
    let workspace = PathBuf::from(&s.workspace);
    let top = gitops::toplevel(&workspace).await;
    st.read_only = st.read_only || options.access() == AccessMode::ReadOnly || top.is_none();
    let lock = turns::workspace_lock(core, s)?;
    let _guard = if st.read_only {
        None
    } else {
        Some(
            tokio::select! {biased;_=cancel.changed()=>return Err("Cancelled".into()),g=lock.lock()=>g},
        )
    };
    let base = match (&top, checkpoint) {
        (_, Some(t)) => t,
        (Some(top), None) if !st.read_only => gitops::write_tree(top).await?,
        _ => String::new(),
    };
    let rel = top
        .as_ref()
        .and_then(|t| workspace.strip_prefix(t).ok())
        .unwrap_or(Path::new(""))
        .to_path_buf();
    let short: String = out.id.chars().take(8).collect();
    st.stage = "running".into();
    st.steps.clear();
    for t in st.subtasks.iter_mut() {
        t.status = "pending".into();
        t.output.clear();
    }
    publish(app, core, s, out, &st);

    let mut lanes: HashMap<String, Lane> = HashMap::new();
    let mut patches: HashMap<String, String> = HashMap::new();
    let mut set = tokio::task::JoinSet::new();
    loop {
        // Start every subtask whose dependencies have finished, up to the parallel limit.
        let finished: Vec<String> = st
            .subtasks
            .iter()
            .filter(|t| t.status == "done" || t.status == "failed")
            .map(|t| t.id.clone())
            .collect();
        let ready: Vec<usize> = st
            .subtasks
            .iter()
            .enumerate()
            .filter(|(_, t)| {
                t.status == "pending" && t.depends_on.iter().all(|d| finished.contains(d))
            })
            .map(|(i, _)| i)
            .collect();
        for i in ready {
            if set.len() >= PARALLEL {
                break;
            }
            let t = st.subtasks[i].clone();
            let Ok(a) = account(core, &t.account_id) else {
                st.subtasks[i].status = "failed".into();
                st.subtasks[i].output = "The assigned account no longer exists.".into();
                continue;
            };
            let text_only = a.provider == Provider::Ollama;
            let writes = t.writes && !st.read_only && !text_only;
            let mut job_dir = workspace.clone();
            if writes {
                let top = top.as_ref().ok_or("Git repository required")?;
                let path = core.root.join("worktrees").join(&short).join(&t.id);
                let branch = format!("lumen/orchestra-{short}-{}", t.id);
                gitops::add_lane(top, &base, &path, &branch).await?;
                // A dependent subtask starts from its dependencies' changes.
                for d in &t.depends_on {
                    if let Some(p) = patches.get(d) {
                        let _ = gitops::apply_patch(&path, p).await;
                    }
                }
                let start = gitops::write_tree(&path).await?;
                job_dir = path.join(&rel);
                lanes.insert(
                    t.id.clone(),
                    Lane {
                        path,
                        branch,
                        start,
                    },
                );
            }
            let deps: String = t
                .depends_on
                .iter()
                .filter_map(|d| st.subtasks.iter().find(|x| &x.id == d))
                .map(|d| {
                    format!(
                        "\nResult of {} ({}):\n{}\n",
                        d.id,
                        d.title,
                        d.output.chars().take(4000).collect::<String>()
                    )
                })
                .collect();
            let prompt = subtask_prompt(&st, &t, &deps, text_only);
            set.spawn(run_job(Job {
                app: app.clone(),
                core: core.clone(),
                session: s.id.clone(),
                message: out.id.clone(),
                id: t.id.clone(),
                account: a,
                model: t.model.clone(),
                effort: t.effort.clone(),
                prompt,
                workspace: job_dir,
                access: if writes {
                    options.access()
                } else {
                    AccessMode::ReadOnly
                },
                cancel: cancel.clone(),
            }));
            st.subtasks[i].status = "running".into();
            st.subtasks[i].started_at = now();
        }
        publish(app, core, s, out, &st);
        if set.is_empty() {
            break;
        }
        let joined = tokio::select! {
            biased;
            _ = cancel.changed() => {
                set.abort_all();
                for l in lanes.values() {
                    if let Some(top) = &top { gitops::remove_lane(top, &l.path, &l.branch).await; }
                }
                return Err("Cancelled".into());
            }
            j = set.join_next() => j,
        };
        let Some(Ok((id, result))) = joined else {
            continue;
        };
        let Some(t) = st.subtasks.iter_mut().find(|t| t.id == id) else {
            continue;
        };
        t.ended_at = now();
        match result {
            Ok((text, retried)) => {
                t.status = "done".into();
                t.output = if retried.is_empty() {
                    text
                } else {
                    format!("{retried}\n\n{text}")
                };
                if let Some(l) = lanes.get(&id) {
                    if let Ok(p) = gitops::lane_patch(&l.path, &l.start).await {
                        if !p.trim().is_empty() {
                            patches.insert(id.clone(), p);
                        }
                    }
                }
            }
            Err(e) => {
                t.status = "failed".into();
                t.output = turns::friendly(&e);
            }
        }
        t.output = t.output.chars().take(16_000).collect();
    }

    let failed: Vec<String> = st
        .subtasks
        .iter()
        .filter(|t| t.status == "failed")
        .map(|t| format!("{} ({})", t.title, t.id))
        .collect();
    if failed.len() == st.subtasks.len() {
        cleanup(&top, &lanes).await;
        st.stage = "failed".into();
        publish(app, core, s, out, &st);
        return Err("Every subtask failed. See the Orchestra card for details.".into());
    }
    let model = Some(st.orchestrator_model.clone()).filter(|m| !m.is_empty());

    // Integration: apply each subtask's patch; the orchestrator resolves conflicts.
    if let Some(top) = top
        .as_ref()
        .filter(|_| !st.read_only && !patches.is_empty())
    {
        st.steps.push(Step {
            name: "Integration".into(),
            status: "running".into(),
            detail: String::new(),
        });
        publish(app, core, s, out, &st);
        let mut conflicts = vec![];
        let mut applied = vec![];
        for t in &st.subtasks {
            if let Some(p) = patches.get(&t.id) {
                match gitops::apply_patch(top, p).await {
                    Ok(()) => applied.push(t.title.clone()),
                    Err(_) => conflicts.push((t.clone(), p.clone())),
                }
            }
        }
        let step = st.steps.len() - 1;
        if !conflicts.is_empty() {
            st.steps[step].detail = format!(
                "{} conflict(s), the orchestrator is merging",
                conflicts.len()
            );
            publish(app, core, s, out, &st);
            let listing: String = conflicts
                .iter()
                .map(|(t, p)| {
                    format!(
                        "\nSubtask {} ({}):\n{}\n",
                        t.id,
                        t.title,
                        gitops::truncate(p.clone(), 30_000)
                    )
                })
                .collect();
            let merge = format!(
                "You are the orchestrator integrating your team's work. The changes of the other subtasks are already applied to the project. These patches could not be applied cleanly; apply their intent by hand, resolving conflicts with minimal edits, and keep every other change intact.\n\nRequest:\n{}\n{listing}",
                st.task
            );
            let mut scratch = Message {
                id: out.id.clone(),
                role: "assistant".into(),
                text: String::new(),
                activity: vec![],
            };
            let r = turns::stage(
                app,
                core,
                s,
                &mut scratch,
                &orch,
                &merge,
                model.as_deref(),
                &st.effort,
                options.access(),
                cancel.clone(),
                false,
            )
            .await;
            if let Err(e) = r {
                st.steps[step].status = "failed".into();
                st.steps[step].detail = turns::friendly(&e);
                st.stage = "failed".into();
                st.lanes = lanes
                    .values()
                    .map(|l| {
                        (
                            top.to_string_lossy().into_owned(),
                            l.path.to_string_lossy().into_owned(),
                            l.branch.clone(),
                        )
                    })
                    .collect();
                out.text = format!(
                    "**Orchestra** could not integrate every subtask. The work is kept in these branches:\n\n{}",
                    st.lanes
                        .iter()
                        .map(|(_, _, b)| format!("- `{b}`"))
                        .collect::<Vec<_>>()
                        .join("\n")
                );
                event(app, &s.id, &out.id, "replace", &out.text);
                publish(app, core, s, out, &st);
                return Err(
                    "Integration failed. Keep the branches to merge by hand, or discard them."
                        .into(),
                );
            }
        }
        st.steps[step].status = "done".into();
        if conflicts.is_empty() {
            st.steps[step].detail = format!("{} subtask(s) applied", applied.len());
        }
        out.activity.push(Activity {
            kind: "file".into(),
            text: st
                .subtasks
                .iter()
                .filter(|t| patches.contains_key(&t.id))
                .map(|t| t.title.clone())
                .collect::<Vec<_>>()
                .join("\n"),
        });
    }
    cleanup(&top, &lanes).await;

    // The orchestrator checks the combined result and writes the final answer.
    let diff = match (&top, base.is_empty()) {
        (Some(top), false) => gitops::diff_since(top, &base, 60_000)
            .await
            .unwrap_or_default(),
        _ => String::new(),
    };
    let results: String = st
        .subtasks
        .iter()
        .map(|t| {
            format!(
                "\n### {} · {} · {}\n{}\n",
                t.id,
                t.title,
                t.status,
                t.output.chars().take(3000).collect::<String>()
            )
        })
        .collect();
    let final_prompt = format!(
        "You are the orchestrator. Your team finished the subtasks below{}. {}Then write the final reply for the user: what was done, the files changed, and anything that failed or remains.\n\nRequest:\n{}\n\nClarifications:\n{}\n\nSubtask results:{results}\n{}",
        if failed.is_empty() { String::new() } else { format!(" (failed: {})", failed.join(", ")) },
        if st.read_only { "Combine their findings into one coherent answer. ".to_string() } else { "Check the combined changes for inconsistencies between the parts and fix them with minimal edits. ".to_string() },
        st.task,
        st.answers,
        if diff.trim().is_empty() { String::new() } else { format!("Combined changes (git diff -U3):\n{diff}") }
    );
    st.steps.push(Step {
        name: "Final check".into(),
        status: "running".into(),
        detail: String::new(),
    });
    publish(app, core, s, out, &st);
    out.text.clear();
    event(app, &s.id, &out.id, "replace", "");
    let access = if st.read_only {
        AccessMode::ReadOnly
    } else {
        options.access()
    };
    let mut draft = turns::stage(
        app,
        core,
        s,
        out,
        &orch,
        &final_prompt,
        model.as_deref(),
        &st.effort,
        access,
        cancel.clone(),
        true,
    )
    .await?;
    let last = st.steps.len() - 1;
    st.steps[last].status = "done".into();
    publish(app, core, s, out, &st);

    // Cross-audit of the integration by the provider opposite the orchestrator.
    if options.auto_review {
        if let Some(reviewer) = reviewer_for(core, &orch.provider) {
            let rounds = core
                .settings
                .lock()
                .map(|s| s.review_rounds)
                .unwrap_or(1)
                .clamp(1, 3);
            st.steps.push(Step {
                name: format!("Audit by {}", display(&reviewer.provider)),
                status: "running".into(),
                detail: String::new(),
            });
            let step = st.steps.len() - 1;
            publish(app, core, s, out, &st);
            for round in 1..=rounds {
                let diff = match (&top, base.is_empty()) {
                    (Some(top), false) => gitops::diff_since(top, &base, 60_000)
                        .await
                        .unwrap_or_default(),
                    _ => String::new(),
                };
                let review_prompt = format!("You are auditing the integrated result of a team of AI agents. Read-only: never edit files. Find bugs, inconsistencies between parts, risks and missing requirements, and propose concrete corrections. If the work is correct and complete, reply with the single word APPROVED on the first line.\n\nRequest:\n{}\n\nClarifications:\n{}\n\nFinal answer:\n{draft}\n\nChanges (git diff -U3):\n{diff}", st.task, st.answers);
                let review = hidden(
                    app,
                    core,
                    s,
                    out,
                    &reviewer,
                    &review_prompt,
                    None,
                    &st.effort,
                    cancel.clone(),
                )
                .await?;
                turns::note(
                    app,
                    &s.id,
                    out,
                    "review",
                    format!(
                        "Audit by {} · round {round}/{rounds}\n\n{review}",
                        reviewer.provider.binary()
                    ),
                );
                if turns::approved(&review) {
                    st.steps[step].detail = format!("Approved in round {round}");
                    break;
                }
                st.steps[step].detail = format!("Round {round}: revising");
                publish(app, core, s, out, &st);
                let revise = format!("Another assistant audited the integrated result. Address every valid point (ignore incorrect ones) with minimal edits and give the final answer. Do not claim to have run checks unless you actually did.\n\nRequest:\n{}\n\nYour answer:\n{draft}\n\nAudit:\n{review}", st.task);
                out.text.clear();
                event(app, &s.id, &out.id, "replace", "");
                draft = turns::stage(
                    app,
                    core,
                    s,
                    out,
                    &orch,
                    &revise,
                    model.as_deref(),
                    &st.effort,
                    access,
                    cancel.clone(),
                    true,
                )
                .await?;
            }
            st.steps[step].status = "done".into();
            publish(app, core, s, out, &st);
        }
    }
    out.text = draft;
    turns::self_heal(
        app,
        core,
        s,
        out,
        &orch,
        model.as_deref(),
        &st.effort,
        options,
        cancel,
    )
    .await?;
    st.stage = "done".into();
    publish(app, core, s, out, &st);
    Ok(())
}

async fn cleanup(top: &Option<PathBuf>, lanes: &HashMap<String, Lane>) {
    if let Some(top) = top {
        for l in lanes.values() {
            gitops::remove_lane(top, &l.path, &l.branch).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn acc(id: &str, p: Provider) -> Account {
        Account {
            id: id.into(),
            name: id.into(),
            provider: p,
        }
    }

    #[test]
    fn json_is_found_inside_prose_and_fences() {
        let (q, c) = parse_questions("Sure!\n```json\n{\"questions\":[{\"q\":\"Which DB?\",\"recommended\":\"SQLite\"}]}\n```");
        assert_eq!(
            q,
            vec![Question {
                q: "Which DB?".into(),
                recommended: "SQLite".into()
            }]
        );
        assert!(c.is_empty());
        let (q, c) = parse_questions("{\"confirm\":\"I'd add a button.\"}");
        assert!(q.is_empty());
        assert_eq!(c, "I'd add a button.");
        assert_eq!(parse_questions("no json"), (vec![], String::new()));
    }

    #[test]
    fn plans_are_sanitized_into_an_acyclic_graph() {
        let (summary, tasks) = parse_plan(r#"{"summary":"Two parts","subtasks":[
            {"id":"api","title":"API","detail":"d","files":["a.rs"],"difficulty":"complex","writes":true,"dependsOn":["ui"]},
            {"id":"ui","title":"UI","detail":"d","files":["b.tsx"],"difficulty":"weird","writes":true,"dependsOn":["api","ghost"]},
            {"id":"x","title":"","detail":""}
        ]}"#)
        .unwrap();
        assert_eq!(summary, "Two parts");
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].id, "s1");
        assert!(tasks[0].depends_on.is_empty(), "forward dependency dropped");
        assert_eq!(tasks[1].depends_on, vec!["s1".to_string()]);
        assert_eq!(tasks[1].difficulty, "normal");
        assert!(parse_plan("{\"subtasks\":[]}").is_none());
    }

    #[test]
    fn trivial_work_goes_to_emergency_agents_and_the_rest_is_spread() {
        let accounts = vec![
            acc("c", Provider::Claude),
            acc("g", Provider::Gpt),
            acc("o", Provider::Ollama),
        ];
        let mk = |d: &str, w: bool| Subtask {
            difficulty: d.into(),
            writes: w,
            ..Default::default()
        };
        let mut t = vec![
            mk("complex", true),
            mk("normal", true),
            mk("trivial", false),
            mk("trivial", true),
        ];
        choose_accounts(&mut t, &accounts, &Provider::Claude);
        assert_eq!(
            t[0].account_id, "g",
            "starts with the non-orchestrating provider"
        );
        assert_eq!(t[1].account_id, "c");
        assert_eq!(t[2].account_id, "o", "Ollama takes trivial text work");
        assert_ne!(t[3].account_id, "o", "Ollama never takes writing work");
    }

    #[test]
    fn orchestrator_and_effort_follow_the_policy() {
        assert_eq!(orchestrator_tier(&AutoPolicy::Budget, 0), 1);
        assert_eq!(orchestrator_tier(&AutoPolicy::Default, 2), 2);
        assert_eq!(orchestrator_tier(&AutoPolicy::Quality, 0), 2);
        assert_eq!(effort_for(2, &AutoPolicy::Budget), "low");
        assert_eq!(effort_for(1, &AutoPolicy::Quality), "medium");
    }

    #[test]
    fn edits_reassign_remove_and_protect_writing_work() {
        let accounts = vec![acc("c", Provider::Claude), acc("o", Provider::Ollama)];
        let mut st = State {
            subtasks: vec![
                Subtask {
                    id: "s1".into(),
                    writes: true,
                    account_id: "c".into(),
                    ..Default::default()
                },
                Subtask {
                    id: "s2".into(),
                    depends_on: vec!["s1".into()],
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let ollama_on_writer = [Edit {
            id: "s1".into(),
            account_id: Some("o".into()),
            remove: false,
        }];
        assert!(apply_edits(&mut st.clone(), &ollama_on_writer, &accounts).is_err());
        apply_edits(
            &mut st,
            &[Edit {
                id: "s1".into(),
                account_id: None,
                remove: true,
            }],
            &accounts,
        )
        .unwrap();
        assert_eq!(st.subtasks.len(), 1);
        assert!(st.subtasks[0].depends_on.is_empty());
        assert!(apply_edits(
            &mut st,
            &[Edit {
                id: "s2".into(),
                account_id: None,
                remove: true
            }],
            &accounts
        )
        .is_err());
    }

    #[test]
    fn answers_fall_back_to_recommendations() {
        let st = State {
            questions: vec![
                Question {
                    q: "A?".into(),
                    recommended: "yes".into(),
                },
                Question {
                    q: "B?".into(),
                    recommended: "no".into(),
                },
            ],
            ..Default::default()
        };
        let t = answers_text(&st, &["".into(), "maybe".into()], "", false);
        assert!(t.contains("→ yes") && t.contains("→ maybe"));
        assert!(answers_text(&st, &[], "", true).starts_with("Skip"));
    }
}

// ---------------------------------------------------------------- commands

fn load(core: &Core, id: &str, message: &str) -> Result<(Message, State), String> {
    let m = core
        .store
        .messages(id)?
        .into_iter()
        .find(|m| m.id == message)
        .ok_or("This Orchestra step is no longer available")?;
    let st = state_of(&m).ok_or("This message has no Orchestra plan")?;
    Ok((m, st))
}

fn save_state(
    app: &tauri::AppHandle,
    core: &Core,
    id: &str,
    m: &mut Message,
    st: &State,
) -> Result<(), String> {
    let text = serde_json::to_string(st).map_err(|e| e.to_string())?;
    if let Some(a) = m.activity.iter_mut().find(|a| a.kind == "orchestra") {
        a.text = text.clone();
    }
    core.store.save_message(id, m)?;
    event(app, id, &m.id, "orchestra", &text);
    Ok(())
}

fn user_message(text: String) -> Message {
    Message {
        id: crate::models::id(),
        role: "user".into(),
        text,
        activity: vec![],
    }
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn orchestra_answer(
    app: tauri::AppHandle,
    core: tauri::State<'_, Arc<Core>>,
    id: String,
    message: String,
    answers: Vec<String>,
    note: Option<String>,
    skip: bool,
    options: RunOptions,
) -> Result<(), String> {
    let (mut m, mut st) = load(&core, &id, &message)?;
    if st.stage != "questions" {
        return Err("These questions were already answered.".into());
    }
    st.answers = answers_text(&st, &answers, note.as_deref().unwrap_or(""), skip);
    let user = user_message(st.answers.clone());
    st.stage = "answered".into();
    save_state(&app, &core, &id, &mut m, &st)?;
    st.read_only = options.access() == AccessMode::ReadOnly;
    turns::spawn_turn(
        app,
        core.inner().clone(),
        id,
        user,
        move |app, core, s, mut out, rx| async move {
            let r = plan(&app, &core, &s, &mut out, st, &options, rx).await;
            (out, r)
        },
    )
    .await
}

#[tauri::command]
pub async fn orchestra_approve(
    app: tauri::AppHandle,
    core: tauri::State<'_, Arc<Core>>,
    id: String,
    message: String,
    edits: Vec<Edit>,
    options: RunOptions,
) -> Result<(), String> {
    let (mut m, mut st) = load(&core, &id, &message)?;
    if st.stage != "plan" {
        return Err("This plan was already handled.".into());
    }
    let accounts = core.store.accounts()?;
    apply_edits(&mut st, &edits, &accounts)?;
    assign(&core, &mut st, &options.auto_policy).await?;
    st.read_only = options.access() == AccessMode::ReadOnly;
    let mut run = st.clone();
    st.stage = "approved".into();
    save_state(&app, &core, &id, &mut m, &st)?;
    let mut user = user_message("Approved the plan.".into());
    let session = core.store.session(&id)?;
    let mut checkpoint = None;
    if !run.read_only {
        if let Some(top) = gitops::toplevel(Path::new(&session.workspace)).await {
            if let Ok(tree) = gitops::checkpoint(&top, &user.id).await {
                checkpoint = Some(tree.clone());
                user.activity.push(Activity {
                    kind: "checkpoint".into(),
                    text: tree,
                });
            }
        }
    }
    run.steps.clear();
    turns::spawn_turn(
        app,
        core.inner().clone(),
        id,
        user,
        move |app, core, s, mut out, rx| async move {
            let r = execute(&app, &core, &s, &mut out, run, &options, checkpoint, rx).await;
            (out, r)
        },
    )
    .await
}

#[tauri::command]
pub fn orchestra_cancel(
    app: tauri::AppHandle,
    core: tauri::State<'_, Arc<Core>>,
    id: String,
    message: String,
) -> Result<(), String> {
    let (mut m, mut st) = load(&core, &id, &message)?;
    if !["questions", "plan"].contains(&st.stage.as_str()) {
        return Err("This step can no longer be cancelled.".into());
    }
    st.stage = "cancelled".into();
    save_state(&app, &core, &id, &mut m, &st)
}

/// After a failed integration: keep the branches for manual merging, or discard them.
#[tauri::command]
pub async fn orchestra_lanes(
    app: tauri::AppHandle,
    core: tauri::State<'_, Arc<Core>>,
    id: String,
    message: String,
    keep: bool,
) -> Result<(), String> {
    let (mut m, mut st) = load(&core, &id, &message)?;
    if st.stage != "failed" {
        return Err("Nothing to keep or discard.".into());
    }
    if !keep {
        for (top, path, branch) in &st.lanes {
            gitops::remove_lane(Path::new(top), Path::new(path), branch).await;
        }
    } else {
        // The branches stay; only the temporary worktree folders go.
        for (top, path, _) in &st.lanes {
            let _ = tokio::process::Command::new("git")
                .current_dir(top)
                .args(["worktree", "remove", "--force", path])
                .output()
                .await;
        }
    }
    st.stage = if keep { "kept" } else { "discarded" }.into();
    save_state(&app, &core, &id, &mut m, &st)
}

#[tauri::command]
pub async fn orchestra_init_git(
    core: tauri::State<'_, Arc<Core>>,
    id: String,
) -> Result<(), String> {
    let s = core.store.session(&id)?;
    gitops::init(Path::new(&s.workspace)).await
}
