//! OpenCode v2 headless adapter. The existing native profile owns credentials and configuration.
use crate::models::*;
use serde_json::Value;
use std::path::Path;

/// Auto's curated OpenCode Go models, one capability tier each (0 fast, 1 balanced, 2 flagship,
/// 3 frontier). The second entry covers a catalog where the first is disabled or missing.
pub const EMERGENCY_TIERS: [&[&str]; 4] = [
    &["opencode-go/claude-haiku-5-5", "opencode-go/gpt-6-luna"],
    &[
        "opencode-go/deepseek-v4.1-flash",
        "opencode-go/minimax-m3.1-flash",
    ],
    &["opencode-go/qwen4-max", "opencode-go/grok-4.7"],
    &["opencode-go/kimi-k3.1-pro", "opencode-go/grok-4.7"],
];
const GO: &str = "OpenCode Go · subscription";
const FREE: &str = "OpenCode · free";
const CREDIT: &str = "OpenCode · console credit";
const EFFORTS: [&str; 7] = ["none", "minimal", "low", "medium", "high", "xhigh", "max"];

async fn api(
    dir: Option<&Path>,
    path: &str,
    cancel: Option<tokio::sync::watch::Receiver<bool>>,
) -> Result<Value, String> {
    let mut command = crate::providers::command(&Provider::Opencode)?;
    crate::providers::isolate(&mut command, &Provider::Opencode, Path::new(""));
    if let Some(dir) = dir {
        command.current_dir(dir);
    }
    command
        .args(["api", "--standalone", "GET", path])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());
    let text = crate::catalog::cli_output(command, cancel).await?;
    let value: Value =
        serde_json::from_str(&text).map_err(|_| "Invalid OpenCode API response".to_string())?;
    if value.get("error").is_some() {
        return Err(format!("OpenCode API rejected {path}"));
    }
    Ok(value.get("data").cloned().unwrap_or(value))
}

/// Every usable model of the profile: real names, reasoning variants and console-credit labels.
/// Providers the profile enables only advertise themselves; catalogue-only providers (activation
/// `auto`, e.g. an unconnected OpenRouter) are left out.
pub async fn models(
    workspace: Option<&Path>,
    cancel: Option<tokio::sync::watch::Receiver<bool>>,
) -> Result<Vec<ModelInfo>, String> {
    let catalog = api(workspace, "/api/model", cancel.clone()).await?;
    let default = api(workspace, "/api/model/default", cancel.clone())
        .await
        .unwrap_or(Value::Null);
    let providers = api(workspace, "/api/provider", cancel)
        .await
        .unwrap_or(Value::Null);
    let enabled = providers.as_array().map(|rows| {
        rows.iter()
            .filter(|p| p["activation"] == "enabled")
            .filter_map(|p| p["id"].as_str().map(str::to_owned))
            .collect::<std::collections::HashSet<_>>()
    });
    let mut models = catalog
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| model_from(v, &default, enabled.as_ref()))
        .collect::<Vec<_>>();
    if models.is_empty() {
        return Err("OpenCode returned no supported model identifiers".into());
    }
    // Subscription models first, then free ones, then models billed to the console balance.
    models.sort_by_key(|m| match m.description.as_str() {
        GO => 0,
        FREE => 1,
        CREDIT => 2,
        _ => 3,
    });
    Ok(models)
}

fn model_from(
    v: &Value,
    default: &Value,
    enabled: Option<&std::collections::HashSet<String>>,
) -> Option<ModelInfo> {
    let provider = v["providerID"].as_str().filter(|p| !p.is_empty())?;
    if enabled.is_some_and(|set| !set.contains(provider)) {
        return None;
    }
    if v["enabled"] == false {
        return None;
    }
    if v["type"].as_str().is_some_and(|t| t != "chat") {
        return None;
    }
    if v["capabilities"]["tools"] != true {
        return None;
    }
    let id = format!("{provider}/{}", v["id"].as_str()?);
    let paid = v["cost"]
        .as_array()
        .and_then(|c| c.first())
        .is_some_and(|c| {
            c["input"].as_f64().unwrap_or(0.0) > 0.0 || c["output"].as_f64().unwrap_or(0.0) > 0.0
        });
    let description = match provider {
        "opencode-go" => GO,
        "opencode" if paid => CREDIT,
        "opencode" => FREE,
        _ => "OpenCode profile",
    };
    let efforts = v["variants"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|variant| variant["id"].as_str())
        .filter(|id| EFFORTS.contains(id))
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let default_effort = v["settings"]["reasoningEffort"]
        .as_str()
        .filter(|e| EFFORTS.contains(e))
        .unwrap_or_default()
        .to_owned();
    let is_default = default["providerID"] == provider && default["id"] == v["id"];
    Some(ModelInfo {
        id,
        name: v["name"].as_str().unwrap_or(provider).into(),
        description: description.into(),
        efforts,
        default_effort,
        is_default,
    })
}

/// Go quota meters from the local server: rolling 5 hours, weekly and the billing month.
pub async fn usage(
    dir: &Path,
    cancel: Option<tokio::sync::watch::Receiver<bool>>,
) -> Result<AccountUsage, String> {
    let data = api(Some(dir), "/api/experimental/usage/limits", cancel).await?;
    let go = &data["go"];
    if !go.is_object() {
        return Err("OpenCode Go is not active in this profile".into());
    }
    let mut windows = vec![];
    for (meter, minutes) in [("rolling", 300), ("weekly", 10_080), ("monthly", 43_200)] {
        let value = &go[meter];
        let Some(percent) = value["percent"].as_f64() else {
            continue;
        };
        let Some(resets_at) = value["resetsAt"].as_i64() else {
            continue;
        };
        let used = if value["limited"] == true {
            100.0
        } else {
            percent.clamp(0.0, 100.0)
        };
        windows.push(UsageWindow {
            group: Some(meter.into()),
            used_percent: used,
            window_minutes: minutes,
            resets_at,
        });
    }
    if windows.is_empty() {
        return Err("OpenCode Go returned no quota meters".into());
    }
    Ok(AccountUsage {
        account_id: String::new(),
        source: "opencode-go".into(),
        updated_at: now(),
        windows,
    })
}

/// Whether Auto may choose this model: the Go subscription and free models only. Models billed
/// to the console balance stay explicit choices.
pub(crate) fn auto_eligible(m: &ModelInfo) -> bool {
    m.description != CREDIT
}

fn tier_of(id: &str) -> Option<usize> {
    EMERGENCY_TIERS.iter().position(|tier| tier.contains(&id))
}

/// The curated tier of an OpenCode model, when it is one of the emergency tiers.
pub(crate) fn tier_of_model(id: &str) -> Option<i32> {
    tier_of(id).map(|tier| tier as i32)
}

/// The emergency model for a task tier. A pinned model is used as-is, or not at all; otherwise the
/// curated model of that tier, then the nearest available tier, going up before down.
pub(crate) fn auto_model<'a>(
    models: &'a [ModelInfo],
    target: i32,
    pinned: Option<&str>,
) -> Option<&'a ModelInfo> {
    if let Some(pinned) = pinned.filter(|p| !p.is_empty()) {
        return models.iter().find(|m| m.id == pinned && auto_eligible(m));
    }
    let target = target.clamp(0, 3) as usize;
    for distance in 0..=3 {
        for tier in [target + distance, target.saturating_sub(distance)] {
            let Some(candidates) = EMERGENCY_TIERS.get(tier) else {
                continue;
            };
            for id in *candidates {
                if let Some(m) = models.iter().find(|m| m.id == *id && auto_eligible(m)) {
                    return Some(m);
                }
            }
        }
    }
    // No curated model (for example without a Go subscription): the best eligible model available.
    let eligible = models
        .iter()
        .filter(|m| auto_eligible(m))
        .collect::<Vec<_>>();
    eligible.iter().copied().min_by_key(|m| {
        let tier = tier_of_model(&m.id).unwrap_or(1);
        ((tier - target as i32).abs(), -tier)
    })
}

/// Budget trades require the exact same tier: a weaker or stronger model is not a quota trade.
pub(crate) fn tier_model<'a>(
    models: &'a [ModelInfo],
    tier: i32,
    pinned: Option<&str>,
) -> Option<&'a ModelInfo> {
    if let Some(pinned) = pinned.filter(|p| !p.is_empty()) {
        return models.iter().find(|m| m.id == pinned && auto_eligible(m));
    }
    EMERGENCY_TIERS
        .get(tier.clamp(0, 3) as usize)
        .and_then(|ids| {
            ids.iter()
                .find_map(|id| models.iter().find(|m| m.id == *id && auto_eligible(m)))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn catalog() -> Value {
        json!([
            {"id":"kimi-k3.1-pro","providerID":"opencode-go","type":"chat","name":"Kimi K3.1 Pro","enabled":true,
             "capabilities":{"tools":true},
             "variants":[{"id":"low"},{"id":"high"},{"id":"max"},{"id":"thinking"}],
             "settings":{"reasoningEffort":"high"},
             "cost":[{"input":2.0,"output":10.0}]},
            {"id":"qwen4-max","providerID":"opencode-go","type":"chat","name":"Qwen4 Max","enabled":true,
             "capabilities":{"tools":true},"variants":[{"id":"high"}],"cost":[{"input":1.5,"output":4.5}]},
            {"id":"seaglass-alpha-free","providerID":"opencode","type":"chat","name":"Seaglass Alpha Free","enabled":true,
             "capabilities":{"tools":true},"variants":[],"cost":[{"input":0.0,"output":0.0}]},
            {"id":"gpt-6.2-sol","providerID":"opencode","type":"chat","name":"GPT-6.2 Sol","enabled":true,
             "capabilities":{"tools":true},"variants":[],"cost":[{"input":2.0,"output":10.0}]},
            {"id":"solar-mini","providerID":"openrouter","type":"chat","name":"Solar Mini","enabled":true,
             "capabilities":{"tools":true},"variants":[],"cost":[{"input":0.1,"output":0.2}]},
            {"id":"no-tools","providerID":"opencode-go","type":"chat","name":"No tools","enabled":true,
             "capabilities":{"tools":false},"variants":[],"cost":[{"input":1.0,"output":1.0}]},
            {"id":"disabled","providerID":"opencode-go","type":"chat","name":"Disabled","enabled":false,
             "capabilities":{"tools":true},"variants":[],"cost":[{"input":1.0,"output":1.0}]}
        ])
    }

    fn parse(catalog: &Value, default: &Value, enabled: &[&str]) -> Vec<ModelInfo> {
        let enabled = enabled.iter().map(|s| s.to_string()).collect();
        let mut rows = catalog
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| model_from(v, default, Some(&enabled)))
            .collect::<Vec<_>>();
        rows.sort_by_key(|m| match m.description.as_str() {
            GO => 0,
            FREE => 1,
            CREDIT => 2,
            _ => 3,
        });
        rows
    }

    #[test]
    fn catalog_exposes_names_efforts_and_subscription_order() {
        let rows = parse(
            &catalog(),
            &json!({"providerID":"opencode-go","id":"qwen4-max"}),
            &["opencode-go", "opencode"],
        );
        let ids: Vec<_> = rows.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "opencode-go/kimi-k3.1-pro",
                "opencode-go/qwen4-max",
                "opencode/seaglass-alpha-free",
                "opencode/gpt-6.2-sol"
            ]
        );
        assert_eq!(rows[0].name, "Kimi K3.1 Pro");
        assert_eq!(rows[0].efforts, ["low", "high", "max"]);
        assert_eq!(rows[0].default_effort, "high");
        assert!(rows[1].is_default);
        assert_eq!(rows[2].description, FREE);
        assert_eq!(rows[3].description, CREDIT);
        assert!(!auto_eligible(&rows[3]));
        // Providers the profile does not enable, models without tools and disabled rows are gone.
        assert!(!ids.iter().any(|id| id.contains("openrouter")
            || id.contains("no-tools")
            || id.contains("disabled")));
    }

    #[test]
    fn catalog_without_provider_listing_keeps_models() {
        let rows = catalog()
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| model_from(v, &Value::Null, None))
            .collect::<Vec<_>>();
        assert!(rows.iter().any(|m| m.id == "opencode-go/kimi-k3.1-pro"));
        assert!(rows.iter().any(|m| m.id == "openrouter/solar-mini"));
    }

    fn model(id: &str) -> ModelInfo {
        ModelInfo {
            id: id.into(),
            name: id.into(),
            description: GO.into(),
            efforts: vec!["low".into(), "medium".into(), "high".into()],
            default_effort: "medium".into(),
            is_default: false,
        }
    }

    #[test]
    fn emergency_model_follows_tiers_up_before_down() {
        let all = EMERGENCY_TIERS
            .iter()
            .flat_map(|tier| tier.iter().copied())
            .map(model)
            .collect::<Vec<_>>();
        assert_eq!(
            auto_model(&all, 0, None).unwrap().id,
            "opencode-go/claude-haiku-5-5"
        );
        assert_eq!(
            auto_model(&all, 1, None).unwrap().id,
            "opencode-go/deepseek-v4.1-flash"
        );
        assert_eq!(
            auto_model(&all, 2, None).unwrap().id,
            "opencode-go/qwen4-max"
        );
        assert_eq!(
            auto_model(&all, 3, None).unwrap().id,
            "opencode-go/kimi-k3.1-pro"
        );
        // Tier 1 gone: go up to tier 2 before dropping to tier 0.
        let without_mid = all
            .iter()
            .filter(|m| tier_of(&m.id) != Some(1))
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(
            auto_model(&without_mid, 1, None).unwrap().id,
            "opencode-go/qwen4-max"
        );
        // A pinned model is the only emergency model used.
        assert_eq!(
            auto_model(&all, 3, Some("opencode-go/grok-4.7"))
                .unwrap()
                .id,
            "opencode-go/grok-4.7"
        );
        assert!(auto_model(&all, 3, Some("opencode-go/missing")).is_none());
        // Budget trades keep the exact tier (an out-of-range tier clamps to the top one).
        assert_eq!(
            tier_model(&all, 1, None).unwrap().id,
            "opencode-go/deepseek-v4.1-flash"
        );
        assert_eq!(
            tier_model(&all, 4, None).unwrap().id,
            "opencode-go/kimi-k3.1-pro"
        );
    }

    #[test]
    fn quota_meters_map_rolling_weekly_and_monthly() {
        let data = json!({"go":{
            "rolling":{"percent":48,"limited":false,"resetsAt":1_800_000_000_000_i64},
            "weekly":{"percent":19,"limited":true,"resetsAt":1_800_600_000_000_i64},
            "monthly":{"percent":9,"limited":false,"resetsAt":1_802_592_000_000_i64}
        }});
        let mut windows = vec![];
        for (meter, minutes) in [("rolling", 300), ("weekly", 10_080), ("monthly", 43_200)] {
            let value = &data["go"][meter];
            windows.push(UsageWindow {
                group: Some(meter.into()),
                used_percent: if value["limited"] == true {
                    100.0
                } else {
                    value["percent"].as_f64().unwrap()
                },
                window_minutes: minutes,
                resets_at: value["resetsAt"].as_i64().unwrap(),
            });
        }
        assert_eq!(windows.len(), 3);
        assert_eq!(windows[1].used_percent, 100.0);
        assert_eq!(windows[2].window_minutes, 43_200);
    }
}
