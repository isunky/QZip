use platform_integration::LaunchRequest;
use serde::Serialize;
use std::{collections::VecDeque, sync::Mutex};
#[cfg(target_os = "macos")]
use tauri::Manager;
use tauri::State;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NativeLaunch {
    id: String,
    request: LaunchRequest,
}

#[derive(Default)]
pub(crate) struct LaunchInbox(Mutex<VecDeque<NativeLaunch>>);

impl LaunchInbox {
    fn peek(&self) -> Option<NativeLaunch> {
        self.0.lock().expect("native launch inbox").front().cloned()
    }
    fn acknowledge(&self, id: &str) {
        let mut queue = self.0.lock().expect("native launch inbox");
        if queue.front().is_some_and(|item| item.id == id) {
            queue.pop_front();
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlatformCapabilities {
    os: &'static str,
    arch: &'static str,
    native_window_controls: bool,
    windows_shell: bool,
    in_app_update: bool,
}

#[tauri::command]
pub(crate) fn get_platform_capabilities() -> PlatformCapabilities {
    PlatformCapabilities {
        os: std::env::consts::OS,
        arch: std::env::consts::ARCH,
        native_window_controls: cfg!(target_os = "macos"),
        windows_shell: cfg!(target_os = "windows"),
        in_app_update: cfg!(all(target_os = "windows", target_arch = "x86_64")),
    }
}

#[tauri::command]
pub(crate) fn pending_native_launch(inbox: State<'_, LaunchInbox>) -> Option<NativeLaunch> {
    inbox.peek()
}

#[tauri::command]
pub(crate) fn acknowledge_native_launch(id: String, inbox: State<'_, LaunchInbox>) {
    inbox.acknowledge(&id);
}

pub(crate) fn on_event(_app: &tauri::AppHandle, _event: tauri::RunEvent) {
    #[cfg(target_os = "macos")]
    match _event {
        tauri::RunEvent::Opened { urls } => {
            let paths: Vec<_> = urls
                .into_iter()
                .filter_map(|url| url.to_file_path().ok())
                .collect();
            if !paths.is_empty() {
                _app.state::<LaunchInbox>()
                    .0
                    .lock()
                    .expect("native launch inbox")
                    .push_back(NativeLaunch {
                        id: uuid::Uuid::new_v4().to_string(),
                        request: LaunchRequest {
                            kind: platform_integration::LaunchKind::Open,
                            paths,
                            source: "finder".into(),
                        },
                    });
            }
            show_main(_app);
        }
        tauri::RunEvent::Reopen { .. } => show_main(_app),
        tauri::RunEvent::ExitRequested { api, .. } => {
            let tasks = _app.state::<super::AppState>().tasks.clone();
            if tasks.snapshots().iter().any(|task| active(task.status)) {
                api.prevent_exit();
                request_quit(_app.clone(), tasks);
            }
        }
        tauri::RunEvent::WindowEvent {
            event: tauri::WindowEvent::CloseRequested { api, .. },
            ..
        } => {
            api.prevent_close();
            if let Some(window) = _app.get_webview_window("main") {
                let _ = window.hide();
            }
        }
        _ => {}
    }
}

#[cfg(target_os = "macos")]
fn active(status: archive_core::TaskStatus) -> bool {
    use archive_core::TaskStatus::*;
    matches!(status, Queued | Scanning | Running | Cancelling)
}

#[cfg(target_os = "macos")]
fn request_quit(app: tauri::AppHandle, tasks: std::sync::Arc<task_runtime::TaskManager>) {
    use std::sync::atomic::{AtomicBool, Ordering};
    static QUITTING: AtomicBool = AtomicBool::new(false);
    if QUITTING.swap(true, Ordering::SeqCst) {
        return;
    }
    tauri::async_runtime::spawn(async move {
        let result = rfd::AsyncMessageDialog::new()
            .set_title("QZip")
            .set_description(
                "仍有任务正在运行。取消任务并退出？\nTasks are running. Cancel them and quit?",
            )
            .set_buttons(rfd::MessageButtons::OkCancel)
            .show()
            .await;
        if result == rfd::MessageDialogResult::Ok {
            for task in tasks.snapshots() {
                if active(task.status) {
                    let _ = tasks.cancel(&task.task_id);
                }
            }
            // Wait for transactional cleanup; never kill a worker while it restores an original file.
            while tasks.snapshots().iter().any(|task| active(task.status)) {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
            app.exit(0);
        }
        QUITTING.store(false, Ordering::SeqCst);
    });
}

#[cfg(target_os = "macos")]
pub(crate) fn install_menu(app: &tauri::AppHandle) -> tauri::Result<()> {
    use tauri::{
        Emitter,
        menu::{Menu, MenuItem, Submenu},
    };
    let menu = Menu::default(app)?;
    let open = MenuItem::with_id(
        app,
        "qzip-open",
        "打开压缩包 / Open…",
        true,
        Some("CmdOrCtrl+O"),
    )?;
    let settings = MenuItem::with_id(
        app,
        "qzip-settings",
        "设置 / Settings…",
        true,
        Some("CmdOrCtrl+,"),
    )?;
    menu.insert(
        &Submenu::with_items(app, "文件 / File", true, &[&open, &settings])?,
        1,
    )?;
    app.set_menu(menu)?;
    app.on_menu_event(|app, event| match event.id().as_ref() {
        "qzip-settings" => {
            show_main(app);
            let _ = app.emit("qzip://menu-settings", ());
        }
        "qzip-open" => {
            show_main(app);
            if let Some(paths) = rfd::FileDialog::new()
                .add_filter(
                    "Archives",
                    &["zip", "7z", "rar", "tar", "gz", "xz", "tgz", "txz", "bz2"],
                )
                .pick_files()
            {
                app.state::<LaunchInbox>()
                    .0
                    .lock()
                    .expect("native launch inbox")
                    .push_back(NativeLaunch {
                        id: uuid::Uuid::new_v4().to_string(),
                        request: LaunchRequest {
                            kind: platform_integration::LaunchKind::Open,
                            paths,
                            source: "menu".into(),
                        },
                    });
            }
        }
        _ => {}
    });
    Ok(())
}

#[cfg(target_os = "macos")]
fn show_main(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn launch_requests_wait_for_matching_acknowledgement_in_order() {
        let inbox = LaunchInbox::default();
        for id in ["first", "second"] {
            inbox.0.lock().unwrap().push_back(NativeLaunch {
                id: id.into(),
                request: LaunchRequest {
                    kind: platform_integration::LaunchKind::Open,
                    paths: vec!["/tmp/中文 文件.zip".into()],
                    source: "finder".into(),
                },
            });
        }
        assert_eq!(inbox.peek().unwrap().id, "first");
        assert_eq!(inbox.peek().unwrap().id, "first");
        inbox.acknowledge("second");
        assert_eq!(inbox.peek().unwrap().id, "first");
        inbox.acknowledge("first");
        inbox.acknowledge("first");
        assert_eq!(inbox.peek().unwrap().id, "second");
        inbox.acknowledge("second");
        assert!(inbox.peek().is_none());
    }
}
