use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Claude,
    Gpt,
    Gemini,
    Antigravity,
    Ollama,
}
impl Provider {
    pub fn binary(&self) -> &str {
        match self {
            Self::Claude => "claude",
            Self::Gpt => "codex",
            Self::Gemini => "gemini",
            Self::Antigravity => "agy",
            Self::Ollama => "ollama",
        }
    }
    pub fn env(&self) -> &str {
        match self {
            Self::Claude => "CLAUDE_CONFIG_DIR",
            Self::Gpt => "CODEX_HOME",
            Self::Gemini => "GEMINI_CLI_HOME",
            Self::Antigravity => "HOME",
            Self::Ollama => "",
        }
    }
    /// Order for rate-limit and quota fallback: subscriptions first, local last.
    pub fn fallback_rank(&self) -> u8 {
        match self {
            Self::Claude | Self::Gpt => 0,
            Self::Antigravity => 1,
            Self::Gemini => 2,
            Self::Ollama => 3,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: String,
    pub name: String,
    pub provider: Provider,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: String,
    pub title: String,
    pub pinned: bool,
    pub status: String,
    pub workspace: String,
    pub updated_at: i64,
    pub workspace_id: Option<String>,
    pub access_mode: AccessMode,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub folder: String,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AccessMode {
    #[default]
    ReadOnly,
    FileEdits,
    Autopilot,
    /// Claude asks the user, through the Gatekeeper, before each action that needs permission.
    Ask,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub efforts: Vec<String>,
    pub default_effort: String,
    pub is_default: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub id: String,
    pub role: String,
    pub text: String,
    pub activity: Vec<Activity>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Activity {
    pub kind: String,
    pub text: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RunOptions {
    pub mode: String,
    pub account_id: Option<String>,
    pub model: Option<String>,
    pub effort: String,
    #[serde(default)]
    pub write_access: bool,
    #[serde(default)]
    pub auto_policy: AutoPolicy,
    #[serde(default)]
    pub balance_usage: bool,
    #[serde(default)]
    pub access_mode: Option<AccessMode>,
    /// Cross-audit the answer with the other provider.
    #[serde(default)]
    pub auto_review: bool,
    /// Workspace-relative files to include in the prompt.
    #[serde(default)]
    pub attachments: Vec<String>,
    /// Assistant message being audited; its turn's diff is attached.
    #[serde(default)]
    pub audit_of: Option<String>,
}
impl RunOptions {
    pub fn access(&self) -> AccessMode {
        self.access_mode.unwrap_or(if self.write_access {
            AccessMode::FileEdits
        } else {
            AccessMode::ReadOnly
        })
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub provider: Provider,
    pub account_id: String,
    pub model: Option<String>,
    pub effort: String,
    pub strategy: String,
    pub reason: String,
    pub partner_account: Option<String>,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunEvent {
    pub session_id: String,
    pub message_id: String,
    pub kind: String,
    pub text: String,
}
pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
pub fn id() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AutoPolicy {
    Budget,
    #[default]
    Default,
    Quality,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageWindow {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    pub used_percent: f64,
    pub resets_at: i64,
    pub window_minutes: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountUsage {
    pub account_id: String,
    pub windows: Vec<UsageWindow>,
    pub updated_at: i64,
    pub source: String,
}

/// The user's own CLI home (`~/.claude`, `~/.codex`), honouring an inherited override.
fn global_home(p: &Provider) -> Option<std::path::PathBuf> {
    let (var, dir) = match p {
        Provider::Claude => ("CLAUDE_CONFIG_DIR", ".claude"),
        Provider::Gpt => ("CODEX_HOME", ".codex"),
        _ => return None,
    };
    if let Some(v) = std::env::var_os(var).filter(|v| !v.is_empty()) {
        return Some(v.into());
    }
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })?;
    Some(std::path::PathBuf::from(home).join(dir))
}

/// Copies the user's global instructions and skills into an isolated account home, so a turn in
/// Lumen follows the same personal rules as the native CLI. Credentials and settings stay
/// isolated: only instruction files and skill/agent/command folders are mirrored.
pub fn share_global_instructions(p: &Provider, config: &std::path::Path) {
    let Some(home) = global_home(p) else { return };
    let same = match (dunce::canonicalize(&home), dunce::canonicalize(config)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    };
    if same || !home.is_dir() {
        return;
    }
    let (files, dirs): (&[&str], &[&str]) = match p {
        Provider::Claude => (&["CLAUDE.md"], &["skills", "agents", "commands"]),
        _ => (&["AGENTS.md", "AGENTS.override.md"], &["skills", "prompts"]),
    };
    for f in files {
        mirror_file(&home.join(f), &config.join(f));
    }
    for d in dirs {
        mirror_dir(&home.join(d), &config.join(d), 0);
    }
}

fn mirror_file(from: &std::path::Path, to: &std::path::Path) {
    let Ok(meta) = std::fs::metadata(from) else {
        return;
    };
    if !meta.is_file() || meta.len() > 2_000_000 {
        return;
    }
    let Ok(data) = std::fs::read(from) else {
        return;
    };
    if std::fs::read(to).ok().as_deref() == Some(&data[..]) {
        return;
    }
    if let Some(parent) = to.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(to, data);
}

pub(crate) fn mirror_dir(from: &std::path::Path, to: &std::path::Path, depth: u8) {
    if depth > 6 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(from) else {
        return;
    };
    for e in entries.flatten() {
        let Ok(kind) = e.file_type() else { continue };
        let name = e.file_name();
        if name.to_string_lossy().starts_with('.') {
            continue;
        }
        if kind.is_dir() {
            mirror_dir(&e.path(), &to.join(&name), depth + 1);
        } else if kind.is_file() {
            mirror_file(&e.path(), &to.join(&name));
        }
    }
}

#[cfg(test)]
mod share_tests {
    use super::*;

    #[test]
    fn mirrors_instructions_and_skills_only() {
        let root = std::env::temp_dir().join(format!("lumen-share-{}", std::process::id()));
        let (home, iso) = (root.join("home"), root.join("iso"));
        std::fs::create_dir_all(home.join("skills/a")).unwrap();
        std::fs::write(home.join("CLAUDE.md"), "rules").unwrap();
        std::fs::write(home.join("skills/a/SKILL.md"), "skill").unwrap();
        std::fs::write(home.join(".credentials.json"), "secret").unwrap();
        std::fs::create_dir_all(&iso).unwrap();
        mirror_file(&home.join("CLAUDE.md"), &iso.join("CLAUDE.md"));
        mirror_dir(&home.join("skills"), &iso.join("skills"), 0);
        assert_eq!(
            std::fs::read_to_string(iso.join("CLAUDE.md")).unwrap(),
            "rules"
        );
        assert_eq!(
            std::fs::read_to_string(iso.join("skills/a/SKILL.md")).unwrap(),
            "skill"
        );
        assert!(!iso.join(".credentials.json").exists());
        let _ = std::fs::remove_dir_all(&root);
    }
}
