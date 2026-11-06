//! Notification-area icon, close-to-tray and open-at-login.
//!
//! The X hides the window when "Keep Lumen in the notification area" is on; the
//! tray menu offers Open Lumen and Quit. Without the tray, closing while a
//! response runs asks the page to confirm first (`lumen:confirm-quit`).
use crate::Core;
use std::sync::Arc;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_autostart::ManagerExt as _;
use tauri_plugin_notification::NotificationExt as _;

const TRAY_ID: &str = "lumen";
/// Passed by the login item so Lumen can start hidden when the tray is enabled.
pub const AUTOSTART_ARG: &str = "--autostart";

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
    let quit_item =
        MenuItem::with_id(app, "quit", "Quit", true, None::<&str>).map_err(|e| e.to_string())?;
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

/// Register or remove the login item to match the setting.
pub fn sync_autostart(app: &AppHandle, enabled: bool) -> Result<(), String> {
    let launcher = app.autolaunch();
    let current = launcher.is_enabled().unwrap_or(false);
    match (enabled, current) {
        (true, false) => launcher
            .enable()
            .map_err(|e| format!("Could not open at login: {e}")),
        (false, true) => launcher
            .disable()
            .map_err(|e| format!("Could not remove login item: {e}")),
        _ => Ok(()),
    }
}

/// Cancel every running job, wait for provider process trees to exit, then quit.
pub fn quit(app: AppHandle) {
    let core = app.state::<Arc<Core>>().inner().clone();
    let pending = core.cancel_all();
    if !pending {
        app.exit(0);
        return;
    }
    tauri::async_runtime::spawn(async move {
        for _ in 0..300 {
            if core.jobs_idle() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        app.exit(0);
    });
}

/// What the window's X should do.
#[derive(Debug, PartialEq, Eq)]
pub enum CloseAction {
    Hide { first_time: bool },
    Confirm,
    Quit,
}
pub fn close_action(to_tray: bool, hint_shown: bool, running: bool) -> CloseAction {
    if to_tray {
        CloseAction::Hide {
            first_time: !hint_shown,
        }
    } else if running {
        CloseAction::Confirm
    } else {
        CloseAction::Quit
    }
}

pub fn on_close_requested(window: &tauri::Window, api: &tauri::CloseRequestApi) {
    let app = window.app_handle().clone();
    let core = app.state::<Arc<Core>>().inner().clone();
    let (to_tray, hint_shown) = core
        .settings
        .lock()
        .map(|s| (s.close_to_tray, s.tray_hint_shown))
        .unwrap_or((false, true));
    match close_action(to_tray, hint_shown, !core.jobs_idle()) {
        CloseAction::Hide { first_time } => {
            api.prevent_close();
            let _ = window.hide();
            if first_time {
                let _ = app
                    .notification()
                    .builder()
                    .title("Lumen")
                    .body("Lumen is still running in the notification area.")
                    .show();
                if let Ok(mut s) = core.settings.lock() {
                    s.tray_hint_shown = true;
                    let _ = s.save(&core.root);
                }
            }
        }
        CloseAction::Confirm => {
            api.prevent_close();
            let _ = app.emit("lumen:confirm-quit", ());
        }
        CloseAction::Quit => {}
    }
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    quit(app);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn close_button_behaviour() {
        assert_eq!(
            close_action(true, false, true),
            CloseAction::Hide { first_time: true }
        );
        assert_eq!(
            close_action(true, true, false),
            CloseAction::Hide { first_time: false }
        );
        assert_eq!(close_action(false, false, true), CloseAction::Confirm);
        assert_eq!(close_action(false, true, false), CloseAction::Quit);
    }
}
