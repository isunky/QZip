use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::SystemTime,
};

use platform_integration::{LaunchKind, LaunchRequest};
use serde::Deserialize;
use uuid::Uuid;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
use super::CREATE_NO_WINDOW;

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

pub(super) fn shell_request_root() -> Option<PathBuf> {
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

pub(super) fn take_pending_shell_request_from_root(
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

pub(super) fn launch_request_from_args(args: &[String]) -> Option<LaunchRequest> {
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

#[cfg(target_os = "windows")]
pub(super) fn shell_package_is_included_at(install_path: &Path) -> bool {
    install_path
        .join("qzip-shell")
        .join("QZip.Shell.msix")
        .is_file()
}

#[cfg(target_os = "windows")]
pub(super) fn shell_package_is_included() -> bool {
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
pub(super) fn retry_shell_registration_after_launch() {
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
pub(super) fn retry_shell_registration_after_launch() {}

#[cfg(test)]
mod tests {
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
