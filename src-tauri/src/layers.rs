//! Cross-layer change review: uncommitted files grouped by layer (client, domain, backend,
//! database, content, simulation…) with the project checks that cover each layer. Read-only;
//! applying or undoing changes stays with the existing approval modes and Rewind.
use serde::Serialize;
use std::path::Path;

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChangedFile {
    pub path: String,
    /// Git's two-letter status, such as `M`, `A`, `D` or `??`.
    pub status: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Layer {
    pub name: String,
    pub files: Vec<ChangedFile>,
    /// Ids of listed checks that cover these files.
    pub checks: Vec<String>,
}

/// `.lumen/layers.json` = [{"name": "Client", "paths": ["game/"]}]; the first match wins and
/// built-in rules apply to everything else.
fn custom_rules(root: &Path) -> Vec<(String, Vec<String>)> {
    std::fs::read_to_string(root.join(".lumen/layers.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<Vec<serde_json::Value>>(&t).ok())
        .unwrap_or_default()
        .iter()
        .filter_map(|v| {
            let name = v["name"].as_str()?.to_string();
            let paths = v["paths"]
                .as_array()?
                .iter()
                .filter_map(|p| p.as_str().map(|s| s.replace('\\', "/")))
                .collect();
            Some((name, paths))
        })
        .collect()
}

pub fn classify(rel: &str, godot_dirs: &[String], rules: &[(String, Vec<String>)]) -> String {
    let lower = rel.to_lowercase();
    if let Some((name, _)) = rules
        .iter()
        .find(|(_, paths)| paths.iter().any(|p| rel.starts_with(p.as_str())))
    {
        return name.clone();
    }
    let has = |dir: &str| lower.split('/').any(|part| part == dir);
    let name = if godot_dirs
        .iter()
        .any(|d| d.is_empty() || rel.starts_with(&format!("{d}/")))
    {
        "Client"
    } else if lower.ends_with(".sql") || has("migrations") {
        "Database"
    } else if has("simulations") || has("simulation") || has("sim") {
        "Simulation"
    } else if has("content") || has("catalog") || has("data") {
        "Content & catalog"
    } else if has("backend") || has("functions") || has("server") || has("api") {
        "Backend"
    } else if has("tests") || has("test") {
        "Tests"
    } else if lower.ends_with(".md") || has("docs") {
        "Docs"
    } else if has("src") || has("lib") {
        "Domain"
    } else {
        "Other"
    };
    name.to_string()
}

const ORDER: &[&str] = &[
    "Client",
    "Domain",
    "Backend",
    "Database",
    "Content & catalog",
    "Simulation",
    "Tests",
    "Docs",
    "Other",
];

pub fn group(
    root: &Path,
    changed: Vec<(String, String)>,
    checks: &[crate::projecttools::CheckEntry],
) -> Vec<Layer> {
    let godot_dirs = crate::projecttools::godot_project_dirs(root);
    let rules = custom_rules(root);
    let mut layers: Vec<Layer> = vec![];
    for (status, path) in changed {
        let name = classify(&path, &godot_dirs, &rules);
        let file = ChangedFile { path, status };
        match layers.iter_mut().find(|l| l.name == name) {
            Some(l) => l.files.push(file),
            None => layers.push(Layer {
                name,
                files: vec![file],
                checks: vec![],
            }),
        }
    }
    for layer in &mut layers {
        layer.checks = checks
            .iter()
            .filter(|c| {
                c.layers.iter().any(|l| l == &layer.name)
                    || (c.cwd != "."
                        && layer
                            .files
                            .iter()
                            .any(|f| f.path.starts_with(&format!("{}/", c.cwd))))
            })
            .map(|c| c.id.clone())
            .collect();
    }
    let rank = |n: &str| ORDER.iter().position(|o| *o == n).unwrap_or(ORDER.len());
    layers.sort_by_key(|l| (rank(&l.name), l.name.clone()));
    layers
}

#[tauri::command]
pub async fn change_layers(folder: String) -> Result<Vec<Layer>, String> {
    let root = dunce::canonicalize(&folder).map_err(|_| "Choose an existing folder")?;
    let top = crate::gitops::toplevel(&root)
        .await
        .ok_or("Change review needs a git repository in this folder")?;
    let prefix = root
        .strip_prefix(&top)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default();
    let changed = crate::gitops::changed_paths(&root)
        .await?
        .into_iter()
        .filter_map(|(status, path)| {
            // Paths are relative to the repository root; keep those inside this folder.
            let rel = if prefix.is_empty() {
                path
            } else {
                path.strip_prefix(&format!("{prefix}/"))?.to_string()
            };
            (!crate::context::is_secret(&rel)).then_some((status, rel))
        })
        .collect();
    let base = root.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let checks = crate::projecttools::list_checks(&base);
        group(&base, changed, &checks)
    })
    .await
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_a_gameplay_change_by_layer() {
        let godot = vec!["game".to_string()];
        let rules = vec![("Shared domain".to_string(), vec!["src/Shared/".to_string()])];
        let cases = [
            ("game/scripts/Battle.cs", "Client"),
            ("game/scenes/battle.tscn", "Client"),
            ("src/RodentHeroes.Application/BattleCache.cs", "Domain"),
            ("src/Shared/Ids.cs", "Shared domain"),
            ("backend/domain/src/battle.ts", "Backend"),
            ("supabase/migrations/002_battles.sql", "Database"),
            ("content/catalog/heroes.json", "Content & catalog"),
            ("simulations/d0engine.py", "Simulation"),
            ("tests/RodentHeroes.Client.Tests/BattleTests.cs", "Tests"),
            ("README.md", "Docs"),
        ];
        for (path, layer) in cases {
            assert_eq!(classify(path, &godot, &rules), layer, "{path}");
        }
    }

    /// `LUMEN_LAYERS_PROJECT=<folder>` prints the grouping of a real working tree.
    #[tokio::test]
    #[ignore]
    async fn live_change_layers() {
        let folder = std::env::var("LUMEN_LAYERS_PROJECT").expect("LUMEN_LAYERS_PROJECT");
        for layer in change_layers(folder).await.unwrap() {
            let files: Vec<&str> = layer.files.iter().map(|f| f.path.as_str()).collect();
            println!("{}: {:?} checks {:?}", layer.name, files, layer.checks);
        }
    }
}
