//! Repository map: a local, incremental symbol index for C#, GDScript, TypeScript/JavaScript, SQL
//! and Godot scenes/resources. Definitions are found with per-language patterns, not a full
//! parser, so it is a map of where things are declared, not a call graph. The index lives in
//! Lumen's data folder, never in the project, and is refreshed by file size and modification
//! time: edits re-read a file, deletions drop it, renames are a deletion plus a new file.
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const MAX_FILES: usize = 20_000;
const MAX_FILE_BYTES: u64 = 512 * 1024;
const BUILD_BUDGET: Duration = Duration::from_secs(8);
const MAX_SYMBOLS_PER_FILE: usize = 400;
/// Upper bound of the block added to a prompt.
pub const CONTEXT_CHARS: usize = 2_400;
const CONTEXT_SYMBOLS: usize = 12;
const VERSION: u32 = 1;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Symbol {
    pub name: String,
    pub kind: String,
    pub file: String,
    pub line: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
struct FileRecord {
    size: u64,
    modified: u64,
    symbols: Vec<Symbol>,
    /// `res://` paths a scene or resource depends on, resolved to project-relative paths.
    uses: Vec<String>,
}

#[derive(Serialize, Deserialize, Default)]
struct Index {
    version: u32,
    files: BTreeMap<String, FileRecord>,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct IndexStats {
    pub files: usize,
    pub symbols: usize,
    /// Files read again in this refresh (new or changed).
    pub refreshed: usize,
    pub removed: usize,
    pub millis: u64,
    /// The file or time limit stopped the walk; the map is incomplete.
    pub partial: bool,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Reference {
    pub file: String,
    pub line: u32,
    pub text: String,
}

fn language(rel: &str) -> Option<&'static str> {
    let ext = rel.rsplit('.').next()?.to_lowercase();
    Some(match ext.as_str() {
        "cs" => "cs",
        "gd" => "gd",
        "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" | "mts" => "ts",
        "sql" => "sql",
        "tscn" | "tres" => "godot",
        _ => return None,
    })
}

struct Patterns {
    cs_type: Regex,
    cs_method: Regex,
    gd: Regex,
    ts: Regex,
    sql: Regex,
    node: Regex,
    resource: Regex,
}

fn patterns() -> &'static Patterns {
    static P: OnceLock<Patterns> = OnceLock::new();
    P.get_or_init(|| Patterns {
        cs_type: Regex::new(r"\b(class|struct|interface|enum|record)\s+([A-Za-z_]\w*)").unwrap(),
        cs_method: Regex::new(
            r"^\s*(?:(?:public|private|protected|internal|static|override|virtual|async|sealed|abstract|partial|extern|new)\s+)+[\w<>\[\],.?]+\s+([A-Za-z_]\w*)\s*(?:<[^>]*>)?\s*\(",
        )
        .unwrap(),
        gd: Regex::new(r"^(?:static\s+)?(class_name|class|func|signal|const|var|enum)\s+([A-Za-z_]\w*)").unwrap(),
        ts: Regex::new(
            r"^\s*(?:export\s+)?(?:default\s+)?(?:declare\s+)?(?:async\s+)?(function\*?|class|interface|type|enum|const|let)\s+([A-Za-z_$][\w$]*)",
        )
        .unwrap(),
        sql: Regex::new(
            r#"(?i)\bcreate\s+(?:or\s+replace\s+)?(table|view|materialized\s+view|function|procedure|type|trigger|policy|index|schema)\s+(?:if\s+not\s+exists\s+)?"?([\w.]+)"?"#,
        )
        .unwrap(),
        node: Regex::new(r#"^\[node\s+name="([^"]+)"(?:\s+type="([^"]+)")?"#).unwrap(),
        resource: Regex::new(r#"path="(res://[^"]+)""#).unwrap(),
    })
}

/// Directory that holds the `project.godot` governing a file, for resolving `res://`.
fn godot_root(root: &Path, rel: &str) -> String {
    let mut dir = Path::new(rel).parent();
    while let Some(d) = dir {
        if root.join(d).join("project.godot").is_file() {
            return d.to_string_lossy().replace('\\', "/");
        }
        dir = d.parent();
    }
    String::new()
}

fn extract(root: &Path, rel: &str, lang: &str, text: &str) -> (Vec<Symbol>, Vec<String>) {
    let p = patterns();
    let mut symbols = vec![];
    let mut uses = vec![];
    let mut push = |name: &str, kind: &str, line: usize| {
        if symbols.len() < MAX_SYMBOLS_PER_FILE {
            symbols.push(Symbol {
                name: name.to_string(),
                kind: kind.to_string(),
                file: rel.to_string(),
                line: line as u32 + 1,
            });
        }
    };
    let res_root = if lang == "godot" {
        godot_root(root, rel)
    } else {
        String::new()
    };
    let mut root_node = true;
    for (i, line) in text.lines().enumerate() {
        match lang {
            "cs" => {
                if let Some(c) = p.cs_type.captures(line) {
                    push(&c[2], &c[1], i);
                } else if let Some(c) = p.cs_method.captures(line) {
                    if !["if", "for", "foreach", "while", "switch", "return", "new"]
                        .contains(&&c[1])
                    {
                        push(&c[1], "method", i);
                    }
                }
            }
            "gd" => {
                if let Some(c) = p.gd.captures(line) {
                    push(&c[2], &c[1], i);
                }
            }
            "ts" => {
                if let Some(c) = p.ts.captures(line) {
                    // Only top-level constants: indented ones are locals.
                    let top = !line.starts_with(' ') && !line.starts_with('\t');
                    if top || !["const", "let"].contains(&&c[1]) {
                        push(&c[2], c[1].trim_end_matches('*'), i);
                    }
                }
            }
            "sql" => {
                for c in p.sql.captures_iter(line) {
                    let kind = c[1]
                        .to_lowercase()
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" ");
                    push(&c[2], &kind, i);
                }
            }
            "godot" => {
                if let Some(c) = p.node.captures(line) {
                    let kind = if root_node { "scene" } else { "node" };
                    root_node = false;
                    push(&c[1], kind, i);
                }
                for c in p.resource.captures_iter(line) {
                    let path = c[1].trim_start_matches("res://");
                    uses.push(if res_root.is_empty() {
                        path.to_string()
                    } else {
                        format!("{res_root}/{path}")
                    });
                }
            }
            _ => {}
        }
    }
    (symbols, uses)
}

fn modified(meta: &std::fs::Metadata) -> u64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or_default()
}

fn store_path(data: &Path, root: &Path) -> PathBuf {
    use std::hash::{DefaultHasher, Hash, Hasher};
    let mut h = DefaultHasher::new();
    root.to_string_lossy().to_lowercase().hash(&mut h);
    data.join("repomap").join(format!("{:x}.json", h.finish()))
}

fn cache() -> &'static Mutex<BTreeMap<PathBuf, Index>> {
    static C: OnceLock<Mutex<BTreeMap<PathBuf, Index>>> = OnceLock::new();
    C.get_or_init(Default::default)
}

/// Brings the index up to date and runs `f` on it. Only new or changed files are read.
fn with_index<T>(data: &Path, root: &Path, f: impl FnOnce(&Index, &IndexStats) -> T) -> T {
    let started = Instant::now();
    let mut guard = cache().lock().unwrap_or_else(|e| e.into_inner());
    let index = guard.entry(root.to_path_buf()).or_insert_with(|| {
        std::fs::read_to_string(store_path(data, root))
            .ok()
            .and_then(|t| serde_json::from_str::<Index>(&t).ok())
            .filter(|i| i.version == VERSION)
            .unwrap_or_default()
    });
    index.version = VERSION;
    let mut stats = IndexStats::default();
    let mut seen = HashSet::new();
    for entry in ignore::WalkBuilder::new(root)
        .hidden(true)
        .git_ignore(true)
        .require_git(false)
        .max_depth(Some(16))
        .build()
        .flatten()
    {
        if seen.len() >= MAX_FILES || started.elapsed() > BUILD_BUDGET {
            stats.partial = true;
            break;
        }
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        let Ok(rel) = entry.path().strip_prefix(root) else {
            continue;
        };
        let rel = rel.to_string_lossy().replace('\\', "/");
        let Some(lang) = language(&rel) else {
            continue;
        };
        if crate::context::is_secret(&rel) || crate::context::is_noise(&rel) {
            continue;
        }
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        if meta.len() > MAX_FILE_BYTES {
            continue;
        }
        seen.insert(rel.clone());
        let stamp = (meta.len(), modified(&meta));
        if index
            .files
            .get(&rel)
            .is_some_and(|r| (r.size, r.modified) == stamp)
        {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(entry.path()) else {
            continue;
        };
        let (symbols, uses) = extract(root, &rel, lang, &text);
        index.files.insert(
            rel,
            FileRecord {
                size: stamp.0,
                modified: stamp.1,
                symbols,
                uses,
            },
        );
        stats.refreshed += 1;
    }
    if !stats.partial {
        let before = index.files.len();
        index.files.retain(|rel, _| seen.contains(rel));
        stats.removed = before - index.files.len();
    }
    if stats.refreshed > 0 || stats.removed > 0 {
        let path = store_path(data, root);
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(text) = serde_json::to_string(&*index) {
            let _ = std::fs::write(path, text);
        }
    }
    stats.files = index.files.len();
    stats.symbols = index.files.values().map(|r| r.symbols.len()).sum();
    stats.millis = started.elapsed().as_millis() as u64;
    f(index, &stats)
}

#[cfg(test)]
fn refresh(data: &Path, root: &Path) -> IndexStats {
    with_index(data, root, |_, s| s.clone())
}

fn rank(query: &str, name: &str) -> Option<u8> {
    let q = query.to_lowercase();
    let n = name.to_lowercase();
    if name == query {
        Some(0)
    } else if n == q {
        Some(1)
    } else if n.starts_with(&q) {
        Some(2)
    } else if n.contains(&q) {
        Some(3)
    } else {
        None
    }
}

pub fn search(data: &Path, root: &Path, query: &str, limit: usize) -> (Vec<Symbol>, IndexStats) {
    let query = query.trim();
    with_index(data, root, |index, stats| {
        if query.is_empty() {
            return (vec![], stats.clone());
        }
        let mut hits: Vec<(u8, &Symbol)> = index
            .files
            .values()
            .flat_map(|r| r.symbols.iter())
            .filter_map(|s| rank(query, &s.name).map(|r| (r, s)))
            .collect();
        hits.sort_by(|a, b| (a.0, &a.1.name, &a.1.file).cmp(&(b.0, &b.1.name, &b.1.file)));
        (
            hits.into_iter()
                .take(limit)
                .map(|(_, s)| s.clone())
                .collect(),
            stats.clone(),
        )
    })
}

/// Lines that mention `name` as a whole word, plus scenes/resources that use a file of that
/// name. Text search over indexed files only; bounded in count and time.
pub fn references(data: &Path, root: &Path, name: &str, limit: usize) -> Vec<Reference> {
    let Ok(word) = Regex::new(&format!(r"\b{}\b", regex::escape(name))) else {
        return vec![];
    };
    let files: Vec<(String, Vec<String>)> = with_index(data, root, |index, _| {
        index
            .files
            .iter()
            .map(|(rel, r)| (rel.clone(), r.uses.clone()))
            .collect()
    });
    let started = Instant::now();
    let mut out = vec![];
    for (rel, uses) in files {
        if out.len() >= limit || started.elapsed() > Duration::from_secs(3) {
            break;
        }
        if let Some(used) = uses.iter().find(|u| {
            Path::new(u)
                .file_stem()
                .is_some_and(|s| s.to_string_lossy() == name)
        }) {
            out.push(Reference {
                file: rel.clone(),
                line: 0,
                text: format!("uses {used}"),
            });
        }
        let Ok(text) = std::fs::read_to_string(root.join(&rel)) else {
            continue;
        };
        for (i, line) in text.lines().enumerate() {
            if word.is_match(line) {
                out.push(Reference {
                    file: rel.clone(),
                    line: i as u32 + 1,
                    text: line.trim().chars().take(160).collect(),
                });
                if out.len() >= limit {
                    break;
                }
            }
        }
    }
    out
}

/// Identifiers in a request worth looking up: CamelCase, snake_case or dotted names of four or
/// more characters, and file stems such as `PlayerController.cs`.
fn mentioned(request: &str) -> Vec<String> {
    static WORD: OnceLock<Regex> = OnceLock::new();
    let word = WORD.get_or_init(|| Regex::new(r"[A-Za-z_][\w]{3,}").unwrap());
    let mut out: Vec<String> = vec![];
    for m in word.find_iter(request) {
        let w = m.as_str();
        let shaped = w.contains('_') || w.chars().skip(1).any(|c| c.is_uppercase());
        if shaped && !out.iter().any(|o| o == w) {
            out.push(w.to_string());
        }
    }
    out.truncate(24);
    out
}

/// Exact definitions of identifiers the request mentions, as a bounded prompt block. Empty when
/// nothing matches, so ordinary requests are unchanged.
pub fn context_for(data: &Path, root: &Path, request: &str) -> (String, Vec<Symbol>) {
    let names = mentioned(request);
    if names.is_empty() {
        return (String::new(), vec![]);
    }
    let picked: Vec<Symbol> = with_index(data, root, |index, _| {
        let mut picked: Vec<Symbol> = vec![];
        for name in &names {
            let mut defs: Vec<&Symbol> = index
                .files
                .values()
                .flat_map(|r| r.symbols.iter())
                // SQL objects also match without their schema: `game.battles` ↔ `battles`.
                .filter(|s| {
                    &s.name == name || s.name.rsplit_once('.').is_some_and(|(_, n)| n == name)
                })
                .collect();
            defs.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));
            for s in defs.into_iter().take(3) {
                if picked.len() < CONTEXT_SYMBOLS {
                    picked.push(s.clone());
                }
            }
        }
        picked
    });
    if picked.is_empty() {
        return (String::new(), vec![]);
    }
    let mut block = String::from(
        "\nRepository map (generated by Lumen from declarations; verify by reading the files):\n",
    );
    let mut kept = vec![];
    for s in picked {
        let line = format!("- {} ({}) — {}:{}\n", s.name, s.kind, s.file, s.line);
        if block.len() + line.len() > CONTEXT_CHARS {
            break;
        }
        block.push_str(&line);
        kept.push(s);
    }
    (block, kept)
}

fn root_of(folder: &str) -> Result<PathBuf, String> {
    let root = dunce::canonicalize(folder).map_err(|_| "Choose an existing folder")?;
    if root.is_dir() {
        Ok(root)
    } else {
        Err("Choose an existing folder".into())
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub symbols: Vec<Symbol>,
    pub stats: IndexStats,
}

#[tauri::command]
pub async fn repo_map_search(
    core: tauri::State<'_, std::sync::Arc<crate::Core>>,
    folder: String,
    query: String,
) -> Result<SearchResult, String> {
    let root = root_of(&folder)?;
    let data = core.root.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let (symbols, stats) = search(&data, &root, &query, 60);
        SearchResult { symbols, stats }
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn repo_map_references(
    core: tauri::State<'_, std::sync::Arc<crate::Core>>,
    folder: String,
    name: String,
) -> Result<Vec<Reference>, String> {
    let root = root_of(&folder)?;
    let data = core.root.clone();
    tauri::async_runtime::spawn_blocking(move || references(&data, &root, &name, 80))
        .await
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, rel: &str, text: &str) {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    fn fixture(name: &str) -> (PathBuf, PathBuf) {
        let base = std::env::temp_dir().join(format!("lumen-map-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let root = base.join("project");
        std::fs::create_dir_all(&root).unwrap();
        write(&root, ".gitignore", "ignored/\n");
        write(&root, "game/project.godot", "config_version=5\n");
        write(
            &root,
            "game/scripts/PlayerController.cs",
            "namespace Game;\npublic partial class PlayerController : Node\n{\n    public override void _Ready() {}\n    private int ComputeDamage(int a) { return a; }\n}\n",
        );
        write(&root, "game/scripts/hud.gd", "class_name Hud\nextends Control\nsignal health_changed\nfunc show_damage(amount):\n\tvar local = 1\n");
        write(
            &root,
            "game/scenes/client.tscn",
            "[gd_scene format=3]\n[ext_resource type=\"Script\" path=\"res://scripts/PlayerController.cs\" id=\"1\"]\n[node name=\"Client\" type=\"Node2D\"]\n[node name=\"Hud\" type=\"Control\" parent=\".\"]\n",
        );
        write(&root, "backend/src/catalog.ts", "export interface CatalogItem { id: string }\nexport async function loadCatalog() {\n  const inner = 1;\n}\n");
        write(&root, "supabase/migrations/001.sql", "create table if not exists public.hero_stats (id int);\nCREATE OR REPLACE FUNCTION public.grant_reward() returns void as $$ $$;\n");
        write(&root, "ignored/Secret.cs", "class Hidden {}\n");
        write(&root, ".env", "KEY=1\n");
        (base.join("data"), root)
    }

    #[test]
    fn indexes_the_four_languages_and_godot_scenes() {
        let (data, root) = fixture("langs");
        let stats = refresh(&data, &root);
        assert!(!stats.partial);
        let (all, _) = search(&data, &root, "e", 500);
        let names: HashSet<&str> = all.iter().map(|s| s.name.as_str()).collect();
        for expected in [
            "PlayerController",
            "ComputeDamage",
            "Hud",
            "health_changed",
            "show_damage",
            "CatalogItem",
            "loadCatalog",
            "public.hero_stats",
            "public.grant_reward",
            "Client",
        ] {
            let found = search(&data, &root, expected, 5).0;
            assert!(
                found.iter().any(|s| s.name == expected),
                "{expected} missing: {names:?}"
            );
        }
        assert!(
            search(&data, &root, "Hidden", 5).0.is_empty(),
            ".gitignore is respected"
        );
        assert!(
            search(&data, &root, "inner", 5).0.is_empty(),
            "locals are not symbols"
        );
        assert!(
            search(&data, &root, "local", 5).0.is_empty(),
            "indented GDScript vars are locals"
        );
        let scene = search(&data, &root, "Client", 1).0.remove(0);
        assert_eq!(
            (scene.kind.as_str(), scene.file.as_str(), scene.line),
            ("scene", "game/scenes/client.tscn", 3)
        );
        let refs = references(&data, &root, "PlayerController", 20);
        assert!(refs.iter().any(|r| r.file == "game/scenes/client.tscn"
            && r.text.contains("game/scripts/PlayerController.cs")));
    }

    #[test]
    fn refreshes_edits_deletions_and_renames_only() {
        let (data, root) = fixture("incremental");
        let first = refresh(&data, &root);
        assert_eq!(first.refreshed, first.files);
        assert_eq!(
            refresh(&data, &root).refreshed,
            0,
            "unchanged files are not read again"
        );
        std::fs::write(
            root.join("game/scripts/hud.gd"),
            "class_name HudPanel\nfunc toggle_panel():\n\tpass\n",
        )
        .unwrap();
        std::fs::rename(
            root.join("backend/src/catalog.ts"),
            root.join("backend/src/shop.ts"),
        )
        .unwrap();
        std::fs::remove_file(root.join("supabase/migrations/001.sql")).unwrap();
        let next = refresh(&data, &root);
        assert_eq!((next.refreshed, next.removed), (2, 2));
        assert!(search(&data, &root, "Hud", 5)
            .0
            .iter()
            .all(|s| s.name != "Hud" || s.file.ends_with(".tscn")));
        assert!(search(&data, &root, "HudPanel", 5)
            .0
            .iter()
            .any(|s| s.file == "game/scripts/hud.gd"));
        assert_eq!(
            search(&data, &root, "loadCatalog", 1).0[0].file,
            "backend/src/shop.ts"
        );
        assert!(search(&data, &root, "grant_reward", 5).0.is_empty());
    }

    #[test]
    fn adds_only_definitions_the_request_mentions() {
        let (data, root) = fixture("context");
        let (block, used) = context_for(
            &data,
            &root,
            "Make PlayerController react to health_changed and store it in hero_stats",
        );
        assert_eq!(used.len(), 3);
        assert!(block.contains("public.hero_stats (table)"));
        assert!(block.contains("game/scripts/PlayerController.cs:2"));
        assert!(block.len() <= CONTEXT_CHARS);
        assert!(
            context_for(&data, &root, "please fix the typo in the readme")
                .0
                .is_empty()
        );
    }

    /// Measurement on a real project: `LUMEN_MAP_PROJECT=<folder>`.
    #[test]
    #[ignore]
    fn measure_real_project() {
        let root = PathBuf::from(std::env::var("LUMEN_MAP_PROJECT").expect("LUMEN_MAP_PROJECT"));
        let data = std::env::temp_dir().join(format!("lumen-map-measure-{}", std::process::id()));
        let cold = refresh(&data, &root);
        cache().lock().unwrap().clear();
        let warm = refresh(&data, &root);
        println!(
            "cold {cold:?}
warm (from disk) {warm:?}"
        );
        let request = std::env::var("LUMEN_MAP_REQUEST").unwrap_or_default();
        let started = Instant::now();
        let (block, used) = context_for(&data, &root, &request);
        println!(
            "context {}ms, {} symbols, {} chars
{block}",
            started.elapsed().as_millis(),
            used.len(),
            block.len()
        );
        let _ = std::fs::remove_dir_all(&data);
    }
}
