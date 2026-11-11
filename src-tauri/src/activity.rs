//! Compact streamed activity without dropping metadata needed to resume or answer input.
use crate::models::Activity;
pub fn record(rows: &mut Vec<Activity>, mut activity: Activity) {
    if activity.kind == "execution" {
        if let Ok(incoming) = serde_json::from_str::<serde_json::Value>(&activity.text) {
            if let Some(id) = incoming["id"].as_str() {
                if let Some(index) = rows.iter().position(|a| {
                    a.kind == "execution"
                        && serde_json::from_str::<serde_json::Value>(&a.text)
                            .is_ok_and(|v| v["id"] == id)
                }) {
                    rows[index] = activity;
                    return;
                }
            }
        }
        if rows.iter().filter(|a| a.kind == "execution").count() >= 64 {
            if let Some(index) = rows.iter().position(|a| a.kind == "execution") {
                rows.remove(index);
            }
        }
    }
    if activity.kind == "execution-phase"
        && rows.iter().filter(|a| a.kind == "execution-phase").count() >= 64
    {
        if let Some(index) = rows.iter().position(|a| a.kind == "execution-phase") {
            rows.remove(index);
        }
    }
    if activity.kind == "thinking" {
        if let Some(last) = rows.last_mut().filter(|a| a.kind == "thinking") {
            last.text.push_str(&activity.text);
            return;
        }
    }
    if activity.kind == "child" {
        if let Ok(incoming) = serde_json::from_str::<serde_json::Value>(&activity.text) {
            if let Some(id) = incoming["id"].as_str() {
                let found = rows.iter().position(|a| {
                    a.kind == "child"
                        && serde_json::from_str::<serde_json::Value>(&a.text)
                            .is_ok_and(|v| v["id"] == id)
                });
                let mut value=found.and_then(|i|serde_json::from_str::<serde_json::Value>(&rows[i].text).ok()).unwrap_or_else(||serde_json::json!({"id":id,"kind":"snapshot","text":"","title":"","status":""}));
                let old = value["text"].as_str().unwrap_or("");
                let text = incoming["text"].as_str().unwrap_or("");
                match incoming["kind"].as_str() {
                    Some("delta") => value["text"] = serde_json::json!(format!("{old}{text}")),
                    Some("message") if !old.ends_with(text) => {
                        value["text"] = serde_json::json!(format!("{old}\n{text}"))
                    }
                    Some("status") => {
                        if !text.is_empty() {
                            value["title"] = serde_json::json!(text);
                        }
                        if let Some(status) = incoming["status"].as_str() {
                            value["status"] = serde_json::json!(status);
                        }
                    }
                    _ => {}
                }
                if let Some(session) = incoming["sessionId"].as_str() {
                    value["sessionId"] = serde_json::json!(session);
                }
                let text = value["text"].as_str().unwrap_or("");
                if text.chars().count() > 20000 {
                    let text = text
                        .chars()
                        .rev()
                        .take(20000)
                        .collect::<String>()
                        .chars()
                        .rev()
                        .collect::<String>();
                    value["text"] = serde_json::json!(text);
                    value["truncated"] = serde_json::json!(true);
                }
                activity.text = value.to_string();
                if let Some(i) = found {
                    rows[i] = activity;
                    return;
                }
            }
        }
    } else if ![
        "interaction",
        "interaction-result",
        "orchestra",
        "native",
        "validation",
    ]
    .contains(&activity.kind.as_str())
    {
        activity.text = activity.text.chars().take(16000).collect();
    }
    rows.push(activity);
    if rows.len() > 500 {
        let remove = rows
            .iter()
            .position(|a| {
                [
                    "tool",
                    "file",
                    "thinking",
                    "child",
                    "model",
                    "resume",
                    "tool-start",
                    "tool-end",
                    "execution",
                ]
                .contains(&a.kind.as_str())
            })
            .unwrap_or(0);
        rows.remove(remove);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn child_streams_preserve_parent_metadata() {
        let mut rows = vec![Activity {
            kind: "checkpoint".into(),
            text: "base".into(),
        }];
        for _ in 0..2000 {
            record(
                &mut rows,
                Activity {
                    kind: "child".into(),
                    text: r#"{"id":"a","kind":"delta","text":"x"}"#.into(),
                },
            );
        }
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].text, "base");
        let child: serde_json::Value = serde_json::from_str(&rows[1].text).unwrap();
        assert_eq!(child["text"].as_str().unwrap().len(), 2000);
    }
}
