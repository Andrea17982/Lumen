//! Notification-area icon and closing the window.
//!
//! With the tray on, X hides the window and work keeps running in the service. Without it, X
//! closes only the window: accepted work still stays in the service. Only Quit stops the service.
use crate::bridge::Bridge;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;

const TRAY_ID: &str = "lumen";

/// The daemon's settings as last read or saved through the shell. Display only.
#[derive(Default)]
pub struct Prefs(pub Mutex<Value>);

pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// Keeps the cached settings and the tray icon in line with what the daemon returned.
pub fn apply(app: &AppHandle, settings: &Value) -> Result<(), String> {
    if let Ok(mut prefs) = app.state::<Prefs>().0.lock() {
        *prefs = settings.clone();
    }
    sync_tray(app, settings["closeToTray"].as_bool().unwrap_or(false))
}

fn sync_tray(app: &AppHandle, enabled: bool) -> Result<(), String> {
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
    let quit_item = MenuItem::with_id(app, "quit", "Quit Lumen", true, None::<&str>)
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

/// Quit stops the service, then the window. If the service cannot stop, the window stays.
pub fn quit(app: AppHandle) {
    let bridge = app.state::<Arc<Bridge>>().inner().clone();
    tauri::async_runtime::spawn(async move {
        if bridge.stop_service().await.is_ok() {
            app.exit(0);
        }
    });
}

pub fn on_close_requested(window: &tauri::Window, api: &tauri::CloseRequestApi) {
    let (tray, hint) = window
        .app_handle()
        .state::<Prefs>()
        .0
        .lock()
        .map(|s| {
            (
                s["closeToTray"].as_bool().unwrap_or(false),
                s["trayHintShown"].as_bool().unwrap_or(false),
            )
        })
        .unwrap_or((false, true));
    if !tray {
        return;
    }
    api.prevent_close();
    let _ = window.hide();
    if hint {
        return;
    }
    let app = window.app_handle().clone();
    let _ = app
        .notification()
        .builder()
        .title("Lumen")
        .body("Lumen is still running in the notification area.")
        .show();
    if let Ok(mut prefs) = app.state::<Prefs>().0.lock() {
        prefs["trayHintShown"] = json!(true);
    }
    let bridge = app.state::<Arc<Bridge>>().inner().clone();
    tauri::async_runtime::spawn(async move {
        let _ = bridge.call("acknowledge_tray_hint", json!({})).await;
    });
}
