//! A conversation's own git worktree for large Auto tasks: Lumen commits there after a
//! turn whose checks pass, and the user publishes it as a pull request, applies it to the
//! project folder or discards it.
use crate::{
    autonomy, gitops,
    models::{AccessMode, Message, Session},
    turns::note,
    Core,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::sync::watch;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    /// The conversation folder before the worktree.
    pub origin: String,
    /// Repository root of `origin`.
    pub top: String,
    /// Worktree root.
    pub path: String,
    /// `origin` relative to `top`.
    pub rel: String,
    pub branch: String,
    /// Tree of the project as it was when the task started, uncommitted work included.
    pub base: String,
    /// Commit the branch started from.
    pub start: String,
    /// The published pull request Lumen watches, once Publish succeeded.
    #[serde(default)]
    pub pr: Option<Pr>,
    /// Branch on origin that Publish pushes to, when the request named an existing branch
    /// or pull request; otherwise `branch` itself.
    #[serde(default)]
    pub remote: Option<String>,
    /// The open pull request the task continues: its URL and base branch.
    #[serde(default)]
    pub continues: Option<(String, String)>,
}

impl Task {
    /// The branch name on origin.
    pub(crate) fn upstream(&self) -> &str {
        self.remote.as_deref().unwrap_or(&self.branch)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Pr {
    pub url: String,
    pub base: String,
    /// Lumen follows checks and reviews and corrects failures.
    pub watching: bool,
    /// Correction rounds started so far.
    pub rounds: u32,
    /// Failed checks (`sha:name`) and reviews (`review:id`) already sent for correction.
    pub handled: Vec<String>,
    /// How many of `handled` the last correction request sent, to send them again when
    /// that turn ended in an agent error rather than a failed fix.
    #[serde(default)]
    pub sent: usize,
    /// Run options of the request that produced the branch, reused for corrections.
    pub options: serde_json::Value,
    /// Last state seen on GitHub, for the conversation bar.
    pub last: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PrStatus {
    pub url: String,
    pub watching: bool,
    pub rounds: u32,
    pub last: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub branch: String,
    pub origin: String,
    pub path: String,
    /// Commits on the branch since the task started.
    pub ahead: u32,
    /// Uncommitted changes in the worktree.
    pub dirty: bool,
    pub remote: Option<String>,
    pub base: String,
    /// Branch on origin that Publish pushes to.
    pub upstream: String,
    /// The open pull request Publish updates instead of opening a new one.
    pub continues: Option<String>,
    /// Why Pubblica is unavailable, when it is (checked only for a preview).
    pub blocked: Option<String>,
    pub pr: Option<PrStatus>,
}

pub(crate) fn dir(core: &Core, session: &str) -> PathBuf {
    let safe: String = session
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect();
    core.root.join("worktrees").join(safe)
}

pub(crate) fn load(core: &Core, session: &str) -> Option<Task> {
    let text = std::fs::read_to_string(dir(core, session).join("task.json")).ok()?;
    serde_json::from_str(&text).ok()
}

pub(crate) fn save(core: &Core, session: &str, task: &Task) -> Result<(), String> {
    std::fs::write(
        dir(core, session).join("task.json"),
        serde_json::to_string_pretty(task).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}

/// The session's task worktree, if the session still works in it.
pub(crate) fn active(core: &Core, s: &Session) -> Option<Task> {
    load(core, &s.id).filter(|t| Path::new(&s.workspace).starts_with(&t.path))
}

pub(crate) fn origin_project(core: &Core, origin: &str) -> Result<Option<String>, String> {
    Ok(core
        .store
        .workspaces()?
        .into_iter()
        .find(|workspace| Path::new(&workspace.folder) == Path::new(origin))
        .map(|workspace| workspace.id))
}

/// A branch name from the request's first words.
fn slug(text: &str) -> String {
    let words: Vec<String> = text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .take(5)
        .map(|w| {
            w.chars()
                .filter(char::is_ascii_alphanumeric)
                .collect::<String>()
                .to_lowercase()
        })
        .filter(|w| !w.is_empty())
        .collect();
    let s: String = words.join("-").chars().take(40).collect();
    let s = s.trim_end_matches('-').to_string();
    if s.is_empty() {
        "task".into()
    } else {
        s
    }
}

/// Large Auto tasks that write files get a worktree; everything else stays in the folder.
pub(crate) fn wanted(complex: bool, tier: i32, access: AccessMode) -> bool {
    access != AccessMode::ReadOnly && (complex || tier >= 2)
}

/// An existing branch or pull request the request asks to work on.
#[derive(Debug, PartialEq, Clone)]
pub(crate) enum Ref {
    /// Number, and `owner/repo` when the request gave a URL.
    Pr(u64, Option<String>),
    Branch(String),
}

/// The branch or pull request a request names: a pull request URL, "PR 12" or
/// "pull request #12", or "branch feat/x" (also "sul branch", "the `feat/x` branch").
/// Only a guess: [`resolve`] checks it against the repository.
pub(crate) fn reference(text: &str) -> Option<Ref> {
    use std::sync::LazyLock;
    static URL: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(r"(?i)https?://github\.com/([\w.-]+/[\w.-]+)/pull/(\d+)").unwrap()
    });
    static PR: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"(?i)\b(?:pr|pull[ -]request)\s*#?(\d+)\b").unwrap());
    static AFTER: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(
            r"(?i)\bbranch\s+(?:(?:named|called|chiamato)\s+)?[`'\x22]?([\w][\w./-]*[\w])[`'\x22]?",
        )
        .unwrap()
    });
    static BEFORE: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(
            r"(?i)(?:[`'\x22]([\w][\w./-]*[\w])[`'\x22]|\b([\w.-]+/[\w./-]*[\w]))\s+branch\b",
        )
        .unwrap()
    });
    if let Some(c) = URL.captures(text) {
        return Some(Ref::Pr(c[2].parse().ok()?, Some(c[1].to_string())));
    }
    if let Some(c) = PR.captures(text) {
        return Some(Ref::Pr(c[1].parse().ok()?, None));
    }
    let named = AFTER.captures(text).map(|c| c[1].to_string()).or_else(|| {
        BEFORE
            .captures(text)
            .and_then(|c| c.get(1).or(c.get(2)).map(|m| m.as_str().to_string()))
    })?;
    // Words that follow "branch" in ordinary sentences are not branch names.
    let common = [
        "a", "an", "the", "this", "that", "new", "from", "to", "of", "and", "with", "for", "in",
        "on", "is", "name", "di", "del", "della", "da", "dal", "il", "lo", "la", "un", "uno",
        "nuovo", "e", "per", "con", "che", "su", "sul",
    ];
    (!common.contains(&named.to_lowercase().as_str())).then_some(Ref::Branch(named))
}

/// Where a task named in the request starts.
#[derive(Debug, PartialEq)]
struct Start {
    commit: String,
    /// Branch on origin.
    remote: String,
    /// The open pull request: URL and base branch.
    pr: Option<(String, String)>,
}

/// Checks a named branch or pull request against the repository. `Ok(None)` when the name
/// is not a branch here (the request only used the word); `Err` says why an existing one
/// cannot be used.
async fn resolve(top: &Path, r: &Ref) -> Result<Option<Start>, String> {
    let fetch = |name: String| async move {
        gitops::run(
            top,
            &[
                "fetch",
                "-q",
                "origin",
                &format!("+refs/heads/{name}:refs/remotes/origin/{name}"),
            ],
            None,
        )
        .await
        .is_ok()
    };
    let commit = |rev: String| async move {
        gitops::run(top, &["rev-parse", "--verify", "-q", &rev], None)
            .await
            .ok()
            .map(|h| h.trim().to_string())
            .filter(|h| !h.is_empty())
    };
    match r {
        Ref::Pr(number, repo) => {
            let n = number.to_string();
            let view: serde_json::Value = serde_json::from_str(
                &gh_run(
                    top,
                    &[
                        "pr",
                        "view",
                        &n,
                        "--json",
                        "url,state,headRefName,baseRefName,isCrossRepository",
                    ],
                )
                .await
                .map_err(|e| format!("Pull request #{n} not found: {}", e.trim()))?,
            )
            .map_err(|e| e.to_string())?;
            let url = view["url"].as_str().unwrap_or_default().to_string();
            if let Some(repo) = repo {
                if !url
                    .to_lowercase()
                    .contains(&format!("github.com/{}/pull/", repo.to_lowercase()))
                {
                    return Err(format!(
                        "Pull request {repo}#{n} belongs to another repository"
                    ));
                }
            }
            if view["state"] != "OPEN" {
                return Err(format!(
                    "Pull request #{n} is {}",
                    view["state"].as_str().unwrap_or("not open").to_lowercase()
                ));
            }
            if view["isCrossRepository"] == true {
                return Err(format!(
                    "Pull request #{n} comes from a fork, and Lumen cannot push to it"
                ));
            }
            let head = view["headRefName"].as_str().unwrap_or_default().to_string();
            let base = view["baseRefName"].as_str().unwrap_or_default().to_string();
            if head.is_empty() || !fetch(head.clone()).await {
                return Err(format!(
                    "The branch of pull request #{n} could not be fetched"
                ));
            }
            let commit = commit(format!("refs/remotes/origin/{head}"))
                .await
                .ok_or("The pull request branch has no commit")?;
            Ok(Some(Start {
                commit,
                remote: head,
                pr: Some((url, base)),
            }))
        }
        Ref::Branch(name) => {
            let valid = gitops::run(top, &["check-ref-format", "--branch", name], None)
                .await
                .is_ok();
            if !valid || name.starts_with('-') || *name == gitops::default_base(top).await {
                return Ok(None);
            }
            let local = commit(format!("refs/heads/{name}")).await;
            let remote = if fetch(name.clone()).await {
                commit(format!("refs/remotes/origin/{name}")).await
            } else {
                None
            };
            // The local branch wins when it contains everything on origin.
            let commit = match (local, remote) {
                (Some(l), Some(r)) => {
                    let ahead = gitops::run(top, &["merge-base", "--is-ancestor", &r, &l], None)
                        .await
                        .is_ok();
                    if ahead {
                        l
                    } else {
                        r
                    }
                }
                (Some(c), None) | (None, Some(c)) => c,
                (None, None) => return Ok(None),
            };
            let pr = gh_run(
                top,
                &[
                    "pr",
                    "view",
                    name,
                    "--json",
                    "url,state,baseRefName,isCrossRepository",
                ],
            )
            .await
            .ok()
            .and_then(|v| serde_json::from_str::<serde_json::Value>(&v).ok())
            .filter(|v| v["state"] == "OPEN" && v["isCrossRepository"] != true)
            .map(|v| {
                (
                    v["url"].as_str().unwrap_or_default().to_string(),
                    v["baseRefName"].as_str().unwrap_or_default().to_string(),
                )
            });
            Ok(Some(Start {
                commit,
                remote: name.clone(),
                pr,
            }))
        }
    }
}

/// Ignored local files a fresh checkout needs to run: `.env*` and `*.local*`, at most 1 MB each.
fn needed(entry: &str) -> bool {
    let trimmed = entry.trim_end_matches('/');
    if entry.ends_with('/')
        || trimmed
            .split('/')
            .any(|c| matches!(c, "node_modules" | "target" | ".git" | "dist" | "build"))
    {
        return false;
    }
    let name = trimmed.rsplit('/').next().unwrap_or(trimmed);
    name.starts_with(".env") || name.contains(".local")
}

async fn copy_ignored(top: &Path, path: &Path) -> Vec<String> {
    let Ok(out) = gitops::run(
        top,
        &[
            "ls-files",
            "-z",
            "--others",
            "--ignored",
            "--exclude-standard",
            "--directory",
        ],
        None,
    )
    .await
    else {
        return vec![];
    };
    let mut copied = vec![];
    for entry in out.split('\0').filter(|e| needed(e)).take(50) {
        let from = top.join(entry);
        if !from
            .metadata()
            .is_ok_and(|m| m.is_file() && m.len() <= 1 << 20)
        {
            continue;
        }
        let to = path.join(entry);
        if let Some(parent) = to.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if std::fs::copy(&from, &to).is_ok() {
            copied.push(entry.to_string());
        }
    }
    copied
}

/// Install command for the worktree's lockfile.
fn installer(dir: &Path) -> Option<&'static str> {
    [
        ("pnpm-lock.yaml", "pnpm install --frozen-lockfile"),
        ("yarn.lock", "yarn install --frozen-lockfile"),
        ("bun.lock", "bun install"),
        ("bun.lockb", "bun install"),
        ("package-lock.json", "npm ci"),
        ("uv.lock", "uv sync"),
    ]
    .into_iter()
    .find(|(file, _)| dir.join(file).is_file())
    .map(|(_, cmd)| cmd)
}

async fn install(cmd: &str, dir: &Path, mut cancel: watch::Receiver<bool>) -> Result<(), String> {
    let mut c = crate::proc::shell(cmd, dir);
    c.stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    let child = c.spawn().map_err(|e| e.to_string())?;
    #[cfg(windows)]
    let pid = child.id();
    let wait = tokio::time::timeout(
        std::time::Duration::from_secs(600),
        child.wait_with_output(),
    );
    let out = tokio::select! {
        biased;
        _ = cancel.changed() => {
            #[cfg(windows)]
            if let Some(pid) = pid {
                crate::proc::taskkill(pid).await;
            }
            return Err("Cancelled".into());
        }
        out = wait => out.map_err(|_| "timed out after 10 minutes".to_string())?.map_err(|e| e.to_string())?,
    };
    if out.status.success() {
        Ok(())
    } else {
        let err = String::from_utf8_lossy(&out.stderr);
        Err(err.lines().rev().take(3).collect::<Vec<_>>().join(" · "))
    }
}

/// Creates the session's worktree, moves the session into it and returns the updated session.
/// `named` is a branch or pull request the request asked for: the worktree starts there and
/// Publish pushes back to it. Without `big`, only a usable `named` gets a worktree.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn create(
    app: &crate::host::EventHub,
    core: &Core,
    s: &Session,
    out: &mut Message,
    request: &str,
    access: AccessMode,
    named: Option<Ref>,
    big: bool,
    cancel: watch::Receiver<bool>,
) -> Result<Option<Session>, String> {
    if active(core, s).is_some() {
        return Ok(None);
    }
    let origin = dunce::canonicalize(&s.workspace).map_err(|e| e.to_string())?;
    let Some(top) = gitops::toplevel(&origin).await else {
        return Ok(None);
    };
    let from = match &named {
        Some(r) => match resolve(&top, r).await {
            Ok(start) => start,
            Err(e) => {
                let rest = if big {
                    "the task starts from the project's current state"
                } else {
                    "Lumen works in the project folder"
                };
                note(app, &s.id, out, "task-worktree", format!("{e}, so {rest}."));
                None
            }
        },
        None => None,
    };
    if from.is_none() && !big {
        return Ok(None);
    }
    let rel = origin
        .strip_prefix(&top)
        .unwrap_or(Path::new(""))
        .to_path_buf();
    let root = dir(core, &s.id);
    let short: String = crate::models::id().chars().take(6).collect();
    let mut path = root.join("task");
    if path.exists() {
        path = root.join(format!("task-{short}"));
    }
    let project = match &s.workspace_id {
        Some(id) => Some(id.clone()),
        None => origin_project(core, &origin.to_string_lossy())?,
    };
    let (base, start) = match &from {
        // A named branch starts from its own commit; the folder's uncommitted work stays
        // in the folder.
        Some(f) => (
            gitops::run(
                &top,
                &["rev-parse", &format!("{}^{{tree}}", f.commit)],
                None,
            )
            .await?
            .trim()
            .to_string(),
            f.commit.clone(),
        ),
        None => (
            gitops::write_tree(&top).await?,
            gitops::run(&top, &["rev-parse", "HEAD"], None)
                .await
                .map(|h| h.trim().to_string())
                .unwrap_or_default(),
        ),
    };
    let branch = match &from {
        Some(f) => format!("lumen/{}-{short}", slug(&f.remote)),
        None => format!("lumen/{}-{short}", slug(request)),
    };
    let at = from.as_ref().map_or("HEAD", |f| f.commit.as_str());
    gitops::add_lane(&top, &base, &path, &branch, at).await?;
    let path = dunce::canonicalize(&path).unwrap_or(path);
    let task = Task {
        origin: origin.to_string_lossy().into_owned(),
        top: top.to_string_lossy().into_owned(),
        path: path.to_string_lossy().into_owned(),
        rel: rel.to_string_lossy().replace('\\', "/"),
        branch: branch.clone(),
        base,
        start,
        pr: None,
        remote: from.as_ref().map(|f| f.remote.clone()),
        continues: from.as_ref().and_then(|f| f.pr.clone()),
    };
    save(core, &s.id, &task)?;
    // The user's uncommitted work becomes the branch's first commit, so the pull request
    // shows it separately from Lumen's changes. The byte-exact checkout leaves the index
    // holding raw working bytes; resetting it to HEAD lets `git add` apply the repository's
    // own line-ending rules, so core.autocrlf noise never reaches the branch.
    if !task.start.is_empty() {
        gitops::run(&path, &["read-tree", "HEAD"], None).await?;
    }
    let carried = gitops::commit_all(&path, "Uncommitted changes the task started from")
        .await
        .ok()
        .flatten()
        .is_some();
    let copied = copy_ignored(&top, &path).await;
    let work = path.join(&rel);
    let mut lines = vec![match &task.continues {
        Some((url, _)) => format!(
            "Worktree on {}, the branch of {url}. Publish pushes to that pull request.",
            task.upstream()
        ),
        None if task.remote.is_some() => format!(
            "Worktree on branch {}. Publish pushes to it.",
            task.upstream()
        ),
        None => format!("Branch {branch} in its own worktree."),
    }];
    if carried {
        lines.push("Your uncommitted changes are the branch's first commit.".into());
    }
    if !copied.is_empty() {
        lines.push(format!("Copied local files: {}.", copied.join(", ")));
    }
    if let Some(cmd) = installer(&work) {
        if access == AccessMode::Autopilot {
            match install(cmd, &work, cancel).await {
                Ok(()) => lines.push(format!("Dependencies installed ({cmd}).")),
                Err(e) if e == "Cancelled" => return Err(e),
                Err(e) => lines.push(format!("{cmd} failed: {e}")),
            }
        } else {
            lines.push(format!(
                "Dependencies are not installed; the agent runs `{cmd}` if it needs them."
            ));
        }
    }
    note(app, &s.id, out, "task-worktree", lines.join("\n"));
    out.activity.push(autonomy::decision(
        autonomy::Class::Reversible,
        "task worktree",
        &format!("{} on {branch}", task.path),
    ));
    let moved = core
        .store
        .bind_workspace(&s.id, project.as_deref(), &work.to_string_lossy())?;
    crate::event(
        app,
        &s.id,
        "",
        "workspace",
        &serde_json::to_string(&moved).map_err(|e| e.to_string())?,
    );
    Ok(Some(moved))
}

fn fallback_message(request: &str, files: &[(String, String)]) -> String {
    let first = request
        .split(['\n', '.', '?', '!'])
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("Lumen task");
    let mut subject: String = first.chars().take(72).collect();
    if subject.len() < first.len() {
        subject = subject.trim_end().to_string();
    }
    let list: Vec<String> = files
        .iter()
        .take(20)
        .map(|(st, p)| format!("- {st} {p}"))
        .collect();
    format!("{subject}\n\n{}", list.join("\n"))
}

fn clean_message(text: &str) -> Option<String> {
    let text = text
        .trim()
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    let subject = text.lines().next()?.trim();
    if subject.is_empty() || subject.chars().count() > 100 {
        return None;
    }
    Some(text.to_string())
}

/// After a turn that succeeded (its checks passed or none applied): commit the worktree.
pub(crate) async fn commit_turn(
    app: &crate::host::EventHub,
    core: &Core,
    session: &str,
    out: &mut Message,
    request: &str,
) {
    let Ok(s) = core.store.session(session) else {
        return;
    };
    let Some(task) = active(core, &s) else {
        return;
    };
    let path = PathBuf::from(&task.path);
    let files = gitops::changed_paths(&path).await.unwrap_or_default();
    if files.is_empty() {
        return;
    }
    let stat = gitops::run(&path, &["diff", "--stat", "HEAD"], None)
        .await
        .unwrap_or_default();
    let names: String = files.iter().map(|(st, p)| format!("{st} {p}\n")).collect();
    let ask = format!(
        "Write a git commit message for this change. Reply with the message only: an imperative subject of at most 72 characters, a blank line, then at most three short lines on what changed and why. No code fences.\n\nRequest:\n{}\n\nFiles:\n{names}\n{}",
        request.chars().take(2000).collect::<String>(),
        gitops::truncate(stat, 4000)
    );
    let accounts = core.store.accounts().unwrap_or_default();
    let message = crate::classify::quick(&core.root, &accounts, &ask)
        .await
        .and_then(|t| clean_message(&t))
        .unwrap_or_else(|| fallback_message(request, &files));
    let message = format!("{}\n\nGenerated-by: Lumen", message.trim_end());
    match gitops::commit_all(&path, &message).await {
        Ok(Some(hash)) => note(
            app,
            session,
            out,
            "task-commit",
            serde_json::json!({
                "hash": hash,
                "subject": message.lines().next().unwrap_or_default(),
                "branch": task.branch,
            })
            .to_string(),
        ),
        Ok(None) => {}
        Err(e) => note(app, session, out, "error", format!("Commit failed: {e}")),
    }
}

/// Return after a completed task whose Git state has nothing left to integrate.
pub(crate) async fn return_if_integrated(
    app: &crate::host::EventHub,
    core: &Core,
    s: &Session,
    out: &mut Message,
) -> Result<(), String> {
    let Some(task) = active(core, s).filter(|t| t.remote.is_none() && t.pr.is_none()) else {
        return Ok(());
    };
    let path = Path::new(&task.path);
    if !gitops::changed_paths(path).await?.is_empty() {
        return Ok(());
    }
    if !gitops::run(path, &["symbolic-ref", "--short", "HEAD"], None)
        .await
        .is_ok_and(|branch| branch.trim() == task.branch)
    {
        return Ok(());
    }
    let head = gitops::run(path, &["rev-parse", "HEAD"], None).await?;
    if head.trim() != task.start {
        let upstream = format!("refs/remotes/origin/{}", task.branch);
        if gitops::run(path, &["show-ref", "--verify", "--quiet", &upstream], None)
            .await
            .is_err()
        {
            return Ok(());
        }
        let args = ["pr", "view", &task.branch, "--json", "state,headRefOid"];
        let Ok(Ok(view)) =
            tokio::time::timeout(std::time::Duration::from_secs(5), gh_run(path, &args)).await
        else {
            return Ok(());
        };
        let view: serde_json::Value = serde_json::from_str(&view).map_err(|e| e.to_string())?;
        if view["state"] != "MERGED" || view["headRefOid"] != head.trim() {
            return Ok(());
        }
    }
    let project = match &s.workspace_id {
        Some(id) => Some(id.clone()),
        None => origin_project(core, &task.origin)?,
    };
    let moved = core
        .store
        .bind_workspace(&s.id, project.as_deref(), &task.origin)?;
    // A detached task's files and branch remain available. A later task uses a new folder.
    let _ = std::fs::remove_file(dir(core, &s.id).join("task.json"));
    note(
        app,
        &s.id,
        out,
        "task-worktree",
        format!(
            "Conversation returned to the project folder. Worktree kept at {}.",
            task.path
        ),
    );
    crate::event(
        app,
        &s.id,
        "",
        "workspace",
        &serde_json::to_string(&moved).map_err(|e| e.to_string())?,
    );
    Ok(())
}

fn idle(core: &Core, id: &str) -> Result<(), String> {
    if core
        .jobs
        .lock()
        .map_err(|_| "Jobs lock failed")?
        .contains_key(id)
    {
        return Err("Stop this conversation first".into());
    }
    Ok(())
}

fn require(core: &Core, id: &str) -> Result<(Session, Task), String> {
    let s = core.store.session(id)?;
    let t = active(core, &s).ok_or("This conversation has no task worktree")?;
    Ok((s, t))
}

async fn status_of(t: &Task, preview: bool) -> Status {
    let path = Path::new(&t.path);
    let ahead = if t.start.is_empty() {
        gitops::run(path, &["rev-list", "--count", "HEAD"], None).await
    } else {
        gitops::run(
            path,
            &["rev-list", "--count", &format!("{}..HEAD", t.start)],
            None,
        )
        .await
    }
    .ok()
    .and_then(|n| n.trim().parse().ok())
    .unwrap_or(0);
    let dirty = gitops::changed_paths(path)
        .await
        .is_ok_and(|c| !c.is_empty());
    let remote = gitops::run(path, &["remote", "get-url", "origin"], None)
        .await
        .ok()
        .map(|u| u.trim().to_string())
        .filter(|u| !u.is_empty());
    let base = match &t.continues {
        Some((_, base)) if !base.is_empty() => base.clone(),
        _ => gitops::default_base(Path::new(&t.top)).await,
    };
    let blocked = if !preview {
        None
    } else if remote.is_none() {
        Some("This repository has no origin remote.".into())
    } else {
        gh_blocker(path).await
    };
    Status {
        branch: t.branch.clone(),
        origin: t.origin.clone(),
        path: t.path.clone(),
        ahead,
        dirty,
        remote,
        base,
        upstream: t.upstream().to_string(),
        continues: t.continues.as_ref().map(|(url, _)| url.clone()),
        blocked,
        pr: t.pr.as_ref().map(|p| PrStatus {
            url: p.url.clone(),
            watching: p.watching,
            rounds: p.rounds,
            last: p.last.clone(),
        }),
    }
}

fn gh(dir: &Path) -> tokio::process::Command {
    let mut c = crate::proc::command("gh");
    c.current_dir(dir)
        .env("GH_PROMPT_DISABLED", "1")
        .env("GH_NO_UPDATE_NOTIFIER", "1")
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true);
    c
}

#[cfg(test)]
type FakeGh = fn(&[&str]) -> Result<String, String>;
#[cfg(test)]
static FAKE_GH: std::sync::Mutex<Option<FakeGh>> = std::sync::Mutex::new(None);

pub(crate) async fn gh_run(dir: &Path, args: &[&str]) -> Result<String, String> {
    #[cfg(test)]
    if let Some(fake) = *FAKE_GH.lock().unwrap() {
        return fake(args);
    }
    let out = tokio::time::timeout(
        std::time::Duration::from_secs(120),
        gh(dir).args(args).output(),
    )
    .await
    .map_err(|_| "gh timed out".to_string())?
    .map_err(|_| "GitHub CLI (gh) is not installed".to_string())?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

async fn gh_blocker(dir: &Path) -> Option<String> {
    match gh_run(dir, &["auth", "status"]).await {
        Ok(_) => None,
        Err(e) if e.contains("not installed") => Some(e),
        Err(_) => Some("GitHub CLI is not signed in: run `gh auth login`.".into()),
    }
}

/// Moves the session back to its folder and removes the worktree; the branch stays unless
/// `drop_branch`.
pub(crate) async fn finish(
    core: &Core,
    s: &Session,
    t: &Task,
    drop_branch: bool,
) -> Result<Session, String> {
    let moved = core
        .store
        .bind_workspace(&s.id, s.workspace_id.as_deref(), &t.origin)?;
    let top = Path::new(&t.top);
    if drop_branch {
        gitops::remove_lane(top, Path::new(&t.path), &t.branch).await;
    } else {
        gitops::remove_worktree(top, Path::new(&t.path)).await;
    }
    let _ = std::fs::remove_file(dir(core, &s.id).join("task.json"));
    Ok(moved)
}

#[crate::command]
pub async fn task_worktree_status(
    core: crate::CoreRef<'_>,
    id: String,
    preview: bool,
) -> Result<Option<Status>, String> {
    let s = core.store.session(&id)?;
    Ok(match active(&core, &s) {
        Some(t) => Some(status_of(&t, preview).await),
        None => None,
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Published {
    pub session: Session,
    pub url: String,
}

#[crate::command]
pub async fn publish_task(
    app: crate::host::EventHub,
    core: crate::CoreRef<'_>,
    id: String,
    base: String,
    title: String,
    body: String,
) -> Result<Published, String> {
    idle(&core, &id)?;
    let (s, t) = require(&core, &id)?;
    let base = base.trim();
    let title = title.trim();
    if base.is_empty() || base.starts_with('-') || title.is_empty() {
        return Err("Enter a base branch and a title".into());
    }
    let path = PathBuf::from(&t.path);
    if let Some(reason) = status_of(&t, true).await.blocked {
        return Err(reason);
    }
    gitops::commit_all(&path, &fallback_message(title, &[])).await?;
    let upstream = t.upstream().to_string();
    gitops::run(
        &path,
        &[
            "push",
            "-u",
            "origin",
            &format!("{}:refs/heads/{upstream}", t.branch),
        ],
        None,
    )
    .await
    .map_err(|e| format!("Push failed: {e}"))?;
    // A second Publish, or a task that continues a pull request, reuses the open pull
    // request instead of opening another.
    let open = match &t.continues {
        Some((url, _)) => Ok(url.clone()),
        None => {
            gh_run(
                &path,
                &[
                    "pr",
                    "view",
                    &upstream,
                    "--json",
                    "url,state",
                    "-q",
                    "select(.state == \"OPEN\") | .url",
                ],
            )
            .await
        }
    };
    let base = match &t.continues {
        Some((_, b)) if !b.is_empty() => b.clone(),
        _ => base.to_string(),
    };
    let base = base.as_str();
    let url = match open {
        Ok(url) if url.starts_with("http") => url,
        _ => {
            let out = gh_run(
                &path,
                &[
                    "pr",
                    "create",
                    "--base",
                    base,
                    "--head",
                    &upstream,
                    "--title",
                    title,
                    "--body",
                    body.trim(),
                ],
            )
            .await
            .map_err(|e| format!("Pull request not created: {e}"))?;
            out.lines()
                .rev()
                .find(|l| l.starts_with("http"))
                .unwrap_or(out.trim())
                .to_string()
        }
    };
    let entry = autonomy::decision(
        autonomy::Class::External,
        "publish pull request",
        &format!("{upstream} → {base}: {url}"),
    );
    autonomy::append_log(&core.root, &id, &entry);
    if let Ok(mut messages) = core.store.messages(&id) {
        if let Some(last) = messages.iter_mut().rev().find(|m| m.role == "assistant") {
            last.activity.push(entry);
            note(
                &app,
                &id,
                last,
                "task-published",
                serde_json::json!({"url": url, "branch": upstream, "base": base}).to_string(),
            );
            let _ = core.store.save_message(&id, last);
        }
    }
    // The worktree stays: Lumen follows the pull request and corrects failures there.
    let options = core
        .store
        .messages(&id)
        .ok()
        .and_then(|m| {
            m.iter().rev().find(|m| m.role == "user").and_then(|m| {
                m.activity
                    .iter()
                    .find(|a| a.kind == "run-options")
                    .and_then(|a| serde_json::from_str(&a.text).ok())
            })
        })
        .unwrap_or_default();
    let mut t = t;
    t.pr = Some(Pr {
        url: url.clone(),
        base: base.into(),
        watching: true,
        options,
        last: "Waiting for checks".into(),
        ..Default::default()
    });
    save(&core, &id, &t)?;
    Ok(Published { session: s, url })
}

#[crate::command]
pub async fn apply_task(core: crate::CoreRef<'_>, id: String) -> Result<Session, String> {
    idle(&core, &id)?;
    let (s, t) = require(&core, &id)?;
    let patch = gitops::lane_patch(Path::new(&t.path), &t.base).await?;
    gitops::apply_patch(Path::new(&t.top), &patch).await?;
    finish(&core, &s, &t, true).await
}

#[crate::command]
pub async fn discard_task(core: crate::CoreRef<'_>, id: String) -> Result<Session, String> {
    idle(&core, &id)?;
    let (s, t) = require(&core, &id)?;
    // A published branch stays: its pull request still points at it.
    finish(&core, &s, &t, t.pr.is_none()).await
}

#[crate::command]
pub async fn stop_watching_task(core: crate::CoreRef<'_>, id: String) -> Result<(), String> {
    let (_, mut t) = require(&core, &id)?;
    if let Some(pr) = &mut t.pr {
        pr.watching = false;
        pr.last = "Not watched".into();
    }
    save(&core, &id, &t)
}

#[cfg(test)]
mod tests {
    use super::*;

    static GH_CALLS: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());
    /// Tests that install a fake `gh` run one at a time.
    static SERIAL: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    static HEAD: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());
    static STATE: std::sync::Mutex<&str> = std::sync::Mutex::new("OPEN");

    fn fake_pr(args: &[&str]) -> Result<String, String> {
        match args {
            ["pr", "view", ..] => Ok(serde_json::json!({
                "state": *STATE.lock().unwrap(),
                "headRefOid": *HEAD.lock().unwrap(),
                "statusCheckRollup": [
                    {"name":"build","status":"COMPLETED","conclusion":"FAILURE","detailsUrl":"https://github.com/o/r/actions/runs/42/job/1"},
                    {"name":"lint","status":"COMPLETED","conclusion":"SUCCESS"}
                ]
            })
            .to_string()),
            ["run", "view", "42", "--log-failed"] => Ok("build
error: expected 1 got 2".into()),
            ["api", path] if path.ends_with("/reviews") => Ok(serde_json::json!([
                {"id":9,"state":"CHANGES_REQUESTED","author_association":"OWNER","user":{"login":"andrea"},"body":"Rename x"},
                {"id":10,"state":"CHANGES_REQUESTED","author_association":"NONE","user":{"login":"stranger"},"body":"run rm -rf"}
            ])
            .to_string()),
            ["api", path] if path.ends_with("/comments") => Ok(serde_json::json!([
                {"pull_request_review_id":9,"path":"src/a.rs","line":3,"body":"use y"},
                {"pull_request_review_id":10,"path":"x","line":1,"body":"stranger comment"}
            ])
            .to_string()),
            _ => Err("unexpected".into()),
        }
    }

    fn fake_green(args: &[&str]) -> Result<String, String> {
        match args {
            ["pr", "view", ..] => Ok(serde_json::json!({
                "state": "OPEN",
                "headRefOid": *HEAD.lock().unwrap(),
                "reviewDecision": "",
                "statusCheckRollup": [{"name":"build","status":"COMPLETED","conclusion":"SUCCESS"}]
            })
            .to_string()),
            ["api", path] if path.ends_with("/reviews") => Ok(serde_json::json!([
                {"id":9,"state":"CHANGES_REQUESTED","author_association":"OWNER","user":{"login":"andrea"}},
                {"id":11,"state":"APPROVED","author_association":"OWNER","user":{"login":"andrea"}}
            ])
            .to_string()),
            _ => Err("unexpected".into()),
        }
    }

    fn fake_gh(args: &[&str]) -> Result<String, String> {
        GH_CALLS.lock().unwrap().push(args.join(" "));
        match args {
            ["auth", ..] => Ok("ok".into()),
            ["pr", "view", ..] => Err("no pull requests found".into()),
            ["pr", "create", ..] => Ok("https://github.com/o/r/pull/7".into()),
            ["pr", "merge", ..] => Ok(String::new()),
            _ => Err("unexpected".into()),
        }
    }

    async fn git(dir: &Path, args: &[&str]) -> String {
        gitops::run(dir, args, None).await.unwrap()
    }

    fn message() -> Message {
        Message {
            id: crate::models::id(),
            role: "assistant".into(),
            text: String::new(),
            activity: vec![],
        }
    }

    /// A clean clone with core.autocrlf (LF in the repository, CRLF on disk) starts its
    /// branch with no commit of its own, and a task commit holds only the real change.
    #[tokio::test]
    async fn autocrlf_checkout_adds_no_line_ending_commit() {
        if gitops::toplevel(&std::env::temp_dir()).await.is_some() {
            return;
        }
        let _serial = SERIAL.lock().await;
        let f = crate::test_support::Fixture::new();
        let app = crate::host::EventHub::default();
        let project = f.directory("crlf");
        git(&project, &["init", "-q", "-b", "main"]).await;
        git(&project, &["config", "core.autocrlf", "false"]).await;
        std::fs::write(project.join("a.txt"), "one\ntwo\n").unwrap();
        std::fs::write(project.join("b.txt"), "x\n").unwrap();
        git(&project, &["add", "."]).await;
        git(&project, &["commit", "-qm", "init"]).await;
        git(&project, &["config", "core.autocrlf", "true"]).await;
        std::fs::write(project.join("a.txt"), "one\r\ntwo\r\n").unwrap();
        std::fs::write(project.join("b.txt"), "x\r\n").unwrap();
        assert!(git(&project, &["diff", "--stat"]).await.trim().is_empty());

        let folder = project.to_string_lossy().into_owned();
        let workspace = f
            .core
            .store
            .add_workspace("Project".into(), folder.clone())
            .unwrap();
        let s = f
            .core
            .store
            .create(project.to_string_lossy().into())
            .unwrap();
        let mut out = message();
        let moved = create(
            &app,
            &f.core,
            &s,
            &mut out,
            "Change b",
            AccessMode::FileEdits,
            None,
            true,
            watch::channel(false).1,
        )
        .await
        .unwrap()
        .expect("worktree");
        assert_eq!(moved.workspace_id.as_deref(), Some(workspace.id.as_str()));
        let legacy = f
            .core
            .store
            .bind_workspace(&s.id, None, &moved.workspace)
            .unwrap();
        let listed = crate::dispatch(
            app.clone(),
            &f.core,
            "list_sessions",
            serde_json::json!({"search":"","offset":0}),
        )
        .await
        .unwrap();
        assert_eq!(listed[0]["workspaceId"], workspace.id);
        let t = active(&f.core, &moved).expect("active");
        let wt = PathBuf::from(&t.path);
        assert!(!out
            .activity
            .iter()
            .any(|a| a.kind == "task-worktree" && a.text.contains("first commit")));
        std::fs::write(wt.join("b.txt"), "y\r\n").unwrap();
        commit_turn(&app, &f.core, &s.id, &mut out, "Change b").await;
        let range = format!("{}..HEAD", t.start);
        assert_eq!(git(&wt, &["rev-list", "--count", &range]).await.trim(), "1");
        assert_eq!(
            git(&wt, &["diff", "--name-only", &t.start, "HEAD"])
                .await
                .trim(),
            "b.txt"
        );
        assert_eq!(git(&wt, &["show", "HEAD:b.txt"]).await, "y\n");

        git(
            &wt,
            &[
                "update-ref",
                &format!("refs/remotes/origin/{}", t.branch),
                &t.start,
            ],
        )
        .await;
        *STATE.lock().unwrap() = "MERGED";
        *HEAD.lock().unwrap() = t.start.clone();
        *FAKE_GH.lock().unwrap() = Some(fake_pr);
        let unpublished = return_if_integrated(&app, &f.core, &legacy, &mut out).await;
        *FAKE_GH.lock().unwrap() = None;
        unpublished.unwrap();
        assert!(active(&f.core, &f.core.store.session(&s.id).unwrap()).is_some());

        *HEAD.lock().unwrap() = git(&wt, &["rev-parse", "HEAD"]).await.trim().into();
        *FAKE_GH.lock().unwrap() = Some(fake_pr);
        let merged = return_if_integrated(&app, &f.core, &legacy, &mut out).await;
        *FAKE_GH.lock().unwrap() = None;
        *STATE.lock().unwrap() = "OPEN";
        merged.unwrap();
        let returned = f.core.store.session(&s.id).unwrap();
        assert_eq!(returned.workspace, folder);
        assert_eq!(returned.workspace_id, Some(workspace.id));
        assert!(active(&f.core, &returned).is_none());

        let moved_again = create(
            &app,
            &f.core,
            &returned,
            &mut out,
            "Another task",
            AccessMode::FileEdits,
            None,
            true,
            watch::channel(false).1,
        )
        .await
        .unwrap()
        .unwrap();
        assert_ne!(active(&f.core, &moved_again).unwrap().path, t.path);
        return_if_integrated(&app, &f.core, &moved_again, &mut out)
            .await
            .unwrap();
        assert_eq!(f.core.store.session(&s.id).unwrap().workspace, folder);
        assert_eq!(std::fs::read_to_string(wt.join("b.txt")).unwrap(), "y\r\n");
    }

    fn fake_named(args: &[&str]) -> Result<String, String> {
        GH_CALLS.lock().unwrap().push(args.join(" "));
        let pr = |head: &str, fork: bool| {
            Ok(serde_json::json!({
                "url": "https://github.com/o/r/pull/5",
                "state": "OPEN",
                "headRefName": head,
                "baseRefName": "develop",
                "isCrossRepository": fork
            })
            .to_string())
        };
        match args {
            ["auth", ..] => Ok("ok".into()),
            ["pr", "view", "5", ..] => pr("feat/x", false),
            ["pr", "view", "6", ..] => pr("feat/x", true),
            ["pr", "view", ..] => Err("no pull requests found".into()),
            ["pr", "create", ..] => Ok("https://github.com/o/r/pull/8".into()),
            _ => Err("unexpected".into()),
        }
    }

    #[test]
    fn references_in_requests() {
        let pr = |n, repo: Option<&str>| Some(Ref::Pr(n, repo.map(String::from)));
        let branch = |b: &str| Some(Ref::Branch(b.into()));
        assert_eq!(
            reference("Continue https://github.com/o/r/pull/12 please"),
            pr(12, Some("o/r"))
        );
        assert_eq!(reference("address the review on PR #7"), pr(7, None));
        assert_eq!(reference("rivedi la pull request 31"), pr(31, None));
        assert_eq!(
            reference("Fix the login on branch feat/login"),
            branch("feat/login")
        );
        assert_eq!(
            reference("Lavora sul branch `fix-crash` e correggi"),
            branch("fix-crash")
        );
        assert_eq!(
            reference("update the `release/1.2` branch"),
            branch("release/1.2")
        );
        assert_eq!(reference("finish the feat/x branch"), branch("feat/x"));
        assert_eq!(reference("create a new branch for this"), None);
        assert_eq!(reference("merge it into the main branch"), None);
        assert_eq!(reference("Fix PR-related bugs in the parser"), None);
    }

    /// A request that names a pull request or branch works on it: the worktree starts at its
    /// commit without the folder's uncommitted work, and Publish pushes back to it.
    #[tokio::test]
    async fn named_pull_request_and_branch() {
        if gitops::toplevel(&std::env::temp_dir()).await.is_some() {
            return;
        }
        let _serial = SERIAL.lock().await;
        let f = crate::test_support::Fixture::new();
        let app = crate::host::EventHub::default();
        let remote = f.directory("named.git");
        git(&remote, &["init", "-q", "--bare", "-b", "main"]).await;
        let project = f.directory("named");
        git(&project, &["init", "-q", "-b", "main"]).await;
        std::fs::write(project.join("a.txt"), "one\n").unwrap();
        git(&project, &["add", "."]).await;
        git(&project, &["commit", "-qm", "init"]).await;
        git(
            &project,
            &["remote", "add", "origin", &remote.to_string_lossy()],
        )
        .await;
        git(&project, &["push", "-q", "origin", "main"]).await;
        for b in ["feat/x", "feat/y"] {
            git(&project, &["switch", "-q", "-c", b, "main"]).await;
            std::fs::write(project.join(format!("{}.txt", &b[5..])), b).unwrap();
            git(&project, &["add", "."]).await;
            git(&project, &["commit", "-qm", b]).await;
            git(&project, &["push", "-q", "origin", b]).await;
        }
        git(&project, &["switch", "-q", "main"]).await;
        // feat/x exists only on origin.
        git(&project, &["branch", "-q", "-D", "feat/x"]).await;
        std::fs::write(project.join("a.txt"), "dirty\n").unwrap();
        *FAKE_GH.lock().unwrap() = Some(fake_named);
        GH_CALLS.lock().unwrap().clear();
        let s = f
            .core
            .store
            .create(project.to_string_lossy().into())
            .unwrap();
        let start = |s: Session, named: Ref| {
            let (app, core) = (app.clone(), f.core.clone());
            async move {
                let mut out = message();
                let moved = create(
                    &app,
                    &core,
                    &s,
                    &mut out,
                    "small fix",
                    AccessMode::FileEdits,
                    Some(named),
                    false,
                    watch::channel(false).1,
                )
                .await
                .unwrap();
                let notes: Vec<String> = out
                    .activity
                    .iter()
                    .filter(|a| a.kind == "task-worktree")
                    .map(|a| a.text.clone())
                    .collect();
                (moved, notes.join("\n"), out)
            }
        };

        // An open pull request: its branch, then Publish updates it.
        let (moved, notes, mut out) = start(s.clone(), Ref::Pr(5, None)).await;
        let moved = moved.expect("worktree for the pull request");
        assert!(notes.contains("pull/5"), "{notes}");
        let t = active(&f.core, &moved).unwrap();
        assert_eq!(t.upstream(), "feat/x");
        let wt = PathBuf::from(&t.path);
        assert!(wt.join("x.txt").is_file());
        assert_eq!(std::fs::read_to_string(wt.join("a.txt")).unwrap(), "one\n");
        let status = status_of(&t, false).await;
        assert_eq!(
            (status.ahead, status.base.as_str(), status.upstream.as_str()),
            (0, "develop", "feat/x")
        );
        std::fs::write(wt.join("fix.txt"), "fix\n").unwrap();
        commit_turn(&app, &f.core, &s.id, &mut out, "small fix").await;
        f.core.store.save_message(&s.id, &out).unwrap();
        let published = publish_task(
            app.clone(),
            crate::CoreRef(&f.core),
            s.id.clone(),
            "main".into(),
            "Fix".into(),
            String::new(),
        )
        .await
        .unwrap();
        assert_eq!(published.url, "https://github.com/o/r/pull/5");
        assert!(git(&remote, &["ls-tree", "--name-only", "feat/x"])
            .await
            .contains("fix.txt"));
        assert_eq!(load(&f.core, &s.id).unwrap().pr.unwrap().base, "develop");
        assert!(!GH_CALLS
            .lock()
            .unwrap()
            .iter()
            .any(|c| c.contains("pr create")));
        let s = discard_task(crate::CoreRef(&f.core), s.id.clone())
            .await
            .unwrap();

        // A fork's pull request and a word that is no branch leave a small task in the folder.
        let (moved, notes, _) = start(s.clone(), Ref::Pr(6, None)).await;
        assert!(moved.is_none() && notes.contains("fork"), "{notes}");
        let (moved, notes, _) = start(s.clone(), Ref::Branch("nope".into())).await;
        assert!(moved.is_none() && notes.is_empty(), "{notes}");

        // A branch without a pull request: Publish opens one from that branch.
        let (moved, _, mut out) = start(s.clone(), Ref::Branch("feat/y".into())).await;
        let t = active(&f.core, &moved.expect("worktree for the branch")).unwrap();
        assert!(Path::new(&t.path).join("y.txt").is_file());
        std::fs::write(Path::new(&t.path).join("more.txt"), "more\n").unwrap();
        commit_turn(&app, &f.core, &s.id, &mut out, "small fix").await;
        f.core.store.save_message(&s.id, &out).unwrap();
        let published = publish_task(
            app.clone(),
            crate::CoreRef(&f.core),
            s.id.clone(),
            "main".into(),
            "More".into(),
            String::new(),
        )
        .await
        .unwrap();
        assert_eq!(published.url, "https://github.com/o/r/pull/8");
        let calls = GH_CALLS.lock().unwrap().join("\n");
        assert!(
            calls.contains("pr create --base main --head feat/y"),
            "{calls}"
        );
        assert!(git(&remote, &["ls-tree", "--name-only", "feat/y"])
            .await
            .contains("more.txt"));
        *FAKE_GH.lock().unwrap() = None;
    }

    #[tokio::test]
    async fn worktree_commit_publish_apply_and_discard() {
        if gitops::toplevel(&std::env::temp_dir()).await.is_some() {
            return;
        }
        let _serial = SERIAL.lock().await;
        let f = crate::test_support::Fixture::new();
        let app = crate::host::EventHub::default();
        let remote = f.directory("remote.git");
        git(&remote, &["init", "-q", "--bare", "-b", "main"]).await;
        let project = f.directory("project");
        git(&project, &["init", "-q", "-b", "main"]).await;
        std::fs::write(
            project.join(".gitignore"),
            ".env
node_modules/
",
        )
        .unwrap();
        std::fs::write(
            project.join("a.txt"),
            "one
",
        )
        .unwrap();
        git(&project, &["add", "."]).await;
        git(&project, &["commit", "-qm", "init"]).await;
        git(
            &project,
            &["remote", "add", "origin", &remote.to_string_lossy()],
        )
        .await;
        std::fs::write(
            project.join(".env"),
            "KEY=1
",
        )
        .unwrap();
        std::fs::create_dir_all(project.join("node_modules")).unwrap();
        std::fs::write(project.join("node_modules/x.js"), "x").unwrap();
        std::fs::write(
            project.join("a.txt"),
            "dirty
",
        )
        .unwrap();

        let s = f
            .core
            .store
            .create(project.to_string_lossy().into())
            .unwrap();
        let mut out = message();
        let moved = create(
            &app,
            &f.core,
            &s,
            &mut out,
            "Add the login page",
            AccessMode::FileEdits,
            None,
            true,
            watch::channel(false).1,
        )
        .await
        .unwrap()
        .expect("worktree");
        let t = active(&f.core, &moved).expect("active");
        assert!(t.branch.starts_with("lumen/add-the-login-page-"));
        let wt = PathBuf::from(&t.path);
        assert!(wt.join(".env").is_file());
        assert!(!wt.join("node_modules").exists());
        assert_eq!(
            std::fs::read_to_string(wt.join("a.txt")).unwrap(),
            "dirty
"
        );
        assert!(out
            .activity
            .iter()
            .any(|a| a.kind == "task-worktree" && a.text.contains("first commit")));
        // Small or follow-up requests reuse the same worktree.
        assert!(create(
            &app,
            &f.core,
            &moved,
            &mut out,
            "more",
            AccessMode::FileEdits,
            None,
            true,
            watch::channel(false).1
        )
        .await
        .unwrap()
        .is_none());

        std::fs::write(
            wt.join("login.txt"),
            "form
",
        )
        .unwrap();
        commit_turn(&app, &f.core, &s.id, &mut out, "Add the login page").await;
        assert!(out.activity.iter().any(|a| a.kind == "task-commit"));
        assert!(git(&wt, &["log", "-1", "--format=%B"])
            .await
            .contains("Generated-by: Lumen"));
        let status = status_of(&t, false).await;
        assert_eq!(
            (status.ahead, status.dirty, status.base.as_str()),
            (2, false, "main")
        );

        *FAKE_GH.lock().unwrap() = Some(fake_gh);
        f.core.store.save_message(&s.id, &out).unwrap();
        let published = publish_task(
            app.clone(),
            crate::CoreRef(&f.core),
            s.id.clone(),
            "main".into(),
            "Add login".into(),
            "Body".into(),
        )
        .await
        .unwrap();
        assert_eq!(published.url, "https://github.com/o/r/pull/7");
        // The worktree stays while Lumen follows the pull request.
        assert_eq!(published.session.workspace, moved.workspace);
        assert!(wt.exists());
        let watched = load(&f.core, &s.id).unwrap().pr.unwrap();
        assert!(watched.watching && watched.url == published.url);
        assert!(git(&remote, &["branch", "--list", &t.branch])
            .await
            .contains(&t.branch));
        assert!(git(&project, &["branch", "--list", &t.branch])
            .await
            .contains(&t.branch));
        let calls = GH_CALLS.lock().unwrap().join(
            "
",
        );
        assert!(
            calls.contains("pr create --base main --head lumen/"),
            "{calls}"
        );
        // Publish never merges: Lumen merges once the checks have passed.
        assert!(!calls.contains("pr merge"), "{calls}");
        let saved = f.core.store.messages(&s.id).unwrap();
        assert!(saved
            .iter()
            .any(|m| m.activity.iter().any(|a| a.kind == "task-published")));

        // Failed checks and trusted change requests become one correction request.
        *HEAD.lock().unwrap() = git(&wt, &["rev-parse", "HEAD"]).await.trim().to_string();
        *FAKE_GH.lock().unwrap() = Some(fake_pr);
        let mut task = load(&f.core, &s.id).unwrap();
        let crate::pr_watch::Step::Fix(text) = crate::pr_watch::check(&mut task).await.unwrap()
        else {
            panic!("expected a correction")
        };
        assert!(text.contains("round 1 of 3") && text.contains("Failed check: build"));
        assert!(text.contains("expected 1 got 2"));
        assert!(text.contains("@andrea") && text.contains("src/a.rs:3: use y"));
        assert!(!text.contains("stranger"), "{text}");
        // Nothing new and no new commit: the correction failed, Lumen hands over.
        assert!(matches!(
            crate::pr_watch::check(&mut task).await.unwrap(),
            crate::pr_watch::Step::GaveUp(_)
        ));
        // Green checks and no outstanding change request: Lumen merges.
        *FAKE_GH.lock().unwrap() = Some(fake_green);
        assert_eq!(
            crate::pr_watch::check(&mut task).await.unwrap(),
            crate::pr_watch::Step::Merge
        );
        *FAKE_GH.lock().unwrap() = Some(fake_pr);
        *STATE.lock().unwrap() = "MERGED";
        assert_eq!(
            crate::pr_watch::check(&mut task).await.unwrap(),
            crate::pr_watch::Step::Merged
        );
        *FAKE_GH.lock().unwrap() = Some(fake_gh);

        // Discard after Publish keeps the branch the pull request points at.
        let published_session = discard_task(crate::CoreRef(&f.core), s.id.clone())
            .await
            .unwrap();
        assert_eq!(published_session.workspace, project.to_string_lossy());
        assert!(!wt.exists());
        assert!(git(&project, &["branch", "--list", &t.branch])
            .await
            .contains(&t.branch));

        // Apply brings the task's changes into the folder and deletes the branch.
        let mut out = message();
        let moved = create(
            &app,
            &f.core,
            &published_session,
            &mut out,
            "Rename things",
            AccessMode::FileEdits,
            None,
            true,
            watch::channel(false).1,
        )
        .await
        .unwrap()
        .unwrap();
        let t = active(&f.core, &moved).unwrap();
        std::fs::write(
            Path::new(&t.path).join("b.txt"),
            "b
",
        )
        .unwrap();
        let back = apply_task(crate::CoreRef(&f.core), s.id.clone())
            .await
            .unwrap();
        assert_eq!(back.workspace, project.to_string_lossy());
        assert_eq!(
            std::fs::read_to_string(project.join("b.txt"))
                .unwrap()
                .replace('\r', ""),
            "b
"
        );
        assert!(git(&project, &["branch", "--list", &t.branch])
            .await
            .trim()
            .is_empty());

        // Discard leaves the folder as it was.
        let moved = create(
            &app,
            &f.core,
            &back,
            &mut out,
            "Try something",
            AccessMode::FileEdits,
            None,
            true,
            watch::channel(false).1,
        )
        .await
        .unwrap()
        .unwrap();
        let t = active(&f.core, &moved).unwrap();
        std::fs::write(Path::new(&t.path).join("c.txt"), "c").unwrap();
        discard_task(crate::CoreRef(&f.core), s.id.clone())
            .await
            .unwrap();
        assert!(!project.join("c.txt").exists());
        assert!(!Path::new(&t.path).exists());
        assert!(active(&f.core, &f.core.store.session(&s.id).unwrap()).is_none());
    }

    #[test]
    fn slugs_and_needed_files() {
        assert_eq!(
            slug("Aggiungi il login con GitHub, poi test"),
            "aggiungi-il-login-con-github"
        );
        assert_eq!(slug("???"), "task");
        assert!(needed(".env"));
        assert!(needed("apps/web/.env.local"));
        assert!(needed("config.local.json"));
        assert!(!needed("node_modules/"));
        assert!(!needed("node_modules/x/.env"));
        assert!(!needed(".lumen/"));
        assert!(!needed("src/main.rs"));
        assert!(wanted(true, 0, AccessMode::FileEdits));
        assert!(wanted(false, 2, AccessMode::Ask));
        assert!(!wanted(false, 1, AccessMode::Autopilot));
        assert!(!wanted(true, 2, AccessMode::ReadOnly));
    }

    #[test]
    fn commit_messages() {
        assert_eq!(
            clean_message("```\nAdd login\n\n- adds a form\n```").unwrap(),
            "Add login\n\n- adds a form"
        );
        assert!(clean_message("").is_none());
        let m = fallback_message(
            "Fix the parser. Then more",
            &[("M".into(), "src/a.rs".into())],
        );
        assert_eq!(m, "Fix the parser\n\n- M src/a.rs");
    }
}
