//! Project-scoped previews. Git external diff/textconv programs are deliberately disabled.
use serde::Serialize;
use std::path::{Component, Path};
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diff {
    text: String,
    truncated: bool,
}
pub(crate) fn relative(file: &str) -> Result<(), String> {
    if file.is_empty()
        || Path::new(file)
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
        || crate::context::is_secret(file)
    {
        return Err("Choose a project file".into());
    }
    Ok(())
}
#[tauri::command]
pub async fn file_change_diff(folder: String, file: String) -> Result<Diff, String> {
    relative(&file)?;
    let root = dunce::canonicalize(&folder).map_err(|e| e.to_string())?;
    let top = crate::gitops::toplevel(&root)
        .await
        .ok_or("Change review requires a Git repository")?;
    let path = root.join(&file);
    let rel = path
        .strip_prefix(&top)
        .map_err(|_| "File is outside the repository")?
        .to_string_lossy()
        .replace('\\', "/");
    let text = if path.exists()
        && crate::gitops::run(&top, &["ls-files", "--error-unmatch", "--", &rel], None)
            .await
            .is_err()
    {
        let path = crate::context::jail(&root, &file)?;
        let metadata = std::fs::metadata(&path).map_err(|e| e.to_string())?;
        if metadata.len() > 512 * 1024 {
            return Err("This untracked file is too large to preview".into());
        }
        let text = std::fs::read_to_string(path)
            .map_err(|_| "This file is binary; open it with its editor")?;
        format!(
            "--- /dev/null\n+++ b/{rel}\n{}",
            text.lines().map(|l| format!("+{l}\n")).collect::<String>()
        )
    } else {
        let head = crate::gitops::run(&top, &["rev-parse", "--verify", "HEAD"], None)
            .await
            .is_ok();
        let args = if head {
            vec!["diff", "--no-ext-diff", "--no-textconv", "HEAD", "--", &rel]
        } else {
            vec![
                "diff",
                "--cached",
                "--no-ext-diff",
                "--no-textconv",
                "--",
                &rel,
            ]
        };
        crate::gitops::run(&top, &args, None).await?
    };
    Ok(Diff {
        truncated: text.chars().count() > 160_000,
        text: text.chars().take(160_000).collect(),
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn scene_diff_previews_content_and_rejects_external_paths() {
        let f = crate::test_support::Fixture::new();
        crate::gitops::init(&f.root).await.unwrap();
        std::fs::write(
            f.root.join("scene.tscn"),
            "[gd_scene format=3]\n[node name=\"Hero\" type=\"Node2D\"]\n",
        )
        .unwrap();
        let diff = file_change_diff(f.root.to_string_lossy().into(), "scene.tscn".into())
            .await
            .unwrap();
        assert!(diff.text.contains("+[node name=\"Hero\""));
        assert!(relative("../outside.tscn").is_err());
        assert!(relative(".env").is_err());
    }
}
