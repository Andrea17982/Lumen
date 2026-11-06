use crate::adapters;
use crate::types::*;
use crate::whitelist::ExactRule;
use rusqlite::{params, Connection};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, oneshot};

const MAX_PENDING: usize = 128;
const MAX_PAYLOAD: usize = 2 * 1024 * 1024;

struct Waiting {
    draft: Draft,
    deadline: Instant,
    sender: oneshot::Sender<Delivery>,
}

struct State {
    db: Journal,
    sessions: HashMap<String, SessionView>,
    entries: HashMap<String, ApprovalRequest>,
    waiting: HashMap<String, Waiting>,
    items: HashMap<(String, String), Value>,
    seen_wire: HashMap<(String, String), String>,
    rules: Vec<ExactRule>,
}

struct Journal {
    connection: Connection,
    owner: String,
}

impl Journal {
    fn is_current(&self) -> Result<bool, String> {
        self.connection
            .query_row(
                "SELECT owner=?1 FROM gatekeeper_meta WHERE id=1",
                [&self.owner],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())
    }
}

#[derive(Clone)]
pub struct Gatekeeper {
    state: Arc<Mutex<State>>,
    events: broadcast::Sender<GatekeeperEvent>,
    timeout: Duration,
}

/// A reply is consumed once. On a failed write, mark uncertain and stop the
/// provider turn; never replay an approval after reconnecting.
#[derive(Debug)]
pub struct Delivery {
    pub request_id: String,
    pub response: Value,
}

pub struct PendingReply {
    pub request_id: String,
    receiver: oneshot::Receiver<Delivery>,
}

impl PendingReply {
    pub async fn wait(self) -> Result<Delivery, String> {
        self.receiver
            .await
            .map_err(|_| "Approval was reconciled or its connection closed; do not reply".into())
    }
}

impl Gatekeeper {
    pub fn open(path: impl AsRef<Path>, timeout: Duration) -> Result<Self, String> {
        let db = Connection::open(path).map_err(|e| e.to_string())?;
        Self::with_connection(db, timeout)
    }

    pub fn in_memory(timeout: Duration) -> Result<Self, String> {
        Self::with_connection(
            Connection::open_in_memory().map_err(|e| e.to_string())?,
            timeout,
        )
    }

    fn with_connection(mut db: Connection, timeout: Duration) -> Result<Self, String> {
        if timeout.is_zero() || timeout > Duration::from_secs(3600) {
            return Err("Approval timeout must be greater than zero and at most one hour".into());
        }
        db.busy_timeout(Duration::from_secs(5))
            .map_err(|e| e.to_string())?;
        db.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;
             CREATE TABLE IF NOT EXISTS gatekeeper_meta (id INTEGER PRIMARY KEY CHECK(id=1), version INTEGER NOT NULL, owner TEXT NOT NULL);
             INSERT OR IGNORE INTO gatekeeper_meta VALUES(1,1,'');
             CREATE TABLE IF NOT EXISTS gatekeeper_requests (id TEXT PRIMARY KEY, body TEXT NOT NULL);"
        ).map_err(|e| e.to_string())?;
        let version: i64 = db
            .query_row("SELECT version FROM gatekeeper_meta WHERE id=1", [], |r| {
                r.get(0)
            })
            .map_err(|e| e.to_string())?;
        if version != 1 {
            return Err("Unsupported Gatekeeper journal version".into());
        }
        let entries = {
            let mut query = db
                .prepare("SELECT body FROM gatekeeper_requests")
                .map_err(|e| e.to_string())?;
            let rows = query
                .query_map([], |r| r.get::<_, String>(0))
                .map_err(|e| e.to_string())?;
            let mut entries = HashMap::new();
            for row in rows {
                let mut request: ApprovalRequest =
                    serde_json::from_str(&row.map_err(|e| e.to_string())?)
                        .map_err(|e| e.to_string())?;
                if request.status == RequestStatus::Pending {
                    request.status = RequestStatus::Stale;
                    request.note = Some("App restarted; request must be reissued by a verified live provider connection".into());
                }
                if request.delivery == DeliveryState::Ready {
                    request.delivery = DeliveryState::Uncertain;
                    request.note = Some("App restarted during response delivery; verify provider state before retrying work".into());
                }
                entries.insert(request.id.clone(), request);
            }
            entries
        };
        let tx = db.transaction().map_err(|e| e.to_string())?;
        let owner = uuid::Uuid::new_v4().to_string();
        tx.execute("UPDATE gatekeeper_meta SET owner=?1 WHERE id=1", [&owner])
            .map_err(|e| e.to_string())?;
        for request in entries.values() {
            tx.execute(
                "UPDATE gatekeeper_requests SET body=?2 WHERE id=?1",
                params![
                    request.id,
                    serde_json::to_string(request).map_err(|e| e.to_string())?
                ],
            )
            .map_err(|e| e.to_string())?;
        }
        tx.commit().map_err(|e| e.to_string())?;
        let (events, _) = broadcast::channel(256);
        Ok(Self {
            state: Arc::new(Mutex::new(State {
                db: Journal {
                    connection: db,
                    owner,
                },
                sessions: HashMap::new(),
                entries,
                waiting: HashMap::new(),
                items: HashMap::new(),
                seen_wire: HashMap::new(),
                rules: vec![],
            })),
            events,
            timeout,
        })
    }

    fn lock(&self) -> Result<MutexGuard<'_, State>, String> {
        self.state
            .lock()
            .map_err(|_| "Gatekeeper state lock failed; stop the provider".into())
    }

    pub fn subscribe(&self) -> broadcast::Receiver<GatekeeperEvent> {
        self.events.subscribe()
    }

    /// Caller must probe the actual selected CLI binary/account first. The
    /// generated connection ID is never restored from a persisted session.
    pub fn start_session(
        &self,
        session_id: &str,
        provider: Provider,
        provider_session_id: &str,
        turn_id: &str,
        cwd: &Path,
        availability: Availability,
    ) -> Result<Scope, String> {
        if [session_id, provider_session_id, turn_id]
            .iter()
            .any(|s| s.is_empty())
        {
            return Err("Session, provider session and turn IDs are required".into());
        }
        let cwd = std::fs::canonicalize(cwd).map_err(|e| e.to_string())?;
        if !cwd.is_dir() {
            return Err("Session cwd is not a directory".into());
        }
        let old: Vec<Scope> = self
            .lock()?
            .sessions
            .values()
            .filter(|s| s.scope.session_id == session_id && s.scope.provider == provider)
            .map(|s| s.scope.clone())
            .collect();
        for scope in old {
            self.disconnect(&scope, "Session connection replaced")?;
        }
        let scope = Scope {
            session_id: session_id.into(),
            connection_id: uuid::Uuid::new_v4().to_string(),
            provider,
            provider_session_id: provider_session_id.into(),
            turn_id: turn_id.into(),
        };
        let view = SessionView {
            scope: scope.clone(),
            cwd: cwd.to_string_lossy().into_owned(),
            availability,
        };
        self.lock()?
            .sessions
            .insert(scope.connection_id.clone(), view.clone());
        let _ = self.events.send(GatekeeperEvent::Session(view));
        Ok(scope)
    }

    pub fn sessions(&self) -> Result<Vec<SessionView>, String> {
        Ok(self.lock()?.sessions.values().cloned().collect())
    }

    /// Journal snapshots are read-only, including historical/stale approvals.
    pub fn snapshot(&self) -> Result<Vec<ApprovalRequest>, String> {
        let mut requests: Vec<_> = self.lock()?.entries.values().cloned().collect();
        requests.sort_by(|a, b| a.created_at_ms.cmp(&b.created_at_ms).then(a.id.cmp(&b.id)));
        Ok(requests)
    }

    pub fn request_codex(&self, scope: &Scope, message: &Value) -> Result<PendingReply, String> {
        if scope.provider != Provider::Codex {
            return Err("Wrong provider adapter".into());
        }
        let s = self.lock()?;
        let session = active(&s, scope)?;
        let item_id = message["params"]["itemId"].as_str().unwrap_or("");
        let item = s.items.get(&(scope.connection_id.clone(), item_id.into()));
        let draft = adapters::codex(scope, message, item, &session.cwd)?;
        drop(s);
        self.enqueue(scope, draft)
    }

    /// Only call this from the dedicated authenticated MCP connection, never
    /// from terminal output or a generic renderer invoke accepting arbitrary JSON.
    pub fn request_claude(&self, scope: &Scope, message: &Value) -> Result<PendingReply, String> {
        if scope.provider != Provider::Claude {
            return Err("Wrong provider adapter".into());
        }
        let s = self.lock()?;
        let draft = adapters::claude(message, &active(&s, scope)?.cwd)?;
        drop(s);
        self.enqueue(scope, draft)
    }

    fn enqueue(&self, scope: &Scope, draft: Draft) -> Result<PendingReply, String> {
        let mut s = self.lock()?;
        active(&s, scope)?;
        if s.waiting.len() >= MAX_PENDING || draft.input.to_string().len() > MAX_PAYLOAD {
            return Err("Gatekeeper approval capacity exceeded; stop the provider turn".into());
        }
        let key = (scope.connection_id.clone(), draft.wire_id.to_string());
        if s.seen_wire.contains_key(&key) {
            drop(s);
            self.disconnect(
                scope,
                "Duplicate wire request ID invalidated this connection",
            )?;
            return Err("Duplicate wire request ID; stop the provider connection".into());
        }
        let created = now_ms();
        let request = ApprovalRequest {
            id: uuid::Uuid::new_v4().to_string(),
            scope: scope.clone(),
            provider_item_id: draft.item_id.clone(),
            kind: draft.kind.clone(),
            tool: draft.tool.clone(),
            command: draft.command.clone(),
            cwd: draft.cwd.clone(),
            reason: draft.reason.clone(),
            diffs: draft.diffs.clone(),
            input: draft.input.clone(),
            can_approve: draft.can_approve,
            unavailable_reason: draft.unavailable_reason.clone(),
            whitelist_eligible: draft.whitelist_eligible && scope.provider == Provider::Codex,
            created_at_ms: created,
            expires_at_ms: created.saturating_add(self.timeout.as_millis() as u64),
            status: RequestStatus::Pending,
            delivery: DeliveryState::None,
            decision: None,
            decision_source: None,
            note: None,
        };
        persist(&s.db, &request)?;
        let id = request.id.clone();
        s.seen_wire.insert(key, id.clone());
        let automatic = s.rules.iter().any(|r| r.matches(&request));
        let (sender, receiver) = oneshot::channel();
        s.entries.insert(id.clone(), request.clone());
        s.waiting.insert(
            id.clone(),
            Waiting {
                draft,
                deadline: Instant::now() + self.timeout,
                sender,
            },
        );
        let _ = self
            .events
            .send(GatekeeperEvent::Changed(Box::new(request)));
        if automatic {
            settle(
                &mut s,
                &id,
                Action::Approve,
                RequestStatus::Approved,
                "whitelist",
                "Exact session command whitelist",
                &self.events,
            )?;
        }
        drop(s);
        let weak_state = Arc::downgrade(&self.state);
        let events = self.events.clone();
        let timeout = self.timeout;
        let timeout_id = id.clone();
        tokio::spawn(async move {
            tokio::time::sleep(timeout).await;
            let Some(state) = weak_state.upgrade() else {
                return;
            };
            let keeper = Gatekeeper {
                state,
                events,
                timeout,
            };
            // An error prevents authorization and closes the waiter, so the
            // transport can stop the provider rather than hanging forever.
            if keeper.expire(&timeout_id).is_err() {
                if let Ok(mut s) = keeper.lock() {
                    s.waiting.remove(&timeout_id);
                }
            }
        });
        Ok(PendingReply {
            request_id: id,
            receiver,
        })
    }

    pub fn decide(&self, decision: Decision) -> Result<(), String> {
        let mut s = self.lock()?;
        active(&s, &decision.scope)?;
        let request = s
            .entries
            .get(&decision.request_id)
            .ok_or("Unknown approval request")?;
        if request.scope != decision.scope || request.status != RequestStatus::Pending {
            return Err("This request is no longer pending in this session connection".into());
        }
        let waiting = s
            .waiting
            .get(&decision.request_id)
            .ok_or("No live approval waiter")?;
        if Instant::now() >= waiting.deadline {
            settle(
                &mut s,
                &decision.request_id,
                Action::Cancel,
                RequestStatus::TimedOut,
                "timeout",
                "Approval timed out",
                &self.events,
            )?;
            return Err("Approval has expired".into());
        }
        if decision.action == Action::Approve && !request.can_approve {
            return Err(
                "This request cannot be approved with the available protocol support".into(),
            );
        }
        let status = match decision.action {
            Action::Approve => RequestStatus::Approved,
            Action::Reject => RequestStatus::Rejected,
            Action::Cancel => RequestStatus::Cancelled,
        };
        let note = decision
            .note
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or(match decision.action {
                Action::Approve => "Approved once in Lumen",
                Action::Reject => "Rejected in Lumen",
                Action::Cancel => "Cancelled in Lumen",
            });
        if note.len() > 4096 {
            return Err("Decision note is too long".into());
        }
        settle(
            &mut s,
            &decision.request_id,
            decision.action,
            status,
            "user",
            note,
            &self.events,
        )
    }

    fn expire(&self, id: &str) -> Result<(), String> {
        let mut s = self.lock()?;
        if s.waiting.contains_key(id) {
            settle(
                &mut s,
                id,
                Action::Cancel,
                RequestStatus::TimedOut,
                "timeout",
                "Approval timed out",
                &self.events,
            )?;
        }
        Ok(())
    }

    pub fn complete_delivery(&self, delivery: &Delivery, success: bool) -> Result<(), String> {
        let mut s = self.lock()?;
        let mut request = s
            .entries
            .get(&delivery.request_id)
            .ok_or("Unknown approval delivery")?
            .clone();
        if request.delivery != DeliveryState::Ready {
            return Err("Approval response cannot be replayed".into());
        }
        request.delivery = if success {
            DeliveryState::Sent
        } else {
            DeliveryState::Uncertain
        };
        persist(&s.db, &request)?;
        s.entries.insert(request.id.clone(), request.clone());
        let _ = self
            .events
            .send(GatekeeperEvent::Changed(Box::new(request)));
        Ok(())
    }

    /// Check immediately before writing a response. A provider may cancel an
    /// already-decided callback while its response is queued for delivery.
    pub fn delivery_is_current(&self, delivery: &Delivery) -> Result<bool, String> {
        let s = self.lock()?;
        let request = s
            .entries
            .get(&delivery.request_id)
            .ok_or("Unknown approval delivery")?;
        Ok(request.delivery == DeliveryState::Ready && active(&s, &request.scope).is_ok())
    }

    /// Transport loss invalidates waiters and session rules. This does not
    /// signal that the provider process stopped; the runner must verify/stop it.
    pub fn disconnect(&self, scope: &Scope, reason: &str) -> Result<(), String> {
        let mut s = self.lock()?;
        let Some(session) = s.sessions.get(&scope.connection_id) else {
            return Ok(());
        };
        if session.scope != *scope {
            return Err("Session scope mismatch".into());
        }
        let ids: Vec<_> = s
            .entries
            .values()
            .filter(|r| {
                r.scope == *scope
                    && (r.status == RequestStatus::Pending || r.delivery == DeliveryState::Ready)
            })
            .map(|r| r.id.clone())
            .collect();
        for id in ids {
            let mut r = s.entries[&id].clone();
            if r.status == RequestStatus::Pending {
                r.status = RequestStatus::Stale;
            }
            if r.delivery == DeliveryState::Ready {
                r.delivery = DeliveryState::Uncertain;
            }
            r.note = Some(reason.into());
            persist(&s.db, &r)?;
            s.entries.insert(id.clone(), r.clone());
            s.waiting.remove(&id);
            let _ = self.events.send(GatekeeperEvent::Changed(Box::new(r)));
        }
        s.rules.retain(|r| r.scope != *scope);
        s.items
            .retain(|(connection, _), _| connection != &scope.connection_id);
        if let Some(session) = s.sessions.get_mut(&scope.connection_id) {
            session.availability = Availability::Unavailable {
                reason: reason.into(),
            };
            let _ = self.events.send(GatekeeperEvent::Session(session.clone()));
        }
        Ok(())
    }

    /// Reconcile only live, structured app-server notifications. Returns false
    /// for unrelated normal notifications, which the existing stream may handle.
    pub fn observe_codex(&self, scope: &Scope, message: &Value) -> Result<bool, String> {
        if scope.provider != Provider::Codex {
            return Err("Wrong provider adapter".into());
        }
        let method = message.get("method").and_then(Value::as_str).unwrap_or("");
        if !matches!(
            method,
            "item/started"
                | "item/fileChange/patchUpdated"
                | "item/completed"
                | "serverRequest/resolved"
                | "turn/completed"
        ) {
            return Ok(false);
        }
        let p = &message["params"];
        let mut s = self.lock()?;
        active(&s, scope)?;
        if method == "serverRequest/resolved" {
            if string(p, "threadId")? != scope.provider_session_id {
                return Err("Resolved request belongs to another thread".into());
            }
            let wire = p.get("requestId").ok_or("Missing resolved request ID")?;
            let id = s
                .seen_wire
                .get(&(scope.connection_id.clone(), wire.to_string()))
                .cloned();
            if let Some(id) = id {
                if s.entries.get(&id).is_some_and(|r| {
                    r.status == RequestStatus::Pending || r.delivery == DeliveryState::Ready
                }) {
                    clear_waiter(
                        &mut s,
                        &id,
                        RequestStatus::Resolved,
                        "Provider cleared the request",
                        &self.events,
                    )?;
                }
            }
            return Ok(true);
        }
        if method == "turn/completed" {
            if string(p, "threadId")? != scope.provider_session_id
                || string(&p["turn"], "id")? != scope.turn_id
            {
                return Err("Completed turn scope mismatch".into());
            }
            drop(s);
            self.disconnect(scope, "Provider turn completed")?;
            return Ok(true);
        }
        adapters::validate_codex_scope(scope, p)?;
        let item_id = if method == "item/fileChange/patchUpdated" {
            string(p, "itemId")?
        } else {
            string(&p["item"], "id")?
        };
        let key = (scope.connection_id.clone(), item_id.into());
        // Preview changes retire the old review. Never silently approve an
        // updated patch using a decision about earlier content.
        let ids: Vec<_> = s
            .entries
            .values()
            .filter(|r| {
                r.scope == *scope
                    && r.provider_item_id.as_deref() == Some(item_id)
                    && (r.status == RequestStatus::Pending || r.delivery == DeliveryState::Ready)
            })
            .map(|r| r.id.clone())
            .collect();
        for id in ids {
            clear_waiter(
                &mut s,
                &id,
                RequestStatus::Stale,
                "Provider item changed; request must be reissued",
                &self.events,
            )?;
        }
        match method {
            "item/started" => {
                s.items.insert(key, p["item"].clone());
            }
            "item/fileChange/patchUpdated" => {
                let item = s
                    .items
                    .get_mut(&key)
                    .ok_or("Patch update has no matching item")?;
                if item["type"] != "fileChange" {
                    return Err("Patch update type mismatch".into());
                }
                item["changes"] = p["changes"].clone();
            }
            "item/completed" => {
                s.items.remove(&key);
            }
            _ => {}
        }
        Ok(true)
    }

    pub fn cancel_wire(&self, scope: &Scope, wire: &Value) -> Result<(), String> {
        let mut s = self.lock()?;
        active(&s, scope)?;
        let id = s
            .seen_wire
            .get(&(scope.connection_id.clone(), wire.to_string()))
            .cloned();
        if let Some(id) = id {
            if s.entries.get(&id).is_some_and(|r| {
                r.status == RequestStatus::Pending || r.delivery == DeliveryState::Ready
            }) {
                clear_waiter(
                    &mut s,
                    &id,
                    RequestStatus::Cancelled,
                    "Provider cancelled the request",
                    &self.events,
                )?;
            }
        }
        Ok(())
    }

    pub fn add_rule(&self, scope: &Scope, command: &str, cwd: &str) -> Result<ExactRule, String> {
        let mut s = self.lock()?;
        let session = active(&s, scope)?;
        if scope.provider != Provider::Codex {
            return Err(
                "Claude MCP does not attest the effective shell directory; use one-time approval"
                    .into(),
            );
        }
        let rule = ExactRule::new(scope, command, cwd)?;
        if Path::new(&rule.cwd) != Path::new(&session.cwd) {
            return Err("Whitelist is restricted to this session directory".into());
        }
        s.rules.push(rule.clone());
        Ok(rule)
    }

    pub fn rules(&self, scope: &Scope) -> Result<Vec<ExactRule>, String> {
        Ok(self
            .lock()?
            .rules
            .iter()
            .filter(|r| r.scope == *scope)
            .cloned()
            .collect())
    }

    pub fn remove_rule(&self, scope: &Scope, id: &str) -> Result<(), String> {
        let mut s = self.lock()?;
        active(&s, scope)?;
        s.rules.retain(|r| !(r.scope == *scope && r.id == id));
        Ok(())
    }
}

fn active<'a>(s: &'a State, scope: &Scope) -> Result<&'a SessionView, String> {
    if !s.db.is_current()? {
        return Err(
            "Gatekeeper journal was recovered by another instance; stop this provider connection"
                .into(),
        );
    }
    let session = s
        .sessions
        .get(&scope.connection_id)
        .ok_or("No live session for this connection")?;
    if session.scope != *scope || !matches!(session.availability, Availability::Available { .. }) {
        return Err("Gatekeeper unavailable for this session connection".into());
    }
    Ok(session)
}

fn persist(db: &Journal, request: &ApprovalRequest) -> Result<(), String> {
    let changed = db.connection.execute("INSERT INTO gatekeeper_requests(id,body) SELECT ?1,?2 WHERE EXISTS(SELECT 1 FROM gatekeeper_meta WHERE id=1 AND owner=?3) ON CONFLICT(id) DO UPDATE SET body=excluded.body", params![request.id, serde_json::to_string(request).map_err(|e| e.to_string())?, db.owner]).map_err(|e| e.to_string())?;
    if changed != 1 {
        return Err("Gatekeeper journal ownership changed; no authorization sent".into());
    }
    Ok(())
}

fn settle(
    s: &mut State,
    id: &str,
    action: Action,
    status: RequestStatus,
    source: &str,
    note: &str,
    events: &broadcast::Sender<GatekeeperEvent>,
) -> Result<(), String> {
    let mut request = s.entries.get(id).ok_or("Unknown approval")?.clone();
    if request.status != RequestStatus::Pending {
        return Err("Approval already settled".into());
    }
    let waiting = s.waiting.get(id).ok_or("No live approval waiter")?;
    let response = adapters::response(&request.scope.provider, &waiting.draft, &action, note);
    request.status = status;
    request.delivery = DeliveryState::Ready;
    request.decision = Some(action);
    request.decision_source = Some(source.into());
    request.note = Some(note.into());
    // Commit before releasing a response to a provider process.
    persist(&s.db, &request)?;
    s.entries.insert(id.into(), request.clone());
    let waiting = s.waiting.remove(id).ok_or("Approval waiter disappeared")?;
    if waiting
        .sender
        .send(Delivery {
            request_id: id.into(),
            response,
        })
        .is_err()
    {
        request.delivery = DeliveryState::Uncertain;
        persist(&s.db, &request)?;
        s.entries.insert(id.into(), request.clone());
    }
    let _ = events.send(GatekeeperEvent::Changed(Box::new(request)));
    Ok(())
}

fn clear_waiter(
    s: &mut State,
    id: &str,
    status: RequestStatus,
    note: &str,
    events: &broadcast::Sender<GatekeeperEvent>,
) -> Result<(), String> {
    let mut request = s.entries.get(id).ok_or("Unknown approval")?.clone();
    request.status = status;
    if request.delivery == DeliveryState::Ready {
        request.delivery = DeliveryState::Uncertain;
    }
    request.note = Some(note.into());
    persist(&s.db, &request)?;
    s.entries.insert(id.into(), request.clone());
    s.waiting.remove(id);
    let _ = events.send(GatekeeperEvent::Changed(Box::new(request)));
    Ok(())
}
