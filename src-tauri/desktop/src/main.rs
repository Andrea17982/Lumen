// Lumen desktop shell. M1 only opens the window; the daemon bridge arrives in M2.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("Could not start Lumen");
}
