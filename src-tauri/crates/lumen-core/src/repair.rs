//! Repair Lumen: one action that checks what can drift on a desktop and fixes what is safe to
//! fix by itself. The database is checked and backed up, conversations left active by a crash
//! are closed as interrupted (never replayed), and each agent's app is checked. Task folders left
//! behind are only listed: removing one needs the user's confirmation, and its branch stays.
//! The service itself is restarted by the desktop shell, which can do it when this cannot answer.
use crate::*;

/// Backups Lumen keeps, newest first.
const BACKUPS: usize = 5;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub database: Database,
    /// Conversations closed as interrupted.
    pub recovered: usize,
    pub accounts: Vec<AgentCheck>,
    pub tasks: Vec<Leftover>,
    /// Task folders moved back from an interrupted in-use check.
    pub restored: usize,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Database {
    pub ok: bool,
    pub detail: Option<String>,
    pub backup: Option<String>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCheck {
    pub id: String,
    pub name: String,
    pub provider: Provider,
    pub installed: bool,
    pub version: Option<String>,
    pub signed_in: Option<bool>,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Leftover {
    pub path: String,
    pub branch: Option<String>,
    /// Uncommitted files that removing the folder would lose (the branch keeps commits).
    pub changes: Option<usize>,
    /// Whether its conversation still exists.
    pub conversation: bool,
}

#[crate::command]
pub async fn repair(
    app: crate::host::EventHub,
    core: crate::CoreRef<'_>,
) -> Result<Report, String> {
    let database = database(core.inner().clone()).await;
    let recovered = recover(&app, &core)?;
    let restored = restore_probes(&core);
    let mut accounts = vec![];
    for a in core.store.accounts()? {
        if a.provider.is_api() || a.provider == Provider::Ollama {
            continue;
        }
        core.logins.forget(&a.id);
        let cli = crate::cli_info(&a).await;
        // None: the check itself failed (timeout, spawn error); that is not "signed out".
        let signed_in = if cli.installed {
            crate::signed_in(&core, &a).await.ok()
        } else {
            Some(false)
        };
        accounts.push(AgentCheck {
            id: a.id,
            name: a.name,
            provider: a.provider,
            installed: cli.installed,
            version: cli.version,
            signed_in,
        });
    }
    let tasks = leftovers(&core).await;
    Ok(Report {
        database,
        recovered,
        accounts,
        tasks,
        restored,
    })
}

/// Integrity check, then a backup next to the database; the oldest backups are pruned. SQLite
/// reads the whole file for both, so they run off the async workers.
async fn database(core: Arc<Core>) -> Database {
    tokio::task::spawn_blocking(move || {
        let check = core.store.integrity();
        let ok = check.as_ref().is_ok_and(|r| r == "ok");
        let mut detail = match &check {
            Ok(r) if r == "ok" => None,
            Ok(r) => Some(r.chars().take(500).collect()),
            Err(e) => Some(e.clone()),
        };
        // A damaged database is not copied over the good backups.
        let backup = if ok {
            match backup(&core, &core.root.join("backups"), BACKUPS) {
                Ok(path) => Some(path),
                Err(e) => {
                    detail = Some(e);
                    None
                }
            }
        } else {
            None
        };
        Database { ok, detail, backup }
    })
    .await
    .unwrap_or_else(|e| Database {
        ok: false,
        detail: Some(e.to_string()),
        backup: None,
    })
}

/// A backup in `dir`, keeping the newest `keep` of Lumen's own backups there. Other files
/// (a database kept by Go back, for example) are never pruned.
pub(crate) fn backup(core: &Core, dir: &Path, keep: usize) -> Result<String, String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("lumen-{}.db", now()));
    core.store.backup(&path)?;
    let stamp = |p: &Path| {
        p.file_name().and_then(|n| n.to_str()).and_then(|n| {
            n.strip_prefix("lumen-")?
                .strip_suffix(".db")?
                .parse::<i64>()
                .ok()
        })
    };
    let mut old: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter_map(|p| stamp(&p).map(|t| (t, p)))
        .collect();
    old.sort();
    while old.len() > keep {
        let _ = std::fs::remove_file(old.remove(0).1);
    }
    Ok(path.to_string_lossy().into_owned())
}

/// Before an update: a backup and the schema it uses, so going back can restore it if the older
/// version cannot open a newer schema. The one taken before an update has its own folder, so
/// Repair's backups never prune it.
#[crate::command]
pub async fn backup_database(
    core: crate::CoreRef<'_>,
    before_update: bool,
) -> Result<serde_json::Value, String> {
    let core = core.inner().clone();
    let path = tokio::task::spawn_blocking(move || {
        let backups = core.root.join("backups");
        if before_update {
            // Pruned by the shell at its next start, once no update record needs older ones.
            backup(&core, &backups.join("update"), usize::MAX)
        } else {
            backup(&core, &backups, BACKUPS)
        }
    })
    .await
    .map_err(|e| e.to_string())??;
    Ok(serde_json::json!({"path": path, "schema": crate::migrations::CURRENT}))
}

/// A conversation that reads as active with no turn behind it is closed as interrupted, with the
/// partial work kept, exactly as Stop would.
fn recover(app: &crate::host::EventHub, core: &Core) -> Result<usize, String> {
    let jobs = core.jobs.lock().map_err(|_| "Jobs lock failed")?;
    let mut n = 0;
    for id in core.store.active_sessions()? {
        if !jobs.contains_key(&id) {
            core.store.status(&id, "interrupted")?;
            event(app, &id, "", "done", "interrupted");
            n += 1;
        }
    }
    Ok(n)
}

/// A directory, not a link or junction to one somewhere else.
fn real_dir(e: &std::fs::DirEntry) -> bool {
    e.file_type().is_ok_and(|t| t.is_dir() && !t.is_symlink())
        && std::fs::symlink_metadata(e.path()).is_ok_and(|m| !is_reparse(&m))
}

#[cfg(windows)]
fn is_reparse(m: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    m.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse(_: &std::fs::Metadata) -> bool {
    false
}

fn worktrees(core: &Core) -> Vec<PathBuf> {
    std::fs::read_dir(core.root.join("worktrees"))
        .map(|d| {
            d.filter_map(Result::ok)
                .filter(real_dir)
                .map(|e| e.path())
                .collect()
        })
        .unwrap_or_default()
}

/// The in-use check renames a task folder and back; if Lumen stopped in between, move it back.
fn restore_probes(core: &Core) -> usize {
    let mut n = 0;
    for session in worktrees(core) {
        let Ok(entries) = std::fs::read_dir(&session) else {
            continue;
        };
        for probe in entries.filter_map(Result::ok).map(|e| e.path()) {
            let Some(name) = probe.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if let Some(original) = name.strip_suffix(".lumen-probe") {
                let target = session.join(original);
                if !target.exists() && std::fs::rename(&probe, &target).is_ok() {
                    n += 1;
                }
            }
        }
    }
    n
}

/// Task folders no conversation works in: their conversation was deleted, or an earlier task's
/// folder could not be removed at the time.
async fn leftovers(core: &Core) -> Vec<Leftover> {
    let mut found = vec![];
    for session in worktrees(core) {
        let id = session
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string();
        let conversation = core.store.session(&id).is_ok();
        let task = crate::task_worktree::load(core, &id);
        let Ok(entries) = std::fs::read_dir(&session) else {
            continue;
        };
        for folder in entries
            .filter_map(Result::ok)
            .filter(real_dir)
            .map(|e| e.path())
        {
            // The same folder however its path was written (case, `\?\` prefix).
            let same = |p: &str| dunce::canonicalize(p).ok() == dunce::canonicalize(&folder).ok();
            let current = task.as_ref().filter(|t| same(&t.path));
            if conversation && current.is_some() {
                continue;
            }
            // Unknown (git failed, broken worktree link) is not "nothing to lose": the
            // confirmation says so.
            let changes = gitops::changed_paths(&folder).await.ok().map(|c| c.len());
            found.push(Leftover {
                path: folder.to_string_lossy().into_owned(),
                branch: current.map(|t| t.branch.clone()),
                changes,
                conversation,
            });
        }
    }
    found
}

/// Removes a task folder Repair listed, after the user confirmed. Its branch stays in the
/// repository, so committed work can still be found.
#[crate::command]
pub async fn repair_remove_task(core: crate::CoreRef<'_>, path: String) -> Result<(), String> {
    let wanted = Path::new(&path);
    let leftover = leftovers(&core)
        .await
        .into_iter()
        .find(|l| Path::new(&l.path) == wanted)
        .ok_or("This folder is no longer a leftover task folder")?;
    let folder = PathBuf::from(&leftover.path);
    crate::task_worktree::free(&folder).await?;
    match gitops::run(&folder, &["rev-parse", "--git-common-dir"], None).await {
        // The repository the worktree belongs to: git forgets the worktree there too.
        Ok(common) => {
            let common = folder.join(common.trim());
            let top = common.parent().map(Path::to_path_buf).unwrap_or_default();
            gitops::remove_worktree(&top, &folder).await;
        }
        Err(_) => {
            let _ = std::fs::remove_dir_all(&folder);
        }
    }
    if folder.exists() {
        return Err("The task folder is in use and was not removed. Close the programs using it (editors, terminals, dev servers) and try again.".into());
    }
    // A deleted conversation's folder holds nothing else worth keeping once its tasks are gone.
    if let Some(session) = folder.parent() {
        let empty = std::fs::read_dir(session)
            .is_ok_and(|mut d| d.all(|e| e.is_ok_and(|e| e.file_name() == "task.json")));
        if !leftover.conversation && empty {
            let _ = std::fs::remove_dir_all(session);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn repairs_the_database_interrupted_turns_and_leftover_probes() {
        let f = crate::test_support::Fixture::new();
        let app = crate::host::EventHub::default();
        let s = f
            .core
            .store
            .create(f.directory("project").to_string_lossy().into())
            .unwrap();
        f.core.store.status(&s.id, "running").unwrap();
        // A turn that really runs has its job: Repair leaves it alone.
        let live = f
            .core
            .store
            .create(f.directory("project").to_string_lossy().into())
            .unwrap();
        f.core.store.status(&live.id, "running").unwrap();
        let (job, _rx) = tokio::sync::watch::channel(false);
        f.core.jobs.lock().unwrap().insert(live.id.clone(), job);
        let session = f.root.join("worktrees").join("gone-conversation");
        std::fs::create_dir_all(session.join("task.lumen-probe")).unwrap();

        let report = repair(app, crate::CoreRef(&f.core)).await.unwrap();
        // An in-memory database checks fine; its backup is a real file.
        assert!(report.database.ok, "{:?}", report.database.detail);
        assert!(Path::new(report.database.backup.as_deref().unwrap()).is_file());
        assert_eq!(report.recovered, 1);
        assert_eq!(f.core.store.session(&s.id).unwrap().status, "interrupted");
        assert_eq!(f.core.store.session(&live.id).unwrap().status, "running");
        assert_eq!(report.restored, 1);
        assert!(session.join("task").is_dir());
        // The restored folder belongs to no conversation: it is listed, not removed.
        assert_eq!(report.tasks.len(), 1);
        assert!(!report.tasks[0].conversation);
        // Not a git worktree any more: its unsaved files are unknown, never "none".
        assert_eq!(report.tasks[0].changes, None);

        repair_remove_task(crate::CoreRef(&f.core), report.tasks[0].path.clone())
            .await
            .unwrap();
        assert!(!session.exists());
        assert!(repair_remove_task(
            crate::CoreRef(&f.core),
            f.directory("project").to_string_lossy().into()
        )
        .await
        .is_err());
    }

    #[test]
    fn reads_the_version_number_of_each_cli() {
        for (line, version) in [
            ("2.1.327 (Claude Code)", "2.1.327"),
            ("codex-cli 0.167.0-alpha.4.1", "0.167.0-alpha.4.1"),
            ("opencode v2.3.0", "2.3.0"),
            ("1.5.18", "1.5.18"),
        ] {
            assert_eq!(
                crate::version_number(line).as_deref(),
                Some(version),
                "{line}"
            );
        }
        assert_eq!(crate::version_number("not a version"), None);
    }

    #[test]
    fn keeps_only_the_newest_backups() {
        let f = crate::test_support::Fixture::new();
        for _ in 0..(BACKUPS + 2) {
            backup(&f.core, &f.root.join("backups"), BACKUPS).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert_eq!(
            std::fs::read_dir(f.root.join("backups")).unwrap().count(),
            BACKUPS
        );
    }
}
