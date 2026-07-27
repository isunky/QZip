#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

/// Starts the QZip desktop shell.
pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("failed to run QZip desktop application");
}
