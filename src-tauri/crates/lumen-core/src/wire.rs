//! Bounded wire projections for transcript payloads (adapted from T3 Code `WireProjection.ts`,
//! see `resources/legal/t3code/ADAPTATIONS.md`). The store keeps full tool output and diffs;
//! the UI receives a head, a truncation marker and totals, and asks for the rest explicitly.
//! "Full" means up to `KEPT` (1 MiB): longer output is cut once, visibly, before it is stored.
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

/// Activity kinds whose text is plain tool output or diff text, never parsed as JSON.
pub(crate) const BOUNDED: [&str; 6] =
    ["tool", "command", "file", "tool-end", "code-output", "code"];
pub(crate) const LIMIT: usize = 16 * 1024;
/// Tool output and diffs the store keeps, in bytes. Longer text is cut once, with a visible
/// marker, before it is stored, digested or sent, so a digest always finds the same text again.
pub(crate) const KEPT: usize = 1024 * 1024;

/// Applies the store's cap to tool output and diffs. `code` holds JSON and is bounded by script.
pub(crate) fn keep(kind: &str, text: &mut String) {
    if kind == "code" || !BOUNDED.contains(&kind) || text.len() <= KEPT {
        return;
    }
    let marker = format!(
        "\n… [output over 1 MiB not kept: {} bytes in total]",
        text.len()
    );
    let mut end = KEPT - marker.len();
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text.truncate(end);
    text.push_str(&marker);
}

fn digest(text: &str) -> String {
    Sha256::digest(text.as_bytes())
        .iter()
        .take(12)
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Shortens `text` in place when it is oversized and returns the detail reference.
pub(crate) fn bound(kind: &str, text: &mut String) -> Option<Value> {
    let limit = if kind == "code-output" {
        24 * 1024
    } else {
        LIMIT
    };
    if !BOUNDED.contains(&kind) || text.len() <= limit {
        return None;
    }
    let mut end = limit;
    let mut tail = text.len();
    let marker = if kind == "code-output" {
        let marker = format!(
            "\n… [full output: {} bytes, detail {}] …\n",
            text.len(),
            digest(text)
        );
        end = (limit - marker.len()) / 2;
        tail -= end;
        while !text.is_char_boundary(tail) {
            tail += 1;
        }
        marker
    } else {
        String::new()
    };
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let detail = json!({
        "digest": digest(text),
        "totalBytes": text.len(),
        "totalLines": text.lines().count(),
        "shownBytes": end + text.len() - tail,
    });
    if kind == "code-output" {
        *text = format!("{}{marker}{}", &text[..end], &text[tail..]);
        return Some(detail);
    }
    text.truncate(end);
    text.push_str(&format!(
        "\n… [truncated: showing {end} of {} bytes]",
        detail["totalBytes"]
    ));
    Some(detail)
}

/// Bounds one activity object (`{kind,text}`) for the wire.
pub(crate) fn project_activity(activity: &mut Value) {
    let kind = activity["kind"].as_str().unwrap_or("").to_string();
    // Most rows need no change: skip copying their text (it can be up to `KEPT`).
    let short = activity["text"]
        .as_str()
        .is_none_or(|t| t.len() <= LIMIT || !BOUNDED.contains(&kind.as_str()));
    if short && kind != "code" {
        return;
    }
    let Some(mut text) = activity["text"].as_str().map(str::to_string) else {
        return;
    };
    if kind == "code" {
        if let Ok(mut value) = serde_json::from_str::<Value>(&text) {
            if let Some(mut script) = value["script"].as_str().map(str::to_string) {
                if let Some(detail) = bound(&kind, &mut script) {
                    value["script"] = json!(script);
                    activity["text"] = json!(value.to_string());
                    activity["detail"] = detail;
                }
            }
        }
        return;
    }
    if let Some(detail) = bound(&kind, &mut text) {
        activity["text"] = json!(text);
        activity["detail"] = detail;
    }
}

/// Bounds every activity in message snapshots.
pub(crate) fn project_messages(messages: &mut [Value]) {
    for m in messages {
        if let Some(rows) = m["activity"].as_array_mut() {
            rows.iter_mut().for_each(project_activity);
        }
    }
}

/// Keeps the last `limit` messages before the one with id `before` (or before the end), so a
/// client can read a long session from the newest messages backwards in bounded replies.
pub(crate) fn page(
    messages: &mut Vec<Value>,
    before: Option<&str>,
    limit: Option<usize>,
) -> Result<(), String> {
    if let Some(before) = before {
        let end = messages
            .iter()
            .position(|m| m["id"] == before)
            .ok_or("This message is no longer in the conversation")?;
        messages.truncate(end);
    }
    if let Some(limit) = limit {
        messages.drain(..messages.len().saturating_sub(limit));
    }
    Ok(())
}

/// Full text of a bounded activity, found by the digest the projection sent.
pub(crate) fn detail(messages: &[Value], message: &str, wanted: &str) -> Option<String> {
    messages
        .iter()
        .find(|m| m["id"] == message)?
        .get("activity")?
        .as_array()?
        .iter()
        .find_map(|a| {
            let text = a["text"].as_str()?;
            let code;
            let text = if a["kind"] == "code" {
                code = serde_json::from_str::<Value>(text).ok()?;
                code["script"].as_str()?
            } else {
                text
            };
            (text.len() > LIMIT && digest(text) == wanted).then(|| text.to_string())
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortened_tool_output_can_be_fetched_whole_after_it_is_saved() {
        let output = "fixture line\n".repeat(4_000);
        // What a client receives live: the event text, bounded for the wire.
        let mut live = output.clone();
        keep("tool", &mut live);
        let reference = bound("tool", &mut live).expect("over the wire limit");
        // What the store keeps for the message.
        let mut rows = vec![];
        crate::activity::record(
            &mut rows,
            crate::models::Activity {
                kind: "tool".into(),
                text: output.clone(),
            },
        );
        let messages = vec![json!({"id": "m", "activity": rows})];
        assert_eq!(
            detail(&messages, "m", reference["digest"].as_str().unwrap()).as_deref(),
            Some(output.as_str())
        );
    }

    #[test]
    fn output_over_the_store_cap_is_cut_once_and_says_so() {
        let mut text = "é".repeat(KEPT);
        keep("command", &mut text);
        assert!(text.len() <= KEPT);
        assert!(text.ends_with("bytes in total]"));
        let once = text.clone();
        keep("command", &mut text);
        assert_eq!(text, once, "cutting is idempotent, so digests stay stable");
        let mut thinking = "x".repeat(KEPT + 10);
        keep("thinking", &mut thinking);
        assert_eq!(
            thinking.len(),
            KEPT + 10,
            "other kinds keep their own limits"
        );
    }

    #[test]
    fn pages_read_a_session_backwards_without_gaps_or_overlaps() {
        let all: Vec<Value> = (0..7).map(|i| json!({"id": format!("m{i}")})).collect();
        let ids = |v: &[Value]| {
            v.iter()
                .map(|m| m["id"].as_str().unwrap().to_string())
                .collect::<Vec<_>>()
        };
        let mut whole = all.clone();
        page(&mut whole, None, None).unwrap();
        assert_eq!(whole, all, "no arguments keeps the old full read");
        let mut read = vec![];
        let mut before: Option<String> = None;
        loop {
            let mut chunk = all.clone();
            page(&mut chunk, before.as_deref(), Some(3)).unwrap();
            if chunk.is_empty() {
                break;
            }
            before = chunk[0]["id"].as_str().map(str::to_owned);
            read.splice(0..0, ids(&chunk));
        }
        assert_eq!(read, ids(&all));
        let mut gone = all.clone();
        assert!(page(&mut gone, Some("rewound"), Some(3)).is_err());
    }

    #[test]
    fn stored_code_script_can_be_loaded_after_wire_projection() {
        let script = "🦀\n".repeat(LIMIT / 2);
        let mut rows = vec![];
        crate::activity::record(
            &mut rows,
            crate::models::Activity {
                kind: "code".into(),
                text: json!({"id":"run","script":script}).to_string(),
            },
        );
        let stored = vec![json!({"id":"m","activity":rows})];
        let mut wire = stored.clone();
        project_messages(&mut wire);
        let activity = &wire[0]["activity"][0];
        let preview: Value = serde_json::from_str(activity["text"].as_str().unwrap()).unwrap();
        assert_eq!(preview["id"], "run");
        assert!(preview["script"].as_str().unwrap().len() < script.len());
        let digest = activity["detail"]["digest"].as_str().unwrap();
        assert_eq!(detail(&stored, "m", digest), Some(script));
    }
    #[test]
    fn code_output_has_head_tail_and_the_same_recoverable_detail() {
        let full = format!("start {} end", "🦀".repeat(10_000));
        let mut messages = vec![json!({"id":"m","activity":[{"kind":"code-output","text":full}]})];
        let stored = messages.clone();
        project_messages(&mut messages);
        let activity = &messages[0]["activity"][0];
        let text = activity["text"].as_str().unwrap();
        assert!(text.starts_with("start ") && text.ends_with(" end"));
        assert!(text.len() <= 24 * 1024);
        let digest = activity["detail"]["digest"].as_str().unwrap();
        assert!(text.contains(digest));
        assert_eq!(detail(&stored, "m", digest), Some(full));
    }
    #[test]
    fn large_output_is_bounded_with_totals_and_recoverable() {
        let full = "é".repeat(LIMIT) + "\nend";
        let snapshot = vec![json!({"id":"m","activity":[
            {"kind":"tool","text":full},
            {"kind":"orchestra","text":"x".repeat(LIMIT * 2)},
            {"kind":"command","text":"small"}
        ]})];
        let mut wire = snapshot.clone();
        project_messages(&mut wire);
        let a = &wire[0]["activity"];
        let text = a[0]["text"].as_str().unwrap();
        assert!(text.len() < LIMIT + 100 && text.ends_with(" bytes]"));
        assert_eq!(a[0]["detail"]["totalBytes"], full.len());
        assert_eq!(a[0]["detail"]["totalLines"], 2);
        assert!(a[1].get("detail").is_none(), "JSON kinds stay intact");
        assert!(a[2].get("detail").is_none());
        let d = a[0]["detail"]["digest"].as_str().unwrap();
        assert_eq!(detail(&snapshot, "m", d).as_deref(), Some(full.as_str()));
        assert_eq!(detail(&snapshot, "m", "nope"), None);
    }
}
