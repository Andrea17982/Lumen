//! Native MCP user input. Consent is connection-scoped and never inferred from Autopilot.
use crate::interactions::{Answers, OptionItem, Question};
use serde_json::{json, Value};
const ACTION: &str = "__lumen_connector_action__";

#[derive(Clone)]
pub struct Request {
    pub questions: Vec<Question>,
    pub url: Option<String>,
    pub origin: Option<String>,
    schema: Option<Value>,
}

pub fn validate_scope(params: &Value, thread: &str, turn: &str) -> Result<(), String> {
    if params["threadId"].as_str() != Some(thread)
        || (!params["turnId"].is_null() && params["turnId"].as_str() != Some(turn))
    {
        return Err("Connector request belongs to another thread or turn".into());
    }
    Ok(())
}

impl Request {
    pub fn parse(p: &Value) -> Result<Self, String> {
        let server = p["serverName"]
            .as_str()
            .filter(|s| !s.is_empty() && s.len() <= 256)
            .ok_or("Missing connector name")?;
        if p.to_string().len() > 64000 {
            return Err("Connector request exceeds 64 KB".into());
        }
        if p["message"].as_str().is_none() {
            return Err("Missing connector explanation".into());
        }
        let action = Question {id:ACTION.into(),header:server.into(),question:"Review this connector request. Choose Accept, Decline or Cancel; no choice is automatic.".into(),options:["Accept","Decline","Cancel"].into_iter().map(|label|OptionItem{label:label.into(),description:String::new()}).collect(),multi_select:false,secret:false,optional:false};
        let mut request = Self {
            questions: vec![],
            url: None,
            origin: None,
            schema: None,
        };
        match p["mode"].as_str() {
            Some("url") => {
                if p["elicitationId"].as_str().is_none_or(str::is_empty) {
                    return Err("Missing connector sign-in identifier".into());
                }
                let raw = p["url"].as_str().ok_or("Missing connector URL")?;
                let url = tauri::Url::parse(raw).map_err(|_| "Invalid connector URL")?;
                if !url.username().is_empty()
                    || url.password().is_some()
                    || url.host_str().is_none()
                    || !(url.scheme() == "https"
                        || (url.scheme() == "http"
                            && matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"))))
                {
                    return Err("Connector sign-in requires HTTPS or a loopback HTTP URL".into());
                }
                request.origin = Some(url.origin().ascii_serialization());
                request.url = Some(raw.into());
                // URL-mode messages can repeat the signed URL. Keep both out of stored activity.
            }
            Some("form") => {
                let schema = &p["requestedSchema"];
                let properties = schema["properties"]
                    .as_object()
                    .ok_or("Connector form needs properties")?;
                if schema.as_object().is_none_or(|o| {
                    o.keys().any(|key| {
                        ![
                            "type",
                            "properties",
                            "required",
                            "title",
                            "description",
                            "$schema",
                            "additionalProperties",
                        ]
                        .contains(&key.as_str())
                    })
                }) {
                    return Err("Unsupported connector form constraints".into());
                }
                if schema
                    .get("additionalProperties")
                    .is_some_and(|value| !value.is_boolean())
                {
                    return Err("Unsupported connector property constraints".into());
                }
                if schema["type"] != "object"
                    || properties.len() > 30
                    || properties.contains_key(ACTION)
                {
                    return Err("Unsupported connector form structure".into());
                }
                if let Some(required) = schema.get("required") {
                    let required = required.as_array().ok_or("Invalid required fields")?;
                    if required
                        .iter()
                        .any(|key| key.as_str().is_none_or(|key| !properties.contains_key(key)))
                    {
                        return Err("Unknown required connector field".into());
                    }
                }
                for (id, field) in properties {
                    let kind = field["type"]
                        .as_str()
                        .ok_or("Connector field needs a type")?;
                    if field.as_object().is_none_or(|o| {
                        o.keys().any(|key| {
                            ![
                                "type",
                                "title",
                                "description",
                                "default",
                                "enum",
                                "enumNames",
                                "format",
                                "minLength",
                                "maxLength",
                                "minimum",
                                "maximum",
                            ]
                            .contains(&key.as_str())
                        })
                    }) {
                        return Err("Unsupported connector field constraints".into());
                    }
                    if field
                        .get("format")
                        .is_some_and(|format| format != "password")
                    {
                        return Err("Unsupported connector field format".into());
                    }
                    if ["minLength", "maxLength"]
                        .iter()
                        .any(|key| field.get(key).is_some_and(|value| value.as_u64().is_none()))
                        || ["minimum", "maximum"]
                            .iter()
                            .any(|key| field.get(key).is_some_and(|value| value.as_f64().is_none()))
                    {
                        return Err("Invalid connector field bounds".into());
                    }
                    if !matches!(kind, "string" | "boolean" | "integer" | "number")
                        || ["oneOf", "anyOf", "allOf", "$ref", "pattern"]
                            .iter()
                            .any(|key| field.get(key).is_some())
                    {
                        return Err("Unsupported connector field schema; no consent sent".into());
                    }
                    let values = field
                        .get("enum")
                        .map(|v| v.as_array().ok_or("Invalid connector choices"))
                        .transpose()?;
                    if values.is_some_and(|v| {
                        kind != "string"
                            || v.is_empty()
                            || v.len() > 64
                            || v.iter().any(|v| !v.is_string())
                    }) {
                        return Err("Unsupported connector choices".into());
                    }
                    let options = if kind == "boolean" {
                        vec![
                            OptionItem {
                                label: "true".into(),
                                description: "Yes".into(),
                            },
                            OptionItem {
                                label: "false".into(),
                                description: "No".into(),
                            },
                        ]
                    } else {
                        values
                            .map(|v| {
                                v.iter()
                                    .map(|value| OptionItem {
                                        label: value.as_str().unwrap().into(),
                                        description: String::new(),
                                    })
                                    .collect()
                            })
                            .unwrap_or_default()
                    };
                    let title = field["title"].as_str().unwrap_or(id);
                    let description = field["description"].as_str().unwrap_or("");
                    request.questions.push(Question {
                        id: id.clone(),
                        header: server.into(),
                        question: format!("{title}\n{description}"),
                        options,
                        multi_select: false,
                        secret: field["format"] == "password",
                        optional: !schema["required"]
                            .as_array()
                            .is_some_and(|r| r.iter().any(|key| key == id)),
                    });
                }
                request.schema = Some(schema.clone());
            }
            _ => return Err("Unsupported connector form mode; no consent sent".into()),
        }
        request.questions.insert(0, action);
        let explanation = p["message"]
            .as_str()
            .unwrap()
            .split_whitespace()
            .map(|word| if word.contains("://") { "[link]" } else { word })
            .collect::<Vec<_>>()
            .join(" ");
        if !explanation.is_empty() {
            request.questions[0]
                .question
                .push_str(&format!("\n{explanation}"));
        }
        Ok(request)
    }

    pub fn response(&self, answers: &Answers) -> Result<Value, String> {
        let action = answers
            .get(ACTION)
            .and_then(|a| a.first())
            .map(String::as_str)
            .ok_or("Choose a connector action")?;
        let action = match action {
            "Accept" => "accept",
            "Decline" => "decline",
            "Cancel" => "cancel",
            _ => return Err("Choose Accept, Decline or Cancel".into()),
        };
        if action != "accept" {
            return Ok(json!({"action":action,"content":null}));
        }
        let Some(schema) = &self.schema else {
            return Ok(json!({"action":action,"content":null}));
        };
        let mut content = serde_json::Map::new();
        for (key, field) in schema["properties"].as_object().unwrap() {
            let Some(text) = answers.get(key).and_then(|a| a.first()) else {
                if schema["required"]
                    .as_array()
                    .is_some_and(|r| r.iter().any(|id| id == key))
                {
                    return Err(format!("Missing connector field: {key}"));
                }
                continue;
            };
            let invalid = || format!("Invalid value for connector field: {key}");
            let value = match field["type"].as_str().unwrap() {
                "boolean" => match text.as_str() {
                    "true" => json!(true),
                    "false" => json!(false),
                    _ => return Err(invalid()),
                },
                "integer" => json!(text.parse::<i64>().map_err(|_| invalid())?),
                "number" => {
                    let number = text.parse::<f64>().map_err(|_| invalid())?;
                    if !number.is_finite() {
                        return Err(invalid());
                    }
                    json!(number)
                }
                _ => json!(text),
            };
            if field["enum"]
                .as_array()
                .is_some_and(|values| !values.contains(&value))
                || field["minLength"]
                    .as_u64()
                    .is_some_and(|min| text.chars().count() < (min as usize))
                || field["maxLength"]
                    .as_u64()
                    .is_some_and(|max| text.chars().count() > (max as usize))
                || value.as_f64().is_some_and(|n| {
                    field["minimum"].as_f64().is_some_and(|min| n < min)
                        || field["maximum"].as_f64().is_some_and(|max| n > max)
                })
            {
                return Err(invalid());
            }
            content.insert(key.clone(), value);
        }
        Ok(json!({"action":action,"content":content}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn form() -> Value {
        json!({"serverName":"offline","message":"Choose a scope","mode":"form","requestedSchema":{"type":"object","properties":{"count":{"type":"integer","minimum":1,"maximum":4},"enabled":{"type":"boolean"},"note":{"type":"string","maxLength":4}},"required":["count","enabled"]}})
    }
    fn answers(action: &str) -> Answers {
        Answers::from([
            (ACTION.into(), vec![action.into()]),
            ("count".into(), vec!["3".into()]),
            ("enabled".into(), vec!["false".into()]),
            ("note".into(), vec![]),
        ])
    }
    #[test]
    fn typed_fields_optional_and_declines() {
        let request = Request::parse(&form()).unwrap();
        assert_eq!(
            request.response(&answers("Accept")).unwrap(),
            json!({"action":"accept","content":{"count":3,"enabled":false}})
        );
        for action in ["Decline", "Cancel"] {
            assert!(request
                .response(&Answers::from([(ACTION.into(), vec![action.into()])]))
                .unwrap()["content"]
                .is_null());
        }
        for value in ["0", "5", "1.2", "NaN"] {
            let mut a = answers("Accept");
            a.insert("count".into(), vec![value.into()]);
            assert!(request.response(&a).is_err());
        }
        for constraint in ["pattern", "multipleOf", "const", "oneOf"] {
            let mut f = form();
            f["requestedSchema"]["properties"]["count"][constraint] = json!(2);
            assert!(Request::parse(&f).is_err());
        }
    }
    #[tokio::test]
    async fn private_retry_expiry_and_no_context_replay() {
        let connection = crate::interactions::Connection::default();
        let (activity, receive) = connection
            .ask_connector(Request::parse(&form()).unwrap(), "wire".into())
            .unwrap();
        let id = serde_json::from_str::<Value>(&activity.text).unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string();
        let mut bad = answers("Accept");
        bad.insert("count".into(), vec!["bad".into()]);
        assert!(crate::interactions::answer_question(id.clone(), bad).is_err());
        crate::interactions::answer_question(id.clone(), answers("Accept")).unwrap();
        assert_eq!(receive.await.unwrap()["count"], vec!["3"]);
        assert!(crate::interactions::answer_question(id, answers("Accept")).is_err());
        let mut rows = vec![activity];
        connection.drain(&mut |row| rows.push(row));
        assert_eq!(
            serde_json::from_str::<Value>(&rows[1].text).unwrap()["answers"],
            json!({})
        );
        assert!(crate::interactions::answer_context(&rows).is_empty());
        let (activity, receive) = connection
            .ask_connector(Request::parse(&form()).unwrap(), "cancel".into())
            .unwrap();
        let id = serde_json::from_str::<Value>(&activity.text).unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string();
        connection.cancel("cancel");
        assert!(receive.await.is_err());
        assert!(crate::interactions::answer_question(id, answers("Decline")).is_err());
    }
    #[test]
    fn url_and_scope_are_bounded_to_the_pending_request() {
        let base = json!({"serverName":"offline","message":"Connect https://example.com/auth?secret=private","mode":"url","url":"https://example.com/auth?secret=private","elicitationId":"auth"});
        for url in [
            "file:///c:/secret",
            "javascript:alert(1)",
            "http://remote.example/auth",
            "https://user:password@example.com/auth",
        ] {
            let mut p = base.clone();
            p["url"] = json!(url);
            assert!(Request::parse(&p).is_err());
        }
        let connection = crate::interactions::Connection::default();
        let (activity, _receive) = connection
            .ask_connector(Request::parse(&base).unwrap(), "wire".into())
            .unwrap();
        assert!(!activity.text.contains("private"));
        let id = serde_json::from_str::<Value>(&activity.text).unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(crate::interactions::connector_url(&id)
            .unwrap()
            .contains("private"));
        drop(connection);
        assert!(crate::interactions::connector_url(&id).is_err());
        assert!(validate_scope(&json!({"threadId":"one"}), "one", "turn").is_ok());
        assert!(validate_scope(&json!({"threadId":"two"}), "one", "turn").is_err());
        assert!(validate_scope(&json!({"threadId":"one","turnId":"old"}), "one", "turn").is_err());
    }
}
