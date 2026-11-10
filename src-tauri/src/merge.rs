//! Manual Both merge: immutable Git snapshots, a private index, then one checked patch.
use crate::{gitops, turns, Core};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeSet, HashMap},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
};
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    id: String,
    files: Vec<File>,
}
#[derive(Serialize)]
pub struct File {
    path: String,
    base: Option<String>,
    left: Option<String>,
    right: Option<String>,
    binary: bool,
}
#[derive(Clone)]
struct Snapshot {
    session: String,
    message: String,
    top: PathBuf,
    base: String,
    trees: [String; 2],
    paths: BTreeSet<String>,
}
fn previews() -> &'static Mutex<HashMap<String, Snapshot>> {
    static PREVIEWS: OnceLock<Mutex<HashMap<String, Snapshot>>> = OnceLock::new();
    PREVIEWS.get_or_init(Default::default)
}
#[derive(Deserialize)]
pub struct Edit {
    path: String,
    side: String,
    text: Option<String>,
}
async fn content(top: &Path, tree: &str, path: &str) -> Option<String> {
    gitops::run(top, &["show", &format!("{tree}:{path}")], None)
        .await
        .ok()
}
#[tauri::command]
pub async fn preview_manual_merge(
    core: tauri::State<'_, Arc<Core>>,
    session: String,
    message: String,
) -> Result<Preview, String> {
    let s = core.store.session(&session)?;
    if !crate::turns::conversations_on_workspace(&core, &s).is_empty()
        || core
            .jobs
            .lock()
            .map_err(|_| "Jobs lock failed")?
            .contains_key(&session)
    {
        return Err("Wait for active project turns before merging".into());
    }
    let rows = core.store.messages(&session)?;
    let first = rows
        .iter()
        .find(|m| m.id == message)
        .and_then(turns::lane_of)
        .ok_or("This version is no longer available")?;
    let group = Path::new(&first.path).parent();
    let second = rows
        .iter()
        .filter(|m| m.id != message)
        .filter_map(turns::lane_of)
        .find(|l| Path::new(&l.path).parent() == group && l.base == first.base);
    let top = gitops::toplevel(Path::new(&s.workspace))
        .await
        .ok_or("The project is no longer a Git repository")?;
    if dunce::canonicalize(&first.top).ok().as_ref() != Some(&top)
        || second
            .as_ref()
            .is_some_and(|second| first.top != second.top)
    {
        return Err("The versions belong to another project".into());
    }
    let trees = [
        gitops::write_tree(Path::new(&first.path)).await?,
        if let Some(second) = second {
            gitops::write_tree(Path::new(&second.path)).await?
        } else {
            first.base.clone()
        },
    ];
    let mut paths = BTreeSet::new();
    for tree in &trees {
        for file in gitops::run(
            &top,
            &["diff", "--name-only", "-z", &first.base, tree],
            None,
        )
        .await?
        .split('\0')
        .filter(|s| !s.is_empty())
        {
            crate::review::relative(file)?;
            paths.insert(file.to_string());
        }
    }
    if paths.is_empty() || paths.len() > 100 {
        return Err("Manual merge needs between 1 and 100 changed files".into());
    }
    let mut files = Vec::new();
    let mut bytes = 0;
    for path in &paths {
        let base = content(&top, &first.base, path).await;
        let left = content(&top, &trees[0], path).await;
        let right = content(&top, &trees[1], path).await;
        let binary = [&base, &left, &right]
            .iter()
            .filter_map(|s| s.as_ref())
            .any(|s| s.contains('\0') || s.contains('\u{fffd}') || s.len() > 128_000);
        let (base, left, right) = if binary {
            (None, None, None)
        } else {
            (base, left, right)
        };
        bytes += [&base, &left, &right]
            .iter()
            .filter_map(|s| s.as_ref())
            .map(|s| s.len())
            .sum::<usize>();
        if bytes > 2_000_000 {
            return Err(
                "This comparison is too large; merge smaller groups with your editor".into(),
            );
        }
        files.push(File {
            path: path.clone(),
            base,
            left,
            right,
            binary,
        });
    }
    let id = uuid::Uuid::new_v4().to_string();
    let mut cache = previews().lock().map_err(|_| "Merge lock failed")?;
    if cache.len() >= 32 {
        cache.clear();
    }
    cache.insert(
        id.clone(),
        Snapshot {
            session,
            message,
            top,
            base: first.base,
            trees,
            paths,
        },
    );
    Ok(Preview { id, files })
}
async fn entry(top: &Path, tree: &str, path: &str) -> Result<Option<(String, String)>, String> {
    let value = gitops::run(
        top,
        &[
            "ls-tree",
            "--format=%(objectmode) %(objectname)",
            tree,
            "--",
            path,
        ],
        None,
    )
    .await?;
    let mut parts = value.split_whitespace();
    Ok(parts
        .next()
        .zip(parts.next())
        .map(|(m, id)| (m.into(), id.into())))
}
async fn patch(snapshot: &Snapshot, edits: &[Edit]) -> Result<String, String> {
    let index = std::env::temp_dir().join(format!("lumen-merge-{}.index", uuid::Uuid::new_v4()));
    let blob = index.with_extension("text");
    let result = async {
        gitops::run(&snapshot.top, &["read-tree", &snapshot.base], Some(&index)).await?;
        let mut seen = BTreeSet::new();
        let mut total = 0;
        for edit in edits {
            if !snapshot.paths.contains(&edit.path) || !seen.insert(&edit.path) {
                return Err("Merge contains an unknown or duplicate file".into());
            }
            let object = match edit.side.as_str() {
                "left" => entry(&snapshot.top, &snapshot.trees[0], &edit.path).await?,
                "right" => entry(&snapshot.top, &snapshot.trees[1], &edit.path).await?,
                "base" => entry(&snapshot.top, &snapshot.base, &edit.path).await?,
                "delete" => None,
                "manual" => {
                    let text = edit.text.as_ref().ok_or("Missing merged text")?;
                    total += text.len();
                    if text.len() > 256_000 || total > 2_000_000 {
                        return Err("Merged text is too large".into());
                    }
                    let mode = entry(&snapshot.top, &snapshot.base, &edit.path)
                        .await?
                        .map(|(mode, _)| mode)
                        .unwrap_or_else(|| "100644".into());
                    if mode != "100644" && mode != "100755" {
                        return Err("Edit symlinks and submodules with your editor".into());
                    }
                    std::fs::write(&blob, text).map_err(|e| e.to_string())?;
                    let id = gitops::run(
                        &snapshot.top,
                        &["hash-object", "-w", "--", &blob.to_string_lossy()],
                        None,
                    )
                    .await?;
                    Some((mode, id.trim().into()))
                }
                _ => return Err("Choose left, right, base, delete or manual for each file".into()),
            };
            if let Some((mode, id)) = object {
                gitops::run(
                    &snapshot.top,
                    &[
                        "update-index",
                        "--add",
                        "--cacheinfo",
                        &mode,
                        &id,
                        &edit.path,
                    ],
                    Some(&index),
                )
                .await?;
            } else {
                gitops::run(
                    &snapshot.top,
                    &["update-index", "--force-remove", "--", &edit.path],
                    Some(&index),
                )
                .await?;
            }
        }
        if seen.len() != snapshot.paths.len() {
            return Err("Choose a result for every changed file".into());
        }
        let tree = gitops::run(&snapshot.top, &["write-tree"], Some(&index)).await?;
        gitops::run(
            &snapshot.top,
            &[
                "diff",
                "--binary",
                "--no-ext-diff",
                "--no-textconv",
                &snapshot.base,
                tree.trim(),
            ],
            None,
        )
        .await
    }
    .await;
    let _ = std::fs::remove_file(index);
    let _ = std::fs::remove_file(blob);
    result
}
#[tauri::command]
pub async fn apply_manual_merge(
    core: tauri::State<'_, Arc<Core>>,
    session: String,
    preview: String,
    edits: Vec<Edit>,
) -> Result<(), String> {
    let snapshot = previews()
        .lock()
        .map_err(|_| "Merge lock failed")?
        .get(&preview)
        .cloned()
        .ok_or("Reopen the merge view; this preview expired")?;
    if snapshot.session != session {
        return Err("This preview belongs to another conversation".into());
    }
    let s = core.store.session(&session)?;
    let lock = turns::workspace_lock(&core, &s)?;
    let guard = lock.lock().await;
    if gitops::toplevel(Path::new(&s.workspace)).await.as_ref() != Some(&snapshot.top)
        || !core.store.messages(&session)?.iter().any(|m| {
            m.id == snapshot.message
                && turns::lane_of(m).is_some_and(|lane| lane.base == snapshot.base)
        })
    {
        return Err("This merge preview no longer belongs to the active project versions".into());
    }
    if core
        .jobs
        .lock()
        .map_err(|_| "Jobs lock failed")?
        .contains_key(&session)
        || !turns::conversations_on_workspace(&core, &s).is_empty()
    {
        return Err("Wait for active project turns before merging".into());
    }
    let changes = patch(&snapshot, &edits).await?;
    if !changes.trim().is_empty() {
        gitops::apply_patch(&snapshot.top, &changes).await?;
    }
    previews()
        .lock()
        .map_err(|_| "Merge lock failed")?
        .remove(&preview);
    drop(guard);
    let engine = core.inner().clone();
    turns::resolve_lanes(core, session.clone(), snapshot.message.clone(), false).await?;
    if let Some(mut message) = engine
        .store
        .messages(&session)?
        .into_iter()
        .find(|m| m.id == snapshot.message)
    {
        for activity in message
            .activity
            .iter_mut()
            .filter(|a| a.kind == "discarded")
        {
            activity.kind = "applied".into();
            activity.text = "Manually merged both versions".into();
        }
        engine.store.save_message(&session, &message)?;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn manual_merge_uses_a_private_index_and_keeps_unrelated_edits() {
        let f = crate::test_support::Fixture::new();
        gitops::init(&f.root).await.unwrap();
        std::fs::write(f.root.join("a.txt"), "base\n").unwrap();
        let base = gitops::write_tree(&f.root).await.unwrap();
        let snapshot = Snapshot {
            session: "s".into(),
            message: "m".into(),
            top: f.root.clone(),
            base: base.clone(),
            trees: [base.clone(), base],
            paths: BTreeSet::from(["a.txt".into()]),
        };
        let patch = patch(
            &snapshot,
            &[Edit {
                path: "a.txt".into(),
                side: "manual".into(),
                text: Some("merged\n".into()),
            }],
        )
        .await
        .unwrap();
        assert_eq!(
            std::fs::read_to_string(f.root.join("a.txt")).unwrap(),
            "base\n"
        );
        std::fs::write(f.root.join("unrelated.txt"), "keep").unwrap();
        gitops::apply_patch(&f.root, &patch).await.unwrap();
        assert_eq!(
            std::fs::read_to_string(f.root.join("a.txt"))
                .unwrap()
                .replace("\r\n", "\n"),
            "merged\n"
        );
        assert_eq!(
            std::fs::read_to_string(f.root.join("unrelated.txt")).unwrap(),
            "keep"
        );
    }
}
