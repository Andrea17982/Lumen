//! Capabilities of Lumen's adapters, not a probe of an installed CLI or account.
//! Keep execution decisions and the settings matrix on the same declaration.
use crate::models::Provider;
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FailureCategory {
    Cancelled,
    Quota,
    Unavailable,
    Permission,
    Authentication,
    Failed,
}

/// Stable runtime failure envelope; raw CLI logs are never part of this contract.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Failure {
    pub schema_version: u8,
    pub category: FailureCategory,
    pub message: String,
    pub retry_at: Option<i64>,
}
impl Failure {
    pub fn new(error: &str, retry_at: Option<i64>) -> Self {
        let text = error.to_lowercase();
        let category = if error == "Cancelled" {
            FailureCategory::Cancelled
        } else if crate::limits::is_rate_limit(error) {
            FailureCategory::Quota
        } else if text.contains("unavailable") || text.contains("not found") {
            FailureCategory::Unavailable
        } else if text.contains("permission") || text.contains("denied") {
            FailureCategory::Permission
        } else if text.contains("login") || text.contains("authentication") {
            FailureCategory::Authentication
        } else {
            FailureCategory::Failed
        };
        Self {
            schema_version: 1,
            category,
            message: crate::turns::friendly(error),
            retry_at: if category == FailureCategory::Quota {
                retry_at
            } else {
                None
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Support {
    Available,
    Limited,
    Unavailable,
}

#[derive(Serialize)]
pub struct Capability {
    pub support: Support,
    pub note: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterContract {
    pub provider: Provider,
    pub label: &'static str,
    pub native_resume: Capability,
    pub ask_first: Capability,
    pub native_skills: Capability,
    pub shared_tools: Capability,
    pub input_usage: Capability,
    pub turn_cost: Capability,
    pub cancellation: Capability,
    pub permission_profiles: Capability,
    pub streaming: Capability,
    pub input_images: Capability,
    pub subagents: Capability,
    pub native_extensions: Capability,
    pub model_change: Capability,
    pub fallback: Capability,
    pub events: Capability,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Matrix {
    pub schema_version: u8,
    pub adapters: Vec<AdapterContract>,
}

fn available() -> Capability {
    Capability {
        support: Support::Available,
        note: "",
    }
}
fn limited(note: &'static str) -> Capability {
    Capability {
        support: Support::Limited,
        note,
    }
}
fn unavailable(note: &'static str) -> Capability {
    Capability {
        support: Support::Unavailable,
        note,
    }
}

pub fn contract(provider: &Provider) -> AdapterContract {
    let native = matches!(
        provider,
        Provider::Claude | Provider::Gpt | Provider::Opencode | Provider::Cursor
    );
    AdapterContract {
        provider: provider.clone(),
        label: match provider {
            Provider::Claude => "Claude Code",
            Provider::Gpt => "Codex",
            Provider::Antigravity => "Antigravity",
            Provider::Ollama => "Ollama",
            Provider::Opencode => "OpenCode",
            Provider::Cursor => "Cursor CLI",
            Provider::Openaiapi => "OpenAI API",
            Provider::Anthropicapi => "Anthropic API",
            Provider::Googleapi => "Google API",
        },
        native_resume: if native {
            available()
        } else {
            unavailable("Lumen replays conversation context; it does not resume a native session.")
        },
        ask_first: if provider.is_api()
            || matches!(
                provider,
                Provider::Claude | Provider::Gpt | Provider::Opencode
            ) {
            available()
        } else {
            unavailable(
                "Per-action approvals are not integrated. Choose another permission profile.",
            )
        },
        native_skills: if matches!(provider, Provider::Claude | Provider::Gpt) {
            available()
        } else {
            limited(
                "Skills are supplied as references in context, outside the native skill runtime.",
            )
        },
        shared_tools: if *provider == Provider::Cursor {
            unavailable("Uses the existing native profile in Autopilot only. Isolated account profiles, restricted permissions and Lumen shared tools are not integrated yet.")
        } else if provider.is_api() {
            limited("Lumen host tools, Moli, graph and shared MCP follow all four access profiles; native runtime parity is not implied.")
        } else if *provider == Provider::Ollama {
            limited("Lumen filters shared tools per permission profile. Model tool support varies.")
        } else {
            limited("Shared namespaces require Ask first or Autopilot. Restricted profiles use the filtered account gateway.")
        },
        input_usage: if matches!(provider, Provider::Cursor | Provider::Opencode) {
            unavailable("No input-token or context-window metadata is integrated for this adapter.")
        } else if *provider == Provider::Ollama {
            unavailable(
                "The context meter uses an estimate; this adapter does not report input tokens.",
            )
        } else {
            limited("Input tokens are shown only when reported by the provider; missing values remain estimates.")
        },
        turn_cost: if provider.is_api() {
            limited("Estimated from user-confirmed model rates and reported usage; uncertain requests retain their budget reservation.")
        } else {
            unavailable("Lumen does not report a monetary cost per turn for this adapter.")
        },
        cancellation: available(),
        permission_profiles: if *provider == Provider::Cursor {
            unavailable("Autopilot only; the existing native profile owns its permissions.")
        } else if *provider == Provider::Antigravity || *provider == Provider::Ollama {
            limited("Read only, File edits and Autopilot; per-action Ask first is not integrated.")
        } else { available() },
        streaming: if provider.is_api() {
            unavailable("Buffered official JSON responses. Tool steps and usage are emitted between requests; token streaming is not implemented.")
        } else { limited("Provider events are normalized as they arrive; exact chunking and thinking output are provider-specific.") },
        input_images: if provider.is_api() || matches!(provider, Provider::Opencode | Provider::Cursor | Provider::Ollama) {
            unavailable("No native image-input payload is integrated. Text attachments and tool-produced previews remain available.")
        } else { limited("Image support depends on the selected native model and its official tool surface.") },
        subagents: if matches!(provider, Provider::Claude | Provider::Gpt) {
            limited("Native agent behavior follows its runtime and task permissions; Lumen does not promise cross-provider parity.")
        } else if *provider == Provider::Opencode {
            limited("Native subagents may run in Autopilot. Disabled for Ask first and restricted profiles because custom agents do not inherit parent permissions.")
        } else { unavailable("No Lumen subagent tool is integrated for this adapter.") },
        native_extensions: if matches!(provider, Provider::Claude | Provider::Gpt | Provider::Opencode | Provider::Cursor) {
            limited("The native profile retains its supported instructions, commands, plugins and hooks. Proprietary cloud connectors and extension runtimes are not translated to other adapters.")
        } else { unavailable("Proprietary commands, hooks and extension runtimes are not executed by this adapter. Portable instructions and shared MCP are separate capabilities.") },
        model_change: limited("Selected model IDs are validated against the official catalog before a turn. No automatic substitution for a rejected explicit selection."),
        fallback: if matches!(provider, Provider::Opencode | Provider::Cursor) {
            limited("Explicit selection only. Native continuation is preferred; no automatic entry or automatic review worker selection.")
        } else if provider.is_api() {
            limited("Auto and fallback require explicit API opt-in and sufficient reserved budget. No automatic retry after observed effects or uncertain requests.")
        } else { limited("Fallback only before observed work; task difficulty and reasoning are retained. Weaker models require a decision.") },
        events: limited("Activity schema v1: text/replace, thinking, tool lifecycle, session, usage, interaction and typed failure. Unknown native events are retained conservatively; missing usage/cost is never invented."),
    }
}

#[tauri::command]
pub fn adapter_capabilities() -> Matrix {
    Matrix {
        schema_version: 1,
        adapters: [
            Provider::Claude,
            Provider::Gpt,
            Provider::Antigravity,
            Provider::Ollama,
            Provider::Opencode,
            Provider::Cursor,
            Provider::Openaiapi,
            Provider::Anthropicapi,
            Provider::Googleapi,
        ]
        .iter()
        .map(contract)
        .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ipc_matrix_has_explicit_limits_and_stable_wire_names() {
        let value = serde_json::to_value(adapter_capabilities()).unwrap();
        assert_eq!(value["schemaVersion"], 1);
        let adapters = value["adapters"].as_array().unwrap();
        assert_eq!(adapters.len(), 9);
        for (adapter, provider) in adapters.iter().zip([
            "claude",
            "gpt",
            "antigravity",
            "ollama",
            "opencode",
            "cursor",
            "openaiapi",
            "anthropicapi",
            "googleapi",
        ]) {
            assert_eq!(adapter["provider"], provider);
            for key in [
                "nativeResume",
                "askFirst",
                "nativeSkills",
                "sharedTools",
                "inputUsage",
                "turnCost",
                "cancellation",
                "permissionProfiles",
                "streaming",
                "inputImages",
                "subagents",
                "nativeExtensions",
                "modelChange",
                "fallback",
                "events",
            ] {
                let capability = &adapter[key];
                assert!(["available", "limited", "unavailable"]
                    .contains(&capability["support"].as_str().unwrap()));
                if capability["support"] != "available" {
                    assert!(
                        !capability["note"].as_str().unwrap().is_empty(),
                        "{provider}/{key}"
                    );
                }
            }
            assert_eq!(
                adapter["turnCost"]["support"],
                if provider.ends_with("api") {
                    "limited"
                } else {
                    "unavailable"
                }
            );
        }
    }
    #[test]
    fn runtime_errors_have_stable_categories_and_quota_only_retry_times() {
        let quota = Failure::new("Your workspace is out of credits", Some(42));
        assert_eq!(quota.category, FailureCategory::Quota);
        assert_eq!(quota.retry_at, Some(42));
        assert_eq!(Failure::new("Cancelled", Some(42)).retry_at, None);
        assert_eq!(serde_json::to_value(quota).unwrap()["schemaVersion"], 1);
    }
}
