#![forbid(unsafe_code)]

//! 7-Zip command-line adapter. Arguments are always passed as an argv array;
//! neither archive names nor user paths are ever interpolated into a shell.

use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
    time::Duration,
};

use archive_core::{
    ArchiveBackend, ArchiveEntry, ArchiveError, ArchiveErrorCode, ArchiveFormat, ArchiveOperation,
    ArchiveResult, BackendCapabilities, CompressionProfile, ConflictPolicy, CreateArchiveRequest,
    ExtractArchiveRequest, ListArchiveRequest, ProgressReporter, TaskProgress, TestArchiveRequest,
    TestResult, UpdateArchiveRequest,
};
use archive_security::safe_relative_path;
use async_trait::async_trait;
use secrecy::ExposeSecret;
use sha2::{Digest, Sha256};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, BufReader},
    process::Command,
    sync::mpsc,
    time::Instant,
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

const EXPECTED_VERSION: &str = "26.02";
const EXPECTED_EXECUTABLE_SHA256: &str =
    "83967f1b02b43c4efeda302795722c809e0e81b8307de73558d10484d5676a7d";
const EXPECTED_LIBRARY_SHA256: &str =
    "69fd4df057985c40e510e2fac182881c7f85e90aa13ec703f763a8fdb2ce61f8";
const MAX_DIAGNOSTIC_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug)]
pub struct SevenZipCliBackend {
    executable: PathBuf,
}

impl SevenZipCliBackend {
    pub fn new(executable: PathBuf) -> Self {
        Self { executable }
    }
    pub fn executable(&self) -> &Path {
        &self.executable
    }

    fn verify_runtime_files(&self) -> Result<(), ArchiveError> {
        verify_sha256(&self.executable, EXPECTED_EXECUTABLE_SHA256)?;
        let library = self.executable.with_file_name("7z.dll");
        verify_sha256(&library, EXPECTED_LIBRARY_SHA256)
    }

    async fn invoke(
        &self,
        operation: ArchiveOperation,
        args: Vec<String>,
        progress: Arc<dyn ProgressReporter>,
        cancellation: CancellationToken,
    ) -> Result<InvocationOutput, ArchiveError> {
        if !self.executable.is_file() {
            return Err(ArchiveError::unavailable("7-Zip sidecar was not found"));
        }
        self.verify_runtime_files()?;
        self.invoke_process(
            operation,
            args,
            progress,
            cancellation,
            InvocationMode::Diagnostic,
        )
        .await
    }

    async fn invoke_listing(
        &self,
        args: Vec<String>,
        cancellation: CancellationToken,
    ) -> Result<Vec<ArchiveEntry>, ArchiveError> {
        if !self.executable.is_file() {
            return Err(ArchiveError::unavailable("7-Zip sidecar was not found"));
        }
        self.verify_runtime_files()?;
        let output = self
            .invoke_process(
                ArchiveOperation::List,
                args,
                Arc::new(archive_core::NoopProgressReporter),
                cancellation,
                InvocationMode::Listing,
            )
            .await?;
        output.entries.ok_or_else(|| {
            ArchiveError::new(
                ArchiveErrorCode::Unknown,
                "7-Zip did not return a parsed archive listing",
            )
        })
    }

    async fn invoke_process(
        &self,
        operation: ArchiveOperation,
        args: Vec<String>,
        progress: Arc<dyn ProgressReporter>,
        cancellation: CancellationToken,
        mode: InvocationMode,
    ) -> Result<InvocationOutput, ArchiveError> {
        let mut command = Command::new(&self.executable);
        command
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(target_os = "windows")]
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        let mut child = command.spawn().map_err(|error| {
            ArchiveError::unavailable(format!("could not start 7-Zip: {error}"))
        })?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| ArchiveError::unavailable("could not capture 7-Zip output"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| ArchiveError::unavailable("could not capture 7-Zip diagnostics"))?;
        let (sender, mut receiver) = mpsc::unbounded_channel();
        let stdout_sender = sender.clone();
        let stdout_task = tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if stdout_sender
                    .send(ProcessLine {
                        stream: ProcessStream::Stdout,
                        line,
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
        let stderr_sender = sender.clone();
        let stderr_task = tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if stderr_sender
                    .send(ProcessLine {
                        stream: ProcessStream::Stderr,
                        line,
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
        drop(sender);
        let mut state = InvocationState::new(mode);
        loop {
            tokio::select! {
                _ = cancellation.cancelled() => { let _ = child.start_kill(); let _ = child.wait().await; let _ = stdout_task.await; let _ = stderr_task.await; return Err(ArchiveError::new(ArchiveErrorCode::Cancelled, "archive operation was cancelled")); }
                Some(line) = receiver.recv() => {
                    state.consume(line, operation, progress.as_ref());
                }
                status = child.wait() => {
                    let status = status.map_err(|error| ArchiveError::new(ArchiveErrorCode::Unknown, format!("7-Zip process failed: {error}")))?;
                    let _ = stdout_task.await;
                    let _ = stderr_task.await;
                    while let Ok(line) = receiver.try_recv() {
                        state.consume(line, operation, progress.as_ref());
                    }
                    let (output, entries) = state.finish();
                    if status.success() || status.code() == Some(1) {
                        return Ok(InvocationOutput { output, warning: status.code() == Some(1), entries });
                    }
                    return Err(map_exit(status.code(), &output));
                }
            }
        }
    }

    async fn check_version(&self) -> Result<String, ArchiveError> {
        let output = self
            .invoke(
                ArchiveOperation::Test,
                vec!["i".into()],
                Arc::new(archive_core::NoopProgressReporter),
                CancellationToken::new(),
            )
            .await?;
        let version = output
            .output
            .lines()
            .find_map(|line| {
                line.strip_prefix("7-Zip ")
                    .and_then(|rest| rest.split_whitespace().next())
            })
            .unwrap_or_default();
        if version != EXPECTED_VERSION {
            return Err(ArchiveError::unavailable(format!(
                "unsupported 7-Zip version: {version}; expected {EXPECTED_VERSION}"
            )));
        }
        Ok(version.to_owned())
    }

    async fn create_tar_compressed(
        &self,
        request: CreateArchiveRequest,
        progress: Arc<dyn ProgressReporter>,
        cancellation: CancellationToken,
    ) -> Result<ArchiveResult, ArchiveError> {
        let compression = match request.format {
            ArchiveFormat::TarGz => "gzip",
            ArchiveFormat::TarXz => "xz",
            _ => unreachable!("only TAR wrappers use this path"),
        };
        let stem = request
            .output
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| {
                name.strip_suffix(".tar.gz")
                    .or_else(|| name.strip_suffix(".tar.xz"))
                    .or_else(|| name.strip_suffix(".tgz"))
                    .or_else(|| name.strip_suffix(".txz"))
            })
            .filter(|name| !name.is_empty())
            .unwrap_or("archive");
        let temporary_dir = std::env::temp_dir().join(format!("qzip-{}", Uuid::new_v4()));
        fs::create_dir_all(&temporary_dir).map_err(|error| {
            ArchiveError::new(
                ArchiveErrorCode::PermissionDenied,
                format!("无法创建临时压缩目录: {error}"),
            )
        })?;
        let temporary_tar = temporary_dir.join(format!("{stem}.tar"));
        let tar_args = vec![
            "a".into(),
            "-ttar".into(),
            temporary_tar.to_string_lossy().into_owned(),
        ]
        .into_iter()
        .chain(
            request
                .inputs
                .iter()
                .map(|path| path.to_string_lossy().into_owned()),
        )
        .collect();
        let tar_result = self
            .invoke(
                ArchiveOperation::Create,
                tar_args,
                Arc::clone(&progress),
                cancellation.child_token(),
            )
            .await;
        if let Err(error) = tar_result {
            let _ = fs::remove_dir_all(&temporary_dir);
            return Err(error);
        }
        let wrapper_result = self
            .invoke(
                ArchiveOperation::Create,
                vec![
                    "a".into(),
                    format!("-t{compression}"),
                    request.output.to_string_lossy().into_owned(),
                    temporary_tar.to_string_lossy().into_owned(),
                ],
                progress,
                cancellation,
            )
            .await;
        let _ = fs::remove_dir_all(&temporary_dir);
        let wrapper = wrapper_result?;
        Ok(ArchiveResult {
            output: Some(request.output),
            entries: vec![],
            warnings: wrapper
                .warning
                .then(|| "7-Zip reported a warning".into())
                .into_iter()
                .collect(),
        })
    }

    async fn expand_tar_wrapper(
        &self,
        archive: &Path,
        cancellation: CancellationToken,
    ) -> Result<(PathBuf, PathBuf), ArchiveError> {
        let directory = std::env::temp_dir().join(format!("qzip-expand-{}", Uuid::new_v4()));
        fs::create_dir_all(&directory).map_err(|_| {
            ArchiveError::new(
                ArchiveErrorCode::PermissionDenied,
                "无法创建复合压缩包临时目录",
            )
        })?;
        let result = self
            .invoke(
                ArchiveOperation::Extract,
                vec![
                    "e".into(),
                    archive.to_string_lossy().into_owned(),
                    format!("-o{}", directory.to_string_lossy()),
                    "-y".into(),
                ],
                Arc::new(archive_core::NoopProgressReporter),
                cancellation,
            )
            .await;
        if let Err(error) = result {
            let _ = fs::remove_dir_all(&directory);
            return Err(error);
        }
        let entries = fs::read_dir(&directory)
            .map_err(|_| {
                ArchiveError::new(
                    ArchiveErrorCode::CleanupFailed,
                    "无法读取复合压缩包临时目录",
                )
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| {
                ArchiveError::new(
                    ArchiveErrorCode::CleanupFailed,
                    "无法读取复合压缩包临时目录",
                )
            })?;
        let Some(entry) = entries.first() else {
            let _ = fs::remove_dir_all(&directory);
            return Err(ArchiveError::new(
                ArchiveErrorCode::CorruptArchive,
                "复合压缩包不包含可读取的 TAR 内容",
            ));
        };
        let is_single_tar = entries.len() == 1
            && entry
                .file_type()
                .map(|kind| kind.is_file() && !kind.is_symlink())
                .unwrap_or(false)
            && entry
                .path()
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("tar"));
        if !is_single_tar {
            let _ = fs::remove_dir_all(&directory);
            return Err(ArchiveError::new(
                ArchiveErrorCode::CorruptArchive,
                "复合压缩包不包含唯一且安全的 TAR 内容",
            ));
        }
        Ok((directory, entry.path()))
    }

    async fn list_plain(
        &self,
        request: ListArchiveRequest,
        cancellation: CancellationToken,
    ) -> Result<Vec<ArchiveEntry>, ArchiveError> {
        let supplied_password = request.password.is_some();
        self.invoke_listing(SevenZipArgumentMapper::list(&request), cancellation)
            .await
            .map_err(|error| map_read_failure(error, supplied_password))
    }

    async fn list_tar_wrapper_streaming(
        &self,
        archive: &Path,
        cancellation: CancellationToken,
    ) -> Result<Vec<ArchiveEntry>, ArchiveError> {
        if !self.executable.is_file() {
            return Err(ArchiveError::unavailable("7-Zip sidecar was not found"));
        }
        self.verify_runtime_files()?;

        let mut extractor = Command::new(&self.executable);
        extractor
            .args([
                "x".to_owned(),
                archive.to_string_lossy().into_owned(),
                "-so".to_owned(),
                "-y".to_owned(),
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        #[cfg(target_os = "windows")]
        extractor.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        let mut extractor = extractor.spawn().map_err(|error| {
            ArchiveError::unavailable(format!("could not start 7-Zip: {error}"))
        })?;

        let mut lister = Command::new(&self.executable);
        lister
            .args([
                "l".to_owned(),
                "-slt".to_owned(),
                "-sccUTF-8".to_owned(),
                "-ttar".to_owned(),
                "-si".to_owned(),
                "-an".to_owned(),
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        #[cfg(target_os = "windows")]
        lister.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        let mut lister = match lister.spawn() {
            Ok(child) => child,
            Err(error) => {
                let _ = extractor.start_kill();
                let _ = extractor.wait().await;
                return Err(ArchiveError::unavailable(format!(
                    "could not start 7-Zip TAR lister: {error}"
                )));
            }
        };

        let extractor_stdout = extractor
            .stdout
            .take()
            .ok_or_else(|| ArchiveError::unavailable("could not capture 7-Zip archive stream"))?;
        let mut lister_stdin = lister
            .stdin
            .take()
            .ok_or_else(|| ArchiveError::unavailable("could not open 7-Zip TAR lister input"))?;
        let pump_task = tokio::spawn(async move {
            let result =
                tokio::io::copy(&mut BufReader::new(extractor_stdout), &mut lister_stdin).await;
            drop(lister_stdin);
            result
        });

        let (sender, mut receiver) = mpsc::unbounded_channel();
        let extractor_stderr_task = spawn_process_line_reader(
            extractor.stderr.take().ok_or_else(|| {
                ArchiveError::unavailable("could not capture 7-Zip extractor diagnostics")
            })?,
            sender.clone(),
            ProcessStream::Stderr,
        );
        let lister_stdout_task = spawn_process_line_reader(
            lister
                .stdout
                .take()
                .ok_or_else(|| ArchiveError::unavailable("could not capture 7-Zip TAR listing"))?,
            sender.clone(),
            ProcessStream::Stdout,
        );
        let lister_stderr_task = spawn_process_line_reader(
            lister.stderr.take().ok_or_else(|| {
                ArchiveError::unavailable("could not capture 7-Zip TAR lister diagnostics")
            })?,
            sender.clone(),
            ProcessStream::Stderr,
        );
        drop(sender);

        let mut state = InvocationState::new(InvocationMode::Listing);
        let reporter = archive_core::NoopProgressReporter;
        let mut extractor_status = None;
        let mut lister_status = None;
        let mut cancelled = false;
        while extractor_status.is_none() || lister_status.is_none() {
            tokio::select! {
                _ = cancellation.cancelled() => {
                    cancelled = true;
                    let _ = extractor.start_kill();
                    let _ = lister.start_kill();
                    break;
                }
                status = extractor.wait(), if extractor_status.is_none() => {
                    extractor_status = Some(status);
                }
                status = lister.wait(), if lister_status.is_none() => {
                    if let Ok(status) = &status
                        && !status.success()
                        && status.code() != Some(1)
                    {
                        let _ = extractor.start_kill();
                    }
                    lister_status = Some(status);
                }
                Some(line) = receiver.recv() => {
                    state.consume(line, ArchiveOperation::List, &reporter);
                }
            }
        }
        if cancelled {
            let _ = extractor.wait().await;
            let _ = lister.wait().await;
        }
        let _ = extractor_stderr_task.await;
        let _ = lister_stdout_task.await;
        let _ = lister_stderr_task.await;
        let _ = pump_task.await;
        while let Some(line) = receiver.recv().await {
            state.consume(line, ArchiveOperation::List, &reporter);
        }
        if cancelled {
            return Err(ArchiveError::new(
                ArchiveErrorCode::Cancelled,
                "archive operation was cancelled",
            ));
        }

        let (diagnostics, entries) = state.finish();
        for status in [extractor_status, lister_status] {
            match status {
                Some(Ok(status)) if status.success() || status.code() == Some(1) => {}
                Some(Ok(status)) => {
                    return Err(map_read_failure(
                        map_exit(status.code(), &diagnostics),
                        false,
                    ));
                }
                Some(Err(error)) => {
                    return Err(map_read_failure(
                        ArchiveError::new(
                            ArchiveErrorCode::Unknown,
                            format!("7-Zip process failed: {error}"),
                        ),
                        false,
                    ));
                }
                None => {
                    return Err(ArchiveError::new(
                        ArchiveErrorCode::Unknown,
                        "7-Zip process ended without an exit status",
                    ));
                }
            }
        }
        Ok(entries.unwrap_or_default())
    }

    async fn test_plain(
        &self,
        request: TestArchiveRequest,
        cancellation: CancellationToken,
    ) -> Result<TestResult, ArchiveError> {
        let supplied_password = request.password.is_some();
        let result = self
            .invoke(
                ArchiveOperation::Test,
                SevenZipArgumentMapper::test(&request),
                Arc::new(archive_core::NoopProgressReporter),
                cancellation,
            )
            .await
            .map_err(|error| map_read_failure(error, supplied_password))?;
        Ok(TestResult {
            valid: true,
            warnings: result
                .warning
                .then(|| "7-Zip reported a warning".to_owned())
                .into_iter()
                .collect(),
        })
    }

    async fn extract_plain(
        &self,
        request: ExtractArchiveRequest,
        progress: Arc<dyn ProgressReporter>,
        cancellation: CancellationToken,
    ) -> Result<ArchiveResult, ArchiveError> {
        let supplied_password = request.password.is_some();
        let result = self
            .invoke(
                ArchiveOperation::Extract,
                SevenZipArgumentMapper::extract(&request),
                progress,
                cancellation,
            )
            .await
            .map_err(|error| map_read_failure(error, supplied_password))?;
        Ok(ArchiveResult {
            output: Some(request.output),
            entries: vec![],
            warnings: result
                .warning
                .then(|| "7-Zip reported a warning".to_owned())
                .into_iter()
                .collect(),
        })
    }
}

fn verify_sha256(path: &Path, expected: &str) -> Result<(), ArchiveError> {
    let mut file = fs::File::open(path)
        .map_err(|_| ArchiveError::unavailable("7-Zip sidecar was not found"))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|_| {
            ArchiveError::unavailable("could not read the 7-Zip sidecar for verification")
        })?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let actual = format!("{:x}", hasher.finalize());
    if actual != expected {
        return Err(ArchiveError::unavailable(
            "7-Zip sidecar integrity verification failed",
        ));
    }
    Ok(())
}

#[async_trait]
impl ArchiveBackend for SevenZipCliBackend {
    fn id(&self) -> &'static str {
        "sevenzip"
    }
    async fn capabilities(&self) -> Result<BackendCapabilities, ArchiveError> {
        let version = self.check_version().await?;
        Ok(BackendCapabilities {
            backend_id: self.id().into(),
            version,
            writable_formats: vec![
                ArchiveFormat::SevenZip,
                ArchiveFormat::Zip,
                ArchiveFormat::Tar,
                ArchiveFormat::TarGz,
                ArchiveFormat::TarXz,
            ],
            readable_formats: vec![
                ArchiveFormat::SevenZip,
                ArchiveFormat::Zip,
                ArchiveFormat::Rar,
                ArchiveFormat::Tar,
                ArchiveFormat::TarGz,
                ArchiveFormat::TarXz,
                ArchiveFormat::Gz,
                ArchiveFormat::Xz,
                ArchiveFormat::Bz2,
                ArchiveFormat::Iso,
                ArchiveFormat::Cab,
                ArchiveFormat::Wim,
            ],
            supports_password: true,
            supports_header_encryption: true,
            supports_partial_extract: true,
            supports_update: true,
            supports_progress: true,
            supports_cancellation: true,
        })
    }
    async fn create(
        &self,
        request: CreateArchiveRequest,
        progress: Arc<dyn ProgressReporter>,
        cancellation: CancellationToken,
    ) -> Result<ArchiveResult, ArchiveError> {
        if request.inputs.is_empty() {
            return Err(ArchiveError::invalid_option(
                "inputs",
                "at least one input is required",
            ));
        }
        if !matches!(
            request.format,
            ArchiveFormat::SevenZip
                | ArchiveFormat::Zip
                | ArchiveFormat::Tar
                | ArchiveFormat::TarGz
                | ArchiveFormat::TarXz
        ) {
            return Err(ArchiveError::new(
                ArchiveErrorCode::UnsupportedOption,
                "this backend cannot create the requested archive format",
            ));
        }
        if request.password.is_some()
            && matches!(
                request.format,
                ArchiveFormat::Tar | ArchiveFormat::TarGz | ArchiveFormat::TarXz
            )
        {
            return Err(ArchiveError::new(
                ArchiveErrorCode::UnsupportedOption,
                "TAR 格式不支持密码保护，请改用 7Z 或 ZIP",
            ));
        }
        if matches!(request.format, ArchiveFormat::TarGz | ArchiveFormat::TarXz) {
            return self
                .create_tar_compressed(request, progress, cancellation)
                .await;
        }
        let args = SevenZipArgumentMapper::create(&request);
        let result = self
            .invoke(ArchiveOperation::Create, args, progress, cancellation)
            .await?;
        Ok(ArchiveResult {
            output: Some(request.output),
            entries: vec![],
            warnings: result
                .warning
                .then(|| "7-Zip reported a warning".into())
                .into_iter()
                .collect(),
        })
    }
    async fn extract(
        &self,
        request: ExtractArchiveRequest,
        progress: Arc<dyn ProgressReporter>,
        cancellation: CancellationToken,
    ) -> Result<ArchiveResult, ArchiveError> {
        if request.conflict_policy == ConflictPolicy::Ask {
            return Err(ArchiveError::new(
                ArchiveErrorCode::UnsupportedOption,
                "interactive conflict prompts are not available in M1",
            ));
        }
        // List before extraction: reject a dangerous entry before 7-Zip writes it.
        for entry in self
            .list(
                ListArchiveRequest {
                    archive: request.archive.clone(),
                    password: request.password.clone(),
                },
                cancellation.child_token(),
            )
            .await?
        {
            safe_relative_path(&entry.path).map_err(|error| {
                ArchiveError::new(ArchiveErrorCode::UnsafePath, error.to_string())
            })?;
        }
        if is_tar_wrapper(&request.archive) {
            if request.password.is_some() {
                return Err(ArchiveError::new(
                    ArchiveErrorCode::UnsupportedOption,
                    "TAR.GZ 和 TAR.XZ 不支持密码参数",
                ));
            }
            let (temporary_dir, inner_tar) = self
                .expand_tar_wrapper(&request.archive, cancellation.child_token())
                .await?;
            let inner_request = ExtractArchiveRequest {
                archive: inner_tar,
                output: request.output,
                selected_entries: request.selected_entries,
                conflict_policy: request.conflict_policy,
                password: None,
            };
            let result = self
                .extract_plain(inner_request, progress, cancellation)
                .await;
            let _ = fs::remove_dir_all(temporary_dir);
            return result;
        }
        self.extract_plain(request, progress, cancellation).await
    }
    async fn list(
        &self,
        request: ListArchiveRequest,
        cancellation: CancellationToken,
    ) -> Result<Vec<ArchiveEntry>, ArchiveError> {
        if is_tar_wrapper(&request.archive) {
            if request.password.is_some() {
                return Err(ArchiveError::new(
                    ArchiveErrorCode::UnsupportedOption,
                    "TAR.GZ 和 TAR.XZ 不支持密码参数",
                ));
            }
            return self
                .list_tar_wrapper_streaming(&request.archive, cancellation)
                .await;
        }
        self.list_plain(request, cancellation).await
    }
    async fn test(
        &self,
        request: TestArchiveRequest,
        cancellation: CancellationToken,
    ) -> Result<TestResult, ArchiveError> {
        if is_tar_wrapper(&request.archive) {
            if request.password.is_some() {
                return Err(ArchiveError::new(
                    ArchiveErrorCode::UnsupportedOption,
                    "TAR.GZ 和 TAR.XZ 不支持密码参数",
                ));
            }
            let outer = self
                .test_plain(request.clone(), cancellation.child_token())
                .await?;
            let (temporary_dir, inner_tar) = self
                .expand_tar_wrapper(&request.archive, cancellation.child_token())
                .await?;
            let inner = self
                .test_plain(
                    TestArchiveRequest {
                        archive: inner_tar,
                        password: None,
                    },
                    cancellation,
                )
                .await;
            let _ = fs::remove_dir_all(temporary_dir);
            let inner = inner?;
            return Ok(TestResult {
                valid: outer.valid && inner.valid,
                warnings: outer.warnings.into_iter().chain(inner.warnings).collect(),
            });
        }
        self.test_plain(request, cancellation).await
    }
    async fn update(
        &self,
        request: UpdateArchiveRequest,
        progress: Arc<dyn ProgressReporter>,
        cancellation: CancellationToken,
    ) -> Result<ArchiveResult, ArchiveError> {
        if request.inputs.is_empty() {
            return Err(ArchiveError::invalid_option(
                "inputs",
                "at least one input is required",
            ));
        }
        let result = self
            .invoke(
                ArchiveOperation::Update,
                SevenZipArgumentMapper::update(&request),
                progress,
                cancellation,
            )
            .await?;
        Ok(ArchiveResult {
            output: Some(request.archive),
            entries: vec![],
            warnings: result
                .warning
                .then(|| "7-Zip reported a warning".to_owned())
                .into_iter()
                .collect(),
        })
    }
}

fn is_tar_wrapper(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            let name = name.to_ascii_lowercase();
            name.ends_with(".tar.gz")
                || name.ends_with(".tar.xz")
                || name.ends_with(".tgz")
                || name.ends_with(".txz")
        })
}

pub struct SevenZipArgumentMapper;
impl SevenZipArgumentMapper {
    pub fn create(request: &CreateArchiveRequest) -> Vec<String> {
        let mut args = vec![
            "a".into(),
            format!("-t{}", request.format.extension()),
            match request.profile {
                CompressionProfile::Store => "-mx=0",
                CompressionProfile::Fast => "-mx=1",
                CompressionProfile::Balanced => "-mx=5",
                CompressionProfile::Small => "-mx=9",
                CompressionProfile::Maximum => "-mx=9",
            }
            .into(),
        ];
        if let Some(password) = &request.password {
            args.push(format!("-p{}", password.expose_secret()));
            if request.encrypt_headers && request.format == ArchiveFormat::SevenZip {
                args.push("-mhe=on".into());
            }
        }
        args.push(request.output.to_string_lossy().into_owned());
        args.extend(
            request
                .inputs
                .iter()
                .map(|path| path.to_string_lossy().into_owned()),
        );
        args
    }
    pub fn extract(request: &ExtractArchiveRequest) -> Vec<String> {
        let mut args = vec![
            "x".into(),
            request.archive.to_string_lossy().into_owned(),
            format!("-o{}", request.output.to_string_lossy()),
            match request.conflict_policy {
                ConflictPolicy::Rename => "-aou",
                ConflictPolicy::Overwrite => "-aoa",
                ConflictPolicy::Skip => "-aos",
                ConflictPolicy::Ask => unreachable!(),
            }
            .into(),
        ];
        if let Some(password) = &request.password {
            args.push(format!("-p{}", password.expose_secret()));
        }
        if let Some(entries) = &request.selected_entries {
            args.extend(entries.iter().cloned());
        }
        args
    }
    pub fn list(request: &ListArchiveRequest) -> Vec<String> {
        let mut args = vec![
            "l".into(),
            "-slt".into(),
            "-sccUTF-8".into(),
            request.archive.to_string_lossy().into_owned(),
        ];
        if let Some(password) = &request.password {
            args.push(format!("-p{}", password.expose_secret()));
        }
        args
    }
    pub fn test(request: &TestArchiveRequest) -> Vec<String> {
        let mut args = vec!["t".into(), request.archive.to_string_lossy().into_owned()];
        if let Some(password) = &request.password {
            args.push(format!("-p{}", password.expose_secret()));
        }
        args
    }
    pub fn update(request: &UpdateArchiveRequest) -> Vec<String> {
        let mut args = vec!["a".into(), request.archive.to_string_lossy().into_owned()];
        if let Some(password) = &request.password {
            args.push(format!("-p{}", password.expose_secret()));
        }
        args.extend(
            request
                .inputs
                .iter()
                .map(|path| path.to_string_lossy().into_owned()),
        );
        args
    }
}

#[derive(Default)]
pub struct SevenZipListParser {
    entries: Vec<ArchiveEntry>,
    pending: PendingEntry,
}

#[derive(Default)]
struct PendingEntry {
    path: Option<String>,
    size: Option<u64>,
    packed: Option<u64>,
    attributes: Option<String>,
    modified: Option<String>,
    crc: Option<String>,
    encrypted: bool,
    symlink: bool,
    hardlink: bool,
}

impl SevenZipListParser {
    pub fn parse(output: &str) -> Vec<ArchiveEntry> {
        let mut parser = Self::default();
        for line in output.lines() {
            parser.push_line(line);
        }
        parser.finish()
    }

    pub fn push_line(&mut self, line: &str) {
        if line.trim().is_empty() {
            self.flush_pending();
            return;
        }
        if let Some((key, value)) = line.split_once(" = ") {
            match key {
                "Path" => self.pending.path = Some(value.into()),
                "Size" => self.pending.size = value.parse().ok(),
                "Packed Size" => self.pending.packed = value.parse().ok(),
                "Attributes" => self.pending.attributes = Some(value.into()),
                "Modified" => self.pending.modified = Some(value.into()),
                "CRC" => self.pending.crc = Some(value.into()),
                "Encrypted" => self.pending.encrypted = value == "+",
                "Symbolic Link" => self.pending.symlink = true,
                "Hard Link" => self.pending.hardlink = true,
                _ => {}
            }
        }
    }

    pub fn finish(mut self) -> Vec<ArchiveEntry> {
        self.flush_pending();
        self.entries
    }

    fn flush_pending(&mut self) {
        if let (Some(path), Some(size)) = (self.pending.path.take(), self.pending.size.take()) {
            let path = path.replace('\\', "/");
            let display_name = path.rsplit('/').next().unwrap_or(&path).to_owned();
            self.entries.push(ArchiveEntry {
                display_name,
                is_directory: self
                    .pending
                    .attributes
                    .as_deref()
                    .is_some_and(|value| value.contains('D')),
                path,
                size,
                compressed_size: self.pending.packed.take(),
                modified_at: self.pending.modified.take(),
                crc: self.pending.crc.take(),
                attributes: self.pending.attributes.take(),
                encrypted: self.pending.encrypted,
                is_symlink: self.pending.symlink,
                is_hardlink: self.pending.hardlink,
            });
        }
        self.pending = PendingEntry::default();
    }
}

#[derive(Debug)]
struct InvocationOutput {
    output: String,
    warning: bool,
    entries: Option<Vec<ArchiveEntry>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InvocationMode {
    Diagnostic,
    Listing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProcessStream {
    Stdout,
    Stderr,
}

#[derive(Debug)]
struct ProcessLine {
    stream: ProcessStream,
    line: String,
}

fn spawn_process_line_reader<R>(
    reader: R,
    sender: mpsc::UnboundedSender<ProcessLine>,
    stream: ProcessStream,
) -> tokio::task::JoinHandle<()>
where
    R: AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut lines = BufReader::new(reader).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if sender.send(ProcessLine { stream, line }).is_err() {
                break;
            }
        }
    })
}

struct InvocationState {
    mode: InvocationMode,
    listing: Option<SevenZipListParser>,
    output: String,
    last_percent: Option<u8>,
    last_report: Instant,
}

impl InvocationState {
    fn new(mode: InvocationMode) -> Self {
        Self {
            mode,
            listing: (mode == InvocationMode::Listing).then(SevenZipListParser::default),
            output: String::new(),
            last_percent: None,
            last_report: Instant::now() - Duration::from_secs(1),
        }
    }

    fn consume(
        &mut self,
        line: ProcessLine,
        operation: ArchiveOperation,
        progress: &dyn ProgressReporter,
    ) {
        if self.mode == InvocationMode::Listing
            && line.stream == ProcessStream::Stdout
            && let Some(parser) = self.listing.as_mut()
        {
            parser.push_line(&line.line);
        }
        if self.mode != InvocationMode::Listing || line.stream == ProcessStream::Stderr {
            append_bounded(&mut self.output, &line.line);
        }
        if let Some(percent) = parse_progress(&line.line)
            && self.last_percent != Some(percent)
            && self.last_report.elapsed() >= Duration::from_millis(100)
        {
            progress.report(TaskProgress {
                operation,
                percent: Some(percent),
                detail: "7-Zip is working".into(),
            });
            self.last_percent = Some(percent);
            self.last_report = Instant::now();
        }
    }

    fn finish(self) -> (String, Option<Vec<ArchiveEntry>>) {
        (self.output, self.listing.map(SevenZipListParser::finish))
    }
}

fn append_bounded(target: &mut String, line: &str) {
    if target.len() < MAX_DIAGNOSTIC_BYTES {
        let available = MAX_DIAGNOSTIC_BYTES - target.len();
        let mut end = line.len().min(available);
        while end > 0 && !line.is_char_boundary(end) {
            end -= 1;
        }
        target.push_str(&line[..end]);
        if target.len() < MAX_DIAGNOSTIC_BYTES {
            target.push('\n');
        }
    }
}
fn parse_progress(line: &str) -> Option<u8> {
    line.split_whitespace().find_map(|token| {
        token
            .strip_suffix('%')
            .and_then(|value| value.parse::<u8>().ok())
            .filter(|value| *value <= 100)
    })
}
fn map_exit(code: Option<i32>, output: &str) -> ArchiveError {
    let lower = output.to_ascii_lowercase();
    let kind = if lower.contains("wrong password") || lower.contains("password is not correct") {
        ArchiveErrorCode::WrongPassword
    } else if lower.contains("not enough space") || lower.contains("disk full") {
        ArchiveErrorCode::DiskFull
    } else if lower.contains("used by another process") {
        ArchiveErrorCode::FileInUse
    } else if lower.contains("access is denied") {
        ArchiveErrorCode::AccessDenied
    } else if lower.contains("cannot open")
        || lower.contains("is not supported")
        || lower.contains("data error")
    {
        ArchiveErrorCode::CorruptArchive
    } else {
        ArchiveErrorCode::Unknown
    };
    ArchiveError::new(
        kind,
        format!("7-Zip failed with exit code {}", code.unwrap_or(-1)),
    )
}

/// Some Windows 7-Zip builds write password failures directly to the console
/// instead of the redirected stdout/stderr handles. An otherwise unclassified
/// failure during an operation that supplied a password is therefore reported
/// as retryable `WRONG_PASSWORD`, never with a raw diagnostic.
fn map_password_failure(error: ArchiveError, supplied_password: bool) -> ArchiveError {
    if supplied_password && error.code == ArchiveErrorCode::Unknown {
        ArchiveError::new(
            ArchiveErrorCode::WrongPassword,
            "7-Zip rejected the supplied password",
        )
    } else {
        error
    }
}

fn map_read_failure(error: ArchiveError, supplied_password: bool) -> ArchiveError {
    let error = map_password_failure(error, supplied_password);
    if error.code == ArchiveErrorCode::Unknown {
        ArchiveError::new(
            ArchiveErrorCode::CorruptArchive,
            "7-Zip could not read the archive",
        )
    } else {
        error
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mapper_keeps_paths_as_separate_arguments() {
        let request = CreateArchiveRequest {
            format: ArchiveFormat::SevenZip,
            output: PathBuf::from("out name.7z"),
            inputs: vec![PathBuf::from("中文 file.txt")],
            profile: CompressionProfile::Balanced,
            password: None,
            encrypt_headers: false,
            test_after_create: false,
        };
        let args = SevenZipArgumentMapper::create(&request);
        assert_eq!(args[3], "out name.7z");
        assert_eq!(args[4], "中文 file.txt");
    }
    #[test]
    fn parses_technical_listing() {
        let text = "Path = a/b.txt\nSize = 12\nPacked Size = 9\nAttributes = A\n\n";
        assert_eq!(
            SevenZipListParser::parse(text),
            vec![ArchiveEntry {
                path: "a/b.txt".into(),
                display_name: "b.txt".into(),
                size: 12,
                compressed_size: Some(9),
                is_directory: false,
                modified_at: None,
                crc: None,
                attributes: Some("A".into()),
                encrypted: false,
                is_symlink: false,
                is_hardlink: false,
            }]
        );
    }
    #[test]
    fn normalizes_windows_separators_in_technical_listing() {
        let text = "Path = 附件\\封面.docx\nSize = 12\nAttributes = A\n\n";
        let entries = SevenZipListParser::parse(text);

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path, "附件/封面.docx");
        assert_eq!(entries[0].display_name, "封面.docx");
    }
    #[test]
    fn streaming_parser_keeps_large_utf8_listing_complete() {
        let mut parser = SevenZipListParser::default();
        for index in 0..1_500 {
            parser.push_line(&format!("Path = 文档/第 {index} 项.txt"));
            parser.push_line("Size = 12");
            parser.push_line("");
        }

        let entries = parser.finish();
        assert_eq!(entries.len(), 1_500);
        assert_eq!(entries[0].path, "文档/第 0 项.txt");
        assert_eq!(entries[1_499].path, "文档/第 1499 项.txt");
    }
    #[test]
    fn listing_mode_keeps_stdout_outside_bounded_diagnostics() {
        let mut state = InvocationState::new(InvocationMode::Listing);
        let reporter = archive_core::NoopProgressReporter;
        for index in 0..1_500 {
            state.consume(
                ProcessLine {
                    stream: ProcessStream::Stdout,
                    line: format!("Path = 文档/第 {index} 项.txt"),
                },
                ArchiveOperation::List,
                &reporter,
            );
            state.consume(
                ProcessLine {
                    stream: ProcessStream::Stdout,
                    line: "Size = 12".into(),
                },
                ArchiveOperation::List,
                &reporter,
            );
            state.consume(
                ProcessLine {
                    stream: ProcessStream::Stdout,
                    line: String::new(),
                },
                ArchiveOperation::List,
                &reporter,
            );
        }

        let (diagnostics, entries) = state.finish();
        assert!(diagnostics.is_empty());
        assert_eq!(entries.expect("listing parser is enabled").len(), 1_500);
    }
    #[test]
    fn bounded_diagnostics_never_split_utf8_or_exceed_limit() {
        let mut diagnostics = String::new();
        append_bounded(&mut diagnostics, &"中文错误 ".repeat(MAX_DIAGNOSTIC_BYTES));

        assert!(diagnostics.len() <= MAX_DIAGNOSTIC_BYTES);
        assert!(std::str::from_utf8(diagnostics.as_bytes()).is_ok());
    }
    #[test]
    fn list_requests_utf8_console_output() {
        let request = ListArchiveRequest {
            archive: PathBuf::from("中文.zip"),
            password: None,
        };

        assert_eq!(
            SevenZipArgumentMapper::list(&request),
            vec!["l", "-slt", "-sccUTF-8", "中文.zip"]
        );
    }
    #[test]
    fn treats_tgz_and_txz_as_tar_wrappers() {
        assert!(is_tar_wrapper(Path::new("release.tgz")));
        assert!(is_tar_wrapper(Path::new("RELEASE.TXZ")));
        assert!(!is_tar_wrapper(Path::new("payload.gz")));
        assert!(!is_tar_wrapper(Path::new("payload.xz")));
    }
    #[cfg(target_os = "windows")]
    #[tokio::test]
    async fn tar_wrapper_listing_streams_without_creating_an_expand_directory() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let backend = SevenZipCliBackend::new(root.join("third_party/7zip/bin/win-x64/7z.exe"));
        let archive = root.join("tests/fixtures/compat/windows-bsdtar-xz.tar.xz");
        let temporary_root = std::env::temp_dir();
        let before = expand_directories(&temporary_root);

        let entries = backend
            .list(
                ListArchiveRequest {
                    archive,
                    password: None,
                },
                CancellationToken::new(),
            )
            .await
            .expect("streaming TAR listing succeeds");

        assert!(
            entries
                .iter()
                .any(|entry| entry.path.ends_with("hello.txt"))
        );
        assert_eq!(before, expand_directories(&temporary_root));
    }
    #[cfg(target_os = "windows")]
    #[tokio::test]
    async fn txz_alias_listing_uses_the_tar_wrapper_stream() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let backend = SevenZipCliBackend::new(root.join("third_party/7zip/bin/win-x64/7z.exe"));
        let source = root.join("tests/fixtures/compat/windows-bsdtar-xz.tar.xz");
        let archive = std::env::temp_dir().join(format!("qzip-alias-{}.txz", Uuid::new_v4()));
        fs::copy(&source, &archive).expect("copy fixture with TXZ alias");
        let temporary_root = std::env::temp_dir();
        let before = expand_directories(&temporary_root);

        let entries = backend
            .list(
                ListArchiveRequest {
                    archive: archive.clone(),
                    password: None,
                },
                CancellationToken::new(),
            )
            .await
            .expect("TXZ alias listing succeeds");

        assert!(
            entries
                .iter()
                .any(|entry| entry.path.ends_with("hello.txt"))
        );
        assert_eq!(before, expand_directories(&temporary_root));
        fs::remove_file(archive).expect("remove TXZ alias fixture");
    }
    #[cfg(target_os = "windows")]
    fn expand_directories(root: &Path) -> Vec<PathBuf> {
        let mut directories = fs::read_dir(root)
            .expect("temporary directory is readable")
            .filter_map(Result::ok)
            .filter(|entry| {
                entry.file_type().is_ok_and(|kind| kind.is_dir())
                    && entry
                        .file_name()
                        .to_string_lossy()
                        .starts_with("qzip-expand-")
            })
            .map(|entry| entry.path())
            .collect::<Vec<_>>();
        directories.sort();
        directories
    }
    #[test]
    fn maps_password_without_echoing_details() {
        let error = map_exit(Some(2), "ERROR: Wrong password");
        assert_eq!(error.code, ArchiveErrorCode::WrongPassword);
        assert!(!error.message.contains("password"));
    }
    #[test]
    fn maps_known_failures_to_safe_codes() {
        assert_eq!(
            map_exit(Some(2), "ERROR: Not enough space").code,
            ArchiveErrorCode::DiskFull
        );
        assert_eq!(
            map_exit(Some(2), "ERROR: Data Error").code,
            ArchiveErrorCode::CorruptArchive
        );
        assert_eq!(
            map_exit(Some(2), "unexpected failure").code,
            ArchiveErrorCode::Unknown
        );
    }
    #[test]
    fn parses_progress_without_localized_labels() {
        assert_eq!(parse_progress("  73% - file name"), Some(73));
        assert_eq!(parse_progress("no percentage"), None);
    }
    #[test]
    fn classifies_silent_password_failure() {
        let error = map_password_failure(
            ArchiveError::new(ArchiveErrorCode::Unknown, "7-Zip failed with exit code 2"),
            true,
        );
        assert_eq!(error.code, ArchiveErrorCode::WrongPassword);
        assert!(!error.message.contains("exit code"));
    }
    #[test]
    fn classifies_silent_read_failure_as_corrupt() {
        let error = map_read_failure(
            ArchiveError::new(ArchiveErrorCode::Unknown, "7-Zip failed with exit code 2"),
            false,
        );
        assert_eq!(error.code, ArchiveErrorCode::CorruptArchive);
    }
    #[cfg(windows)]
    #[tokio::test]
    async fn cancellation_kills_a_blocking_child_process() {
        let executable = std::env::var_os("SystemRoot")
            .map(PathBuf::from)
            .expect("SystemRoot is available on Windows")
            .join("System32")
            .join("ping.exe");
        let backend = SevenZipCliBackend::new(executable);
        let cancellation = CancellationToken::new();
        let trigger = cancellation.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            trigger.cancel();
        });
        let result = backend
            .invoke_process(
                ArchiveOperation::Test,
                vec!["-n".into(), "20".into(), "127.0.0.1".into()],
                Arc::new(archive_core::NoopProgressReporter),
                cancellation,
                InvocationMode::Diagnostic,
            )
            .await;
        assert_eq!(result.unwrap_err().code, ArchiveErrorCode::Cancelled);
    }
}
