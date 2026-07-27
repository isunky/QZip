#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{path::PathBuf, sync::Arc};

use archive_core::{ArchiveBackend, ArchiveError};
use archive_sevenzip::SevenZipCliBackend;
use serde::Serialize;
use tauri::{AppHandle, Manager, State};

struct AppState {
    backend: Arc<SevenZipCliBackend>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CommandErrorDto {
    code: String,
    message: String,
}
impl From<ArchiveError> for CommandErrorDto {
    fn from(value: ArchiveError) -> Self {
        Self {
            code: serde_json::to_value(value.code)
                .expect("error code serializes")
                .as_str()
                .unwrap_or("UNKNOWN")
                .to_owned(),
            message: value.message,
        }
    }
}

/// Keeps resource resolution at the desktop boundary. Domain crates only receive
/// an exact executable path and remain independent of Tauri.
fn sidecar_path(app: &AppHandle) -> PathBuf {
    if let Ok(path) = app
        .path()
        .resource_dir()
        .map(|directory| directory.join("7zip").join("7z.exe"))
        && path.is_file()
    {
        return path;
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("third_party/7zip/bin/win-x64/7z.exe")
}

#[tauri::command]
async fn get_backend_capabilities(
    state: State<'_, AppState>,
) -> Result<archive_core::BackendCapabilities, CommandErrorDto> {
    state
        .backend
        .capabilities()
        .await
        .map_err(CommandErrorDto::from)
}

/// Starts the QZip desktop shell.
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            app.manage(AppState {
                backend: Arc::new(SevenZipCliBackend::new(sidecar_path(app.handle()))),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![get_backend_capabilities])
        .run(tauri::generate_context!())
        .expect("failed to run QZip desktop application");
}
