use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Claude,
    Gpt,
    Gemini,
    Antigravity,
}
impl Provider {
    pub fn binary(&self) -> &str {
        match self {
            Self::Claude => "claude",
            Self::Gpt => "codex",
            Self::Gemini => "gemini",
            Self::Antigravity => "agy",
        }
    }
    pub fn env(&self) -> &str {
        match self {
            Self::Claude => "CLAUDE_CONFIG_DIR",
            Self::Gpt => "CODEX_HOME",
            Self::Gemini => "GEMINI_CLI_HOME",
            Self::Antigravity => "HOME",
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
#[derive(Clone, Debug, Serialize, Deserialize)]
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
