//! Quota metadata only; credentials remain inside the official CLI.
use crate::{models::*, providers};
use serde_json::{json, Value};
use std::path::Path;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
pub async fn read(a: &Account, dir: &Path) -> Result<AccountUsage, String> {
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
                used_percent: v["usedPercent"].as_f64()?,
                resets_at: v["resetsAt"].as_i64()?.checked_mul(1000)?,
                window_minutes: v["windowDurationMins"].as_i64()?,
            })
        })
        .collect()
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
}
