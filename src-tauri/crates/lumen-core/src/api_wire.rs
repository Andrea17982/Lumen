//! Provider wire formats stay separate from permissions and budget enforcement.
use crate::models::Provider;
use serde_json::{json, Value};
pub struct Call {
    pub id: String,
    pub name: String,
    pub args: Value,
}
pub struct Turn {
    pub text: String,
    pub calls: Vec<Call>,
    pub usage: Option<(u64, u64)>,
    pub error: Option<String>,
}
pub fn initial(p: &Provider, prompt: &str) -> Vec<Value> {
    vec![if *p == Provider::Googleapi {
        json!({"role":"user","parts":[{"text":prompt}]})
    } else {
        json!({"role":"user","content":prompt})
    }]
}
pub fn request(
    p: &Provider,
    model: &str,
    history: &[Value],
    tools: &[Value],
    max: u32,
) -> Result<(String, Value), String> {
    let (path, mut body) = match p {
        Provider::Openaiapi => (
            "responses".into(),
            json!({"model":model,"input":history,"tools":tools.iter().map(|t|json!({"type":"function","name":t["name"],"description":t["description"],"parameters":t["inputSchema"],"strict":false})).collect::<Vec<_>>(),"max_output_tokens":max,"store":false}),
        ),
        Provider::Anthropicapi => (
            "messages".into(),
            json!({"model":model,"messages":history,"tools":tools.iter().map(|t|json!({"name":t["name"],"description":t["description"],"input_schema":t["inputSchema"]})).collect::<Vec<_>>(),"max_tokens":max}),
        ),
        Provider::Googleapi => (
            format!("models/{model}:generateContent"),
            json!({"contents":history,"tools":[{"functionDeclarations":tools.iter().map(|t|json!({"name":t["name"],"description":t["description"],"parametersJsonSchema":t["inputSchema"]})).collect::<Vec<_>>()}],"generationConfig":{"maxOutputTokens":max}}),
        ),
        _ => return Err("Not an API provider".into()),
    };
    if tools.is_empty() {
        body.as_object_mut().unwrap().remove("tools");
    }
    Ok((path, body))
}
fn parts_text(parts: &Value) -> String {
    parts
        .as_array()
        .into_iter()
        .flatten()
        .filter(|x| x["thought"] != true)
        .filter_map(|x| x["text"].as_str())
        .collect::<Vec<_>>()
        .join("")
}
pub fn response(p: &Provider, v: &Value) -> Result<Turn, String> {
    let mut turn = Turn {
        text: String::new(),
        calls: vec![],
        usage: None,
        error: None,
    };
    match p {
        Provider::Openaiapi => {
            for item in v["output"]
                .as_array()
                .ok_or("OpenAI returned no output array")?
            {
                if item["type"] == "message" {
                    for part in item["content"].as_array().into_iter().flatten() {
                        if let Some(t) = part["text"].as_str().or(part["refusal"].as_str()) {
                            turn.text.push_str(t)
                        }
                    }
                }
                if item["type"] == "function_call" {
                    turn.calls.push(Call {
                        id: item["call_id"]
                            .as_str()
                            .ok_or("Missing API call ID")?
                            .into(),
                        name: item["name"].as_str().ok_or("Missing tool name")?.into(),
                        args: serde_json::from_str(
                            item["arguments"].as_str().ok_or("Missing tool arguments")?,
                        )
                        .map_err(|_| "Invalid API tool arguments")?,
                    });
                }
            }
            if let (Some(i), Some(o)) = (
                v["usage"]["input_tokens"].as_u64(),
                v["usage"]["output_tokens"].as_u64(),
            ) {
                turn.usage = Some((i, o))
            }
            if v["status"] != "completed" {
                turn.error =
                    Some("OpenAI response did not complete; partial output retained".into())
            }
        }
        Provider::Anthropicapi => {
            let content = v["content"]
                .as_array()
                .ok_or("Anthropic returned no content array")?;
            turn.text = parts_text(&v["content"]);
            for part in content {
                if part["type"] == "tool_use" {
                    turn.calls.push(Call {
                        id: part["id"].as_str().ok_or("Missing API call ID")?.into(),
                        name: part["name"].as_str().ok_or("Missing tool name")?.into(),
                        args: part["input"].clone(),
                    })
                }
            }
            if let (Some(i), Some(o)) = (
                v["usage"]["input_tokens"].as_u64(),
                v["usage"]["output_tokens"].as_u64(),
            ) {
                turn.usage = Some((
                    i.saturating_add(v["usage"]["cache_read_input_tokens"].as_u64().unwrap_or(0))
                        .saturating_add(
                            v["usage"]["cache_creation_input_tokens"]
                                .as_u64()
                                .unwrap_or(0),
                        ),
                    o,
                ))
            }
            if v["stop_reason"] != "end_turn" && v["stop_reason"] != "tool_use" {
                turn.error = Some(
                    "Anthropic response stopped before completion; partial output retained".into(),
                )
            }
        }
        Provider::Googleapi => {
            let candidate = v["candidates"]
                .as_array()
                .and_then(|a| a.first())
                .ok_or("Google returned no candidate; check model policy")?;
            turn.text = parts_text(&candidate["content"]["parts"]);
            for part in candidate["content"]["parts"]
                .as_array()
                .into_iter()
                .flatten()
            {
                if let Some(call) = part.get("functionCall") {
                    turn.calls.push(Call {
                        id: call["id"].as_str().unwrap_or("").into(),
                        name: call["name"].as_str().ok_or("Missing tool name")?.into(),
                        args: call["args"].clone(),
                    })
                }
            }
            if let (Some(i), Some(o)) = (
                v["usageMetadata"]["promptTokenCount"].as_u64(),
                v["usageMetadata"]["candidatesTokenCount"].as_u64(),
            ) {
                turn.usage = Some((
                    i,
                    o.saturating_add(
                        v["usageMetadata"]["thoughtsTokenCount"]
                            .as_u64()
                            .unwrap_or(0),
                    ),
                ))
            }
            if candidate["finishReason"] != "STOP" {
                turn.error = Some(
                    "Google response stopped before completion; partial output retained".into(),
                )
            }
        }
        Provider::Ollama => {
            let message = v["message"]
                .as_object()
                .ok_or("Ollama returned no message")?;
            turn.text = message
                .get("content")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .into();
            for call in message
                .get("tool_calls")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let function = &call["function"];
                turn.calls.push(Call {
                    id: String::new(),
                    name: function["name"]
                        .as_str()
                        .ok_or("Missing Ollama tool name")?
                        .into(),
                    args: function["arguments"].clone(),
                });
            }
            turn.usage = v["prompt_eval_count"]
                .as_u64()
                .zip(v["eval_count"].as_u64());
            if v["done"] != true {
                turn.error = Some("Ollama response did not complete".into());
            }
        }
        _ => return Err("Not an API provider".into()),
    }
    if turn.calls.len() > 32
        || turn
            .calls
            .iter()
            .any(|c| c.name.len() > 128 || !c.args.is_object())
    {
        return Err("Invalid or excessive API tool calls".into());
    }
    Ok(turn)
}
pub fn append_assistant(p: &Provider, history: &mut Vec<Value>, v: &Value) {
    match p {
        Provider::Openaiapi => {
            history.extend(v["output"].as_array().into_iter().flatten().cloned())
        }
        Provider::Anthropicapi => history.push(json!({"role":"assistant","content":v["content"]})),
        Provider::Googleapi => history.push(v["candidates"][0]["content"].clone()),
        Provider::Ollama => history.push(json!({"role":"assistant","content":v["message"]["content"],"tool_calls":v["message"]["tool_calls"]})),
        _ => {}
    }
}
fn tool_text(mut result: Value) -> String {
    if let Some(parts) = result["content"].as_array_mut() {
        parts.retain(|p| p["type"] != "image");
    }
    result.to_string()
}
pub fn append_results(p: &Provider, h: &mut Vec<Value>, results: Vec<(Call, Value)>) {
    match p{
 Provider::Openaiapi=>h.extend(results.into_iter().map(|(c,r)|json!({"type":"function_call_output","call_id":c.id,"output":tool_text(r)}))),
 Provider::Anthropicapi=>h.push(json!({"role":"user","content":results.into_iter().map(|(c,r)|json!({"type":"tool_result","tool_use_id":c.id,"content":tool_text(r)})).collect::<Vec<_>>()})),
 Provider::Googleapi=>h.push(json!({"role":"user","parts":results.into_iter().map(|(c,r)|{let mut value=json!({"functionResponse":{"name":c.name,"response":{"result":tool_text(r)}}});if !c.id.is_empty(){value["functionResponse"]["id"]=json!(c.id)}value}).collect::<Vec<_>>()})),
 Provider::Ollama=>h.extend(results.into_iter().map(|(c,r)|json!({"role":"tool","tool_name":c.name,"content":tool_text(r)}))),_=>{}}
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_three_official_formats_round_trip_calls_and_usage() {
        let cases = [
            (
                Provider::Openaiapi,
                json!({"status":"completed","output":[{"type":"function_call","call_id":"c","name":"lumen_read_file","arguments":"{\"path\":\"a\"}"}],"usage":{"input_tokens":11,"output_tokens":2}}),
            ),
            (
                Provider::Anthropicapi,
                json!({"content":[{"type":"tool_use","id":"c","name":"lumen_read_file","input":{"path":"a"}}],"stop_reason":"tool_use","usage":{"input_tokens":11,"output_tokens":2}}),
            ),
            (
                Provider::Googleapi,
                json!({"candidates":[{"content":{"role":"model","parts":[{"functionCall":{"name":"lumen_read_file","args":{"path":"a"}},"thoughtSignature":"keep-me"}]},"finishReason":"STOP"}],"usageMetadata":{"promptTokenCount":11,"candidatesTokenCount":2}}),
            ),
        ];
        for (p, v) in cases {
            let t = response(&p, &v).unwrap();
            assert_eq!(t.usage, Some((11, 2)));
            assert_eq!(t.calls.len(), 1);
            let mut h = initial(&p, "Read it");
            append_assistant(&p, &mut h, &v);
            append_results(
                &p,
                &mut h,
                t.calls
                    .into_iter()
                    .map(|c| (c, json!({"content":[{"type":"text","text":"ok"}]})))
                    .collect(),
            );
            let (_, r) = request(&p, "m", &h, &[], 256).unwrap();
            assert!(r.to_string().contains("ok"));
            if p == Provider::Googleapi {
                assert!(r.to_string().contains("keep-me"))
            }
        }
    }
    #[test]
    fn truncated_results_keep_text_and_fail_closed() {
        let t=response(&Provider::Openaiapi,&json!({"status":"incomplete","output":[{"type":"message","content":[{"text":"partial"}]}]})).unwrap();
        assert_eq!(t.text, "partial");
        assert!(t.error.is_some());
    }
}
