//! Invisible git plumbing: checkpoints and isolated worktrees never touch the
//! user's index, HEAD or visible branches (except the explicit lane branches).
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};
use tokio::process::Command;

fn git_cmd(dir: &Path) -> Command {
    let mut c = Command::new("git");
    c.current_dir(dir)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_AUTHOR_NAME", "Lumen")
        .env("GIT_AUTHOR_EMAIL", "lumen@localhost")
        .env("GIT_COMMITTER_NAME", "Lumen")
        .env("GIT_COMMITTER_EMAIL", "lumen@localhost")
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true);
    #[cfg(windows)]
    c.creation_flags(0x08000000);
    c
}

async fn run(dir: &Path, args: &[&str], index: Option<&Path>) -> Result<String, String> {
    let mut c = git_cmd(dir);
    if let Some(i) = index {
        // Private-index plumbing is byte-exact: no CRLF conversion in snapshots or restores.
        c.args(["-c", "core.autocrlf=false", "-c", "core.safecrlf=false"]);
        c.env("GIT_INDEX_FILE", i);
    }
    c.args(args);
    let out = tokio::time::timeout(std::time::Duration::from_secs(120), c.output())
        .await
        .map_err(|_| "git timed out")?
        .map_err(|e| format!("git is not available: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Repository root for a workspace folder, if it is inside a git work tree.
pub async fn toplevel(dir: &Path) -> Option<PathBuf> {
    run(dir, &["rev-parse", "--show-toplevel"], None)
        .await
        .ok()
        .map(|s| PathBuf::from(s.trim()))
        .and_then(|p| dunce::canonicalize(p).ok())
}

fn temp_index(top: &Path) -> PathBuf {
    std::env::temp_dir().join(format!(
        "lumen-index-{}-{}",
        top.to_string_lossy().len(),
        crate::models::id()
    ))
}

/// Writes the current working tree (tracked + untracked, honoring .gitignore)
/// to a tree object through a private index.
pub async fn write_tree(top: &Path) -> Result<String, String> {
    let index = temp_index(top);
    let result = async {
        // Seeding from HEAD keeps `add -A` fast on large repositories.
        let _ = run(top, &["read-tree", "HEAD"], Some(&index)).await;
        run(top, &["add", "-A", "--", "."], Some(&index)).await?;
        Ok::<_, String>(
            run(top, &["write-tree"], Some(&index))
                .await?
                .trim()
                .to_string(),
        )
    }
    .await;
    let _ = std::fs::remove_file(&index);
    result
}

/// Snapshot kept alive against `git gc` through a private ref.
pub async fn checkpoint(top: &Path, key: &str) -> Result<String, String> {
    let tree = write_tree(top).await?;
    let commit = run(top, &["commit-tree", &tree, "-m", "Lumen checkpoint"], None)
        .await?
        .trim()
        .to_string();
    run(
        top,
        &[
            "update-ref",
            &format!("refs/lumen/checkpoints/{key}"),
            &commit,
        ],
        None,
    )
    .await?;
    Ok(tree)
}

async fn files(top: &Path, tree: &str) -> Result<BTreeSet<String>, String> {
    Ok(
        run(top, &["ls-tree", "-r", "--name-only", "-z", tree], None)
            .await?
            .split('\0')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect(),
    )
}

/// Files that a restore would change, as `M path`, `A path` (recreated) or `D path` (removed).
pub async fn preview(top: &Path, tree: &str) -> Result<Vec<String>, String> {
    let now = write_tree(top).await?;
    let out = run(
        top,
        &[
            "diff-tree",
            "-r",
            "--name-status",
            "--no-renames",
            &now,
            tree,
        ],
        None,
    )
    .await?;
    Ok(out
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.replace('\t', " "))
        .collect())
}

/// Restores the working tree to a snapshot without changing the user's index or HEAD.
pub async fn restore(top: &Path, tree: &str) -> Result<usize, String> {
    let current_tree = write_tree(top).await?;
    let before = files(top, &current_tree).await?;
    let after = files(top, tree).await?;
    let changed = preview(top, tree).await?.len();
    for extra in before.difference(&after) {
        let path = top.join(extra);
        if path.starts_with(top) {
            let _ = std::fs::remove_file(&path);
        }
    }
    let index = temp_index(top);
    let result = async {
        run(top, &["read-tree", tree], Some(&index)).await?;
        run(top, &["checkout-index", "-a", "-f"], Some(&index)).await
    }
    .await;
    let _ = std::fs::remove_file(&index);
    result.map(|_| changed)
}

/// Unified diff (-U3) between a snapshot and the current working tree.
pub async fn diff_since(top: &Path, tree: &str, limit: usize) -> Result<String, String> {
    let now = write_tree(top).await?;
    let diff = run(top, &["diff", "-U3", "--no-color", tree, &now], None).await?;
    Ok(truncate(diff, limit))
}

/// Summary of uncommitted work relative to HEAD, for compact handoffs.
pub async fn diff_head(top: &Path, limit: usize) -> Result<String, String> {
    let stat = run(top, &["diff", "--stat", "HEAD"], None)
        .await
        .unwrap_or_default();
    let new = run(top, &["ls-files", "--others", "--exclude-standard"], None)
        .await
        .unwrap_or_default();
    let new = if new.trim().is_empty() {
        String::new()
    } else {
        format!("New files:\n{new}")
    };
    let diff = run(top, &["diff", "-U3", "--no-color", "HEAD"], None)
        .await
        .unwrap_or_default();
    Ok(truncate(format!("{stat}{new}\n{diff}"), limit))
}

pub fn truncate(mut s: String, limit: usize) -> String {
    if s.len() > limit {
        let mut cut = limit;
        while !s.is_char_boundary(cut) {
            cut -= 1;
        }
        s.truncate(cut);
        s.push_str("\n… (diff truncated)");
    }
    s
}

/// A detached worktree on its own branch, seeded with the user's uncommitted state.
pub async fn add_lane(top: &Path, base: &str, path: &Path, branch: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let p = path.to_string_lossy();
    if run(top, &["rev-parse", "--verify", "HEAD"], None)
        .await
        .is_ok()
    {
        run(top, &["worktree", "add", "-b", branch, &p, "HEAD"], None).await?;
    } else {
        run(
            top,
            &["worktree", "add", "--orphan", "-b", branch, &p],
            None,
        )
        .await?;
    }
    run(path, &["read-tree", "-u", "--reset", base], None).await?;
    Ok(())
}

pub async fn lane_patch(path: &Path, base: &str) -> Result<String, String> {
    let now = write_tree(path).await?;
    run(path, &["diff", "--binary", "--no-color", base, &now], None).await
}

pub async fn apply_patch(top: &Path, patch: &str) -> Result<(), String> {
    if patch.trim().is_empty() {
        return Err("This version has no file changes".into());
    }
    let file = std::env::temp_dir().join(format!("lumen-{}.patch", crate::models::id()));
    std::fs::write(&file, patch).map_err(|e| e.to_string())?;
    let f = file.to_string_lossy().to_string();
    let result = match run(top, &["apply", "--check", &f], None).await {
        Ok(_) => run(top, &["apply", "--whitespace=nowarn", &f], None).await.map(|_| ()),
        Err(e) => Err(format!(
            "The project changed since this version was produced, so it cannot be applied cleanly: {e}"
        )),
    };
    let _ = std::fs::remove_file(file);
    result
}

pub async fn remove_lane(top: &Path, path: &Path, branch: &str) {
    let _ = run(
        top,
        &["worktree", "remove", "--force", &path.to_string_lossy()],
        None,
    )
    .await;
    let _ = run(top, &["branch", "-D", branch], None).await;
    let _ = run(top, &["worktree", "prune"], None).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    async fn setup() -> Option<PathBuf> {
        let dir = std::env::temp_dir().join(format!("lumen-git-{}", crate::models::id()));
        std::fs::create_dir_all(&dir).ok()?;
        run(&dir, &["init", "-q"], None).await.ok()?;
        std::fs::write(dir.join("a.txt"), "one").ok()?;
        run(&dir, &["add", "."], None).await.ok()?;
        run(
            &dir,
            &[
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "commit",
                "-qm",
                "init",
            ],
            None,
        )
        .await
        .ok()?;
        dunce::canonicalize(dir).ok()
    }
    #[tokio::test]
    async fn checkpoint_restore_and_lanes() {
        let Some(top) = setup().await else { return };
        std::fs::write(top.join("a.txt"), "dirty").unwrap();
        let tree = checkpoint(&top, "t1").await.unwrap();
        std::fs::write(top.join("a.txt"), "changed").unwrap();
        std::fs::write(top.join("new.txt"), "x").unwrap();
        assert_eq!(preview(&top, &tree).await.unwrap().len(), 2);
        restore(&top, &tree).await.unwrap();
        assert_eq!(std::fs::read_to_string(top.join("a.txt")).unwrap(), "dirty");
        assert!(!top.join("new.txt").exists());
        // The user's own index is untouched: a.txt stays unstaged.
        let status = run(&top, &["status", "--porcelain"], None).await.unwrap();
        assert!(status.contains(" M a.txt"));
        let lane = top.parent().unwrap().join(format!(
            "{}-lane",
            top.file_name().unwrap().to_string_lossy()
        ));
        add_lane(&top, &tree, &lane, "lumen/test-lane")
            .await
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(lane.join("a.txt")).unwrap(),
            "dirty"
        );
        std::fs::write(lane.join("a.txt"), "from lane").unwrap();
        let patch = lane_patch(&lane, &tree).await.unwrap();
        apply_patch(&top, &patch).await.unwrap();
        assert_eq!(
            std::fs::read_to_string(top.join("a.txt")).unwrap(),
            "from lane"
        );
        remove_lane(&top, &lane, "lumen/test-lane").await;
        assert!(!lane.exists());
        let _ = std::fs::remove_dir_all(top);
    }
}
