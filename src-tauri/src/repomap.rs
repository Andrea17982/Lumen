//! Repository map: a local, incremental symbol index for Rust, TypeScript/JavaScript, Python, Go,
//! C#, Java, Kotlin, C/C++, PHP, Ruby, Swift, GDScript, SQL and Godot scenes/resources.
//! Definitions are found with per-language patterns, not a full parser. Each symbol keeps its
//! line span and signature; each file keeps its import specifiers and the identifiers it uses,
//! which `codegraph` turns into a dependency graph. The index lives in Lumen's data folder,
//! never in the project, and is refreshed by file size and modification time: edits re-read a
//! file, deletions drop it, renames are a deletion plus a new file.
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
const VERSION: u32 = 2;
/// Distinct identifiers kept per file for the reference graph.
const MAX_IDENTS_PER_FILE: usize = 4_000;
const MAX_SPAN_LINES: usize = 4_000;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Symbol {
    pub name: String,
    pub kind: String,
    pub file: String,
    pub line: u32,
    /// Last line of the definition (inclusive); equals `line` for one-line declarations.
    #[serde(default)]
    pub end: u32,
    /// The declaration line, trimmed.
    #[serde(default)]
    pub signature: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub(crate) struct FileRecord {
    size: u64,
    pub(crate) modified: u64,
    pub(crate) symbols: Vec<Symbol>,
    /// `res://` paths a scene or resource depends on, resolved to project-relative paths.
    pub(crate) uses: Vec<String>,
    /// Import specifiers as written (`./util`, `crate::store`, `os.path`, `res://x.gd`).
    #[serde(default)]
    pub(crate) imports: Vec<String>,
    /// Distinct identifiers the file mentions, for the reference graph.
    #[serde(default)]
    pub(crate) idents: Vec<String>,
    #[serde(default)]
    pub(crate) lang: String,
}

#[derive(Serialize, Deserialize, Default)]
pub(crate) struct Index {
    version: u32,
    pub(crate) files: BTreeMap<String, FileRecord>,
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
        "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" | "mts" | "cts" | "vue" | "svelte" => "ts",
        "sql" => "sql",
        "tscn" | "tres" => "godot",
        "rs" => "rs",
        "py" | "pyi" => "py",
        "go" => "go",
        "java" => "java",
        "kt" | "kts" => "kt",
        "c" | "h" | "cc" | "cpp" | "cxx" | "hpp" | "hh" | "hxx" => "c",
        "php" => "php",
        "rb" => "rb",
        "swift" => "swift",
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
    ts_method: Regex,
    ts_import: Regex,
    rs: Regex,
    rs_import: Regex,
    rs_impl: Regex,
    rs_path: Regex,
    py: Regex,
    py_import: Regex,
    go_func: Regex,
    go_type: Regex,
    go_import: Regex,
    java_method: Regex,
    kt: Regex,
    dotted_import: Regex,
    c_type: Regex,
    c_func: Regex,
    c_include: Regex,
    php: Regex,
    php_import: Regex,
    rb: Regex,
    rb_import: Regex,
    swift: Regex,
    gd_import: Regex,
    ident: Regex,
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
        ts_method: Regex::new(
            r"^\s+(?:(?:public|private|protected|static|async|readonly|override|abstract|get|set)\s+)*([A-Za-z_$][\w$]*)\s*(?:<[^>]*>)?\([^;]*\)\s*(?::\s*[^={;]+)?\{\s*$",
        )
        .unwrap(),
        ts_import: Regex::new(
            r#"(?:\bfrom\s+|\bimport\s*\(\s*|\brequire\s*\(\s*|^\s*import\s+)['"]([^'"]+)['"]"#,
        )
        .unwrap(),
        rs: Regex::new(
            r#"^\s*(?:pub(?:\([^)]*\))?\s+)?(?:default\s+)?(?:const\s+)?(?:async\s+)?(?:unsafe\s+)?(?:extern\s+"[^"]*"\s+)?(fn|struct|enum|trait|type|mod|const|static|union|macro_rules!)\s+([A-Za-z_]\w*)"#,
        )
        .unwrap(),
        rs_import: Regex::new(r"^\s*(?:pub(?:\([^)]*\))?\s+)?(?:use\s+([\w:]+)|mod\s+(\w+)\s*;)").unwrap(),
        rs_impl: Regex::new(r"^\s*(?:unsafe\s+)?impl(?:<[^{]*?>)?\s+(?:[\w:]+(?:<[^{]*?>)?\s+for\s+)?(?:[\w]+::)*([A-Za-z_]\w*)").unwrap(),
        rs_path: Regex::new(r"\b(?:crate|super)::\w+(?:::\w+)*").unwrap(),
        py: Regex::new(r"^(\s*)(?:async\s+)?(def|class)\s+([A-Za-z_]\w*)").unwrap(),
        py_import: Regex::new(r"^\s*(?:from\s+(\.*[\w.]*)\s+import|import\s+([\w.]+))").unwrap(),
        go_func: Regex::new(r"^func\s+(\([^)]*\)\s*)?([A-Za-z_]\w*)").unwrap(),
        go_type: Regex::new(r"^type\s+([A-Za-z_]\w*)\s+(struct|interface)?").unwrap(),
        go_import: Regex::new(r#"^\s*(?:import\s+)?(?:[\w.]+\s+)?"([^"]+)"\s*$"#).unwrap(),
        java_method: Regex::new(
            r"^\s*(?:@\w+\s+)*(?:(?:public|private|protected|static|final|abstract|synchronized|native|default)\s+)+(?:<[^>]*>\s*)?[\w<>\[\],.?]+\s+([A-Za-z_]\w*)\s*\(",
        )
        .unwrap(),
        kt: Regex::new(
            r"^\s*(?:(?:public|private|protected|internal|open|abstract|sealed|data|enum|inline|suspend|override|operator|infix|tailrec|companion|annotation|value)\s+)*(class|interface|object|fun|typealias)\s+(?:<[^>]*>\s*)?(?:[A-Za-z_]\w*\.)?([A-Za-z_]\w*)",
        )
        .unwrap(),
        dotted_import: Regex::new(r"^\s*import\s+(?:static\s+)?([\w.]+)").unwrap(),
        c_type: Regex::new(r"^\s*(?:typedef\s+)?(struct|class|enum|union|namespace)\s+([A-Za-z_]\w*)\s*(?:[:{]|$)").unwrap(),
        c_func: Regex::new(r"^(?:[A-Za-z_][\w:<>,*&\s]*?[\s*&])?([A-Za-z_][\w:~]*)\s*\([^;]*$").unwrap(),
        c_include: Regex::new(r#"^\s*#\s*include\s*"([^"]+)""#).unwrap(),
        php: Regex::new(r"^\s*(?:(?:abstract|final|public|private|protected|static|readonly)\s+)*(class|interface|trait|enum|function)\s+([A-Za-z_]\w*)").unwrap(),
        php_import: Regex::new(r#"(?:require|include)(?:_once)?\s*\(?\s*(?:__DIR__\s*\.\s*)?['"]([^'"]+)['"]"#).unwrap(),
        rb: Regex::new(r"^\s*(class|module|def)\s+(?:self\.)?([A-Za-z_][\w:]*[?!]?)").unwrap(),
        rb_import: Regex::new(r#"^\s*require_relative\s+['"]([^'"]+)['"]"#).unwrap(),
        swift: Regex::new(
            r"^\s*(?:(?:public|private|fileprivate|internal|open|final|static|override|mutating|@\w+)\s+)*(class|struct|enum|protocol|extension|func|actor|typealias)\s+([A-Za-z_]\w*)",
        )
        .unwrap(),
        gd_import: Regex::new(r#"(?:preload|load)\(\s*"(res://[^"]+)"|^extends\s+"(res://[^"]+)""#).unwrap(),
        ident: Regex::new(r"[A-Za-z_][A-Za-z0-9_]{2,}").unwrap(),
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

/// What one file contributes to the index.
struct Extracted {
    symbols: Vec<Symbol>,
    uses: Vec<String>,
    imports: Vec<String>,
    idents: Vec<String>,
}

const NOT_FUNCTIONS: [&str; 14] = [
    "if",
    "for",
    "foreach",
    "while",
    "switch",
    "return",
    "new",
    "catch",
    "sizeof",
    "else",
    "do",
    "function",
    "constructor",
    "super",
];

fn extract(root: &Path, rel: &str, lang: &str, text: &str) -> Extracted {
    let p = patterns();
    let lines: Vec<&str> = text.lines().collect();
    let mut symbols = vec![];
    let mut uses = vec![];
    let mut imports: Vec<String> = vec![];
    let mut push = |name: &str, kind: &str, line: usize| {
        if symbols.len() < MAX_SYMBOLS_PER_FILE && !name.is_empty() {
            symbols.push(Symbol {
                name: name.to_string(),
                kind: kind.to_string(),
                file: rel.to_string(),
                line: line as u32 + 1,
                end: line as u32 + 1,
                signature: lines[line].trim().chars().take(200).collect(),
            });
        }
    };
    let res_root = if matches!(lang, "godot" | "gd") {
        godot_root(root, rel)
    } else {
        String::new()
    };
    let res = |path: &str| {
        let path = path.trim_start_matches("res://");
        if res_root.is_empty() {
            path.to_string()
        } else {
            format!("{res_root}/{path}")
        }
    };
    let mut root_node = true;
    let mut go_imports = false;
    for (i, line) in lines.iter().enumerate() {
        let line = *line;
        let top = !line.starts_with(' ') && !line.starts_with('\t');
        match lang {
            "cs" => {
                if let Some(c) = p.cs_type.captures(line) {
                    push(&c[2], &c[1], i);
                } else if let Some(c) = p.cs_method.captures(line) {
                    if !NOT_FUNCTIONS.contains(&&c[1]) {
                        push(&c[1], "method", i);
                    }
                }
            }
            "gd" => {
                if let Some(c) = p.gd.captures(line) {
                    push(&c[2], &c[1], i);
                }
                for c in p.gd_import.captures_iter(line) {
                    if let Some(m) = c.get(1).or_else(|| c.get(2)) {
                        imports.push(format!("res://{}", res(m.as_str())));
                    }
                }
            }
            "ts" => {
                if let Some(c) = p.ts.captures(line) {
                    // Only top-level constants: indented ones are locals.
                    if top || !["const", "let"].contains(&&c[1]) {
                        push(&c[2], c[1].trim_end_matches('*'), i);
                    }
                } else if let Some(c) = p.ts_method.captures(line) {
                    if !NOT_FUNCTIONS.contains(&&c[1]) {
                        push(&c[1], "method", i);
                    }
                }
                for c in p.ts_import.captures_iter(line) {
                    imports.push(c[1].to_string());
                }
            }
            "rs" => {
                if let Some(c) = p.rs.captures(line) {
                    let kind = c[1].trim_end_matches('!');
                    let kind = if kind == "fn" && !top { "method" } else { kind };
                    push(&c[2], kind, i);
                } else if let Some(c) = p.rs_impl.captures(line) {
                    // Methods nest inside their impl block.
                    push(&c[1], "impl", i);
                }
                if let Some(c) = p.rs_import.captures(line) {
                    if let Some(m) = c.get(1) {
                        imports.push(m.as_str().to_string());
                    } else if let Some(m) = c.get(2) {
                        imports.push(format!("mod {}", m.as_str()));
                    }
                }
                for m in p.rs_path.find_iter(line) {
                    imports.push(m.as_str().to_string());
                }
            }
            "py" => {
                if let Some(c) = p.py.captures(line) {
                    let kind = if c[1].is_empty() {
                        &c[2]
                    } else if &c[2] == "def" {
                        "method"
                    } else {
                        "class"
                    };
                    push(&c[3], kind, i);
                }
                if let Some(c) = p.py_import.captures(line) {
                    if let Some(m) = c.get(1) {
                        // `from pkg import mod_a, mod_b`: the names may be submodules.
                        let base = m.as_str();
                        imports.push(base.to_string());
                        let names = line.split_once(" import ").map_or("", |x| x.1);
                        for n in names.split(',').take(12) {
                            let n = n.trim().trim_matches(|c| c == '(' || c == ')');
                            let n = n.split_whitespace().next().unwrap_or("");
                            if !n.is_empty() && n != "*" {
                                let sep = if base.ends_with('.') { "" } else { "." };
                                imports.push(format!("{base}{sep}{n}"));
                            }
                        }
                    } else if let Some(m) = c.get(2) {
                        imports.push(m.as_str().to_string());
                    }
                }
            }
            "go" => {
                if let Some(c) = p.go_func.captures(line) {
                    push(&c[2], if c.get(1).is_some() { "method" } else { "func" }, i);
                } else if let Some(c) = p.go_type.captures(line) {
                    push(&c[1], c.get(2).map_or("type", |m| m.as_str()), i);
                }
                if line.starts_with("import (") {
                    go_imports = true;
                } else if go_imports && line.trim() == ")" {
                    go_imports = false;
                } else if go_imports || line.starts_with("import ") {
                    if let Some(c) = p.go_import.captures(line) {
                        imports.push(c[1].to_string());
                    }
                }
            }
            "java" | "kt" => {
                if lang == "kt" {
                    if let Some(c) = p.kt.captures(line) {
                        push(&c[2], &c[1], i);
                    }
                } else if let Some(c) = p.cs_type.captures(line) {
                    push(&c[2], &c[1], i);
                } else if let Some(c) = p.java_method.captures(line) {
                    if !NOT_FUNCTIONS.contains(&&c[1]) {
                        push(&c[1], "method", i);
                    }
                }
                if let Some(c) = p.dotted_import.captures(line) {
                    imports.push(c[1].to_string());
                }
            }
            "c" => {
                if let Some(c) = p.c_type.captures(line) {
                    push(&c[2], &c[1], i);
                } else if top && !line.starts_with('#') && !line.starts_with('}') {
                    if let Some(c) = p.c_func.captures(line) {
                        let name = c[1].rsplit("::").next().unwrap_or(&c[1]);
                        if !NOT_FUNCTIONS.contains(&name) {
                            push(&c[1], "function", i);
                        }
                    }
                }
                if let Some(c) = p.c_include.captures(line) {
                    imports.push(c[1].to_string());
                }
            }
            "php" => {
                if let Some(c) = p.php.captures(line) {
                    let kind = if &c[1] == "function" && !top {
                        "method"
                    } else {
                        &c[1]
                    };
                    push(&c[2], kind, i);
                }
                if let Some(c) = p.php_import.captures(line) {
                    imports.push(c[1].to_string());
                }
            }
            "rb" => {
                if let Some(c) = p.rb.captures(line) {
                    push(&c[2], &c[1], i);
                }
                if let Some(c) = p.rb_import.captures(line) {
                    imports.push(c[1].to_string());
                }
            }
            "swift" => {
                if let Some(c) = p.swift.captures(line) {
                    let kind = if &c[1] == "func" && !top {
                        "method"
                    } else {
                        &c[1]
                    };
                    push(&c[2], kind, i);
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
                    uses.push(res(&c[1]));
                }
            }
            _ => {}
        }
    }
    spans(&mut symbols, &lines, lang);
    imports.sort();
    imports.dedup();
    let mut idents: Vec<String> = if lang == "godot" {
        vec![]
    } else {
        let mut set: HashSet<&str> = HashSet::new();
        for m in p.ident.find_iter(text) {
            if set.len() >= MAX_IDENTS_PER_FILE {
                break;
            }
            set.insert(m.as_str());
        }
        set.into_iter().map(str::to_string).collect()
    };
    idents.sort();
    Extracted {
        symbols,
        uses,
        imports,
        idents,
    }
}

/// Fills each symbol's last line: matching braces for C-like languages, indentation for
/// Python, GDScript and Ruby, the next declaration for SQL.
fn spans(symbols: &mut [Symbol], lines: &[&str], lang: &str) {
    let starts: Vec<usize> = symbols.iter().map(|s| s.line as usize - 1).collect();
    for (k, s) in symbols.iter_mut().enumerate() {
        let start = s.line as usize - 1;
        let end = match lang {
            "py" | "gd" | "rb" => indent_end(lines, start),
            "sql" | "godot" => starts
                .get(k + 1)
                .map(|n| n.saturating_sub(1).max(start))
                .unwrap_or(lines.len().saturating_sub(1)),
            _ => brace_end(lines, start),
        };
        s.end = end as u32 + 1;
    }
}

fn indent_of(line: &str) -> usize {
    line.chars()
        .take_while(|c| c.is_whitespace())
        .map(|c| if c == '\t' { 4 } else { 1 })
        .sum()
}

fn indent_end(lines: &[&str], start: usize) -> usize {
    let base = indent_of(lines[start]);
    let mut end = start;
    for (i, line) in lines
        .iter()
        .enumerate()
        .skip(start + 1)
        .take(MAX_SPAN_LINES)
    {
        if line.trim().is_empty() {
            continue;
        }
        if indent_of(line) <= base {
            // Ruby closes blocks with an `end` at the same indentation.
            if line.trim() == "end" {
                end = i;
            }
            break;
        }
        end = i;
    }
    end
}

/// Line of the brace that closes the block opened at or after `start`. A `;` before any `{`
/// ends a one-line declaration. Strings, chars and `//` comments are skipped roughly.
fn brace_end(lines: &[&str], start: usize) -> usize {
    let mut depth = 0i32;
    let mut opened = false;
    for (i, line) in lines.iter().enumerate().skip(start).take(MAX_SPAN_LINES) {
        let mut quote: Option<char> = None;
        let mut prev = ' ';
        for ch in line.chars() {
            if let Some(q) = quote {
                if ch == q && prev != '\\' {
                    quote = None;
                }
                prev = if prev == '\\' && ch == '\\' { ' ' } else { ch };
                continue;
            }
            match ch {
                '/' if prev == '/' => break,
                '"' | '`' => quote = Some(ch),
                '{' => {
                    depth += 1;
                    opened = true;
                }
                '}' => {
                    depth -= 1;
                    if opened && depth <= 0 {
                        return i;
                    }
                }
                ';' if !opened && depth == 0 => return i,
                _ => {}
            }
            prev = ch;
        }
    }
    start
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
pub(crate) fn with_index<T>(
    data: &Path,
    root: &Path,
    f: impl FnOnce(&Index, &IndexStats) -> T,
) -> T {
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
        let found = extract(root, &rel, lang, &text);
        index.files.insert(
            rel,
            FileRecord {
                size: stamp.0,
                modified: stamp.1,
                symbols: found.symbols,
                uses: found.uses,
                imports: found.imports,
                idents: found.idents,
                lang: lang.to_string(),
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
            let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
            if std::fs::write(&tmp, text).is_ok() && std::fs::rename(&tmp, &path).is_err() {
                let _ = std::fs::remove_file(&tmp);
            }
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
        let started = Instant::now();
        let graph = crate::codegraph::graph(&data, &root);
        println!("graph {}ms", started.elapsed().as_millis());
        println!("{}", graph.overview(15));
        if let Ok(name) = std::env::var("LUMEN_MAP_SYMBOL") {
            let started = Instant::now();
            let text = graph.symbol(&name, None, false);
            println!(
                "symbol {}ms, {} chars\n{text}",
                started.elapsed().as_millis(),
                text.len()
            );
        }
        let _ = std::fs::remove_dir_all(&data);
    }
}
