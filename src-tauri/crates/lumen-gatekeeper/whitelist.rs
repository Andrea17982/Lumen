use crate::{ApprovalRequest, Provider, RequestKind, Scope};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Deliberately finite recipes. No patterns, prefixes, argument wildcards,
/// shell wrappers, quoting, substitutions, paths or executable options.
pub const WHITELIST_COMMANDS: &[&str] = &[
    "git status --short",
    "git diff --stat",
    "git diff --check",
    "git rev-parse --show-toplevel",
];

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExactRule {
    pub id: String,
    pub scope: Scope,
    pub command: String,
    pub cwd: String,
}

impl ExactRule {
    pub(crate) fn new(scope: &Scope, command: &str, cwd: &str) -> Result<Self, String> {
        if !WHITELIST_COMMANDS.contains(&command) {
            return Err("Only the listed exact command recipes can be whitelisted".into());
        }
        if !Path::new(cwd).is_absolute() {
            return Err("Whitelist directory must be absolute".into());
        }
        let path = std::fs::canonicalize(cwd).map_err(|e| e.to_string())?;
        if !path.is_dir() {
            return Err("Whitelist directory is not a directory".into());
        }
        Ok(Self {
            id: uuid::Uuid::new_v4().to_string(),
            scope: scope.clone(),
            command: command.into(),
            cwd: path.to_string_lossy().into_owned(),
        })
    }

    pub(crate) fn matches(&self, request: &ApprovalRequest) -> bool {
        if self.scope != request.scope
            || request.kind != RequestKind::Command
            || !request.can_approve
            || !request.whitelist_eligible
            || request.command.as_deref() != Some(self.command.as_str())
            || !Path::new(&request.cwd).is_absolute()
        {
            return false;
        }
        // Claude's shell cwd can change independently after `cd` in an earlier
        // tool call. The MCP prompt has no authoritative effective cwd, so it
        // must always be approved manually until a native cwd proof exists.
        if request.scope.provider == Provider::Claude {
            return false;
        }
        std::fs::canonicalize(&request.cwd).is_ok_and(|p| p == Path::new(&self.cwd) && p.is_dir())
    }
}
