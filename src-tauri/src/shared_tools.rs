//! Turn-owned MCP client for local inference. Native account gateways remain the owners
//! of their authentication. Tool results are data; consent never enters model messages.
use crate::{
    browser::Registration,
    models::{AccessMode, Activity},
    providers::Invocation,
};
use lumen_gatekeeper::mcp::write_frame;
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};

struct Frames {
    reader: BufReader<tokio::process::ChildStdout>,
    pending: Vec<u8>,
}
impl Frames {
    // The buffer survives select cancellation when a GUI answer wins a race
    // against a partial server frame. Input remains bounded before allocation.
    async fn next(&mut self) -> Result<Value, String> {
        let remaining = (2_000_001usize).saturating_sub(self.pending.len());
        let count = (&mut self.reader)
            .take(remaining as u64)
            .read_until(b'\n', &mut self.pending)
            .await
            .map_err(|_| "Shared MCP read failed")?;
        if count == 0 && self.pending.is_empty() {
            return Err("Shared MCP service disconnected".into());
        }
        if self.pending.len() > 2_000_000 || self.pending.last() != Some(&b'\n') {
            return Err("Shared MCP frame is oversized or incomplete".into());
        }
        let message = serde_json::from_slice(&self.pending)
            .map_err(|_| "Invalid shared MCP frame".to_owned());
        self.pending.clear();
        message
    }
}

struct Session {
    _child: crate::proc::ServiceChild,
    input: tokio::process::ChildStdin,
    output: Frames,
    sequence: u64,
    server: String,
}

impl Session {
    async fn close(mut self) -> Result<(), String> {
        drop(self.input);
        self._child.finish(true).await
    }
    async fn start(
        config: &Value,
        server: &str,
        workspace: &Path,
        browser: &Registration,
    ) -> Result<Self, String> {
        let fields = config
            .as_object()
            .ok_or("Invalid shared MCP configuration")?;
        if fields
            .keys()
            .any(|key| !matches!(key.as_str(), "command" | "args" | "env" | "cwd" | "type"))
            || config.get("type").is_some_and(|value| value != "stdio")
            || config.get("args").is_some_and(|value| !value.is_array())
            || config.get("env").is_some_and(|value| !value.is_object())
            || config.get("cwd").is_some_and(|value| !value.is_string())
        {
            return Err(
                "Shared MCP settings require an adapter; their policies were not discarded".into(),
            );
        }
        let program = config["command"]
            .as_str()
            .ok_or("Ollama MCP currently requires stdio; HTTP needs a transport adapter")?;
        let mut command = crate::proc::command(crate::capability_catalog::expand(program)?);
        for arg in config["args"].as_array().into_iter().flatten() {
            command.arg(crate::capability_catalog::expand(
                arg.as_str().ok_or("Invalid MCP argument")?,
            )?);
        }
        for (key, value) in config["env"].as_object().into_iter().flatten() {
            command.env(
                key,
                crate::capability_catalog::expand(
                    value.as_str().ok_or("Invalid MCP environment")?,
                )?,
            );
        }
        let cwd = config["cwd"]
            .as_str()
            .map(PathBuf::from)
            .map(|p| {
                if p.is_absolute() {
                    p
                } else {
                    workspace.join(p)
                }
            })
            .unwrap_or_else(|| workspace.into());
        browser.env(&mut command);
        command
            .current_dir(cwd)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null());
        let mut child = crate::proc::ServiceChild::spawn(command)
            .map_err(|_| "Cannot start shared MCP service")?;
        Ok(Self {
            input: child.stdin.take().ok_or("MCP input unavailable")?,
            output: Frames {
                reader: BufReader::new(child.stdout.take().ok_or("MCP output unavailable")?),
                pending: vec![],
            },
            _child: child,
            sequence: 0,
            server: server.into(),
        })
    }
    async fn rpc(
        &mut self,
        method: &str,
        params: Value,
        browser: &Registration,
    ) -> Result<Value, String> {
        self.sequence += 1;
        let id = self.sequence;
        let operation = async {
            write_frame(
                &mut self.input,
                &json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}),
            )
            .await
            .map_err(|_| "Shared MCP write failed")?;
            loop {
                let message = self.output.next().await?;
                if message["jsonrpc"] != "2.0" {
                    return Err("Invalid shared MCP JSON-RPC version".into());
                }
                if message.get("method").is_none() {
                    if message["id"] != id {
                        return Err("Shared MCP response does not match its request".into());
                    }
                    if message.get("error").is_some() {
                        return Err("Shared MCP request failed; check the owning integration. No automatic retry.".into());
                    }
                    return Ok(message["result"].clone());
                }
                if let Some(request_id) = message.get("id") {
                    let response = if message["method"] == "elicitation/create" {
                        let mut params = message["params"].clone();
                        params["serverName"] = json!(self.server);
                        if params["mode"].is_null() {
                            params["mode"] = json!("form");
                        }
                        let consent = browser.call("lumen_connector_consent", &params);
                        tokio::pin!(consent);
                        let answer = loop {
                            tokio::select! {biased;
                                next=self.output.next()=>{
                                    let next=next?;
                                    if (next["method"]=="notifications/cancelled" && next["params"]["requestId"]==*request_id) || (next.get("method").is_none() && next["id"]==id) {
                                        return Err("Shared MCP service withdrew the pending consent; no automatic retry".into());
                                    }
                                    if let Some(nested_id)=next.get("id") {
                                        write_frame(&mut self.input,&json!({"jsonrpc":"2.0","id":nested_id,"error":{"code":-32000,"message":"Waiting for connector consent"}})).await.map_err(|_|"Shared MCP reply failed")?;
                                    }
                                },
                                answer=&mut consent=>break answer,
                            }
                        };
                        json!({"jsonrpc":"2.0","id":request_id,"result":answer})
                    } else if message["method"] == "ping" {
                        json!({"jsonrpc":"2.0","id":request_id,"result":{}})
                    } else {
                        json!({"jsonrpc":"2.0","id":request_id,"error":{"code":-32601,"message":"Unsupported client capability; no model was invoked"}})
                    };
                    write_frame(&mut self.input, &response)
                        .await
                        .map_err(|_| "Shared MCP reply failed")?;
                }
            }
        };
        tokio::time::timeout(
            if method == "tools/call" {
                Duration::from_secs(600)
            } else {
                Duration::from_secs(45)
            },
            operation,
        )
        .await
        .map_err(|_| "Shared MCP request timed out; no automatic retry")?
    }
    async fn initialize(&mut self, browser: &Registration) -> Result<Vec<Value>, String> {
        let initialized=self.rpc("initialize",json!({"protocolVersion":"2025-06-18","capabilities":{"elicitation":{"form":{},"url":{}}},"clientInfo":{"name":"lumen-local-tools","version":env!("CARGO_PKG_VERSION")}}),browser).await?;
        if !matches!(
            initialized["protocolVersion"].as_str(),
            Some("2024-11-05" | "2025-03-26" | "2025-06-18" | "2025-11-25")
        ) {
            return Err("Unsupported shared MCP protocol version".into());
        }
        write_frame(
            &mut self.input,
            &json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
        )
        .await
        .map_err(|_| "Shared MCP initialization failed")?;
        if initialized["capabilities"].get("tools").is_none() {
            return Ok(vec![]);
        }
        let mut tools = vec![];
        let mut cursor = None;
        for _ in 0..20 {
            let page = self
                .rpc(
                    "tools/list",
                    cursor.map(|c| json!({"cursor":c})).unwrap_or(json!({})),
                    browser,
                )
                .await?;
            let rows = page["tools"]
                .as_array()
                .ok_or("Invalid shared MCP tool catalog")?;
            if tools.len() + rows.len() > 2000 {
                return Err("Shared MCP tool catalog exceeds limit".into());
            }
            tools.extend(rows.iter().cloned());
            cursor = page["nextCursor"].as_str().map(str::to_owned);
            if cursor.is_none() {
                return Ok(tools);
            }
        }
        Err("Shared MCP catalog exceeds pagination limit".into())
    }
}
fn alias(server: &str, tool: &str) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in server.bytes().chain([0]).chain(tool.bytes()) {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("shared_{hash:016x}")
}
fn error(reason: &str) -> Value {
    json!({"isError":true,"content":[{"type":"text","text":reason}]})
}
#[derive(Default)]
pub struct Hub {
    sessions: BTreeMap<String, Session>,
    routes: BTreeMap<String, (String, String)>,
    pub tools: Vec<Value>,
    skills: BTreeMap<PathBuf, String>,
}
impl Hub {
    pub async fn close(&mut self) -> Result<(), String> {
        let mut failure = None;
        for (_, session) in std::mem::take(&mut self.sessions) {
            if session.close().await.is_err() {
                failure = Some("Shared MCP process cleanup failed".to_owned());
            }
        }
        failure.map_or(Ok(()), Err)
    }
    pub async fn connect(
        &mut self,
        input: &Invocation<'_>,
        browser: &Registration,
        emit: &mut impl FnMut(Activity),
    ) -> Result<(), String> {
        let data = input
            .config
            .parent()
            .and_then(Path::parent)
            .ok_or("Invalid account directory")?;
        let catalog = crate::capability_catalog::discover(data, Some(input.workspace));
        let gateways =
            crate::capability_catalog::shared_servers(data, Some(input.workspace), input.access);
        // Execute portable stdio from its original declaration in this process.
        // Avoid an extra relay/group between the local client and its owned child.
        let mut servers = catalog
            .servers()
            .into_iter()
            .map(|(name, config)| {
                let config = if config["lumenNativeProfile"].is_string() {
                    gateways[&name].clone()
                } else {
                    config
                };
                (name, config)
            })
            .collect::<serde_json::Map<_, _>>();
        servers.extend(crate::integrations::enabled(data)?);
        if servers.len() > 64 {
            return Err("Too many shared MCP services (maximum 64)".into());
        }
        let hub = self;
        for (name, config) in servers {
            emit(Activity {
                kind: "connector-status".into(),
                text: format!("Connecting shared integration {name}"),
            });
            let result = async {
                let session = Session::start(&config, &name, input.workspace, browser).await?;
                hub.sessions.insert(name.clone(), session);
                let tools = hub
                    .sessions
                    .get_mut(&name)
                    .unwrap()
                    .initialize(browser)
                    .await?;
                for mut tool in tools {
                    if matches!(input.access, AccessMode::ReadOnly | AccessMode::FileEdits)
                        && tool["annotations"]["readOnlyHint"] != true
                    {
                        continue;
                    }
                    let Some(original) = tool["name"].as_str().map(str::to_owned) else {
                        continue;
                    };
                    if !tool["inputSchema"].is_object() {
                        continue;
                    }
                    let exposed = alias(&name, &original);
                    tool["name"] = json!(exposed);
                    tool["description"] = json!(format!(
                        "Shared integration {name}/{original}: {}",
                        tool["description"].as_str().unwrap_or("")
                    ));
                    hub.routes.insert(exposed, (name.clone(), original));
                    hub.tools.push(tool);
                }
                Ok::<_, String>(())
            }
            .await;
            if let Err(reason) = result {
                if let Some(session) = hub.sessions.remove(&name) {
                    let _ = session.close().await;
                }
                emit(Activity {
                    kind: "connector-status".into(),
                    text: format!("Shared integration {name} unavailable: {reason}"),
                });
            } else {
                emit(Activity {
                    kind: "connector-status".into(),
                    text: format!("Shared integration {name} ready"),
                });
            }
        }
        for entry in catalog
            .entries
            .into_iter()
            .filter(|e| e.kind == "skill" && e.status == "available")
        {
            if let Ok(file) = dunce::canonicalize(entry.path.join("SKILL.md")) {
                hub.skills
                    .insert(file, format!("{} ({})", entry.name, entry.source));
            }
        }
        for folder in crate::integrations::enabled_skills(data) {
            if let Ok(file) = dunce::canonicalize(folder.join("SKILL.md")) {
                hub.skills.insert(file, "Lumen shared skill".into());
            }
        }
        if !hub.skills.is_empty() {
            hub.tools.extend([
                json!({"name":"lumen_list_skills","description":"List enabled skills from all connected provider profiles and this project. Read the skill before using it; it does not grant permissions.","inputSchema":{"type":"object","properties":{}}}),
                json!({"name":"lumen_read_skill","description":"Read a listed SKILL.md or a text resource relative to its folder. Paths outside the listed skill are denied; maximum 1 MB.","inputSchema":{"type":"object","properties":{"path":{"type":"string"},"resource":{"type":"string"}},"required":["path"]}}),
            ]);
        }
        Ok(())
    }
    pub async fn call(
        &mut self,
        name: &str,
        arguments: &Value,
        access: AccessMode,
        browser: &Registration,
    ) -> Value {
        if name == "lumen_list_skills" && !self.skills.is_empty() {
            return json!({"content":[{"type":"text","text":json!(self.skills.iter().map(|(path,source)|json!({"path":path,"source":source})).collect::<Vec<_>>()).to_string()}]});
        }
        if name == "lumen_read_skill" && !self.skills.is_empty() {
            return self.read_skill(arguments);
        }
        let Some((server, tool)) = self.routes.get(name).cloned() else {
            return error("Tool is not in this turn's shared catalog or permission profile");
        };
        if !arguments.is_object() {
            return error("Tool arguments must be an object");
        }
        if access == AccessMode::Ask {
            let consent=browser.call("lumen_connector_consent",&json!({"serverName":server,"mode":"form","message":format!("Allow one call to {tool}? Review the task and integration before accepting."),"requestedSchema":{"type":"object","properties":{"allow":{"type":"boolean","title":"Allow this tool call once"}},"required":["allow"]}})).await;
            if consent["action"] != "accept" || consent["content"]["allow"] != true {
                return error("Shared tool call denied or cancelled");
            }
        }
        let Some(session) = self.sessions.get_mut(&server) else {
            return error("Integration disconnected; no automatic retry");
        };
        match session
            .rpc(
                "tools/call",
                json!({"name":tool,"arguments":arguments}),
                browser,
            )
            .await
        {
            Ok(result) => result,
            Err(reason) => {
                if let Some(session) = self.sessions.remove(&server) {
                    let _ = session.close().await;
                }
                error(&reason)
            }
        }
    }
    fn read_skill(&self, arguments: &Value) -> Value {
        let content = arguments["path"]
            .as_str()
            .and_then(|p| dunce::canonicalize(p).ok())
            .filter(|p| self.skills.contains_key(p))
            .and_then(|file| {
                let folder = file.parent()?;
                let target = arguments["resource"]
                    .as_str()
                    .map(|p| dunce::canonicalize(folder.join(p)).ok())
                    .unwrap_or(Some(file.clone()))?;
                if !target.starts_with(folder) || std::fs::metadata(&target).ok()?.len() > 1_000_000
                {
                    return None;
                }
                std::fs::read_to_string(target).ok()
            });
        content
            .map(|text| json!({"content":[{"type":"text","text":text}]}))
            .unwrap_or_else(|| {
                error("List the skill first; choose a text file inside its folder (maximum 1 MB)")
            })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::test_support::Fixture;
    use std::collections::HashMap;
    pub(crate) fn server(marker: &str) -> Value {
        json!({"command":which::which("node").unwrap(),"args":[Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/local-tools.mjs")],"env":{"OWNER_MARKER":marker}})
    }
    async fn start(
        f: &Fixture,
        access: AccessMode,
        browser: &Registration,
        events: &mut Vec<Activity>,
    ) -> Hub {
        let account = crate::models::Account {
            id: "local".into(),
            name: "Local fixture".into(),
            provider: crate::models::Provider::Ollama,
        };
        let mut hub = Hub::default();
        hub.connect(
            &Invocation {
                account: &account,
                config: &f.directory("accounts/local"),
                workspace: &f.directory("project"),
                prompt: "fixture",
                model: Some("fixture"),
                effort: "",
                access,
                approvals: None,
                resume: None,
                browser_scope: None,
                browser_artifacts: None,
                browser: Some(browser),
            },
            browser,
            &mut |event| events.push(event),
        )
        .await
        .unwrap();
        hub
    }
    #[tokio::test]
    async fn scoped_catalog_pagination_collisions_permissions_and_skill_resources() {
        let f = Fixture::new();
        crate::integrations::save_mcp(&f.root,&json!({"mcpServers":{"a":server("owner-a"),"b":server("owner-b"),"off":{"command":"never-launch","disabled":true},"http":{"url":"https://example.test/mcp"}}}).to_string()).unwrap();
        let skill = f.directory("project/.agents/skills/review");
        std::fs::write(skill.join("SKILL.md"), "Review original resources").unwrap();
        std::fs::write(skill.join("data.txt"), "Original skill resource").unwrap();
        std::fs::write(
            skill.parent().unwrap().join("outside.txt"),
            "private-outside",
        )
        .unwrap();
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let browser =
            crate::browser::tests::fixture_registration(&f, AccessMode::ReadOnly, None, tx).await;
        let mut events = vec![];
        let mut hub = start(&f, AccessMode::ReadOnly, &browser, &mut events).await;
        assert_eq!(hub.routes.len(), 2);
        assert_eq!(hub.tools.len(), 4);
        assert!(events
            .iter()
            .any(|e| e.text.contains("http") && e.text.contains("adapter")));
        for (owner, marker) in [("a", "owner-a"), ("b", "owner-b")] {
            let result = hub
                .call(
                    &alias(owner, "echo"),
                    &json!({"count":3,"enabled":false}),
                    AccessMode::ReadOnly,
                    &browser,
                )
                .await;
            assert_eq!(result["structuredContent"]["preserved"], true);
            assert_eq!(
                serde_json::from_str::<Value>(result["content"][0]["text"].as_str().unwrap())
                    .unwrap(),
                json!({"owner":marker,"arguments":{"count":3,"enabled":false}})
            );
        }
        assert_eq!(
            hub.call(
                &alias("a", "write"),
                &json!({}),
                AccessMode::ReadOnly,
                &browser
            )
            .await["isError"],
            true
        );
        let path = skill.join("SKILL.md");
        assert_eq!(
            hub.call(
                "lumen_read_skill",
                &json!({"path":path,"resource":"data.txt"}),
                AccessMode::ReadOnly,
                &browser
            )
            .await["content"][0]["text"],
            "Original skill resource"
        );
        assert_eq!(
            hub.call(
                "lumen_read_skill",
                &json!({"path":path,"resource":"../outside.txt"}),
                AccessMode::ReadOnly,
                &browser
            )
            .await["isError"],
            true
        );
        assert_eq!(
            hub.call(
                "lumen_read_skill",
                &json!({"path":skill.parent().unwrap().join("outside.txt")}),
                AccessMode::ReadOnly,
                &browser
            )
            .await["isError"],
            true
        );
        hub.close().await.unwrap();
    }
    async fn answer(
        events: &mut tokio::sync::mpsc::UnboundedReceiver<Activity>,
        action: &str,
        field: Option<&str>,
    ) {
        loop {
            let event = events.recv().await.unwrap();
            if event.kind != "interaction" {
                continue;
            }
            let value: Value = serde_json::from_str(&event.text).unwrap();
            let mut answers: HashMap<String, Vec<String>> = value["questions"]
                .as_array()
                .unwrap()
                .iter()
                .map(|question| (question["id"].as_str().unwrap().into(), vec![]))
                .collect();
            answers.insert("__lumen_connector_action__".into(), vec![action.into()]);
            if let Some(field) = field {
                answers.insert(field.into(), vec!["true".into()]);
            }
            crate::interactions::answer_question(value["id"].as_str().unwrap().into(), answers)
                .unwrap();
            return;
        }
    }
    #[tokio::test]
    async fn ask_requires_one_explicit_decision_and_native_elicitation_remains_private() {
        let f = Fixture::new();
        let log = f.root.join("calls.txt");
        let mut config = server("owner");
        config["env"]["CALL_FILE"] = json!(log);
        crate::integrations::save_mcp(&f.root, &json!({"mcpServers":{"a":config}}).to_string())
            .unwrap();
        let (tx, mut events) = tokio::sync::mpsc::unbounded_channel();
        let browser =
            crate::browser::tests::fixture_registration(&f, AccessMode::Ask, None, tx).await;
        let mut hub = start(&f, AccessMode::Ask, &browser, &mut vec![]).await;
        assert_eq!(
            hub.routes.len(),
            2,
            "Second catalog page includes the write tool"
        );
        let call = async {
            hub.call(&alias("a", "write"), &json!({}), AccessMode::Ask, &browser)
                .await
        };
        let (result, ()) = tokio::join!(call, answer(&mut events, "Decline", None));
        assert_eq!(result["isError"], true);
        assert!(!log.exists());
        let (result, ()) = tokio::join!(
            async {
                hub.call(&alias("a", "write"), &json!({}), AccessMode::Ask, &browser)
                    .await
            },
            answer(&mut events, "Accept", Some("allow"))
        );
        assert!(result.get("isError").is_none());
        assert_eq!(std::fs::read_to_string(&log).unwrap(), "write\n");
        let (result, ()) = tokio::join!(
            async {
                hub.call(
                    &alias("a", "echo"),
                    &json!({"elicit":true}),
                    AccessMode::Autopilot,
                    &browser,
                )
                .await
            },
            answer(&mut events, "Accept", Some("approved"))
        );
        assert_eq!(result["content"][0]["text"], "Consent accept");
        let recorded: Vec<_> = std::iter::from_fn(|| events.try_recv().ok()).collect();
        assert!(crate::interactions::answer_context(&recorded).is_empty());
        assert!(recorded
            .iter()
            .filter(|e| e.kind == "interaction-result")
            .all(|e| !e.text.contains("approved")
                && serde_json::from_str::<Value>(&e.text).unwrap()["answers"] == json!({})));
        let result = hub
            .call(
                &alias("a", "echo"),
                &json!({"sample":true}),
                AccessMode::Autopilot,
                &browser,
            )
            .await;
        assert_eq!(
            result["content"][0]["text"],
            "Sampling unsupported; no inference"
        );
        hub.close().await.unwrap();
    }
    #[tokio::test]
    async fn cancelling_a_pending_call_retires_consent_and_terminates_its_process() {
        let f = Fixture::new();
        let pid_file = f.root.join("mcp.pid");
        let mut config = server("owner");
        config["env"]["PID_FILE"] = json!(pid_file);
        crate::integrations::save_mcp(&f.root, &json!({"mcpServers":{"a":config}}).to_string())
            .unwrap();
        let (tx, mut events) = tokio::sync::mpsc::unbounded_channel();
        let browser =
            crate::browser::tests::fixture_registration(&f, AccessMode::Autopilot, None, tx).await;
        let mut hub = start(&f, AccessMode::Autopilot, &browser, &mut vec![]).await;
        let pid: u32 = std::fs::read_to_string(pid_file).unwrap().parse().unwrap();
        let id = {
            let call = async {
                hub.call(
                    &alias("a", "echo"),
                    &json!({"elicit":true}),
                    AccessMode::Autopilot,
                    &browser,
                )
                .await
            };
            tokio::pin!(call);
            let event = tokio::select! {result=&mut call=>panic!("Unexpected completed call: {result}"),event=events.recv()=>event.unwrap()};
            assert_eq!(event.kind, "interaction");
            serde_json::from_str::<Value>(&event.text).unwrap()["id"]
                .as_str()
                .unwrap()
                .to_owned()
        };
        drop(hub);
        assert!(crate::interactions::answer_question(id, HashMap::new()).is_err());
        let status = crate::proc::command(which::which("node").unwrap())
            .args([
                "-e",
                &format!("try{{process.kill({pid},0);process.exit(1)}}catch{{process.exit(0)}}"),
            ])
            .status()
            .await
            .unwrap();
        assert!(status.success(), "Cancelled MCP process is still alive");
    }
    #[tokio::test]
    async fn withdrawn_service_consent_closes_the_form_without_a_model_or_retry() {
        let f = Fixture::new();
        crate::integrations::save_mcp(
            &f.root,
            &json!({"mcpServers":{"a":server("owner")}}).to_string(),
        )
        .unwrap();
        let (tx, mut events) = tokio::sync::mpsc::unbounded_channel();
        let browser =
            crate::browser::tests::fixture_registration(&f, AccessMode::Autopilot, None, tx).await;
        let mut hub = start(&f, AccessMode::Autopilot, &browser, &mut vec![]).await;
        let result = tokio::time::timeout(
            Duration::from_secs(3),
            hub.call(
                &alias("a", "echo"),
                &json!({"withdraw":true}),
                AccessMode::Autopilot,
                &browser,
            ),
        )
        .await
        .unwrap();
        assert_eq!(result["isError"], true);
        assert!(result.to_string().contains("withdrew"));
        let event = events.recv().await.unwrap();
        assert_eq!(event.kind, "interaction");
        let id = serde_json::from_str::<Value>(&event.text).unwrap()["id"]
            .as_str()
            .unwrap()
            .to_owned();
        assert!(crate::interactions::answer_question(id, HashMap::new()).is_err());
        assert!(hub.sessions.is_empty());
    }
    #[tokio::test]
    async fn malformed_service_is_retired_without_retry_or_private_error_leak() {
        let f = Fixture::new();
        crate::integrations::save_mcp(
            &f.root,
            &json!({"mcpServers":{"a":server("owner")}}).to_string(),
        )
        .unwrap();
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let browser =
            crate::browser::tests::fixture_registration(&f, AccessMode::Autopilot, None, tx).await;
        let mut hub = start(&f, AccessMode::Autopilot, &browser, &mut vec![]).await;
        let result = hub
            .call(
                &alias("a", "echo"),
                &json!({"malformed":true}),
                AccessMode::Autopilot,
                &browser,
            )
            .await;
        assert_eq!(result["isError"], true);
        assert!(!result.to_string().contains("private-invalid"));
        assert!(hub.sessions.is_empty());
        assert_eq!(
            hub.call(
                &alias("a", "echo"),
                &json!({}),
                AccessMode::Autopilot,
                &browser
            )
            .await["isError"],
            true
        );
    }
}
