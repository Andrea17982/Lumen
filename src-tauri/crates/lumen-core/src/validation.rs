//! Evidence-driven end-of-turn checks, shared by ordinary and Orchestra turns.
use crate::{diagnostics::Report, models::*, projecttools::CheckEntry, Core};
use std::path::{Path, PathBuf};
use tokio::sync::watch;

pub async fn changed_files(core: &Core, s: &Session, out: &Message) -> Result<Vec<String>, String> {
    let root = dunce::canonicalize(&s.workspace).map_err(|e| e.to_string())?;
    if let Some(top) = crate::gitops::toplevel(&root).await {
        let checkpoint = core
            .store
            .messages(&s.id)?
            .iter()
            .rev()
            .find(|m| m.role == "user")
            .and_then(crate::turns::checkpoint_of);
        if let Some(tree) = checkpoint {
            let prefix = root.strip_prefix(&top).map_err(|e| e.to_string())?;
            return Ok(crate::gitops::paths_since(&top, &tree)
                .await?
                .into_iter()
                .filter_map(|p| {
                    Path::new(&p)
                        .strip_prefix(prefix)
                        .ok()
                        .map(|p| p.to_string_lossy().replace('\\', "/"))
                })
                .collect());
        }
    }
    let mut files = vec![];
    for activity in out.activity.iter().filter(|a| a.kind == "file") {
        for line in activity.text.lines() {
            let path = Path::new(line.trim());
            let relative = if path.is_absolute() {
                path.strip_prefix(&root).ok()
            } else {
                Some(path)
            };
            if let Some(path) = relative.filter(|p| !p.as_os_str().is_empty()) {
                let text = path.to_string_lossy().replace('\\', "/");
                if !crate::context::is_secret(&text) && crate::context::jail(&root, &text).is_ok() {
                    files.push(text);
                }
            }
        }
    }
    files.sort();
    files.dedup();
    Ok(files)
}

fn nearest(root: &Path, file: &str, marker: &str) -> Option<PathBuf> {
    root.join(file)
        .ancestors()
        .skip(1)
        .take_while(|p| p.starts_with(root))
        .find(|p| p.join(marker).is_file())
        .map(Path::to_path_buf)
}

pub fn checks(root: &Path, files: &[String]) -> Vec<CheckEntry> {
    let notes = crate::context::read_context(root);
    if notes.lines().any(|l| l.trim() == "verify: off") {
        return vec![];
    }
    let available = crate::projecttools::list_checks(root);
    let mut selected = vec![];
    if notes.lines().any(|l| l.trim().starts_with("verify:")) {
        selected.extend(available.iter().filter(|c| c.id == "verify").cloned());
    }
    let mut add = |label: &str, command: &str, cwd: &Path| {
        let cwd = cwd
            .strip_prefix(root)
            .unwrap_or(Path::new("."))
            .to_string_lossy()
            .replace('\\', "/");
        let cwd = if cwd.is_empty() { ".".into() } else { cwd };
        if !selected
            .iter()
            .any(|c: &CheckEntry| c.command == command && c.cwd == cwd)
        {
            selected.push(CheckEntry {
                id: format!("automatic:{cwd}:{label}"),
                label: label.into(),
                command: command.into(),
                cwd,
                source: "Detected from changed files".into(),
                playtest: false,
                layers: vec![],
            });
        }
    };
    for file in files {
        let extension = Path::new(file)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        if extension == "rs" || file.ends_with("Cargo.toml") {
            if let Some(dir) = nearest(root, file, "Cargo.toml") {
                add("Rust compile", "cargo check --quiet", &dir);
                add("Rust tests", "cargo test --quiet", &dir);
            }
        }
        if ["ts", "tsx", "js", "jsx", "mjs", "cjs", "mts", "cts"].contains(&extension)
            || file.ends_with("package.json")
        {
            if let Some(dir) = nearest(root, file, "package.json") {
                if let Ok(text) = std::fs::read_to_string(dir.join("package.json")) {
                    if let Ok(package) = serde_json::from_str::<serde_json::Value>(&text) {
                        if let Some(scripts) = package["scripts"].as_object() {
                            for choices in [
                                &["typecheck", "check", "build"][..],
                                &["test:unit", "test"][..],
                            ] {
                                if let Some(name) =
                                    choices.iter().find(|name| scripts.contains_key(**name))
                                {
                                    add(
                                        &format!("JavaScript {name}"),
                                        &format!("npm run {name}"),
                                        &dir,
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
        if extension == "cs" {
            for dir in root
                .join(file)
                .ancestors()
                .skip(1)
                .take_while(|p| p.starts_with(root))
            {
                let project = std::fs::read_dir(dir).ok().is_some_and(|es| {
                    es.flatten().any(|e| {
                        e.path()
                            .extension()
                            .is_some_and(|x| x == "csproj" || x == "sln")
                    })
                });
                if project {
                    add("C# compile", "dotnet build --nologo", dir);
                    break;
                }
            }
        }
        if extension == "go" || file.ends_with("go.mod") {
            if let Some(dir) = nearest(root, file, "go.mod") {
                add("Go compile", "go build ./...", &dir);
                add("Go tests", "go test ./...", &dir);
            }
        }
    }
    // Only explicitly opted-in custom headless checks run automatically.
    if let Ok(text) = std::fs::read_to_string(root.join(".lumen/checks.json")) {
        if let Ok(items) = serde_json::from_str::<Vec<serde_json::Value>>(&text) {
            for item in items.iter().filter(|i| i["automatic"] == true) {
                let id = format!(
                    "custom:{}:{}",
                    item["cwd"].as_str().unwrap_or("."),
                    item["name"].as_str().unwrap_or("")
                );
                selected.extend(
                    available
                        .iter()
                        .filter(|c| c.id == id && !c.playtest)
                        .cloned(),
                );
            }
        }
    }
    for check in available.iter().filter(|c| {
        c.id.starts_with("godot-assets:")
            && files
                .iter()
                .any(|f| f.ends_with(".gd") || f.ends_with(".tscn") || f.ends_with(".tres"))
    }) {
        selected.push(check.clone());
    }
    selected.sort_by(|a, b| a.cwd.cmp(&b.cwd).then(a.command.cmp(&b.command)));
    if selected.is_empty() {
        selected.extend(available.into_iter().filter(|c| c.id == "verify"));
    }
    selected.dedup_by(|a, b| a.cwd == b.cwd && a.command == b.command);
    selected
}

pub async fn run(
    root: &Path,
    files: &[String],
    cancel: watch::Receiver<bool>,
) -> Result<Vec<Report>, String> {
    if *cancel.borrow() {
        return Err("Cancelled".into());
    }
    let mut reports = crate::diagnostics::run(root, files, cancel.clone()).await?;
    let selected = checks(root, files);
    if selected.len() > 8 {
        reports.push(Report::unavailable(
            "Additional project checks".into(),
            format!(
                "{} checks exceed the automatic budget of 8 and were not run",
                selected.len() - 8
            ),
        ));
    }
    for check in selected.into_iter().take(8) {
        let result = crate::projecttools::execute_check(root, &check, cancel.clone()).await;
        match result {
            Ok(result) => reports.push(Report {
                label: format!("{} ({})", check.label, check.cwd),
                status: if result.ok { "passed" } else { "failed" }.into(),
                detail: format!("{}\n{}", check.command, result.output),
                diagnostics: vec![],
            }),
            Err(e) if e == "Cancelled" => return Err(e),
            Err(e) => reports.push(Report::unavailable(check.label, e)),
        }
    }
    if reports.is_empty() {
        reports.push(Report::unavailable(
            "Project validation".into(),
            "No applicable language server or headless project check was found".into(),
        ));
    }
    Ok(reports)
}

pub fn failure_text(reports: &[Report]) -> String {
    reports
        .iter()
        .filter(|r| r.status == "failed")
        .map(|r| {
            format!("{}\n{}\n{}", r.label, r.errors(), r.detail)
                .chars()
                .take(8000)
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n\n")
        .chars()
        .take(32_000)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Fixture;

    #[test]
    fn changed_languages_select_their_own_nested_checks_and_never_playtests() {
        let fixture = Fixture::new();
        let backend = fixture.directory("backend/src");
        std::fs::write(
            backend.parent().unwrap().join("Cargo.toml"),
            "[package]\nname='example'\nversion='0.1.0'\n",
        )
        .unwrap();
        let web = fixture.directory("web/src");
        std::fs::write(web.parent().unwrap().join("package.json"), r#"{"scripts":{"build":"tsc","test":"vitest run","deploy":"danger","test:watch":"vitest"}}"#).unwrap();
        let config = fixture.directory(".lumen");
        std::fs::write(config.join("checks.json"), r#"[{"name":"Play","command":"never","automatic":true,"playtest":true},{"name":"Manual","command":"never"}]"#).unwrap();
        let selected = checks(
            &fixture.root,
            &["backend/src/lib.rs".into(), "web/src/index.ts".into()],
        );
        assert!(selected
            .iter()
            .any(|c| c.command == "cargo check --quiet" && c.cwd == "backend"));
        assert!(selected
            .iter()
            .any(|c| c.command == "cargo test --quiet" && c.cwd == "backend"));
        assert!(selected
            .iter()
            .any(|c| c.command == "npm run test" && c.cwd == "web"));
        assert!(!selected.iter().any(|c| c.command.contains("danger")
            || c.command.contains("never")
            || c.command.contains("watch")));
        crate::context::write_context(&fixture.root, "verify: off").unwrap();
        assert!(checks(&fixture.root, &["backend/src/lib.rs".into()]).is_empty());
    }

    #[tokio::test]
    async fn actual_failing_check_becomes_passed_only_after_the_file_is_corrected() {
        let fixture = Fixture::new();
        let config = fixture.directory(".lumen");
        std::fs::write(config.join("diagnostics.json"), r#"{"servers":[]}"#).unwrap();
        #[cfg(windows)]
        let command = "if exist broken.flag (echo expected failure & exit /b 1) else (echo passed & exit /b 0)";
        #[cfg(unix)]
        let command =
            "if [ -f broken.flag ]; then echo 'expected failure'; exit 1; else echo passed; fi";
        std::fs::write(
            config.join("checks.json"),
            serde_json::json!([{"name":"Regression","command":command,"automatic":true}])
                .to_string(),
        )
        .unwrap();
        std::fs::write(fixture.root.join("broken.flag"), "broken").unwrap();
        let (_tx, rx) = watch::channel(false);
        let failed = run(&fixture.root, &["code.txt".into()], rx.clone())
            .await
            .unwrap();
        assert!(failure_text(&failed).contains("expected failure"));
        assert_eq!(failed[0].status, "failed");
        std::fs::remove_file(fixture.root.join("broken.flag")).unwrap();
        let passed = run(&fixture.root, &["code.txt".into()], rx).await.unwrap();
        assert_eq!(passed[0].status, "passed");
        assert!(failure_text(&passed).is_empty());
    }

    #[tokio::test]
    async fn checkpoint_detects_writes_without_provider_file_events_and_excludes_existing_work() {
        let fixture = Fixture::new();
        let root = fixture.directory("project");
        crate::gitops::init(&root).await.unwrap();
        std::fs::write(root.join("preexisting.txt"), "preserve").unwrap();
        std::fs::write(root.join("changed.txt"), "before").unwrap();
        let session = fixture
            .core
            .store
            .create(root.to_string_lossy().into())
            .unwrap();
        let user = Message {
            id: id(),
            role: "user".into(),
            text: "Fix changed.txt".into(),
            activity: vec![Activity {
                kind: "checkpoint".into(),
                text: crate::gitops::write_tree(&root).await.unwrap(),
            }],
        };
        fixture.core.store.save_message(&session.id, &user).unwrap();
        std::fs::write(root.join("changed.txt"), "after").unwrap();
        std::fs::write(root.join("new.txt"), "new").unwrap();
        let out = Message {
            id: id(),
            role: "assistant".into(),
            text: "done".into(),
            activity: vec![],
        };
        let mut paths = changed_files(&fixture.core, &session, &out).await.unwrap();
        paths.sort();
        assert_eq!(paths, vec!["changed.txt", "new.txt"]);
    }
}
