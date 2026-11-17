//! Invisible git plumbing: checkpoints and isolated worktrees never touch the
//! user's index, HEAD or visible branches (except the explicit lane branches).
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};
use tokio::process::Command;

pub(crate) fn git_cmd(dir: &Path) -> Command {
    let mut c = crate::proc::command("git");
    c.current_dir(dir)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_AUTHOR_NAME", "Lumen")
        .env("GIT_AUTHOR_EMAIL", "lumen@localhost")
        .env("GIT_COMMITTER_NAME", "Lumen")
        .env("GIT_COMMITTER_EMAIL", "lumen@localhost")
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true);
    c
}

pub(crate) async fn run(dir: &Path, args: &[&str], index: Option<&Path>) -> Result<String, String> {
    run_io(dir, args, index, None)
        .await
        .map(|o| String::from_utf8_lossy(&o).into_owned())
}

/// Raw git call; `input` is written to stdin. Private-index calls never convert line endings.
async fn run_io(
    dir: &Path,
    args: &[&str],
    index: Option<&Path>,
    input: Option<Vec<u8>>,
) -> Result<Vec<u8>, String> {
    use tokio::io::AsyncWriteExt;
    let mut c = git_cmd(dir);
    if let Some(i) = index {
        // Private-index plumbing is byte-exact: no CRLF conversion in snapshots or restores.
        c.args(["-c", "core.autocrlf=false", "-c", "core.safecrlf=false"]);
        c.env("GIT_INDEX_FILE", i);
    }
    c.args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    if input.is_some() {
        c.stdin(std::process::Stdio::piped());
    }
    let run = async {
        let mut child = c
            .spawn()
            .map_err(|e| format!("git is not available: {e}"))?;
        if let (Some(bytes), Some(mut stdin)) = (input, child.stdin.take()) {
            // Written concurrently with reading, so large inputs cannot deadlock on full pipes.
            tokio::spawn(async move {
                let _ = stdin.write_all(&bytes).await;
            });
        }
        child.wait_with_output().await.map_err(|e| e.to_string())
    };
    let out = tokio::time::timeout(std::time::Duration::from_secs(120), run)
        .await
        .map_err(|_| "git timed out")??;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(out.stdout)
}

/// Paths whose bytes git would transform on add or checkout (`text`, `eol`, `filter`,
/// `ident`, `working-tree-encoding` attributes), and the filter drivers they name.
async fn attributed(top: &Path, paths: &[String]) -> Result<(Vec<String>, Vec<String>), String> {
    if paths.is_empty() {
        return Ok((vec![], vec![]));
    }
    let out = run_io(
        top,
        &[
            "check-attr",
            "-z",
            "--stdin",
            "text",
            "eol",
            "filter",
            "ident",
            "working-tree-encoding",
        ],
        None,
        Some(paths.join("\0").into_bytes()),
    )
    .await?;
    let out = String::from_utf8_lossy(&out);
    let fields: Vec<&str> = out.split('\0').collect();
    let (mut hit, mut filters) = (BTreeSet::new(), BTreeSet::new());
    for row in fields.as_chunks::<3>().0 {
        let (path, attr, value) = (row[0], row[1], row[2]);
        if value == "unspecified" || value == "unset" {
            continue;
        }
        hit.insert(path.to_string());
        if attr == "filter" {
            filters.insert(value.to_string());
        }
    }
    Ok((hit.into_iter().collect(), filters.into_iter().collect()))
}

/// Config that turns named filter drivers into byte-exact no-ops, so neither a snapshot
/// nor a restore runs an external clean/smudge process (for example a Git LFS download).
fn neutral(filters: &[String]) -> Vec<String> {
    filters
        .iter()
        .filter(|f| {
            f.chars()
                .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
        })
        .flat_map(|f| {
            [
                format!("filter.{f}.clean=cat"),
                format!("filter.{f}.smudge=cat"),
                format!("filter.{f}.process="),
                format!("filter.{f}.required=false"),
            ]
        })
        .flat_map(|kv| ["-c".to_string(), kv])
        .collect()
}

fn argv(v: &[String]) -> Vec<&str> {
    v.iter().map(String::as_str).collect()
}

async fn all_paths(top: &Path) -> Result<Vec<String>, String> {
    Ok(run(
        top,
        &[
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ],
        None,
    )
    .await?
    .split('\0')
    .filter(|p| !p.is_empty())
    .map(String::from)
    .collect())
}

/// Uncommitted paths (tracked changes and untracked files) with their two-letter status.
pub async fn changed_paths(top: &Path) -> Result<Vec<(String, String)>, String> {
    let out = run(
        top,
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
        None,
    )
    .await?;
    let mut items = vec![];
    let mut parts = out.split('\0').filter(|p| !p.is_empty());
    while let Some(entry) = parts.next() {
        if entry.len() < 4 {
            continue;
        }
        let (status, path) = entry.split_at(3);
        // Renames list the new path first, then the old one.
        if status.starts_with('R') || status.starts_with('C') {
            parts.next();
        }
        items.push((status.trim().to_string(), path.to_string()));
    }
    Ok(items)
}

/// `git init` in a folder so Orchestra can use worktrees there.
pub async fn init(dir: &Path) -> Result<(), String> {
    run(dir, &["init", "-q"], None).await.map(|_| ())
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
        let (exact, filters) = attributed(top, &all_paths(top).await?).await?;
        // Seeding from HEAD keeps `add -A` fast on large repositories.
        let _ = run(top, &["read-tree", "HEAD"], Some(&index)).await;
        let mut add = neutral(&filters);
        add.extend(["add", "-A", "--", "."].map(String::from));
        run(top, &argv(&add), Some(&index)).await?;
        // Attributed files are stored exactly as they are on disk, not as git would normalize them.
        let present: Vec<&String> = exact.iter().filter(|p| top.join(p).is_file()).collect();
        if !present.is_empty() {
            let list: String = present.iter().map(|p| format!("{p}\n")).collect();
            let hashes = run_io(
                top,
                &["hash-object", "-w", "--no-filters", "--stdin-paths"],
                None,
                Some(list.into_bytes()),
            )
            .await?;
            let hashes = String::from_utf8_lossy(&hashes);
            let staged = run(top, &["ls-files", "-s", "-z"], Some(&index)).await?;
            let modes: std::collections::HashMap<&str, &str> = staged
                .split('\0')
                .filter_map(|r| {
                    let (meta, path) = r.split_once('\t')?;
                    Some((path, meta.split(' ').next()?))
                })
                .collect();
            let info: String = present
                .iter()
                .zip(hashes.lines())
                .map(|(p, h)| {
                    let mode = modes.get(p.as_str()).copied().unwrap_or("100644");
                    format!("{mode} {h}\t{p}\0")
                })
                .collect();
            run_io(
                top,
                &["update-index", "-z", "--index-info"],
                Some(&index),
                Some(info.into_bytes()),
            )
            .await?;
        }
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
    // As in Hermes Agent's rollback, the state being replaced is kept first, so a restore
    // can itself be undone from `refs/lumen/checkpoints/before-restore-*`.
    let current_tree = checkpoint(top, &format!("before-restore-{}", crate::models::id())).await?;
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
        let paths: Vec<String> = after.iter().cloned().collect();
        let (_, filters) = attributed(top, &paths).await?;
        let mut checkout = neutral(&filters);
        checkout.extend(["checkout-index", "-a", "-f"].map(String::from));
        run(top, &argv(&checkout), Some(&index)).await?;
        // Attributes as of the snapshot, now that its own .gitattributes is back on disk.
        let (exact, _) = attributed(top, &paths).await?;
        // Snapshot bytes of attributed files were stored unconverted: write them back as-is.
        for path in exact {
            let spec = format!("{tree}:{path}");
            let blob = run_io(top, &["cat-file", "blob", &spec], None, None).await?;
            std::fs::write(top.join(&path), blob)
                .map_err(|e| format!("Cannot restore {path}: {e}"))?;
        }
        Ok::<_, String>(())
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

/// Exact paths changed by this turn, including new files, relative to its checkpoint.
pub async fn paths_since(top: &Path, tree: &str) -> Result<Vec<String>, String> {
    let current = write_tree(top).await?;
    let changed = run(top, &["diff", "--name-only", "-z", tree, &current], None).await?;
    Ok(changed
        .split('\0')
        .filter(|p| !p.is_empty())
        .map(str::to_string)
        .collect())
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
    // Byte-exact checkout: with core.autocrlf the lane would get CRLF files and its
    // patch would rewrite every line, so it could no longer apply to the user's folder.
    run(
        path,
        &[
            "-c",
            "core.autocrlf=false",
            "-c",
            "core.safecrlf=false",
            "read-tree",
            "-u",
            "--reset",
            base,
        ],
        None,
    )
    .await?;
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

    fn bytes(top: &Path, p: &str) -> Vec<u8> {
        std::fs::read(top.join(p)).unwrap()
    }

    /// Rollback returns exactly to the checkpoint and can itself be undone.
    #[tokio::test]
    async fn rollback_is_exact_and_undoable() {
        let Some(top) = setup().await else { return };
        std::fs::write(top.join("keep.txt"), "kept").unwrap();
        let tree = checkpoint(&top, "exact").await.unwrap();
        std::fs::write(top.join("a.txt"), "edited").unwrap();
        std::fs::remove_file(top.join("keep.txt")).unwrap();
        std::fs::create_dir_all(top.join("deep/er")).unwrap();
        std::fs::write(top.join("deep/er/new.txt"), "agent").unwrap();
        let after = write_tree(&top).await.unwrap();
        restore(&top, &tree).await.unwrap();
        assert_eq!(write_tree(&top).await.unwrap(), tree);
        assert!(preview(&top, &tree).await.unwrap().is_empty());
        assert_eq!(bytes(&top, "keep.txt"), b"kept");
        assert!(!top.join("deep/er/new.txt").exists());
        let undo = run(
            &top,
            &[
                "for-each-ref",
                "--format=%(refname)",
                "refs/lumen/checkpoints/",
            ],
            None,
        )
        .await
        .unwrap();
        let undo = undo
            .lines()
            .find(|r| r.contains("before-restore-"))
            .expect("restore keeps the replaced state");
        let undo_tree = run(&top, &["rev-parse", &format!("{undo}^{{tree}}")], None)
            .await
            .unwrap();
        assert_eq!(undo_tree.trim(), after);
        restore(&top, undo_tree.trim()).await.unwrap();
        assert_eq!(bytes(&top, "deep/er/new.txt"), b"agent");
        let _ = std::fs::remove_dir_all(top);
    }

    /// CRLF, mixed endings and eol attributes survive snapshot and restore byte for byte,
    /// even with core.autocrlf enabled in the user's repository.
    #[tokio::test]
    async fn crlf_and_eol_attributes_round_trip_exactly() {
        let Some(top) = setup().await else { return };
        run(&top, &["config", "core.autocrlf", "true"], None)
            .await
            .unwrap();
        std::fs::write(
            top.join(".gitattributes"),
            "*.txt text eol=lf\n*.bat text eol=crlf\n",
        )
        .unwrap();
        let crlf = b"one\r\ntwo\r\n".to_vec();
        let mixed = b"a\nb\r\nc\rd".to_vec();
        let lf_bat = b"@echo off\nexit\n".to_vec();
        std::fs::write(top.join("crlf.txt"), &crlf).unwrap();
        std::fs::write(top.join("mixed.txt"), &mixed).unwrap();
        std::fs::write(top.join("run.bat"), &lf_bat).unwrap();
        std::fs::write(top.join("plain.md"), b"x\r\ny\r\n").unwrap();
        let tree = checkpoint(&top, "crlf").await.unwrap();
        for f in ["crlf.txt", "mixed.txt", "run.bat", "plain.md"] {
            std::fs::write(top.join(f), "changed").unwrap();
        }
        restore(&top, &tree).await.unwrap();
        assert_eq!(bytes(&top, "crlf.txt"), crlf);
        assert_eq!(bytes(&top, "mixed.txt"), mixed);
        assert_eq!(bytes(&top, "run.bat"), lf_bat);
        assert_eq!(bytes(&top, "plain.md"), b"x\r\ny\r\n");
        assert!(preview(&top, &tree).await.unwrap().is_empty());
        let _ = std::fs::remove_dir_all(top);
    }

    /// A clean/smudge filter (LFS-like) never runs and never alters checkpoint bytes,
    /// even when its driver is required and would fail.
    #[tokio::test]
    async fn filter_drivers_are_neutralized() {
        let Some(top) = setup().await else { return };
        for (k, v) in [
            ("filter.boom.clean", "false"),
            ("filter.boom.smudge", "false"),
            ("filter.boom.required", "true"),
        ] {
            run(&top, &["config", k, v], None).await.unwrap();
        }
        std::fs::write(top.join(".gitattributes"), "*.bin filter=boom\n").unwrap();
        std::fs::write(top.join("data.bin"), b"\x00raw\r\nbytes").unwrap();
        let tree = checkpoint(&top, "filter").await.unwrap();
        std::fs::write(top.join("data.bin"), b"overwritten").unwrap();
        restore(&top, &tree).await.unwrap();
        assert_eq!(bytes(&top, "data.bin"), b"\x00raw\r\nbytes");
        let _ = std::fs::remove_dir_all(top);
    }

    /// Concurrent snapshots use private indexes: identical trees, user index untouched.
    #[tokio::test]
    async fn concurrent_checkpoints_are_isolated() {
        let Some(top) = setup().await else { return };
        std::fs::write(top.join("a.txt"), "dirty").unwrap();
        std::fs::write(top.join("new.txt"), "untracked").unwrap();
        let runs = (0..6).map(|i| {
            let top = top.clone();
            async move { checkpoint(&top, &format!("c{i}")).await }
        });
        let trees = futures_util::future::join_all(runs).await;
        let first = trees[0].clone().unwrap();
        assert!(trees.iter().all(|t| t.as_ref().unwrap() == &first));
        let status = run(&top, &["status", "--porcelain"], None).await.unwrap();
        assert!(status.contains(" M a.txt") && status.contains("?? new.txt"));
        let _ = std::fs::remove_dir_all(top);
    }
}
