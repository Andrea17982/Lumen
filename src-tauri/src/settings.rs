//! Small persisted preferences that the backend needs (frontend-only ones stay in localStorage).
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::Path};

/// Optional Orchestra caps; `None` and missing thresholds mean no cap of the user's own.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct OrchestraLimits {
    pub max_concurrent: Option<usize>,
    pub max_subtasks: Option<usize>,
    /// Used percentage per provider id at which it gets no new work.
    pub quota_thresholds: HashMap<String, f64>,
}

/// Optional local helper (Ollama) for titles, commit messages and handoff drafts.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct LocalAssistantSettings {
    pub enabled: bool,
    /// Installed Ollama model used by the helper; empty means none chosen yet.
    pub model: String,
    /// Name new chats automatically after their first reply.
    pub auto_titles: bool,
    pub timeout_ms: u64,
}
impl Default for LocalAssistantSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            model: String::new(),
            auto_titles: true,
            timeout_ms: 60_000,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppSettings {
    /// Optional Lumen validation after a native turn; CLI-run checks stay native and unaffected.
    pub verify_changes: bool,
    pub capture_hotkey: bool,
    /// Retry a rate-limited turn on the other provider.
    pub auto_fallback: bool,
    /// Run the project's check after file edits and send failures back (max 3 attempts).
    pub self_heal: bool,
    /// Cross-audit rounds for Auto-review (1–3).
    pub review_rounds: u8,
    /// Orchestra: complex Auto tasks are planned, split and run by several agents.
    pub orchestra: bool,
    /// The window's X hides Lumen to the notification area.
    pub close_to_tray: bool,
    /// Start Lumen when the user signs in.
    pub open_at_login: bool,
    /// Pinned Ollama model; empty lets Lumen choose the largest installed one.
    pub ollama_model: String,
    /// The one-time "still running in the notification area" notice was shown.
    pub tray_hint_shown: bool,
    pub orchestra_limits: OrchestraLimits,
    pub local_assistant: LocalAssistantSettings,
    /// Add repository-map locations of identifiers a request mentions to its prompt.
    pub repo_context: bool,
}
impl Default for AppSettings {
    fn default() -> Self {
        Self {
            verify_changes: false,
            capture_hotkey: false,
            auto_fallback: true,
            self_heal: true,
            review_rounds: 1,
            orchestra: false,
            close_to_tray: true,
            open_at_login: false,
            ollama_model: String::new(),
            tray_hint_shown: false,
            orchestra_limits: OrchestraLimits::default(),
            local_assistant: LocalAssistantSettings::default(),
            repo_context: true,
        }
    }
}
impl AppSettings {
    pub fn load(root: &Path) -> Self {
        std::fs::read(root.join("settings.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }
    pub fn save(&self, root: &Path) -> Result<(), String> {
        std::fs::write(
            root.join("settings.json"),
            serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())
    }
    pub fn validate(&mut self) -> Result<(), String> {
        self.review_rounds = self.review_rounds.clamp(1, 3);
        self.ollama_model = self.ollama_model.trim().chars().take(120).collect();
        let a = &mut self.local_assistant;
        a.model = a.model.trim().chars().take(120).collect();
        a.timeout_ms = a.timeout_ms.clamp(5_000, 300_000);
        let l = &mut self.orchestra_limits;
        l.max_concurrent = l.max_concurrent.filter(|&n| n >= 1).map(|n| n.min(32));
        l.max_subtasks = l.max_subtasks.filter(|&n| n >= 1).map(|n| n.min(999));
        l.quota_thresholds.retain(|p, v| {
            ["claude", "gpt", "gemini", "antigravity"].contains(&p.as_str())
                && v.is_finite()
                && *v > 0.0
                && *v < 100.0
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn old_settings_files_get_new_defaults() {
        let s: AppSettings = serde_json::from_str(r#"{"autoFallback":false}"#).unwrap();
        assert!(!s.auto_fallback && !s.orchestra && s.close_to_tray && !s.open_at_login);
        assert!(s.ollama_model.is_empty() && s.self_heal);
        assert!(!s.local_assistant.enabled && s.local_assistant.auto_titles);
    }
}
