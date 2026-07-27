#![forbid(unsafe_code)]

//! 7-Zip command-line adapter. Arguments are always passed as an argv array;
//! neither archive names nor user paths are ever interpolated into a shell.

use std::{
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
    time::Duration,
};

use archive_core::{
    ArchiveBackend, ArchiveEntry, ArchiveError, ArchiveErrorCode, ArchiveFormat, ArchiveOperation,
    ArchiveResult, BackendCapabilities, CompressionProfile, ConflictPolicy, CreateArchiveRequest,
    ExtractArchiveRequest, ListArchiveRequest, ProgressReporter, TaskProgress, TestArchiveRequest,
    TestResult,
};
use archive_security::safe_relative_path;
use async_trait::async_trait;
use secrecy::ExposeSecret;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
    sync::mpsc,
    time::Instant,
};
use tokio_util::sync::CancellationToken;

const EXPECTED_VERSION: &str = "26.02";
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
        let mut child = Command::new(&self.executable)
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| {
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
        let forward = |stream: tokio::process::ChildStdout,
                       sender: mpsc::UnboundedSender<String>| async move {
            let mut lines = BufReader::new(stream).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = sender.send(line);
            }
        };
        let stdout_task = tokio::spawn(forward(stdout, sender.clone()));
        let stderr_sender = sender.clone();
        let stderr_task = tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = stderr_sender.send(line);
            }
        });
        drop(sender);
        let mut output = String::new();
        let mut last_percent = None;
        let mut last_report = Instant::now() - Duration::from_secs(1);
        loop {
            tokio::select! {
                _ = cancellation.cancelled() => { let _ = child.start_kill(); let _ = child.wait().await; let _ = stdout_task.await; let _ = stderr_task.await; return Err(ArchiveError::new(ArchiveErrorCode::Cancelled, "archive operation was cancelled")); }
                Some(line) = receiver.recv() => {
                    append_bounded(&mut output, &line);
                    if let Some(percent) = parse_progress(&line)
                        && last_percent != Some(percent)
                        && last_report.elapsed() >= Duration::from_millis(100)
                    {
                        progress.report(TaskProgress { operation, percent: Some(percent), detail: "7-Zip is working".into() });
                        last_percent = Some(percent);
                        last_report = Instant::now();
                    }
                }
                status = child.wait() => { let status = status.map_err(|error| ArchiveError::new(ArchiveErrorCode::Unknown, format!("7-Zip process failed: {error}")))?; while let Ok(line) = receiver.try_recv() { append_bounded(&mut output, &line); } let _ = stdout_task.await; let _ = stderr_task.await; if status.success() || status.code() == Some(1) { return Ok(InvocationOutput { output, warning: status.code() == Some(1) }); } return Err(map_exit(status.code(), &output)); }
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
            writable_formats: vec![ArchiveFormat::SevenZip, ArchiveFormat::Zip],
            readable_formats: vec![
                ArchiveFormat::SevenZip,
                ArchiveFormat::Zip,
                ArchiveFormat::Rar,
            ],
            supports_password: true,
            supports_header_encryption: true,
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
        if !matches!(request.format, ArchiveFormat::SevenZip | ArchiveFormat::Zip) {
            return Err(ArchiveError::new(
                ArchiveErrorCode::UnsupportedOption,
                "M1 can only create 7z and zip archives",
            ));
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
        let result = self
            .invoke(
                ArchiveOperation::Extract,
                SevenZipArgumentMapper::extract(&request),
                progress,
                cancellation,
            )
            .await?;
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
    async fn list(
        &self,
        request: ListArchiveRequest,
        cancellation: CancellationToken,
    ) -> Result<Vec<ArchiveEntry>, ArchiveError> {
        let supplied_password = request.password.is_some();
        let output = self
            .invoke(
                ArchiveOperation::List,
                SevenZipArgumentMapper::list(&request),
                Arc::new(archive_core::NoopProgressReporter),
                cancellation,
            )
            .await
            .map_err(|error| map_read_failure(error, supplied_password))?;
        Ok(SevenZipListParser::parse(&output.output))
    }
    async fn test(
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
}

pub struct SevenZipArgumentMapper;
impl SevenZipArgumentMapper {
    pub fn create(request: &CreateArchiveRequest) -> Vec<String> {
        let mut args = vec![
            "a".into(),
            format!("-t{}", request.format.extension()),
            match request.profile {
                CompressionProfile::Fast => "-mx=1",
                CompressionProfile::Balanced => "-mx=5",
                CompressionProfile::Small => "-mx=9",
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
        args
    }
    pub fn list(request: &ListArchiveRequest) -> Vec<String> {
        let mut args = vec![
            "l".into(),
            "-slt".into(),
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
}

pub struct SevenZipListParser;
impl SevenZipListParser {
    pub fn parse(output: &str) -> Vec<ArchiveEntry> {
        let mut entries = Vec::new();
        let mut path = None;
        let mut size = None;
        let mut packed = None;
        let mut attributes = None;
        let mut flush = |path: &mut Option<String>,
                         size: &mut Option<u64>,
                         packed: &mut Option<u64>,
                         attributes: &mut Option<String>| {
            if let (Some(path), Some(size)) = (path.take(), size.take()) {
                entries.push(ArchiveEntry {
                    is_directory: attributes
                        .as_deref()
                        .is_some_and(|value| value.contains('D')),
                    path,
                    size,
                    compressed_size: packed.take(),
                });
            }
            *attributes = None;
        };
        for line in output.lines().chain(std::iter::once("")) {
            if line.trim().is_empty() {
                flush(&mut path, &mut size, &mut packed, &mut attributes);
                continue;
            }
            if let Some((key, value)) = line.split_once(" = ") {
                match key {
                    "Path" => path = Some(value.into()),
                    "Size" => size = value.parse().ok(),
                    "Packed Size" => packed = value.parse().ok(),
                    "Attributes" => attributes = Some(value.into()),
                    _ => {}
                }
            }
        }
        entries
    }
}

#[derive(Debug)]
struct InvocationOutput {
    output: String,
    warning: bool,
}
fn append_bounded(target: &mut String, line: &str) {
    if target.len() < MAX_DIAGNOSTIC_BYTES {
        let available = MAX_DIAGNOSTIC_BYTES - target.len();
        target.push_str(&line[..line.len().min(available)]);
        target.push('\n');
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
                size: 12,
                compressed_size: Some(9),
                is_directory: false
            }]
        );
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
            .invoke(
                ArchiveOperation::Test,
                vec!["-n".into(), "20".into(), "127.0.0.1".into()],
                Arc::new(archive_core::NoopProgressReporter),
                cancellation,
            )
            .await;
        assert_eq!(result.unwrap_err().code, ArchiveErrorCode::Cancelled);
    }
}
