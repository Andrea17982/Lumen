//! Built-in content integrity check for Godot projects: every `res://` path that scenes,
//! resources, scripts and project settings mention must exist. Read-only; source assets and
//! generated files are never modified.
use regex::Regex;
use std::path::Path;
use std::sync::OnceLock;

const SCANNED: &[&str] = &["tscn", "tres", "gd", "cs", "godot", "cfg", "gdshader"];
const MAX_FILES: usize = 20_000;

pub const COMMAND: &str = "Lumen: check res:// references";

fn reference() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r#"res://[^"'`\s)\]>]+"#).unwrap())
}

/// Built paths (`"res://x_%d.png"`, `"res://" + name`) cannot be checked statically.
fn dynamic(path: &str) -> bool {
    path.contains('%')
        || path.contains('{')
        || path.contains('*')
        || path.ends_with('/')
        || path.contains("::")
}

/// `.gitignore` rules from the project root down to the Godot folder.
fn ignored_by_git(root: &Path, project: &Path) -> ignore::gitignore::Gitignore {
    let mut builder = ignore::gitignore::GitignoreBuilder::new(root);
    let mut dir = Some(project);
    let mut chain = vec![];
    while let Some(d) = dir {
        chain.push(d.join(".gitignore"));
        if d == root {
            break;
        }
        dir = d.parent();
    }
    for file in chain.into_iter().rev() {
        let _ = builder.add(file);
    }
    builder
        .build()
        .unwrap_or_else(|_| ignore::gitignore::Gitignore::empty())
}

pub struct Scan {
    /// `file:line: missing res://...`, project-relative to `root` so source links apply.
    pub missing: Vec<String>,
    /// Missing paths that git ignores: local or generated files a clean copy does not have.
    pub local_only: Vec<String>,
    pub checked: usize,
}

pub fn missing_references(root: &Path, project: &Path) -> Scan {
    let git = ignored_by_git(root, project);
    let mut local_only = vec![];
    let mut missing = vec![];
    let mut checked = 0;
    let mut files = 0;
    for entry in ignore::WalkBuilder::new(project)
        .hidden(true)
        .git_ignore(true)
        .require_git(false)
        .build()
        .flatten()
    {
        let path = entry.path();
        let scanned = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| SCANNED.contains(&e));
        if !scanned || !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        files += 1;
        if files > MAX_FILES {
            break;
        }
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        let rel = path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        for (i, line) in text.lines().enumerate() {
            for m in reference().find_iter(line) {
                let res = m.as_str().trim_end_matches([',', ';', '.']);
                if dynamic(res) {
                    continue;
                }
                checked += 1;
                let target = project.join(res.trim_start_matches("res://"));
                if !target.exists() {
                    let entry = format!("{rel}:{}: missing {res}", i + 1);
                    if git.matched_path_or_any_parents(&target, false).is_ignore() {
                        local_only.push(entry);
                    } else {
                        missing.push(entry);
                    }
                }
            }
        }
    }
    Scan {
        missing,
        local_only,
        checked,
    }
}

/// Runs the check over one Godot project folder; text output like any other check.
pub fn run(root: &Path, project: &Path) -> (bool, String) {
    let scan = missing_references(root, project);
    let mut text = if scan.missing.is_empty() {
        format!("{} res:// references checked; none missing.", scan.checked)
    } else {
        format!(
            "{} of {} res:// references point to missing files:
{}",
            scan.missing.len(),
            scan.checked,
            scan.missing.join(
                "
"
            )
        )
    };
    if !scan.local_only.is_empty() {
        text.push_str(&format!(
            "

{} missing path(s) are ignored by git (local or generated files), not counted as failures:
{}",
            scan.local_only.len(),
            scan.local_only.join(
                "
"
            )
        ));
    }
    (scan.missing.is_empty(), text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_missing_paths_with_their_source_line() {
        let base = std::env::temp_dir().join(format!("lumen-assets-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let game = base.join("game");
        std::fs::create_dir_all(game.join("art")).unwrap();
        std::fs::write(
            game.join("project.godot"),
            "run/main_scene=\"res://main.tscn\"\n",
        )
        .unwrap();
        std::fs::write(game.join("art/hero.png"), b"png").unwrap();
        std::fs::write(
            game.join("main.tscn"),
            "[gd_scene]\n[ext_resource type=\"Texture2D\" path=\"res://art/hero.png\" id=\"1\"]\n[ext_resource type=\"Texture2D\" path=\"res://art/missing.png\" id=\"2\"]\n",
        )
        .unwrap();
        std::fs::write(
            game.join("hud.gd"),
            "var f = \"res://art/frame_%d.png\"\nconst C = \"res://local/config.json\"\n",
        )
        .unwrap();
        std::fs::write(base.join(".gitignore"), "/game/local/\n").unwrap();
        let base = dunce::canonicalize(&base).unwrap();
        let game = base.join("game");
        let (ok, out) = run(&base, &game);
        assert!(!ok);
        assert!(
            out.contains("game/main.tscn:3: missing res://art/missing.png"),
            "{out}"
        );
        assert!(
            out.starts_with("1 of 4"),
            "dynamic paths are skipped: {out}"
        );
        assert!(
            out.contains("ignored by git") && out.contains("game/hud.gd:2"),
            "git-ignored local files are reported apart: {out}"
        );
        let links = crate::projecttools::source_links(&base, &base, &out);
        assert!(links
            .iter()
            .any(|l| l.file == "game/main.tscn" && l.line == 3));
        let _ = std::fs::remove_dir_all(&base);
    }
}
