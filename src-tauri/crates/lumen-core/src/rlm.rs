//! Turn-local data and inference for Code Mode. Children have only the nested query tool.
use crate::{
    api, api_wire,
    models::{Account, Activity, Message, Provider},
};
use futures_util::{future::BoxFuture, FutureExt};
use serde_json::{json, Value};
use std::{
    io::Read,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicUsize, Ordering::Relaxed},
        Arc, Mutex,
    },
};
use tokio::sync::{OnceCell, Semaphore};

pub(crate) const STORE_LIMIT: usize = 32 * 1024 * 1024;
const RESULT_LIMIT: usize = 24 * 1024;
const QUERY_LIMIT: usize = 256 * 1024;
const CHILD: &str = "lumen_llm";

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Deserialize, serde::Serialize,
)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Effort {
    Low,
    Medium,
    High,
    Max,
}

impl Effort {
    fn parse(raw: &str) -> Option<Self> {
        Some(match raw {
            "low" => Self::Low,
            "medium" => Self::Medium,
            "high" => Self::High,
            "max" => Self::Max,
            _ => return None,
        })
    }

    pub(crate) fn for_turn(raw: &str, default: Self) -> Self {
        Self::parse(raw).unwrap_or(default)
    }

    pub(crate) fn limits(self) -> (usize, usize, usize) {
        match self {
            Self::Low => (0, 0, 0),
            Self::Medium => (8, 1, 4),
            Self::High => (32, 1, 8),
            Self::Max => (128, 2, 8),
        }
    }
}

#[derive(Clone, Default)]
pub(crate) struct Sources {
    documents: Documents,
    pub(crate) accounts: Vec<Account>,
}

impl Sources {
    pub(crate) fn new(accounts: Vec<Account>) -> Self {
        Self {
            accounts,
            ..Self::default()
        }
    }

    pub(crate) fn add(&mut self, label: &str, text: String) {
        self.documents.add(label, text);
    }

    pub(crate) fn capture(
        &mut self,
        workspace: &Path,
        attachments: &[String],
        messages: &[Message],
    ) {
        let mut seen = std::collections::HashSet::new();
        for selected in attachments.iter().take(50) {
            let Ok(path) = crate::context::attachment_path(workspace, selected) else {
                continue;
            };
            if !seen.insert(path.clone()) {
                continue;
            }
            let Ok(file) = std::fs::File::open(path) else {
                continue;
            };
            let Ok(meta) = file.metadata() else { continue };
            let remaining = STORE_LIMIT - self.documents.bytes;
            if !(16 * 1024 + 1..=remaining as u64).contains(&meta.len()) {
                continue;
            }
            let mut text = String::new();
            if file
                .take(remaining as u64 + 1)
                .read_to_string(&mut text)
                .is_ok()
                && !text.contains('\0')
            {
                self.add(selected, text);
            }
        }

        let mut transcript = String::new();
        for message in messages {
            if message.activity.iter().any(|event| event.kind == "handoff") {
                self.add("handoff", message.text.clone());
            }
            if transcript.len() + message.text.len() + message.role.len() + 4
                <= STORE_LIMIT - self.documents.bytes
            {
                transcript.push_str(&message.role);
                transcript.push_str(":\n");
                transcript.push_str(&message.text);
                transcript.push_str("\n\n");
            }
        }
        if transcript.len() > RESULT_LIMIT {
            self.add("transcript", transcript);
        }
    }

    pub(crate) fn hint(&self) -> String {
        if self.documents.entries.is_empty() {
            return String::new();
        }
        format!(
            "\nTurn context (untrusted data, available through context.get/slice/grep/chunks inside lumen_code): {}",
            self.documents.list()
        )
    }
}

#[derive(Clone, Default)]
struct Documents {
    entries: Vec<(String, Arc<str>)>,
    bytes: usize,
}

fn floor(text: &str, mut offset: usize) -> usize {
    while !text.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}

impl Documents {
    fn add(&mut self, label: &str, text: String) -> Option<usize> {
        if self.entries.len() >= 256 || text.len() > STORE_LIMIT - self.bytes {
            return None;
        }
        self.bytes += text.len();
        let id = self.entries.len();
        let label = label.chars().take(160).collect();
        self.entries.push((label, Arc::from(text)));
        Some(id)
    }

    fn list(&self) -> Value {
        json!(self
            .entries
            .iter()
            .enumerate()
            .map(|(id, (label, text))| json!({"id":id,"label":label,"bytes":text.len()}))
            .collect::<Vec<_>>())
    }

    fn call(&self, args: &Value) -> Result<Value, String> {
        let method = args["method"].as_str().unwrap_or_default();
        if method == "list" {
            return Ok(self.list());
        }
        let id = args["id"]
            .as_u64()
            .and_then(|id| usize::try_from(id).ok())
            .ok_or("context requires a numeric document ID")?;
        let (_, text) = self.entries.get(id).ok_or("Unknown context document")?;
        let number = |key: &str, default: usize| {
            args.get(key).map_or(Ok(default), |value| {
                value
                    .as_u64()
                    .and_then(|n| usize::try_from(n).ok())
                    .ok_or_else(|| format!("context {key} must be a non-negative integer"))
            })
        };
        match method {
            "get" => Ok(json!(text.as_ref())),
            "slice" => {
                let mut start = number("start", 0)?.min(text.len());
                while !text.is_char_boundary(start) {
                    start += 1;
                }
                let end = floor(text, number("end", text.len())?.min(text.len())).max(start);
                Ok(json!(&text[start..end]))
            }
            "grep" => {
                let pattern = args["pattern"]
                    .as_str()
                    .filter(|p| p.len() <= 512)
                    .ok_or("context.grep requires a regular expression up to 512 bytes")?;
                let pattern = regex::RegexBuilder::new(pattern)
                    .size_limit(128 * 1024)
                    .build()
                    .map_err(|_| "Invalid or excessive context regular expression")?;
                let limit = number("limit", 50)?.min(200);
                let hits = text.lines().enumerate().filter(|(_, line)| pattern.is_match(line)).take(limit)
                    .map(|(n, line)| json!({"line":n + 1,"text":&line[..floor(line, line.len().min(4096))]}))
                    .collect::<Vec<_>>();
                Ok(json!(hits))
            }
            "chunks" => {
                let n = number("size", 4096)?.clamp(256, 64 * 1024);
                let mut chunks = Vec::new();
                let mut rest: &str = text;
                while !rest.is_empty() {
                    let split = floor(rest, rest.len().min(n));
                    let (chunk, next) = rest.split_at(split);
                    chunks.push(chunk);
                    rest = next;
                }
                Ok(json!(chunks))
            }
            _ => Err("Unknown context operation".into()),
        }
    }
}

pub(crate) struct Turn {
    data: PathBuf,
    endpoint: String,
    local_model: Option<String>,
    accounts: Vec<Account>,
    docs: Mutex<Documents>,
    effort: Effort,
    calls: Mutex<usize>,
    slots: Semaphore,
    target: OnceCell<crate::router::RlmTarget>,
    pub(crate) usage: Usage,
}

#[derive(Default)]
pub(crate) struct Usage {
    pub(crate) rounds: AtomicUsize,
    pub(crate) input: AtomicUsize,
    pub(crate) output: AtomicUsize,
}

impl Turn {
    pub(crate) fn new(
        data: &Path,
        endpoint: &str,
        local_model: Option<&str>,
        effort: Effort,
        sources: &Sources,
    ) -> Self {
        Self {
            data: data.into(),
            endpoint: endpoint.into(),
            local_model: local_model.map(str::to_owned),
            accounts: sources.accounts.clone(),
            docs: Mutex::new(sources.documents.clone()),
            effort,
            calls: Mutex::new(0),
            slots: Semaphore::new(effort.limits().2),
            target: OnceCell::new(),
            usage: Usage::default(),
        }
    }

    pub(crate) fn from_invocation(
        input: &crate::providers::Invocation<'_>,
        data: &Path,
        endpoint: &str,
        effort: Effort,
    ) -> Self {
        let local_model = if input.account.provider == Provider::Ollama {
            input.model
        } else {
            None
        };
        Self::new(
            data,
            endpoint,
            local_model,
            effort,
            input.rlm.unwrap_or(&Sources::default()),
        )
    }

    pub(crate) fn document(&self, args: &Value) -> Result<Value, String> {
        self.docs.lock().unwrap().call(args)
    }

    pub(crate) fn retain_tool_text(&self, name: &str, result: &mut Value) {
        let Some(blocks) = result["content"].as_array_mut() else {
            return;
        };
        for block in blocks {
            if block["type"] != "text" {
                continue;
            }
            let Some(text) = block["text"].as_str().filter(|s| s.len() > RESULT_LIMIT) else {
                continue;
            };
            let mut docs = self.docs.lock().unwrap();
            let Some(id) = docs.add(name, text.into()) else {
                continue;
            };
            let mut preview = text.to_owned();
            crate::wire::bound("code-output", &mut preview);
            block["text"] = json!(format!(
                "Context document {id}, read with context.get({id}).\n{preview}"
            ));
            block["contextId"] = json!(id);
        }
    }

    pub(crate) fn inference_enabled(&self) -> bool {
        self.effort != Effort::Low
    }

    pub(crate) async fn ask(
        &self,
        args: &Value,
        emit: &(impl Fn(Activity) + Sync),
    ) -> Result<String, String> {
        self.query(args, self.effort, self.effort.limits().1, emit, &self.usage)
            .await
    }

    fn query<'a>(
        &'a self,
        args: &'a Value,
        ceiling: Effort,
        depth: usize,
        emit: &'a (impl Fn(Activity) + Sync),
        usage: &'a Usage,
    ) -> BoxFuture<'a, Result<String, String>> {
        async move {
            if depth == 0 {
                return Err("llm() is disabled at this effort or depth".into());
            }
            let prompt = args["prompt"].as_str().filter(|p| !p.trim().is_empty() && p.len() <= QUERY_LIMIT)
                .ok_or("llm() requires a non-empty prompt within 256 KiB")?;
            let effort = match args.get("effort") {
                None => ceiling,
                Some(raw) => Effort::parse(raw.as_str().unwrap_or_default())
                    .filter(|effort| *effort <= ceiling)
                    .ok_or("llm() effort must be low, medium, high or max, within the parent's effort")?,
            };
            let max = match args.get("maxTokens") {
                None => 512,
                Some(value) => value.as_u64().filter(|&n| (16..=4096).contains(&n))
                    .ok_or("llm() maxTokens must be an integer between 16 and 4096")? as u32,
            };
            {
                let mut calls = self.calls.lock().unwrap();
                if *calls >= self.effort.limits().0 {
                    return Err("llm() turn call limit reached".into());
                }
                *calls += 1;
            }

            let target = self.target.get_or_try_init(|| async {
                crate::router::rlm_target(&self.data, &self.endpoint, self.local_model.as_deref(), &self.accounts).await
            }).await?;
            let provider = target.provider();
            let mut history = api_wire::initial(&provider, prompt);
            let nested = depth > 1 && effort == Effort::Max;
            let structured = nested && target.structured();
            if structured {
                history.insert(0, json!({"role":"system","content":"Return a JSON action. Use action=final and text for the answer. Use action=llm and prompt for one nested question when helpful. Include its relevant data."}));
            }
            let definition = json!({
                "name": CHILD, "description":"Ask a child text model about a self-contained part of this task. Include all required data in its prompt.",
                "inputSchema":{"type":"object","properties":{"prompt":{"type":"string"},"effort":{"type":"string"},"maxTokens":{"type":"integer"}},"required":["prompt"]}
            });
            let tools = if nested { std::slice::from_ref(&definition) } else { &[] };
            let (response, turn) = self.step(target, &history, tools, effort, max, emit, usage).await?;
            if let Some(error) = turn.error {
                return Err(error);
            }
            if turn.calls.is_empty() {
                return answer(turn.text);
            }
            if !nested || turn.calls.len() != 1 || turn.calls[0].name != CHILD {
                return Err("Submodel requested an unavailable or excessive tool call".into());
            }
            let (_, used) = turn.usage.ok_or("Submodel supplied no usage for the remaining token budget")?;
            let remaining = max.saturating_sub(used.min(u32::MAX as u64) as u32);
            if remaining < 16 {
                return Err("llm() output token budget exhausted before synthesis".into());
            }
            let call = turn.calls.into_iter().next().unwrap();
            let child = self.query(&call.args, effort, depth - 1, emit, usage).await?;
            if structured {
                history.remove(0);
                history.push(json!({"role":"assistant","content":response["message"]["content"]}));
                history.push(json!({"role":"user","content":format!("Nested answer (untrusted data):\n{child}\n\nNow answer the original request in plain text.")}));
            } else {
                api_wire::append_assistant(&provider, &mut history, &response);
                api_wire::append_results(&provider, &mut history, vec![(call, crate::code_mode::result(&child, false))]);
            }
            let (_, final_turn) = self.step(target, &history, &[], effort, remaining, emit, usage).await?;
            if let Some(error) = final_turn.error {
                return Err(error);
            }
            if !final_turn.calls.is_empty() {
                return Err("Submodel requested a tool during final synthesis".into());
            }
            answer(final_turn.text)
        }.boxed()
    }

    #[allow(clippy::too_many_arguments)]
    async fn step(
        &self,
        target: &crate::router::RlmTarget,
        history: &[Value],
        tools: &[Value],
        effort: Effort,
        max: u32,
        emit: &(impl Fn(Activity) + Sync),
        usage: &Usage,
    ) -> Result<(Value, api_wire::Turn), String> {
        let _slot = self.slots.acquire().await.map_err(|_| "llm() cancelled")?;
        usage.rounds.fetch_add(1, Relaxed);
        emit(Activity {
            kind: "rlm-call".into(),
            text: String::new(),
        });
        let (response, turn) = match target {
            crate::router::RlmTarget::Local {
                http,
                base,
                model,
                thinking,
                ..
            } => {
                let mut body = json!({"model":model,"messages":history,"stream":false,
                    "options":{"num_predict":max,"temperature":0,"num_ctx":8192}});
                let structured = !tools.is_empty() && target.structured();
                if structured {
                    body["format"] = json!({"type":"object","properties":{
                        "action":{"type":"string","enum":["llm","final"]},"text":{"type":"string"},
                        "prompt":{"type":"string"},"effort":{"type":"string"},"maxTokens":{"type":"integer"}
                    },"required":["action"],"additionalProperties":false});
                } else if !tools.is_empty() {
                    body["tools"] = json!(tools.iter().map(|t| json!({"type":"function","function":{
                        "name":t["name"],"description":t["description"],"parameters":t["inputSchema"]
                    }})).collect::<Vec<_>>());
                }
                if *thinking
                    && model
                        .rsplit('/')
                        .next()
                        .is_some_and(|id| id.starts_with("gpt-oss:"))
                {
                    body["think"] = json!(match effort {
                        Effort::Low => "low",
                        Effort::Medium => "medium",
                        _ => "high",
                    });
                }
                let raw = crate::ollama_agent::post(http, base, "api/chat", &body).await?;
                let response = bounded_json(raw).await?;
                crate::ollama_agent::local_model(model, &response)?;
                let mut turn = api_wire::response(&Provider::Ollama, &response)?;
                if structured && turn.error.is_none() && turn.calls.is_empty() {
                    let action: Value = serde_json::from_str(&turn.text)
                        .map_err(|_| "Submodel returned an invalid JSON action")?;
                    match action["action"].as_str() {
                        Some("final") => {
                            turn.text = action["text"].as_str().unwrap_or_default().into()
                        }
                        Some("llm") => turn.calls.push(api_wire::Call {
                            id: String::new(),
                            name: CHILD.into(),
                            args: action,
                        }),
                        _ => return Err("Submodel requested an invalid JSON action".into()),
                    }
                }
                (response, turn)
            }
            crate::router::RlmTarget::Api { account, model } => {
                api::Client::new(&self.data, account, model, true)?
                    .step(history, tools, Some(max), emit)
                    .await?
            }
        };
        if let Some((input, output)) = turn.usage {
            usage.input.fetch_add(input as usize, Relaxed);
            usage.output.fetch_add(output as usize, Relaxed);
        }
        Ok((response, turn))
    }
}

fn answer(text: String) -> Result<String, String> {
    if text.trim().is_empty() || text.len() > QUERY_LIMIT {
        return Err("Submodel returned no text or exceeded its output limit".into());
    }
    Ok(text)
}

pub(crate) async fn bounded_json(mut response: reqwest::Response) -> Result<Value, String> {
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
        if bytes.len() + chunk.len() > 2 * 1024 * 1024 {
            return Err("Ollama response exceeded 2 MiB".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes).map_err(|_| "Invalid Ollama JSON response".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{code_mode::Session, test_support::Fixture};
    use axum::{routing::post, Json, Router};
    use std::time::Duration;
    use tokio::sync::watch;

    #[tokio::test]
    async fn context_operations_keep_utf8_and_large_tool_data() {
        let fixture = Fixture::new();
        let mut sources = Sources::default();
        sources.add("document", "éx\nmatch".into());
        let (_stop, cancel) = watch::channel(false);
        let sandbox = Session::new(cancel).unwrap().with_rlm(Turn::new(
            &fixture.root,
            "127.0.0.1:1",
            Some("fixture"),
            Effort::Low,
            &sources,
        ));
        let unit = |name: &str| json!({"name":name});
        let tool = [unit("probe")];
        let output = sandbox
            .run(
                r#"(async () => {
                const r = await tools.probe();
                return [
                    await context.list(),
                    await context.slice(0, 0, 1),
                    await context.slice(0, 1, 3),
                    (await context.grep(0, '^match'))[0],
                    (await context.chunks(0, 256))[0],
                    (await context.get(r.content[0].contextId)).length,
                    typeof llm
                ];
            })()"#,
                &tool,
                |_, _| async { crate::code_mode::result(&"x".repeat(30_000), false) },
            )
            .await
            .unwrap();
        let value: Value = serde_json::from_str(&output).unwrap();
        assert_eq!(value[0][0]["label"], "document");
        assert_eq!(value[0][1]["bytes"], 30_000);
        assert_eq!(value[1], "");
        assert_eq!(value[2], "x");
        assert_eq!(value[3], json!({"line":2,"text":"match"}));
        assert_eq!(value[4], "éx\nmatch");
        assert_eq!(value[5], 30_000);
        assert_eq!(value[6], "undefined");
    }

    #[tokio::test]
    async fn effort_limits_hold_across_parallel_and_nested_requests() {
        let fixture = Fixture::new();
        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let observed = peak.clone();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap().to_string();
        let router = Router::new()
            .route("/api/show", post(|Json(body): Json<Value>| async move {
                let capabilities = if body["model"] == "fixture-json" { vec!["completion"] }
                    else { vec!["completion", "tools"] };
                Json(json!({"capabilities":capabilities}))
            }))
            .route("/api/chat", post(move |Json(body): Json<Value>| {
                let (active, peak) = (active.clone(), peak.clone());
                async move {
                    let count = active.fetch_add(1, Relaxed) + 1;
                    peak.fetch_max(count, Relaxed);
                    tokio::time::sleep(Duration::from_millis(20)).await;
                    active.fetch_sub(1, Relaxed);
                    if body["model"] == "fixture-json" {
                        assert!(body["tools"].is_null());
                        let messages = body["messages"].as_array().unwrap();
                        let content = if !body["format"].is_null() {
                            assert_eq!(messages[1]["content"], "outer");
                            assert_eq!(body["options"]["num_predict"], 80);
                            json!({"action":"llm","prompt":"inner","maxTokens":32}).to_string()
                        } else if messages[0]["content"] == "inner" {
                            assert_eq!(body["options"]["num_predict"], 32);
                            "leaf".into()
                        } else {
                            assert_eq!(body["options"]["num_predict"], 70);
                            assert!(messages[2]["content"].as_str().unwrap().contains("leaf"));
                            "merged".into()
                        };
                        return Json(json!({"message":{"content":content},"done":true,"prompt_eval_count":20,"eval_count":10}));
                    }
                    assert_eq!(body["model"], "fixture");
                    let prompt = body["messages"][0]["content"].as_str().unwrap();
                    let message = match prompt {
                        "outer" if body["messages"].as_array().unwrap().len() == 1 => {
                            assert_eq!(body["tools"][0]["function"]["name"], CHILD);
                            assert_eq!(body["options"]["num_predict"], 80);
                            json!({"content":"","tool_calls":[{"function":{
                                "name":CHILD,"arguments":{"prompt":"inner","maxTokens":32}
                            }}]})
                        }
                        "outer" => {
                            assert_eq!(body["options"]["num_predict"], 70);
                            assert!(body["tools"].is_null());
                            assert!(body["messages"][2]["content"].as_str().unwrap().contains("leaf"));
                            json!({"content":"merged"})
                        }
                        "inner" => {
                            assert_eq!(body["options"]["num_predict"], 32);
                            assert!(body["tools"].is_null());
                            json!({"content":"leaf"})
                        }
                        _ => {
                            assert!(body["tools"].is_null());
                            json!({"content":"ok"})
                        }
                    };
                    Json(json!({"message":message,"done":true,"prompt_eval_count":20,"eval_count":10}))
                }
            }));
        let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        for (effort, expected) in [
            (Effort::Low, (0, 0, 0)),
            (Effort::Medium, (8, 1, 4)),
            (Effort::High, (32, 1, 8)),
            (Effort::Max, (128, 2, 8)),
        ] {
            assert_eq!(effort.limits(), expected);
        }
        let sources = Sources::default();
        let turn = Turn::new(
            &fixture.root,
            &address,
            Some("fixture"),
            Effort::Medium,
            &sources,
        );
        let args = json!({"prompt":"parallel"});
        let emit = |_| {};
        let results = futures_util::future::join_all((0..10).map(|_| turn.ask(&args, &emit))).await;
        assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 8);
        assert_eq!(observed.load(Relaxed), 4);
        let raised = turn
            .ask(&json!({"prompt":"up","effort":"max"}), &emit)
            .await
            .unwrap_err();
        assert!(raised.contains("within the parent's effort"));
        for model in ["fixture", "fixture-json"] {
            let turn = Turn::new(&fixture.root, &address, Some(model), Effort::Max, &sources);
            let reported = AtomicUsize::new(0);
            let emit = |event: Activity| {
                if event.kind == "rlm-call" {
                    assert!(event.text.is_empty());
                    reported.fetch_add(1, Relaxed);
                }
            };
            assert_eq!(
                turn.ask(&json!({"prompt":"outer","maxTokens":80}), &emit)
                    .await
                    .unwrap(),
                "merged"
            );
            assert_eq!(turn.usage.rounds.load(Relaxed), 3);
            assert_eq!(reported.load(Relaxed), 3);
            assert_eq!(*turn.calls.lock().unwrap(), 2);
        }
        server.abort();
    }
}
