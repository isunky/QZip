#![forbid(unsafe_code)]

//! Archive domain contracts. This crate deliberately knows nothing about Tauri
//! or a concrete archiver implementation.

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
    Rar,
}

impl ArchiveFormat {
    pub const fn extension(self) -> &'static str {
        match self {
            Self::SevenZip => "7z",
            Self::Zip => "zip",
            Self::Rar => "rar",
        }
    }
}

impl FromStr for ArchiveFormat {
    type Err = ArchiveError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "7z" | "sevenzip" | "seven_zip" => Ok(Self::SevenZip),
            "zip" => Ok(Self::Zip),
            "rar" => Ok(Self::Rar),
            _ => Err(ArchiveError::invalid_option("format", value)),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CompressionProfile {
    Fast,
    #[default]
    Balanced,
    Small,
}

impl FromStr for CompressionProfile {
    type Err = ArchiveError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "fast" => Ok(Self::Fast),
            "balanced" => Ok(Self::Balanced),
            "small" => Ok(Self::Small),
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
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Queued,
    Running,
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
}

#[derive(Clone)]
pub struct ExtractArchiveRequest {
    pub archive: PathBuf,
    pub output: PathBuf,
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveEntry {
    pub path: String,
    pub size: u64,
    pub compressed_size: Option<u64>,
    pub is_directory: bool,
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
    WrongPassword,
    CorruptArchive,
    DiskFull,
    FileInUse,
    AccessDenied,
    InvalidOption,
    UnsupportedOption,
    UnsafePath,
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
            ArchiveOperation::Create => {
                matches!(format, ArchiveFormat::SevenZip | ArchiveFormat::Zip)
            }
            _ => matches!(
                format,
                ArchiveFormat::SevenZip | ArchiveFormat::Zip | ArchiveFormat::Rar
            ),
        };
        supported.then(|| Arc::clone(backend)).ok_or_else(|| {
            ArchiveError::new(
                ArchiveErrorCode::UnsupportedOption,
                format!("{operation:?} does not support .{}", format.extension()),
            )
        })
    }
}

impl fmt::Display for ArchiveFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.extension())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn serializes_public_contract_in_camel_case() {
        let capabilities = BackendCapabilities {
            backend_id: "sevenzip".into(),
            version: "26.02".into(),
            writable_formats: vec![ArchiveFormat::SevenZip],
            readable_formats: vec![ArchiveFormat::Rar],
            supports_password: true,
            supports_header_encryption: true,
            supports_progress: true,
            supports_cancellation: true,
        };
        assert_eq!(
            serde_json::to_value(capabilities).unwrap()["writableFormats"][0],
            "sevenZip"
        );
    }
    #[test]
    fn rejects_unknown_format() {
        assert_eq!(
            "tar".parse::<ArchiveFormat>().unwrap_err().code,
            ArchiveErrorCode::InvalidOption
        );
    }
}
