// Lumen desktop shell: a window over the daemon. All state and decisions live in the daemon.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod bridge;
mod tray;
mod update;

// The automation server must never reach a release build.
#[cfg(all(feature = "webdriver", not(debug_assertions)))]
compile_error!("the webdriver feature is for debug test builds only");

use bridge::{Bridge, Connection};
use serde_json::{json, Value};
use std::{path::PathBuf, sync::Arc};
use tauri::{Manager, State};
use tray::Prefs;

/// Plugin panels: an opaque, sandboxed document that can run its own inline code and post
/// messages to Lumen, and nothing else (no network, no frames, no Lumen origin, no IPC).
const PANEL_CSP: &str = "default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data:; form-action 'none'; base-uri 'none'; sandbox allow-scripts";

#[tauri::command]
async fn daemon_call(
    app: tauri::AppHandle,
    bridge: State<'_, Arc<Bridge>>,
    command: String,
    mut args: Value,
) -> Result<Value, String> {
    // Repair Lumen's first step and updates run here: the service they stop cannot answer for itself.
    match command.as_str() {
        "restart_service" => return bridge.restart_service().await.map(|_| Value::Null),
        "update_status" => {
            return Ok(json!({
                "current": env!("CARGO_PKG_VERSION"),
                "available": update::known(&app),
                "previous": update::previous(&bridge),
                "installable": update::installable(),
                "problem": app.state::<update::Updates>().problem(),
            }))
        }
        "update_check" => return update::check(&app).await.map(|found| json!(found)),
        // Only the page of the release the shell itself found: the webview names no URL.
        "update_open_page" => {
            use tauri_plugin_opener::OpenerExt;
            let page = update::known(&app)
                .map(|found| found.page)
                .ok_or("No newer version is known")?;
            return app
                .opener()
                .open_url(page, None::<&str>)
                .map(|_| Value::Null)
                .map_err(|e| e.to_string());
        }
        "update_install" => {
            return update::install(&app, bridge.inner())
                .await
                .map(|_| Value::Null)
        }
        "update_back" => {
            return update::go_back(&app, bridge.inner())
                .await
                .map(|_| Value::Null)
        }
        _ => {}
    }
    // A screen capture must not show Lumen itself: the window steps aside for a moment.
    let main = app.get_webview_window("main");
    let screen = command == "capture_screenshot" && args["window"].is_null();
    let hidden = screen
        && main
            .as_ref()
            .is_some_and(|w| w.is_visible().unwrap_or(false));
    if screen {
        let fields = args.as_object_mut().ok_or("Invalid arguments")?;
        if let Some(point) = main.as_ref().and_then(|w| w.outer_position().ok()) {
            fields.insert("point".into(), json!([point.x + 40, point.y + 40]));
        }
        if hidden {
            if let Some(w) = &main {
                w.hide().map_err(|e| e.to_string())?;
            }
            tokio::time::sleep(std::time::Duration::from_millis(350)).await;
        }
    }
    // A capture that hangs must not leave the window hidden: it comes back after 20 s regardless.
    let result = if hidden {
        tokio::time::timeout(
            std::time::Duration::from_secs(20),
            bridge.call(&command, args),
        )
        .await
        .unwrap_or_else(|_| Err("The screenshot took too long".into()))
    } else {
        bridge.call(&command, args).await
    };
    if hidden {
        if let Some(w) = main {
            let _ = w.show();
            let _ = w.set_focus();
        }
    }
    let value = result?;
    match command.as_str() {
        // The daemon saved the settings; a tray that cannot follow them must not undo that.
        "get_settings" | "save_settings" | "reset_settings" => {
            let _ = tray::apply(&app, &value);
            Ok(value)
        }
        // Lumen's own window is never offered as a capture target.
        "capture_targets" => Ok(Value::Array(
            value
                .as_array()
                .ok_or("Invalid capture targets")?
                .iter()
                .filter(|v| v["pid"].as_u64() != Some(u64::from(std::process::id())))
                .cloned()
                .collect(),
        )),
        // The daemon validates the link (web URL without credentials, or a file inside the
        // project) and the shell opens exactly what it returned. The page never opens anything.
        // A project file is shown in its folder or an editor, never launched: agent text could
        // otherwise link to a script and run it on one click.
        "open_response_link" | "open_source_location" => {
            use tauri_plugin_opener::OpenerExt;
            if let Some(url) = value["url"].as_str() {
                app.opener()
                    .open_url(url, None::<&str>)
                    .map_err(|e| e.to_string())?;
                return Ok(Value::Null);
            }
            let path = PathBuf::from(value["path"].as_str().ok_or("Invalid file location")?);
            let line = value["line"].as_u64();
            let in_editor = match line {
                Some(line) => {
                    let path = path.clone();
                    tokio::task::spawn_blocking(move || open_at_line(&path, line))
                        .await
                        .map_err(|e| e.to_string())?
                }
                None => false,
            };
            if !in_editor {
                app.opener()
                    .reveal_item_in_dir(&path)
                    .map_err(|e| e.to_string())?;
            }
            Ok(json!(in_editor))
        }
        // A connector's sign-in URL (validated by the daemon: HTTPS or loopback HTTP, no
        // credentials) can carry a secret in its query: the shell opens it, the page never sees it.
        // Same for a CLI's sign-in page (the daemon accepts only the provider's own HTTPS hosts).
        "open_connector_url" | "open_login_url" => {
            use tauri_plugin_opener::OpenerExt;
            let url = value.as_str().ok_or("Invalid sign-in URL")?;
            app.opener()
                .open_url(url, None::<&str>)
                .map_err(|e| e.to_string())?;
            Ok(Value::Null)
        }
        // The daemon writes a script that runs the official CLI's own sign-in against the
        // account's profile; the shell opens it in a terminal. Lumen never sees credentials.
        "login_account" => {
            open_login(PathBuf::from(
                value.as_str().ok_or("Invalid sign-in script")?,
            ))?;
            Ok(Value::Null)
        }
        _ => Ok(value),
    }
}

/// Opens a source file at a line in an editor that supports it, quietly; false if none did.
fn open_at_line(path: &std::path::Path, line: u64) -> bool {
    // On Windows the editors start through .cmd shims, where cmd.exe expands these characters.
    if cfg!(windows)
        && path
            .to_string_lossy()
            .contains(['%', '&', '^', '!', '|', '<', '>'])
    {
        return false;
    }
    [
        ("code", true),
        ("cursor", true),
        ("zed", false),
        ("subl", false),
    ]
    .iter()
    .any(|(name, goto)| {
        let program = if cfg!(windows) && matches!(*name, "code" | "cursor") {
            format!("{name}.cmd")
        } else {
            name.to_string()
        };
        let mut c = std::process::Command::new(program);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            c.creation_flags(CREATE_NO_WINDOW);
        }
        if *goto {
            c.arg("--goto");
        }
        c.arg(format!("{}:{line}", path.display()))
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    })
}

fn open_login(path: PathBuf) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;
        std::process::Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-NoExit",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
            ])
            .arg(path)
            .creation_flags(CREATE_NEW_CONSOLE)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .args(["-a", "Terminal"])
            .arg(path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let p = path.to_string_lossy();
        let terminals: [(&str, &[&str]); 4] = [
            ("x-terminal-emulator", &["-e", "sh"]),
            ("gnome-terminal", &["--", "sh"]),
            ("konsole", &["-e", "sh"]),
            ("xterm", &["-e", "sh"]),
        ];
        if !terminals.iter().any(|(c, a)| {
            std::process::Command::new(c)
                .args(*a)
                .arg(p.as_ref())
                .spawn()
                .is_ok()
        }) {
            return Err(format!(
                "No terminal found. Run sh {p} in a terminal to sign in."
            ));
        }
    }
    Ok(())
}

/// The folder picker for adding a project. The page gets the chosen path, or nothing.
#[tauri::command]
async fn pick_folder(app: tauri::AppHandle) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog().file().pick_folder(move |folder| {
        let _ = tx.send(folder);
    });
    let folder = rx.await.ok().flatten()?.into_path().ok()?;
    Some(folder.to_string_lossy().into_owned())
}

/// A system notification, only while the window is not in front: in front, the page shows it.
#[tauri::command]
fn notify(app: tauri::AppHandle, title: String, body: String) {
    use tauri_plugin_notification::NotificationExt;
    let focused = app
        .get_webview_window("main")
        .is_some_and(|w| w.is_visible().unwrap_or(false) && w.is_focused().unwrap_or(false));
    if !focused {
        let title: String = title.chars().take(120).collect();
        let body: String = body.chars().take(240).collect();
        let _ = app.notification().builder().title(title).body(body).show();
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

/// On every connection: read the settings the shell acts on (tray, close behaviour).
fn on_connect(app: &tauri::AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let bridge = app.state::<Arc<Bridge>>().inner().clone();
        if let Ok(settings) = bridge.call("get_settings", json!({})).await {
            let _ = tray::apply(&app, &settings);
        }
    });
}

fn main() {
    // One shell per installation. A second launch (Start menu while the window sits in the
    // tray) shows the open window and exits. Two shells would each supervise the service, and
    // one would restart it while the other stops it to install an update or go back.
    // Test builds on a throwaway data directory stay independent of the user's Lumen.
    let isolated = cfg!(debug_assertions) && std::env::var_os("LUMEN_TEST_DATA_DIR").is_some();
    let builder = if isolated {
        tauri::Builder::default()
    } else {
        tauri::Builder::default().plugin(tauri_plugin_single_instance::init(|app, _, _| {
            tray::show_main(app)
        }))
    };
    let builder = builder
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build());
    // Test builds: W3C WebDriver on 127.0.0.1 on TAURI_WEBDRIVER_PORT (tests/app/desktop.mjs).
    #[cfg(feature = "webdriver")]
    let builder = builder.plugin(tauri_plugin_wdio_webdriver::init());
    builder
        .manage(Prefs::default())
        .manage(update::Updates::default())
        .invoke_handler(tauri::generate_handler![
            daemon_call,
            daemon_connection,
            pick_folder,
            notify
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                tray::on_close_requested(window, api);
            }
        })
        .register_asynchronous_uri_scheme_protocol("lumen-plugin", |ctx, request, responder| {
            let app = ctx.app_handle().clone();
            let parts: Vec<String> = request
                .uri()
                .path()
                .split('/')
                .filter(|p| !p.is_empty())
                .map(str::to_owned)
                .collect();
            tauri::async_runtime::spawn(async move {
                let bridge = app.state::<Arc<Bridge>>().inner().clone();
                let html = match parts.as_slice() {
                    [id, panel] => bridge
                        .call("plugin_panel", json!({"id": id, "panel": panel}))
                        .await
                        .ok()
                        .and_then(|v| v.as_str().map(str::to_owned)),
                    _ => None,
                };
                let response = tauri::http::Response::builder()
                    .header("Content-Security-Policy", PANEL_CSP)
                    .header("X-Content-Type-Options", "nosniff")
                    .header("Content-Type", "text/html; charset=utf-8");
                let response = match html {
                    Some(html) => response.status(200).body(html.into_bytes()),
                    None => response
                        .status(404)
                        .body(b"Plugin panel unavailable".to_vec()),
                };
                if let Ok(response) = response {
                    responder.respond(response);
                }
            });
        })
        .setup(|app| {
            #[cfg(feature = "webdriver")]
            app.add_capability(
                tauri::ipc::CapabilityBuilder::new("webdriver")
                    .window("main")
                    .permission("wdio-webdriver:default"),
            )?;
            let (root, resources, daemon) = locations(app)?;
            // A Go back finishes here, before the service opens the database.
            // A failure leaves the current database in place; the app says so instead of a log
            // line nobody sees.
            if let Err(e) = update::settle(&root) {
                app.state::<update::Updates>().report(e);
            }
            let bridge = Arc::new(Bridge::new(root, resources, daemon)?);
            app.manage(bridge.clone());
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move { bridge.supervise(handle, on_connect).await });
            // Development and test builds never reach the network for updates.
            if !cfg!(debug_assertions) {
                tauri::async_runtime::spawn(update::watch(app.handle().clone()));
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Could not start Lumen");
}
