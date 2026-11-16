//! Official MCP HTTP client, with turn-owned consent and no model sampling/replay.
use rmcp::{
    model::{ClientConfig, ClientRequest, ElicitRequestParams, ElicitResult, ErrorData},
    service::{RequestContext, RunningService},
    transport::{
        streamable_http_client::StreamableHttpClientTransportConfig, StreamableHttpClientTransport,
    },
    ClientHandler, RoleClient, ServiceExt,
};
use serde_json::{json, Value};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::sync::{mpsc, oneshot};

struct Consent {
    params: Value,
    reply: oneshot::Sender<Value>,
}
pub struct Handler {
    inputs: mpsc::Sender<Consent>,
    active: Arc<AtomicBool>,
    pub changed: Arc<AtomicBool>,
}
impl ClientHandler for Handler {
    fn get_info(&self) -> ClientConfig {
        serde_json::from_value(json!({"protocolVersion":"2025-06-18","capabilities":{"elicitation":{"form":{},"url":{}}},"clientInfo":{"name":"lumen-shared-http","version":env!("CARGO_PKG_VERSION")}})).expect("Static MCP client info")
    }
    async fn create_elicitation(
        &self,
        request: ElicitRequestParams,
        context: RequestContext<RoleClient>,
    ) -> Result<ElicitResult, ErrorData> {
        if !self.active.load(Ordering::SeqCst) {
            return serde_json::from_value(json!({"action":"cancel"}))
                .map_err(|_| ErrorData::internal_error("Consent unavailable", None));
        }
        let (reply, receive) = oneshot::channel();
        self.inputs
            .try_send(Consent {
                params: serde_json::to_value(request)
                    .map_err(|_| ErrorData::internal_error("Invalid consent", None))?,
                reply,
            })
            .map_err(|_| ErrorData::internal_error("Turn closed", None))?;
        let answer = tokio::select! {biased;_=context.ct.cancelled()=>json!({"action":"cancel"}),answer=receive=>answer.unwrap_or(json!({"action":"cancel"}))};
        serde_json::from_value(answer)
            .map_err(|_| ErrorData::internal_error("Invalid consent response", None))
    }
    async fn on_tool_list_changed(&self, _: rmcp::service::NotificationContext<RoleClient>) {
        self.changed.store(true, Ordering::SeqCst);
    }
}
struct Active(Arc<AtomicBool>);
impl Drop for Active {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}
pub struct Session {
    service: RunningService<RoleClient, Handler>,
    inputs: mpsc::Receiver<Consent>,
    active: Arc<AtomicBool>,
    pub changed: Arc<AtomicBool>,
    name: String,
}
async fn receive<T>(
    operation: impl std::future::Future<Output = Result<T, String>>,
    inputs: &mut mpsc::Receiver<Consent>,
    browser: &crate::browser::Registration,
    name: &str,
) -> Result<T, String> {
    tokio::pin!(operation);
    loop {
        tokio::select! {biased;
            input=inputs.recv()=>{
                let Some(mut input)=input else{return Err("Shared MCP connection closed".into());};
                input.params["serverName"]=json!(name);
                if input.params["mode"].is_null(){input.params["mode"]=json!("form");}
                let answer=tokio::select! {biased;_=input.reply.closed()=>continue,answer=browser.call("lumen_connector_consent",&input.params)=>answer,result=&mut operation=>return result};
                let _=input.reply.send(answer);
            },
            result=&mut operation=>return result,
        }
    }
}
impl Session {
    pub fn capabilities(&self) -> Value {
        serde_json::to_value(self.service.peer_info()).unwrap_or(Value::Null)["capabilities"]
            .clone()
    }
    pub fn take_changed(&self) -> bool {
        self.changed.swap(false, Ordering::SeqCst)
    }
    pub async fn start(
        config: &Value,
        name: &str,
        browser: &crate::browser::Registration,
    ) -> Result<Self, String> {
        let fields = config.as_object().ok_or("Invalid HTTP MCP settings")?;
        if fields
            .keys()
            .any(|key| !matches!(key.as_str(), "url" | "headers" | "type"))
            || config.get("type").is_some_and(|value| value != "http")
            || config
                .get("headers")
                .is_some_and(|value| !value.is_object())
        {
            return Err("HTTP MCP policy requires an owner adapter; not discarded".into());
        }
        let address = crate::capability_catalog::expand(
            config["url"].as_str().ok_or("Missing HTTP MCP address")?,
        )?;
        let url = reqwest_mcp::Url::parse(&address).map_err(|_| "Invalid HTTP MCP address")?;
        if !matches!(url.scheme(), "http" | "https")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.fragment().is_some()
        {
            return Err("HTTP MCP needs HTTP(S) without embedded credentials".into());
        }
        let http = reqwest_mcp::Client::builder()
            .no_proxy()
            .redirect(reqwest_mcp::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(600))
            .build()
            .map_err(|_| "HTTP MCP client unavailable")?;
        let mut settings = StreamableHttpClientTransportConfig::with_uri(address);
        settings.reinit_on_expired_session = false; // Writes are never replayed after session expiry.
        settings.max_sse_event_size = 2_000_000;
        settings.max_concurrent_requests = 1;
        for (key, value) in config["headers"].as_object().into_iter().flatten() {
            let key = reqwest_mcp::header::HeaderName::from_bytes(key.as_bytes())
                .map_err(|_| "Invalid MCP header name")?;
            if matches!(
                key.as_str(),
                "host"
                    | "accept"
                    | "content-type"
                    | "content-length"
                    | "mcp-session-id"
                    | "mcp-protocol-version"
            ) {
                return Err("MCP transport headers cannot be overridden".into());
            }
            let value =
                crate::capability_catalog::expand(value.as_str().ok_or("Invalid MCP header")?)?;
            settings.custom_headers.insert(
                key,
                reqwest_mcp::header::HeaderValue::from_str(&value)
                    .map_err(|_| "Invalid MCP header value")?,
            );
        }
        let (send, mut inputs) = mpsc::channel(8);
        let active = Arc::new(AtomicBool::new(true));
        let changed = Arc::new(AtomicBool::new(false));
        let handler = Handler {
            inputs: send,
            active: active.clone(),
            changed: changed.clone(),
        };
        let guard = Active(active.clone());
        let operation = async {
            handler
                .serve(StreamableHttpClientTransport::with_client(http, settings))
                .await
                .map_err(|_| {
                    "HTTP MCP initialization failed; verify access in the owning integration"
                        .to_owned()
                })
        };
        let service = tokio::time::timeout(
            Duration::from_secs(45),
            receive(operation, &mut inputs, browser, name),
        )
        .await
        .map_err(|_| "HTTP MCP initialization timed out")??;
        drop(guard);
        Ok(Self {
            service,
            inputs,
            active,
            changed,
            name: name.into(),
        })
    }
    pub async fn rpc(
        &mut self,
        method: &str,
        params: Value,
        browser: &crate::browser::Registration,
    ) -> Result<Value, String> {
        if method == "initialize" {
            return serde_json::to_value(self.service.peer_info())
                .map_err(|_| "Invalid HTTP MCP initialization".into());
        }
        self.active.store(true, Ordering::SeqCst);
        let _guard = Active(self.active.clone());
        let request: ClientRequest =
            serde_json::from_value(json!({"method":method,"params":params}))
                .map_err(|_| "Unsupported HTTP MCP request")?;
        let operation = async {
            let result = self
                .service
                .peer()
                .send_request(request)
                .await
                .map_err(|_| "HTTP MCP call failed; no automatic retry".to_owned())?;
            let value = serde_json::to_value(result).map_err(|_| "Invalid HTTP MCP result")?;
            if value.to_string().len() > 2_000_000 {
                return Err("HTTP MCP result exceeds 2 MB".into());
            }
            Ok(value)
        };
        tokio::time::timeout(
            if method == "tools/call" {
                Duration::from_secs(600)
            } else {
                Duration::from_secs(45)
            },
            receive(operation, &mut self.inputs, browser, &self.name),
        )
        .await
        .map_err(|_| "HTTP MCP call timed out; no automatic retry")?
    }
    pub async fn close(self) -> Result<(), String> {
        tokio::time::timeout(Duration::from_secs(5), self.service.cancel())
            .await
            .map_err(|_| "HTTP MCP shutdown timed out")?
            .map(|_| ())
            .map_err(|_| "HTTP MCP shutdown failed".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        extract::State,
        http::{HeaderMap, StatusCode},
        response::{IntoResponse, Response},
        routing::post,
        Json, Router,
    };
    use std::sync::Mutex;
    #[tokio::test]
    async fn http_sse_uses_host_consent_and_reports_catalog_changes() {
        use axum::response::sse::{Event, Sse};
        use futures_util::StreamExt;
        type Reply = Arc<Mutex<Option<oneshot::Sender<Value>>>>;
        async fn handle(State(reply): State<Reply>, Json(body): Json<Value>) -> Response {
            match body["method"].as_str() {
                Some("initialize")=>([("mcp-session-id","consent-session")],Json(json!({"jsonrpc":"2.0","id":body["id"],"result":{"protocolVersion":body["params"]["protocolVersion"],"capabilities":{"tools":{"listChanged":true}},"serverInfo":{"name":"sse-consent","version":"1"}}}))).into_response(),
                Some("notifications/initialized")=>StatusCode::ACCEPTED.into_response(),
                Some("tools/call")=>{
                    let (send,receive)=oneshot::channel();*reply.lock().unwrap()=Some(send);
                    let ask=json!({"jsonrpc":"2.0","id":"ask","method":"elicitation/create","params":{"message":"Confirm HTTP connector","requestedSchema":{"type":"object","properties":{"approved":{"type":"boolean"}},"required":["approved"]}}});
                    let begin=futures_util::stream::iter(vec![Ok::<_,std::convert::Infallible>(Event::default().data(ask.to_string()))]);
                    let end=futures_util::stream::once(async move{
                        let answer=receive.await.unwrap();assert_eq!(answer,json!({"action":"accept","content":{"approved":true}}));
                        Ok::<_,std::convert::Infallible>(Event::default().data(json!({"jsonrpc":"2.0","id":body["id"],"result":{"content":[{"type":"text","text":"Original HTTP result"}]}}).to_string()))
                    });
                    let changed=futures_util::stream::iter(vec![Ok::<_,std::convert::Infallible>(Event::default().data(json!({"jsonrpc":"2.0","method":"notifications/tools/list_changed"}).to_string()))]);
                    Sse::new(begin.chain(changed).chain(end)).into_response()
                },
                None=>{let send=reply.lock().unwrap().take().unwrap();send.send(body["result"].clone()).unwrap();StatusCode::ACCEPTED.into_response()},
                _=>StatusCode::ACCEPTED.into_response(),
            }
        }
        let f = crate::test_support::Fixture::new();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = format!("http://{}/mcp", listener.local_addr().unwrap());
        let state: Reply = Default::default();
        let router = Router::new()
            .route(
                "/mcp",
                post(handle)
                    .get(|| async { StatusCode::METHOD_NOT_ALLOWED })
                    .delete(|| async { StatusCode::OK }),
            )
            .with_state(state);
        let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        let (send, mut events) = mpsc::unbounded_channel();
        let browser = crate::browser::tests::fixture_registration(
            &f,
            crate::models::AccessMode::Autopilot,
            None,
            send,
        )
        .await;
        let mut session = Session::start(&json!({"url":address}), "HTTP original", &browser)
            .await
            .unwrap();
        let call = async {
            session
                .rpc(
                    "tools/call",
                    json!({"name":"echo","arguments":{}}),
                    &browser,
                )
                .await
                .unwrap()
        };
        let answer = async {
            let event = events.recv().await.unwrap();
            assert_eq!(event.kind, "interaction");
            let request: Value = serde_json::from_str(&event.text).unwrap();
            crate::interactions::answer_question(
                request["id"].as_str().unwrap().into(),
                std::collections::HashMap::from([
                    ("__lumen_connector_action__".into(), vec!["Accept".into()]),
                    ("approved".into(), vec!["true".into()]),
                ]),
            )
            .unwrap();
        };
        let (result, ()) = tokio::join!(call, answer);
        assert_eq!(result["content"][0]["text"], "Original HTTP result");
        assert!(session.take_changed());
        let recorded: Vec<_> = std::iter::from_fn(|| events.try_recv().ok()).collect();
        assert!(crate::interactions::answer_context(&recorded).is_empty());
        session.close().await.unwrap();
        server.abort();
    }
    #[tokio::test]
    async fn http_session_preserves_headers_arguments_and_does_not_replay_expired_calls() {
        let f = crate::test_support::Fixture::new();
        let seen = Arc::new(Mutex::new(Vec::<Value>::new()));
        async fn handle(
            State(seen): State<Arc<Mutex<Vec<Value>>>>,
            headers: HeaderMap,
            Json(body): Json<Value>,
        ) -> Response {
            assert_eq!(headers["authorization"], "Bearer fixture-private");
            seen.lock().unwrap().push(body.clone());
            if body["method"] == "initialize" {
                return ([("mcp-session-id","fixture-session")],Json(json!({"jsonrpc":"2.0","id":body["id"],"result":{"protocolVersion":body["params"]["protocolVersion"],"capabilities":{"tools":{}},"serverInfo":{"name":"http-fixture","version":"1"}}}))).into_response();
            }
            assert_eq!(headers["mcp-session-id"], "fixture-session");
            if body["method"] == "notifications/initialized" {
                return StatusCode::ACCEPTED.into_response();
            }
            if body["method"] == "tools/list" {
                return Json(json!({"jsonrpc":"2.0","id":body["id"],"result":{"tools":[{"name":"echo","inputSchema":{"type":"object"},"annotations":{"readOnlyHint":true}}]}})).into_response();
            }
            if body["method"] == "tools/call" {
                if body["params"]["arguments"]["expire"] == true {
                    return StatusCode::NOT_FOUND.into_response();
                }
                return Json(json!({"jsonrpc":"2.0","id":body["id"],"result":{"content":[{"type":"text","text":body["params"]["arguments"].to_string()}],"structuredContent":{"preserved":true}}})).into_response();
            }
            StatusCode::ACCEPTED.into_response()
        }
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/mcp", listener.local_addr().unwrap());
        let router = Router::new()
            .route(
                "/mcp",
                post(handle)
                    .get(|| async { StatusCode::METHOD_NOT_ALLOWED })
                    .delete(|| async { StatusCode::OK }),
            )
            .with_state(seen.clone());
        let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        let (tx, _) = mpsc::unbounded_channel();
        let browser = crate::browser::tests::fixture_registration(
            &f,
            crate::models::AccessMode::Autopilot,
            None,
            tx,
        )
        .await;
        let mut session = Session::start(
            &json!({"type":"http","url":url,"headers":{"authorization":"Bearer fixture-private"}}),
            "original-http",
            &browser,
        )
        .await
        .unwrap();
        assert!(Session::start(
            &json!({"url":url,"headers":["invalid"]}),
            "invalid-header-config",
            &browser
        )
        .await
        .is_err());
        assert_eq!(
            session
                .rpc("tools/list", json!({}), &browser)
                .await
                .unwrap()["tools"][0]["name"],
            "echo"
        );
        let arguments = json!({"count":3,"enabled":false});
        let result = session
            .rpc(
                "tools/call",
                json!({"name":"echo","arguments":arguments}),
                &browser,
            )
            .await
            .unwrap();
        assert_eq!(result["structuredContent"]["preserved"], true);
        assert_eq!(
            serde_json::from_str::<Value>(result["content"][0]["text"].as_str().unwrap()).unwrap(),
            arguments
        );
        let failed = session
            .rpc(
                "tools/call",
                json!({"name":"echo","arguments":{"expire":true}}),
                &browser,
            )
            .await
            .unwrap_err();
        assert!(!failed.contains("fixture-private"));
        {
            let calls = seen.lock().unwrap();
            assert_eq!(
                calls
                    .iter()
                    .filter(|call| call["method"] == "initialize")
                    .count(),
                1
            );
            assert_eq!(
                calls
                    .iter()
                    .filter(|call| call["params"]["arguments"]["expire"] == true)
                    .count(),
                1
            );
        }
        session.close().await.unwrap();
        server.abort();
    }
}
