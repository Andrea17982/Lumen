//! Impact snapshots survive edits/deletions and accompany worker handoffs and checks.
use crate::{
    codegraph::{Graph, Impact},
    models::AccessMode,
    Core,
};
use std::{path::Path, sync::Arc};

pub async fn snapshot(
    core: &Core,
    root: &Path,
    access: AccessMode,
) -> Result<Option<Arc<Graph>>, String> {
    if access == AccessMode::ReadOnly
        || !core
            .settings
            .lock()
            .map(|s| s.repo_context)
            .unwrap_or(false)
    {
        return Ok(None);
    }
    let (data, root) = (core.root.clone(), root.to_path_buf());
    tauri::async_runtime::spawn_blocking(move || Some(crate::codegraph::graph(&data, &root)))
        .await
        .map_err(|e| e.to_string())
}

fn merge(mut current: Impact, before: Option<Impact>) -> Impact {
    if let Some(before) = before {
        current.truncated |= before.truncated;
        current
            .unknown
            .retain(|target| before.unknown.contains(target));
        for file in before.files {
            if let Some(existing) = current.files.iter_mut().find(|f| f.path == file.path) {
                existing.distance = existing.distance.min(file.distance);
            } else {
                current.files.push(file);
            }
        }
        for contract in before.contracts {
            if !current.contracts.contains(&contract) {
                current.contracts.push(contract);
            }
        }
    }
    current.files.sort_by(|a, b| {
        a.distance
            .cmp(&b.distance)
            .then(b.test.cmp(&a.test))
            .then(a.path.cmp(&b.path))
    });
    if current.files.len() > 120 {
        current.truncated = true;
        current.files.truncate(120);
    }
    current.contracts.truncate(24);
    current
}

pub async fn analyse(
    core: &Core,
    root: &Path,
    targets: &[String],
    before: Option<Arc<Graph>>,
) -> Result<Option<Impact>, String> {
    if !core
        .settings
        .lock()
        .map(|s| s.repo_context)
        .unwrap_or(false)
        || targets.is_empty()
    {
        return Ok(None);
    }
    let (data, root, targets) = (core.root.clone(), root.to_path_buf(), targets.to_vec());
    tauri::async_runtime::spawn_blocking(move || {
        let current = crate::codegraph::graph(&data, &root).impact_details(&targets, 3);
        Some(merge(
            current,
            before.map(|g| g.impact_details(&targets, 3)),
        ))
    })
    .await
    .map_err(|e| e.to_string())
}

pub fn handoff(root: &Path, report: &Impact) -> String {
    let paths: Vec<_> = report.files.iter().map(|f| f.path.clone()).collect();
    let checks = crate::validation::checks(root, &paths);
    format!("<lumen-impact>\n{}\nRelevant headless checks (run in their listed project directory):\n{}\nInspect affected contracts and tests as part of implementation. Respect assigned file ownership; report necessary changes in another agent's files to the orchestrator. This is execution context, not a request for another planning phase.\n</lumen-impact>\n",report.text(),checks.iter().map(|c|format!("- {}: {}",c.cwd,c.command)).collect::<Vec<_>>().join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Fixture;

    #[tokio::test]
    async fn deleted_shared_code_keeps_its_callers_and_tests_in_the_handoff() {
        let fixture = Fixture::new();
        let root = fixture.directory("project");
        std::fs::create_dir_all(root.join("tests")).unwrap();
        std::fs::write(
            root.join("package.json"),
            r#"{"scripts":{"test":"vitest run","build":"tsc"}}"#,
        )
        .unwrap();
        std::fs::write(
            root.join("shared.ts"),
            "export function SharedValue(n: number): string { return `${n}`; }\n",
        )
        .unwrap();
        std::fs::write(root.join("consumer.ts"),"import { SharedValue } from './shared';\nexport function ConsumerValue() { return SharedValue(1); }\n").unwrap();
        std::fs::write(root.join("tests/consumer.test.ts"),"import { ConsumerValue } from '../consumer';\nexport function ContractTest() { return ConsumerValue(); }\n").unwrap();
        let before = snapshot(&fixture.core, &root, AccessMode::Autopilot)
            .await
            .unwrap();
        assert!(before.is_some());
        std::fs::write(
            root.join("shared.ts"),
            "export function SharedValue(n: number): number { return n; }\n",
        )
        .unwrap();
        let edited = analyse(&fixture.core, &root, &["shared.ts".into()], before.clone())
            .await
            .unwrap()
            .unwrap();
        assert!(edited.contracts.iter().any(|c| c.contains("): string")));
        assert!(edited.contracts.iter().any(|c| c.contains("): number")));
        std::fs::remove_file(root.join("shared.ts")).unwrap();
        let report = analyse(
            &fixture.core,
            &root,
            &["shared.ts".into(), "unknown.generated".into()],
            before,
        )
        .await
        .unwrap()
        .unwrap();
        assert!(report
            .files
            .iter()
            .any(|f| f.path == "consumer.ts" && f.distance == 1));
        assert!(report
            .files
            .iter()
            .any(|f| f.path == "tests/consumer.test.ts" && f.test));
        assert_eq!(report.unknown, vec!["unknown.generated"]);
        assert!(report.contracts.iter().any(|c| c.contains("SharedValue")));
        let prompt = handoff(&root, &report);
        assert!(prompt.contains("npm run test") && prompt.contains("consumer.test.ts"));
        assert!(prompt.contains("not a request for another planning phase"));
        let task = crate::orchestra::Subtask {
            writes: true,
            files: vec!["shared.ts".into()],
            impact: prompt,
            ..Default::default()
        };
        let worker =
            crate::orchestra::subtask_prompt(&crate::orchestra::State::default(), &task, "", false);
        assert!(
            worker.contains("consumer.test.ts")
                && worker.contains("SharedValue")
                && worker.contains("npm run test")
        );
    }
}
