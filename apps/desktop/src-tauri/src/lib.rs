#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, Mutex},
};

use archive_core::{
    ArchiveBackend, ArchiveEntry, ArchiveError, ArchiveErrorCode, ArchiveFormat,
    BackendCapabilities, CompressionProfile, ConflictPolicy,
};
use archive_security::{ExtractionSecurityPolicy, assess_entries};
use archive_sevenzip::SevenZipCliBackend;
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use task_runtime::{TaskManager, TaskSnapshot, TaskSpec};
use tauri::{AppHandle, Emitter, Manager, State};
use uuid::Uuid;

struct ArchiveSession {
    archive: PathBuf,
    entries: Vec<ArchiveEntry>,
    fingerprint: String,
}
struct AppState {
    backend: Arc<SevenZipCliBackend>,
    tasks: Arc<TaskManager>,
    sessions: Mutex<HashMap<String, ArchiveSession>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CommandErrorDto {
    code: String,
    message: String,
    recoverable: bool,
}
impl From<ArchiveError> for CommandErrorDto {
    fn from(value: ArchiveError) -> Self {
        let recoverable = matches!(
            value.code,
            ArchiveErrorCode::WrongPassword
                | ArchiveErrorCode::DiskFull
                | ArchiveErrorCode::FileInUse
                | ArchiveErrorCode::ConflictRequiresDecision
                | ArchiveErrorCode::ArchiveBombRisk
                | ArchiveErrorCode::Cancelled
        );
        Self {
            code: serde_json::to_value(value.code)
                .expect("error code serializes")
                .as_str()
                .unwrap_or("UNKNOWN")
                .to_owned(),
            message: value.message,
            recoverable,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateTaskDto {
    inputs: Vec<PathBuf>,
    output: PathBuf,
    format: ArchiveFormat,
    profile: CompressionProfile,
    password: Option<String>,
    encrypt_headers: bool,
    test_after_create: bool,
    delete_sources_after_success: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExtractTaskDto {
    archive: PathBuf,
    output: PathBuf,
    selected_entries: Option<Vec<String>>,
    conflict_policy: ConflictPolicy,
    password: Option<String>,
    accept_risk: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateTaskDto {
    archive: PathBuf,
    inputs: Vec<PathBuf>,
    password: Option<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ScanResult {
    paths: Vec<PathBuf>,
    archive_paths: Vec<PathBuf>,
    normal_paths: Vec<PathBuf>,
    total_bytes: u64,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ArchiveSessionDto {
    session_id: String,
    format: ArchiveFormat,
    compressed_size: u64,
    estimated_uncompressed_size: u64,
    entry_count: usize,
    encrypted: bool,
    risks: Vec<RiskDto>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RiskDto {
    code: String,
    message: String,
    overridable: bool,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EntryPage {
    entries: Vec<ArchiveEntry>,
    total: usize,
    next_offset: Option<usize>,
}

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
fn secret(password: Option<String>) -> Option<SecretString> {
    password.map(SecretString::from)
}
fn detect(path: &Path) -> ArchiveFormat {
    let lower = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    for (suffix, format) in [
        (".tar.gz", ArchiveFormat::TarGz),
        (".tar.xz", ArchiveFormat::TarXz),
        (".7z", ArchiveFormat::SevenZip),
        (".zip", ArchiveFormat::Zip),
        (".rar", ArchiveFormat::Rar),
        (".tar", ArchiveFormat::Tar),
        (".gz", ArchiveFormat::Gz),
        (".xz", ArchiveFormat::Xz),
        (".bz2", ArchiveFormat::Bz2),
        (".iso", ArchiveFormat::Iso),
        (".cab", ArchiveFormat::Cab),
        (".wim", ArchiveFormat::Wim),
    ] {
        if lower.ends_with(suffix) {
            return format;
        }
    }
    ArchiveFormat::Unknown
}
fn archive_fingerprint(path: &Path) -> String {
    std::fs::metadata(path)
        .map(|metadata| {
            format!(
                "{}:{}",
                metadata.len(),
                metadata
                    .modified()
                    .ok()
                    .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|duration| duration.as_secs())
                    .unwrap_or_default()
            )
        })
        .unwrap_or_default()
}

#[tauri::command]
async fn get_backend_capabilities(
    state: State<'_, AppState>,
) -> Result<BackendCapabilities, CommandErrorDto> {
    state
        .backend
        .capabilities()
        .await
        .map_err(CommandErrorDto::from)
}
#[tauri::command]
fn pick_input_paths(archives_only: bool) -> Vec<PathBuf> {
    let mut dialog = rfd::FileDialog::new();
    if archives_only {
        dialog = dialog.add_filter("压缩包", &["7z", "zip", "rar", "tar", "gz", "xz", "bz2"]);
    }
    dialog.pick_files().unwrap_or_default()
}
#[tauri::command]
async fn scan_input_paths(paths: Vec<PathBuf>) -> Result<ScanResult, CommandErrorDto> {
    let result = tokio::task::spawn_blocking(move || {
        let mut archives = Vec::new();
        let mut normal = Vec::new();
        let mut total = 0;
        for path in &paths {
            if let Ok(metadata) = std::fs::metadata(path) {
                if metadata.is_file() {
                    total += metadata.len();
                }
            }
            if detect(path) == ArchiveFormat::Unknown {
                normal.push(path.clone());
            } else {
                archives.push(path.clone());
            }
        }
        ScanResult {
            paths,
            archive_paths: archives,
            normal_paths: normal,
            total_bytes: total,
        }
    })
    .await
    .map_err(|_| CommandErrorDto {
        code: "UNKNOWN".into(),
        message: "无法扫描输入路径".into(),
        recoverable: true,
    })?;
    Ok(result)
}
#[tauri::command]
async fn prepare_archive_session(
    archive: PathBuf,
    password: Option<String>,
    state: State<'_, AppState>,
) -> Result<ArchiveSessionDto, CommandErrorDto> {
    let format = detect(&archive);
    if format == ArchiveFormat::Unknown {
        return Err(CommandErrorDto::from(ArchiveError::new(
            ArchiveErrorCode::UnsupportedFormat,
            "不支持的压缩包格式",
        )));
    }
    let entries = state
        .backend
        .list(
            archive_core::ListArchiveRequest {
                archive: archive.clone(),
                password: secret(password),
            },
            tokio_util::sync::CancellationToken::new(),
        )
        .await
        .map_err(CommandErrorDto::from)?;
    let compressed_size = std::fs::metadata(&archive)
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    let risks = assess_entries(
        &entries,
        compressed_size,
        None,
        &ExtractionSecurityPolicy::default(),
    );
    let session_id = Uuid::new_v4().to_string();
    state.sessions.lock().expect("session lock").insert(
        session_id.clone(),
        ArchiveSession {
            archive: archive.clone(),
            entries: entries.clone(),
            fingerprint: archive_fingerprint(&archive),
        },
    );
    Ok(ArchiveSessionDto {
        session_id,
        format,
        compressed_size,
        estimated_uncompressed_size: entries.iter().map(|entry| entry.size).sum(),
        entry_count: entries.len(),
        encrypted: entries.iter().any(|entry| entry.encrypted),
        risks: risks
            .into_iter()
            .map(|risk| RiskDto {
                code: format!("{:?}", risk.code),
                message: risk.message,
                overridable: risk.overridable,
            })
            .collect(),
    })
}
#[tauri::command]
fn list_archive_entries(
    session_id: String,
    directory: Option<String>,
    search: Option<String>,
    offset: usize,
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<EntryPage, CommandErrorDto> {
    let sessions = state.sessions.lock().expect("session lock");
    let session = sessions.get(&session_id).ok_or_else(|| CommandErrorDto {
        code: "INVALID_REQUEST".into(),
        message: "压缩包会话已失效，请重新打开".into(),
        recoverable: true,
    })?;
    if session.fingerprint != archive_fingerprint(&session.archive) {
        return Err(CommandErrorDto {
            code: "INVALID_REQUEST".into(),
            message: "压缩包已变化，请重新打开".into(),
            recoverable: true,
        });
    }
    let prefix = directory.unwrap_or_default();
    let needle = search.unwrap_or_default().to_ascii_lowercase();
    let mut entries: Vec<_> = session
        .entries
        .iter()
        .filter(|entry| {
            entry
                .path
                .strip_prefix(&prefix)
                .is_some_and(|rest| !rest.is_empty() && !rest.trim_matches('/').contains('/'))
                && (needle.is_empty() || entry.display_name.to_ascii_lowercase().contains(&needle))
        })
        .cloned()
        .collect();
    entries.sort_by(|left, right| {
        left.is_directory
            .cmp(&right.is_directory)
            .reverse()
            .then_with(|| left.display_name.cmp(&right.display_name))
    });
    let total = entries.len();
    let page_size = limit.unwrap_or(500).min(500);
    let end = (offset + page_size).min(total);
    let page = if offset < total {
        entries[offset..end].to_vec()
    } else {
        vec![]
    };
    Ok(EntryPage {
        entries: page,
        total,
        next_offset: (end < total).then_some(end),
    })
}
#[tauri::command]
fn close_archive_session(session_id: String, state: State<'_, AppState>) {
    state
        .sessions
        .lock()
        .expect("session lock")
        .remove(&session_id);
}
#[tauri::command]
fn create_archive_task(request: CreateTaskDto, state: State<'_, AppState>) -> TaskSnapshot {
    state.tasks.submit(
        TaskSpec::Create {
            inputs: request.inputs,
            output: request.output,
            format: request.format,
            profile: request.profile,
            encrypt_headers: request.encrypt_headers,
            test_after_create: request.test_after_create,
            delete_sources_after_success: request.delete_sources_after_success,
        },
        secret(request.password),
    )
}
#[tauri::command]
fn extract_archive_task(request: ExtractTaskDto, state: State<'_, AppState>) -> TaskSnapshot {
    state.tasks.submit(
        TaskSpec::Extract {
            archive: request.archive,
            output: request.output,
            selected_entries: request.selected_entries,
            conflict_policy: request.conflict_policy,
            accept_risk: request.accept_risk,
        },
        secret(request.password),
    )
}
#[tauri::command]
fn test_archive_task(
    archive: PathBuf,
    password: Option<String>,
    state: State<'_, AppState>,
) -> TaskSnapshot {
    state
        .tasks
        .submit(TaskSpec::Test { archive }, secret(password))
}
#[tauri::command]
fn update_archive_task(request: UpdateTaskDto, state: State<'_, AppState>) -> TaskSnapshot {
    state.tasks.submit(
        TaskSpec::Update {
            archive: request.archive,
            inputs: request.inputs,
        },
        secret(request.password),
    )
}
#[tauri::command]
fn cancel_task(task_id: String, state: State<'_, AppState>) -> Result<(), CommandErrorDto> {
    state.tasks.cancel(&task_id).map_err(CommandErrorDto::from)
}
#[tauri::command]
fn retry_task(
    task_id: String,
    password: Option<String>,
    state: State<'_, AppState>,
) -> Result<TaskSnapshot, CommandErrorDto> {
    state
        .tasks
        .retry(&task_id, secret(password))
        .map_err(CommandErrorDto::from)
}
#[tauri::command]
fn get_tasks(state: State<'_, AppState>) -> Vec<TaskSnapshot> {
    state.tasks.snapshots()
}
#[tauri::command]
fn clear_completed_tasks(state: State<'_, AppState>) {
    state.tasks.clear_completed();
}
#[tauri::command]
fn open_path(path: PathBuf) -> Result<(), CommandErrorDto> {
    Command::new("explorer.exe")
        .arg(&path)
        .spawn()
        .map(|_| ())
        .map_err(|_| CommandErrorDto {
            code: "ACCESS_DENIED".into(),
            message: "无法打开目标位置".into(),
            recoverable: true,
        })
}
#[tauri::command]
fn reveal_in_file_manager(path: PathBuf) -> Result<(), CommandErrorDto> {
    Command::new("explorer.exe")
        .arg("/select,")
        .arg(&path)
        .spawn()
        .map(|_| ())
        .map_err(|_| CommandErrorDto {
            code: "ACCESS_DENIED".into(),
            message: "无法在资源管理器中定位文件".into(),
            recoverable: true,
        })
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let backend = Arc::new(SevenZipCliBackend::new(sidecar_path(app.handle())));
            let history = app.path().app_data_dir()?.join("task-history-v1.json");
            let tasks = TaskManager::new(backend.clone(), history);
            let handle = app.handle().clone();
            let events = tasks.subscribe();
            tauri::async_runtime::spawn(async move {
                let mut events = events;
                while let Ok(event) = events.recv().await {
                    let _ = handle.emit("qzip://task-event", event);
                }
            });
            app.manage(AppState {
                backend,
                tasks,
                sessions: Mutex::new(HashMap::new()),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_backend_capabilities,
            pick_input_paths,
            scan_input_paths,
            prepare_archive_session,
            list_archive_entries,
            close_archive_session,
            create_archive_task,
            extract_archive_task,
            test_archive_task,
            update_archive_task,
            cancel_task,
            retry_task,
            get_tasks,
            clear_completed_tasks,
            open_path,
            reveal_in_file_manager
        ])
        .run(tauri::generate_context!())
        .expect("failed to run QZip desktop application");
}
