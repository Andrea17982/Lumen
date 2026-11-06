//! Quota metadata only; credentials remain inside the official CLI.
use crate::{models::*, providers};
use serde_json::{json, Value};
use std::path::Path;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
pub async fn read(a: &Account, dir: &Path) -> Result<AccountUsage, String> {
    if a.provider == Provider::Antigravity {
        return read_antigravity(a, dir).await;
    }
    if a.provider != Provider::Gpt {
        return Err("Claude CLI does not expose a supported quota query. Enter the values from /usage manually.".into());
    }
    let mut c = providers::command(&a.provider)?;
    providers::isolate(&mut c, &a.provider, dir);
    c.args(["app-server", "--listen", "stdio://"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);
    let mut child = c.spawn().map_err(|e| e.to_string())?;
    let mut stdin = child.stdin.take().ok_or("Missing stdin")?;
    let mut lines = BufReader::new(child.stdout.take().ok_or("Missing stdout")?).lines();
    let query = async {
        stdin.write_all(format!("{}\n",json!({"id":1,"method":"initialize","params":{"clientInfo":{"name":"lumen","version":env!("CARGO_PKG_VERSION")}}})).as_bytes()).await.map_err(|e|e.to_string())?;
        while let Some(line) = lines.next_line().await.map_err(|e| e.to_string())? {
            let Ok(v) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            if v["id"] == 1 {
                if v.get("error").is_some() {
                    return Err("Quota query initialization failed".to_string());
                }
                stdin
                    .write_all(
                        format!(
                            "{}\n{}\n",
                            json!({"method":"initialized"}),
                            json!({"id":2,"method":"account/rateLimits/read"})
                        )
                        .as_bytes(),
                    )
                    .await
                    .map_err(|e| e.to_string())?;
            } else if v["id"] == 2 {
                if v.get("error").is_some() {
                    return Err(
                        "Official CLI could not read account quotas. Check login and CLI version."
                            .into(),
                    );
                }
                let windows = parse(&v["result"]["rateLimits"]);
                if windows.is_empty() {
                    return Err("Official CLI returned no quota windows".into());
                }
                return Ok(AccountUsage {
                    account_id: a.id.clone(),
                    windows,
                    updated_at: now(),
                    source: "codex-cli".into(),
                });
            }
        }
        Err("CLI closed quota query".into())
    };
    let result = tokio::time::timeout(std::time::Duration::from_secs(15), query)
        .await
        .map_err(|_| "Quota query timed out".to_string())
        .and_then(|r| r);
    drop(stdin);
    providers::terminate(&mut child).await;
    result
}
fn parse(value: &Value) -> Vec<UsageWindow> {
    ["primary", "secondary"]
        .iter()
        .filter_map(|key| {
            let v = &value[*key];
            Some(UsageWindow {
                group: None,
                used_percent: v["usedPercent"].as_f64()?,
                resets_at: v["resetsAt"].as_i64()?.checked_mul(1000)?,
                window_minutes: v["windowDurationMins"].as_i64()?,
            })
        })
        .collect()
}

async fn read_antigravity(a: &Account, dir: &Path) -> Result<AccountUsage, String> {
    let mut command = providers::command(&a.provider)?;
    providers::isolate(&mut command, &a.provider, dir);
    command
        .args(["-p", "/usage"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);
    let mut child = command.spawn().map_err(|e| e.to_string())?;
    let stdout = child.stdout.take().ok_or("Missing stdout")?;
    let result = tokio::time::timeout(std::time::Duration::from_secs(15), async {
        use tokio::io::AsyncReadExt;
        let mut output = String::new();
        stdout
            .take(65536)
            .read_to_string(&mut output)
            .await
            .map_err(|e| e.to_string())?;
        let status = child.wait().await.map_err(|e| e.to_string())?;
        if !status.success() {
            return Err("Antigravity quota query failed. Check login and CLI version.".to_string());
        }
        let windows = parse_antigravity(&output)?;
        if windows.is_empty() {
            return Err("Antigravity returned no recognized quota windows".into());
        }
        Ok(AccountUsage {
            account_id: a.id.clone(),
            windows,
            updated_at: now(),
            source: "antigravity-cli".into(),
        })
    })
    .await
    .unwrap_or_else(|_| Err("Antigravity quota query timed out".into()));
    providers::terminate(&mut child).await;
    result
}

fn parse_antigravity(output: &str) -> Result<Vec<UsageWindow>, String> {
    let mut windows = Vec::new();
    for line in output.lines() {
        let fields: Vec<_> = line.split('\t').map(str::trim).collect();
        // Do not replace a complete snapshot with a silently incomplete one if
        // a quota row has changed format or contains invalid values.
        let lower = line.to_ascii_lowercase();
        let quota_row = lower.contains("limit") || lower.contains('%');
        if !quota_row {
            continue;
        }
        let window = (|| {
            if fields.len() != 4 || fields[0].is_empty() {
                return None;
            }
            let minutes = match fields[1] {
                "Five Hour Limit Remaining" => 300,
                "Weekly Limit Remaining" => 10080,
                _ => return None,
            };
            let remaining: f64 = fields[2].strip_suffix('%')?.parse().ok()?;
            if !remaining.is_finite() || !(0.0..=100.0).contains(&remaining) {
                return None;
            }
            let resets_at = chrono::DateTime::parse_from_rfc3339(fields[3])
                .ok()?
                .timestamp_millis();
            Some(UsageWindow {
                group: Some(fields[0].to_owned()),
                used_percent: 100.0 - remaining,
                resets_at,
                window_minutes: minutes,
            })
        })();
        match window {
            Some(window) => windows.push(window),
            None => return Err(
                "Antigravity returned an unrecognized or invalid quota row. Check the CLI version."
                    .into(),
            ),
        }
    }
    Ok(windows)
}

pub fn merge_claude_windows(incoming: &mut AccountUsage, previous: &AccountUsage) {
    for window in &previous.windows {
        if !incoming
            .windows
            .iter()
            .any(|w| w.group == window.group && w.window_minutes == window.window_minutes)
        {
            incoming.windows.push(window.clone());
            // The snapshot is only as fresh as its oldest retained window.
            incoming.updated_at = incoming.updated_at.min(previous.updated_at);
        }
    }
}

pub fn claude_event(account_id: &str, v: &Value) -> Option<AccountUsage> {
    if v["type"] != "rate_limit_event" {
        return None;
    }
    let windows = [("five_hour", 300), ("seven_day", 10080)]
        .iter()
        .filter_map(|(key, minutes)| {
            let w = &v["rate_limit_info"]["unifiedWindows"][*key];
            let fraction = w["utilization"].as_f64()?;
            if !fraction.is_finite() || !(0.0..=1.0).contains(&fraction) {
                return None;
            }
            Some(UsageWindow {
                group: None,
                used_percent: fraction * 100.0,
                resets_at: w["resetsAt"].as_i64()?.checked_mul(1000)?,
                window_minutes: *minutes,
            })
        })
        .collect::<Vec<_>>();
    if windows.is_empty() {
        return None;
    }
    Some(AccountUsage {
        account_id: account_id.into(),
        windows,
        updated_at: now(),
        source: "claude-cli".into(),
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_weekly_only() {
        let w = parse(
            &json!({"primary":{"usedPercent":90,"resetsAt":1800000000_i64,"windowDurationMins":10080},"secondary":null}),
        );
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].resets_at, 1800000000000);
    }
    #[test]
    fn parses_antigravity_groups_and_rejects_invalid_values() {
        let w = parse_antigravity("Gemini Models\tFive Hour Limit Remaining\t99%\t2026-11-06T16:22:29Z\nClaude and GPT models\tWeekly Limit Remaining\t45%\t2026-11-13T14:20:12Z").unwrap();
        assert_eq!(w.len(), 2);
        assert_eq!(w[0].group.as_deref(), Some("Gemini Models"));
        assert_eq!(w[0].used_percent, 1.0);
        assert_eq!(w[1].used_percent, 55.0);
        assert_eq!(w[1].window_minutes, 10080);
        assert!(parse_antigravity("Gemini Models\tFive Hour Limit Remaining\t99%\t2026-11-06T16:22:29Z\nBad\tWeekly Limit Remaining\t101%\tinvalid").is_err());
        assert!(
            parse_antigravity("Models\tDaily Limit Remaining\t50%\t2026-11-06T16:22:29Z").is_err()
        );
        assert!(parse_antigravity("Gemini Models\tFive Hour Limit Remaining\t99%\t2026-11-06T16:22:29Z\nClaude Models Weekly Limit Remaining 45% 2026-11-13T14:20:12Z").is_err());
        assert!(parse_antigravity("Gemini Models\tFive Hour Limit Remaining\t99%\t2026-11-06T16:22:29Z\nClaude Models\tWeekly Remaining Limit\t45%\t2026-11-13T14:20:12Z").is_err());
    }
    #[test]
    fn parses_claude_stream_quota_and_ignores_missing_data() {
        let v = json!({"type":"rate_limit_event","rate_limit_info":{"unifiedWindows":{"five_hour":{"utilization":0.36,"resetsAt":1793982600_i64},"seven_day":{"utilization":0.15,"resetsAt":1794513600_i64}}}});
        let snapshot = claude_event("account", &v).unwrap();
        assert_eq!(snapshot.windows.len(), 2);
        assert_eq!(snapshot.windows[0].used_percent, 36.0);
        assert_eq!(snapshot.windows[1].window_minutes, 10080);
        assert!(claude_event("account", &json!({"type":"rate_limit_event"})).is_none());
        let mut partial = claude_event("account", &json!({"type":"rate_limit_event","rate_limit_info":{"unifiedWindows":{"five_hour":{"utilization":0.5,"resetsAt":1793982600_i64}}}})).unwrap();
        let mut previous = snapshot;
        previous.updated_at = 1;
        merge_claude_windows(&mut partial, &previous);
        assert_eq!(partial.windows.len(), 2);
        assert_eq!(
            partial
                .windows
                .iter()
                .find(|w| w.window_minutes == 300)
                .unwrap()
                .used_percent,
            50.0
        );
        assert_eq!(
            partial
                .windows
                .iter()
                .find(|w| w.window_minutes == 10080)
                .unwrap()
                .used_percent,
            15.0
        );
        assert_eq!(partial.updated_at, 1);
    }
}
