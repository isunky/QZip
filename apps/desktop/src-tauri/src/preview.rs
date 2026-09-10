use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
};

use archive_core::{
    ArchiveBackend, ArchiveEntry, ConflictPolicy, ExtractArchiveRequest, NoopProgressReporter,
};
use archive_security::output_path;
use tauri::State;
use uuid::Uuid;

use super::{AppState, CommandErrorDto, archive_fingerprint};

const PREVIEW_CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(7 * 24 * 60 * 60);
const PREVIEW_MAX_FILE_SIZE: u64 = 1024 * 1024 * 1024;

pub(super) fn cleanup_stale_preview_cache() {
    let root = preview_cache_root();
    let Ok(entries) = std::fs::read_dir(&root) else {
        return;
    };
    for entry in entries.flatten() {
        let stale = entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .ok()
            .and_then(|modified| std::time::SystemTime::now().duration_since(modified).ok())
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

fn preview_cache_root() -> PathBuf {
    std::env::temp_dir().join("QZip").join("preview")
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
pub(super) async fn open_archive_entry(
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
pub(super) fn open_path(path: PathBuf) -> Result<(), CommandErrorDto> {
    launch_default_application(&path).map_err(|_| CommandErrorDto {
        code: "ACCESS_DENIED".into(),
        message: "无法打开目标位置".into(),
        recoverable: true,
    })
}

#[tauri::command]
pub(super) fn reveal_in_file_manager(path: PathBuf) -> Result<(), CommandErrorDto> {
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

#[cfg(test)]
mod tests {
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
