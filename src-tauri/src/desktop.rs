use serde_json::{json, Value};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tauri::{Emitter, Manager};
use tauri_plugin_opener::OpenerExt;

pub struct Desktop {
    pub client: lumen_ipc::Client,
    pub settings: Mutex<Value>,
}
pub fn daemon_path() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    Ok(exe
        .parent()
        .ok_or("Missing executable directory")?
        .join(if cfg!(windows) {
            "lumen-daemon.exe"
        } else {
            "lumen-daemon"
        }))
}
pub async fn connect(root: PathBuf, resources: PathBuf) -> Result<Arc<Desktop>, String> {
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let daemon = daemon_path()?;
    let client = lumen_ipc::Client {
        endpoint: lumen_ipc::endpoint(&root)?,
        build: lumen_ipc::binary_digest(&daemon)?,
    };
    if lumen_ipc::connect(&client.endpoint).await.is_err() {
        let mut c = std::process::Command::new(&daemon);
        c.arg("--data-dir")
            .arg(root)
            .arg("--resources")
            .arg(resources)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            c.creation_flags(0x08000000);
        }
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            c.process_group(0);
        }
        c.spawn()
            .map_err(|e| format!("Could not start the local Lumen service: {e}"))?;
    }
    let mut last = "Local service did not start".to_string();
    for _ in 0..150 {
        match client.call("get_settings", json!({})).await {
            Ok(settings) => {
                return Ok(Arc::new(Desktop {
                    client,
                    settings: Mutex::new(settings),
                }))
            }
            Err(e) => {
                if e.contains("different build") {
                    return Err(e);
                }
                last = e;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    Err(last)
}
pub async fn events(app: tauri::AppHandle, desktop: Arc<Desktop>) -> Result<(), String> {
    let mut io = desktop.client.start("__events", json!({})).await?;
    lumen_ipc::read::<lumen_ipc::Reply>(&mut io).await?.result?;
    tauri::async_runtime::spawn(async move {
        let mut io = io;
        loop {
            match lumen_ipc::read::<Value>(&mut io).await {
                Ok(e) => {
                    if let Some(event) = e["event"].as_str() {
                        let _ = app.emit(event, e["payload"].clone());
                    }
                }
                Err(_) => {
                    let _ = app.emit("lumen:daemon-disconnected", ());
                    // Reconnect to a restarted service; never replay a mutating command.
                    loop {
                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                        if let Ok(mut next) = desktop.client.start("__events", json!({})).await {
                            if lumen_ipc::read::<lumen_ipc::Reply>(&mut next)
                                .await
                                .is_ok_and(|r| r.result.is_ok())
                            {
                                io = next;
                                let _ = app.emit("lumen:resync", ());
                                break;
                            }
                        }
                    }
                }
            }
        }
    });
    Ok(())
}
pub async fn call(
    app: tauri::AppHandle,
    desktop: Arc<Desktop>,
    command: String,
    mut args: Value,
) -> Result<Value, String> {
    let main = app.get_webview_window("main");
    let capture = command == "capture_screenshot" && args["window"].is_null();
    let visible = capture
        && main
            .as_ref()
            .is_some_and(|w| w.is_visible().unwrap_or(false));
    if capture {
        if let Some(point) = main.as_ref().and_then(|w| w.outer_position().ok()) {
            args["point"] = json!([point.x + 40, point.y + 40]);
        }
        if visible {
            if let Some(w) = &main {
                w.hide().map_err(|e| e.to_string())?;
            }
            tokio::time::sleep(std::time::Duration::from_millis(350)).await;
        }
    }
    let result = desktop.client.call(&command, args).await;
    if visible {
        if let Some(w) = main {
            let _ = w.show();
            let _ = w.set_focus();
        }
    }
    let value = result?;
    match command.as_str() {
        "save_settings" | "reset_settings" => {
            *desktop
                .settings
                .lock()
                .map_err(|_| "Settings lock failed")? = value.clone();
            crate::tray::sync_tray(&app, value["closeToTray"].as_bool().unwrap_or(false))?;
            crate::hotkeys::sync(&app, value["captureHotkey"].as_bool().unwrap_or(false))?;
        }
        "capture_targets" => {
            return Ok(Value::Array(
                value
                    .as_array()
                    .ok_or("Invalid capture targets")?
                    .iter()
                    .filter(|v| v["pid"].as_u64() != Some(std::process::id() as u64))
                    .cloned()
                    .collect(),
            ))
        }
        "open_connector_url" => {
            app.opener()
                .open_url(
                    value.as_str().ok_or("Invalid connector link")?,
                    None::<&str>,
                )
                .map_err(|_| "Could not open connector sign-in")?;
            return Ok(Value::Null);
        }
        "open_response_link" | "open_source_location" => {
            if let Some(url) = value["url"].as_str() {
                app.opener()
                    .open_url(url, None::<&str>)
                    .map_err(|e| e.to_string())?;
                return Ok(Value::Null);
            }
            let path = PathBuf::from(value["path"].as_str().ok_or("Invalid file location")?);
            let line = value["line"].as_u64().unwrap_or(1);
            let at_line = tokio::task::spawn_blocking({
                let path = path.clone();
                move || open_at_line(&path, line)
            })
            .await
            .map_err(|e| e.to_string())?;
            if !at_line {
                app.opener()
                    .open_path(path.to_string_lossy(), None::<&str>)
                    .map_err(|e| e.to_string())?;
            }
            return Ok(if command == "open_source_location" {
                json!(at_line)
            } else {
                Value::Null
            });
        }
        "login_account" => {
            open_login(PathBuf::from(value.as_str().ok_or("Invalid login script")?))?;
            return Ok(Value::Null);
        }
        "open_godot_editor" => {
            std::process::Command::new(
                value["program"]
                    .as_str()
                    .ok_or("Invalid Godot executable")?,
            )
            .args([
                "--editor",
                "--path",
                value["directory"].as_str().ok_or("Invalid Godot project")?,
            ])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?;
            return Ok(Value::Null);
        }
        _ => {}
    }
    Ok(value)
}
fn open_at_line(path: &std::path::Path, line: u64) -> bool {
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
            c.creation_flags(0x08000000);
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
        std::process::Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-NoExit",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
            ])
            .arg(path)
            .creation_flags(0x00000010)
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
    #[cfg(target_os = "linux")]
    {
        let p = path.to_string_lossy();
        if ![
            ("x-terminal-emulator", vec!["-e", p.as_ref()]),
            ("gnome-terminal", vec!["--", p.as_ref()]),
            ("konsole", vec!["-e", p.as_ref()]),
            ("xterm", vec!["-e", p.as_ref()]),
        ]
        .iter()
        .any(|(c, a)| std::process::Command::new(c).args(a).spawn().is_ok())
        {
            return Err(format!(
                "No terminal found. Run {p} in a terminal to sign in."
            ));
        }
    }
    Ok(())
}
