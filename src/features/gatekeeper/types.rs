use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Provider {
    Codex,
    Claude,
}

/// A new connection ID is issued for every child process / resumed turn.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Scope {
    pub session_id: String,
    pub connection_id: String,
    pub provider: Provider,
    pub provider_session_id: String,
    pub turn_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum Availability {
    Available { version: String, evidence: String },
    Unavailable { reason: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RequestKind {
    Command,
    FileChange,
    Tool,
    Unsupported,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RequestStatus {
    Pending,
    Approved,
    Rejected,
    Cancelled,
    TimedOut,
    Stale,
    Resolved,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DeliveryState {
    None,
    Ready,
    Sent,
    Uncertain,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffPreview {
    pub path: String,
    pub diff: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalRequest {
    pub id: String,
    pub scope: Scope,
    pub provider_item_id: Option<String>,
    pub kind: RequestKind,
    pub tool: String,
    pub command: Option<String>,
    pub cwd: String,
    pub reason: Option<String>,
    pub diffs: Vec<DiffPreview>,
    /// Exact provider payload, rendered as text only. Never accepted back from the UI.
    pub input: Value,
    pub can_approve: bool,
    pub unavailable_reason: Option<String>,
    pub whitelist_eligible: bool,
    pub created_at_ms: u64,
    pub expires_at_ms: u64,
    pub status: RequestStatus,
    pub delivery: DeliveryState,
    pub decision: Option<Action>,
    pub decision_source: Option<String>,
    pub note: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Action {
    Approve,
    Reject,
    Cancel,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Decision {
    pub request_id: String,
    pub scope: Scope,
    pub action: Action,
    pub note: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionView {
    pub scope: Scope,
    pub cwd: String,
    pub availability: Availability,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GatekeeperEvent {
    Changed(Box<ApprovalRequest>),
    Session(SessionView),
}

#[derive(Clone, Debug)]
pub(crate) struct Draft {
    pub wire_id: Value,
    pub item_id: Option<String>,
    pub method: String,
    pub kind: RequestKind,
    pub tool: String,
    pub command: Option<String>,
    pub cwd: String,
    pub reason: Option<String>,
    pub diffs: Vec<DiffPreview>,
    pub input: Value,
    pub can_approve: bool,
    pub unavailable_reason: Option<String>,
    pub whitelist_eligible: bool,
}

pub(crate) fn string<'a>(v: &'a Value, key: &str) -> Result<&'a str, String> {
    v.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("Missing or invalid {key}"))
}

pub(crate) fn wire_id(v: &Value) -> Result<Value, String> {
    match v.get("id") {
        Some(Value::String(s)) if !s.is_empty() => Ok(Value::String(s.clone())),
        Some(Value::Number(n)) if n.is_i64() || n.is_u64() => Ok(Value::Number(n.clone())),
        _ => Err("Missing or invalid protocol request ID".into()),
    }
}

pub(crate) fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u64::MAX as u128) as u64
}
