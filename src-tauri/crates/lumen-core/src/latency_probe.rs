//! Explicit metadata-only probe of installed CLIs. Never submits a model turn.
use crate::{
    catalog,
    models::{Account, Provider},
    providers,
};
use serde_json::json;
use std::{
    collections::HashSet,
    time::{Duration, Instant},
};

#[tokio::test]
#[ignore = "opt-in installed CLI metadata timing; no inference or authentication"]
async fn installed_cli_metadata_latency() {
    let output = std::path::PathBuf::from(
        std::env::var("LUMEN_CLI_LATENCY_OUTPUT").expect("Explicit output required"),
    );
    let root = output
        .parent()
        .unwrap()
        .join(format!("cli-config-{}", uuid::Uuid::new_v4()));
    let mut samples = vec![];
    let mut blocked = HashSet::new();
    for repetition in 0..20 {
        let order = if repetition % 2 == 0 {
            [Provider::Claude, Provider::Gpt]
        } else {
            [Provider::Gpt, Provider::Claude]
        };
        for provider in order {
            let name = provider.binary().to_owned();
            if blocked.contains(&name) {
                continue;
            }
            let dir = root.join(&name);
            std::fs::create_dir_all(&dir).unwrap();
            let account = Account {
                id: name.clone(),
                name: "Isolated metadata probe".into(),
                provider: provider.clone(),
            };
            let start = Instant::now();
            let mut milestones = vec![];
            let version = async {
                let mut c = providers::command(&provider)?;
                providers::isolate(&mut c, &provider, &dir);
                c.arg("--version")
                    .current_dir(&dir)
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::null())
                    .kill_on_drop(true);
                let out = tokio::time::timeout(Duration::from_secs(20), c.output())
                    .await
                    .map_err(|_| "version timeout")?
                    .map_err(|_| "version failed")?;
                if !out.status.success() {
                    return Err("version failed".to_string());
                }
                Ok(String::from_utf8_lossy(&out.stdout)
                    .trim()
                    .chars()
                    .take(100)
                    .collect::<String>())
            }
            .await;
            let version_ms = start.elapsed().as_secs_f64() * 1000.0;
            let catalog_start = Instant::now();
            let models = if version.is_ok() {
                catalog::discover_observed(&account, &dir, None, |label| {
                    milestones.push((label, catalog_start.elapsed().as_secs_f64() * 1000.0))
                })
                .await
            } else {
                Err("version unavailable".into())
            };
            let row = json!({"provider":name,"repetition":repetition,"version":version.as_ref().ok(),"versionMs":version_ms,"catalogMs":catalog_start.elapsed().as_secs_f64()*1000.0,"milestones":milestones,"modelCount":models.as_ref().ok().map(Vec::len),"status":if models.is_ok(){"ok"}else{"prerequisite-unavailable"}});
            println!("metadata {} {} {}", name, repetition, row["status"]);
            if models.is_err() {
                blocked.insert(name);
            }
            samples.push(row);
            std::fs::write(
                &output,
                serde_json::to_vec_pretty(
                    &json!({"inferenceCalls":0,"samples":samples,"complete":false}),
                )
                .unwrap(),
            )
            .unwrap();
        }
    }
    std::fs::write(
        output,
        serde_json::to_vec_pretty(&json!({"inferenceCalls":0,"samples":samples,"complete":true}))
            .unwrap(),
    )
    .unwrap();
}
