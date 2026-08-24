use std::{
    collections::{HashMap, HashSet},
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
    BackendCapabilities, CompressionProfile, ConflictPolicy, ExtractArchiveRequest,
    NoopProgressReporter,
};
use archive_security::{ExtractionSecurityPolicy, assess_entries, output_path};
use archive_sevenzip::SevenZipCliBackend;
use platform_integration::{
    AppSettings, AppSettingsPatch, IntegrationStatus, LaunchKind, LaunchRequest,
};
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use task_runtime::{TaskManager, TaskSnapshot, TaskSpec};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_store::StoreExt;
use uuid::Uuid;

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
const PREVIEW_CACHE_TTL: Duration = Duration::from_secs(7 * 24 * 60 * 60);
const PREVIEW_MAX_FILE_SIZE: u64 = 1024 * 1024 * 1024;
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
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EntryPage {
    entries: Vec<ArchiveEntry>,
    total: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    next_offset: Option<usize>,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
enum EntrySortKey {
    Name,
    Size,
    Type,
    Modified,
}

#[derive(Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum SortDirection {
    Ascending,
    Descending,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EntrySortDto {
    key: EntrySortKey,
    direction: SortDirection,
}

fn entries_in_directory(
    archive_entries: &[ArchiveEntry],
    directory: &str,
    search: &str,
) -> Vec<ArchiveEntry> {
    let directory = directory.replace('\\', "/");
    let directory = directory.trim_matches('/');
    let prefix = if directory.is_empty() {
        String::new()
    } else {
        format!("{directory}/")
    };
    let needle = search.to_ascii_lowercase();
    let mut entries = HashMap::<String, ArchiveEntry>::new();

    for entry in archive_entries {
        let Some(relative_path) = entry.path.strip_prefix(&prefix) else {
            continue;
        };
        if relative_path.is_empty() {
            continue;
        }

        if let Some((folder_name, _)) = relative_path.split_once('/') {
            if folder_name.is_empty() {
                continue;
            }
            let path = format!("{prefix}{folder_name}");
            entries.entry(path.clone()).or_insert_with(|| ArchiveEntry {
                path,
                display_name: folder_name.to_owned(),
                size: 0,
                compressed_size: None,
                is_directory: true,
                modified_at: None,
                crc: None,
                attributes: Some("D".to_owned()),
                encrypted: entry.encrypted,
                is_symlink: false,
                is_hardlink: false,
            });
        } else {
            entries.insert(entry.path.clone(), entry.clone());
        }
    }

    let mut entries = entries
        .into_values()
        .filter(|entry| {
            needle.is_empty() || entry.display_name.to_ascii_lowercase().contains(&needle)
        })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| {
        left.is_directory
            .cmp(&right.is_directory)
            .reverse()
            .then_with(|| left.display_name.cmp(&right.display_name))
    });
    entries
}

fn sort_archive_entries(
    entries: &mut [ArchiveEntry],
    sort_key: EntrySortKey,
    direction: SortDirection,
) {
    entries.sort_by(|left, right| {
        let folder_order = right.is_directory.cmp(&left.is_directory);
        if !folder_order.is_eq() {
            return folder_order;
        }

        let field_order = match sort_key {
            EntrySortKey::Name => left
                .display_name
                .to_lowercase()
                .cmp(&right.display_name.to_lowercase()),
            EntrySortKey::Size => left.size.cmp(&right.size),
            EntrySortKey::Type => {
                archive_entry_extension(left).cmp(&archive_entry_extension(right))
            }
            EntrySortKey::Modified => left
                .modified_at
                .as_deref()
                .unwrap_or_default()
                .cmp(right.modified_at.as_deref().unwrap_or_default()),
        };
        let field_order = if direction == SortDirection::Descending {
            field_order.reverse()
        } else {
            field_order
        };
        field_order.then_with(|| {
            left.display_name
                .to_lowercase()
                .cmp(&right.display_name.to_lowercase())
        })
    });
}

fn archive_entry_extension(entry: &ArchiveEntry) -> String {
    entry
        .display_name
        .rsplit_once('.')
        .filter(|(stem, extension)| !stem.is_empty() && !extension.is_empty())
        .map(|(_, extension)| extension.to_ascii_lowercase())
        .unwrap_or_default()
}

#[cfg(test)]
mod archive_entry_tests {
    use super::*;

    fn file(path: &str) -> ArchiveEntry {
        ArchiveEntry {
            path: path.to_owned(),
            display_name: path.rsplit('/').next().unwrap_or(path).to_owned(),
            size: 12,
            compressed_size: Some(9),
            is_directory: false,
            modified_at: None,
            crc: None,
            attributes: Some("A".to_owned()),
            encrypted: false,
            is_symlink: false,
            is_hardlink: false,
        }
    }

    #[test]
    fn creates_navigable_folders_for_archives_without_directory_entries() {
        let archive_entries = vec![
            file("附件/封面.docx"),
            file("附件/投标人承诺函.docx"),
            file("招标文件.pdf"),
        ];

        let root = entries_in_directory(&archive_entries, "", "");
        assert_eq!(root.len(), 2);
        assert_eq!(root[0].path, "附件");
        assert!(root[0].is_directory);
        assert_eq!(root[1].path, "招标文件.pdf");

        let attachment = entries_in_directory(&archive_entries, "附件", "");
        assert_eq!(attachment.len(), 2);
        assert!(attachment.iter().all(|entry| !entry.is_directory));
        assert!(
            attachment
                .iter()
                .any(|entry| entry.display_name == "封面.docx")
        );
    }

    #[test]
    fn directory_matching_does_not_include_similar_prefixes() {
        let archive_entries = vec![file("附件/inside.txt"), file("附件二/outside.txt")];

        let entries = entries_in_directory(&archive_entries, "附件", "");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path, "附件/inside.txt");
    }

    #[test]
    fn sorts_the_full_entry_set_while_keeping_folders_first() {
        let mut entries = vec![
            file("small.txt"),
            ArchiveEntry {
                path: "文件夹".into(),
                display_name: "文件夹".into(),
                size: 0,
                compressed_size: None,
                is_directory: true,
                modified_at: None,
                crc: None,
                attributes: Some("D".into()),
                encrypted: false,
                is_symlink: false,
                is_hardlink: false,
            },
            ArchiveEntry {
                size: 99,
                ..file("large.pdf")
            },
        ];

        sort_archive_entries(&mut entries, EntrySortKey::Size, SortDirection::Descending);
        assert!(entries[0].is_directory);
        assert_eq!(entries[1].display_name, "large.pdf");
        assert_eq!(entries[2].display_name, "small.txt");
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateCheckResult {
    configured: bool,
    status: &'static str,
}

#[derive(Deserialize)]
struct ShellRequestFile {
    action: String,
    paths: Vec<PathBuf>,
}

fn launch_kind(value: &str) -> Option<LaunchKind> {
    match value {
        "open" => Some(LaunchKind::Open),
        "compress-sevenzip" => Some(LaunchKind::CompressSevenZip),
        "compress-zip" => Some(LaunchKind::CompressZip),
        "extract-here" => Some(LaunchKind::ExtractHere),
        "extract-named" => Some(LaunchKind::ExtractNamed),
        "more-options" => Some(LaunchKind::MoreOptions),
        _ => None,
    }
}

fn shell_request_root() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA")
        .map(|base| PathBuf::from(base).join("QZip").join("ShellRequests"))
}

fn consume_shell_request_from_root(root: &Path, token: &str) -> Option<LaunchRequest> {
    Uuid::parse_str(token).ok()?;
    let path = root.join(format!("{token}.json"));
    let canonical_root = root.canonicalize().ok()?;
    let canonical_path = path.canonicalize().ok()?;
    if !canonical_path.starts_with(&canonical_root)
        || canonical_path.extension().and_then(|value| value.to_str()) != Some("json")
    {
        return None;
    }
    let metadata = std::fs::metadata(&canonical_path).ok()?;
    if metadata.len() > 4 * 1024 * 1024 {
        return None;
    }
    let request: ShellRequestFile =
        serde_json::from_slice(&std::fs::read(&canonical_path).ok()?).ok()?;
    let _ = std::fs::remove_file(&canonical_path);
    let paths = request
        .paths
        .into_iter()
        .filter(|path| path.exists())
        .take(1_000)
        .collect::<Vec<_>>();
    let kind = launch_kind(&request.action)?;
    (!paths.is_empty()).then(|| LaunchRequest {
        kind,
        paths,
        source: "shell".to_owned(),
    })
}

fn consume_shell_request(token: &str) -> Option<LaunchRequest> {
    consume_shell_request_from_root(&shell_request_root()?, token)
}

fn take_pending_shell_request_from_root(
    root: &Path,
    not_before: SystemTime,
) -> Option<LaunchRequest> {
    let mut candidates = std::fs::read_dir(root)
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                return None;
            }
            let modified = entry.metadata().ok()?.modified().ok()?;
            if modified < not_before {
                return None;
            }
            let token = path.file_stem()?.to_str()?.to_owned();
            Uuid::parse_str(&token).ok()?;
            Some((modified, token))
        })
        .collect::<Vec<_>>();
    candidates.sort_by_key(|(modified, _)| *modified);
    candidates
        .into_iter()
        .rev()
        .find_map(|(_, token)| consume_shell_request_from_root(root, &token))
}

fn launch_request_from_args(args: &[String]) -> Option<LaunchRequest> {
    if let Some(index) = args.iter().position(|arg| arg == "--shell-request") {
        return args
            .get(index + 1)
            .and_then(|token| consume_shell_request(token));
    }
    let paths = args
        .iter()
        .skip(1)
        .map(PathBuf::from)
        .filter(|path| path.is_file())
        .take(1_000)
        .collect::<Vec<_>>();
    (!paths.is_empty()).then(|| LaunchRequest {
        kind: LaunchKind::Open,
        paths,
        source: "fileAssociation".to_owned(),
    })
}

#[cfg(test)]
mod shell_request_tests {
    use super::*;

    #[test]
    fn consumes_braced_windows_guid_request() {
        let root = std::env::temp_dir().join(format!("qzip-shell-request-{}", Uuid::new_v4()));
        let input = root.join("selected-folder");
        std::fs::create_dir_all(&input).expect("create shell request fixture");
        let token = format!("{{{}}}", Uuid::new_v4().to_string().to_uppercase());
        let request_path = root.join(format!("{token}.json"));
        let body = serde_json::json!({
            "action": "compress-sevenzip",
            "paths": [input]
        });
        std::fs::write(
            &request_path,
            serde_json::to_vec(&body).expect("serialize shell request fixture"),
        )
        .expect("write shell request fixture");

        let request = take_pending_shell_request_from_root(&root, SystemTime::UNIX_EPOCH)
            .expect("consume braced Windows GUID request");

        assert!(matches!(request.kind, LaunchKind::CompressSevenZip));
        assert_eq!(request.source, "shell");
        assert!(!request_path.exists());
        std::fs::remove_dir_all(&root).expect("remove shell request fixture");
    }

    #[test]
    fn detects_compound_archive_aliases() {
        assert_eq!(detect(Path::new("release.tgz")), ArchiveFormat::TarGz);
        assert_eq!(detect(Path::new("release.txz")), ArchiveFormat::TarXz);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn detects_whether_the_optional_shell_package_is_included() {
        let root = std::env::temp_dir().join(format!("qzip-shell-package-{}", Uuid::new_v4()));
        let shell_root = root.join("qzip-shell");
        std::fs::create_dir_all(&shell_root).expect("create shell package fixture");

        assert!(!shell_package_is_included_at(&root));
        std::fs::write(shell_root.join("QZip.Shell.msix"), b"fixture")
            .expect("write shell package fixture");
        assert!(shell_package_is_included_at(&root));

        std::fs::remove_dir_all(&root).expect("remove shell package fixture");
    }
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

#[cfg(target_os = "windows")]
fn shell_package_is_included_at(install_path: &Path) -> bool {
    install_path
        .join("qzip-shell")
        .join("QZip.Shell.msix")
        .is_file()
}

#[cfg(target_os = "windows")]
fn shell_package_is_included() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|executable| executable.parent().map(Path::to_path_buf))
        .is_some_and(|install_path| shell_package_is_included_at(&install_path))
}

#[cfg(target_os = "windows")]
fn shell_registration_is_missing() -> bool {
    !Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "$package = Get-AppxPackage -Name 'app.qzip.desktop.shell' -ErrorAction SilentlyContinue; if ($package -and ([version]$package.Version -ge [version]'1.0.0.5')) { exit 0 } else { exit 1 }",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .is_ok_and(|status| status.success())
}

#[cfg(target_os = "windows")]
fn retry_shell_registration_after_launch() {
    if !shell_package_is_included() {
        return;
    }
    if !shell_registration_is_missing() {
        return;
    }
    let Ok(executable) = std::env::current_exe() else {
        return;
    };
    let Some(install_path) = executable.parent().map(Path::to_path_buf) else {
        return;
    };
    let script = install_path
        .join("qzip-shell")
        .join("Register-QZipShell.ps1");
    let package = install_path.join("qzip-shell").join("QZip.Shell.msix");
    if !script.is_file() || !package.is_file() {
        return;
    }
    std::thread::spawn(move || {
        let _ = Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
            ])
            .arg(script)
            .arg("-InstallPath")
            .arg(install_path)
            .arg("-PackagePath")
            .arg(package)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW)
            .status();
    });
}

#[cfg(not(target_os = "windows"))]
fn retry_shell_registration_after_launch() {}

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
        dialog = dialog.add_filter("压缩包", &["7z", "zip", "rar", "tar", "gz", "xz", "bz2"]);
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
    let modern_context_menu_available = shell_package_is_included();
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
fn check_for_updates() -> UpdateCheckResult {
    if cfg!(feature = "official-updater") {
        UpdateCheckResult {
            configured: true,
            status: "ready",
        }
    } else {
        UpdateCheckResult {
            configured: false,
            status: "unconfigured",
        }
    }
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
    take_pending_shell_request_from_root(&shell_request_root()?, state.shell_request_not_before)
}

fn preview_cache_root() -> PathBuf {
    std::env::temp_dir().join("QZip").join("preview")
}

fn cleanup_stale_preview_cache() {
    let root = preview_cache_root();
    let Ok(entries) = std::fs::read_dir(&root) else {
        return;
    };
    for entry in entries.flatten() {
        let stale = entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .ok()
            .and_then(|modified| SystemTime::now().duration_since(modified).ok())
            .is_some_and(|age| age >= PREVIEW_CACHE_TTL);
        if stale {
            let path = entry.path();
            if path.is_dir() {
                let _ = std::fs::remove_dir_all(path);
            } else {
                let _ = std::fs::remove_file(path);
            }
        }
    }
}

fn preview_extension_is_blocked(path: &Path) -> bool {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    matches!(
        extension.as_str(),
        "app"
            | "appx"
            | "appxbundle"
            | "bat"
            | "cmd"
            | "com"
            | "command"
            | "cpl"
            | "desktop"
            | "dmg"
            | "exe"
            | "hta"
            | "jar"
            | "js"
            | "jse"
            | "lnk"
            | "msi"
            | "msix"
            | "msixbundle"
            | "msp"
            | "pkg"
            | "ps1"
            | "psm1"
            | "reg"
            | "scr"
            | "sh"
            | "url"
            | "vbe"
            | "vbs"
            | "wsf"
            | "wsh"
    )
}

fn validate_preview_entry(entry: &ArchiveEntry) -> Result<PathBuf, CommandErrorDto> {
    if entry.is_directory {
        return Err(CommandErrorDto {
            code: "INVALID_REQUEST".into(),
            message: "文件夹不能作为文件打开".into(),
            recoverable: true,
        });
    }
    if entry.is_symlink || entry.is_hardlink {
        return Err(CommandErrorDto {
            code: "UNSAFE_PATH".into(),
            message: "出于安全考虑，链接文件不能直接打开，请先解压后检查".into(),
            recoverable: true,
        });
    }
    if entry.size > PREVIEW_MAX_FILE_SIZE {
        return Err(CommandErrorDto {
            code: "ARCHIVE_BOMB_RISK".into(),
            message: "文件超过 1 GB，请先解压到磁盘后再打开".into(),
            recoverable: true,
        });
    }
    let relative =
        archive_security::safe_relative_path(&entry.path).map_err(|error| CommandErrorDto {
            code: "UNSAFE_PATH".into(),
            message: format!("文件路径不安全：{error}"),
            recoverable: false,
        })?;
    if preview_extension_is_blocked(&relative) {
        return Err(CommandErrorDto {
            code: "ACCESS_DENIED".into(),
            message: "出于安全考虑，可执行文件或脚本不能从压缩包内直接运行，请先解压后检查".into(),
            recoverable: true,
        });
    }
    Ok(relative)
}

#[cfg(test)]
mod preview_entry_tests {
    use super::*;

    fn file(path: &str) -> ArchiveEntry {
        ArchiveEntry {
            path: path.into(),
            display_name: path.rsplit('/').next().unwrap_or(path).into(),
            size: 1024,
            compressed_size: None,
            is_directory: false,
            modified_at: None,
            crc: None,
            attributes: None,
            encrypted: false,
            is_symlink: false,
            is_hardlink: false,
        }
    }

    #[test]
    fn preview_accepts_safe_document_paths() {
        let entry = file("资料/项目说明.docx");
        assert_eq!(
            validate_preview_entry(&entry).unwrap(),
            PathBuf::from("资料").join("项目说明.docx")
        );
    }

    #[test]
    fn preview_rejects_traversal_links_and_executables() {
        assert_eq!(
            validate_preview_entry(&file("../escape.txt"))
                .unwrap_err()
                .code,
            "UNSAFE_PATH"
        );
        assert_eq!(
            validate_preview_entry(&file("tools/setup.EXE"))
                .unwrap_err()
                .code,
            "ACCESS_DENIED"
        );
        let mut link = file("safe.txt");
        link.is_symlink = true;
        assert_eq!(
            validate_preview_entry(&link).unwrap_err().code,
            "UNSAFE_PATH"
        );
    }

    #[test]
    fn preview_rejects_oversized_files() {
        let mut entry = file("video.mp4");
        entry.size = PREVIEW_MAX_FILE_SIZE + 1;
        assert_eq!(
            validate_preview_entry(&entry).unwrap_err().code,
            "ARCHIVE_BOMB_RISK"
        );
    }
}

fn launch_default_application(path: &Path) -> std::io::Result<()> {
    #[cfg(target_os = "windows")]
    let mut command = Command::new("explorer.exe");
    #[cfg(target_os = "macos")]
    let mut command = Command::new("open");
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = Command::new("xdg-open");
    command.arg(path).spawn().map(|_| ())
}

#[tauri::command]
async fn open_archive_entry(
    session_id: String,
    entry_path: String,
    state: State<'_, AppState>,
) -> Result<(), CommandErrorDto> {
    let (archive, password, entry) = {
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
        let entry = session
            .entries
            .iter()
            .find(|entry| entry.path == entry_path)
            .cloned()
            .ok_or_else(|| CommandErrorDto {
                code: "INVALID_REQUEST".into(),
                message: "压缩包内未找到该文件".into(),
                recoverable: true,
            })?;
        (session.archive.clone(), session.password.clone(), entry)
    };

    let _relative_path = validate_preview_entry(&entry)?;
    let preview_root = preview_cache_root().join(Uuid::new_v4().to_string());
    std::fs::create_dir_all(&preview_root).map_err(|_| CommandErrorDto {
        code: "ACCESS_DENIED".into(),
        message: "无法创建临时预览目录".into(),
        recoverable: true,
    })?;
    let extracted_path =
        output_path(&preview_root, &entry.path).map_err(|error| CommandErrorDto {
            code: "UNSAFE_PATH".into(),
            message: format!("文件路径不安全：{error}"),
            recoverable: false,
        })?;

    let result = state
        .backend
        .extract(
            ExtractArchiveRequest {
                archive,
                output: preview_root.clone(),
                selected_entries: Some(vec![entry.path]),
                conflict_policy: ConflictPolicy::Overwrite,
                password,
            },
            Arc::new(NoopProgressReporter),
            tokio_util::sync::CancellationToken::new(),
        )
        .await;
    if let Err(error) = result {
        let _ = std::fs::remove_dir_all(&preview_root);
        return Err(CommandErrorDto::from(error));
    }
    if !extracted_path.is_file() {
        let _ = std::fs::remove_dir_all(&preview_root);
        return Err(CommandErrorDto {
            code: "UNKNOWN".into(),
            message: "文件已解压，但未能定位临时预览文件".into(),
            recoverable: true,
        });
    }
    launch_default_application(&extracted_path).map_err(|_| CommandErrorDto {
        code: "ACCESS_DENIED".into(),
        message: "无法调用系统关联应用打开文件".into(),
        recoverable: true,
    })
}

#[tauri::command]
fn open_path(path: PathBuf) -> Result<(), CommandErrorDto> {
    launch_default_application(&path).map_err(|_| CommandErrorDto {
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

fn normalize_system_icon_extension(value: &str) -> Option<String> {
    let extension = value.trim().trim_start_matches('.').to_ascii_lowercase();
    (!extension.is_empty()
        && extension.len() <= 32
        && extension.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '+')
        }))
    .then_some(extension)
}

#[cfg(target_os = "windows")]
fn system_icon_data_url(extension: &str) -> Option<String> {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use windows_sys::Win32::{
        Storage::FileSystem::FILE_ATTRIBUTE_NORMAL,
        UI::Shell::{SHFILEINFOW, SHGFI_ICON, SHGFI_USEFILEATTRIBUTES, SHGetFileInfoW},
    };

    let path = format!("qzip.{extension}");
    let wide_path = path
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let mut file_info = SHFILEINFOW::default();
    let result = unsafe {
        SHGetFileInfoW(
            wide_path.as_ptr(),
            FILE_ATTRIBUTE_NORMAL,
            &mut file_info,
            std::mem::size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_USEFILEATTRIBUTES,
        )
    };
    if result == 0 || file_info.hIcon.is_null() {
        return None;
    }

    let png = unsafe { encode_windows_icon_as_png(file_info.hIcon) };
    unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::DestroyIcon(file_info.hIcon);
    }
    png.map(|bytes| format!("data:image/png;base64,{}", STANDARD.encode(bytes)))
}

#[cfg(target_os = "windows")]
unsafe fn encode_windows_icon_as_png(
    icon: windows_sys::Win32::UI::WindowsAndMessaging::HICON,
) -> Option<Vec<u8>> {
    use image::ImageEncoder as _;
    use windows_sys::Win32::Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS,
        DeleteDC, DeleteObject, SelectObject,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{DI_NORMAL, DrawIconEx};

    const ICON_SIZE: usize = 32;
    let dc = unsafe { CreateCompatibleDC(std::ptr::null_mut()) };
    if dc.is_null() {
        return None;
    }

    let mut bitmap_info = BITMAPINFO::default();
    bitmap_info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
    bitmap_info.bmiHeader.biWidth = ICON_SIZE as i32;
    bitmap_info.bmiHeader.biHeight = -(ICON_SIZE as i32);
    bitmap_info.bmiHeader.biPlanes = 1;
    bitmap_info.bmiHeader.biBitCount = 32;
    bitmap_info.bmiHeader.biCompression = BI_RGB;
    let mut bits = std::ptr::null_mut();
    let bitmap = unsafe {
        CreateDIBSection(
            dc,
            &bitmap_info,
            DIB_RGB_COLORS,
            &mut bits,
            std::ptr::null_mut(),
            0,
        )
    };
    if bitmap.is_null() || bits.is_null() {
        unsafe { DeleteDC(dc) };
        return None;
    }

    let previous = unsafe { SelectObject(dc, bitmap) };
    let byte_count = ICON_SIZE * ICON_SIZE * 4;
    unsafe { std::ptr::write_bytes(bits, 0, byte_count) };
    let drawn = unsafe {
        DrawIconEx(
            dc,
            0,
            0,
            icon,
            ICON_SIZE as i32,
            ICON_SIZE as i32,
            0,
            std::ptr::null_mut(),
            DI_NORMAL,
        )
    } != 0;
    let bgra = unsafe { std::slice::from_raw_parts(bits.cast::<u8>(), byte_count) }.to_vec();
    if !previous.is_null() {
        unsafe { SelectObject(dc, previous) };
    }
    unsafe {
        DeleteObject(bitmap);
        DeleteDC(dc);
    }
    if !drawn {
        return None;
    }

    let has_alpha = bgra.chunks_exact(4).any(|pixel| pixel[3] != 0);
    let mut rgba = Vec::with_capacity(byte_count);
    for pixel in bgra.chunks_exact(4) {
        let alpha = if has_alpha {
            pixel[3]
        } else if pixel[0] != 0 || pixel[1] != 0 || pixel[2] != 0 {
            u8::MAX
        } else {
            0
        };
        let expand = |channel: u8| {
            if has_alpha && alpha > 0 && alpha < u8::MAX {
                ((u16::from(channel) * 255 / u16::from(alpha)).min(255)) as u8
            } else {
                channel
            }
        };
        rgba.extend_from_slice(&[expand(pixel[2]), expand(pixel[1]), expand(pixel[0]), alpha]);
    }

    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(&mut png)
        .write_image(
            &rgba,
            ICON_SIZE as u32,
            ICON_SIZE as u32,
            image::ExtendedColorType::Rgba8,
        )
        .ok()?;
    Some(png)
}

#[cfg(not(target_os = "windows"))]
fn system_icon_data_url(_extension: &str) -> Option<String> {
    None
}

#[tauri::command]
async fn get_system_file_icons(extensions: Vec<String>) -> HashMap<String, String> {
    tokio::task::spawn_blocking(move || {
        let mut seen = HashSet::new();
        let mut icons = HashMap::new();
        for extension in extensions
            .into_iter()
            .filter_map(|value| normalize_system_icon_extension(&value))
        {
            if !seen.insert(extension.clone()) {
                continue;
            }
            if seen.len() > 128 {
                break;
            }
            if let Some(icon) = system_icon_data_url(&extension) {
                icons.insert(extension, icon);
            }
        }
        icons
    })
    .await
    .unwrap_or_default()
}

#[cfg(test)]
mod system_file_icon_tests {
    use super::{normalize_system_icon_extension, system_icon_data_url};

    #[test]
    fn normalizes_safe_file_extensions() {
        assert_eq!(
            normalize_system_icon_extension(" .DOCX "),
            Some("docx".into())
        );
        assert_eq!(
            normalize_system_icon_extension("tar-gz"),
            Some("tar-gz".into())
        );
    }

    #[test]
    fn rejects_paths_and_unbounded_extensions() {
        assert_eq!(normalize_system_icon_extension("../exe"), None);
        assert_eq!(normalize_system_icon_extension(&"x".repeat(33)), None);
        assert_eq!(normalize_system_icon_extension(""), None);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn renders_a_shell_file_icon_as_png_data() {
        let icon = system_icon_data_url("txt").expect("Windows supplies a text-file icon");
        assert!(icon.starts_with("data:image/png;base64,iVBORw0KGgo"));
    }
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
            if let Some(request) = launch_request_from_args(&args) {
                let _ = app.emit("qzip://launch-request", request);
            }
        }))
        .setup(|app| {
            cleanup_stale_preview_cache();
            retry_shell_registration_after_launch();
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
            let settings = load_settings(app.handle());
            let initial_launch_request =
                launch_request_from_args(&std::env::args().collect::<Vec<_>>());
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
            open_archive_entry,
            get_system_file_icons,
            open_path,
            reveal_in_file_manager,
            record_performance_marker
        ])
        .run(tauri::generate_context!())
        .expect("failed to run QZip desktop application");
}
