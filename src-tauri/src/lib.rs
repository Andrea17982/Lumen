mod desktop;
mod hotkeys;
mod tray;
use std::sync::Arc;
use tauri::Manager;

pub fn run() {
    let mut context = tauri::generate_context!();
    if cfg!(debug_assertions) {
        if let Some(root) = std::env::var_os("LUMEN_TEST_DATA_DIR") {
            for window in &mut context.config_mut().app.windows {
                window.data_directory = Some(std::path::PathBuf::from(&root).join("webview"));
            }
        }
    }
    let local: fn(tauri::ipc::Invoke<tauri::Wry>) -> bool =
        tauri::generate_handler![hotkeys::capture_hotkey_registered, tray::quit_app];
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .on_window_event(|w, e| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = e {
                tray::on_close_requested(w, api);
            }
        })
        .setup(|app| {
            let root = if cfg!(debug_assertions) {
                std::env::var_os("LUMEN_TEST_DATA_DIR")
                    .map(std::path::PathBuf::from)
                    .unwrap_or(app.path().app_data_dir()?)
            } else {
                app.path().app_data_dir()?
            };
            let desktop =
                tauri::async_runtime::block_on(desktop::connect(root, app.path().resource_dir()?))
                    .map_err(std::io::Error::other)?;
            app.manage(desktop.clone());
            tauri::async_runtime::block_on(desktop::events(app.handle().clone(), desktop.clone()))
                .map_err(std::io::Error::other)?;
            let settings = desktop
                .settings
                .lock()
                .map_err(|_| std::io::Error::other("Settings lock failed"))?;
            tray::sync_tray(
                app.handle(),
                settings["closeToTray"].as_bool().unwrap_or(false),
            )
            .map_err(std::io::Error::other)?;
            hotkeys::sync(
                app.handle(),
                settings["captureHotkey"].as_bool().unwrap_or(false),
            )
            .map_err(std::io::Error::other)?;
            tray::show_main(app.handle());
            Ok(())
        })
        .invoke_handler(move |invoke| {
            if matches!(
                invoke.message.command(),
                "capture_hotkey_registered" | "quit_app"
            ) {
                return local(invoke);
            }
            let command = invoke.message.command().to_string();
            let args = match invoke.message.payload() {
                tauri::ipc::InvokeBody::Json(v) => v.clone(),
                _ => {
                    invoke.resolver.reject("Expected a JSON command");
                    return true;
                }
            };
            let app = invoke.message.webview().app_handle().clone();
            let desktop = app.state::<Arc<desktop::Desktop>>().inner().clone();
            invoke.resolver.respond_async(async move {
                desktop::call(app, desktop, command, args)
                    .await
                    .map_err(tauri::ipc::InvokeError::from)
            });
            true
        })
        .run(context)
        .expect("Could not start Lumen");
}
