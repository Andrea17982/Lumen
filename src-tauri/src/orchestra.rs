//! Orchestra: for complex Auto tasks, one orchestrator model asks clarifying
//! questions, splits the work into subtasks, shows the plan for approval, runs
//! the subtasks in parallel git worktrees across every available provider and
//! integrates the result.
//!
//! The whole flow lives in assistant messages: each carries one `orchestra`
//! activity with the JSON [`State`], replaced in place as the run advances, so
//! the interface can redraw the card and the history survives restarts.
use crate::{
    account, config_dir, event, gitops,
    models::*,
    models_for, providers, resolve_effort, router,
    scheduling::{self, Outcome, Scheduler, Wait},
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

/// How often a run with queued work re-reads quotas, cooldowns and raised limits.
const TICK_MS: u64 = 30_000;
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
    /// Shared contracts, dependents and relevant checks for this worker's owned files.
    pub impact: String,
    /// "trivial", "normal" or "complex".
    pub difficulty: String,
    pub writes: bool,
    pub depends_on: Vec<String>,
    pub account_id: String,
    pub agent: String,
    pub model: String,
    pub effort: String,
    /// Chosen by the user in the plan editor: the scheduler never moves it.
    pub pinned: bool,
    /// pending · queued · running · done · failed · skipped
    pub status: String,
    pub output: String,
    pub started_at: i64,
    pub ended_at: i64,
    /// Attempts started so far, kept across a restart.
    pub attempts: u32,
    /// Process of the running attempt, checked before a resume.
    pub pid: u32,
    /// Changes of a finished writing subtask, kept so a resume need not rerun it.
    pub patch: String,
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
    /// Conversation snapshot shared with the planner and workers, including prior decisions.
    pub context: String,
    pub execution_authorized: bool,
    /// Root where file ownership and the plan were determined.
    pub workspace: String,
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
    /// Why queued subtasks wait, while the run is in progress.
    pub queue: Option<Queue>,
    /// Tree the subtask lanes start from; a resume reuses it.
    pub base: String,
    /// Options of the approved run, so it can be resumed after a restart.
    pub options: Option<RunOptions>,
    /// Interrupted while subtasks ran: completed work is kept and the rest can resume.
    pub resumable: bool,
    /// Why the last resume was refused, shown on the card.
    pub notice: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Waiting {
    pub id: String,
    /// concurrency · total-cap · probe · cooldown · quota
    pub reason: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Queue {
    pub running: usize,
    pub admitted: usize,
    pub concurrent: usize,
    pub total: Option<usize>,
    pub waiting: Vec<Waiting>,
    pub wake_at: Option<i64>,
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
    // Accounts whose catalog cannot load (signed out, CLI missing) are skipped,
    // and their subtasks go to the next agent instead of failing at run time.
    let mut usable = accounts;
    for _ in 0..4 {
        choose_accounts(&mut st.subtasks, &usable, &orchestrator);
        let mut broken = vec![];
        for t in st.subtasks.iter_mut().filter(|t| t.model.is_empty()) {
            let Some(a) = usable.iter().find(|a| a.id == t.account_id) else {
                continue;
            };
            match models_for(core, a, false, None).await {
                Ok(models) => {
                    if let Some(m) = router::select_tier(&models, tier_of(&t.difficulty)) {
                        t.model = m.id.clone();
                        t.agent = format!("{} · {}", display(&a.provider), m.name);
                    }
                }
                Err(_) => {
                    broken.push(a.id.clone());
                    t.account_id.clear();
                }
            }
        }
        if broken.is_empty() {
            break;
        }
        usable.retain(|a| !broken.contains(&a.id));
        if usable.is_empty() {
            return Err(
                "No connected account can load its models. Check the accounts in Settings.".into(),
            );
        }
    }
    for t in st.subtasks.iter_mut() {
        let Some(a) = usable.iter().find(|a| a.id == t.account_id) else {
            continue;
        };
        let tier = tier_of(&t.difficulty);
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

/// A visible orchestrator turn; on a rate limit the other primary provider takes
/// over for the rest of the run instead of the run stopping.
#[allow(clippy::too_many_arguments)]
async fn orchestrator_stage(
    app: &tauri::AppHandle,
    core: &Core,
    s: &Session,
    out: &mut Message,
    orch: &mut Account,
    model: &mut Option<String>,
    prompt: &str,
    effort: &str,
    access: AccessMode,
    cancel: watch::Receiver<bool>,
) -> Result<String, String> {
    let first = turns::stage(
        app,
        core,
        s,
        out,
        orch,
        prompt,
        model.as_deref(),
        effort,
        access,
        cancel.clone(),
        true,
    )
    .await;
    let Err(e) = &first else { return first };
    if !e.starts_with(crate::limits::RATE_LIMIT) {
        return first;
    }
    let Some(next) = reviewer_for(core, &orch.provider) else {
        return first;
    };
    turns::note(
        app,
        &s.id,
        out,
        "coordination",
        format!(
            "{} hit a rate limit; {} takes over.",
            display(&orch.provider),
            display(&next.provider)
        ),
    );
    *orch = next;
    *model = None;
    out.text.clear();
    event(app, &s.id, &out.id, "replace", "");
    turns::stage(
        app, core, s, out, orch, prompt, None, effort, access, cancel, true,
    )
    .await
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
    core: &Arc<Core>,
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
        workspace: s.workspace.clone(),
        ..Default::default()
    };
    publish(app, core, s, out, &st);
    let (context, history) = turns::build_prompt(core, s, &[&out.id], options).await?;
    st.context = history;
    let prompt = format!(
        "{context}\n\n---\nYou are the orchestrator of a team of AI agents that will work on the request above in parallel. Ask only for missing information that materially changes the result. Preserve the scope and authorizations already given in this conversation; a request to proceed continues that work. You may read project files during this preparation call; do not edit them in this call. This temporary restriction does NOT make the requested work read-only. Do not propose a separate read-only investigation when implementation was requested.\nReply with JSON only, in one of these two shapes:\n{{\"questions\":[{{\"q\":\"a question that changes the result\",\"recommended\":\"your recommended answer\"}}]}}  (at most 6 questions, each with a recommendation)\n{{\"confirm\":\"I'd do X, Y and Z.\"}}  (when the request is already clear)"
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
        cancel.clone(),
    )
    .await?;
    let (questions, confirm) = parse_questions(&reply);
    st.questions = questions;
    if st.questions.is_empty() {
        return self::plan(app, core, s, out, st, options, cancel).await;
    }
    st.confirm = confirm;
    st.stage = "questions".into();
    let mut t = "**Orchestra** · a few questions before planning:\n\n".to_string();
    for (i, q) in st.questions.iter().enumerate() {
        t.push_str(&format!(
            "{}. {}\n   *Recommended:* {}\n",
            i + 1,
            q.q,
            q.recommended
        ));
    }
    out.text = t;
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

fn planning_prompt(st: &State, context: &str) -> String {
    let schema = "Reply with JSON only:\n{\"summary\":\"one sentence\",\"subtasks\":[{\"id\":\"s1\",\"title\":\"short title\",\"detail\":\"precise, self-contained instructions\",\"files\":[\"paths this subtask owns\"],\"difficulty\":\"trivial|normal|complex\",\"writes\":true,\"dependsOn\":[]}]}";
    let rules = "Rules: 1 to 8 subtasks, parallel only when useful, each in its own copy of the repository. Preserve the complete scope, decisions and authorizations in the conversation. A follow-up such as 'proceed' means implement the previously agreed work, not recover a missing plan. When implementation is requested and file edits are enabled, assign implementation and verification tasks with writes=true; include necessary inspection inside those tasks rather than making a plan consisting only of research. The restriction on edits applies ONLY to this planning call, not to the work being planned. Give each subtask clear file ownership and never let two subtasks edit the same file. Use dependsOn only when a subtask truly needs another one's result. difficulty is trivial (messages, text, renames, tiny edits), normal or complex. writes is false for research or text-only work. You will integrate the results yourself at the end, so do not add an integration subtask.";
    format!(
        "{context}\n\nYou are the orchestrator of a team of AI agents. Plan the work below. You may read project files; do not edit them during this planning call.\n\nRequest:\n{}\n\nClarifications from the user:\n{}\n\n{rules}\n{schema}",
        st.task, st.answers
    )
}

/// A ready plan is an execution boundary in Autopilot or after explicit approval.
/// Do not briefly publish an actionable approval card while starting that execution.
fn ready_plan(st: &mut State, options: &RunOptions) -> bool {
    let authorized = options.access() == AccessMode::Autopilot || st.execution_authorized;
    st.stage = if authorized { "approved" } else { "plan" }.into();
    authorized
}

/// Stage 2: split into subtasks, assign agents, optionally audit the plan.
#[allow(clippy::too_many_arguments)]
pub async fn plan(
    app: &tauri::AppHandle,
    core: &Arc<Core>,
    s: &Session,
    out: &mut Message,
    mut st: State,
    options: &RunOptions,
    cancel: watch::Receiver<bool>,
) -> Result<(), String> {
    let orch = account(core, &st.orchestrator)?;
    st.stage = "planning".into();
    st.workspace = s.workspace.clone();
    st.subtasks.clear();
    publish(app, core, s, out, &st);
    let (context, history) = turns::build_prompt(core, s, &[&out.id], options).await?;
    st.context = history;
    let prompt = planning_prompt(&st, &context);
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
                "{context}\n\nYou are auditing a work plan written by another AI orchestrator. Read-only during this audit call: never edit files. Check that the plan implements the full agreed request (not just research when changes were requested), that file ownership does not overlap, that dependencies are right and that each assignment fits the difficulty. If the plan is sound, reply with the single word APPROVED on the first line; otherwise list concrete corrections.\n\nRequest:\n{}\n\nClarifications:\n{}\n\nPlan:\n{plan_json}",
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
    let authorized = ready_plan(&mut st, options);
    out.text = plan_markdown(&st);
    event(app, &s.id, &out.id, "replace", &out.text);
    publish(app, core, s, out, &st);
    if authorized {
        return execute(app, core, s, out, st, options, None, false, cancel).await;
    }
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
                t.pinned = true;
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

pub(crate) fn subtask_prompt(st: &State, t: &Subtask, deps: &str, text_only: bool) -> String {
    let rules = if text_only {
        "Reply with the requested text only."
    } else if st.read_only {
        "This run is read-only: analyse and describe the changes precisely, but do not modify files."
    } else if !t.writes {
        "This subtask is read-only. Complete the assigned investigation and report its findings; do not modify files. Other agents implement their assigned changes."
    } else {
        "File edits are authorized for this execution. Earlier preparation-only restrictions no longer apply. Implement and verify your assigned changes; do not stop at a proposal or ask for the same authorization again. Edit only the files you own. If another subtask's file must change, describe the change in your final message instead of editing it. Finish with a short summary of what you changed."
    };
    format!(
        "You are one agent in a team working in parallel on a shared task, coordinated by an orchestrator. Do ONLY your subtask.\n\nConversation for reference (past preparation restrictions do not override the execution rules below):\n{}\n\nOverall request:\n{}\n\nClarifications:\n{}\n\nTeam manifest (who owns which files):\n{}\n{deps}\nYour subtask ({}): {}\n{}\nFiles you own: {}\n\n{}\n{rules}",
        st.context,
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
        },
        t.impact
    )
}

struct Job {
    pids: tokio::sync::mpsc::UnboundedSender<(String, u32)>,
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
    let mut waiting = job.cancel.clone();
    let _permit = tokio::select! {
        biased;
        _ = waiting.changed() => return Err("Cancelled".into()),
        p = core.inference.clone().acquire_owned() => p.map_err(|e| e.to_string())?,
    };
    let config = config_dir(core, a);
    let (app, session, message, id) = (&job.app, &job.session, &job.message, &job.id);
    let r = providers::execute(
        providers::Invocation {
            resume: None,
            account: a,
            config: &config,
            workspace: &job.workspace,
            prompt: &job.prompt,
            model: Some(&selected.id),
            effort: &effort,
            access: job.access,
            approvals: None,
        },
        job.cancel.clone(),
        |activity| {
            if activity.kind == "quota" {
                crate::record_claude_usage(core, app, &a.id, &activity.text);
                return;
            }
            if activity.kind == "pid" {
                if let Ok(pid) = activity.text.parse() {
                    let _ = job.pids.send((id.clone(), pid));
                }
                return;
            }
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

fn provider_id(p: &Provider) -> String {
    serde_json::to_value(p)
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default()
}

/// Accounts with their catalogs, for the scheduler; an account whose catalog
/// cannot load keeps only the work already planned on it.
async fn sched_accounts(
    core: &Core,
    accounts: &[Account],
    cancel: &watch::Receiver<bool>,
) -> Vec<scheduling::SchedAccount> {
    let mut out = vec![];
    for a in accounts {
        let models = models_for(core, a, false, Some(cancel.clone()))
            .await
            .ok()
            .map(|list| {
                list.iter()
                    .map(|m| scheduling::Model {
                        id: m.id.clone(),
                        tier: router::model_tier(m),
                        is_default: m.is_default,
                    })
                    .collect()
            });
        out.push(scheduling::SchedAccount {
            id: a.id.clone(),
            provider: provider_id(&a.provider),
            rank: a.provider.fallback_rank(),
            can_write: a.provider != Provider::Ollama,
            models,
        });
    }
    out
}

/// Feed the scheduler the latest quota snapshots and rate-limit cooldowns.
fn refresh_quota(core: &Core, sched: &mut Scheduler, accounts: &[Account]) {
    let t = now();
    for u in crate::read_usage(core).unwrap_or_default() {
        sched.set_usage(
            &u.account_id,
            scheduling::Usage {
                updated_at: u.updated_at,
                windows: u
                    .windows
                    .iter()
                    .map(|w| scheduling::Window {
                        used_percent: w.used_percent,
                        resets_at: w.resets_at,
                        window_minutes: w.window_minutes,
                    })
                    .collect(),
            },
        );
    }
    for a in accounts {
        if core.limits.cooling(&a.id, t) {
            sched.cool_down(&a.id, t + TICK_MS as i64);
        }
    }
}

fn wait_name(w: Wait) -> &'static str {
    match w {
        Wait::Dependencies => "dependencies",
        Wait::Concurrency => "concurrency",
        Wait::TotalCap => "total-cap",
        Wait::Probe => "probe",
        Wait::Cooldown => "cooldown",
        Wait::Quota => "quota",
    }
}

fn queue_of(sched: &Scheduler, t: i64) -> Option<Queue> {
    let (waits, wake_at) = sched.waiting(t);
    let waiting: Vec<Waiting> = waits
        .into_iter()
        .filter(|(_, w)| *w != Wait::Dependencies)
        .map(|(id, w)| Waiting {
            id,
            reason: wait_name(w).into(),
        })
        .collect();
    (!waiting.is_empty()).then(|| Queue {
        running: sched.running(),
        admitted: sched.admitted(),
        concurrent: sched.limits.concurrent(),
        total: sched.limits.max_subtasks,
        waiting,
        wake_at,
    })
}

/// Classify a finished attempt for the scheduler, which owns every retry.
fn outcome_of(core: &Core, account: &str, r: &Result<String, String>) -> Outcome {
    match r {
        Ok(_) => Outcome::Done,
        Err(e) if e == "Cancelled" => Outcome::Cancelled,
        Err(e) if e.starts_with(crate::limits::RATE_LIMIT) => {
            let msg = e.trim_start_matches(crate::limits::RATE_LIMIT);
            let until = crate::limits::unlock_at(msg, now());
            core.limits.cool(account, until);
            Outcome::RateLimited(Some(until))
        }
        Err(e) => Outcome::Failed(e.clone()),
    }
}

/// One attempt; the scheduler decides whether and where to retry.
async fn run_job(job: Job) -> (String, String, Result<String, String>) {
    let model = Some(job.model.as_str()).filter(|m| !m.is_empty());
    let r = attempt(&job, &job.account, model).await;
    (job.id.clone(), job.account.id.clone(), r)
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
    resume: bool,
    mut cancel: watch::Receiver<bool>,
) -> Result<(), String> {
    if !st.workspace.is_empty()
        && dunce::canonicalize(&st.workspace).ok() != dunce::canonicalize(&s.workspace).ok()
    {
        return Err("The conversation folder changed after this plan was prepared. Replan in the current folder before executing or resuming it.".into());
    }
    let mut orch = account(core, &st.orchestrator)?;
    let workspace = PathBuf::from(&s.workspace);
    let top = gitops::toplevel(&workspace).await;
    st.read_only = options.access() == AccessMode::ReadOnly;
    if !st.read_only && top.is_none() {
        return Err("Orchestra needs a Git repository in the conversation's current folder to apply changes. Change the conversation folder or initialize the repository.".into());
    }
    let lock = turns::workspace_lock(core, s)?;
    let other_sessions = turns::conversations_on_workspace(core, s);
    if !other_sessions.is_empty() && !st.read_only {
        turns::note(
            app,
            &s.id,
            out,
            "coordination",
            format!(
                "Another Lumen conversation is active on this project ({}). Orchestra integrates changes only after the project write lock is released.",
                other_sessions.join(", ")
            ),
        );
    }
    let _guard = if st.read_only {
        None
    } else {
        Some(
            tokio::select! {biased;_=cancel.changed()=>return Err("Cancelled".into()),g=lock.lock()=>g},
        )
    };
    let checkpoint = if st.read_only || checkpoint.is_some() {
        checkpoint
    } else if let Some(user) = core
        .store
        .messages(&s.id)?
        .into_iter()
        .rev()
        .find(|m| m.role == "user")
    {
        turns::checkpoint_message(core, s, &user.id).await?
    } else {
        None
    };
    let base = match (&top, checkpoint) {
        _ if resume && !st.base.is_empty() => st.base.clone(),
        (_, Some(t)) => t,
        (Some(top), None) if !st.read_only => gitops::write_tree(top).await?,
        _ => String::new(),
    };
    let baseline = crate::impact::snapshot(core, &workspace, options.access()).await?;
    if let Some(graph) = &baseline {
        for task in &mut st.subtasks {
            if !task.files.is_empty() {
                task.impact =
                    crate::impact::handoff(&workspace, &graph.impact_details(&task.files, 3));
            }
        }
    }
    let rel = top
        .as_ref()
        .and_then(|t| workspace.strip_prefix(t).ok())
        .unwrap_or(Path::new(""))
        .to_path_buf();
    let short: String = out.id.chars().take(8).collect();
    st.stage = "running".into();
    st.base = base.clone();
    st.options = Some(options.clone());
    st.resumable = false;
    st.notice.clear();
    st.steps.clear();
    // A resume keeps finished work; everything else starts over.
    for t in st.subtasks.iter_mut() {
        if resume && matches!(t.status.as_str(), "done" | "failed") {
            continue;
        }
        t.status = "pending".into();
        t.output.clear();
        t.patch.clear();
        t.pid = 0;
        if !resume {
            t.attempts = 0;
        }
    }
    publish(app, core, s, out, &st);

    let accounts = core.store.accounts().unwrap_or_default();
    let limits = core
        .settings
        .lock()
        .map(|s| s.orchestra_limits.clone())
        .unwrap_or_default();
    let inputs = st
        .subtasks
        .iter()
        .map(|t| scheduling::SubtaskInput {
            id: t.id.clone(),
            tier: tier_of(&t.difficulty),
            writes: t.writes,
            depends_on: t.depends_on.clone(),
            account_id: t.account_id.clone(),
            model: t.model.clone(),
            effort: t.effort.clone(),
            pinned: t.pinned,
            status: if resume {
                t.status.clone()
            } else {
                String::new()
            },
            attempts: t.attempts,
        })
        .collect();
    let mut sched = Scheduler::new(
        inputs,
        sched_accounts(core, &accounts, &cancel).await,
        scheduling::Limits {
            max_concurrent: limits.max_concurrent,
            max_subtasks: limits.max_subtasks,
            quota_thresholds: limits.quota_thresholds,
        },
        scheduling::Options {
            read_only: st.read_only,
            ..Default::default()
        },
    );
    if let Ok(mut r) = core.orchestra_raises.lock() {
        r.remove(&out.id);
    }

    let mut lanes: HashMap<String, Lane> = HashMap::new();
    let mut patches: HashMap<String, String> = st
        .subtasks
        .iter()
        .filter(|t| t.status == "done" && !t.patch.is_empty())
        .map(|t| (t.id.clone(), t.patch.clone()))
        .collect();
    let (pid_tx, mut pid_rx) = tokio::sync::mpsc::unbounded_channel::<(String, u32)>();
    let mut notes: HashMap<String, String> = HashMap::new();
    let mut set = tokio::task::JoinSet::new();
    loop {
        refresh_quota(core, &mut sched, &accounts);
        let extra = core
            .orchestra_raises
            .lock()
            .ok()
            .and_then(|mut r| r.remove(&out.id))
            .unwrap_or(0);
        if extra > 0 {
            let admitted = sched.admitted();
            sched.limits.raise_total(admitted, extra);
        }
        // Start whatever the scheduler admits: dependencies done, under the caps and quotas.
        for start in sched.next(now()) {
            let Some(i) = st.subtasks.iter().position(|t| t.id == start.id) else {
                continue;
            };
            let Ok(a) = account(core, &start.account_id) else {
                let gone = Outcome::Failed("The assigned account no longer exists.".into());
                sched.finish(&start.id, gone, now());
                st.subtasks[i].status = "failed".into();
                st.subtasks[i].output = "The assigned account no longer exists.".into();
                continue;
            };
            if start.account_id != st.subtasks[i].account_id {
                let from = st.subtasks[i].agent.clone();
                let name = models_for(core, &a, false, None)
                    .await
                    .ok()
                    .and_then(|m| m.into_iter().find(|m| m.id == start.model))
                    .map_or_else(|| start.model.clone(), |m| m.name);
                st.subtasks[i].agent = format!("{} · {name}", display(&a.provider));
                notes.insert(
                    start.id.clone(),
                    format!(
                        "Moved from {from} to {} by the scheduler.",
                        display(&a.provider)
                    ),
                );
            }
            st.subtasks[i].account_id = a.id.clone();
            st.subtasks[i].model = start.model.clone();
            let t = st.subtasks[i].clone();
            let text_only = a.provider == Provider::Ollama;
            let writes = t.writes && !st.read_only && !text_only;
            let mut job_dir = workspace.clone();
            if writes {
                let top = top.as_ref().ok_or("Git repository required")?;
                // A retry starts again from a clean lane.
                if let Some(old) = lanes.remove(&t.id) {
                    gitops::remove_lane(top, &old.path, &old.branch).await;
                    st.lanes.retain(|(_, _, b)| *b != old.branch);
                }
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
                st.lanes.push((
                    top.to_string_lossy().into_owned(),
                    path.to_string_lossy().into_owned(),
                    branch.clone(),
                ));
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
            let prompt = format!(
                "Execution workspace: {}. Use this directory for every tool call and apply your changes here, including when the conversation mentions the original repository by absolute path.\n\n{}",
                job_dir.display(), subtask_prompt(&st, &t, &deps, text_only)
            );
            set.spawn(run_job(Job {
                pids: pid_tx.clone(),
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
            st.subtasks[i].attempts += 1;
            st.subtasks[i].pid = 0;
            st.subtasks[i].started_at = now();
            st.subtasks[i].ended_at = 0;
        }
        st.queue = queue_of(&sched, now());
        for t in st.subtasks.iter_mut() {
            let queued = st
                .queue
                .as_ref()
                .is_some_and(|q| q.waiting.iter().any(|w| w.id == t.id));
            if t.status == "pending" && queued {
                t.status = "queued".into();
            } else if t.status == "queued" && !queued {
                t.status = "pending".into();
            }
        }
        publish(app, core, s, out, &st);
        if set.is_empty() {
            if sched.complete() {
                break;
            }
            // Suspended. Work held only by the total cap cannot start by itself
            // once nothing runs, so it is skipped; cooldowns and quotas wait.
            let (waits, _) = sched.waiting(now());
            if waits
                .iter()
                .all(|(_, w)| matches!(w, Wait::TotalCap | Wait::Dependencies))
            {
                sched.cancel_pending();
                break;
            }
        }
        let tick = tokio::time::sleep(std::time::Duration::from_millis(TICK_MS));
        let joined = tokio::select! {
            biased;
            _ = cancel.changed() => {
                set.abort_all();
                for l in lanes.values() {
                    if let Some(top) = &top { gitops::remove_lane(top, &l.path, &l.branch).await; }
                }
                st.lanes.clear();
                st.queue = None;
                st.stage = "interrupted".into();
                publish(app, core, s, out, &st);
                return Err("Cancelled".into());
            }
            Some((id, pid)) = pid_rx.recv() => {
                // Saved at once: after a crash this is how a resume finds the process.
                if let Some(t) = st.subtasks.iter_mut().find(|t| t.id == id && t.status == "running") {
                    t.pid = pid;
                    publish(app, core, s, out, &st);
                }
                continue;
            }
            j = set.join_next(), if !set.is_empty() => j,
            _ = tick => continue,
        };
        let Some(Ok((id, acc, result))) = joined else {
            continue;
        };
        sched.finish(&id, outcome_of(core, &acc, &result), now());
        let Some(t) = st.subtasks.iter_mut().find(|t| t.id == id) else {
            continue;
        };
        t.ended_at = now();
        t.pid = 0;
        match (sched.state(&id), result) {
            (Some(scheduling::State::Done), Ok(text)) => {
                t.status = "done".into();
                t.output = match notes.get(&id) {
                    Some(n) => format!("{n}\n\n{text}"),
                    None => text,
                };
                if let Some(l) = lanes.get(&id) {
                    if let Ok(p) = gitops::lane_patch(&l.path, &l.start).await {
                        if !p.trim().is_empty() {
                            t.patch = p.clone();
                            patches.insert(id.clone(), p);
                        }
                    }
                }
            }
            (Some(scheduling::State::Pending), Err(e)) => {
                t.status = "pending".into();
                t.output = format!("{} Waiting to retry.", turns::friendly(&e));
            }
            (_, r) => {
                let e = sched
                    .error(&id)
                    .map(String::from)
                    .or(r.err())
                    .unwrap_or_default();
                t.status = "failed".into();
                t.output = turns::friendly(&e);
            }
        }
        t.output = t.output.chars().take(16_000).collect();
    }
    st.queue = None;
    for t in st
        .subtasks
        .iter_mut()
        .filter(|t| matches!(t.status.as_str(), "pending" | "queued"))
    {
        if let Some(e) = sched
            .error(&t.id)
            .filter(|_| sched.state(&t.id) == Some(scheduling::State::Failed))
        {
            t.status = "failed".into();
            t.output = turns::friendly(e);
        } else {
            t.status = "skipped".into();
            t.output = "Not run: the subtask limit for this run was reached.".into();
        }
    }

    let failed: Vec<String> = st
        .subtasks
        .iter()
        .filter(|t| t.status == "failed")
        .map(|t| format!("{} ({})", t.title, t.id))
        .collect();
    if !st.subtasks.iter().any(|t| t.status == "done") {
        cleanup(&top, &lanes).await;
        st.lanes.clear();
        st.stage = "failed".into();
        publish(app, core, s, out, &st);
        return Err("Every subtask failed. See the Orchestra card for details.".into());
    }
    let mut model = Some(st.orchestrator_model.clone()).filter(|m| !m.is_empty());

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
                "Conversation for reference:\n{}\n\nYou are the orchestrator integrating your team's work. The changes of the other subtasks are already applied to the project. These patches could not be applied cleanly; apply their intent by hand, resolving conflicts with minimal edits, and keep every other change intact.\n\nRequest:\n{}\n\nClarifications:\n{}\n{listing}",
                st.context, st.task, st.answers
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
    st.lanes.clear();
    for t in st.subtasks.iter_mut() {
        t.patch.clear();
    }

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
    let team_impact = st
        .subtasks
        .iter()
        .filter(|t| !t.impact.is_empty())
        .map(|t| format!("{}:\n{}", t.title, t.impact))
        .collect::<Vec<_>>()
        .join("\n");
    let final_prompt = format!(
        "Conversation for reference:\n{}\n\nShared contracts and affected tests:\n{team_impact}\n\nYou are the orchestrator. Your team finished the subtasks below{}. {}Then write the final reply for the user: what was done, the files changed, and anything that failed or remains.\n\nRequest:\n{}\n\nClarifications:\n{}\n\nSubtask results:{results}\n{}",
        st.context,
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
    let last = st.steps.len() - 1;
    let mut draft = match orchestrator_stage(
        app,
        core,
        s,
        out,
        &mut orch,
        &mut model,
        &final_prompt,
        &st.effort,
        access,
        cancel.clone(),
    )
    .await
    {
        Ok(d) => d,
        Err(e) => {
            st.steps[last].status = "failed".into();
            publish(app, core, s, out, &st);
            return Err(e);
        }
    };
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
                let review_prompt = format!("Conversation for reference:\n{}\n\nYou are auditing the integrated result of a team of AI agents. Read-only during this audit call: never edit files. Find bugs, inconsistencies between parts, risks and missing requirements, and propose concrete corrections. If the work is correct and complete, reply with the single word APPROVED on the first line.\n\nRequest:\n{}\n\nClarifications:\n{}\n\nFinal answer:\n{draft}\n\nChanges (git diff -U3):\n{diff}", st.context, st.task, st.answers);
                let review = match hidden(
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
                .await
                {
                    Ok(r) => r,
                    // The auditor ran out of quota: keep the checked answer.
                    Err(e) if e.starts_with(crate::limits::RATE_LIMIT) => {
                        st.steps[step].detail = "Skipped: rate limit".into();
                        break;
                    }
                    Err(e) => return Err(e),
                };
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
                let revise = format!("Conversation for reference:\n{}\n\nAnother assistant audited the integrated result. Address every valid point (ignore incorrect ones) with minimal edits and give the final answer. Do not claim to have run checks unless you actually did.\n\nRequest:\n{}\n\nClarifications:\n{}\n\nYour answer:\n{draft}\n\nAudit:\n{review}", st.context, st.task, st.answers);
                out.text.clear();
                event(app, &s.id, &out.id, "replace", "");
                draft = orchestrator_stage(
                    app,
                    core,
                    s,
                    out,
                    &mut orch,
                    &mut model,
                    &revise,
                    &st.effort,
                    access,
                    cancel.clone(),
                )
                .await?;
            }
            st.steps[step].status = "done".into();
            publish(app, core, s, out, &st);
        }
    }
    out.text = draft;
    let verification = turns::self_heal(
        app,
        core,
        s,
        out,
        &orch,
        model.as_deref(),
        &st.effort,
        options,
        baseline,
        cancel,
    )
    .await;
    if let Err(error) = verification {
        st.stage = "failed".into();
        publish(app, core, s, out, &st);
        return Err(error);
    }
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

    fn stored_plan(st: &State) -> Message {
        Message {
            id: "approved-work".into(),
            role: "assistant".into(),
            text: "Plan".into(),
            activity: vec![Activity {
                kind: "orchestra".into(),
                text: serde_json::to_string(st).unwrap(),
            }],
        }
    }

    #[test]
    fn chat_approval_keeps_the_pending_plan_instead_of_starting_over() {
        let st = State {
            stage: "plan".into(),
            task: "Implement radius, menus and message queue".into(),
            context: "The changes are approved; preserve existing work".into(),
            subtasks: vec![Subtask {
                id: "queue".into(),
                writes: true,
                files: vec!["App.tsx".into()],
                ..Default::default()
            }],
            ..Default::default()
        };
        let history = vec![stored_plan(&st)];
        for approval in [
            "Ok procedi con le modifiche ora.",
            "Agreed.",
            "Approved the plan.",
            "vai, fallo",
            "proceed with the approved plan",
            "Procedi secondo il piano approvato",
        ] {
            let (id, continued) = continuation(&history, approval).unwrap();
            assert_eq!(id, history[0].id);
            assert_eq!(continued, st, "{approval}");
        }
        for message in [
            "Non procedere",
            "Agreed, but remove the queue",
            "Mostrami il piano",
            "procedi?",
            "Ecco una citazione: Agreed.",
        ] {
            assert!(continuation(&history, message).is_none(), "{message}");
        }
        for stage in ["done", "cancelled", "superseded", "running"] {
            let other = State {
                stage: stage.into(),
                ..st.clone()
            };
            assert!(continuation(&[stored_plan(&other)], "procedi").is_none());
        }
        let questions = State {
            stage: "questions".into(),
            questions: vec![Question {
                q: "Which project?".into(),
                recommended: "Lumen".into(),
            }],
            ..st
        };
        assert!(continuation(&[stored_plan(&questions)], "procedi").is_none());
    }

    #[tokio::test]
    async fn planner_and_workers_receive_prior_decisions_after_a_short_follow_up() {
        use crate::test_support::Fixture;
        let fixture = Fixture::new();
        let directory = fixture.directory("Lumen");
        let session = fixture
            .core
            .store
            .create(directory.to_string_lossy().into())
            .unwrap();
        for (role, text) in [
            (
                "user",
                "Implementa radius condiviso, menu contestuale e Ctrl+Invio",
            ),
            (
                "assistant",
                "La coda sarà FIFO e separata per conversazione",
            ),
            (
                "user",
                "Approvato. Mantieni Shift+F10 e sospendi la coda in caso di errore",
            ),
            ("user", "Ok procedi con le modifiche ora"),
        ] {
            fixture
                .core
                .store
                .save_message(
                    &session.id,
                    &Message {
                        id: crate::models::id(),
                        role: role.into(),
                        text: text.into(),
                        activity: vec![],
                    },
                )
                .unwrap();
        }
        let options = Fixture::options(AccessMode::Autopilot);
        let (context, history) = turns::build_prompt(&fixture.core, &session, &[], &options)
            .await
            .unwrap();
        let st = State {
            task: "Ok procedi con le modifiche ora".into(),
            context: history,
            ..Default::default()
        };
        let prompt = planning_prompt(&st, &context);
        let worker = subtask_prompt(
            &st,
            &Subtask {
                title: "Implement queue".into(),
                writes: true,
                ..Default::default()
            },
            "",
            false,
        );
        for decision in ["radius condiviso", "FIFO", "Shift+F10", "sospendi la coda"] {
            assert!(prompt.contains(decision), "planner lost {decision}");
            assert!(worker.contains(decision), "worker lost {decision}");
        }
        assert!(prompt.contains("Autopilot is enabled"));
        assert!(worker.contains("File edits are authorized for this execution"));
    }

    #[test]
    fn changed_roots_and_upgraded_permissions_invalidate_preparation() {
        use crate::test_support::Fixture;
        let fixture = Fixture::new();
        let old = fixture.directory("Rodent Heroes");
        let new = fixture.directory("Lumen");
        let mut session = fixture
            .core
            .store
            .create(old.to_string_lossy().into())
            .unwrap();
        let mut st = State {
            workspace: session.workspace.clone(),
            ..Default::default()
        };
        let options = Fixture::options(AccessMode::Autopilot);
        assert!(!needs_replan(&st, &session, &options));
        session.workspace = new.to_string_lossy().into();
        assert!(needs_replan(&st, &session, &options));
        st.workspace = session.workspace.clone();
        st.read_only = true;
        assert!(needs_replan(&st, &session, &options));
        assert!(!needs_replan(
            &st,
            &session,
            &Fixture::options(AccessMode::ReadOnly)
        ));
    }

    #[test]
    fn autopilot_and_explicit_approval_do_not_publish_another_approval_gate() {
        use crate::test_support::Fixture;
        for (access, explicit, expected) in [
            (AccessMode::Autopilot, false, true),
            (AccessMode::FileEdits, true, true),
            (AccessMode::FileEdits, false, false),
            (AccessMode::ReadOnly, false, false),
            (AccessMode::Ask, false, false),
        ] {
            let mut st = State {
                execution_authorized: explicit,
                ..Default::default()
            };
            assert_eq!(ready_plan(&mut st, &Fixture::options(access)), expected);
            assert_eq!(st.stage, if expected { "approved" } else { "plan" });
        }
        let st = State {
            read_only: true,
            ..Default::default()
        };
        let task = Subtask {
            writes: true,
            ..Default::default()
        };
        assert!(subtask_prompt(&st, &task, "", false).ends_with("This run is read-only: analyse and describe the changes precisely, but do not modify files."));
        let st = State::default();
        assert!(subtask_prompt(&st, &Subtask::default(), "", false)
            .contains("This subtask is read-only"));
    }

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
    fn ps_elapsed_times_parse() {
        assert_eq!(parse_etime("05:07"), Some(307));
        assert_eq!(parse_etime("01:00:00"), Some(3600));
        assert_eq!(parse_etime("2-00:00:01"), Some(172_801));
        assert_eq!(parse_etime("garbage"), None);
    }

    #[tokio::test]
    async fn orphans_are_checked_only_for_started_interrupted_work() {
        let mut st = State {
            subtasks: vec![
                Subtask {
                    id: "a".into(),
                    status: "done".into(),
                    pid: std::process::id(),
                    ..Default::default()
                },
                Subtask {
                    id: "b".into(),
                    status: "interrupted".into(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        // A finished task and one that never started a CLI need no check.
        assert!(stop_orphans(&st).await.is_ok());
        // A recorded PID that belongs to a process started much earlier is a reused PID.
        st.subtasks[1].pid = std::process::id();
        st.subtasks[1].started_at = now() + 3_600_000;
        assert!(stop_orphans(&st).await.is_ok());
    }

    #[tokio::test]
    async fn an_orphaned_cli_is_stopped_before_resuming() {
        let started = now();
        #[cfg(windows)]
        let mut child = tokio::process::Command::new("ping")
            .args(["-n", "60", "127.0.0.1"])
            .stdout(std::process::Stdio::null())
            .spawn()
            .unwrap();
        #[cfg(unix)]
        let mut child = tokio::process::Command::new("sleep")
            .arg("60")
            .process_group(0)
            .spawn()
            .unwrap();
        #[cfg(unix)]
        let child_pid = child.id().unwrap();
        #[cfg(unix)]
        assert_eq!(unsafe { libc::getpgid(child_pid as i32) }, child_pid as i32);
        let st = State {
            subtasks: vec![Subtask {
                id: "a".into(),
                status: "interrupted".into(),
                pid: {
                    #[cfg(unix)]
                    {
                        child_pid
                    }
                    #[cfg(windows)]
                    {
                        child.id().unwrap()
                    }
                },
                started_at: started,
                ..Default::default()
            }],
            ..Default::default()
        };
        assert!(stop_orphans(&st).await.is_ok());
        let exited = tokio::time::timeout(std::time::Duration::from_secs(5), child.wait()).await;
        assert!(exited.is_ok(), "the orphan should have been stopped");
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

// ---------------------------------------------------------------- lifecycle

/// At startup: an Orchestra step that was running when Lumen closed can never
/// finish by itself. Nothing is rerun here: a run interrupted while its subtasks
/// worked keeps its results and lanes and waits for an explicit Resume. Anything
/// later (integration, final check) or earlier (questions, plan) is closed.
pub fn recover(core: &Core) {
    let Ok(rows) = core.store.orchestra_messages() else {
        return;
    };
    for (session, mut m) in rows {
        let Some(mut st) = state_of(&m) else {
            continue;
        };
        if !["asking", "planning", "running"].contains(&st.stage.as_str()) {
            continue;
        }
        st.resumable = st.stage == "running" && st.steps.is_empty() && st.options.is_some();
        st.stage = "interrupted".into();
        st.queue = None;
        for t in st.subtasks.iter_mut() {
            match t.status.as_str() {
                "running" => {
                    t.status = "interrupted".into();
                    t.output = "Interrupted: Lumen was closed.".into();
                }
                "queued" => t.status = "pending".into(),
                _ => {}
            }
        }
        for x in st.steps.iter_mut().filter(|x| x.status == "running") {
            x.status = "failed".into();
        }
        if !st.resumable {
            let lanes = std::mem::take(&mut st.lanes);
            tauri::async_runtime::spawn(async move {
                for (top, path, branch) in lanes {
                    gitops::remove_lane(Path::new(&top), Path::new(&path), &branch).await;
                }
            });
        }
        if let (Some(a), Ok(text)) = (
            m.activity.iter_mut().find(|a| a.kind == "orchestra"),
            serde_json::to_string(&st),
        ) {
            a.text = text;
        }
        let _ = core.store.save_message(&session, &m);
    }
}

/// Start time (ms) of a live process, `None` when it no longer runs.
/// `Err` when the system cannot tell.
async fn process_start(pid: u32) -> Result<Option<i64>, String> {
    #[cfg(windows)]
    {
        let script = format!(
            "$p = Get-Process -Id {pid} -ErrorAction SilentlyContinue; if ($p) {{ [DateTimeOffset]::new($p.StartTime).ToUnixTimeMilliseconds() }} else {{ 'none' }}"
        );
        let mut c = crate::proc::command("powershell.exe");
        c.args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .kill_on_drop(true);
        let o = c.output().await.map_err(|e| e.to_string())?;
        let text = String::from_utf8_lossy(&o.stdout).trim().to_string();
        if !o.status.success() {
            return Err("cannot inspect processes".into());
        }
        if text == "none" {
            return Ok(None);
        }
        text.parse()
            .map(Some)
            .map_err(|_| "cannot inspect processes".into())
    }
    #[cfg(unix)]
    {
        let o = tokio::process::Command::new("ps")
            .args(["-o", "stat=,etime=", "-p", &pid.to_string()])
            .kill_on_drop(true)
            .output()
            .await
            .map_err(|e| e.to_string())?;
        let text = String::from_utf8_lossy(&o.stdout).trim().to_string();
        if text.is_empty() {
            return Ok(None);
        }
        let (status, elapsed) = text
            .split_once(char::is_whitespace)
            .ok_or("cannot inspect processes")?;
        if status.starts_with('Z') {
            return Ok(None);
        }
        let secs = parse_etime(elapsed.trim()).ok_or("cannot inspect processes")?;
        Ok(Some(now() - secs * 1000))
    }
}

/// `ps` elapsed time: `[[dd-]hh:]mm:ss`.
#[cfg_attr(windows, allow(dead_code))]
fn parse_etime(text: &str) -> Option<i64> {
    let (days, rest) = match text.split_once('-') {
        Some((d, r)) => (d.parse::<i64>().ok()?, r),
        None => (0, text),
    };
    let parts: Vec<i64> = rest
        .split(':')
        .map(|p| p.parse().ok())
        .collect::<Option<_>>()?;
    let (h, m, s) = match parts[..] {
        [m, s] => (0, m, s),
        [h, m, s] => (h, m, s),
        _ => return None,
    };
    Some(((days * 24 + h) * 60 + m) * 60 + s)
}

async fn kill_tree(pid: u32) {
    #[cfg(windows)]
    crate::proc::taskkill(pid).await;
    #[cfg(unix)]
    unsafe {
        // The PID has just been verified against the recorded start time.
        // Kill its process group when it leads one; otherwise kill the process
        // itself (the test and some wrappers may not create a new group).
        let group = libc::getpgid(pid as i32);
        if group == pid as i32 {
            libc::kill(-(pid as i32), libc::SIGKILL);
        } else {
            libc::kill(pid as i32, libc::SIGKILL);
        }
    }
}

/// Stop the CLIs an interrupted run left behind. A process is ours only if it
/// started within seconds of the recorded attempt; a reused PID is left alone.
/// Errors name the subtasks whose process could not be verified.
async fn stop_orphans(st: &State) -> Result<(), String> {
    let mut unknown = vec![];
    for t in st.subtasks.iter().filter(|t| t.status == "interrupted") {
        if t.pid == 0 {
            // Still waiting for a slot or a model list: no CLI was started.
            continue;
        }
        match process_start(t.pid).await {
            Ok(None) => {}
            Ok(Some(at)) if (at - t.started_at).abs() <= 15_000 => {
                kill_tree(t.pid).await;
                // A killed process can take a moment to disappear.
                let mut gone = false;
                for _ in 0..20 {
                    if matches!(process_start(t.pid).await, Ok(None)) {
                        gone = true;
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
                if !gone {
                    unknown.push(t.title.clone());
                }
            }
            Ok(Some(_)) => {}
            Err(_) => unknown.push(t.title.clone()),
        }
    }
    if unknown.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "Could not verify that these subtasks stopped: {}. Make sure no CLI from the previous session is still running, then choose Resume anyway.",
            unknown.join(", ")
        ))
    }
}

/// A new message replaces any question or plan still waiting for an answer.
pub fn supersede(app: &tauri::AppHandle, core: &Core, session: &str) {
    let Ok(messages) = core.store.messages(session) else {
        return;
    };
    for mut m in messages {
        if let Some(mut st) = state_of(&m) {
            if ["questions", "plan"].contains(&st.stage.as_str()) {
                st.stage = "superseded".into();
                let _ = save_state(app, core, session, &mut m, &st);
            }
        }
    }
}

/// Explicit approval in chat is equivalent to the card button, not a new task.
pub(crate) fn continuation(messages: &[Message], text: &str) -> Option<(String, State)> {
    let approval = regex::Regex::new(
        r"(?i)^\s*(?:(?:ok|sì|si|yes)[,\s]+)?(?:procedi(?:\s+(?:con\s+(?:le\s+)?modifiche|secondo\s+il\s+(?:piano|plan)(?:\s+(?:approvato|creato))?))?(?:\s+ora)?|vai[,\s]+fallo|agreed|approved(?:\s+the\s+plan)?|approvato(?:\s+il\s+piano)?|proceed(?:\s+with\s+the\s+(?:approved\s+)?plan)?|go ahead|ti ho dato tutti i permessi di scrittura ora,?\s*puoi procedere secondo il plan creato)[.!\s]*$",
    ).unwrap();
    if !approval.is_match(text) {
        return None;
    }
    let m = messages.iter().rev().find(|m| m.role == "assistant")?;
    let st = state_of(m)?;
    // Real unanswered questions cannot be silently answered by a generic 'proceed'.
    (st.stage == "plan" || (st.stage == "questions" && st.questions.is_empty()))
        .then(|| (m.id.clone(), st))
}

fn needs_replan(st: &State, session: &Session, options: &RunOptions) -> bool {
    (st.workspace.is_empty()
        || dunce::canonicalize(&st.workspace).ok() != dunce::canonicalize(&session.workspace).ok())
        || (st.read_only && options.access() != AccessMode::ReadOnly)
}

pub(crate) async fn continue_message(
    app: tauri::AppHandle,
    core: Arc<Core>,
    id: String,
    message: String,
    text: String,
    options: RunOptions,
    workspace_changed: bool,
) -> Result<Plan, String> {
    let (mut m, mut st) = load(&core, &id, &message)?;
    let a = account(&core, &st.orchestrator)?;
    let routed = Plan {
        provider: a.provider,
        account_id: st.orchestrator.clone(),
        model: Some(st.orchestrator_model.clone()),
        effort: st.effort.clone(),
        strategy: "orchestra".into(),
        reason: "Continuing the agreed Orchestra task".into(),
        partner_account: None,
    };
    let execute_existing = st.stage == "plan"
        && !workspace_changed
        && !needs_replan(&st, &core.store.session(&id)?, &options);
    st.read_only = options.access() == AccessMode::ReadOnly;
    st.execution_authorized = true;
    st.answers.push_str(&format!("\n{text}"));
    st.stage = if execute_existing {
        "approved"
    } else {
        "answered"
    }
    .into();
    save_state(&app, &core, &id, &mut m, &st)?;
    turns::spawn_turn(
        app,
        core,
        id,
        user_message(text),
        move |app, core, s, mut out, rx| async move {
            let result = async {
                if execute_existing {
                    let (_, history) = turns::build_prompt(&core, &s, &[&out.id], &options).await?;
                    st.context = history;
                    execute(&app, &core, &s, &mut out, st, &options, None, false, rx).await
                } else {
                    plan(&app, &core, &s, &mut out, st, &options, rx).await
                }
            }
            .await;
            (out, result)
        },
    )
    .await?;
    Ok(routed)
}

/// After Rewind: if the conversation now ends on an answered question or an
/// approved plan, open it again so it can be changed and approved anew.
pub fn reopen(core: &Core, session: &str) -> bool {
    let Ok(messages) = core.store.messages(session) else {
        return false;
    };
    let Some(mut m) = messages.into_iter().last() else {
        return false;
    };
    let Some(mut st) = state_of(&m) else {
        return false;
    };
    st.stage = match st.stage.as_str() {
        "answered" => "questions",
        "approved" => "plan",
        _ => return false,
    }
    .into();
    if let (Some(a), Ok(text)) = (
        m.activity.iter_mut().find(|a| a.kind == "orchestra"),
        serde_json::to_string(&st),
    ) {
        a.text = text;
    }
    core.store.save_message(session, &m).is_ok()
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
    crate::workspace::apply(&app, &core, &id, note.as_deref().unwrap_or(""))?;
    st.execution_authorized = st.questions.is_empty();
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
    let replan = needs_replan(&st, &core.store.session(&id)?, &options);
    st.read_only = options.access() == AccessMode::ReadOnly;
    let mut run = st.clone();
    run.execution_authorized = true;
    st.stage = "approved".into();
    save_state(&app, &core, &id, &mut m, &st)?;
    let user = user_message("Approved the plan.".into());
    run.steps.clear();
    turns::spawn_turn(
        app,
        core.inner().clone(),
        id,
        user,
        move |app, core, s, mut out, rx| async move {
            let r = if replan {
                plan(&app, &core, &s, &mut out, run, &options, rx).await
            } else {
                execute(&app, &core, &s, &mut out, run, &options, None, false, rx).await
            };
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

/// Resume a run interrupted by a restart: completed subtasks keep their results,
/// interrupted ones are verified stopped and rerun from a clean lane.
#[tauri::command]
pub async fn orchestra_resume(
    app: tauri::AppHandle,
    core: tauri::State<'_, Arc<Core>>,
    id: String,
    message: String,
    force: bool,
) -> Result<(), String> {
    let (mut m, mut st) = load(&core, &id, &message)?;
    if st.stage != "interrupted" || !st.resumable {
        return Err("This Orchestra run cannot be resumed.".into());
    }
    let options = st
        .options
        .clone()
        .ok_or("The run options were not saved.")?;
    if let Err(e) = stop_orphans(&st).await {
        if !force {
            st.notice = e.clone();
            save_state(&app, &core, &id, &mut m, &st)?;
            return Err(e);
        }
    }
    // Lanes of interrupted work may hold partial edits; finished work is kept as patches.
    for (top, path, branch) in std::mem::take(&mut st.lanes) {
        gitops::remove_lane(Path::new(&top), Path::new(&path), &branch).await;
    }
    for t in st.subtasks.iter_mut().filter(|t| t.status == "interrupted") {
        t.status = "pending".into();
    }
    st.resumable = false;
    st.notice.clear();
    let run = st.clone();
    let user = user_message("Resume the interrupted Orchestra run.".into());
    turns::spawn_turn(
        app.clone(),
        core.inner().clone(),
        id.clone(),
        user,
        move |app, core, s, mut out, rx| async move {
            let r = execute(&app, &core, &s, &mut out, run, &options, None, true, rx).await;
            (out, r)
        },
    )
    .await?;
    // The old card hands over to the resumed run in the new message.
    st.stage = "resumed".into();
    save_state(&app, &core, &id, &mut m, &st)
}

/// After a failed integration: keep the branches for manual merging, or discard them.
/// Admit `extra` more subtasks than the configured total for a running Orchestra message.
#[tauri::command]
pub fn orchestra_raise(
    core: tauri::State<'_, Arc<Core>>,
    message_id: String,
    extra: usize,
) -> Result<(), String> {
    if !(1..=99).contains(&extra) {
        return Err("Choose between 1 and 99 more subtasks".into());
    }
    let mut r = core.orchestra_raises.lock().map_err(|_| "Lock failed")?;
    *r.entry(message_id).or_insert(0) += extra;
    Ok(())
}

#[tauri::command]
pub async fn orchestra_lanes(
    app: tauri::AppHandle,
    core: tauri::State<'_, Arc<Core>>,
    id: String,
    message: String,
    keep: bool,
) -> Result<(), String> {
    let (mut m, mut st) = load(&core, &id, &message)?;
    let interrupted = st.stage == "interrupted" && st.resumable;
    if st.stage != "failed" && !interrupted {
        return Err("Nothing to keep or discard.".into());
    }
    if interrupted {
        // Discarding an interrupted run stops what it left running first.
        let _ = stop_orphans(&st).await;
        st.resumable = false;
        st.notice.clear();
    }
    if !keep {
        for (top, path, branch) in &st.lanes {
            gitops::remove_lane(Path::new(top), Path::new(path), branch).await;
        }
    } else {
        // The branches stay; only the temporary worktree folders go.
        for (top, path, _) in &st.lanes {
            let _ = crate::gitops::git_cmd(Path::new(top))
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
