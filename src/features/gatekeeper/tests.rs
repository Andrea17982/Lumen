use super::*;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::io::{AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

fn available() -> Availability {
    Availability::Available {
        version: "fixture".into(),
        evidence: "Test fixture".into(),
    }
}

fn keeper() -> Gatekeeper {
    Gatekeeper::in_memory(Duration::from_secs(30)).unwrap()
}

fn scope(k: &Gatekeeper, provider: Provider) -> Scope {
    k.start_session(
        "chat-1",
        provider,
        "thread-1",
        "turn-1",
        &std::env::current_dir().unwrap(),
        available(),
    )
    .unwrap()
}

fn fixture(name: &str) -> Value {
    let source = match name {
        "command" => include_str!("fixtures/codex-command.json"),
        "file-started" => include_str!("fixtures/codex-file-started.json"),
        "file-approval" => include_str!("fixtures/codex-file-approval.json"),
        "bash" => include_str!("fixtures/claude-bash.json"),
        "edit" => include_str!("fixtures/claude-edit.json"),
        _ => panic!("Unknown fixture"),
    };
    let mut v: Value = serde_json::from_str(source).unwrap();
    if name == "command" {
        v["params"]["cwd"] = json!(std::env::current_dir().unwrap().to_string_lossy());
    }
    v
}

fn decide(k: &Gatekeeper, s: &Scope, id: &str, action: Action) -> Result<(), String> {
    k.decide(Decision {
        request_id: id.into(),
        scope: s.clone(),
        action,
        note: None,
    })
}

#[tokio::test]
async fn codex_command_approval_is_correlated_and_consumed_once() {
    let k = keeper();
    let s = scope(&k, Provider::Codex);
    let p = k.request_codex(&s, &fixture("command")).unwrap();
    assert_eq!(k.snapshot().unwrap()[0].status, RequestStatus::Pending);
    decide(&k, &s, &p.request_id, Action::Approve).unwrap();
    assert!(decide(&k, &s, &p.request_id, Action::Approve).is_err());
    let reply = p.wait().await.unwrap();
    assert_eq!(
        reply.response,
        json!({"id":42,"result":{"decision":"accept"}})
    );
    assert!(k.delivery_is_current(&reply).unwrap());
    k.complete_delivery(&reply, true).unwrap();
    assert!(!k.delivery_is_current(&reply).unwrap());
    assert!(k.complete_delivery(&reply, true).is_err());
    assert_eq!(k.snapshot().unwrap()[0].delivery, DeliveryState::Sent);
}

#[tokio::test]
async fn codex_reject_and_cancel_are_distinct() {
    for (action, result) in [(Action::Reject, "decline"), (Action::Cancel, "cancel")] {
        let k = keeper();
        let s = scope(&k, Provider::Codex);
        let p = k.request_codex(&s, &fixture("command")).unwrap();
        decide(&k, &s, &p.request_id, action).unwrap();
        assert_eq!(
            p.wait().await.unwrap().response["result"]["decision"],
            result
        );
    }
}

#[tokio::test]
async fn scope_checks_cover_session_connection_thread_and_turn() {
    let k = keeper();
    let s = scope(&k, Provider::Codex);
    let p = k.request_codex(&s, &fixture("command")).unwrap();
    for index in 0..5 {
        let mut wrong = s.clone();
        match index {
            0 => wrong.session_id = "other".into(),
            1 => wrong.connection_id = "other".into(),
            2 => wrong.provider_session_id = "other".into(),
            3 => wrong.turn_id = "other".into(),
            _ => wrong.provider = Provider::Claude,
        }
        assert!(decide(&k, &wrong, &p.request_id, Action::Approve).is_err());
    }
    for field in ["threadId", "turnId"] {
        let mut wrong = fixture("command");
        wrong["params"][field] = json!("other");
        assert!(k.request_codex(&s, &wrong).is_err());
    }
    decide(&k, &s, &p.request_id, Action::Reject).unwrap();
    assert_eq!(p.wait().await.unwrap().response["id"], 42);
}

#[tokio::test]
async fn duplicate_wire_id_invalidates_the_entire_connection() {
    let k = keeper();
    let s = scope(&k, Provider::Codex);
    let p = k.request_codex(&s, &fixture("command")).unwrap();
    let mut duplicate = fixture("command");
    duplicate["params"]["command"] = json!("erase everything");
    assert!(k.request_codex(&s, &duplicate).is_err());
    assert!(decide(&k, &s, &p.request_id, Action::Approve).is_err());
    assert!(p.wait().await.is_err());
    assert_eq!(k.snapshot().unwrap()[0].status, RequestStatus::Stale);
}

#[tokio::test]
async fn integer_and_string_wire_ids_are_distinct() {
    let k = keeper();
    let s = scope(&k, Provider::Codex);
    let a = k.request_codex(&s, &fixture("command")).unwrap();
    let mut message = fixture("command");
    message["id"] = json!("42");
    let b = k.request_codex(&s, &message).unwrap();
    decide(&k, &s, &a.request_id, Action::Reject).unwrap();
    decide(&k, &s, &b.request_id, Action::Approve).unwrap();
    assert_eq!(a.wait().await.unwrap().response["id"], 42);
    assert_eq!(b.wait().await.unwrap().response["id"], "42");
}

#[tokio::test]
async fn timeout_never_approves_and_gui_cannot_resurrect_it() {
    let k = Gatekeeper::in_memory(Duration::from_millis(20)).unwrap();
    let s = scope(&k, Provider::Codex);
    let p = k.request_codex(&s, &fixture("command")).unwrap();
    let id = p.request_id.clone();
    let reply = tokio::time::timeout(Duration::from_secs(2), p.wait())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(reply.response["result"]["decision"], "cancel");
    assert_eq!(k.snapshot().unwrap()[0].status, RequestStatus::TimedOut);
    assert!(decide(&k, &s, &id, Action::Approve).is_err());
}

#[tokio::test]
async fn failed_delivery_is_uncertain_and_cannot_be_replayed() {
    let k = keeper();
    let s = scope(&k, Provider::Codex);
    let p = k.request_codex(&s, &fixture("command")).unwrap();
    decide(&k, &s, &p.request_id, Action::Approve).unwrap();
    let reply = p.wait().await.unwrap();
    k.complete_delivery(&reply, false).unwrap();
    assert_eq!(k.snapshot().unwrap()[0].delivery, DeliveryState::Uncertain);
    assert!(!k.delivery_is_current(&reply).unwrap());
}

#[tokio::test]
async fn provider_resolution_prevents_late_gui_and_wire_responses() {
    for approved in [false, true] {
        let k = keeper();
        let s = scope(&k, Provider::Codex);
        let p = k.request_codex(&s, &fixture("command")).unwrap();
        let id = p.request_id.clone();
        if approved {
            decide(&k, &s, &id, Action::Approve).unwrap();
        }
        k.observe_codex(&s, &json!({"method":"serverRequest/resolved","params":{"threadId":"thread-1","requestId":42}})).unwrap();
        assert!(decide(&k, &s, &id, Action::Approve).is_err());
        if approved {
            assert!(!k.delivery_is_current(&p.wait().await.unwrap()).unwrap());
        } else {
            assert!(p.wait().await.is_err());
        }
        assert_eq!(k.snapshot().unwrap()[0].status, RequestStatus::Resolved);
    }
}

#[tokio::test]
async fn file_preview_is_bound_to_the_exact_item_and_retired_when_changed() {
    let k = keeper();
    let s = scope(&k, Provider::Codex);
    k.observe_codex(&s, &fixture("file-started")).unwrap();
    let p = k.request_codex(&s, &fixture("file-approval")).unwrap();
    assert_eq!(
        k.snapshot().unwrap()[0].diffs[0].diff,
        "@@ -1 +1 @@\n-before\n+after\n"
    );
    k.observe_codex(&s, &json!({"method":"item/fileChange/patchUpdated","params":{"threadId":"thread-1","turnId":"turn-1","itemId":"patch-1","changes":[{"path":"src/example.ts","diff":"different"}]}})).unwrap();
    assert!(decide(&k, &s, &p.request_id, Action::Approve).is_err());
    assert!(p.wait().await.is_err());
    let mut next = fixture("file-approval");
    next["id"] = json!("patch-request-2");
    let p = k.request_codex(&s, &next).unwrap();
    let request = k
        .snapshot()
        .unwrap()
        .into_iter()
        .find(|r| r.id == p.request_id)
        .unwrap();
    assert_eq!(request.diffs[0].diff, "different");
    decide(&k, &s, &p.request_id, Action::Approve).unwrap();
    assert_eq!(
        p.wait().await.unwrap().response["result"]["decision"],
        "accept"
    );
}

#[tokio::test]
async fn cached_command_is_used_only_with_matching_thread_turn_and_item() {
    let k = keeper();
    let s = scope(&k, Provider::Codex);
    let mut v = fixture("command");
    let command = v["params"]["command"].take();
    let cwd = v["params"]["cwd"].take();
    k.observe_codex(&s, &json!({"method":"item/started","params":{"threadId":"thread-1","turnId":"turn-1","item":{"id":"item-1","type":"commandExecution","command":command,"cwd":cwd}}})).unwrap();
    let p = k.request_codex(&s, &v).unwrap();
    assert!(k.snapshot().unwrap()[0].can_approve);
    decide(&k, &s, &p.request_id, Action::Reject).unwrap();
    v["id"] = json!(43);
    v["params"]["itemId"] = json!("missing");
    let _ = k.request_codex(&s, &v).unwrap();
    assert!(
        !k.snapshot()
            .unwrap()
            .iter()
            .find(|r| r.provider_item_id.as_deref() == Some("missing"))
            .unwrap()
            .can_approve
    );
}

#[tokio::test]
async fn unsupported_grants_and_missing_previews_are_never_approvable() {
    let k = keeper();
    let s = scope(&k, Provider::Codex);
    let mut variants = vec![];
    for (field, value) in [
        (
            "networkApprovalContext",
            json!({"host":"example.com","protocol":"https"}),
        ),
        ("additionalPermissions", json!({"network":{"enabled":true}})),
        ("kind", json!("writeStdin")),
        (
            "availableDecisions",
            json!(["acceptForSession", "decline", "cancel"]),
        ),
    ] {
        let mut message = fixture("command");
        message["params"][field] = value;
        variants.push(message);
    }
    variants.push(fixture("file-approval"));
    let mut grant = fixture("file-approval");
    grant["params"]["grantRoot"] = json!("/");
    variants.push(grant);
    for (index, mut message) in variants.into_iter().enumerate() {
        message["id"] = json!(index);
        let p = k.request_codex(&s, &message).unwrap();
        assert!(decide(&k, &s, &p.request_id, Action::Approve).is_err());
        decide(&k, &s, &p.request_id, Action::Reject).unwrap();
        assert_eq!(
            p.wait().await.unwrap().response["result"]["decision"],
            "decline"
        );
    }
}

#[tokio::test]
async fn unknown_codex_approval_gets_an_error_without_permissions() {
    let k = keeper();
    let s = scope(&k, Provider::Codex);
    let mut message = fixture("command");
    message["method"] = json!("item/permissions/requestApproval");
    let p = k.request_codex(&s, &message).unwrap();
    assert!(decide(&k, &s, &p.request_id, Action::Approve).is_err());
    decide(&k, &s, &p.request_id, Action::Cancel).unwrap();
    let response = p.wait().await.unwrap().response;
    assert_eq!(response["error"]["code"], -32601);
    assert!(response.get("result").is_none());
}

#[tokio::test]
async fn explicit_whitelist_is_exact_scoped_and_does_not_approve_current_request() {
    let k = keeper();
    let s = scope(&k, Provider::Codex);
    let command = fixture("command");
    let cwd = command["params"]["cwd"].as_str().unwrap();
    let current = k.request_codex(&s, &command).unwrap();
    let rule = k.add_rule(&s, "git status --short", cwd).unwrap();
    assert_eq!(k.snapshot().unwrap()[0].status, RequestStatus::Pending);
    decide(&k, &s, &current.request_id, Action::Reject).unwrap();
    let mut next = command.clone();
    next["id"] = json!(43);
    let p = k.request_codex(&s, &next).unwrap();
    let row = k
        .snapshot()
        .unwrap()
        .into_iter()
        .find(|r| r.id == p.request_id)
        .unwrap();
    assert_eq!(row.decision_source.as_deref(), Some("whitelist"));
    assert_eq!(
        p.wait().await.unwrap().response["result"]["decision"],
        "accept"
    );
    k.remove_rule(&s, &rule.id).unwrap();
    next["id"] = json!(44);
    let p = k.request_codex(&s, &next).unwrap();
    assert_eq!(
        k.snapshot()
            .unwrap()
            .iter()
            .find(|r| r.id == p.request_id)
            .unwrap()
            .status,
        RequestStatus::Pending
    );
}

#[tokio::test]
async fn whitelist_cannot_be_bypassed_by_shell_syntax_arguments_or_wrappers() {
    let k = keeper();
    let s = scope(&k, Provider::Codex);
    let command = fixture("command");
    let cwd = command["params"]["cwd"].as_str().unwrap();
    k.add_rule(&s, "git status --short", cwd).unwrap();
    let attempts = [
        "git status --short; whoami",
        "git status --short && whoami",
        "git status --short | whoami",
        "git status --short\nwhoami",
        "git status --short > output",
        "git status --short $(whoami)",
        "git status --short `whoami`",
        "git status --short & whoami",
        "git status --short --ignored",
        "git -c core.fsmonitor=evil status --short",
        "bash -c 'git status --short'",
        "cmd /c git status --short",
        "pwsh -Command git status --short",
        "git status --short\u{202e}evil",
        "git status --short ",
        "git  status --short",
        "git status --short%COMSPEC%",
        "Git status --short",
    ];
    for (index, attempt) in attempts.iter().enumerate() {
        assert!(k.add_rule(&s, attempt, cwd).is_err());
        let mut message = command.clone();
        message["id"] = json!(100 + index);
        message["params"]["command"] = json!(attempt);
        let p = k.request_codex(&s, &message).unwrap();
        assert_eq!(
            k.snapshot()
                .unwrap()
                .iter()
                .find(|r| r.id == p.request_id)
                .unwrap()
                .status,
            RequestStatus::Pending
        );
        decide(&k, &s, &p.request_id, Action::Reject).unwrap();
    }
}

#[tokio::test]
async fn whitelist_does_not_cross_directories_connections_or_providers() {
    let k = keeper();
    let s = scope(&k, Provider::Codex);
    let mut command = fixture("command");
    let cwd = command["params"]["cwd"].as_str().unwrap().to_owned();
    k.add_rule(&s, "git status --short", &cwd).unwrap();
    command["params"]["cwd"] = json!(std::env::temp_dir().to_string_lossy());
    let p = k.request_codex(&s, &command).unwrap();
    assert_eq!(k.snapshot().unwrap()[0].status, RequestStatus::Pending);
    k.disconnect(&s, "Disconnected").unwrap();
    assert!(p.wait().await.is_err());
    assert!(k.rules(&s).unwrap().is_empty());
    let next = scope(&k, Provider::Codex);
    let p = k.request_codex(&next, &fixture("command")).unwrap();
    assert_eq!(
        k.snapshot()
            .unwrap()
            .iter()
            .find(|r| r.id == p.request_id)
            .unwrap()
            .status,
        RequestStatus::Pending
    );
    let claude = scope(&k, Provider::Claude);
    assert!(k.add_rule(&claude, "git status --short", &cwd).is_err());
}

#[tokio::test]
async fn claude_mcp_allow_returns_exact_original_input_in_text_content() {
    let k = keeper();
    let s = scope(&k, Provider::Claude);
    let message = fixture("bash");
    let p = k.request_claude(&s, &message).unwrap();
    assert!(!k.snapshot().unwrap()[0].whitelist_eligible);
    decide(&k, &s, &p.request_id, Action::Approve).unwrap();
    let response = p.wait().await.unwrap().response;
    assert_eq!(response["id"], "mcp-42");
    let payload: Value =
        serde_json::from_str(response["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(
        payload,
        json!({"behavior":"allow","updatedInput":message["params"]["arguments"]["input"]})
    );
    assert!(response.get("__lumenReceipt").is_none());
}

#[tokio::test]
async fn claude_reject_cancel_and_timeout_return_deny() {
    for action in [Action::Reject, Action::Cancel] {
        let k = keeper();
        let s = scope(&k, Provider::Claude);
        let p = k.request_claude(&s, &fixture("bash")).unwrap();
        decide(&k, &s, &p.request_id, action).unwrap();
        let response = p.wait().await.unwrap().response;
        let payload: Value =
            serde_json::from_str(response["result"]["content"][0]["text"].as_str().unwrap())
                .unwrap();
        assert_eq!(payload["behavior"], "deny");
        assert!(payload.get("updatedInput").is_none());
    }
    let k = Gatekeeper::in_memory(Duration::from_millis(10)).unwrap();
    let s = scope(&k, Provider::Claude);
    let response = k
        .request_claude(&s, &fixture("bash"))
        .unwrap()
        .wait()
        .await
        .unwrap()
        .response;
    assert!(response["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("deny"));
}

#[tokio::test]
async fn claude_edit_preview_is_literal_and_unknown_tool_is_unavailable() {
    let k = keeper();
    let s = scope(&k, Provider::Claude);
    let p = k.request_claude(&s, &fixture("edit")).unwrap();
    let request = k.snapshot().unwrap()[0].clone();
    assert_eq!(request.kind, RequestKind::FileChange);
    assert!(request.diffs[0]
        .diff
        .contains("Old text:\nbefore\n\nNew text:\nafter"));
    decide(&k, &s, &p.request_id, Action::Cancel).unwrap();
    let mut unknown = fixture("bash");
    unknown["id"] = json!("unknown");
    unknown["params"]["arguments"]["tool_name"] = json!("AskUserQuestion");
    let p = k.request_claude(&s, &unknown).unwrap();
    assert!(decide(&k, &s, &p.request_id, Action::Approve).is_err());
}

#[tokio::test]
async fn terminal_text_and_malformed_messages_are_never_permission_requests() {
    let k = keeper();
    let s = scope(&k, Provider::Codex);
    for invalid in [
        json!("Run this command? y/n"),
        json!({"text":"Approve?"}),
        json!({"id":true,"method":"item/commandExecution/requestApproval"}),
    ] {
        assert!(k.request_codex(&s, &invalid).is_err());
    }
    let c = scope(&k, Provider::Claude);
    for invalid in [
        json!("Do you want to proceed?"),
        json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"other"}}),
    ] {
        assert!(k.request_claude(&c, &invalid).is_err());
    }
    assert!(k.snapshot().unwrap().is_empty());
}

#[tokio::test]
async fn unavailable_support_cannot_queue_or_auto_authorize() {
    let k = keeper();
    let s = k
        .start_session(
            "chat-1",
            Provider::Codex,
            "thread-1",
            "turn-1",
            Path::new("."),
            Availability::Unavailable {
                reason: "No app server".into(),
            },
        )
        .unwrap();
    assert!(k.request_codex(&s, &fixture("command")).is_err());
    assert!(k
        .add_rule(
            &s,
            "git status --short",
            &std::env::current_dir().unwrap().to_string_lossy()
        )
        .is_err());
    assert!(k.snapshot().unwrap().is_empty());
}

struct TempJournal(PathBuf);
impl TempJournal {
    fn new() -> Self {
        let directory =
            std::env::temp_dir().join(format!("lumen-gatekeeper-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&directory).unwrap();
        Self(directory)
    }
    fn path(&self) -> PathBuf {
        self.0.join("journal.sqlite")
    }
}
impl Drop for TempJournal {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(self.path());
        let _ = std::fs::remove_file(self.0.join("journal.sqlite-shm"));
        let _ = std::fs::remove_file(self.0.join("journal.sqlite-wal"));
        let _ = std::fs::remove_dir(&self.0);
    }
}

#[tokio::test]
async fn restart_marks_pending_stale_and_keeps_decisions_without_reusing_them() {
    let directory = TempJournal::new();
    let k = Gatekeeper::open(directory.path(), Duration::from_secs(30)).unwrap();
    let s = scope(&k, Provider::Codex);
    let pending = k.request_codex(&s, &fixture("command")).unwrap();
    let mut command = fixture("command");
    command["id"] = json!(43);
    let ready = k.request_codex(&s, &command).unwrap();
    decide(&k, &s, &ready.request_id, Action::Approve).unwrap();
    let delivery = ready.wait().await.unwrap();
    command["id"] = json!(44);
    let sent = k.request_codex(&s, &command).unwrap();
    decide(&k, &s, &sent.request_id, Action::Reject).unwrap();
    k.complete_delivery(&sent.wait().await.unwrap(), true)
        .unwrap();
    let recovered = Gatekeeper::open(directory.path(), Duration::from_secs(30)).unwrap();
    let rows = recovered.snapshot().unwrap();
    assert_eq!(
        rows.iter()
            .find(|r| r.id == pending.request_id)
            .unwrap()
            .status,
        RequestStatus::Stale
    );
    assert_eq!(
        rows.iter()
            .find(|r| r.id == delivery.request_id)
            .unwrap()
            .delivery,
        DeliveryState::Uncertain
    );
    assert_eq!(
        rows.iter()
            .filter(|r| r.delivery == DeliveryState::Sent)
            .count(),
        1
    );
    assert!(recovered.sessions().unwrap().is_empty());
    assert!(decide(&recovered, &s, &pending.request_id, Action::Approve).is_err());
    // The previous in-memory service also loses authority after recovery.
    assert!(decide(&k, &s, &pending.request_id, Action::Approve).is_err());
    assert!(!k.delivery_is_current(&delivery).unwrap());
    drop(pending);
    drop(k);
    drop(recovered);
}

#[tokio::test]
async fn concurrent_approvals_are_bound_to_their_own_waiters() {
    let k = keeper();
    let s = scope(&k, Provider::Codex);
    let mut pending = vec![];
    for id in 0..20 {
        let mut v = fixture("command");
        v["id"] = json!(id);
        pending.push((id, k.request_codex(&s, &v).unwrap()));
    }
    for (id, p) in pending.iter().rev() {
        decide(
            &k,
            &s,
            &p.request_id,
            if id % 2 == 0 {
                Action::Approve
            } else {
                Action::Reject
            },
        )
        .unwrap();
    }
    for (id, p) in pending {
        let reply = p.wait().await.unwrap();
        assert_eq!(reply.response["id"], id);
        assert_eq!(
            reply.response["result"]["decision"],
            if id % 2 == 0 { "accept" } else { "decline" }
        );
    }
}

#[test]
fn capabilities_require_actual_schema_and_handshake_evidence() {
    let schema: Value =
        serde_json::from_str(include_str!("fixtures/codex-server-schema.json")).unwrap();
    let command: Value = serde_json::from_str(include_str!(
        "fixtures/CommandExecutionRequestApprovalResponse.json"
    ))
    .unwrap();
    let file: Value = serde_json::from_str(include_str!(
        "fixtures/FileChangeRequestApprovalResponse.json"
    ))
    .unwrap();
    assert!(matches!(
        support::codex(
            "0.167.0-alpha.3.2",
            &schema,
            &command,
            &file,
            &json!({"id":1,"result":{"userAgent":"codex"}})
        ),
        Availability::Available { .. }
    ));
    assert!(matches!(
        support::codex("future", &json!({}), &command, &file, &json!({"result":{}})),
        Availability::Unavailable { .. }
    ));
    assert!(matches!(
        support::codex(
            "future",
            &schema,
            &command,
            &file,
            &json!({"error":{"message":"unsupported"}})
        ),
        Availability::Unavailable { .. }
    ));
    assert!(matches!(
        support::claude("2.1.314", true, true),
        Availability::Available { .. }
    ));
    assert!(matches!(
        support::claude("2.1.314", true, false),
        Availability::Unavailable { .. }
    ));
    assert!(!support::codex_args()
        .iter()
        .any(|s| s.contains("never") || s.contains("bypass")));
    assert!(!support::claude_args(&json!({}), "uuid")
        .iter()
        .any(|s| s.contains("skip-permissions")));
}

async fn mcp_client(endpoint: &mcp::ClaudeEndpoint) -> BufReader<TcpStream> {
    let config = endpoint
        .config(&std::env::current_exe().unwrap(), false)
        .unwrap();
    let env = &config["mcpServers"]["lumen_gatekeeper"]["env"];
    let mut stream = TcpStream::connect(env["LUMEN_GATEKEEPER_ADDR"].as_str().unwrap())
        .await
        .unwrap();
    mcp::write_frame(&mut stream, &json!({"token":env["LUMEN_GATEKEEPER_TOKEN"]}))
        .await
        .unwrap();
    BufReader::new(stream)
}

async fn exchange(reader: &mut BufReader<TcpStream>, value: Value) -> Value {
    mcp::write_frame(reader.get_mut(), &value).await.unwrap();
    tokio::time::timeout(Duration::from_secs(2), mcp::read_frame(reader))
        .await
        .unwrap()
        .unwrap()
        .unwrap()
}

async fn initialize_mcp(reader: &mut BufReader<TcpStream>) {
    let reply=exchange(reader,json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"1"}}})).await;
    assert_eq!(reply["result"]["protocolVersion"], "2025-06-18");
    mcp::write_frame(
        reader.get_mut(),
        &json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
    )
    .await
    .unwrap();
    let tools = exchange(
        reader,
        json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}),
    )
    .await;
    assert_eq!(tools["result"]["tools"][0]["name"], "approve");
}

#[tokio::test]
async fn real_mcp_transport_roundtrip_and_partial_frames_preserve_decisions() {
    let k = keeper();
    let s = scope(&k, Provider::Claude);
    let endpoint = mcp::ClaudeEndpoint::bind(k.clone(), s.clone())
        .await
        .unwrap();
    let mut reader = mcp_client(&endpoint).await;
    let task = tokio::spawn(endpoint.serve());
    initialize_mcp(&mut reader).await;
    let mut events = k.subscribe();
    mcp::write_frame(reader.get_mut(), &fixture("bash"))
        .await
        .unwrap();
    let GatekeeperEvent::Changed(request) = events.recv().await.unwrap() else {
        panic!("Expected request")
    };
    // A partial next frame must survive the outgoing decision select branch.
    reader
        .get_mut()
        .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":999,")
        .await
        .unwrap();
    tokio::task::yield_now().await;
    decide(&k, &s, &request.id, Action::Approve).unwrap();
    let reply = tokio::time::timeout(Duration::from_secs(2), mcp::read_frame(&mut reader))
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(reply["id"], "mcp-42");
    let payload: Value =
        serde_json::from_str(reply["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(payload["behavior"], "allow");
    assert!(reply.get("__lumenReceipt").is_none());
    reader
        .get_mut()
        .write_all(b"\"method\":\"ping\"}\n")
        .await
        .unwrap();
    let reply = tokio::time::timeout(Duration::from_secs(2), mcp::read_frame(&mut reader))
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(reply["id"], 999);
    assert_eq!(k.snapshot().unwrap()[0].delivery, DeliveryState::Sent);
    task.abort();
    let _ = task.await;
}

#[tokio::test]
async fn mcp_cancellation_sends_no_response_for_the_retired_request() {
    let k = keeper();
    let s = scope(&k, Provider::Claude);
    let endpoint = mcp::ClaudeEndpoint::bind(k.clone(), s.clone())
        .await
        .unwrap();
    let mut reader = mcp_client(&endpoint).await;
    let task = tokio::spawn(endpoint.serve());
    initialize_mcp(&mut reader).await;
    let mut events = k.subscribe();
    mcp::write_frame(reader.get_mut(), &fixture("bash"))
        .await
        .unwrap();
    let GatekeeperEvent::Changed(request) = events.recv().await.unwrap() else {
        panic!("Expected request")
    };
    mcp::write_frame(reader.get_mut(),&json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":"mcp-42"}})).await.unwrap();
    let reply = exchange(&mut reader, json!({"jsonrpc":"2.0","id":3,"method":"ping"})).await;
    assert_eq!(reply["id"], 3);
    assert!(decide(&k, &s, &request.id, Action::Approve).is_err());
    assert_eq!(k.snapshot().unwrap()[0].status, RequestStatus::Cancelled);
    task.abort();
    let _ = task.await;
}

#[tokio::test]
async fn mcp_endpoint_abort_invalidates_pending_approvals() {
    let k = keeper();
    let s = scope(&k, Provider::Claude);
    let endpoint = mcp::ClaudeEndpoint::bind(k.clone(), s.clone())
        .await
        .unwrap();
    let mut reader = mcp_client(&endpoint).await;
    let task = tokio::spawn(endpoint.serve());
    initialize_mcp(&mut reader).await;
    let mut events = k.subscribe();
    mcp::write_frame(reader.get_mut(), &fixture("bash"))
        .await
        .unwrap();
    let GatekeeperEvent::Changed(request) = events.recv().await.unwrap() else {
        panic!("Expected request")
    };
    task.abort();
    let _ = task.await;
    assert!(decide(&k, &s, &request.id, Action::Approve).is_err());
    assert_eq!(k.snapshot().unwrap()[0].status, RequestStatus::Stale);
}

#[tokio::test]
async fn protocol_frames_are_bounded_and_terminal_text_is_rejected() {
    let mut bytes = &b"Approve? y/n\n"[..];
    assert!(mcp::read_frame(&mut bytes).await.is_err());
    let mut bytes = &b"{\"id\":1}"[..];
    assert!(mcp::read_frame(&mut bytes).await.is_err());
    let big = vec![b'a'; 2 * 1024 * 1024 + 1];
    let mut bytes = big.as_slice();
    assert!(mcp::read_frame(&mut bytes).await.is_err());
}

#[tokio::test]
async fn partially_malformed_file_previews_cannot_hide_an_unreviewed_change() {
    let k = keeper();
    let s = scope(&k, Provider::Codex);
    let mut started = fixture("file-started");
    started["params"]["item"]["changes"]
        .as_array_mut()
        .unwrap()
        .push(json!({"path":"hidden.ts","kind":{"type":"update"}}));
    k.observe_codex(&s, &started).unwrap();
    let p = k.request_codex(&s, &fixture("file-approval")).unwrap();
    assert!(!k.snapshot().unwrap()[0].can_approve);
    assert!(decide(&k, &s, &p.request_id, Action::Approve).is_err());
}

#[tokio::test]
async fn unknown_command_fields_disable_automatic_whitelist_matches() {
    let k = keeper();
    let s = scope(&k, Provider::Codex);
    let mut command = fixture("command");
    k.add_rule(
        &s,
        "git status --short",
        command["params"]["cwd"].as_str().unwrap(),
    )
    .unwrap();
    command["params"]["futureExecutionMode"] = json!("different interpreter");
    let p = k.request_codex(&s, &command).unwrap();
    let row = k.snapshot().unwrap()[0].clone();
    assert_eq!(row.status, RequestStatus::Pending);
    assert!(!row.whitelist_eligible);
    decide(&k, &s, &p.request_id, Action::Reject).unwrap();
}

#[tokio::test]
async fn mcp_authentication_is_required_before_discovery_or_permission_calls() {
    let k = keeper();
    let s = scope(&k, Provider::Claude);
    let endpoint = mcp::ClaudeEndpoint::bind(k.clone(), s).await.unwrap();
    let config = endpoint
        .config(&std::env::current_exe().unwrap(), false)
        .unwrap();
    let address = config["mcpServers"]["lumen_gatekeeper"]["env"]["LUMEN_GATEKEEPER_ADDR"]
        .as_str()
        .unwrap();
    let mut unauthorized = BufReader::new(TcpStream::connect(address).await.unwrap());
    let mut authorized = mcp_client(&endpoint).await;
    let task = tokio::spawn(endpoint.serve());
    mcp::write_frame(unauthorized.get_mut(), &json!({"token":"wrong"}))
        .await
        .unwrap();
    let closed = tokio::time::timeout(Duration::from_secs(2), mcp::read_frame(&mut unauthorized))
        .await
        .unwrap();
    assert!(matches!(closed, Ok(None) | Err(_)));
    initialize_mcp(&mut authorized).await;
    assert!(k.snapshot().unwrap().is_empty());
    task.abort();
    let _ = task.await;
}

#[tokio::test]
async fn concurrent_gui_decisions_cannot_both_settle_one_callback() {
    let k = keeper();
    let s = scope(&k, Provider::Codex);
    let p = k.request_codex(&s, &fixture("command")).unwrap();
    let approve = {
        let k = k.clone();
        let s = s.clone();
        let id = p.request_id.clone();
        tokio::spawn(async move { decide(&k, &s, &id, Action::Approve) })
    };
    let reject = {
        let k = k.clone();
        let s = s.clone();
        let id = p.request_id.clone();
        tokio::spawn(async move { decide(&k, &s, &id, Action::Reject) })
    };
    let a = approve.await.unwrap();
    let b = reject.await.unwrap();
    assert_ne!(a.is_ok(), b.is_ok());
    let response = p.wait().await.unwrap().response;
    assert!(
        response["result"]["decision"] == "accept" || response["result"]["decision"] == "decline"
    );
}
