//! Code graph over the repository map. Files are nodes; edges come from resolved imports, from
//! identifiers that name a symbol defined in another file, and from git co-change history.
//! Files are ranked with PageRank so answers lead with the code that matters. Every answer is
//! compact text with exact line ranges, so a model reads only the lines it needs.
use crate::repomap::{with_index, Index, Symbol};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

/// A name defined in more files than this is too ambiguous to be a reference edge.
const MAX_DEF_FILES: usize = 6;
const MAX_BODY_LINES: usize = 150;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ImpactFile {
    pub path: String,
    pub distance: usize,
    pub test: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Impact {
    pub targets: Vec<String>,
    pub files: Vec<ImpactFile>,
    pub unknown: Vec<String>,
    pub contracts: Vec<String>,
    pub truncated: bool,
}

impl Impact {
    pub fn text(&self) -> String {
        let affected: Vec<_> = self.files.iter().filter(|f| f.distance > 0).collect();
        let mut out = if affected.is_empty() {
            "No additional dependents were found in the approximate code graph.\n".into()
        } else {
            format!("{} files may be affected (by distance):\n", affected.len())
        };
        for file in affected.iter().take(40) {
            out.push_str(&format!("- [{}] {}\n", file.distance, file.path));
        }
        let tests: Vec<_> = self
            .files
            .iter()
            .filter(|f| f.test)
            .map(|f| f.path.as_str())
            .collect();
        if !tests.is_empty() {
            out.push_str(&format!(
                "Tests to run: {}\n",
                tests
                    .iter()
                    .take(20)
                    .copied()
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        if !self.contracts.is_empty() {
            out.push_str(&format!(
                "Shared declarations to preserve or update callers of:\n{}\n",
                self.contracts
                    .iter()
                    .take(16)
                    .map(|c| format!("- {c}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            ));
        }
        if self.truncated || affected.len() > 40 || tests.len() > 20 || self.contracts.len() > 16 {
            out.push_str("Impact listing is bounded; additional dependents or declarations may exist. Query narrower targets for omitted detail.\n");
        }
        for unknown in &self.unknown {
            out.push_str(&format!("`{unknown}` is neither an indexed file nor a known symbol; its impact is unknown.\n"));
        }
        out.push_str(
            "Graph edges are approximate; absence of an edge is not proof that a change is safe.\n",
        );
        out
    }
}

#[cfg(test)]
mod projection_tests {
    #[test]
    fn context_budget_contract_omissions_are_explicit() {
        let report = super::Impact {
            contracts: (0..20).map(|i| format!("contract-{i}")).collect(),
            ..Default::default()
        };
        let text = report.text();
        assert!(text.contains("contract-15"));
        assert!(!text.contains("contract-16"));
        assert!(text.contains("bounded"));
    }
}

fn test_path(path: &str) -> bool {
    let path = path.to_lowercase();
    path.split('/')
        .any(|part| matches!(part, "test" | "tests" | "__tests__" | "spec" | "specs"))
        || [".test.", ".spec.", "_test.", "_tests.", "test_"]
            .iter()
            .any(|marker| path.rsplit('/').next().unwrap_or("").contains(marker))
}

pub struct Graph {
    partial: bool,
    root: PathBuf,
    files: Vec<String>,
    id: HashMap<String, usize>,
    lang: Vec<String>,
    symbols: Vec<Vec<Symbol>>,
    /// Name → (file, symbol index).
    defs: HashMap<String, Vec<(usize, usize)>>,
    /// Defined name → files that mention it.
    users: HashMap<String, Vec<usize>>,
    imports: Vec<Vec<usize>>,
    importers: Vec<Vec<usize>>,
    /// Child modules a file declares (`mod x;`), a subset of `imports`.
    mods: Vec<Vec<usize>>,
    refs_out: Vec<Vec<(usize, f32)>>,
    refs_in: Vec<Vec<(usize, f32)>>,
    rank: Vec<f32>,
}

type Graphs = Mutex<HashMap<PathBuf, (u64, Arc<Graph>)>>;
type CoChange = HashMap<String, HashMap<String, u32>>;

fn ident() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"[A-Za-z_][A-Za-z0-9_]{2,}").unwrap())
}

fn cache() -> &'static Graphs {
    static C: OnceLock<Graphs> = OnceLock::new();
    C.get_or_init(Default::default)
}

fn stamp(index: &Index) -> u64 {
    use std::hash::{DefaultHasher, Hash, Hasher};
    let mut h = DefaultHasher::new();
    for (rel, r) in &index.files {
        rel.hash(&mut h);
        r.modified.hash(&mut h);
        r.size.hash(&mut h);
    }
    h.finish()
}

/// The graph of `root`, rebuilt only when an indexed file changed.
pub fn graph(data: &Path, root: &Path) -> Arc<Graph> {
    with_index(data, root, |index, stats| {
        let key = stamp(index);
        let mut c = cache().lock().unwrap_or_else(|e| e.into_inner());
        if let Some((k, g)) = c.get(root) {
            if *k == key && g.partial == stats.partial {
                return g.clone();
            }
        }
        let mut built = build(root, index);
        built.partial = stats.partial;
        let g = Arc::new(built);
        c.insert(root.to_path_buf(), (key, g.clone()));
        g
    })
}

/// Languages that can call each other's definitions. SQL objects are reachable from any code.
fn family(lang: &str) -> &str {
    match lang {
        "gd" | "godot" | "cs" => "godot",
        "java" | "kt" => "jvm",
        other => other,
    }
}

/// A name specific enough to count as a reference without an import confirming it:
/// CamelCase, snake_case, or long. `prompt`, `first` or `config` are usually locals.
pub(crate) fn distinctive(name: &str) -> bool {
    let inner = name.trim_start_matches('_');
    inner.contains('_')
        || inner.chars().skip(1).any(|c| c.is_ascii_uppercase())
        || (inner.chars().next().is_some_and(|c| c.is_ascii_uppercase()) && inner.len() >= 4)
        || inner.len() >= 10
}

/// Whether another file can name this definition: Rust items without `pub` are private to
/// their module, and Python/Ruby leading-underscore names are private by convention.
fn visible_outside(lang: &str, s: &Symbol) -> bool {
    match lang {
        "rs" => s.signature.starts_with("pub") || s.kind == "macro_rules",
        "py" => !s.name.starts_with('_'),
        _ => true,
    }
}

/// Kinds that are containers or duplicates, not things code refers to by name.
fn referable(kind: &str) -> bool {
    !matches!(kind, "mod" | "impl" | "node" | "namespace")
}

fn build(root: &Path, index: &Index) -> Graph {
    let files: Vec<String> = index.files.keys().cloned().collect();
    let id: HashMap<String, usize> = files
        .iter()
        .enumerate()
        .map(|(i, f)| (f.clone(), i))
        .collect();
    let records: Vec<_> = index.files.values().collect();
    let by_name = names(&files);
    let mut imports: Vec<Vec<usize>> = vec![vec![]; files.len()];
    let mut mods: Vec<Vec<usize>> = vec![vec![]; files.len()];
    for (f, r) in records.iter().enumerate() {
        mods[f] = r
            .imports
            .iter()
            .filter(|spec| spec.starts_with("mod "))
            .flat_map(|spec| resolve(root, &files, f, &r.lang, spec, &id, &by_name))
            .collect();
        let mut targets: Vec<usize> = r
            .imports
            .iter()
            .chain(r.uses.iter())
            .flat_map(|spec| resolve(root, &files, f, &r.lang, spec, &id, &by_name))
            .filter(|&g| g != f)
            .collect();
        targets.sort_unstable();
        targets.dedup();
        imports[f] = targets;
    }
    let mut importers: Vec<Vec<usize>> = vec![vec![]; files.len()];
    for (f, targets) in imports.iter().enumerate() {
        for &g in targets {
            importers[g].push(f);
        }
    }
    let mut defs: HashMap<String, Vec<(usize, usize)>> = HashMap::new();
    for (f, r) in records.iter().enumerate() {
        for (k, s) in r.symbols.iter().enumerate() {
            if s.name.len() >= 3 && referable(&s.kind) {
                defs.entry(s.name.clone()).or_default().push((f, k));
            }
        }
    }
    let def_files: HashMap<&str, Vec<usize>> = defs
        .iter()
        .map(|(n, d)| {
            let mut v: Vec<usize> = d.iter().map(|x| x.0).collect();
            v.dedup();
            (n.as_str(), v)
        })
        .collect();
    // Files where each name has a definition other files can reach.
    let public: HashMap<&str, HashSet<usize>> = defs
        .iter()
        .map(|(n, d)| {
            let set = d
                .iter()
                .filter(|&&(g, k)| visible_outside(&records[g].lang, &records[g].symbols[k]))
                .map(|x| x.0)
                .collect();
            (n.as_str(), set)
        })
        .collect();
    let mut users: HashMap<String, Vec<usize>> = HashMap::new();
    let mut refs_out: Vec<Vec<(usize, f32)>> = vec![vec![]; files.len()];
    for (f, r) in records.iter().enumerate() {
        let fam = family(&r.lang);
        let mut out: HashMap<usize, f32> = HashMap::new();
        for ident in &r.idents {
            let Some(owners) = def_files.get(ident.as_str()) else {
                continue;
            };
            let specific = distinctive(ident);
            // Owners this file can plausibly mean: same language family (or SQL), and for
            // common-looking names only files it imports or itself.
            let owners: Vec<usize> = owners
                .iter()
                .copied()
                .filter(|&g| {
                    let lang = records[g].lang.as_str();
                    g == f
                        || ((family(lang) == fam || lang == "sql")
                            && (specific || imports[f].contains(&g))
                            && public[ident.as_str()].contains(&g))
                })
                .collect();
            if owners.is_empty() {
                continue;
            }
            users.entry(ident.clone()).or_default().push(f);
            if owners.len() > MAX_DEF_FILES || owners.contains(&f) {
                continue;
            }
            for &g in &owners {
                *out.entry(g).or_default() += 1.0 / owners.len() as f32;
            }
        }
        refs_out[f] = out.into_iter().collect();
        refs_out[f].sort_by(|a, b| b.1.total_cmp(&a.1));
    }
    let mut refs_in: Vec<Vec<(usize, f32)>> = vec![vec![]; files.len()];
    for (f, out) in refs_out.iter().enumerate() {
        for &(g, w) in out {
            refs_in[g].push((f, w));
        }
    }
    for v in &mut refs_in {
        v.sort_by(|a, b| b.1.total_cmp(&a.1));
    }
    let rank = pagerank(&imports, &refs_out);
    Graph {
        partial: false,
        root: root.to_path_buf(),
        lang: records.iter().map(|r| r.lang.clone()).collect(),
        symbols: records.iter().map(|r| r.symbols.clone()).collect(),
        files,
        id,
        defs,
        users,
        imports,
        importers,
        mods,
        refs_out,
        refs_in,
        rank,
    }
}

/// File name → files with that name, for suffix-style import resolution.
fn names(files: &[String]) -> HashMap<String, Vec<usize>> {
    let mut m: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, f) in files.iter().enumerate() {
        let name = f.rsplit('/').next().unwrap_or(f).to_string();
        m.entry(name).or_default().push(i);
    }
    m
}

fn parent(rel: &str) -> &str {
    rel.rsplit_once('/').map(|(d, _)| d).unwrap_or("")
}

fn join(dir: &str, spec: &str) -> String {
    let mut parts: Vec<&str> = dir.split('/').filter(|p| !p.is_empty()).collect();
    for seg in spec.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            s => parts.push(s),
        }
    }
    parts.join("/")
}

fn first(id: &HashMap<String, usize>, candidates: &[String]) -> Vec<usize> {
    candidates
        .iter()
        .find_map(|c| id.get(c).copied())
        .into_iter()
        .collect()
}

/// Files a file with suffix `tail` (`a/b/C.java`) may be; only unambiguous matches count.
fn by_suffix(by_name: &HashMap<String, Vec<usize>>, files: &[String], tail: &str) -> Vec<usize> {
    let name = tail.rsplit('/').next().unwrap_or(tail);
    let hits: Vec<usize> = by_name
        .get(name)
        .into_iter()
        .flatten()
        .copied()
        .filter(|&i| files[i] == tail || files[i].ends_with(&format!("/{tail}")))
        .collect();
    if hits.len() <= 2 {
        hits
    } else {
        vec![]
    }
}

fn resolve(
    root: &Path,
    files: &[String],
    from: usize,
    lang: &str,
    spec: &str,
    id: &HashMap<String, usize>,
    by_name: &HashMap<String, Vec<usize>>,
) -> Vec<usize> {
    let from = files[from].as_str();
    let dir = parent(from);
    match lang {
        "ts" => {
            let base = if spec.starts_with('.') {
                let Some(path) = crate::repomap::relative_path(dir, spec) else {
                    return vec![];
                };
                path
            } else if let Some(rest) = spec.strip_prefix("@/").or_else(|| spec.strip_prefix("~/")) {
                format!("src/{rest}")
            } else {
                return vec![];
            };
            let typed = [".ts", ".tsx", ".mts", ".cts"]
                .iter()
                .any(|ext| from.ends_with(ext));
            for (runtime, sources) in [
                (".mjs", &[".mts", ".d.mts"][..]),
                (".cjs", &[".cts", ".d.cts"][..]),
                (".jsx", &[".tsx", ".ts", ".d.ts"][..]),
                (".js", &[".ts", ".tsx", ".d.ts"][..]),
            ] {
                if let Some(stem) = base.strip_suffix(runtime) {
                    let mut candidates = if typed {
                        sources
                            .iter()
                            .map(|ext| format!("{stem}{ext}"))
                            .collect::<Vec<_>>()
                    } else {
                        vec![]
                    };
                    candidates.push(base.clone());
                    if typed && runtime == ".js" {
                        candidates.push(format!("{stem}.jsx"));
                    }
                    return first(id, &candidates);
                }
            }
            let stem = &base;
            let mut c = vec![base.clone()];
            for e in [
                ".ts", ".tsx", ".js", ".jsx", ".mjs", ".cjs", ".vue", ".svelte",
            ] {
                c.push(format!("{stem}{e}"));
            }
            for e in ["index.ts", "index.tsx", "index.js", "index.jsx"] {
                c.push(format!("{base}/{e}"));
            }
            first(id, &c)
        }
        "rs" => resolve_rust(root, from, spec, id),
        "py" => {
            let dots = spec.chars().take_while(|c| *c == '.').count();
            let path = spec[dots..].replace('.', "/");
            let mut bases: Vec<String> = vec![];
            if dots > 0 {
                let mut d = dir.to_string();
                for _ in 1..dots {
                    d = parent(&d).to_string();
                }
                bases.push(if path.is_empty() { d } else { join(&d, &path) });
            } else {
                bases.push(path.clone());
                bases.push(format!("src/{path}"));
                bases.push(join(dir, &path));
            }
            let c: Vec<String> = bases
                .iter()
                .flat_map(|b| {
                    [
                        format!("{b}.py"),
                        format!("{b}/__init__.py"),
                        format!("{b}.pyi"),
                    ]
                })
                .collect();
            first(id, &c)
        }
        "go" => {
            // Module paths end with the package folder: match the longest existing suffix.
            let segs: Vec<&str> = spec.split('/').collect();
            for k in 0..segs.len() {
                let tail = segs[k..].join("/");
                let hits: Vec<usize> = id
                    .iter()
                    .filter(|(f, _)| parent(f) == tail || parent(f).ends_with(&format!("/{tail}")))
                    .filter(|(f, _)| f.ends_with(".go") && !f.ends_with("_test.go"))
                    .map(|(_, &i)| i)
                    .take(24)
                    .collect();
                if !hits.is_empty() {
                    return hits;
                }
            }
            vec![]
        }
        "java" | "kt" => {
            let path = spec.trim_end_matches(".*").replace('.', "/");
            let mut hits = by_suffix(by_name, files, &format!("{path}.java"));
            hits.extend(by_suffix(by_name, files, &format!("{path}.kt")));
            hits
        }
        "c" => {
            let mut hits = first(id, &[join(dir, spec)]);
            if hits.is_empty() {
                hits = by_suffix(by_name, files, spec.trim_start_matches("./"));
            }
            hits
        }
        "php" => first(id, &[join(dir, spec.trim_start_matches('/'))]),
        "rb" => first(id, &[join(dir, &format!("{spec}.rb")), join(dir, spec)]),
        _ => {
            // Godot `res://` paths and scene dependencies are already project-relative.
            let path = spec.trim_start_matches("res://");
            first(id, &[path.to_string()])
        }
    }
}

/// Module path of a Rust `use` (`crate::a::b::C`, `super::x`, `mod x`) to a file.
fn resolve_rust(root: &Path, from: &str, spec: &str, id: &HashMap<String, usize>) -> Vec<usize> {
    let dir = parent(from);
    let file = from.rsplit('/').next().unwrap_or(from);
    // Folder holding this file's child modules.
    let own = if ["mod.rs", "lib.rs", "main.rs"].contains(&file) {
        dir.to_string()
    } else {
        join(dir, file.trim_end_matches(".rs"))
    };
    if let Some(name) = spec.strip_prefix("mod ") {
        return first(
            id,
            &[format!("{own}/{name}.rs"), format!("{own}/{name}/mod.rs")]
                .map(|p| p.trim_start_matches('/').to_string()),
        );
    }
    let segs: Vec<&str> = spec.split("::").collect();
    let (base, rest) = match segs.first().copied() {
        Some("crate") => {
            // The crate's src folder: nearest ancestor with a Cargo.toml.
            let mut d = dir.to_string();
            loop {
                if root.join(&d).join("Cargo.toml").is_file() {
                    break (join(&d, "src"), &segs[1..]);
                }
                if d.is_empty() {
                    break ("src".to_string(), &segs[1..]);
                }
                d = parent(&d).to_string();
            }
        }
        Some("super") => (parent(&own).to_string(), &segs[1..]),
        Some("self") => (own.clone(), &segs[1..]),
        _ => return vec![],
    };
    let mut found = vec![];
    let mut path = base;
    for seg in rest {
        path = join(&path, seg);
        let hit = first(id, &[format!("{path}.rs"), format!("{path}/mod.rs")]);
        if hit.is_empty() {
            break;
        }
        found = hit;
    }
    found
}

fn pagerank(imports: &[Vec<usize>], refs: &[Vec<(usize, f32)>]) -> Vec<f32> {
    let n = imports.len();
    if n == 0 {
        return vec![];
    }
    let edges: Vec<Vec<(usize, f32)>> = (0..n)
        .map(|f| {
            let mut e: HashMap<usize, f32> = refs[f].iter().copied().collect();
            for &g in &imports[f] {
                *e.entry(g).or_default() += 1.0;
            }
            let total: f32 = e.values().sum();
            e.into_iter()
                .map(|(g, w)| (g, w / total.max(1e-6)))
                .collect()
        })
        .collect();
    let mut rank = vec![1.0 / n as f32; n];
    for _ in 0..30 {
        let mut next = vec![0.15 / n as f32; n];
        let mut dangling = 0.0;
        for f in 0..n {
            if edges[f].is_empty() {
                dangling += rank[f];
            }
            for &(g, w) in &edges[f] {
                next[g] += 0.85 * rank[f] * w;
            }
        }
        for v in &mut next {
            *v += 0.85 * dangling / n as f32;
        }
        rank = next;
    }
    rank
}

fn word(name: &str) -> Option<Regex> {
    Regex::new(&format!(r"\b{}\b", regex::escape(name))).ok()
}

impl Graph {
    fn rel(&self, path: &str) -> Option<usize> {
        let p = path.replace('\\', "/");
        let root = self.root.to_string_lossy().replace('\\', "/");
        let p = p
            .strip_prefix(&root)
            .map(|x| x.trim_start_matches('/').to_string())
            .unwrap_or(p)
            .trim_start_matches("./")
            .to_string();
        self.id.get(&p).copied().or_else(|| {
            // A unique file-name or suffix match is accepted too.
            let hits: Vec<usize> = self
                .files
                .iter()
                .enumerate()
                .filter(|(_, f)| f.ends_with(&format!("/{p}")))
                .map(|(i, _)| i)
                .collect();
            (hits.len() == 1).then(|| hits[0])
        })
    }

    fn read(&self, f: usize) -> Vec<String> {
        std::fs::read_to_string(self.root.join(&self.files[f]))
            .map(|t| t.lines().map(str::to_string).collect())
            .unwrap_or_default()
    }

    fn loc(&self, s: &Symbol) -> String {
        if s.end > s.line {
            format!("{}:{}-{}", s.file, s.line, s.end)
        } else {
            format!("{}:{}", s.file, s.line)
        }
    }

    /// Innermost symbol of file `f` whose span holds `line`.
    fn enclosing(&self, f: usize, line: u32) -> Option<&Symbol> {
        self.symbols[f]
            .iter()
            .filter(|s| s.line <= line && line <= s.end.max(s.line))
            .min_by_key(|s| s.end.max(s.line) - s.line)
    }

    fn top_symbols(&self, f: usize, n: usize) -> Vec<&Symbol> {
        let syms = &self.symbols[f];
        let mut top: Vec<&Symbol> = syms
            .iter()
            .filter(|s| {
                !syms
                    .iter()
                    .any(|o| !std::ptr::eq(o, *s) && o.line < s.line && s.end <= o.end)
            })
            .collect();
        // Most used first.
        top.sort_by_key(|s| std::cmp::Reverse(self.users.get(&s.name).map_or(0, Vec::len)));
        top.truncate(n);
        top
    }

    pub fn overview(&self, limit: usize) -> String {
        let mut langs: BTreeMap<&str, usize> = BTreeMap::new();
        for l in &self.lang {
            *langs.entry(l.as_str()).or_default() += 1;
        }
        let symbols: usize = self.symbols.iter().map(Vec::len).sum();
        let mut out = format!(
            "{} files, {} symbols. Languages: {}\n",
            self.files.len(),
            symbols,
            langs
                .iter()
                .map(|(l, n)| format!("{l} {n}"))
                .collect::<Vec<_>>()
                .join(", ")
        );
        let mut dirs: BTreeMap<String, (usize, f32)> = BTreeMap::new();
        for (i, f) in self.files.iter().enumerate() {
            let d = f.split('/').take(2).collect::<Vec<_>>();
            let key = if d.len() > 1 {
                d[0].to_string() + "/"
            } else {
                "./".into()
            };
            let e = dirs.entry(key).or_default();
            e.0 += 1;
            e.1 += self.rank[i];
        }
        let mut dirs: Vec<_> = dirs.into_iter().collect();
        dirs.sort_by(|a, b| b.1 .1.total_cmp(&a.1 .1));
        out.push_str("\nAreas by importance:\n");
        for (d, (n, _)) in dirs.iter().take(12) {
            out.push_str(&format!("- {d} ({n} files)\n"));
        }
        out.push_str("\nCentral files (most depended on), with their main symbols:\n");
        let mut order: Vec<usize> = (0..self.files.len()).collect();
        order.sort_by(|a, b| self.rank[*b].total_cmp(&self.rank[*a]));
        for f in order.into_iter().take(limit) {
            let syms = self
                .top_symbols(f, 5)
                .iter()
                .map(|s| format!("{} ({})", s.name, s.kind))
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&format!(
                "- {} · used by {} files · {}\n",
                self.files[f],
                self.refs_in[f].len().max(self.importers[f].len()),
                syms
            ));
        }
        out
    }

    pub fn find_symbol(&self, query: &str, kind: Option<&str>, limit: usize) -> String {
        let q = query.trim();
        if q.is_empty() {
            return "Give a name or part of one.".into();
        }
        let ql = q.to_lowercase();
        let mut hits: Vec<(u8, f32, &Symbol)> = vec![];
        for (f, syms) in self.symbols.iter().enumerate() {
            for s in syms {
                if kind.is_some_and(|k| !s.kind.eq_ignore_ascii_case(k)) {
                    continue;
                }
                let n = s.name.to_lowercase();
                let r = if s.name == q {
                    0
                } else if n == ql {
                    1
                } else if n.starts_with(&ql) {
                    2
                } else if n.contains(&ql) {
                    3
                } else {
                    continue;
                };
                hits.push((r, self.rank[f], s));
            }
        }
        if hits.is_empty() {
            return format!("No symbol matches `{q}`. Try a shorter part of the name.");
        }
        hits.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.total_cmp(&a.1)));
        let total = hits.len();
        let mut out: String = hits
            .into_iter()
            .take(limit)
            .map(|(_, _, s)| {
                format!(
                    "{} ({}) {} — {}\n",
                    s.name,
                    s.kind,
                    self.loc(s),
                    s.signature
                )
            })
            .collect();
        if total > limit {
            out.push_str(&format!(
                "… {} more; narrow the query or pass a kind.\n",
                total - limit
            ));
        }
        out
    }

    fn definitions(&self, name: &str, file: Option<&str>) -> Vec<(usize, &Symbol)> {
        let want = file.and_then(|p| self.rel(p));
        let mut d: Vec<(usize, &Symbol)> = self
            .defs
            .get(name)
            .into_iter()
            .flatten()
            .filter(|(f, _)| want.is_none_or(|w| w == *f))
            .map(|&(f, k)| (f, &self.symbols[f][k]))
            .collect();
        if d.is_empty() {
            // `Type::method`, `Type.method` and schema-qualified SQL names.
            if let Some((_, short)) = name.rsplit_once("::").or_else(|| name.rsplit_once('.')) {
                return self.definitions(short, file);
            }
        }
        d.sort_by(|a, b| self.rank[b.0].total_cmp(&self.rank[a.0]));
        d
    }

    pub fn symbol(&self, name: &str, file: Option<&str>, body: bool) -> String {
        let defs = self.definitions(name, file);
        if defs.is_empty() {
            return format!("`{name}` is not a known definition. find_symbol lists close names.");
        }
        let short = name.rsplit("::").next().unwrap_or(name);
        let short = short.rsplit('.').next().unwrap_or(short);
        let mut out = String::new();
        if defs.len() > 1 {
            out.push_str(&format!(
                "{} definitions; the most central first.\n",
                defs.len()
            ));
        }
        for (f, s) in defs.iter().take(3) {
            out.push_str(&format!(
                "\n{} ({}) {}\n  {}\n",
                s.name,
                s.kind,
                self.loc(s),
                s.signature
            ));
            if let Some(parent) = self.symbols[*f]
                .iter()
                .filter(|o| o.line < s.line && s.end <= o.end)
                .min_by_key(|o| o.end - o.line)
            {
                out.push_str(&format!(
                    "  inside {} ({}) {}\n",
                    parent.name,
                    parent.kind,
                    self.loc(parent)
                ));
            }
            let lines = self.read(*f);
            let (a, b) = (
                s.line as usize - 1,
                (s.end.max(s.line) as usize).min(lines.len()),
            );
            if body && b > a {
                if b - a <= MAX_BODY_LINES {
                    out.push_str("  body:\n");
                    for (i, l) in lines[a..b].iter().enumerate() {
                        out.push_str(&format!("  {:>5} {}\n", a + i + 1, l));
                    }
                } else {
                    out.push_str(&format!(
                        "  body is {} lines: Read {} with an offset instead.\n",
                        b - a,
                        s.file
                    ));
                }
            }
            // What it uses: identifiers in its body that name other definitions.
            if b > a {
                let mut seen = HashSet::new();
                let mut uses = vec![];
                for l in &lines[a..b] {
                    for m in ident().find_iter(l) {
                        let n = m.as_str();
                        if n == s.name || !seen.insert(n.to_string()) {
                            continue;
                        }
                        if let Some(d) = self.defs.get(n) {
                            let fam = family(&self.lang[*f]);
                            let d: Vec<&(usize, usize)> = d
                                .iter()
                                .filter(|x| {
                                    let l = self.lang[x.0].as_str();
                                    x.0 == *f
                                        || ((family(l) == fam || l == "sql")
                                            && (distinctive(n) || self.imports[*f].contains(&x.0))
                                            && visible_outside(l, &self.symbols[x.0][x.1]))
                                })
                                .collect();
                            if d.is_empty() || d.len() > MAX_DEF_FILES * 2 {
                                continue;
                            }
                            // A qualifier (`providers::execute`, `store.save`) names the file;
                            // otherwise prefer this file, then imported files.
                            let qualifier = l[..m.start()]
                                .strip_suffix("::")
                                .or_else(|| l[..m.start()].strip_suffix('.'))
                                .and_then(|p| {
                                    p.rsplit(|c: char| !c.is_alphanumeric() && c != '_').next()
                                })
                                .filter(|q| !q.is_empty());
                            let stem = |g: usize| {
                                let f = &self.files[g];
                                let name = f.rsplit('/').next().unwrap_or(f);
                                name.split('.').next().unwrap_or(name).to_string()
                            };
                            let pick = qualifier
                                .and_then(|q| d.iter().find(|x| stem(x.0).eq_ignore_ascii_case(q)))
                                .or_else(|| d.iter().find(|x| x.0 == *f))
                                .or_else(|| d.iter().find(|x| self.imports[*f].contains(&x.0)))
                                .unwrap_or(&d[0]);
                            uses.push(&self.symbols[pick.0][pick.1]);
                        }
                    }
                }
                if !uses.is_empty() {
                    out.push_str(&format!("  uses ({}):\n", uses.len()));
                    for u in uses.iter().take(25) {
                        out.push_str(&format!("   - {} ({}) {}\n", u.name, u.kind, self.loc(u)));
                    }
                }
            }
        }
        // Who uses it: call sites grouped by the enclosing symbol.
        let Some(re) = word(short) else {
            return out;
        };
        let mut callers: Vec<(usize, Option<&Symbol>, u32, String)> = vec![];
        let mut users: Vec<usize> = self.users.get(short).cloned().unwrap_or_default();
        users.sort_by(|a, b| self.rank[*b].total_cmp(&self.rank[*a]));
        let def_lines: HashSet<(usize, u32)> = defs.iter().map(|(f, s)| (*f, s.line)).collect();
        let mut sites = 0;
        for f in users.into_iter().take(60) {
            for (i, l) in self.read(f).iter().enumerate() {
                let line = i as u32 + 1;
                if def_lines.contains(&(f, line)) || !re.is_match(l) {
                    continue;
                }
                sites += 1;
                if callers.len() < 30 {
                    callers.push((
                        f,
                        self.enclosing(f, line),
                        line,
                        l.trim().chars().take(120).collect(),
                    ));
                }
            }
        }
        if sites == 0 {
            out.push_str("\nNo uses found outside its definition.\n");
        } else {
            out.push_str(&format!(
                "\nUsed at {sites} sites{}:\n",
                if sites > callers.len() {
                    " (most central shown)"
                } else {
                    ""
                }
            ));
            for (f, enc, line, text) in callers {
                let at = enc.map_or(String::new(), |e| format!(" in {} ({})", e.name, e.kind));
                out.push_str(&format!("- {}:{}{} — {}\n", self.files[f], line, at, text));
            }
        }
        out
    }

    pub fn file_outline(&self, path: &str) -> String {
        let Some(f) = self.rel(path) else {
            return format!("`{path}` is not an indexed source file.");
        };
        let mut out = format!(
            "{} ({}, {} symbols)\n",
            self.files[f],
            self.lang[f],
            self.symbols[f].len()
        );
        let mut stack: Vec<u32> = vec![];
        for s in &self.symbols[f] {
            while stack.last().is_some_and(|end| s.line > *end) {
                stack.pop();
            }
            let pad = "  ".repeat(stack.len() + 1);
            let span = if s.end > s.line {
                format!("L{}-{}", s.line, s.end)
            } else {
                format!("L{}", s.line)
            };
            out.push_str(&format!(
                "{pad}{} ({}) {span} — {}\n",
                s.name, s.kind, s.signature
            ));
            if s.end > s.line {
                stack.push(s.end);
            }
        }
        let list = |ids: &[usize]| {
            ids.iter()
                .take(20)
                .map(|&g| self.files[g].as_str())
                .collect::<Vec<_>>()
                .join(", ")
        };
        if !self.imports[f].is_empty() {
            out.push_str(&format!("imports: {}\n", list(&self.imports[f])));
        }
        if !self.importers[f].is_empty() {
            out.push_str(&format!("imported by: {}\n", list(&self.importers[f])));
        }
        let top = |v: &[(usize, f32)]| {
            v.iter()
                .take(12)
                .map(|(g, _)| self.files[*g].as_str())
                .collect::<Vec<_>>()
                .join(", ")
        };
        if !self.refs_out[f].is_empty() {
            out.push_str(&format!("uses symbols from: {}\n", top(&self.refs_out[f])));
        }
        if !self.refs_in[f].is_empty() {
            out.push_str(&format!(
                "its symbols are used by: {}\n",
                top(&self.refs_in[f])
            ));
        }
        out
    }

    pub fn related(&self, path: &str, limit: usize) -> String {
        let Some(f) = self.rel(path) else {
            return format!("`{path}` is not an indexed source file.");
        };
        let mut score: HashMap<usize, (f32, Vec<&str>)> = HashMap::new();
        let mut add = |g: usize, w: f32, why: &'static str| {
            if g != f {
                let e = score.entry(g).or_default();
                e.0 += w;
                if !e.1.contains(&why) {
                    e.1.push(why);
                }
            }
        };
        for &g in &self.imports[f] {
            add(g, 3.0, "imported");
        }
        for &g in &self.importers[f] {
            add(g, 3.0, "imports it");
        }
        for &(g, w) in &self.refs_out[f] {
            add(g, 1.0 + w.min(4.0), "uses its symbols");
        }
        for &(g, w) in &self.refs_in[f] {
            add(g, 1.0 + w.min(4.0), "uses this file's symbols");
        }
        let co = cochange(&self.root);
        if let Some(partners) = co.get(&self.files[f]) {
            let max = partners.values().copied().max().unwrap_or(1) as f32;
            for (other, n) in partners {
                if let Some(&g) = self.id.get(other) {
                    add(g, 3.0 * *n as f32 / max, "changed together in git");
                }
            }
        }
        let mut list: Vec<_> = score.into_iter().collect();
        list.sort_by(|a, b| b.1 .0.total_cmp(&a.1 .0));
        if list.is_empty() {
            return format!("{} has no known relations.", self.files[f]);
        }
        let mut out = format!("Files related to {}:\n", self.files[f]);
        for (g, (_, why)) in list.into_iter().take(limit) {
            out.push_str(&format!("- {} — {}\n", self.files[g], why.join(", ")));
        }
        out
    }

    pub fn impact_details(&self, targets: &[String], depth: usize) -> Impact {
        let mut seeds: Vec<usize> = vec![];
        let mut notes = vec![];
        let mut unknown = vec![];
        for t in targets {
            if let Some(f) = self.rel(t) {
                seeds.push(f);
            } else if !self.definitions(t, None).is_empty() {
                let short = t.rsplit("::").next().unwrap_or(t);
                seeds.extend(self.definitions(t, None).iter().map(|d| d.0));
                // Files that mention the symbol are affected directly.
                notes.extend(self.users.get(short).cloned().unwrap_or_default());
            } else {
                unknown.push(t.clone());
            }
        }
        let mut dist: HashMap<usize, usize> = seeds.iter().map(|&s| (s, 0)).collect();
        for f in notes {
            dist.entry(f).or_insert(1);
        }
        let mut initial: Vec<_> = dist.keys().copied().collect();
        initial.sort_by_key(|f| dist[f]);
        let mut queue: VecDeque<usize> = initial.into();
        while let Some(f) = queue.pop_front() {
            let d = dist[&f];
            if d >= depth.clamp(1, 4) {
                continue;
            }
            let next = self.importers[f]
                .iter()
                .copied()
                .filter(|&g| !self.mods[g].contains(&f) || self.refs_in[f].iter().any(|x| x.0 == g))
                .chain(self.refs_in[f].iter().map(|x| x.0));
            for g in next {
                if dist.get(&g).is_none_or(|old| *old > d + 1) {
                    dist.insert(g, d + 1);
                    queue.push_back(g);
                }
            }
        }
        let mut list: Vec<(usize, usize)> = dist.into_iter().collect();
        list.sort_by(|a, b| {
            a.1.cmp(&b.1)
                .then(self.rank[b.0].total_cmp(&self.rank[a.0]))
                .then(self.files[a.0].cmp(&self.files[b.0]))
        });
        let contracts = seeds
            .iter()
            .flat_map(|&f| {
                self.symbols[f]
                    .iter()
                    .filter(move |s| {
                        visible_outside(&self.lang[f], s)
                            && referable(&s.kind)
                            && self
                                .users
                                .get(&s.name)
                                .is_some_and(|users| users.iter().any(|&g| g != f))
                    })
                    .map(move |s| {
                        format!(
                            "{}:{} {}",
                            self.files[f],
                            s.line,
                            s.signature.chars().take(300).collect::<String>()
                        )
                    })
            })
            .take(20)
            .collect();
        Impact {
            targets: targets.to_vec(),
            files: list
                .into_iter()
                .map(|(f, d)| ImpactFile {
                    path: self.files[f].clone(),
                    distance: d,
                    test: test_path(&self.files[f]),
                })
                .collect(),
            unknown,
            contracts,
            truncated: self.partial,
        }
    }

    pub fn impact(&self, targets: &[String], depth: usize) -> String {
        self.impact_details(targets, depth).text()
    }
}

/// File → files changed in the same commits, from recent history, cached per HEAD.
fn cochange(root: &Path) -> Arc<CoChange> {
    type Heads = Mutex<HashMap<PathBuf, (String, Arc<CoChange>)>>;
    static C: OnceLock<Heads> = OnceLock::new();
    let git = |args: &[&str]| -> Option<String> {
        let mut c = crate::proc::std_command("git");
        c.arg("-C").arg(root).args(args);
        let out = c.output().ok()?;
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
    };
    let Some(head) = git(&["rev-parse", "HEAD"]) else {
        return Arc::default();
    };
    let cache = C.get_or_init(Default::default);
    if let Some((h, m)) = cache.lock().unwrap_or_else(|e| e.into_inner()).get(root) {
        if *h == head {
            return m.clone();
        }
    }
    let log = git(&[
        "log",
        "--relative",
        "--no-merges",
        "-n",
        "400",
        "--name-only",
        "--format=%x1e",
    ])
    .unwrap_or_default();
    let mut m: HashMap<String, HashMap<String, u32>> = HashMap::new();
    for commit in log.split('\u{1e}') {
        let files: Vec<&str> = commit
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect();
        if files.len() < 2 || files.len() > 20 {
            continue;
        }
        for a in &files {
            for b in &files {
                if a != b {
                    *m.entry(a.to_string())
                        .or_default()
                        .entry(b.to_string())
                        .or_default() += 1;
                }
            }
        }
    }
    let m = Arc::new(m);
    cache
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(root.to_path_buf(), (head, m.clone()));
    m
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
        let base = std::env::temp_dir().join(format!("lumen-graph-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let root = base.join("p");
        write(&root, "Cargo.toml", "[package]\nname=\"x\"\n");
        write(
            &root,
            "src/main.rs",
            "mod store;\nmod api;\nfn main() {\n    api::serve();\n}\n",
        );
        write(
            &root,
            "src/store.rs",
            "pub struct Store {\n    rows: Vec<u8>,\n}\n\nimpl Store {\n    pub fn save_row(&self) {\n        validate_row();\n    }\n}\n\nfn validate_row() {}\n",
        );
        write(
            &root,
            "src/api.rs",
            "use crate::store::Store;\n\npub fn serve() {\n    let s = Store { rows: vec![] };\n    s.save_row();\n}\n",
        );
        write(
            &root,
            "web/util.ts",
            "export function formatPrice(n: number) {\n  return `${n}`;\n}\n",
        );
        write(&root, "web/cart.ts", "import { formatPrice } from './util';\nexport class Cart {\n  total() {\n    return formatPrice(1);\n  }\n}\n");
        write(&root, "tools/report.py", "from tools import helpers\n\nclass Report:\n    def render(self):\n        return helpers.fmt()\n\nx = 1\n");
        write(&root, "tools/helpers.py", "def fmt():\n    return ''\n");
        (base.join("data"), root)
    }

    #[test]
    fn roadmap_typescript_and_godot_import_corpus() {
        let fixture = crate::test_support::Fixture::new();
        let root = fixture.directory("project");
        let data = fixture.directory("index");
        for (path, text) in [
            ("web/state.mts", "export const count = 1;\n"),
            ("web/common.cts", "export const count = 2;\n"),
            ("web/source.ts", "export const count = 3;\n"),
            ("web/source.js", "export const count = 4;\n"),
            (
                "web/typed.mts",
                "import { count as x } from './state.mjs';\nexport const value = x;\n",
            ),
            (
                "web/common-user.cts",
                "import { count as x } from './common.cjs';\nexport const value = x;\n",
            ),
            (
                "web/typed-user.ts",
                "import { count as x } from './source.js';\nexport const value = x;\n",
            ),
            (
                "web/native-user.js",
                "import { count as x } from './source.js';\nexport const value = x;\n",
            ),
            (
                "web/missing.js",
                "import { count as x } from './state.mjs';\nexport const value = x;\n",
            ),
            ("outside.ts", "export const count = 5;\n"),
            (
                "web/escape.ts",
                "import { count as x } from '../../outside.ts';\nexport const value = x;\n",
            ),
            ("game/project.godot", "config_version=5\n"),
            ("game/base.gd", "extends Node\n"),
            (
                "game/scripts/quoted.gd",
                "extends Node\nconst value = preload('res://base.gd')\n",
            ),
            ("game/scripts/derived.gd", "extends '../base.gd'\n"),
            ("game/scripts/escape.gd", "extends '../../outside.gd'\n"),
            ("outside.gd", "extends Node\n"),
            (
                "game/CounterState.cs",
                "public class CounterState {\n public int Count;\n}\n",
            ),
            (
                "game/scripts/managed.gd",
                "extends Node\nvar state: CounterState\n",
            ),
        ] {
            write(&root, path, text);
        }
        let graph = graph(&data, &root);
        let mut failures = vec![];
        for (from, target, expected) in [
            ("web/typed.mts", "web/state.mts", true),
            ("web/common-user.cts", "web/common.cts", true),
            ("web/typed-user.ts", "web/source.ts", true),
            ("web/typed-user.ts", "web/source.js", false),
            ("web/native-user.js", "web/source.js", true),
            ("web/native-user.js", "web/source.ts", false),
            ("web/missing.js", "web/state.mts", false),
            ("web/escape.ts", "outside.ts", false),
            ("game/scripts/quoted.gd", "game/base.gd", true),
            ("game/scripts/derived.gd", "game/base.gd", true),
            ("game/scripts/escape.gd", "outside.gd", false),
        ] {
            let actual = graph.imports[graph.id[from]].contains(&graph.id[target]);
            if actual != expected {
                failures.push(format!(
                    "{from} -> {target}: expected {expected}, got {actual}"
                ));
            }
            let impact = graph.impact_details(&[target.into()], 1);
            let actual = impact
                .files
                .iter()
                .any(|f| f.path == from && f.distance == 1);
            if actual != expected {
                failures.push(format!(
                    "impact({target}) -> {from}: expected {expected}, got {actual}"
                ));
            }
        }
        let managed = graph.impact_details(&["game/CounterState.cs".into()], 1);
        if !managed
            .files
            .iter()
            .any(|f| f.path == "game/scripts/managed.gd")
        {
            failures.push("C# type -> GDScript impact was lost".into());
        }
        assert!(
            failures.is_empty(),
            "{} corpus failures:\n{}",
            failures.len(),
            failures.join("\n")
        );
    }

    #[test]
    fn roadmap_comments_and_normalized_resource_paths() {
        let f = crate::test_support::Fixture::new();
        let root = f.directory("project");
        let data = f.directory("index");
        for (path, text) in [
            ("game/project.godot", "config_version=5\n"),
            ("game/base.gd", "extends Node\n"),
            ("game/notes.gd", "# preload(\"res://base.gd\")\n# class_name ImaginaryType\n"),
            ("game/notes.cs", "// public class ImaginaryType {}\n"),
            ("web/base.ts", "export const count = 1;\n"),
            ("web/view.jsx", "export const count = 2;\n"),
            ("web/view-user.ts", "import {count as x} from './view.js';\nexport const value = x;\n"),
            ("web/notes.ts", "// import { count } from './base';\n// export class ImaginaryType {}\n"),
            ("game/scenes/Main.tscn", "[gd_scene load_steps=2 format=3]\n[ext_resource type=\"Script\" path=\"res://scripts/../base.gd\" id=\"1\"]\n[node name=\"Main\" type=\"Node\"]\nscript = ExtResource(\"1\")\n"),
        ] { write(&root, path, text); }
        let g = graph(&data, &root);
        for path in ["game/notes.gd", "game/notes.cs", "web/notes.ts"] {
            assert!(g.imports[g.id[path]].is_empty(), "{path}");
            assert!(g.symbols[g.id[path]].is_empty(), "{path}");
            assert!(g.refs_out[g.id[path]].is_empty(), "{path}");
        }
        assert_eq!(
            g.imports[g.id["game/scenes/Main.tscn"]],
            vec![g.id["game/base.gd"]]
        );
        assert_eq!(
            g.imports[g.id["web/view-user.ts"]],
            vec![g.id["web/view.jsx"]]
        );
    }

    #[test]
    fn roadmap_graph_refreshes_edited_imports_renames_and_deletions() {
        let f = crate::test_support::Fixture::new();
        let root = f.directory("project");
        let data = f.directory("index");
        for (path, text) in [
            ("web/state.mts", "export const count = 1;\n"),
            (
                "web/use.test.mts",
                "import { count as x } from './state.mjs';\nexport const value = x;\n",
            ),
            ("game/project.godot", "config_version=5\n"),
            ("game/base.gd", "extends Node\n"),
            ("game/scripts/derived.gd", "extends '../base.gd'\n"),
        ] {
            write(&root, path, text);
        }
        let first = graph(&data, &root);
        crate::repomap::with_index(&data, &root, |_, stats| assert_eq!(stats.refreshed, 0));
        assert!(Arc::ptr_eq(&first, &graph(&data, &root)));
        std::fs::rename(root.join("web/state.mts"), root.join("web/renamed.mts")).unwrap();
        std::fs::rename(root.join("game/base.gd"), root.join("game/renamed_base.gd")).unwrap();
        write(
            &root,
            "web/use.test.mts",
            "import { count as x } from './renamed.mjs';\nexport const value = x;\n",
        );
        write(
            &root,
            "game/scripts/derived.gd",
            "extends '../renamed_base.gd'\n",
        );
        let renamed = graph(&data, &root);
        assert!(
            !renamed.id.contains_key("web/state.mts") && !renamed.id.contains_key("game/base.gd")
        );
        assert_eq!(
            renamed.imports[renamed.id["web/use.test.mts"]],
            vec![renamed.id["web/renamed.mts"]]
        );
        assert_eq!(
            renamed.imports[renamed.id["game/scripts/derived.gd"]],
            vec![renamed.id["game/renamed_base.gd"]]
        );
        let impact = renamed.impact_details(&["web/renamed.mts".into()], 1);
        assert!(impact
            .files
            .iter()
            .any(|f| f.path == "web/use.test.mts" && f.test && f.distance == 1));
        std::fs::remove_file(root.join("web/renamed.mts")).unwrap();
        std::fs::remove_file(root.join("game/renamed_base.gd")).unwrap();
        let deleted = graph(&data, &root);
        assert!(deleted.imports[deleted.id["web/use.test.mts"]].is_empty());
        assert!(deleted.imports[deleted.id["game/scripts/derived.gd"]].is_empty());
        assert_eq!(
            deleted
                .impact_details(&["web/renamed.mts".into()], 1)
                .unknown,
            vec!["web/renamed.mts"]
        );
        assert!(
            first.id.contains_key("web/state.mts"),
            "Pre-edit snapshots stay immutable"
        );
    }

    #[test]
    fn spans_imports_references_and_impact() {
        let (data, root) = fixture("all");
        let g = graph(&data, &root);
        let store = g.id["src/store.rs"];
        let api = g.id["src/api.rs"];
        let s = g.symbols[store].iter().find(|s| s.name == "Store").unwrap();
        assert_eq!((s.line, s.end), (1, 3));
        let save = g.symbols[store]
            .iter()
            .find(|s| s.name == "save_row")
            .unwrap();
        assert_eq!((save.line, save.end, save.kind.as_str()), (6, 8, "method"));
        assert!(g.imports[api].contains(&store), "crate:: path resolves");
        assert!(
            g.imports[g.id["src/main.rs"]].contains(&api),
            "mod x; resolves"
        );
        assert!(g.imports[g.id["web/cart.ts"]].contains(&g.id["web/util.ts"]));
        assert!(g.imports[g.id["tools/report.py"]].contains(&g.id["tools/helpers.py"]));
        let render = g.symbols[g.id["tools/report.py"]]
            .iter()
            .find(|s| s.name == "render")
            .unwrap();
        assert_eq!((render.line, render.end), (4, 5));
        assert!(g.refs_out[api].iter().any(|(f, _)| *f == store));

        let sym = g.symbol("save_row", None, true);
        assert!(sym.contains("src/store.rs:6-8"), "{sym}");
        assert!(sym.contains("inside Store"), "{sym}");
        assert!(sym.contains("validate_row (fn)"), "callees: {sym}");
        assert!(sym.contains("src/api.rs:5 in serve (fn)"), "callers: {sym}");

        let imp = g.impact(&["src/store.rs".into()], 2);
        assert!(
            imp.contains("[1] src/api.rs") && imp.contains("[2] src/main.rs"),
            "{imp}"
        );
        let far = g.impact(&["web/util.ts".into()], 2);
        assert!(
            far.contains("[1] web/cart.ts") && !far.contains("src/"),
            "{far}"
        );
        let outline = g.file_outline("store.rs");
        assert!(outline.contains("    save_row (method) L6-8"), "{outline}");
        assert!(g
            .find_symbol("formatp", None, 5)
            .starts_with("formatPrice (function) web/util.ts:1-3"));
        assert!(g.overview(5).contains("Central files"));
        let _ = std::fs::remove_dir_all(data.parent().unwrap());
    }
}
