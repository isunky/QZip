use super::history::HISTORY_LIMIT;
use super::*;
use secrecy::ExposeSecret;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::Notify;

struct UnusedBackend;

#[derive(Clone, Copy)]
enum BackendMode {
    BlockTest,
    CancelCreate,
    PasswordExtract,
}

struct ScriptedState {
    mode: BackendMode,
    started: Notify,
    release: CancellationToken,
    active: AtomicUsize,
    peak: AtomicUsize,
}

struct ScriptedBackend {
    state: Arc<ScriptedState>,
}

impl ScriptedBackend {
    fn new(mode: BackendMode) -> (Arc<Self>, Arc<ScriptedState>) {
        let state = Arc::new(ScriptedState {
            mode,
            started: Notify::new(),
            release: CancellationToken::new(),
            active: AtomicUsize::new(0),
            peak: AtomicUsize::new(0),
        });
        (
            Arc::new(Self {
                state: state.clone(),
            }),
            state,
        )
    }

    async fn enter(&self) {
        let active = self.state.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.state.peak.fetch_max(active, Ordering::SeqCst);
        // Keep a permit when the backend starts before the test begins
        // awaiting the signal. `notify_waiters` would lose that event and
        // make cancellation tests intermittently time out on fast workers.
        self.state.started.notify_one();
    }

    fn leave(&self) {
        self.state.active.fetch_sub(1, Ordering::SeqCst);
    }

    async fn wait_for_release(&self, cancellation: CancellationToken) -> Result<(), ArchiveError> {
        tokio::select! {
            _ = self.state.release.cancelled() => Ok(()),
            _ = cancellation.cancelled() => Err(ArchiveError::new(ArchiveErrorCode::Cancelled, "任务已取消")),
        }
    }
}

#[async_trait::async_trait]
impl ArchiveBackend for ScriptedBackend {
    fn id(&self) -> &'static str {
        "scripted"
    }

    async fn capabilities(&self) -> Result<archive_core::BackendCapabilities, ArchiveError> {
        Err(ArchiveError::unavailable("not used by this test"))
    }

    async fn create(
        &self,
        request: CreateArchiveRequest,
        _: Arc<dyn ProgressReporter>,
        cancellation: CancellationToken,
    ) -> Result<ArchiveResult, ArchiveError> {
        self.enter().await;
        let result = match self.state.mode {
            BackendMode::CancelCreate => {
                fs::write(&request.output, b"partial archive").unwrap();
                self.wait_for_release(cancellation).await
            }
            _ => Ok(()),
        };
        self.leave();
        result.map(|_| ArchiveResult {
            output: Some(request.output),
            entries: vec![],
            warnings: vec![],
        })
    }

    async fn extract(
        &self,
        request: ExtractArchiveRequest,
        _: Arc<dyn ProgressReporter>,
        cancellation: CancellationToken,
    ) -> Result<ArchiveResult, ArchiveError> {
        self.enter().await;
        let result = if matches!(self.state.mode, BackendMode::PasswordExtract) {
            fs::create_dir_all(&request.output).unwrap();
            fs::write(request.output.join("file.txt"), b"extracted").unwrap();
            Ok(())
        } else {
            self.wait_for_release(cancellation).await
        };
        self.leave();
        result.map(|_| ArchiveResult {
            output: Some(request.output),
            entries: vec![],
            warnings: vec![],
        })
    }

    async fn list(
        &self,
        request: ListArchiveRequest,
        _: CancellationToken,
    ) -> Result<Vec<archive_core::ArchiveEntry>, ArchiveError> {
        if matches!(self.state.mode, BackendMode::PasswordExtract)
            && request
                .password
                .as_ref()
                .is_none_or(|password| password.expose_secret() != "correct")
        {
            return Err(ArchiveError::new(
                ArchiveErrorCode::WrongPassword,
                "密码错误",
            ));
        }
        Ok(vec![archive_core::ArchiveEntry {
            path: "file.txt".into(),
            display_name: "file.txt".into(),
            size: 9,
            compressed_size: None,
            is_directory: false,
            modified_at: None,
            crc: None,
            attributes: None,
            encrypted: false,
            is_symlink: false,
            is_hardlink: false,
        }])
    }

    async fn test(
        &self,
        _: TestArchiveRequest,
        cancellation: CancellationToken,
    ) -> Result<TestResult, ArchiveError> {
        self.enter().await;
        let result = match self.state.mode {
            BackendMode::BlockTest => self.wait_for_release(cancellation).await,
            _ => Ok(()),
        };
        self.leave();
        result.map(|_| TestResult {
            valid: true,
            warnings: vec![],
        })
    }
}

async fn wait_for_terminal(manager: &Arc<TaskManager>, task_id: &str) -> TaskSnapshot {
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            if let Some(snapshot) = manager
                .snapshots()
                .into_iter()
                .find(|snapshot| snapshot.task_id == task_id)
                && matches!(
                    snapshot.status,
                    TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled
                )
            {
                return snapshot;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("task did not reach a terminal status")
}

#[async_trait::async_trait]
impl ArchiveBackend for UnusedBackend {
    fn id(&self) -> &'static str {
        "unused"
    }

    async fn capabilities(&self) -> Result<archive_core::BackendCapabilities, ArchiveError> {
        Err(ArchiveError::unavailable("not used by this test"))
    }

    async fn create(
        &self,
        _: CreateArchiveRequest,
        _: Arc<dyn ProgressReporter>,
        _: CancellationToken,
    ) -> Result<ArchiveResult, ArchiveError> {
        Err(ArchiveError::unavailable("not used by this test"))
    }

    async fn extract(
        &self,
        _: ExtractArchiveRequest,
        _: Arc<dyn ProgressReporter>,
        _: CancellationToken,
    ) -> Result<ArchiveResult, ArchiveError> {
        Err(ArchiveError::unavailable("not used by this test"))
    }

    async fn list(
        &self,
        _: ListArchiveRequest,
        _: CancellationToken,
    ) -> Result<Vec<archive_core::ArchiveEntry>, ArchiveError> {
        Err(ArchiveError::unavailable("not used by this test"))
    }

    async fn test(
        &self,
        _: TestArchiveRequest,
        _: CancellationToken,
    ) -> Result<TestResult, ArchiveError> {
        Err(ArchiveError::unavailable("not used by this test"))
    }
}

fn test_directory(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("qzip-runtime-{label}-{}", Uuid::new_v4()));
    fs::create_dir_all(&path).unwrap();
    path
}

fn history_snapshot(task_id: impl Into<String>, updated_at: u64) -> TaskSnapshot {
    TaskSnapshot {
        task_id: task_id.into(),
        operation: ArchiveOperation::Test,
        status: TaskStatus::Completed,
        display_name: "archive.7z".into(),
        output: None,
        created_at: updated_at.saturating_sub(1),
        updated_at,
        progress: None,
        error: None,
        warnings: vec![],
        retryable: false,
    }
}

fn insert_history_snapshot(manager: &Arc<TaskManager>, snapshot: TaskSnapshot) {
    manager.tasks.lock().expect("task lock").insert(
        snapshot.task_id.clone(),
        TaskRecord {
            snapshot,
            spec: TaskSpec::Test {
                archive: PathBuf::new(),
            },
            cancellation: CancellationToken::new(),
            can_retry: false,
        },
    );
}

#[test]
fn refuses_an_archive_inside_an_input_directory() {
    let root = test_directory("recursive-output");
    let input = root.join("input");
    fs::create_dir(&input).unwrap();
    let error = validate_create_destination(std::slice::from_ref(&input), &input.join("result.7z"))
        .unwrap_err();
    assert_eq!(error.code, ArchiveErrorCode::InvalidRequest);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn submit_without_a_tokio_runtime_returns_an_error_instead_of_panicking() {
    let root = test_directory("missing-runtime");
    let manager = TaskManager::new(Arc::new(UnusedBackend), root.join("history.json"));
    let error = manager
        .submit(
            TaskSpec::Test {
                archive: root.join("archive.7z"),
            },
            None,
        )
        .unwrap_err();
    assert_eq!(error.code, ArchiveErrorCode::Unknown);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn history_keeps_the_most_recent_updated_tasks() {
    let root = test_directory("history-order");
    let history_path = root.join("history.json");
    let persisted = (0..(HISTORY_LIMIT + 20))
        .map(|index| history_snapshot(format!("task-{index:03}"), index as u64))
        .collect::<Vec<_>>();
    fs::write(&history_path, serde_json::to_vec(&persisted).unwrap()).unwrap();

    let manager = TaskManager::new(Arc::new(UnusedBackend), history_path.clone());
    let loaded = manager.snapshots();
    assert_eq!(loaded.len(), HISTORY_LIMIT);
    assert!(loaded.iter().all(|snapshot| snapshot.updated_at >= 20));

    manager.persist_history();
    let written =
        serde_json::from_slice::<Vec<TaskSnapshot>>(&fs::read(&history_path).unwrap()).unwrap();
    assert_eq!(written.len(), HISTORY_LIMIT);
    assert_eq!(
        written.first().map(|snapshot| snapshot.updated_at),
        Some(119)
    );
    assert_eq!(written.last().map(|snapshot| snapshot.updated_at), Some(20));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn concurrent_history_writes_leave_a_valid_snapshot() {
    let root = test_directory("history-concurrent");
    let history_path = root.join("history.json");
    let manager = TaskManager::new(Arc::new(UnusedBackend), history_path.clone());
    for index in 0..8 {
        insert_history_snapshot(&manager, history_snapshot(format!("task-{index}"), index));
    }

    std::thread::scope(|scope| {
        for _ in 0..16 {
            let manager = Arc::clone(&manager);
            scope.spawn(move || manager.persist_history());
        }
    });

    let written = serde_json::from_slice::<Vec<TaskSnapshot>>(&fs::read(&history_path).unwrap())
        .expect("concurrent history writes must leave valid JSON");
    assert_eq!(written.len(), 8);
    assert!(
        written
            .windows(2)
            .all(|pair| pair[0].updated_at >= pair[1].updated_at)
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn task_event_receiver_continues_after_buffer_overflow() {
    let root = test_directory("event-overflow");
    let manager = TaskManager::new(Arc::new(UnusedBackend), root.join("history.json"));
    let task_id = Uuid::new_v4().to_string();
    let snapshot = TaskSnapshot {
        task_id: task_id.clone(),
        operation: ArchiveOperation::Test,
        status: TaskStatus::Running,
        display_name: "archive.7z".into(),
        output: None,
        created_at: now(),
        updated_at: now(),
        progress: None,
        error: None,
        warnings: vec![],
        retryable: false,
    };
    manager.tasks.lock().expect("task lock").insert(
        task_id.clone(),
        TaskRecord {
            snapshot,
            spec: TaskSpec::Test {
                archive: root.join("archive.7z"),
            },
            cancellation: CancellationToken::new(),
            can_retry: true,
        },
    );
    let mut events = manager.subscribe();
    for _ in 0..(EVENT_CHANNEL_CAPACITY + 1) {
        manager.emit("task.progress", &task_id);
    }

    assert!(matches!(
        events.try_recv(),
        Err(broadcast::error::TryRecvError::Lagged(_))
    ));
    let event = events
        .try_recv()
        .expect("receiver remains usable after lag");
    assert_eq!(event.task.task_id, task_id);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn never_overwrites_an_existing_archive_on_commit() {
    let root = test_directory("existing-output");
    let temporary = root.join(".qzip-temporary.7z");
    let output = root.join("result.7z");
    fs::write(&temporary, "new").unwrap();
    fs::write(&output, "old").unwrap();
    let error = commit_created_archive(&temporary, &output).unwrap_err();
    assert_eq!(error.code, ArchiveErrorCode::ConflictRequiresDecision);
    assert_eq!(fs::read_to_string(&output).unwrap(), "old");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn extraction_rename_keeps_an_existing_file() {
    let root = test_directory("extract-rename");
    let staging = root.join(".qzip-extract-test");
    let output = root.join("output");
    fs::create_dir(&staging).unwrap();
    fs::create_dir(&output).unwrap();
    fs::write(staging.join("report.txt"), "new").unwrap();
    fs::write(output.join("report.txt"), "old").unwrap();
    commit_extraction(&staging, &output, ConflictPolicy::Rename).unwrap();
    assert_eq!(
        fs::read_to_string(output.join("report.txt")).unwrap(),
        "old"
    );
    assert_eq!(
        fs::read_to_string(output.join("report (1).txt")).unwrap(),
        "new"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn extraction_conflict_policies_are_explicit_and_non_silent() {
    for policy in [
        ConflictPolicy::Rename,
        ConflictPolicy::Overwrite,
        ConflictPolicy::Skip,
    ] {
        let root = test_directory("extract-conflict");
        let staging = root.join(".qzip-extract-policy");
        let output = root.join("output");
        fs::create_dir(&staging).unwrap();
        fs::create_dir(&output).unwrap();
        fs::write(staging.join("report.txt"), "new").unwrap();
        fs::write(output.join("report.txt"), "old").unwrap();
        commit_extraction(&staging, &output, policy).unwrap();
        match policy {
            ConflictPolicy::Rename => assert_eq!(
                fs::read_to_string(output.join("report (1).txt")).unwrap(),
                "new"
            ),
            ConflictPolicy::Overwrite => assert_eq!(
                fs::read_to_string(output.join("report.txt")).unwrap(),
                "new"
            ),
            ConflictPolicy::Skip => assert_eq!(
                fs::read_to_string(output.join("report.txt")).unwrap(),
                "old"
            ),
            ConflictPolicy::Ask => unreachable!(),
        }
        cleanup_staging(&staging).unwrap();
        let _ = fs::remove_dir_all(root);
    }
    let root = test_directory("extract-ask");
    let staging = root.join(".qzip-extract-ask");
    let output = root.join("output");
    fs::create_dir(&staging).unwrap();
    fs::create_dir(&output).unwrap();
    fs::write(staging.join("report.txt"), "new").unwrap();
    fs::write(output.join("report.txt"), "old").unwrap();
    let error = commit_extraction(&staging, &output, ConflictPolicy::Ask).unwrap_err();
    assert_eq!(error.code, ArchiveErrorCode::ConflictRequiresDecision);
    assert_eq!(
        fs::read_to_string(output.join("report.txt")).unwrap(),
        "old"
    );
    cleanup_staging(&staging).unwrap();
    let _ = fs::remove_dir_all(root);
}

#[test]
fn extraction_overwrite_rolls_back_when_a_later_conflict_blocks_commit() {
    let root = test_directory("extract-transaction");
    let staging = root.join(".qzip-extract-transaction");
    let output = root.join("output");
    fs::create_dir(&staging).unwrap();
    fs::create_dir(&output).unwrap();
    fs::write(staging.join("01-existing.txt"), "new").unwrap();
    fs::create_dir(staging.join("02-conflict")).unwrap();
    fs::write(staging.join("02-conflict").join("inside.txt"), "new nested").unwrap();
    fs::write(output.join("01-existing.txt"), "old").unwrap();
    fs::write(output.join("02-conflict"), "old conflict").unwrap();

    let error = commit_extraction(&staging, &output, ConflictPolicy::Overwrite).unwrap_err();
    assert_eq!(error.code, ArchiveErrorCode::ConflictRequiresDecision);
    assert_eq!(
        fs::read_to_string(output.join("01-existing.txt")).unwrap(),
        "old"
    );
    assert_eq!(
        fs::read_to_string(output.join("02-conflict")).unwrap(),
        "old conflict"
    );
    assert!(!output.join("02-conflict").join("inside.txt").exists());
    cleanup_staging(&staging).unwrap();
    let _ = fs::remove_dir_all(root);
}

#[cfg(target_os = "windows")]
#[test]
fn locked_file_is_reported_without_overwriting_the_original() {
    use std::os::windows::fs::OpenOptionsExt;

    let root = test_directory("file-in-use");
    let staging = root.join(".qzip-extract-locked");
    let output = root.join("output");
    fs::create_dir(&staging).unwrap();
    fs::create_dir(&output).unwrap();
    let target = output.join("locked.txt");
    fs::write(&target, "old").unwrap();
    fs::write(staging.join("locked.txt"), "new").unwrap();
    let handle = fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&target)
        .unwrap();
    let error = commit_extraction(&staging, &output, ConflictPolicy::Overwrite).unwrap_err();
    assert_eq!(error.code, ArchiveErrorCode::FileInUse);
    drop(handle);
    assert_eq!(fs::read_to_string(&target).unwrap(), "old");
    cleanup_staging(&staging).unwrap();
    let _ = fs::remove_dir_all(root);
}

#[cfg(target_os = "windows")]
#[test]
fn source_move_failure_restores_original_before_reporting_error() {
    use std::os::windows::fs::OpenOptionsExt;

    let root = test_directory("source-move-failure");
    let staging = root.join(".qzip-extract-source-locked");
    let output = root.join("output");
    fs::create_dir(&staging).unwrap();
    fs::create_dir(&output).unwrap();
    let source = staging.join("locked.txt");
    let target = output.join("locked.txt");
    fs::write(&source, "new").unwrap();
    fs::write(&target, "old").unwrap();
    let handle = fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&source)
        .unwrap();

    let error = commit_extraction(&staging, &output, ConflictPolicy::Overwrite).unwrap_err();
    assert_eq!(error.code, ArchiveErrorCode::FileInUse);
    assert_eq!(fs::read_to_string(&target).unwrap(), "old");
    drop(handle);
    cleanup_staging(&staging).unwrap();
    let _ = fs::remove_dir_all(root);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancellation_removes_create_staging_and_never_commits_output() {
    let root = test_directory("cancel-create");
    let input = root.join("input.txt");
    let output = root.join("result.7z");
    fs::write(&input, "source").unwrap();
    let (backend, state) = ScriptedBackend::new(BackendMode::CancelCreate);
    let manager = TaskManager::new(backend, root.join("history.json"));
    let task = manager
        .submit(
            TaskSpec::Create {
                inputs: vec![input],
                output: output.clone(),
                format: ArchiveFormat::SevenZip,
                profile: CompressionProfile::Balanced,
                encrypt_headers: false,
                test_after_create: false,
                delete_sources_after_success: false,
            },
            None,
        )
        .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(5), state.started.notified())
        .await
        .unwrap();
    manager.cancel(&task.task_id).unwrap();
    let final_task = wait_for_terminal(&manager, &task.task_id).await;
    assert_eq!(final_task.status, TaskStatus::Cancelled);
    assert!(!output.exists());
    assert_eq!(
        fs::read_dir(&root)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry
                .file_name()
                .to_string_lossy()
                .starts_with(".qzip-create-"))
            .count(),
        0
    );
    let _ = fs::remove_dir_all(root);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn wrong_password_retry_creates_a_new_successful_task_without_persisting_password() {
    let root = test_directory("password-retry");
    let archive = root.join("secret.7z");
    let output = root.join("output");
    fs::write(&archive, "archive").unwrap();
    let (backend, _) = ScriptedBackend::new(BackendMode::PasswordExtract);
    let manager = TaskManager::new(backend, root.join("history.json"));
    let failed = manager
        .submit(
            TaskSpec::Extract {
                archive: archive.clone(),
                output: output.clone(),
                selected_entries: None,
                conflict_policy: ConflictPolicy::Rename,
                accept_risk: true,
            },
            Some(SecretString::new("wrong".into())),
        )
        .unwrap();
    let failed_task = wait_for_terminal(&manager, &failed.task_id).await;
    assert_eq!(failed_task.status, TaskStatus::Failed);
    assert_eq!(
        failed_task.error.as_ref().unwrap().code,
        ArchiveErrorCode::WrongPassword
    );
    assert!(failed_task.retryable);
    let retried = manager
        .retry(&failed.task_id, Some(SecretString::new("correct".into())))
        .unwrap();
    assert_ne!(failed.task_id, retried.task_id);
    let completed = wait_for_terminal(&manager, &retried.task_id).await;
    assert_eq!(completed.status, TaskStatus::Completed);
    assert_eq!(
        completed
            .progress
            .as_ref()
            .map(|progress| progress.phase.as_str()),
        Some("committing")
    );
    assert_eq!(
        fs::read_to_string(output.join("file.txt")).unwrap(),
        "extracted"
    );
    let snapshots = serde_json::to_string(&manager.snapshots()).unwrap();
    assert!(!snapshots.contains("wrong"));
    assert!(!snapshots.contains("correct"));
    manager.clear_completed();
    assert!(manager.snapshots().is_empty());
    let _ = fs::remove_dir_all(root);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn task_runtime_never_runs_more_than_two_tasks_at_once() {
    let root = test_directory("concurrency");
    let (backend, state) = ScriptedBackend::new(BackendMode::BlockTest);
    let manager = TaskManager::new(backend, root.join("history.json"));
    let tasks = (0..4)
        .map(|_| {
            manager
                .submit(
                    TaskSpec::Test {
                        archive: root.join("archive.7z"),
                    },
                    None,
                )
                .unwrap()
        })
        .collect::<Vec<_>>();
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            if state.active.load(Ordering::SeqCst) == 2 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(state.peak.load(Ordering::SeqCst), 2);
    // A cancelled token remains observable by tasks that acquire a runtime
    // permit later. Unlike `Notify::notify_waiters`, the release signal
    // cannot be lost while the next queued tasks are being scheduled.
    state.release.cancel();
    for task in tasks {
        assert_eq!(
            wait_for_terminal(&manager, &task.task_id).await.status,
            TaskStatus::Completed
        );
    }
    let _ = fs::remove_dir_all(root);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn retry_and_cancel_reject_invalid_task_states() {
    let root = test_directory("invalid-states");
    let (backend, state) = ScriptedBackend::new(BackendMode::BlockTest);
    let manager = TaskManager::new(backend, root.join("history.json"));
    let running = manager
        .submit(
            TaskSpec::Test {
                archive: root.join("archive.7z"),
            },
            None,
        )
        .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(5), state.started.notified())
        .await
        .unwrap();
    assert_eq!(
        manager.retry(&running.task_id, None).unwrap_err().code,
        ArchiveErrorCode::InvalidRequest
    );
    manager.cancel(&running.task_id).unwrap();
    state.release.cancel();
    let cancelled = wait_for_terminal(&manager, &running.task_id).await;
    assert_eq!(cancelled.status, TaskStatus::Cancelled);
    assert!(cancelled.retryable);
    assert_eq!(
        manager.cancel(&running.task_id).unwrap_err().code,
        ArchiveErrorCode::InvalidRequest
    );
    let _ = fs::remove_dir_all(root);
}
