//! Bounded, content-free observations of a provider invocation. Silence is not a failure.
use crate::models::{now, Activity, Message};
use serde::Serialize;
use std::collections::HashSet;

pub(crate) fn signal(emit: &mut impl FnMut(Activity), milestone: &str) {
    emit(Activity {
        kind: "latency".into(),
        text: serde_json::json!({"label":milestone,"atMs":clock_ms()}).to_string(),
    });
}
fn clock_ms() -> u64 {
    static ORIGIN: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    ORIGIN
        .get_or_init(std::time::Instant::now)
        .elapsed()
        .as_millis()
        .min(u64::MAX as u128) as u64
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    version: u8,
    pub id: String,
    provider: String,
    pub phase: String,
    started_at: i64,
    last_event_at: Option<i64>,
    process_started_at: Option<i64>,
    first_provider_event_at: Option<i64>,
    first_text_at: Option<i64>,
    ended_at: Option<i64>,
    events: u64,
    tool_events: u64,
    pending_tools: usize,
    pending_questions: usize,
    last_kind: String,
    timeline: Vec<(String, i64)>,
    timings: Vec<(String, u64)>,
}
pub struct Trace {
    snapshot: Snapshot,
    tools: HashSet<String>,
    questions: HashSet<String>,
    published: std::time::Instant,
    started: u64,
}
impl Trace {
    pub fn new(provider: &str) -> Self {
        let at = now();
        Self {
            snapshot: Snapshot {
                version: 1,
                id: crate::models::id(),
                provider: provider.into(),
                phase: "catalog".into(),
                started_at: at,
                last_event_at: None,
                process_started_at: None,
                first_provider_event_at: None,
                first_text_at: None,
                ended_at: None,
                events: 0,
                tool_events: 0,
                pending_tools: 0,
                pending_questions: 0,
                last_kind: String::new(),
                timeline: vec![("catalog".into(), at)],
                timings: vec![("catalog".into(), 0)],
            },
            tools: HashSet::new(),
            questions: HashSet::new(),
            published: std::time::Instant::now(),
            started: clock_ms(),
        }
    }
    pub fn phase(&mut self, phase: &str) {
        if self.snapshot.phase != phase {
            self.snapshot.phase = phase.into();
            if matches!(
                phase,
                "capacity" | "starting-provider" | "returned" | "cancelled" | "error"
            ) {
                self.mark(phase);
            }
            self.snapshot.timeline.push((phase.into(), now()));
            if self.snapshot.timeline.len() > 32 {
                self.snapshot.timeline.remove(1);
            }
        }
    }
    pub fn observe(&mut self, event: &Activity) -> bool {
        if event.kind == "latency" {
            let Ok(value) = serde_json::from_str::<serde_json::Value>(&event.text) else {
                return false;
            };
            let (Some(label), Some(at)) = (value["label"].as_str(), value["atMs"].as_u64()) else {
                return false;
            };
            if at < self.started || at > clock_ms() {
                return false;
            }
            if matches!(
                label,
                "adapter-start"
                    | "spawn-start"
                    | "protocol-ready"
                    | "request-enqueued"
                    | "request-acknowledged"
                    | "version-ready"
                    | "schema-ready"
                    | "thread-ready"
                    | "process-started"
            ) {
                self.mark_at(label, at);
                return true;
            }
            return false;
        }
        let at = now();
        let previous = self.snapshot.phase.clone();
        let kind = event.kind.as_str();
        if kind == "pid" {
            if !self
                .snapshot
                .timings
                .iter()
                .any(|(name, _)| name == "process-started")
            {
                self.mark("process-started");
            }
            self.snapshot.process_started_at.get_or_insert(at);
            self.phase("waiting-provider");
        } else {
            if self.snapshot.first_provider_event_at.is_none() {
                self.mark("first-provider-event");
            }
            self.snapshot.events += 1;
            self.snapshot.last_event_at = Some(at);
            self.snapshot.first_provider_event_at.get_or_insert(at);
            self.snapshot.last_kind = match kind {
                "delta" | "text" | "replace" | "final" => "text",
                "thinking" => "reasoning",
                "tool" | "file" | "tool-start" | "tool-end" => "tool",
                "interaction" | "interaction-result" => "question",
                "error" => "error",
                "session" => "initialized",
                _ => "provider-event",
            }
            .into();
            match kind {
                "delta" | "text" | "replace" | "final" if !event.text.is_empty() => {
                    if self.snapshot.first_text_at.is_none() {
                        self.mark("first-text");
                    }
                    self.snapshot.first_text_at.get_or_insert(at);
                    self.phase("response");
                }
                "thinking" => self.phase("reasoning"),
                "tool" | "file" => {
                    self.snapshot.tool_events += 1;
                    self.phase("tool-activity");
                }
                "tool-start" | "tool-end" | "interaction" | "interaction-result" => {
                    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&event.text) {
                        if let Some(id) = value["id"].as_str().filter(|id| id.len() <= 256) {
                            match kind {
                                "tool-start" => {
                                    if self.tools.len() < 256 {
                                        self.tools.insert(id.into());
                                    }
                                }
                                "tool-end" => {
                                    self.tools.remove(id);
                                }
                                "interaction" => {
                                    if self.questions.len() < 256 {
                                        self.questions.insert(id.into());
                                    }
                                }
                                _ => {
                                    self.questions.remove(id);
                                }
                            }
                        }
                    }
                    self.phase("waiting-provider");
                }
                _ => {}
            }
        }
        // Only explicit start/end pairs establish that a tool is still running.
        self.snapshot.pending_tools = self.tools.len();
        self.snapshot.pending_questions = self.questions.len();
        if !self.tools.is_empty() {
            self.phase("tool");
        }
        if !self.questions.is_empty() {
            self.phase("question");
        }
        previous != self.snapshot.phase
            || self.snapshot.events == 1
            || matches!(
                kind,
                "tool-start" | "tool-end" | "interaction" | "interaction-result"
            )
    }
    pub fn finish(&mut self, result: &Result<String, String>) {
        self.mark("return");
        self.snapshot.ended_at = Some(now());
        self.phase(match result {
            Ok(_) => "returned",
            Err(e) if e == "Cancelled" => "cancelled",
            Err(_) => "error",
        });
    }
    fn mark(&mut self, name: &str) {
        self.mark_at(name, clock_ms());
    }
    fn mark_at(&mut self, name: &str, at: u64) {
        if self.snapshot.timings.len() < 64 {
            self.snapshot
                .timings
                .push((name.into(), at.saturating_sub(self.started)));
        }
    }
    pub fn publish(
        &mut self,
        app: &tauri::AppHandle,
        session: &str,
        out: &mut Message,
        force: bool,
    ) {
        if !force && self.published.elapsed().as_millis() < 750 {
            return;
        }
        self.published = std::time::Instant::now();
        let text = serde_json::to_string(&self.snapshot).unwrap_or_default();
        crate::event(app, session, &out.id, "execution", &text);
        crate::activity::record(
            &mut out.activity,
            Activity {
                kind: "execution".into(),
                text,
            },
        );
    }
}

pub fn phase(app: &tauri::AppHandle, session: &str, out: &mut Message, phase: &str) {
    crate::turns::note(
        app,
        session,
        out,
        "execution-phase",
        serde_json::json!({"phase":phase,"at":now()}).to_string(),
    );
}
pub fn merge(target: &mut Message, source: &Message) {
    for row in source.activity.iter().filter(|a| a.kind == "execution") {
        crate::activity::record(&mut target.activity, row.clone());
    }
}
pub fn queued(out: &mut Message) {
    crate::activity::record(
        &mut out.activity,
        Activity {
            kind: "execution-phase".into(),
            text: serde_json::json!({"phase":"waiting-start","at":now()}).to_string(),
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn latency_is_monotonic_bounded_and_never_provider_output() {
        let mut t = Trace::new("test");
        let mut queued = vec![];
        signal(&mut |a| queued.push(a), "adapter-start");
        let captured = serde_json::from_str::<serde_json::Value>(&queued[0].text).unwrap()["atMs"]
            .as_u64()
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(5));
        t.observe(&queued[0]);
        assert_eq!(t.snapshot.timings.last().unwrap().1, captured - t.started);
        assert!(!t.observe(&event("latency", "secret command or arbitrary payload")));
        signal(
            &mut |a| {
                t.observe(&a);
            },
            "spawn-start",
        );
        t.observe(&event("pid", "1234"));
        signal(
            &mut |a| {
                t.observe(&a);
            },
            "protocol-ready",
        );
        assert_eq!(t.snapshot.events, 0);
        assert!(t.snapshot.first_provider_event_at.is_none());
        assert!(t.snapshot.first_text_at.is_none());
        t.observe(&event("delta", "secret response"));
        t.finish(&Ok(String::new()));
        assert!(t.snapshot.timings.windows(2).all(|w| w[0].1 <= w[1].1));
        assert!(t.snapshot.timings.iter().any(|(n, _)| n == "first-text"));
        let text = serde_json::to_string(&t.snapshot).unwrap();
        assert!(!text.contains("secret"));
        for _ in 0..1000 {
            signal(
                &mut |a| {
                    t.observe(&a);
                },
                "spawn-start",
            );
        }
        assert_eq!(t.snapshot.timings.len(), 64);
    }
    fn event(kind: &str, text: &str) -> Activity {
        Activity {
            kind: kind.into(),
            text: text.into(),
        }
    }
    #[test]
    fn scratch_observations_are_merged_and_survive_storage_without_response_text() {
        let db = crate::store::Store::open(std::path::Path::new(":memory:")).unwrap();
        let session = db.create("/project".into()).unwrap();
        let mut target = Message {
            id: crate::models::id(),
            role: "assistant".into(),
            text: "Visible answer".into(),
            activity: vec![],
        };
        let mut source = target.clone();
        source.text = "Hidden prompt/response".into();
        source.activity.push(event(
            "execution",
            &serde_json::to_string(&Trace::new("claude").snapshot).unwrap(),
        ));
        merge(&mut target, &source);
        db.save_message(&session.id, &target).unwrap();
        let stored = db.messages(&session.id).unwrap().pop().unwrap();
        assert_eq!(stored.text, "Visible answer");
        assert_eq!(stored.activity[0].kind, "execution");
        assert!(!stored.activity[0].text.contains("Hidden"));
    }
    #[test]
    fn lifecycle_pressure_keeps_questions_checkpoints_and_bounded_traces() {
        let mut rows = vec![
            event("checkpoint", "tree"),
            event("interaction", r#"{"id":"q"}"#),
            event("native", r#"{"id":"native"}"#),
        ];
        for i in 0..350 {
            for kind in ["tool-start", "tool-end"] {
                crate::activity::record(
                    &mut rows,
                    event(kind, &format!("{{\"id\":\"tool-{i}\"}}")),
                );
            }
            let t = Trace::new("claude");
            crate::activity::record(
                &mut rows,
                event("execution", &serde_json::to_string(&t.snapshot).unwrap()),
            );
        }
        assert!(rows.len() <= 500);
        assert!(rows.iter().filter(|a| a.kind == "execution").count() <= 64);
        assert_eq!(rows[0].kind, "checkpoint");
        assert_eq!(rows[1].kind, "interaction");
        assert_eq!(rows[2].kind, "native");
    }
    #[test]
    fn tool_results_do_not_claim_a_running_tool_and_questions_take_precedence() {
        let mut t = Trace::new("claude");
        t.observe(&event("tool", "private command/output"));
        assert_eq!(t.snapshot.phase, "tool-activity");
        t.observe(&event("tool-start", r#"{"id":"a"}"#));
        t.observe(&event(
            "interaction",
            r#"{"id":"q","questions":[{"secret":true}]}"#,
        ));
        assert_eq!(t.snapshot.phase, "question");
        t.observe(&event(
            "interaction-result",
            r#"{"id":"q","answers":{"secret":"password"}}"#,
        ));
        assert_eq!(t.snapshot.phase, "tool");
        t.observe(&event("tool-end", r#"{"id":"a"}"#));
        assert_eq!(t.snapshot.phase, "waiting-provider");
        let serialized = serde_json::to_string(&t.snapshot).unwrap();
        assert!(!serialized.contains("password") && !serialized.contains("private"));
    }
    #[test]
    fn ended_provider_with_no_tools_is_a_return_not_task_completion() {
        let mut t = Trace::new("codex");
        t.observe(&event("delta", "I will implement this."));
        t.finish(&Ok("I will implement this.".into()));
        assert_eq!(t.snapshot.phase, "returned");
        assert_eq!(t.snapshot.tool_events, 0);
        assert!(t.snapshot.first_text_at.is_some() && t.snapshot.ended_at.is_some());
        for _ in 0..100 {
            t.phase("reasoning");
            t.phase("response");
        }
        assert!(t.snapshot.timeline.len() <= 32);
    }
}
