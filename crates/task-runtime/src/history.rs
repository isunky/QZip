use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::Mutex,
};

use uuid::Uuid;

use crate::TaskSnapshot;

pub(crate) const HISTORY_LIMIT: usize = 100;

pub(crate) struct TaskHistory {
    path: PathBuf,
    write_lock: Mutex<()>,
}

impl TaskHistory {
    pub(crate) fn new(path: PathBuf) -> Self {
        recover_history_file(&path);
        Self {
            path,
            write_lock: Mutex::new(()),
        }
    }

    pub(crate) fn load(&self) -> Vec<TaskSnapshot> {
        let Ok(bytes) = fs::read(&self.path) else {
            return vec![];
        };
        let Ok(mut history) = serde_json::from_slice::<Vec<TaskSnapshot>>(&bytes) else {
            return vec![];
        };
        sort_history(&mut history);
        history.truncate(HISTORY_LIMIT);
        history
    }

    pub(crate) fn persist(&self, snapshots: &[TaskSnapshot]) {
        // Task completions can arrive on different worker threads. Serialize
        // the snapshot, durable write, and replacement as one critical section.
        let _write_guard = self.write_lock.lock().expect("history write lock");
        let mut history = snapshots.to_vec();
        sort_history(&mut history);
        history.truncate(HISTORY_LIMIT);

        let result = (|| -> io::Result<()> {
            if let Some(parent) = self.path.parent() {
                fs::create_dir_all(parent)?;
            }
            let temporary = temporary_history_path(&self.path);
            let write_result = (|| -> io::Result<()> {
                let mut file = fs::File::create(&temporary)?;
                serde_json::to_writer(&mut file, &history).map_err(io::Error::other)?;
                file.sync_all()?;
                replace_history_file(&temporary, &self.path)?;
                Ok(())
            })();
            if write_result.is_err() {
                let _ = fs::remove_file(&temporary);
            }
            write_result
        })();
        if let Err(error) = result {
            eprintln!("任务历史写入失败（{}）：{}", self.path.display(), error);
        }
    }
}

fn sort_history(history: &mut [TaskSnapshot]) {
    history.sort_by(|left, right| {
        right
            .updated_at
            .cmp(&left.updated_at)
            .then_with(|| right.created_at.cmp(&left.created_at))
            .then_with(|| right.task_id.cmp(&left.task_id))
    });
}

fn temporary_history_path(history_path: &Path) -> PathBuf {
    let extension = history_path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("json");
    history_path.with_extension(format!("{extension}.{}.tmp", Uuid::new_v4()))
}

fn history_backup_path(history_path: &Path) -> PathBuf {
    let extension = history_path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("json");
    history_path.with_extension(format!("{extension}.bak"))
}

#[cfg(not(windows))]
fn recover_history_file(_: &Path) {}

#[cfg(windows)]
fn recover_history_file(history_path: &Path) {
    let backup = history_backup_path(history_path);
    match (history_path.exists(), backup.exists()) {
        (false, true) => {
            if let Err(error) = fs::rename(&backup, history_path) {
                eprintln!("任务历史恢复失败（{}）：{}", history_path.display(), error);
            }
        }
        (true, true) => {
            if let Err(error) = fs::remove_file(&backup) {
                eprintln!("任务历史备份清理失败（{}）：{}", backup.display(), error);
            }
        }
        _ => {}
    }
}

#[cfg(not(windows))]
fn replace_history_file(temporary: &Path, history_path: &Path) -> io::Result<()> {
    // POSIX rename atomically replaces the destination while leaving the old
    // file intact if the write or rename fails.
    fs::rename(temporary, history_path)
}

#[cfg(windows)]
fn replace_history_file(temporary: &Path, history_path: &Path) -> io::Result<()> {
    // Windows `rename` does not replace an existing file. Keep a backup until
    // the new file is in place and restore it if the second move fails.
    let backup = history_backup_path(history_path);
    if backup.exists() {
        fs::remove_file(&backup)?;
    }
    let had_existing = history_path.exists();
    if had_existing {
        fs::rename(history_path, &backup)?;
    }
    match fs::rename(temporary, history_path) {
        Ok(()) => {
            if had_existing {
                let _ = fs::remove_file(backup);
            }
            Ok(())
        }
        Err(error) => {
            if had_existing && let Err(restore_error) = fs::rename(&backup, history_path) {
                return Err(io::Error::other(format!(
                    "替换任务历史失败：{error}；恢复原文件失败：{restore_error}"
                )));
            }
            Err(error)
        }
    }
}
