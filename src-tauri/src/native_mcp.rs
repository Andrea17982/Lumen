//! Native-account MCP gateway. Direct app-server RPC only: never sends turn/start or model input.
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::PathBuf, time::Duration};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
type Client = tokio::io::Lines<BufReader<tokio::io::Stdin>>;
type Output = tokio::io::Stdout;
struct Wire {
    child: crate::proc::ServiceChild,
    input: tokio::process::ChildStdin,
    output: tokio::io::Lines<BufReader<tokio::process::ChildStdout>>,
    next: u64,
    client_id: Value,
    pending_consent: Option<HostConsent>,
    queued: std::collections::VecDeque<Value>,
}
struct HostConsent {
    http: reqwest::Client,
    url: reqwest::Url,
    token: String,
    key: String,
}
impl HostConsent {
    fn from_env() -> Result<Option<Self>, String> {
        let (Ok(address), Ok(token)) = (
            std::env::var("LUMEN_BROWSER_URL"),
            std::env::var("LUMEN_BROWSER_CAP"),
        ) else {
            return Ok(None);
        };
        let url = reqwest::Url::parse(&address).map_err(|_| "Invalid Lumen consent endpoint")?;
        if url.scheme() != "http"
            || !matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"))
            || url.path() != "/call"
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err("Invalid Lumen consent endpoint".into());
        }
        let http = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(3))
            .timeout(Duration::from_secs(600))
            .build()
            .map_err(|_| "Consent transport unavailable")?;
        Ok(Some(Self {
            http,
            url,
            token,
            key: uuid::Uuid::new_v4().to_string(),
        }))
    }
    async fn cancel(&self) {
        let mut url = self.url.clone();
        url.set_path("/cancel");
        let _ = tokio::time::timeout(
            Duration::from_secs(3),
            self.http
                .post(url)
                .bearer_auth(&self.token)
                .json(&json!({"requestId":self.key}))
                .send(),
        )
        .await;
    }
    #[allow(clippy::too_many_arguments)]
    async fn ask(
        &self,
        params: Value,
        client: &mut Client,
        output: &mut Output,
        active: &Value,
        owner: &mut tokio::io::Lines<BufReader<tokio::process::ChildStdout>>,
        queued: &mut std::collections::VecDeque<Value>,
        native_id: &Value,
        rpc_id: u64,
    ) -> Result<Value, String> {
        let request = async {
            let response=self.http.post(self.url.clone()).bearer_auth(&self.token).json(&json!({"requestId":self.key,"name":"lumen_connector_consent","arguments":params})).send().await.map_err(|_|"Lumen consent connection closed")?;
            if !response.status().is_success() {
                return Err("Lumen consent scope is unavailable".to_string());
            }
            response
                .json::<Value>()
                .await
                .map_err(|_| "Invalid Lumen consent response".into())
        };
        tokio::pin!(request);
        loop {
            tokio::select! {biased;
                message=owner.next_line()=>{
                    let line=message.map_err(|_|"Native account disconnected")?.ok_or("Native account disconnected")?;
                    if line.len()>8_000_000{return Err("Native account frame exceeds limit".into());}
                    if let Ok(message)=serde_json::from_str::<Value>(&line) {
                    if message.get("method").is_none() && message["id"]==rpc_id {return Err("Native account ended the call before consent completed; no automatic retry".into());}
                        if message["method"]=="serverRequest/resolved" && message["params"]["requestId"]==*native_id {return Err("Native account withdrew its consent request".into());}
                        if queued.len()>=128{return Err("Native consent event queue exceeds limit".into());}queued.push_back(message);
                    }
                },
                line=client.next_line()=>{
                    let line=line.map_err(|_|"MCP client disconnected")?.ok_or("MCP client disconnected")?;
                    if line.len()>1_000_000{return Err("MCP message exceeds limit".into());}
                    let message:Value=serde_json::from_str(&line).map_err(|_|"Invalid MCP message")?;
                    if message["method"]=="notifications/cancelled" && message["params"]["requestId"]==*active {return Err("MCP operation cancelled; no automatic retry".into());}
                    if let Some(id)=message.get("id"){send(output,json!({"jsonrpc":"2.0","id":id,"error":{"code":-32000,"message":"Lumen is waiting for connector consent"}})).await?;}
                },
                result=&mut request=>return result,
            }
        }
    }
}
async fn send(output: &mut Output, value: Value) -> Result<(), String> {
    output
        .write_all(format!("{value}\n").as_bytes())
        .await
        .map_err(|_| "MCP client closed")?;
    output.flush().await.map_err(|_| "MCP client closed".into())
}
impl Wire {
    async fn write(&mut self, value: Value) -> Result<(), String> {
        self.input
            .write_all(format!("{value}\n").as_bytes())
            .await
            .map_err(|_| "Native account connection closed")?;
        self.input
            .flush()
            .await
            .map_err(|_| "Native account connection closed".into())
    }
    async fn rpc(
        &mut self,
        method: &str,
        params: Value,
        client: &mut Client,
        output: &mut Output,
    ) -> Result<Value, String> {
        self.next += 1;
        let id = self.next;
        let expected_thread = params["threadId"].as_str().map(str::to_owned);
        self.write(json!({"id":id,"method":method,"params":params}))
            .await?;
        let operation = async {
            let mut consent_sequence = 0u64;
            loop {
                let line = if let Some(message) = self.queued.pop_front() {
                    message.to_string()
                } else {
                    tokio::select! {
                        line=self.output.next_line()=>line.map_err(|_|"Native account connection failed")?.ok_or("Native account disconnected")?,
                        incoming=client.next_line()=>{
                            let incoming=incoming.map_err(|_|"MCP client disconnected")?.ok_or("MCP client disconnected")?;
                            let request:Value=serde_json::from_str(&incoming).map_err(|_|"Invalid MCP message")?;
                            if request["method"]=="notifications/cancelled" && request["params"]["requestId"]==self.client_id {return Err("MCP operation cancelled; no automatic retry".into());}
                            if let Some(id)=request.get("id") {send(output,json!({"jsonrpc":"2.0","id":id,"error":{"code":-32000,"message":"Another account tool request is pending"}})).await?;}
                            continue;
                        }
                    }
                };
                if line.len() > 8_000_000 {
                    return Err("Native account frame exceeds limit".into());
                }
                let Ok(message) = serde_json::from_str::<Value>(&line) else {
                    continue;
                };
                if message.get("method").is_none() && message["id"] == id {
                    if message.get("error").is_some() {
                        return Err("Native account RPC unavailable. Check official CLI version, sign-in and connector access. No model turn was sent.".into());
                    }
                    return Ok(message["result"].clone());
                }
                if message.get("id").is_some()
                    && matches!(
                        message["method"].as_str(),
                        Some(
                            "mcpServer/elicitation/request"
                                | "item/tool/requestUserInput"
                                | "tool/requestUserInput"
                        )
                    )
                {
                    if expected_thread.as_deref().is_some_and(|thread| {
                        message["params"]["threadId"].as_str() != Some(thread)
                    }) {
                        self.write(json!({"id":message["id"],"result":if message["method"]=="mcpServer/elicitation/request" {json!({"action":"cancel","content":null})}else{json!({"answers":{}})}}))
                        .await?;
                        continue;
                    }
                    let user_input = message["method"] != "mcpServer/elicitation/request";
                    let mut params = if user_input {
                        input_form(&message["params"])?
                    } else {
                        message["params"].clone()
                    };
                    if let Some(host) = HostConsent::from_env()? {
                        if params["serverName"].is_null() {
                            params["serverName"] = json!("ChatGPT account tool");
                        }
                        self.pending_consent = Some(host);
                        let answer = self
                            .pending_consent
                            .as_ref()
                            .unwrap()
                            .ask(
                                params,
                                client,
                                output,
                                &self.client_id,
                                &mut self.output,
                                &mut self.queued,
                                &message["id"],
                                id,
                            )
                            .await?;
                        self.pending_consent.take();
                        let answer = if answer["action"].is_string() {
                            answer
                        } else {
                            json!({"action":"cancel","content":null})
                        };
                        let answer = if user_input {
                            input_response(&message["params"], &answer)
                        } else {
                            answer
                        };
                        self.write(json!({"id":message["id"],"result":answer}))
                            .await?;
                        continue;
                    }
                    if let Some(object) = params.as_object_mut() {
                        object.remove("threadId");
                        object.remove("turnId");
                        object.remove("serverName");
                    }
                    consent_sequence += 1;
                    let request_id = format!("owner-consent-{id}-{consent_sequence}");
                    send(output,json!({"jsonrpc":"2.0","id":request_id,"method":"elicitation/create","params":params})).await?;
                    loop {
                        let answer = client
                            .next_line()
                            .await
                            .map_err(|_| "MCP client disconnected")?
                            .ok_or("MCP client disconnected")?;
                        if answer.len() > 1_000_000 {
                            return Err("MCP answer exceeds limit".into());
                        }
                        let answer: Value = serde_json::from_str(&answer)
                            .map_err(|_| "Invalid MCP consent response")?;
                        if answer["id"] == request_id && answer.get("method").is_none() {
                            let result =
                                if answer["result"]["action"].as_str().is_some_and(|action| {
                                    matches!(action, "accept" | "decline" | "cancel")
                                }) {
                                    answer["result"].clone()
                                } else {
                                    json!({"action":"cancel","content":null})
                                };
                            let result = if user_input {
                                input_response(&message["params"], &result)
                            } else {
                                result
                            };
                            self.write(json!({"id":message["id"],"result":result}))
                                .await?;
                            break;
                        }
                        if answer["method"] == "notifications/cancelled"
                            && (answer["params"]["requestId"] == self.client_id
                                || answer["params"]["requestId"] == request_id)
                        {
                            return Err("MCP operation cancelled; no automatic retry".into());
                        }
                        if let Some(id) = answer.get("id") {
                            send(output,json!({"jsonrpc":"2.0","id":id,"error":{"code":-32000,"message":"Account consent is pending"}})).await?;
                        }
                    }
                } else if message.get("id").is_some() && message.get("method").is_some() {
                    self.write(json!({"id":message["id"],"error":{"code":-32601,"message":"Unsupported native account request; no permission granted"}})).await?;
                }
            }
        };
        tokio::time::timeout(
            if method == "mcpServer/tool/call" {
                Duration::from_secs(600)
            } else {
                Duration::from_secs(90)
            },
            operation,
        )
        .await
        .map_err(|_| "Native account request timed out; no automatic retry".to_string())?
    }
}
fn input_form(params: &Value) -> Result<Value, String> {
    let questions = params["questions"]
        .as_array()
        .filter(|q| !q.is_empty() && q.len() <= 6)
        .ok_or("Unsupported native account question")?;
    let mut properties = serde_json::Map::new();
    let mut required = vec![];
    for question in questions {
        let id = question["id"]
            .as_str()
            .ok_or("Missing native account question id")?;
        if question["multiSelect"] == true {
            return Err("Multiple-choice account question needs another adapter".into());
        }
        properties.insert(id.into(),json!({"type":"string","title":question["question"].as_str().unwrap_or("Choose an account tool response")}));
        required.push(id);
        // Choices remain visible in the field explanation; allow the native free-text path.
        if let Some(options) = question["options"].as_array() {
            properties[id]["description"] = json!(options
                .iter()
                .filter_map(|option| option["label"].as_str())
                .collect::<Vec<_>>()
                .join(" / "));
        }
    }
    Ok(
        json!({"mode":"form","message":"Respond to the owning account tool request. Agent permissions do not replace connector consent.","requestedSchema":{"type":"object","properties":properties,"required":required}}),
    )
}
fn input_response(params: &Value, response: &Value) -> Value {
    let mut answers = serde_json::Map::new();
    for question in params["questions"].as_array().into_iter().flatten() {
        let Some(id) = question["id"].as_str() else {
            continue;
        };
        let value = if response["action"] == "accept" {
            response["content"][id].as_str()
        } else {
            let label = if response["action"] == "decline" {
                "Decline"
            } else {
                "Cancel"
            };
            question["options"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|option| option["label"].as_str())
                .find(|option| option.eq_ignore_ascii_case(label))
        };
        answers.insert(
            id.into(),
            json!({"answers":value.into_iter().collect::<Vec<_>>()}),
        );
    }
    json!({"answers":answers})
}
fn alias(server: &str, tool: &str) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in server.bytes().chain([0]).chain(tool.bytes()) {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("account_tool_{hash:016x}")
}
pub async fn serve(profile: PathBuf, workspace: PathBuf, read_only: bool) -> Result<(), String> {
    let mut command = crate::providers::command(&crate::models::Provider::Gpt)?;
    crate::providers::isolate(&mut command, &crate::models::Provider::Gpt, &profile);
    command
        .env_remove("LUMEN_BROWSER_URL")
        .env_remove("LUMEN_BROWSER_CAP");
    command
        .args(["app-server"])
        .current_dir(&workspace)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);
    let mut child = crate::proc::ServiceChild::spawn(command)
        .map_err(|_| "Could not start native account gateway")?;
    let input = child
        .stdin
        .take()
        .ok_or("Native account input unavailable")?;
    let output = child
        .stdout
        .take()
        .ok_or("Native account output unavailable")?;
    let mut wire = Wire {
        child,
        input,
        output: BufReader::new(output).lines(),
        next: 0,
        client_id: Value::Null,
        pending_consent: None,
        queued: Default::default(),
    };
    let mut client = BufReader::new(tokio::io::stdin()).lines();
    let mut output = tokio::io::stdout();
    let mut active = None;
    let result = run(
        &mut wire,
        &mut client,
        &mut output,
        &workspace,
        read_only,
        &mut active,
    )
    .await;
    if let (Err(reason), Some(id)) = (&result, active) {
        let _ = send(
            &mut output,
            json!({"jsonrpc":"2.0","id":id,"error":{"code":-32000,"message":reason}}),
        )
        .await;
    }
    if let Some(consent) = wire.pending_consent.take() {
        consent.cancel().await;
    }
    drop(wire.input);
    wire.child.finish(result.is_ok()).await?;
    result
}
async fn run(
    wire: &mut Wire,
    client: &mut Client,
    output: &mut Output,
    workspace: &std::path::Path,
    read_only: bool,
    active: &mut Option<Value>,
) -> Result<(), String> {
    let mut thread = None::<String>;
    let mut routes = BTreeMap::<String, (String, String, Value)>::new();
    let mut skill_files = std::collections::BTreeSet::<PathBuf>::new();
    while let Some(line) = client
        .next_line()
        .await
        .map_err(|_| "MCP client disconnected")?
    {
        if line.len() > 8_000_000 {
            return Err("MCP message exceeds limit".into());
        }
        let request: Value = serde_json::from_str(&line).map_err(|_| "Invalid MCP message")?;
        let Some(id) = request.get("id") else {
            continue;
        };
        *active = Some(id.clone());
        wire.client_id = id.clone();
        let method = request["method"].as_str().unwrap_or("");
        if method == "initialize" {
            send(output,json!({"jsonrpc":"2.0","id":id,"result":{"protocolVersion":request["params"]["protocolVersion"],"serverInfo":{"name":"lumen-native-account","version":env!("CARGO_PKG_VERSION")},"capabilities":{"tools":{}},"instructions":"Tools execute through their owning ChatGPT account. This gateway never runs a model turn. Native connector consent is forwarded to your client."}})).await?;
            continue;
        }
        if !matches!(method, "tools/list" | "tools/call" | "ping") {
            send(output,json!({"jsonrpc":"2.0","id":id,"error":{"code":-32601,"message":"Unsupported account gateway method"}})).await?;
            continue;
        }
        if method == "ping" {
            send(output, json!({"jsonrpc":"2.0","id":id,"result":{}})).await?;
            continue;
        }
        if thread.is_none() {
            wire.rpc("initialize",json!({"clientInfo":{"name":"lumen-account-tools","version":env!("CARGO_PKG_VERSION")},"capabilities":{"experimentalApi":true}}),client,output).await?;
            wire.write(json!({"method":"initialized"})).await?;
            let started=wire.rpc("thread/start",json!({"cwd":workspace,"ephemeral":true,"approvalPolicy":"on-request","sandbox":"read-only"}),client,output).await?;
            thread = Some(
                started["thread"]["id"]
                    .as_str()
                    .ok_or("Native account returned no tool thread")?
                    .into(),
            );
        }
        if method == "tools/list" {
            routes.clear();
            let mut cursor = Value::Null;
            let mut tools = vec![
                json!({"name":"list_account_skills","description":"Discover skills installed in the owning ChatGPT/Codex account, including plugin skills, without running its model.","inputSchema":{"type":"object","properties":{}},"annotations":{"readOnlyHint":true,"idempotentHint":true}}),
                json!({"name":"read_account_skill","description":"Read a SKILL.md returned by list_account_skills, or a text resource relative to that skill. Only listed skills and files inside their folder can be read.","inputSchema":{"type":"object","properties":{"path":{"type":"string"},"resource":{"type":"string"}},"required":["path"]},"annotations":{"readOnlyHint":true,"idempotentHint":true}}),
            ];
            for page in 0..20 {
                let list=wire.rpc("mcpServerStatus/list",json!({"threadId":thread,"detail":"toolsAndAuthOnly","cursor":cursor,"limit":100}),client,output).await?;
                for server in list["data"].as_array().into_iter().flatten() {
                    if server["authStatus"] == "notLoggedIn"
                        || server["runtimeStatus"]
                            .as_str()
                            .is_some_and(|status| status != "connected")
                    {
                        continue;
                    }
                    let Some(server_name) = server["name"].as_str() else {
                        continue;
                    };
                    for (key, tool) in server["tools"].as_object().into_iter().flatten() {
                        if read_only && tool["annotations"]["readOnlyHint"] != true {
                            continue;
                        }
                        let name = tool["name"].as_str().unwrap_or(key);
                        let exposed = alias(server_name, name);
                        let mut definition = tool.clone();
                        definition["name"] = json!(exposed);
                        definition["description"] = json!(format!(
                            "Via owning ChatGPT account, {server_name}/{name}: {}",
                            tool["description"].as_str().unwrap_or("")
                        ));
                        routes.insert(exposed, (server_name.into(), name.into(), tool.clone()));
                        tools.push(definition);
                    }
                }
                cursor = list["nextCursor"].clone();
                if cursor.is_null() {
                    break;
                }
                if page == 19 {
                    return Err("Native account tool catalog exceeds pagination limit".into());
                }
            }
            send(
                output,
                json!({"jsonrpc":"2.0","id":id,"result":{"tools":tools}}),
            )
            .await?;
        } else {
            let name = request["params"]["name"].as_str().unwrap_or("");
            if name == "list_account_skills" {
                let result = wire
                    .rpc(
                        "skills/list",
                        json!({"cwds":[workspace],"forceReload":true}),
                        client,
                        output,
                    )
                    .await?;
                skill_files.clear();
                for skill in result["data"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .flat_map(|row| row["skills"].as_array().into_iter().flatten())
                {
                    if skill["enabled"] == false {
                        continue;
                    }
                    if let Some(path) = skill["path"].as_str() {
                        if let Ok(path) = dunce::canonicalize(path) {
                            if path.file_name().is_some_and(|name| name == "SKILL.md") {
                                skill_files.insert(path);
                            }
                        }
                    }
                }
                send(output,json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":result.to_string()}]}})).await?;
                continue;
            }
            if name == "read_account_skill" {
                let arguments = &request["params"]["arguments"];
                let path = arguments["path"]
                    .as_str()
                    .and_then(|path| dunce::canonicalize(path).ok());
                let content = path
                    .filter(|path| skill_files.contains(path))
                    .and_then(|path| {
                        let root = path.parent()?;
                        let target = if let Some(resource) = arguments["resource"].as_str() {
                            dunce::canonicalize(root.join(resource)).ok()?
                        } else {
                            path.clone()
                        };
                        if !target.starts_with(root)
                            || std::fs::metadata(&target).ok()?.len() > 1_000_000
                        {
                            return None;
                        }
                        std::fs::read_to_string(target).ok()
                    });
                let result = if let Some(content) = content {
                    json!({"content":[{"type":"text","text":content}]})
                } else {
                    json!({"isError":true,"content":[{"type":"text","text":"List the skill first, then choose SKILL.md or a text resource inside its folder (maximum 1 MB)."}]})
                };
                send(output, json!({"jsonrpc":"2.0","id":id,"result":result})).await?;
                continue;
            }
            if let Some((server, tool, _)) = routes.get(name) {
                let result=wire.rpc("mcpServer/tool/call",json!({"threadId":thread,"server":server,"tool":tool,"arguments":request["params"]["arguments"]}),client,output).await?;
                send(output, json!({"jsonrpc":"2.0","id":id,"result":result})).await?;
            } else {
                send(output,json!({"jsonrpc":"2.0","id":id,"error":{"code":-32602,"message":"Tool not in the current account catalog or permission profile"}})).await?;
            }
        }
    }
    Ok(())
}
