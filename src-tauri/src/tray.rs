//! Notification-area icon and interface closing.
//!
//! X hides the interface when the tray is enabled, otherwise it closes only the GUI.
//! Accepted work stays in the daemon. Explicit Quit drains and stops the daemon.
use std::sync::Arc;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

const TRAY_ID: &str = "lumen";

pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// Create or remove the tray icon to match the setting.
pub fn sync_tray(app: &AppHandle, enabled: bool) -> Result<(), String> {
    let existing = app.tray_by_id(TRAY_ID);
    if !enabled {
        if existing.is_some() {
            app.remove_tray_by_id(TRAY_ID);
        }
        return Ok(());
    }
    if existing.is_some() {
        return Ok(());
    }
    let open = MenuItem::with_id(app, "open", "Open Lumen", true, None::<&str>)
        .map_err(|e| e.to_string())?;
    let quit_item = MenuItem::with_id(app, "quit", "Stop Lumen and quit", true, None::<&str>)
        .map_err(|e| e.to_string())?;
    let menu = Menu::with_items(app, &[&open, &quit_item]).map_err(|e| e.to_string())?;
    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("Lumen")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_main(app),
            "quit" => quit(app.clone()),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app).map_err(|e| e.to_string())?;
    Ok(())
}

/// Explicit Quit stops the service; closing the window leaves its work running.
pub fn quit(app: AppHandle) {
    let desktop = app.state::<Arc<crate::desktop::Desktop>>().inner().clone();
    tauri::async_runtime::spawn(async move {
        match desktop
            .client
            .call("__shutdown", serde_json::json!({}))
            .await
        {
            Ok(_) => app.exit(0),
            Err(error) => {
                let _ = app.emit("lumen:daemon-error", error);
            }
        }
    });
}
pub fn on_close_requested(window: &tauri::Window, api: &tauri::CloseRequestApi) {
    let desktop = window.app_handle().state::<Arc<crate::desktop::Desktop>>();
    let (tray, hint) = desktop
        .settings
        .lock()
        .map(|s| {
            (
                s["closeToTray"].as_bool().unwrap_or(false),
                s["trayHintShown"].as_bool().unwrap_or(false),
            )
        })
        .unwrap_or((false, true));
    if tray {
        api.prevent_close();
        let _ = window.hide();
        if !hint {
            let app = window.app_handle().clone();
            let _ = app
                .notification()
                .builder()
                .title("Lumen")
                .body("Lumen is still running in the notification area.")
                .show();
            let desktop = desktop.inner().clone();
            tauri::async_runtime::spawn(async move {
                if desktop
                    .client
                    .call("acknowledge_tray_hint", serde_json::json!({}))
                    .await
                    .is_ok()
                {
                    if let Ok(mut prefs) = desktop.settings.lock() {
                        prefs["trayHintShown"] = serde_json::json!(true);
                    }
                }
            });
        }
    }
}
#[tauri::command]
pub fn quit_app(app: AppHandle) {
    quit(app);
}
