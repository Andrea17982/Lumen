//! Optional global capture key. Registering never captures; only a key press emits a request.
use tauri::Emitter;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
pub const CAPTURE: &str = "CommandOrControl+Alt+Shift+S";
pub fn sync(app: &tauri::AppHandle, enabled: bool) -> Result<(), String> {
    let keys = app.global_shortcut();
    let registered = keys.is_registered(CAPTURE);
    if enabled && !registered {
        keys.on_shortcut(CAPTURE, |app, _, event| {
            if event.state == ShortcutState::Pressed {
                let _ = app.emit("lumen:capture-hotkey", ());
            }
        })
        .map_err(|e| format!("Could not register the capture shortcut: {e}"))?;
    } else if !enabled && registered {
        keys.unregister(CAPTURE).map_err(|e| e.to_string())?;
    }
    Ok(())
}
#[tauri::command]
pub fn capture_hotkey_registered(app: tauri::AppHandle) -> bool {
    app.global_shortcut().is_registered(CAPTURE)
}
