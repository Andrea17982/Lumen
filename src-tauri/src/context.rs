//! Shared project context: LUMEN.md, attached files, log compression and verification.
use std::{
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
};

pub const CONTEXT_FILE: &str = "LUMEN.md";

/// Files that waste context and almost never help a model.
pub fn is_noise(rel: &str) -> bool {
    let lower = rel.replace('\\', "/").to_lowercase();
    let name = lower.rsplit('/').next().unwrap_or(&lower);
    [
        "package-lock.json",
        "yarn.lock",
        "pnpm-lock.yaml",
        "cargo.lock",
        "poetry.lock",
        "composer.lock",
        "gemfile.lock",
        "bun.lockb",
    ]
    .contains(&name)
        || name.ends_with(".min.js")
        || name.ends_with(".map")
        || lower
            .split('/')
            .any(|p| ["dist", "build", "target", "node_modules", ".git"].contains(&p))
}

pub fn is_secret(rel: &str) -> bool {
    let lower = rel.replace('\\', "/").to_lowercase();
    let name = lower.rsplit('/').next().unwrap_or(&lower);
    (name == ".env" || name.starts_with(".env.")) && !name.ends_with(".example")
        || name.ends_with(".pem")
        || name.ends_with(".key")
        || name == "id_rsa"
        || name == "id_ed25519"
        || lower.contains(".ssh/")
        || lower.contains(".aws/")
}

pub fn read_context(workspace: &Path) -> String {
    std::fs::read_to_string(workspace.join(CONTEXT_FILE)).unwrap_or_default()
}

pub fn write_context(workspace: &Path, text: &str) -> Result<(), String> {
    if text.len() > 200_000 {
        return Err("Project notes are limited to 200 KB".into());
    }
    let path = workspace.join(CONTEXT_FILE);
    if text.trim().is_empty() {
        if path.exists() {
            std::fs::remove_file(path).map_err(|e| e.to_string())?;
        }
        return Ok(());
    }
    std::fs::write(path, text).map_err(|e| e.to_string())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub path: String,
    pub size: u64,
    pub tokens: u64,
    pub noise: bool,
}

/// Project files via ripgrep's walker: honors .gitignore and skips hidden/VCS folders.
pub fn list_files(workspace: &Path, limit: usize) -> Vec<FileEntry> {
    let mut out = vec![];
    for entry in ignore::WalkBuilder::new(workspace)
        .hidden(true)
        .git_ignore(true)
        .require_git(false)
        .max_depth(Some(12))
        .build()
        .flatten()
    {
        if out.len() >= limit {
            break;
        }
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        let Ok(rel) = entry.path().strip_prefix(workspace) else {
            continue;
        };
        let rel = rel.to_string_lossy().replace('\\', "/");
        if is_secret(&rel) {
            continue;
        }
        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
        out.push(FileEntry {
            noise: is_noise(&rel),
            tokens: estimate_bytes(size),
            path: rel,
            size,
        });
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

pub fn estimate_bytes(bytes: u64) -> u64 {
    bytes.div_ceil(4)
}
pub fn estimate(text: &str) -> u64 {
    // ~4 characters per token for English/code; non-ASCII text is denser.
    let ascii = text.bytes().filter(|b| b.is_ascii()).count() as u64;
    let other = text.chars().count() as u64 - ascii.min(text.chars().count() as u64);
    ascii.div_ceil(4) + other.div_ceil(2)
}

/// Safe path inside the workspace (no traversal, no secrets).
pub fn jail(workspace: &Path, rel: &str) -> Result<PathBuf, String> {
    let root = dunce::canonicalize(workspace).map_err(|e| e.to_string())?;
    let path =
        dunce::canonicalize(root.join(rel)).map_err(|_| format!("{rel} is not available"))?;
    if !path.starts_with(&root) {
        return Err(format!("{rel} is outside the project folder"));
    }
    if is_secret(rel) {
        return Err(format!("{rel} looks like a secret and is never attached"));
    }
    Ok(path)
}

pub fn attachments(workspace: &Path, files: &[String]) -> Result<String, String> {
    let mut out = String::new();
    let mut budget: usize = 400_000;
    for rel in files.iter().take(50) {
        let path = jail(workspace, rel)?;
        let text = std::fs::read(&path).map_err(|e| e.to_string())?;
        if text.contains(&0) {
            return Err(format!("{rel} is a binary file"));
        }
        let text = String::from_utf8_lossy(&text);
        if text.len() > budget {
            return Err("Attached files exceed 400 KB. Attach fewer files.".into());
        }
        budget -= text.len();
        out.push_str(&format!("\n<file path=\"{rel}\">\n{text}\n</file>\n"));
    }
    Ok(out)
}

fn is_progress(line: &str) -> bool {
    let t = line.trim();
    t.is_empty()
        || t.contains('\r')
        || t.starts_with("Compiling ")
        || t.starts_with("Downloaded ")
        || t.starts_with("Downloading ")
        || t.starts_with("Checking ")
        || t.starts_with("Fresh ")
        || t.starts_with("npm http")
        || t.chars().filter(|c| "█▓▒░=#>-.|/\\".contains(*c)).count() * 2 > t.chars().count().max(1)
}
fn is_signal(line: &str) -> bool {
    let l = line.to_lowercase();
    [
        "error",
        "fail",
        "panic",
        "exception",
        "traceback",
        "fatal",
        "cannot",
        "undefined",
        "denied",
        "expected",
        "warning:",
    ]
    .iter()
    .any(|k| l.contains(k))
}

/// Keeps the meaningful part of a long log; the full log goes to `.lumen/spill`.
pub fn compress_log(text: &str, max_lines: usize) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let mut keep = vec![false; lines.len()];
    for (i, l) in lines.iter().enumerate() {
        if is_signal(l) {
            for k in keep
                .iter_mut()
                .take((i + 4).min(lines.len()))
                .skip(i.saturating_sub(2))
            {
                *k = true;
            }
        }
    }
    let mut out: Vec<String> = vec![];
    let mut last = "";
    for (i, l) in lines.iter().enumerate() {
        if !keep[i] || is_progress(l) || *l == last {
            continue;
        }
        last = l;
        out.push(l.to_string());
    }
    if out.is_empty() {
        out = lines
            .iter()
            .rev()
            .take(max_lines)
            .rev()
            .map(|s| s.to_string())
            .collect();
    }
    if out.len() > max_lines {
        let tail = out.split_off(out.len() - max_lines / 2);
        out.truncate(max_lines / 2);
        out.push("…".into());
        out.extend(tail);
    }
    out.join("\n")
}

/// Replaces long pasted logs in a prompt with a compact excerpt plus a pointer to the full log.
pub fn spill(workspace: &Path, text: &str) -> (String, Option<String>) {
    if text.lines().count() < 400 {
        return (text.into(), None);
    }
    let mut h = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut h);
    let name = format!("{:016x}.log", h.finish());
    let dir = workspace.join(".lumen").join("spill");
    if std::fs::create_dir_all(&dir).is_err() {
        return (text.into(), None);
    }
    // The folder ignores itself so spill files never appear in git status.
    let _ = std::fs::write(workspace.join(".lumen").join(".gitignore"), "*\n");
    let path = dir.join(&name);
    if std::fs::write(&path, text).is_err() {
        return (text.into(), None);
    }
    let rel = format!(".lumen/spill/{name}");
    let excerpt = compress_log(text, 50);
    (
        format!(
            "{excerpt}\n\n[Lumen: the full {}-line log is saved at {rel}. Read only the line ranges you need.]",
            text.lines().count()
        ),
        Some(rel),
    )
}

/// Verification command: `verify: <cmd>` in LUMEN.md, otherwise detected from the project.
pub fn verify_command(workspace: &Path) -> Option<String> {
    for line in read_context(workspace).lines() {
        if let Some(cmd) = line.trim().strip_prefix("verify:") {
            let cmd = cmd.trim();
            return (!cmd.is_empty() && cmd != "off").then(|| cmd.to_string());
        }
    }
    if workspace.join("Cargo.toml").is_file() {
        return Some("cargo check --quiet".into());
    }
    if workspace.join("tsconfig.json").is_file() && workspace.join("node_modules").is_dir() {
        return Some("npx --no-install tsc --noEmit".into());
    }
    if workspace.join("go.mod").is_file() {
        return Some("go build ./...".into());
    }
    None
}

pub async fn run_shell(
    workspace: &Path,
    cmd: &str,
    mut cancel: tokio::sync::watch::Receiver<bool>,
) -> Result<(bool, String), String> {
    #[cfg(windows)]
    let mut c = {
        let mut c = tokio::process::Command::new("cmd.exe");
        c.args(["/D", "/S", "/C", cmd]).creation_flags(0x08000000);
        c
    };
    #[cfg(not(windows))]
    let mut c = {
        let mut c = tokio::process::Command::new("sh");
        c.args(["-c", cmd]);
        c
    };
    c.current_dir(workspace)
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true);
    let out = tokio::select! {
        biased;
        _ = cancel.changed() => return Err("Cancelled".into()),
        r = tokio::time::timeout(std::time::Duration::from_secs(600), c.output()) => r.map_err(|_| "Verification timed out after 10 minutes")?.map_err(|e| e.to_string())?,
    };
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
    Ok((out.status.success(), compress_log(&text, 60)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn log_compression_and_filters() {
        let mut log = String::new();
        for i in 0..5000 {
            log.push_str(&format!("   Compiling crate{i} v1.0\n"));
        }
        log.push_str("error[E0308]: mismatched types\n --> src/main.rs:3:5\n  expected u32\n");
        let small = compress_log(&log, 50);
        assert!(small.lines().count() < 50);
        assert!(small.contains("E0308"));
        assert!(is_noise("app/package-lock.json"));
        assert!(is_secret("config/.env.local"));
        assert!(!is_secret(".env.example"));
        assert!(estimate("hello world, this is code") > 3);
    }
}
