//! Small persisted preferences that the backend needs (frontend-only ones stay in localStorage).
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppSettings {
    /// Retry a rate-limited turn on the other provider.
    pub auto_fallback: bool,
    /// Run the project's check after file edits and send failures back (max 3 attempts).
    pub self_heal: bool,
    /// Cross-audit rounds for Auto-review (1–3).
    pub review_rounds: u8,
    pub api_enabled: bool,
    pub api_port: u16,
    pub api_token: String,
}
impl Default for AppSettings {
    fn default() -> Self {
        Self {
            auto_fallback: true,
            self_heal: true,
            review_rounds: 1,
            api_enabled: false,
            api_port: 8317,
            api_token: String::new(),
        }
    }
}
impl AppSettings {
    pub fn load(root: &Path) -> Self {
        let mut s: Self = std::fs::read(root.join("settings.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        if s.api_token.is_empty() {
            s.api_token = format!("lumen-{}", crate::models::id().replace('-', ""));
            let _ = s.save(root);
        }
        s
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
        if self.api_port < 1024 {
            return Err("Choose a port between 1024 and 65535".into());
        }
        if self.api_token.len() < 16 {
            return Err("The API key must be at least 16 characters".into());
        }
        Ok(())
    }
}
