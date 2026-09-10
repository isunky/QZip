use std::{
    collections::HashMap,
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

use archive_core::{
    ArchiveBackend, ArchiveEntry, ArchiveError, ArchiveErrorCode, ArchiveFormat,
    BackendCapabilities, CompressionProfile, ConflictPolicy,
};
use archive_entries::{
    EntryPage, EntrySortDto, EntrySortKey, SortDirection, entries_in_directory,
    sort_archive_entries,
};
use archive_security::{ExtractionSecurityPolicy, assess_entries};
use archive_sevenzip::SevenZipCliBackend;
use platform_integration::{AppSettings, AppSettingsPatch, IntegrationStatus, LaunchRequest};
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use task_runtime::{TaskEvent, TaskManager, TaskSnapshot, TaskSpec};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_store::StoreExt;
use uuid::Uuid;

mod archive_entries;
mod preview;
mod shell_integration;
mod system_icons;
mod updates;

struct ArchiveSession {
    archive: PathBuf,
    entries: Vec<ArchiveEntry>,
    fingerprint: String,
    password: Option<SecretString>,
}
struct AppState {
    backend: Arc<SevenZipCliBackend>,
    tasks: Arc<TaskManager>,
    sessions: Mutex<HashMap<String, ArchiveSession>>,
    settings: Mutex<AppSettings>,
    initial_launch_request: Mutex<Option<LaunchRequest>>,
    shell_request_not_before: SystemTime,
}

const SETTINGS_STORE: &str = "settings.json";
const SETTINGS_KEY: &str = "appSettings";
#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

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
struct PerformanceMarker {
    name: String,
    timestamp_unix_milliseconds: u128,
}
fn load_settings(app: &AppHandle) -> AppSettings {
    let loaded = app
        .store(SETTINGS_STORE)
        .ok()
        .and_then(|store| store.get(SETTINGS_KEY));
    let settings = loaded.map(AppSettings::migrated).unwrap_or_default();
    let _ = save_settings(app, &settings);
    settings
}

fn save_settings(app: &AppHandle, settings: &AppSettings) -> Result<(), String> {
    let store = app
        .store(SETTINGS_STORE)
        .map_err(|error| error.to_string())?;
    store.set(
        SETTINGS_KEY,
        serde_json::to_value(settings).map_err(|error| error.to_string())?,
    );
    store.save().map_err(|error| error.to_string())
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
        (".tgz", ArchiveFormat::TarGz),
        (".txz", ArchiveFormat::TarXz),
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
fn format_extension(format: ArchiveFormat) -> &'static str {
    match format {
        ArchiveFormat::SevenZip => "7z",
        ArchiveFormat::Zip => "zip",
        ArchiveFormat::Tar => "tar",
        ArchiveFormat::TarGz => "tar.gz",
        ArchiveFormat::TarXz => "tar.xz",
        _ => "7z",
    }
}
fn unique_path(parent: &Path, stem: &str, extension: &str) -> PathBuf {
    let candidate = parent.join(format!("{stem}.{extension}"));
    if !candidate.exists() {
        return candidate;
    }
    for index in 1..10_000 {
        let candidate = parent.join(format!("{stem} ({index}).{extension}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    parent.join(format!("{stem}-{}.{}", Uuid::new_v4(), extension))
}
fn input_parent(path: &Path) -> PathBuf {
    if path.is_dir() {
        path.parent().unwrap_or(path).to_path_buf()
    } else {
        path.parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf()
    }
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
        dialog = dialog.add_filter(
            "压缩包",
            &["7z", "zip", "rar", "tar", "gz", "tgz", "xz", "txz", "bz2"],
        );
    }
    dialog.pick_files().unwrap_or_default()
}
#[tauri::command]
fn pick_input_folder() -> Option<PathBuf> {
    rfd::FileDialog::new().pick_folder()
}
#[tauri::command]
fn suggest_create_output(
    inputs: Vec<PathBuf>,
    format: ArchiveFormat,
) -> Result<PathBuf, CommandErrorDto> {
    let first = inputs.first().ok_or_else(|| {
        CommandErrorDto::from(ArchiveError::invalid_option(
            "inputs",
            "请先选择要压缩的文件或文件夹",
        ))
    })?;
    let parent = input_parent(first);
    let stem = if inputs.len() == 1 {
        first
            .file_stem()
            .or_else(|| first.file_name())
            .and_then(|v| v.to_str())
            .unwrap_or("新建压缩包")
    } else {
        "压缩文件"
    };
    Ok(unique_path(&parent, stem, format_extension(format)))
}
#[tauri::command]
fn suggest_extract_output(archive: PathBuf, named: bool) -> Result<PathBuf, CommandErrorDto> {
    let parent = archive.parent().ok_or_else(|| {
        CommandErrorDto::from(ArchiveError::new(
            ArchiveErrorCode::InvalidRequest,
            "压缩包路径无效",
        ))
    })?;
    if !named {
        return Ok(parent.to_path_buf());
    }
    let file_name = archive
        .file_name()
        .and_then(|v| v.to_str())
        .unwrap_or("解压结果");
    let stem = [
        ".tar.gz", ".tar.xz", ".tgz", ".txz", ".7z", ".zip", ".rar", ".tar", ".gz", ".xz", ".bz2",
        ".iso", ".cab", ".wim",
    ]
    .iter()
    .find_map(|suffix| file_name.strip_suffix(suffix))
    .unwrap_or(file_name);
    Ok(parent.join(stem))
}
#[tauri::command]
async fn scan_input_paths(paths: Vec<PathBuf>) -> Result<ScanResult, CommandErrorDto> {
    let result = tokio::task::spawn_blocking(move || {
        let mut archives = Vec::new();
        let mut normal = Vec::new();
        let mut total = 0;
        for path in &paths {
            if let Ok(metadata) = std::fs::metadata(path)
                && metadata.is_file()
            {
                total += metadata.len();
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
    let password = secret(password);
    let entries = state
        .backend
        .list(
            archive_core::ListArchiveRequest {
                archive: archive.clone(),
                password: password.clone(),
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
            password,
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
    sort: Option<EntrySortDto>,
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
    let mut entries = entries_in_directory(
        &session.entries,
        directory.as_deref().unwrap_or_default(),
        search.as_deref().unwrap_or_default(),
    );
    let sort = sort.unwrap_or(EntrySortDto {
        key: EntrySortKey::Name,
        direction: SortDirection::Ascending,
    });
    sort_archive_entries(&mut entries, sort.key, sort.direction);
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
async fn create_archive_task(
    request: CreateTaskDto,
    state: State<'_, AppState>,
) -> Result<TaskSnapshot, CommandErrorDto> {
    state
        .tasks
        .submit(
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
        .map_err(CommandErrorDto::from)
}
#[tauri::command]
async fn extract_archive_task(
    request: ExtractTaskDto,
    state: State<'_, AppState>,
) -> Result<TaskSnapshot, CommandErrorDto> {
    state
        .tasks
        .submit(
            TaskSpec::Extract {
                archive: request.archive,
                output: request.output,
                selected_entries: request.selected_entries,
                conflict_policy: request.conflict_policy,
                accept_risk: request.accept_risk,
            },
            secret(request.password),
        )
        .map_err(CommandErrorDto::from)
}
#[tauri::command]
async fn test_archive_task(
    archive: PathBuf,
    password: Option<String>,
    state: State<'_, AppState>,
) -> Result<TaskSnapshot, CommandErrorDto> {
    state
        .tasks
        .submit(TaskSpec::Test { archive }, secret(password))
        .map_err(CommandErrorDto::from)
}
#[tauri::command]
async fn update_archive_task(
    request: UpdateTaskDto,
    state: State<'_, AppState>,
) -> Result<TaskSnapshot, CommandErrorDto> {
    state
        .tasks
        .submit(
            TaskSpec::Update {
                archive: request.archive,
                inputs: request.inputs,
            },
            secret(request.password),
        )
        .map_err(CommandErrorDto::from)
}
#[tauri::command]
fn cancel_task(task_id: String, state: State<'_, AppState>) -> Result<(), CommandErrorDto> {
    state.tasks.cancel(&task_id).map_err(CommandErrorDto::from)
}
#[tauri::command]
async fn retry_task(
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
fn get_app_settings(state: State<'_, AppState>) -> AppSettings {
    state.settings.lock().expect("settings lock").clone()
}
#[tauri::command]
fn update_app_settings(
    patch: AppSettingsPatch,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<AppSettings, String> {
    let mut settings = state.settings.lock().expect("settings lock");
    settings.apply(patch).map_err(|error| error.to_string())?;
    save_settings(&app, &settings)?;
    Ok(settings.clone())
}
#[tauri::command]
fn reset_app_settings(state: State<'_, AppState>, app: AppHandle) -> Result<AppSettings, String> {
    let mut settings = state.settings.lock().expect("settings lock");
    *settings = AppSettings::default();
    save_settings(&app, &settings)?;
    Ok(settings.clone())
}
#[tauri::command]
fn get_integration_status() -> IntegrationStatus {
    #[cfg(target_os = "windows")]
    let modern_context_menu_available = shell_integration::shell_package_is_included();
    #[cfg(not(target_os = "windows"))]
    let modern_context_menu_available = false;
    #[cfg(target_os = "windows")]
    let registered = modern_context_menu_available
        && Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command", "if (Get-AppxPackage -Name 'app.qzip.desktop.shell' -ErrorAction SilentlyContinue) { exit 0 } else { exit 1 }"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW)
            .status()
            .is_ok_and(|status| status.success());
    #[cfg(not(target_os = "windows"))]
    let registered = false;
    #[cfg(target_os = "windows")]
    let file_associations_declared = Command::new("reg.exe")
        .args(["query", r"HKCU\Software\QZip\Capabilities\FileAssociations"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .is_ok_and(|status| status.success());
    #[cfg(not(target_os = "windows"))]
    let file_associations_declared = false;
    IntegrationStatus {
        platform: std::env::consts::OS.to_owned(),
        file_associations_declared,
        modern_context_menu_available,
        modern_context_menu_registered: registered,
        updater_configured: cfg!(feature = "official-updater"),
        distribution: if cfg!(feature = "official-updater") {
            "official-release".to_owned()
        } else {
            "local-or-store-unconfigured".to_owned()
        },
        app_version: env!("CARGO_PKG_VERSION").to_owned(),
    }
}
#[tauri::command]
fn open_default_apps_settings() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        Command::new("explorer.exe")
            .arg("ms-settings:defaultapps?registeredAppUser=QZip")
            .spawn()
            .map(|_| ())
            .map_err(|_| "无法打开 Windows 默认应用设置".to_owned())
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err("当前平台暂不支持打开系统默认应用设置".to_owned())
    }
}
#[tauri::command]
async fn check_for_updates() -> Result<updates::UpdateCheckResult, CommandErrorDto> {
    let current_version = env!("CARGO_PKG_VERSION");
    updates::TLS_PROVIDER_INIT.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
    let client = reqwest::Client::builder()
        .user_agent(format!("QZip/{current_version}"))
        .timeout(updates::UPDATE_REQUEST_TIMEOUT)
        .build()
        .map_err(|error| {
            updates::update_check_error(
                "UPDATE_CHECK_CLIENT",
                format!("无法初始化更新检查：{error}"),
            )
        })?;
    let response = client
        .get(updates::UPDATE_API_URL)
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .send()
        .await
        .map_err(|error| {
            updates::update_check_error(
                "UPDATE_CHECK_NETWORK",
                format!("无法连接 GitHub 检查更新，请检查网络后重试：{error}"),
            )
        })?;
    if !response.status().is_success() {
        let status = response.status();
        let message = if status.as_u16() == 403 || status.as_u16() == 429 {
            "GitHub 请求次数已达到限制，请稍后重试。".to_owned()
        } else {
            format!("GitHub 更新服务返回异常状态（HTTP {}）。", status.as_u16())
        };
        return Err(updates::update_check_error("UPDATE_CHECK_HTTP", message));
    }
    let release = response
        .json::<updates::GitHubRelease>()
        .await
        .map_err(|error| {
            updates::update_check_error(
                "UPDATE_CHECK_INVALID_RESPONSE",
                format!("无法读取 GitHub 返回的版本信息：{error}"),
            )
        })?;
    updates::update_result_for_release(current_version, release)
}
#[tauri::command]
fn take_initial_launch_request(state: State<'_, AppState>) -> Option<LaunchRequest> {
    state
        .initial_launch_request
        .lock()
        .expect("launch request lock")
        .take()
}
#[tauri::command]
fn take_pending_shell_request(state: State<'_, AppState>) -> Option<LaunchRequest> {
    shell_integration::take_pending_shell_request_from_root(
        &shell_integration::shell_request_root()?,
        state.shell_request_not_before,
    )
}

/// Records UI readiness only when the local RC performance harness supplies a
/// constrained temporary output path. Normal application runs do not persist
/// these markers.
#[tauri::command]
fn record_performance_marker(name: String) {
    if !matches!(
        name.as_str(),
        "home-interactive" | "archive-list-first-page" | "archive-error-presented"
    ) {
        return;
    }
    let Ok(raw_path) = std::env::var("QZIP_PERF_MARKER_PATH") else {
        return;
    };
    let path = PathBuf::from(raw_path);
    let valid_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|value| {
            value.starts_with("qzip-performance-markers-") && value.ends_with(".jsonl")
        });
    let Ok(temp_root) = std::env::temp_dir().canonicalize() else {
        return;
    };
    let Ok(parent) = path
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .canonicalize()
    else {
        return;
    };
    if !valid_name || !parent.starts_with(temp_root) {
        return;
    }
    let marker = PerformanceMarker {
        name,
        timestamp_unix_milliseconds: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis(),
    };
    let Ok(line) = serde_json::to_string(&marker) else {
        return;
    };
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{line}");
    }
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if let Some(request) = shell_integration::launch_request_from_args(&args) {
                let _ = app.emit("qzip://launch-request", request);
            }
        }))
        .setup(|app| {
            preview::cleanup_stale_preview_cache();
            shell_integration::retry_shell_registration_after_launch();
            let backend = Arc::new(SevenZipCliBackend::new(sidecar_path(app.handle())));
            let history = app.path().app_data_dir()?.join("task-history-v1.json");
            let tasks = TaskManager::new(backend.clone(), history);
            let handle = app.handle().clone();
            let events = tasks.subscribe();
            let tasks_for_events = Arc::clone(&tasks);
            tauri::async_runtime::spawn(async move {
                let mut events = events;
                loop {
                    match events.recv().await {
                        Ok(event) => {
                            let _ = handle.emit("qzip://task-event", event);
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                            eprintln!(
                                "QZip task event stream lagged by {skipped} events; resyncing"
                            );
                            // Drop stale buffered events before publishing the current
                            // snapshots, so an old progress event cannot overwrite a
                            // terminal state recovered by the resync.
                            events = tasks_for_events.subscribe();
                            for task in tasks_for_events.snapshots() {
                                let _ = handle.emit(
                                    "qzip://task-event",
                                    TaskEvent {
                                        event_type: "task.resync".into(),
                                        task,
                                    },
                                );
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
            });
            let settings = load_settings(app.handle());
            let initial_launch_request =
                shell_integration::launch_request_from_args(&std::env::args().collect::<Vec<_>>());
            let shell_request_not_before = SystemTime::now()
                .checked_sub(Duration::from_secs(15))
                .unwrap_or(SystemTime::UNIX_EPOCH);
            app.manage(AppState {
                backend,
                tasks,
                sessions: Mutex::new(HashMap::new()),
                settings: Mutex::new(settings),
                initial_launch_request: Mutex::new(initial_launch_request),
                shell_request_not_before,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_backend_capabilities,
            pick_input_paths,
            pick_input_folder,
            suggest_create_output,
            suggest_extract_output,
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
            get_app_settings,
            update_app_settings,
            reset_app_settings,
            get_integration_status,
            open_default_apps_settings,
            check_for_updates,
            take_initial_launch_request,
            take_pending_shell_request,
            preview::open_archive_entry,
            system_icons::get_system_file_icons,
            preview::open_path,
            preview::reveal_in_file_manager,
            record_performance_marker
        ])
        .run(tauri::generate_context!())
        .expect("failed to run QZip desktop application");
}
