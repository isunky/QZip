#![forbid(unsafe_code)]

//! Archive domain contracts shared by the desktop adapter, task runtime and
//! concrete archive backends. This crate deliberately has no Tauri dependency.

use std::{collections::HashMap, fmt, path::PathBuf, str::FromStr, sync::Arc};

use async_trait::async_trait;
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio_util::sync::CancellationToken;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ArchiveFormat {
    #[default]
    SevenZip,
    Zip,
    Tar,
    TarGz,
    TarXz,
    Rar,
    Gz,
    Xz,
    Bz2,
    Iso,
    Cab,
    Wim,
    Unknown,
}

impl ArchiveFormat {
    pub const fn extension(self) -> &'static str {
        match self {
            Self::SevenZip => "7z",
            Self::Zip => "zip",
            Self::Tar => "tar",
            Self::TarGz => "tar.gz",
            Self::TarXz => "tar.xz",
            Self::Rar => "rar",
            Self::Gz => "gz",
            Self::Xz => "xz",
            Self::Bz2 => "bz2",
            Self::Iso => "iso",
            Self::Cab => "cab",
            Self::Wim => "wim",
            Self::Unknown => "",
        }
    }
    pub const fn is_writable(self) -> bool {
        matches!(
            self,
            Self::SevenZip | Self::Zip | Self::Tar | Self::TarGz | Self::TarXz
        )
    }
}
impl FromStr for ArchiveFormat {
    type Err = ArchiveError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "7z" | "sevenzip" | "seven_zip" => Ok(Self::SevenZip),
            "zip" => Ok(Self::Zip),
            "tar" => Ok(Self::Tar),
            "tar.gz" | "tar-gz" | "tgz" => Ok(Self::TarGz),
            "tar.xz" | "tar-xz" | "txz" => Ok(Self::TarXz),
            "rar" => Ok(Self::Rar),
            "gz" | "gzip" => Ok(Self::Gz),
            "xz" => Ok(Self::Xz),
            "bz2" | "bzip2" => Ok(Self::Bz2),
            "iso" => Ok(Self::Iso),
            "cab" => Ok(Self::Cab),
            "wim" => Ok(Self::Wim),
            _ => Err(ArchiveError::invalid_option("format", value)),
        }
    }
}
impl fmt::Display for ArchiveFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.extension())
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CompressionProfile {
    Store,
    Fast,
    #[default]
    Balanced,
    Small,
    Maximum,
}
impl FromStr for CompressionProfile {
    type Err = ArchiveError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "store" => Ok(Self::Store),
            "fast" => Ok(Self::Fast),
            "balanced" => Ok(Self::Balanced),
            "small" => Ok(Self::Small),
            "maximum" | "max" => Ok(Self::Maximum),
            _ => Err(ArchiveError::invalid_option("profile", value)),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ConflictPolicy {
    Rename,
    Overwrite,
    Skip,
    #[default]
    Ask,
}
impl FromStr for ConflictPolicy {
    type Err = ArchiveError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "rename" => Ok(Self::Rename),
            "overwrite" => Ok(Self::Overwrite),
            "skip" => Ok(Self::Skip),
            "ask" => Ok(Self::Ask),
            _ => Err(ArchiveError::invalid_option("conflict", value)),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ArchiveOperation {
    Create,
    Extract,
    List,
    Test,
    Update,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Queued,
    Scanning,
    Running,
    Cancelling,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Clone)]
pub struct CreateArchiveRequest {
    pub format: ArchiveFormat,
    pub output: PathBuf,
    pub inputs: Vec<PathBuf>,
    pub profile: CompressionProfile,
    pub password: Option<SecretString>,
    pub encrypt_headers: bool,
    pub test_after_create: bool,
}
#[derive(Clone)]
pub struct ExtractArchiveRequest {
    pub archive: PathBuf,
    pub output: PathBuf,
    pub selected_entries: Option<Vec<String>>,
    pub conflict_policy: ConflictPolicy,
    pub password: Option<SecretString>,
}
#[derive(Clone, Debug)]
pub struct ListArchiveRequest {
    pub archive: PathBuf,
    pub password: Option<SecretString>,
}
#[derive(Clone, Debug)]
pub struct TestArchiveRequest {
    pub archive: PathBuf,
    pub password: Option<SecretString>,
}
#[derive(Clone)]
pub struct UpdateArchiveRequest {
    pub archive: PathBuf,
    pub inputs: Vec<PathBuf>,
    pub password: Option<SecretString>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveEntry {
    pub path: String,
    pub display_name: String,
    pub size: u64,
    pub compressed_size: Option<u64>,
    pub is_directory: bool,
    pub modified_at: Option<String>,
    pub crc: Option<String>,
    pub attributes: Option<String>,
    pub encrypted: bool,
    pub is_symlink: bool,
    pub is_hardlink: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackendCapabilities {
    pub backend_id: String,
    pub version: String,
    pub writable_formats: Vec<ArchiveFormat>,
    pub readable_formats: Vec<ArchiveFormat>,
    pub supports_password: bool,
    pub supports_header_encryption: bool,
    pub supports_partial_extract: bool,
    pub supports_update: bool,
    pub supports_progress: bool,
    pub supports_cancellation: bool,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveResult {
    pub output: Option<PathBuf>,
    pub entries: Vec<ArchiveEntry>,
    pub warnings: Vec<String>,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestResult {
    pub valid: bool,
    pub warnings: Vec<String>,
}
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskProgress {
    pub operation: ArchiveOperation,
    pub percent: Option<u8>,
    pub detail: String,
}
pub trait ProgressReporter: Send + Sync {
    fn report(&self, progress: TaskProgress);
}
#[derive(Clone, Debug, Default)]
pub struct NoopProgressReporter;
impl ProgressReporter for NoopProgressReporter {
    fn report(&self, _: TaskProgress) {}
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ArchiveErrorCode {
    BackendUnavailable,
    FileNotFound,
    PermissionDenied,
    WrongPassword,
    CorruptArchive,
    DiskFull,
    FileInUse,
    AccessDenied,
    InvalidOption,
    InvalidRequest,
    UnsupportedOption,
    UnsupportedFormat,
    UnsafePath,
    ArchiveBombRisk,
    ConflictRequiresDecision,
    CleanupFailed,
    Cancelled,
    Unknown,
}
#[derive(Clone, Debug, Deserialize, Eq, Error, PartialEq, Serialize)]
#[error("{code:?}: {message}")]
#[serde(rename_all = "camelCase")]
pub struct ArchiveError {
    pub code: ArchiveErrorCode,
    pub message: String,
}
impl ArchiveError {
    pub fn new(code: ArchiveErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
    pub fn invalid_option(name: &str, value: &str) -> Self {
        Self::new(
            ArchiveErrorCode::InvalidOption,
            format!("invalid {name}: {value}"),
        )
    }
    pub fn unavailable(message: impl Into<String>) -> Self {
        Self::new(ArchiveErrorCode::BackendUnavailable, message)
    }
}

#[async_trait]
pub trait ArchiveBackend: Send + Sync {
    fn id(&self) -> &'static str;
    async fn capabilities(&self) -> Result<BackendCapabilities, ArchiveError>;
    async fn create(
        &self,
        request: CreateArchiveRequest,
        progress: Arc<dyn ProgressReporter>,
        cancellation: CancellationToken,
    ) -> Result<ArchiveResult, ArchiveError>;
    async fn extract(
        &self,
        request: ExtractArchiveRequest,
        progress: Arc<dyn ProgressReporter>,
        cancellation: CancellationToken,
    ) -> Result<ArchiveResult, ArchiveError>;
    async fn list(
        &self,
        request: ListArchiveRequest,
        cancellation: CancellationToken,
    ) -> Result<Vec<ArchiveEntry>, ArchiveError>;
    async fn test(
        &self,
        request: TestArchiveRequest,
        cancellation: CancellationToken,
    ) -> Result<TestResult, ArchiveError>;
    async fn update(
        &self,
        _: UpdateArchiveRequest,
        _: Arc<dyn ProgressReporter>,
        _: CancellationToken,
    ) -> Result<ArchiveResult, ArchiveError> {
        Err(ArchiveError::new(
            ArchiveErrorCode::UnsupportedOption,
            "this backend does not support updating archives",
        ))
    }
}
pub trait BackendRegistry: Send + Sync {
    fn backend_for(
        &self,
        operation: ArchiveOperation,
        format: ArchiveFormat,
    ) -> Result<Arc<dyn ArchiveBackend>, ArchiveError>;
}
#[derive(Default)]
pub struct InMemoryBackendRegistry {
    backends: HashMap<&'static str, Arc<dyn ArchiveBackend>>,
}
impl InMemoryBackendRegistry {
    pub fn with_backend(backend: Arc<dyn ArchiveBackend>) -> Self {
        let mut registry = Self::default();
        registry.register(backend);
        registry
    }
    pub fn register(&mut self, backend: Arc<dyn ArchiveBackend>) {
        self.backends.insert(backend.id(), backend);
    }
}
impl BackendRegistry for InMemoryBackendRegistry {
    fn backend_for(
        &self,
        operation: ArchiveOperation,
        format: ArchiveFormat,
    ) -> Result<Arc<dyn ArchiveBackend>, ArchiveError> {
        let backend = self
            .backends
            .get("sevenzip")
            .ok_or_else(|| ArchiveError::unavailable("no archive backend is configured"))?;
        let supported = match operation {
            ArchiveOperation::Create | ArchiveOperation::Update => format.is_writable(),
            _ => !matches!(format, ArchiveFormat::Unknown),
        };
        supported.then(|| Arc::clone(backend)).ok_or_else(|| {
            ArchiveError::new(
                ArchiveErrorCode::UnsupportedFormat,
                format!("{operation:?} does not support .{}", format.extension()),
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn serializes_public_contract_in_camel_case() {
        assert_eq!(serde_json::to_value(ArchiveFormat::TarGz).unwrap(), "tarGz");
    }
    #[test]
    fn supports_compound_extensions() {
        assert_eq!(
            "tar.xz".parse::<ArchiveFormat>().unwrap(),
            ArchiveFormat::TarXz
        );
    }
    #[test]
    fn rejects_unknown_format() {
        assert_eq!(
            "foo".parse::<ArchiveFormat>().unwrap_err().code,
            ArchiveErrorCode::InvalidOption
        );
    }
}
