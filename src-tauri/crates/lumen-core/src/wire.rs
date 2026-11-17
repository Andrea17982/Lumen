//! Bounded wire projections for transcript payloads (adapted from T3 Code `WireProjection.ts`,
//! see `resources/legal/t3code/ADAPTATIONS.md`). The store keeps full tool output and diffs;
//! the UI receives a head, a truncation marker and totals, and asks for the rest explicitly.
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

/// Activity kinds whose text is plain tool output or diff text, never parsed as JSON.
const BOUNDED: [&str; 4] = ["tool", "command", "file", "tool-end"];
pub(crate) const LIMIT: usize = 16 * 1024;

fn digest(text: &str) -> String {
    Sha256::digest(text.as_bytes())
        .iter()
        .take(12)
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Shortens `text` in place when it is oversized and returns the detail reference.
fn bound(kind: &str, text: &mut String) -> Option<Value> {
    if !BOUNDED.contains(&kind) || text.len() <= LIMIT {
        return None;
    }
    let mut end = LIMIT;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let detail = json!({
        "digest": digest(text),
        "totalBytes": text.len(),
        "totalLines": text.lines().count(),
        "shownBytes": end,
    });
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
    let Some(mut text) = activity["text"].as_str().map(str::to_string) else {
        return;
    };
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

/// Full text of a bounded activity, found by the digest the projection sent.
pub(crate) fn detail(messages: &[Value], message: &str, wanted: &str) -> Option<String> {
    messages
        .iter()
        .find(|m| m["id"] == message)?
        .get("activity")?
        .as_array()?
        .iter()
        .filter_map(|a| a["text"].as_str())
        .find(|t| t.len() > LIMIT && digest(t) == wanted)
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
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
