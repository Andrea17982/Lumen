// Lumen desktop shell: a window over the daemon. All state and decisions live in the daemon.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod bridge;

// The automation server must never reach a release build.
#[cfg(all(feature = "webdriver", not(debug_assertions)))]
compile_error!("the webdriver feature is for debug test builds only");

use bridge::{Bridge, Connection};
use serde_json::Value;
use std::{path::PathBuf, sync::Arc};
use tauri::{Manager, State};

#[tauri::command]
async fn daemon_call(
    app: tauri::AppHandle,
    bridge: State<'_, Arc<Bridge>>,
    command: String,
    args: Value,
) -> Result<Value, String> {
    let value = bridge.call(&command, args).await?;
    match command.as_str() {
        // The daemon validates the link (web URL without credentials, or a file inside the
        // project) and the shell opens exactly what it returned. The page never opens anything.
        // A project file is shown in its folder, never launched: agent text could otherwise
        // link to a script and run it on one click.
        "open_response_link" | "open_source_location" => {
            use tauri_plugin_opener::OpenerExt;
            if let Some(url) = value["url"].as_str() {
                app.opener()
                    .open_url(url, None::<&str>)
                    .map_err(|e| e.to_string())?;
            } else if let Some(path) = value["path"].as_str() {
                app.opener()
                    .reveal_item_in_dir(path)
                    .map_err(|e| e.to_string())?;
            }
            Ok(Value::Null)
        }
        _ => Ok(value),
    }
}

#[tauri::command]
fn daemon_connection(bridge: State<'_, Arc<Bridge>>) -> Connection {
    bridge.connection()
}

const DAEMON: &str = if cfg!(windows) {
    "lumen-daemon.exe"
} else {
    "lumen-daemon"
};

/// Release builds ship the service next to the shell. A development build falls back to the
/// workspace's debug daemon, and may use a throwaway data directory (`LUMEN_TEST_DATA_DIR`).
fn locations(app: &tauri::App) -> Result<(PathBuf, PathBuf, PathBuf), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let beside = exe
        .parent()
        .ok_or("Missing executable directory")?
        .join(DAEMON);
    let mut root = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let mut resources = app.path().resource_dir().map_err(|e| e.to_string())?;
    let mut daemon = beside;
    if cfg!(debug_assertions) {
        let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
        if !daemon.exists() {
            daemon = workspace.join("target").join("debug").join(DAEMON);
            resources = workspace;
        }
        if let Some(test) = std::env::var_os("LUMEN_TEST_DATA_DIR") {
            root = PathBuf::from(test);
        }
    }
    Ok((root, resources, daemon))
}

fn main() {
    let builder = tauri::Builder::default().plugin(tauri_plugin_opener::init());
    // Test builds: W3C WebDriver on 127.0.0.1 on TAURI_WEBDRIVER_PORT (tests/app/desktop.mjs).
    #[cfg(feature = "webdriver")]
    let builder = builder.plugin(tauri_plugin_wdio_webdriver::init());
    builder
        .invoke_handler(tauri::generate_handler![daemon_call, daemon_connection])
        .setup(|app| {
            #[cfg(feature = "webdriver")]
            app.add_capability(
                tauri::ipc::CapabilityBuilder::new("webdriver")
                    .window("main")
                    .permission("wdio-webdriver:default"),
            )?;
            let (root, resources, daemon) = locations(app)?;
            let bridge = Arc::new(Bridge::new(root, resources, daemon)?);
            app.manage(bridge.clone());
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move { bridge.supervise(handle).await });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Could not start Lumen");
}
