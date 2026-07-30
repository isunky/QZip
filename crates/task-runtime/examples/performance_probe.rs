#![forbid(unsafe_code)]

use std::{
    env, fs,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use archive_core::{
    ArchiveBackend, ArchiveEntry, ArchiveError, ArchiveErrorCode, ArchiveFormat, ArchiveResult,
    BackendCapabilities, CompressionProfile, CreateArchiveRequest, ExtractArchiveRequest,
    ListArchiveRequest, ProgressReporter, TaskProgress, TaskStatus, TestArchiveRequest, TestResult,
};
use async_trait::async_trait;
use serde::Serialize;
use task_runtime::{TaskManager, TaskSpec};
use tokio::{
    sync::broadcast::error::TryRecvError,
    time::{Duration, sleep},
};
use tokio_util::sync::CancellationToken;

const PROGRESS_EVENTS: usize = 2_000;

#[derive(Default)]
struct ProbeBackend {
    fail_next_create: AtomicBool,
}

#[async_trait]
impl ArchiveBackend for ProbeBackend {
    fn id(&self) -> &'static str {
        "performance-probe"
    }

    async fn capabilities(&self) -> Result<BackendCapabilities, ArchiveError> {
        Ok(BackendCapabilities {
            backend_id: self.id().into(),
            version: "1".into(),
            writable_formats: vec![ArchiveFormat::SevenZip],
            readable_formats: vec![ArchiveFormat::SevenZip],
            supports_password: false,
            supports_header_encryption: false,
            supports_partial_extract: false,
            supports_update: false,
            supports_progress: true,
            supports_cancellation: true,
        })
    }

    async fn create(
        &self,
        request: CreateArchiveRequest,
        progress: Arc<dyn ProgressReporter>,
        _: CancellationToken,
    ) -> Result<ArchiveResult, ArchiveError> {
        for index in 0..PROGRESS_EVENTS {
            progress.report(TaskProgress {
                operation: archive_core::ArchiveOperation::Create,
                percent: Some(((index * 100) / PROGRESS_EVENTS) as u8),
                detail: format!("entry-{index}"),
            });
        }
        if self.fail_next_create.swap(false, Ordering::SeqCst) {
            return Err(ArchiveError::new(
                ArchiveErrorCode::Unknown,
                "synthetic backend failure",
            ));
        }
        fs::write(&request.output, b"qzip performance probe").map_err(|_| {
            ArchiveError::new(ArchiveErrorCode::Unknown, "cannot create probe output")
        })?;
        Ok(ArchiveResult {
            output: Some(request.output),
            entries: vec![],
            warnings: vec![],
        })
    }

    async fn extract(
        &self,
        _: ExtractArchiveRequest,
        _: Arc<dyn ProgressReporter>,
        _: CancellationToken,
    ) -> Result<ArchiveResult, ArchiveError> {
        Err(ArchiveError::new(
            ArchiveErrorCode::UnsupportedOption,
            "probe does not extract",
        ))
    }
    async fn list(
        &self,
        _: ListArchiveRequest,
        _: CancellationToken,
    ) -> Result<Vec<ArchiveEntry>, ArchiveError> {
        Ok(vec![])
    }
    async fn test(
        &self,
        _: TestArchiveRequest,
        _: CancellationToken,
    ) -> Result<TestResult, ArchiveError> {
        Ok(TestResult {
            valid: true,
            warnings: vec![],
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProgressReport {
    emitted: usize,
    received: usize,
    dropped: usize,
    duration_milliseconds: u128,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RecoveryReport {
    failed_task_milliseconds: u128,
    subsequent_task_milliseconds: u128,
    subsequent_task_completed: bool,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Report {
    schema_version: u8,
    recorded_at: u64,
    progress: ProgressReport,
    exception_recovery: RecoveryReport,
}

fn create_spec(input: &Path, output: PathBuf) -> TaskSpec {
    TaskSpec::Create {
        inputs: vec![input.to_path_buf()],
        output,
        format: ArchiveFormat::SevenZip,
        profile: CompressionProfile::Balanced,
        encrypt_headers: false,
        test_after_create: false,
        delete_sources_after_success: false,
    }
}

async fn wait_for_terminal(manager: &Arc<TaskManager>, task_id: &str) -> TaskStatus {
    loop {
        if let Some(snapshot) = manager
            .snapshots()
            .into_iter()
            .find(|item| item.task_id == task_id)
            && matches!(
                snapshot.status,
                TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled
            )
        {
            return snapshot.status;
        }
        sleep(Duration::from_millis(5)).await;
    }
}

#[tokio::main]
async fn main() {
    let output = env::args()
        .nth(1)
        .map(PathBuf::from)
        .expect("usage: performance_probe <output.json>");
    let root = env::temp_dir().join(format!("qzip-performance-probe-{}", std::process::id()));
    fs::create_dir_all(&root).expect("create probe root");
    let input = root.join("input.txt");
    fs::write(&input, b"synthetic input").expect("write probe input");
    let backend = Arc::new(ProbeBackend::default());
    let manager = TaskManager::new(backend.clone(), root.join("history.json"));
    let mut receiver = manager.subscribe();

    let started = Instant::now();
    let progress_task = manager
        .submit(create_spec(&input, root.join("progress.7z")), None)
        .expect("submit progress task");
    assert_eq!(
        wait_for_terminal(&manager, &progress_task.task_id).await,
        TaskStatus::Completed
    );
    let mut received = 0usize;
    let mut dropped = 0usize;
    loop {
        match receiver.try_recv() {
            Ok(event) => {
                if event.event_type == "task.progress" {
                    received += 1;
                }
            }
            Err(TryRecvError::Lagged(count)) => dropped += count as usize,
            Err(TryRecvError::Empty | TryRecvError::Closed) => break,
        }
    }
    let progress = ProgressReport {
        emitted: PROGRESS_EVENTS,
        received,
        dropped,
        duration_milliseconds: started.elapsed().as_millis(),
    };

    backend.fail_next_create.store(true, Ordering::SeqCst);
    let failed_started = Instant::now();
    let failed_task = manager
        .submit(create_spec(&input, root.join("failed.7z")), None)
        .expect("submit failure task");
    assert_eq!(
        wait_for_terminal(&manager, &failed_task.task_id).await,
        TaskStatus::Failed
    );
    let failed_task_milliseconds = failed_started.elapsed().as_millis();

    let recovery_started = Instant::now();
    let recovery_task = manager
        .submit(create_spec(&input, root.join("recovery.7z")), None)
        .expect("submit recovery task");
    let subsequent_task_completed =
        wait_for_terminal(&manager, &recovery_task.task_id).await == TaskStatus::Completed;
    let report = Report {
        schema_version: 1,
        recorded_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        progress,
        exception_recovery: RecoveryReport {
            failed_task_milliseconds,
            subsequent_task_milliseconds: recovery_started.elapsed().as_millis(),
            subsequent_task_completed,
        },
    };
    fs::write(
        &output,
        serde_json::to_vec_pretty(&report).expect("serialize report"),
    )
    .expect("write report");
    let _ = fs::remove_dir_all(root);
}
