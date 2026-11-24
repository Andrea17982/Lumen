//! Bug fixes in Auto: a test that fails on the old code first, then the fix. Lumen asks the
//! agent to work that way and checks it itself: with the turn's source changes set aside,
//! the new tests must fail; with the fix back in place, they must pass.
use crate::{
    gitops,
    models::{AccessMode, Message, RunOptions},
    projecttools::CheckEntry,
};
use std::path::{Path, PathBuf};
use tokio::sync::watch;

/// Told to the agent on a bug-fix turn.
pub(crate) const INSTRUCTION: &str = "Lumen: this request is a bug fix, so work test-first. 1) Write or extend an automated test that reproduces the bug, in a test file of its own where the project's conventions allow. 2) Run it and see it fail for the reason the bug describes. 3) Fix the code with minimal changes. 4) Run the test again and see it pass. Keep the test. Lumen checks this by running your new tests against the code as it was before your fix.\n";

/// Keyword guess, used only when the routing model is unavailable.
pub(crate) fn looks_like_bugfix(text: &str) -> bool {
    let t = text.to_lowercase();
    [
        "bug",
        "crash",
        "regression",
        "broken",
        "doesn't work",
        "does not work",
        "not working",
        "wrong result",
        "non funziona",
        "regressione",
        "si blocca",
        "risultato sbagliato",
    ]
    .iter()
    .any(|k| t.contains(k))
}

/// The current request is an Auto bug fix that writes files. Lumen's own pull request
/// corrections are excluded: CI failures are not bugs a new test should reproduce.
pub(crate) fn wanted(messages: &[Message], options: &RunOptions) -> bool {
    if options.mode != "auto"
        || !matches!(
            options.access(),
            AccessMode::FileEdits | AccessMode::Autopilot
        )
    {
        return false;
    }
    messages
        .iter()
        .rev()
        .find(|m| m.role == "user")
        .filter(|m| !m.text.starts_with("Lumen · "))
        .and_then(|m| m.activity.iter().find(|a| a.kind == "route"))
        .and_then(|a| serde_json::from_str::<serde_json::Value>(&a.text).ok())
        .is_some_and(|v| v["bugfix"] == true)
}

/// Test code by the usual layouts: `tests/`, `__tests__/`, `*.test.*`, `*_test.*`,
/// `test_*.py`, `FooTest.java` and the like.
pub(crate) fn is_test(path: &str) -> bool {
    let path = path.replace('\\', "/");
    let mut parts: Vec<&str> = path.split('/').collect();
    let name = parts.pop().unwrap_or_default();
    if parts.iter().any(|d| {
        matches!(
            d.to_lowercase().as_str(),
            "test" | "tests" | "__tests__" | "spec" | "specs"
        )
    }) {
        return true;
    }
    let stem = name.split('.').next().unwrap_or(name);
    let lower = name.to_lowercase();
    lower.contains(".test.")
        || lower.contains(".spec.")
        || stem.ends_with("_test")
        || stem.ends_with("_spec")
        || lower.starts_with("test_")
        || stem.ends_with("Test")
        || stem.ends_with("Tests")
}

fn is_code(path: &str) -> bool {
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    [
        "rs", "ts", "tsx", "js", "jsx", "mjs", "cjs", "mts", "cts", "py", "go", "java", "kt",
        "cs", "rb", "php", "swift", "c", "cc", "cpp", "h", "hpp", "gd", "lua", "dart", "scala",
        "ex", "exs", "vue", "svelte",
    ]
    .contains(&ext.as_str())
}

#[derive(Debug, PartialEq)]
pub(crate) enum Outcome {
    /// The new tests failed on the old code and pass with the fix.
    Verified(String),
    /// Evidence is missing; the text is what the agent should do next.
    Missing(String),
    /// Lumen cannot check this project or change this way; nothing is held.
    Unavailable(String),
}

/// The tests checks Lumen runs for the changed test files.
fn test_checks(workspace: &Path, tests: &[String]) -> Vec<CheckEntry> {
    let all = crate::validation::checks(workspace, tests);
    let tests: Vec<CheckEntry> = all
        .iter()
        .filter(|c| c.label.to_lowercase().contains("test"))
        .cloned()
        .collect();
    if tests.is_empty() {
        all.into_iter().filter(|c| c.id == "verify").collect()
    } else {
        tests
    }
}

/// Puts the set-aside files back even when the check is cancelled or panics.
struct Restore(Vec<(PathBuf, Option<Vec<u8>>)>);

impl Restore {
    fn now(&mut self) {
        for (path, bytes) in self.0.drain(..) {
            match bytes {
                Some(b) => {
                    if let Some(parent) = path.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    let _ = std::fs::write(&path, b);
                }
                None => {
                    let _ = std::fs::remove_file(&path);
                }
            }
        }
    }
}

impl Drop for Restore {
    fn drop(&mut self) {
        self.now();
    }
}

async fn run_checks(
    workspace: &Path,
    checks: &[CheckEntry],
    cancel: &watch::Receiver<bool>,
) -> Result<(bool, String), String> {
    let mut ok = true;
    let mut output = String::new();
    for check in checks {
        let r = crate::projecttools::execute_check(workspace, check, cancel.clone()).await?;
        ok &= r.ok;
        if !r.ok {
            let tail: String = r
                .output
                .chars()
                .rev()
                .take(3000)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            output.push_str(&format!("{} ({})\n{tail}\n", check.label, check.command));
        }
    }
    Ok((ok, output))
}

/// Runs the new tests without and then with the turn's source changes. `tree` is the
/// checkpoint taken before the turn (paths relative to `top`); `files` are the turn's
/// changes relative to `workspace`.
pub(crate) async fn verify(
    top: &Path,
    workspace: &Path,
    tree: &str,
    files: &[String],
    cancel: watch::Receiver<bool>,
) -> Result<Outcome, String> {
    let tests: Vec<String> = files.iter().filter(|f| is_test(f)).cloned().collect();
    let sources: Vec<String> = files
        .iter()
        .filter(|f| !is_test(f) && is_code(f))
        .cloned()
        .collect();
    if tests.is_empty() {
        let diff = gitops::diff_since(top, tree, 200_000)
            .await
            .unwrap_or_default();
        let inline = diff.lines().any(|l| {
            l.starts_with('+')
                && [
                    "#[test]",
                    "#[tokio::test]",
                    "def test_",
                    "func Test",
                    "@Test",
                    "it(",
                    "test(",
                ]
                .iter()
                .any(|m| l.contains(m))
        });
        return Ok(if inline {
            Outcome::Unavailable(
                "The new test is in the same file as the fix, so Lumen could not run it against the old code.".into(),
            )
        } else {
            Outcome::Missing(
                "Lumen found no test that reproduces the bug. Add an automated test, in a test file of its own, that fails on the code as it was before your fix and passes with it. Keep the fix.".into(),
            )
        });
    }
    if sources.is_empty() {
        return Ok(Outcome::Unavailable(
            "Only test files changed, so there is no fix to set aside.".into(),
        ));
    }
    let checks = test_checks(workspace, &tests);
    if checks.is_empty() {
        return Ok(Outcome::Unavailable(format!(
            "No test command was found for {}.",
            tests.join(", ")
        )));
    }
    let prefix = workspace
        .strip_prefix(top)
        .unwrap_or(Path::new(""))
        .to_path_buf();
    // Set the fix aside: each changed source file goes back to its state before the turn.
    let mut restore = Restore(vec![]);
    for file in &sources {
        let path = workspace.join(file);
        let key = prefix.join(file).to_string_lossy().replace('\\', "/");
        let before = gitops::run(top, &["cat-file", "-p", &format!("{tree}:{key}")], None)
            .await
            .ok();
        let now = std::fs::read(&path).ok();
        restore.0.push((path.clone(), now));
        match before {
            Some(text) => std::fs::write(&path, text).map_err(|e| e.to_string())?,
            None => {
                let _ = std::fs::remove_file(&path);
            }
        }
    }
    let red = run_checks(workspace, &checks, &cancel).await;
    restore.now();
    let (passed_without_fix, _) = red?;
    if passed_without_fix {
        return Ok(Outcome::Missing(format!(
            "Lumen ran your new tests ({}) on the code as it was before your fix, and they passed: they do not reproduce the bug. Change them so they fail without the fix and pass with it. Keep the fix.",
            tests.join(", ")
        )));
    }
    let (green, output) = run_checks(workspace, &checks, &cancel).await?;
    Ok(if green {
        Outcome::Verified(format!(
            "{} failed without the fix and pass with it.",
            tests.join(", ")
        ))
    } else {
        Outcome::Missing(format!(
            "The new tests fail with your fix too. Correct the fix (not the test, unless the test itself is wrong).\n{output}"
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_files_and_bug_reports() {
        for t in [
            "tests/parse.rs",
            "src/__tests__/a.ts",
            "web/a.test.tsx",
            "pkg/a_test.go",
            "test_mod.py",
            "src/test/java/FooTest.java",
            "x.spec.js",
        ] {
            assert!(is_test(t), "{t}");
        }
        for s in ["src/parse.rs", "src/testing_utils.rs", "src/contest.ts", "README.md"] {
            assert!(!is_test(s), "{s}");
        }
        assert!(looks_like_bugfix("The parser crashes on empty input"));
        assert!(looks_like_bugfix("Il login non funziona più"));
        assert!(!looks_like_bugfix("Add a dark theme"));
    }

    /// A JavaScript project whose `npm test` runs node's test runner: a test that catches
    /// the bug is verified, one that passes on the old code is sent back, and a fix without
    /// a test asks for one. The fix is back in place afterwards.
    #[tokio::test]
    async fn red_then_green_on_a_real_project() {
        if gitops::toplevel(&std::env::temp_dir()).await.is_some()
            || crate::proc::command("node")
                .arg("--version")
                .output()
                .await
                .is_err()
        {
            return;
        }
        let f = crate::test_support::Fixture::new();
        let p = f.directory("redgreen");
        let git = |args: &'static [&'static str]| {
            let p = p.clone();
            async move { gitops::run(&p, args, None).await.unwrap() }
        };
        git(&["init", "-q", "-b", "main"]).await;
        std::fs::write(
            p.join("package.json"),
            r#"{"type":"module","scripts":{"test":"node --test"}}"#,
        )
        .unwrap();
        std::fs::create_dir_all(p.join("src")).unwrap();
        std::fs::create_dir_all(p.join("test")).unwrap();
        let buggy = "export const add = (a, b) => a - b;\n";
        std::fs::write(p.join("src/math.js"), buggy).unwrap();
        git(&["add", "."]).await;
        git(&["commit", "-qm", "init"]).await;
        let tree = gitops::write_tree(&p).await.unwrap();

        // The fix and a test that catches the bug.
        let fixed = "export const add = (a, b) => a + b;\n";
        std::fs::write(p.join("src/math.js"), fixed).unwrap();
        std::fs::write(
            p.join("test/math.test.js"),
            "import { test } from 'node:test';\nimport assert from 'node:assert';\nimport { add } from '../src/math.js';\ntest('add', () => assert.equal(add(2, 3), 5));\n",
        )
        .unwrap();
        let files = vec!["src/math.js".to_string(), "test/math.test.js".to_string()];
        let rx = watch::channel(false).1;
        let outcome = verify(&p, &p, &tree, &files, rx.clone()).await.unwrap();
        assert!(matches!(outcome, Outcome::Verified(_)), "{outcome:?}");
        assert_eq!(std::fs::read_to_string(p.join("src/math.js")).unwrap(), fixed);

        // A test that passes on the old code does not reproduce the bug.
        std::fs::write(
            p.join("test/math.test.js"),
            "import { test } from 'node:test';\ntest('nothing', () => {});\n",
        )
        .unwrap();
        let outcome = verify(&p, &p, &tree, &files, rx.clone()).await.unwrap();
        assert!(
            matches!(&outcome, Outcome::Missing(t) if t.contains("do not reproduce")),
            "{outcome:?}"
        );
        assert_eq!(std::fs::read_to_string(p.join("src/math.js")).unwrap(), fixed);

        // A fix without any test asks for one.
        let outcome = verify(&p, &p, &tree, &files[..1], rx).await.unwrap();
        assert!(
            matches!(&outcome, Outcome::Missing(t) if t.contains("no test")),
            "{outcome:?}"
        );
    }
}
