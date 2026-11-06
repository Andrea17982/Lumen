//! Optional OpenAI-compatible endpoint on 127.0.0.1, backed by the official CLIs (read-only).
use crate::{models::now, Core};
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{
        sse::{Event, Sse},
        IntoResponse, Response,
    },
    routing::{get, post},
    Json, Router,
};
use serde::Serialize;
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};

pub const MODELS: [&str; 3] = ["lumen-auto", "lumen-claude", "lumen-chatgpt"];

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiLog {
    pub at: i64,
    pub model: String,
    pub status: String,
    pub stream: bool,
    pub millis: i64,
}

pub struct Running {
    pub port: u16,
    pub stop: oneshot::Sender<()>,
}

fn log(core: &Core, entry: ApiLog) {
    if let Ok(mut l) = core.api_log.lock() {
        l.push_front(entry);
        l.truncate(50);
    }
}

fn error(status: StatusCode, message: &str) -> Response {
    (
        status,
        Json(json!({"error":{"message":message,"type":"lumen_error"}})),
    )
        .into_response()
}

fn authorized(core: &Core, headers: &HeaderMap) -> bool {
    let token = core
        .settings
        .lock()
        .map(|s| s.api_token.clone())
        .unwrap_or_default();
    headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .is_some_and(|t| !token.is_empty() && t.trim() == token)
}

/// Flattens OpenAI chat messages (string or text-part content) into one CLI prompt.
pub fn flatten(messages: &[Value]) -> String {
    messages
        .iter()
        .map(|m| {
            let role = m["role"].as_str().unwrap_or("user");
            let content = match &m["content"] {
                Value::String(s) => s.clone(),
                Value::Array(parts) => parts
                    .iter()
                    .filter_map(|p| p["text"].as_str())
                    .collect::<Vec<_>>()
                    .join("\n"),
                _ => String::new(),
            };
            format!("{role}:\n{content}")
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

async fn models(State(core): State<Arc<Core>>, headers: HeaderMap) -> Response {
    if !authorized(&core, &headers) {
        return error(StatusCode::UNAUTHORIZED, "Invalid API key");
    }
    Json(json!({"object":"list","data":MODELS.iter().map(|m| json!({"id":m,"object":"model","owned_by":"lumen"})).collect::<Vec<_>>()})).into_response()
}

async fn completions(
    State(core): State<Arc<Core>>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    if !authorized(&core, &headers) {
        return error(StatusCode::UNAUTHORIZED, "Invalid API key");
    }
    let model = body["model"].as_str().unwrap_or("lumen-auto").to_string();
    if !MODELS.contains(&model.as_str()) && model != "lumen-gpt" {
        return error(
            StatusCode::NOT_FOUND,
            "Unknown model. Use lumen-auto, lumen-claude or lumen-chatgpt.",
        );
    }
    let Some(messages) = body["messages"].as_array() else {
        return error(StatusCode::BAD_REQUEST, "messages is required");
    };
    let prompt = flatten(messages);
    if prompt.trim().is_empty() || prompt.len() > 400_000 {
        return error(
            StatusCode::BAD_REQUEST,
            "Prompt is empty or larger than 400 KB",
        );
    }
    let stream = body["stream"].as_bool().unwrap_or(false);
    let started = now();
    let id = format!("chatcmpl-{}", crate::models::id().replace('-', ""));
    let created = started / 1000;
    if !stream {
        let result = crate::headless(&core, &model, &prompt, |_| {}).await;
        log(
            &core,
            ApiLog {
                at: started,
                model: model.clone(),
                status: result
                    .as_ref()
                    .map(|_| "200".into())
                    .unwrap_or_else(|e| e.clone()),
                stream,
                millis: now() - started,
            },
        );
        return match result {
            Ok(text) => {
                let prompt_tokens = crate::context::estimate(&prompt);
                let completion_tokens = crate::context::estimate(&text);
                Json(json!({
                    "id": id, "object": "chat.completion", "created": created, "model": model,
                    "choices": [{"index":0,"message":{"role":"assistant","content":text},"finish_reason":"stop"}],
                    "usage": {"prompt_tokens":prompt_tokens,"completion_tokens":completion_tokens,"total_tokens":prompt_tokens+completion_tokens}
                }))
                .into_response()
            }
            Err(e) => error(StatusCode::BAD_GATEWAY, &e),
        };
    }
    let (tx, rx) = mpsc::channel::<Result<Event, std::convert::Infallible>>(256);
    let worker = core.clone();
    tokio::spawn(async move {
        let chunk = |delta: Value, finish: Value| {
            Event::default().data(
                json!({"id":id,"object":"chat.completion.chunk","created":created,"model":model,"choices":[{"index":0,"delta":delta,"finish_reason":finish}]}).to_string(),
            )
        };
        let _ = tx
            .send(Ok(chunk(json!({"role":"assistant"}), Value::Null)))
            .await;
        let sender = tx.clone();
        let first = chunk;
        let result = crate::headless(&worker, &model, &prompt, move |piece| {
            let _ = sender.try_send(Ok(first(json!({ "content": piece }), Value::Null)));
        })
        .await;
        if let Err(e) = &result {
            let _ = tx
                .send(Ok(chunk(
                    json!({"content": format!("\n[Lumen error: {e}]")}),
                    Value::Null,
                )))
                .await;
        }
        let _ = tx.send(Ok(chunk(json!({}), json!("stop")))).await;
        let _ = tx.send(Ok(Event::default().data("[DONE]"))).await;
        log(
            &worker,
            ApiLog {
                at: started,
                model: model.clone(),
                status: result.map(|_| "200".into()).unwrap_or_else(|e| e),
                stream: true,
                millis: now() - started,
            },
        );
    });
    Sse::new(tokio_stream::wrappers::ReceiverStream::new(rx)).into_response()
}

pub async fn start(core: Arc<Core>, port: u16) -> Result<Running, String> {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port))
        .await
        .map_err(|e| format!("Port {port} is not available: {e}"))?;
    let app = Router::new()
        .route("/v1/models", get(models))
        .route("/v1/chat/completions", post(completions))
        .layer(axum::extract::DefaultBodyLimit::max(1_000_000))
        .with_state(core);
    let (stop, rx) = oneshot::channel::<()>();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = rx.await;
            })
            .await;
    });
    Ok(Running { port, stop })
}

#[cfg(test)]
mod tests {
    #[test]
    fn flattens_text_parts() {
        let m = serde_json::json!([{"role":"system","content":"Be brief"},{"role":"user","content":[{"type":"text","text":"Hi"}]}]);
        let p = super::flatten(m.as_array().unwrap());
        assert!(p.contains("system:\nBe brief") && p.contains("user:\nHi"));
    }
}
