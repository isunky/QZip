use std::{path::PathBuf, sync::Mutex, time::Instant};

use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Manager, State, ipc::Channel};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::{AppState, CommandErrorDto, updates::*};

#[derive(Default)]
pub(super) struct UpdateDownloads {
    active: Mutex<Option<CancellationToken>>,
    verified: Mutex<Option<VerifiedInstaller>>,
}

#[derive(Clone)]
struct VerifiedInstaller {
    token: String,
    path: PathBuf,
    hash: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DownloadProgress {
    phase: &'static str,
    downloaded: u64,
    total: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DownloadedUpdate {
    token: String,
    version: String,
}

fn checksum_for(text: &str, name: &str) -> Result<String, CommandErrorDto> {
    let matches: Vec<_> = text
        .lines()
        .filter_map(|line| {
            let (hash, file) = line.trim().split_once(char::is_whitespace)?;
            let file = file.trim_start().trim_start_matches('*');
            (file == name && hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()))
                .then(|| hash.to_ascii_lowercase())
        })
        .collect();
    if matches.len() != 1 {
        return Err(update_check_error(
            "UPDATE_CHECKSUM_INVALID",
            "安装包校验信息无效，下载已停止。",
        ));
    }
    Ok(matches[0].clone())
}

async fn file_hash(path: &std::path::Path) -> Result<String, CommandErrorDto> {
    let mut file = tokio::fs::File::open(path).await.map_err(io_error)?;
    let mut hash = Sha256::new();
    let mut buffer = vec![0; 64 * 1024];
    loop {
        let size = file.read(&mut buffer).await.map_err(io_error)?;
        if size == 0 {
            break;
        }
        hash.update(&buffer[..size]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

fn io_error(_: std::io::Error) -> CommandErrorDto {
    update_check_error(
        "UPDATE_FILE",
        "无法保存或读取安装包，请检查磁盘空间和访问权限。",
    )
}

async fn transfer(
    tag: &str,
    part: &std::path::Path,
    on_progress: &Channel<DownloadProgress>,
) -> Result<String, CommandErrorDto> {
    let release = fetch_release(Some(tag)).await?;
    if release.tag_name != tag {
        return Err(update_check_error(
            "UPDATE_INVALID_RELEASE",
            "版本信息发生变化，请重新检查更新。",
        ));
    }
    let (setup, checksums) = release_download_assets(&release)?;
    let client = http_client()?;
    let checksum_data = read_limited(
        get_response(&client, &checksums.browser_download_url, true).await?,
        64 * 1024,
    )
    .await?;
    let checksum_text = std::str::from_utf8(&checksum_data)
        .map_err(|_| update_check_error("UPDATE_CHECKSUM_INVALID", "校验文件格式无效。"))?;
    let expected_hash = checksum_for(checksum_text, &setup.name)?;
    let mut response = get_response(&client, &setup.browser_download_url, false).await?;
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(part)
        .await
        .map_err(io_error)?;
    let mut downloaded = 0;
    let mut last_progress = Instant::now();
    let _ = on_progress.send(DownloadProgress {
        phase: "downloading",
        downloaded,
        total: setup.size,
    });
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| update_check_error("UPDATE_NETWORK", "下载中断，请重试。"))?
    {
        downloaded += chunk.len() as u64;
        if downloaded > setup.size {
            return Err(update_check_error(
                "UPDATE_SIZE",
                "安装包大小与发布信息不一致。",
            ));
        }
        file.write_all(&chunk).await.map_err(io_error)?;
        if last_progress.elapsed().as_millis() >= 100 {
            let _ = on_progress.send(DownloadProgress {
                phase: "downloading",
                downloaded,
                total: setup.size,
            });
            last_progress = Instant::now();
        }
    }
    file.flush().await.map_err(io_error)?;
    file.sync_all().await.map_err(io_error)?;
    drop(file);
    if downloaded != setup.size {
        return Err(update_check_error(
            "UPDATE_SIZE",
            "安装包未完整下载，请重试。",
        ));
    }
    let _ = on_progress.send(DownloadProgress {
        phase: "verifying",
        downloaded,
        total: setup.size,
    });
    if file_hash(part).await? != expected_hash {
        return Err(update_check_error(
            "UPDATE_CHECKSUM_MISMATCH",
            "安装包校验失败，请重新下载。",
        ));
    }
    Ok(expected_hash)
}

#[tauri::command]
pub(super) async fn download_update(
    tag: String,
    on_progress: Channel<DownloadProgress>,
    app: AppHandle,
    downloads: State<'_, UpdateDownloads>,
) -> Result<DownloadedUpdate, CommandErrorDto> {
    if !cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        return Err(update_check_error(
            "UPDATE_PLATFORM",
            "当前平台不支持此安装包。",
        ));
    }
    if !valid_release_tag(&tag) {
        return Err(update_check_error("UPDATE_INVALID_RELEASE", "版本号无效。"));
    }
    let root = app
        .path()
        .app_cache_dir()
        .map_err(|_| update_check_error("UPDATE_FILE", "无法访问更新缓存。"))?
        .join("updates");
    let cancellation = CancellationToken::new();
    {
        let mut active = downloads.active.lock().expect("update lock");
        if active.is_some() {
            return Err(update_check_error(
                "UPDATE_BUSY",
                "已有更新正在下载，请等待或取消。",
            ));
        }
        *active = Some(cancellation.clone());
    }
    let token = Uuid::new_v4().to_string();
    let part = root.join(format!("{token}.part"));
    let path = root.join(format!("QZip-{token}-setup.exe"));
    let result = tokio::select! {
        biased;
        _ = cancellation.cancelled() => Err(update_check_error("UPDATE_CANCELLED", "下载已取消。")),
        result = async {
            tokio::fs::create_dir_all(&root).await.map_err(io_error)?;
            transfer(&tag, &part, &on_progress).await
        } => result,
    };
    let result = match result {
        Ok(hash) if !cancellation.is_cancelled() => {
            match tokio::fs::rename(&part, &path).await {
                Ok(()) => {
                    let previous = downloads.verified.lock().expect("update lock").replace(
                        VerifiedInstaller {
                            token: token.clone(),
                            path,
                            hash,
                        },
                    );
                    if let Some(previous) = previous {
                        let _ = tokio::fs::remove_file(previous.path).await;
                    }
                    Ok(DownloadedUpdate {
                        token,
                        version: tag,
                    })
                }
                Err(error) => Err(io_error(error)),
            }
        }
        Ok(_) => Err(update_check_error("UPDATE_CANCELLED", "下载已取消。")),
        Err(error) => Err(error),
    };
    let _ = tokio::fs::remove_file(&part).await;
    *downloads.active.lock().expect("update lock") = None;
    result
}

#[tauri::command]
pub(super) fn cancel_update_download(downloads: State<'_, UpdateDownloads>) {
    if let Some(token) = downloads.active.lock().expect("update lock").as_ref() {
        token.cancel();
    }
}

#[tauri::command]
pub(super) async fn install_update(
    token: String,
    downloads: State<'_, UpdateDownloads>,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<(), CommandErrorDto> {
    let installer = downloads
        .verified
        .lock()
        .expect("update lock")
        .clone()
        .filter(|item| item.token == token)
        .ok_or_else(|| update_check_error("UPDATE_NOT_READY", "请先下载并校验安装包。"))?;
    if file_hash(&installer.path).await? != installer.hash {
        return Err(update_check_error(
            "UPDATE_CHECKSUM_MISMATCH",
            "安装包已变化，请重新下载。",
        ));
    }
    if state.tasks.snapshots().iter().any(|task| {
        matches!(
            task.status,
            archive_core::TaskStatus::Queued
                | archive_core::TaskStatus::Scanning
                | archive_core::TaskStatus::Running
                | archive_core::TaskStatus::Cancelling
        )
    }) {
        return Err(update_check_error(
            "UPDATE_TASKS_ACTIVE",
            "请等待当前压缩任务完成后再安装更新。",
        ));
    }
    launch_installer(&installer.path)?;
    app.exit(0);
    Ok(())
}

fn launch_installer(path: &std::path::Path) -> Result<(), CommandErrorDto> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        // Use the Windows shell so installers requiring elevation can show UAC.
        // The path comes only from the verified native download state.
        let result = unsafe {
            windows_sys::Win32::UI::Shell::ShellExecuteW(
                std::ptr::null_mut(),
                std::ptr::null(),
                path.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL,
            )
        };
        if result as isize <= 32 {
            return Err(update_check_error(
                "UPDATE_INSTALL",
                "未能打开安装向导，可能已取消授权，请重试。",
            ));
        }
    }
    #[cfg(not(windows))]
    std::process::Command::new(path)
        .spawn()
        .map_err(|_| update_check_error("UPDATE_INSTALL", "无法打开安装向导，请重试。"))?;
    Ok(())
}

#[tauri::command]
pub(super) fn open_update_link(url: String) -> Result<(), CommandErrorDto> {
    let parsed =
        reqwest::Url::parse(&url).map_err(|_| update_check_error("UPDATE_LINK", "链接无效。"))?;
    if parsed.scheme() != "https"
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        return Err(update_check_error("UPDATE_LINK", "不支持此链接。"));
    }
    #[cfg(target_os = "windows")]
    let launcher = "explorer.exe";
    #[cfg(target_os = "macos")]
    let launcher = "open";
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let launcher = "xdg-open";
    std::process::Command::new(launcher)
        .arg(parsed.as_str())
        .spawn()
        .map(|_| ())
        .map_err(|_| update_check_error("UPDATE_LINK", "无法打开浏览器。"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    #[ignore = "Downloads the latest official GitHub installer; never executes it"]
    async fn github_download_passes_size_and_checksum_verification() {
        let release = fetch_release(None).await.expect("GitHub release");
        let root = std::env::temp_dir().join(format!("qzip-update-test-{}", Uuid::new_v4()));
        tokio::fs::create_dir(&root).await.unwrap();
        let part = root.join("installer.part");
        let result = transfer(&release.tag_name, &part, &Channel::new(|_| Ok(()))).await;
        let _ = tokio::fs::remove_file(&part).await;
        tokio::fs::remove_dir(&root).await.unwrap();
        assert!(result.is_ok(), "download verification failed: {result:?}");
    }

    #[test]
    fn selects_exact_checksum_and_rejects_missing_or_ambiguous_entries() {
        let hash = "a".repeat(64);
        assert_eq!(
            checksum_for(&format!("{hash} *setup.exe\r\n"), "setup.exe").unwrap(),
            hash
        );
        assert!(checksum_for(&format!("{hash} *other.exe"), "setup.exe").is_err());
        assert!(checksum_for("bad *setup.exe", "setup.exe").is_err());
        assert!(
            checksum_for(
                &format!("{hash} *setup.exe\n{hash} *setup.exe"),
                "setup.exe"
            )
            .is_err()
        );
    }
}
