#![forbid(unsafe_code)]

//! Bounded archive task queue. Task snapshots and persisted history are always
//! password-free; secrets are accepted only for the single running invocation.

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use archive_core::{
    ArchiveBackend, ArchiveError, ArchiveErrorCode, ArchiveFormat, ArchiveOperation, ArchiveResult,
    CompressionProfile, ConflictPolicy, CreateArchiveRequest, ExtractArchiveRequest,
    ListArchiveRequest, ProgressReporter, TaskProgress as BackendProgress, TaskStatus,
    TestArchiveRequest, TestResult, UpdateArchiveRequest,
};
use archive_security::{ExtractionSecurityPolicy, assess_entries};
use fs2::available_space;
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use tokio::sync::{Semaphore, broadcast};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

const HISTORY_LIMIT: usize = 100;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum TaskSpec {
    Create {
        inputs: Vec<PathBuf>,
        output: PathBuf,
        format: ArchiveFormat,
        profile: CompressionProfile,
        encrypt_headers: bool,
        test_after_create: bool,
        delete_sources_after_success: bool,
    },
    Extract {
        archive: PathBuf,
        output: PathBuf,
        selected_entries: Option<Vec<String>>,
        conflict_policy: ConflictPolicy,
        accept_risk: bool,
    },
    Test {
        archive: PathBuf,
    },
    Update {
        archive: PathBuf,
        inputs: Vec<PathBuf>,
    },
}
impl TaskSpec {
    pub fn operation(&self) -> ArchiveOperation {
        match self {
            Self::Create { .. } => ArchiveOperation::Create,
            Self::Extract { .. } => ArchiveOperation::Extract,
            Self::Test { .. } => ArchiveOperation::Test,
            Self::Update { .. } => ArchiveOperation::Update,
        }
    }
    pub fn display_name(&self) -> String {
        let path = match self {
            Self::Create { output, .. } => output,
            Self::Extract { archive, .. }
            | Self::Test { archive }
            | Self::Update { archive, .. } => archive,
        };
        path.file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("未命名任务")
            .to_owned()
    }
    pub fn output(&self) -> Option<PathBuf> {
        match self {
            Self::Create { output, .. } | Self::Extract { output, .. } => Some(output.clone()),
            Self::Test { .. } | Self::Update { .. } => None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeProgress {
    pub phase: String,
    pub percent: Option<u8>,
    pub current_entry: Option<String>,
    pub elapsed_seconds: u64,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskSnapshot {
    pub task_id: String,
    pub operation: ArchiveOperation,
    pub status: TaskStatus,
    pub display_name: String,
    pub output: Option<PathBuf>,
    pub created_at: u64,
    pub updated_at: u64,
    pub progress: Option<RuntimeProgress>,
    pub error: Option<ArchiveError>,
    pub warnings: Vec<String>,
    pub retryable: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskEvent {
    pub event_type: String,
    pub task: TaskSnapshot,
}

struct TaskRecord {
    snapshot: TaskSnapshot,
    spec: TaskSpec,
    cancellation: CancellationToken,
    can_retry: bool,
}
pub struct TaskManager {
    backend: Arc<dyn ArchiveBackend>,
    tasks: Arc<Mutex<BTreeMap<String, TaskRecord>>>,
    events: broadcast::Sender<TaskEvent>,
    semaphore: Arc<Semaphore>,
    history_path: PathBuf,
}

impl TaskManager {
    pub fn new(backend: Arc<dyn ArchiveBackend>, history_path: PathBuf) -> Arc<Self> {
        let (events, _) = broadcast::channel(256);
        let manager = Arc::new(Self {
            backend,
            tasks: Arc::new(Mutex::new(BTreeMap::new())),
            events,
            semaphore: Arc::new(Semaphore::new(2)),
            history_path,
        });
        manager.load_history();
        manager
    }
    pub fn subscribe(&self) -> broadcast::Receiver<TaskEvent> {
        self.events.subscribe()
    }
    pub fn snapshots(&self) -> Vec<TaskSnapshot> {
        self.tasks
            .lock()
            .expect("task lock")
            .values()
            .map(|record| record.snapshot.clone())
            .collect()
    }
    pub fn submit(
        self: &Arc<Self>,
        spec: TaskSpec,
        password: Option<SecretString>,
    ) -> TaskSnapshot {
        let now = now();
        let task_id = Uuid::new_v4().to_string();
        let snapshot = TaskSnapshot {
            task_id: task_id.clone(),
            operation: spec.operation(),
            status: TaskStatus::Queued,
            display_name: spec.display_name(),
            output: spec.output(),
            created_at: now,
            updated_at: now,
            progress: None,
            error: None,
            warnings: vec![],
            retryable: false,
        };
        self.tasks.lock().expect("task lock").insert(
            task_id.clone(),
            TaskRecord {
                snapshot: snapshot.clone(),
                spec,
                cancellation: CancellationToken::new(),
                can_retry: true,
            },
        );
        self.emit("task.created", &task_id);
        let manager = Arc::clone(self);
        tokio::spawn(async move {
            manager.run(task_id, password).await;
        });
        snapshot
    }
    pub fn cancel(&self, task_id: &str) -> Result<(), ArchiveError> {
        let mut tasks = self.tasks.lock().expect("task lock");
        let record = tasks
            .get_mut(task_id)
            .ok_or_else(|| ArchiveError::new(ArchiveErrorCode::InvalidRequest, "任务不存在"))?;
        match record.snapshot.status {
            TaskStatus::Queued | TaskStatus::Scanning | TaskStatus::Running => {
                record.snapshot.status = if record.snapshot.status == TaskStatus::Queued {
                    TaskStatus::Cancelled
                } else {
                    TaskStatus::Cancelling
                };
                record.snapshot.updated_at = now();
                record.cancellation.cancel();
            }
            _ => {
                return Err(ArchiveError::new(
                    ArchiveErrorCode::InvalidRequest,
                    "此任务无法取消",
                ));
            }
        }
        drop(tasks);
        self.emit("task.updated", task_id);
        Ok(())
    }
    pub fn retry(
        self: &Arc<Self>,
        task_id: &str,
        password: Option<SecretString>,
    ) -> Result<TaskSnapshot, ArchiveError> {
        let tasks = self.tasks.lock().expect("task lock");
        let record = tasks
            .get(task_id)
            .ok_or_else(|| ArchiveError::new(ArchiveErrorCode::InvalidRequest, "任务不存在"))?;
        if !record.can_retry {
            return Err(ArchiveError::new(
                ArchiveErrorCode::InvalidRequest,
                "重启后的历史记录不包含源文件位置，请重新选择文件后创建任务",
            ));
        }
        let spec = record.spec.clone();
        drop(tasks);
        Ok(self.submit(spec, password))
    }
    pub fn clear_completed(&self) {
        self.tasks.lock().expect("task lock").retain(|_, record| {
            !matches!(
                record.snapshot.status,
                TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled
            )
        });
        self.persist_history();
    }
    async fn run(self: Arc<Self>, task_id: String, password: Option<SecretString>) {
        let permit = match self.semaphore.clone().acquire_owned().await {
            Ok(permit) => permit,
            Err(_) => return,
        };
        if self.status(&task_id) == Some(TaskStatus::Cancelled) {
            drop(permit);
            self.persist_history();
            return;
        }
        self.set_status(&task_id, TaskStatus::Scanning, None);
        let (spec, cancellation) = match self.tasks.lock().expect("task lock").get(&task_id) {
            Some(record) => (record.spec.clone(), record.cancellation.clone()),
            None => return,
        };
        let reporter: Arc<dyn ProgressReporter> = Arc::new(RuntimeReporter {
            manager: Arc::clone(&self),
            task_id: task_id.clone(),
            started_at: now(),
        });
        let result = self
            .execute(&task_id, &spec, password, reporter, cancellation.clone())
            .await;
        match result {
            Ok(warnings) => self.finish(&task_id, TaskStatus::Completed, warnings, None),
            Err(error)
                if error.code == ArchiveErrorCode::Cancelled || cancellation.is_cancelled() =>
            {
                self.finish(
                    &task_id,
                    TaskStatus::Cancelled,
                    vec![],
                    Some(ArchiveError::new(ArchiveErrorCode::Cancelled, "任务已取消")),
                )
            }
            Err(error) => self.finish(&task_id, TaskStatus::Failed, vec![], Some(error)),
        }
        drop(permit);
        self.persist_history();
    }
    async fn execute(
        &self,
        task_id: &str,
        spec: &TaskSpec,
        password: Option<SecretString>,
        reporter: Arc<dyn ProgressReporter>,
        cancellation: CancellationToken,
    ) -> Result<Vec<String>, ArchiveError> {
        self.set_status(task_id, TaskStatus::Running, None);
        match spec {
            TaskSpec::Create {
                inputs,
                output,
                format,
                profile,
                encrypt_headers,
                test_after_create,
                ..
            } => {
                validate_create_destination(inputs, output)?;
                let temporary_output = temporary_archive_path(output)?;
                let result = self
                    .backend
                    .create(
                        CreateArchiveRequest {
                            inputs: inputs.clone(),
                            output: temporary_output.clone(),
                            format: *format,
                            profile: *profile,
                            password,
                            encrypt_headers: *encrypt_headers,
                            test_after_create: *test_after_create,
                        },
                        reporter,
                        cancellation.child_token(),
                    )
                    .await;
                let result = match result {
                    Ok(result) => result,
                    Err(error) => {
                        let _ = fs::remove_file(&temporary_output);
                        return Err(error);
                    }
                };
                if *test_after_create {
                    let test = self
                        .backend
                        .test(
                            TestArchiveRequest {
                                archive: temporary_output.clone(),
                                password: None,
                            },
                            cancellation.child_token(),
                        )
                        .await;
                    if let Err(error) = test {
                        let _ = fs::remove_file(&temporary_output);
                        return Err(error);
                    }
                }
                if cancellation.is_cancelled() {
                    let _ = fs::remove_file(&temporary_output);
                    return Err(ArchiveError::new(ArchiveErrorCode::Cancelled, "任务已取消"));
                }
                commit_created_archive(&temporary_output, output)?;
                Ok(result.warnings)
            }
            TaskSpec::Extract {
                archive,
                output,
                selected_entries,
                conflict_policy,
                accept_risk,
            } => {
                let entries = self
                    .backend
                    .list(
                        ListArchiveRequest {
                            archive: archive.clone(),
                            password: password.clone(),
                        },
                        cancellation.child_token(),
                    )
                    .await?;
                let archive_size = std::fs::metadata(archive)
                    .map(|metadata| metadata.len())
                    .unwrap_or(0);
                let available_bytes = output.parent().and_then(|parent| {
                    fs::create_dir_all(parent).ok()?;
                    available_space(parent).ok()
                });
                let risks = assess_entries(
                    &entries,
                    archive_size,
                    available_bytes,
                    &ExtractionSecurityPolicy::default(),
                );
                if risks.iter().any(|risk| !risk.overridable)
                    || (!*accept_risk && !risks.is_empty())
                {
                    return Err(ArchiveError::new(
                        if risks.iter().any(|risk| !risk.overridable) {
                            ArchiveErrorCode::UnsafePath
                        } else {
                            ArchiveErrorCode::ArchiveBombRisk
                        },
                        risks
                            .first()
                            .map(|risk| risk.message.clone())
                            .unwrap_or_else(|| "压缩包风险检查失败".into()),
                    ));
                }
                let staging = prepare_extraction_staging(output)?;
                let result = self
                    .backend
                    .extract(
                        ExtractArchiveRequest {
                            archive: archive.clone(),
                            output: staging.clone(),
                            selected_entries: selected_entries.clone(),
                            conflict_policy: *conflict_policy,
                            password,
                        },
                        reporter,
                        cancellation.child_token(),
                    )
                    .await;
                let result = match result {
                    Ok(result) if !cancellation.is_cancelled() => result,
                    Ok(_) => {
                        cleanup_staging(&staging);
                        return Err(ArchiveError::new(ArchiveErrorCode::Cancelled, "任务已取消"));
                    }
                    Err(error) => {
                        cleanup_staging(&staging);
                        return Err(error);
                    }
                };
                if let Err(error) = commit_extraction(&staging, output, *conflict_policy) {
                    cleanup_staging(&staging);
                    return Err(error);
                }
                Ok(result.warnings)
            }
            TaskSpec::Test { archive } => {
                let result: TestResult = self
                    .backend
                    .test(
                        TestArchiveRequest {
                            archive: archive.clone(),
                            password,
                        },
                        cancellation.child_token(),
                    )
                    .await?;
                Ok(result.warnings)
            }
            TaskSpec::Update { archive, inputs } => {
                let result: ArchiveResult = self
                    .backend
                    .update(
                        UpdateArchiveRequest {
                            archive: archive.clone(),
                            inputs: inputs.clone(),
                            password,
                        },
                        reporter,
                        cancellation.child_token(),
                    )
                    .await?;
                Ok(result.warnings)
            }
        }
    }
    fn set_status(&self, task_id: &str, status: TaskStatus, progress: Option<RuntimeProgress>) {
        if let Some(record) = self.tasks.lock().expect("task lock").get_mut(task_id) {
            record.snapshot.status = status;
            record.snapshot.updated_at = now();
            if progress.is_some() {
                record.snapshot.progress = progress;
            }
        }
        self.emit("task.updated", task_id);
    }
    fn finish(
        &self,
        task_id: &str,
        status: TaskStatus,
        warnings: Vec<String>,
        error: Option<ArchiveError>,
    ) {
        if let Some(record) = self.tasks.lock().expect("task lock").get_mut(task_id) {
            record.snapshot.status = status;
            record.snapshot.updated_at = now();
            record.snapshot.warnings = warnings;
            record.snapshot.retryable =
                matches!(status, TaskStatus::Failed | TaskStatus::Cancelled);
            record.snapshot.error = error;
        }
        self.emit(
            match status {
                TaskStatus::Completed => "task.completed",
                TaskStatus::Cancelled => "task.cancelled",
                _ => "task.failed",
            },
            task_id,
        );
    }
    fn status(&self, task_id: &str) -> Option<TaskStatus> {
        self.tasks
            .lock()
            .expect("task lock")
            .get(task_id)
            .map(|record| record.snapshot.status)
    }
    fn emit(&self, event_type: &str, task_id: &str) {
        if let Some(record) = self.tasks.lock().expect("task lock").get(task_id) {
            let _ = self.events.send(TaskEvent {
                event_type: event_type.into(),
                task: record.snapshot.clone(),
            });
        }
    }
    fn load_history(&self) {
        let Ok(bytes) = fs::read(&self.history_path) else {
            return;
        };
        let Ok(history) = serde_json::from_slice::<Vec<TaskSnapshot>>(&bytes) else {
            return;
        };
        let mut tasks = self.tasks.lock().expect("task lock");
        for snapshot in history.into_iter().take(HISTORY_LIMIT) {
            let spec = TaskSpec::Test {
                archive: PathBuf::new(),
            };
            tasks.insert(
                snapshot.task_id.clone(),
                TaskRecord {
                    snapshot,
                    spec,
                    cancellation: CancellationToken::new(),
                    can_retry: false,
                },
            );
        }
    }
    fn persist_history(&self) {
        let history: Vec<_> = self
            .tasks
            .lock()
            .expect("task lock")
            .values()
            .filter(|record| {
                matches!(
                    record.snapshot.status,
                    TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled
                )
            })
            .map(|record| record.snapshot.clone())
            .take(HISTORY_LIMIT)
            .collect();
        if let Some(parent) = self.history_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let temporary = self.history_path.with_extension("json.tmp");
        if let Ok(file) = fs::File::create(&temporary)
            && serde_json::to_writer(file, &history).is_ok()
        {
            let _ = fs::remove_file(&self.history_path);
            let _ = fs::rename(temporary, &self.history_path);
        }
    }
}

fn validate_create_destination(inputs: &[PathBuf], output: &Path) -> Result<(), ArchiveError> {
    if inputs.is_empty() {
        return Err(ArchiveError::invalid_option(
            "inputs",
            "at least one input is required",
        ));
    }
    if output.exists() {
        return Err(ArchiveError::new(
            ArchiveErrorCode::ConflictRequiresDecision,
            "目标压缩包已存在，请选择其他文件名",
        ));
    }
    let parent = output
        .parent()
        .ok_or_else(|| ArchiveError::new(ArchiveErrorCode::InvalidRequest, "压缩包保存位置无效"))?;
    fs::create_dir_all(parent).map_err(|_| {
        ArchiveError::new(ArchiveErrorCode::PermissionDenied, "无法创建压缩包保存目录")
    })?;
    let canonical_parent = parent.canonicalize().map_err(|_| {
        ArchiveError::new(ArchiveErrorCode::PermissionDenied, "无法访问压缩包保存目录")
    })?;
    let target =
        canonical_parent.join(output.file_name().ok_or_else(|| {
            ArchiveError::new(ArchiveErrorCode::InvalidRequest, "压缩包文件名无效")
        })?);
    for input in inputs {
        let metadata = fs::metadata(input).map_err(|_| {
            ArchiveError::new(ArchiveErrorCode::FileNotFound, "待压缩的文件或目录不存在")
        })?;
        if metadata.is_dir() {
            let canonical_input = input.canonicalize().map_err(|_| {
                ArchiveError::new(ArchiveErrorCode::PermissionDenied, "无法访问待压缩目录")
            })?;
            if target.starts_with(canonical_input) {
                return Err(ArchiveError::new(
                    ArchiveErrorCode::InvalidRequest,
                    "压缩包不能保存到待压缩目录内部",
                ));
            }
        }
    }
    Ok(())
}

fn temporary_archive_path(output: &Path) -> Result<PathBuf, ArchiveError> {
    let parent = output
        .parent()
        .ok_or_else(|| ArchiveError::new(ArchiveErrorCode::InvalidRequest, "压缩包保存位置无效"))?;
    let name = output
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| ArchiveError::new(ArchiveErrorCode::InvalidRequest, "压缩包文件名无效"))?;
    Ok(parent.join(format!(".qzip-{}-{name}", Uuid::new_v4())))
}

fn commit_created_archive(temporary: &Path, output: &Path) -> Result<(), ArchiveError> {
    if !temporary.is_file() {
        return Err(ArchiveError::new(
            ArchiveErrorCode::CleanupFailed,
            "压缩任务未生成可提交的临时文件",
        ));
    }
    if output.exists() {
        let _ = fs::remove_file(temporary);
        return Err(ArchiveError::new(
            ArchiveErrorCode::ConflictRequiresDecision,
            "目标压缩包已存在，未覆盖原文件",
        ));
    }
    fs::rename(temporary, output)
        .map_err(|_| ArchiveError::new(ArchiveErrorCode::PermissionDenied, "无法提交新建压缩包"))
}

fn prepare_extraction_staging(output: &Path) -> Result<PathBuf, ArchiveError> {
    let parent = output
        .parent()
        .ok_or_else(|| ArchiveError::new(ArchiveErrorCode::InvalidRequest, "解压目标位置无效"))?;
    fs::create_dir_all(parent).map_err(|_| {
        ArchiveError::new(ArchiveErrorCode::PermissionDenied, "无法创建解压目标目录")
    })?;
    let parent = parent.canonicalize().map_err(|_| {
        ArchiveError::new(ArchiveErrorCode::PermissionDenied, "无法访问解压目标目录")
    })?;
    let staging = parent.join(format!(".qzip-extract-{}", Uuid::new_v4()));
    fs::create_dir(&staging).map_err(|_| {
        ArchiveError::new(
            ArchiveErrorCode::PermissionDenied,
            "无法创建安全解压暂存目录",
        )
    })?;
    Ok(staging)
}

fn cleanup_staging(staging: &Path) {
    if staging
        .file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.starts_with(".qzip-extract-"))
    {
        let _ = fs::remove_dir_all(staging);
    }
}

fn commit_extraction(
    staging: &Path,
    output: &Path,
    policy: ConflictPolicy,
) -> Result<(), ArchiveError> {
    if !staging.is_dir() {
        return Err(ArchiveError::new(
            ArchiveErrorCode::CleanupFailed,
            "解压暂存目录已丢失",
        ));
    }
    if !output.exists() {
        fs::rename(staging, output).map_err(|_| {
            ArchiveError::new(ArchiveErrorCode::PermissionDenied, "无法提交解压结果")
        })?;
        return Ok(());
    }
    if !output.is_dir() || path_is_link_or_reparse(output)? {
        return Err(ArchiveError::new(
            ArchiveErrorCode::UnsafePath,
            "解压目标不是安全目录",
        ));
    }
    merge_directory(staging, output, policy)?;
    cleanup_staging(staging);
    Ok(())
}

fn merge_directory(
    source: &Path,
    target: &Path,
    policy: ConflictPolicy,
) -> Result<(), ArchiveError> {
    for entry in fs::read_dir(source).map_err(|_| {
        ArchiveError::new(ArchiveErrorCode::PermissionDenied, "无法读取解压暂存目录")
    })? {
        let entry = entry.map_err(|_| {
            ArchiveError::new(ArchiveErrorCode::PermissionDenied, "无法读取解压暂存条目")
        })?;
        let source_path = entry.path();
        if path_is_link_or_reparse(&source_path)? {
            return Err(ArchiveError::new(
                ArchiveErrorCode::UnsafePath,
                "解压结果包含不安全链接",
            ));
        }
        let is_directory = entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false);
        let mut target_path = target.join(entry.file_name());
        if target_path.exists() {
            if path_is_link_or_reparse(&target_path)? {
                return Err(ArchiveError::new(
                    ArchiveErrorCode::UnsafePath,
                    "解压目标包含不安全链接",
                ));
            }
            if is_directory && target_path.is_dir() {
                merge_directory(&source_path, &target_path, policy)?;
                continue;
            }
            match policy {
                ConflictPolicy::Rename => target_path = renamed_path(&target_path),
                ConflictPolicy::Overwrite => {
                    if target_path.is_dir() {
                        return Err(ArchiveError::new(
                            ArchiveErrorCode::ConflictRequiresDecision,
                            "文件与目录同名，无法安全覆盖",
                        ));
                    }
                    fs::remove_file(&target_path).map_err(|_| {
                        ArchiveError::new(ArchiveErrorCode::PermissionDenied, "无法覆盖现有文件")
                    })?;
                }
                ConflictPolicy::Skip => continue,
                ConflictPolicy::Ask => {
                    return Err(ArchiveError::new(
                        ArchiveErrorCode::ConflictRequiresDecision,
                        "需要选择冲突文件处理方式",
                    ));
                }
            }
        }
        if is_directory {
            fs::create_dir(&target_path).map_err(|_| {
                ArchiveError::new(ArchiveErrorCode::PermissionDenied, "无法创建解压目录")
            })?;
            merge_directory(&source_path, &target_path, policy)?;
        } else {
            fs::rename(&source_path, &target_path).map_err(|_| {
                ArchiveError::new(ArchiveErrorCode::PermissionDenied, "无法写入解压文件")
            })?;
        }
    }
    Ok(())
}

fn renamed_path(path: &Path) -> PathBuf {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("文件");
    let extension = path.extension().and_then(|value| value.to_str());
    for index in 1..10_000 {
        let name = match extension {
            Some(extension) => format!("{stem} ({index}).{extension}"),
            None => format!("{stem} ({index})"),
        };
        let candidate = parent.join(name);
        if !candidate.exists() {
            return candidate;
        }
    }
    parent.join(format!("{stem} ({})", Uuid::new_v4()))
}

fn path_is_link_or_reparse(path: &Path) -> Result<bool, ArchiveError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|_| ArchiveError::new(ArchiveErrorCode::PermissionDenied, "无法检查输出路径"))?;
    if metadata.file_type().is_symlink() {
        return Ok(true);
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::fs::MetadataExt;
        Ok(metadata.file_attributes() & 0x400 != 0)
    }
    #[cfg(not(target_os = "windows"))]
    Ok(false)
}
struct RuntimeReporter {
    manager: Arc<TaskManager>,
    task_id: String,
    started_at: u64,
}
impl ProgressReporter for RuntimeReporter {
    fn report(&self, progress: BackendProgress) {
        let detail = progress
            .detail
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(&progress.detail)
            .to_owned();
        self.manager.set_status(
            &self.task_id,
            TaskStatus::Running,
            Some(RuntimeProgress {
                phase: format!("{:?}", progress.operation).to_ascii_lowercase(),
                percent: progress.percent,
                current_entry: Some(detail),
                elapsed_seconds: now().saturating_sub(self.started_at),
            }),
        );
        self.manager.emit("task.progress", &self.task_id);
    }
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_directory(label: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("qzip-runtime-{label}-{}", Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn refuses_an_archive_inside_an_input_directory() {
        let root = test_directory("recursive-output");
        let input = root.join("input");
        fs::create_dir(&input).unwrap();
        let error =
            validate_create_destination(std::slice::from_ref(&input), &input.join("result.7z"))
                .unwrap_err();
        assert_eq!(error.code, ArchiveErrorCode::InvalidRequest);
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
}
