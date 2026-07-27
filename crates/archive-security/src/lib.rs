#![forbid(unsafe_code)]

//! Path, link and archive-bomb guards used before anything is extracted.

use std::path::{Component, Path, PathBuf};

use archive_core::ArchiveEntry;
use thiserror::Error;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtractionLimits {
    pub max_entries: u64,
    pub max_total_size: u64,
    pub max_single_file_size: u64,
    pub max_directory_depth: usize,
    pub max_compression_ratio: u64,
}
impl Default for ExtractionLimits {
    fn default() -> Self {
        Self {
            max_entries: 100_000,
            max_total_size: 256 * 1024 * 1024 * 1024,
            max_single_file_size: 64 * 1024 * 1024 * 1024,
            max_directory_depth: 64,
            max_compression_ratio: 1_000,
        }
    }
}
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ExtractionSecurityPolicy {
    pub allow_symlinks: bool,
    pub allow_hardlinks: bool,
    pub limits: ExtractionLimits,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArchiveRisk {
    pub code: ArchiveRiskCode,
    pub message: String,
    pub overridable: bool,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArchiveRiskCode {
    EntryCount,
    TotalSize,
    SingleFileSize,
    Depth,
    CompressionRatio,
    InsufficientDisk,
    Symlink,
    Hardlink,
    UnsafePath,
}
#[derive(Debug, Error, Eq, PartialEq)]
pub enum PathSafetyError {
    #[error("archive entry path is empty")]
    Empty,
    #[error("archive entry path is absolute or a UNC path")]
    Absolute,
    #[error("archive entry path contains a drive prefix")]
    DrivePrefix,
    #[error("archive entry path escapes its output directory")]
    Traversal,
    #[error("archive entry path contains a Windows reserved name or invalid suffix")]
    ReservedName,
}

pub fn safe_relative_path(entry: &str) -> Result<PathBuf, PathSafetyError> {
    if entry.is_empty() {
        return Err(PathSafetyError::Empty);
    }
    let normalized = entry.replace('\\', "/");
    if normalized.starts_with('/') || normalized.starts_with("//") {
        return Err(PathSafetyError::Absolute);
    }
    if normalized.len() >= 2 && normalized.as_bytes()[1] == b':' {
        return Err(PathSafetyError::DrivePrefix);
    }
    let mut safe = PathBuf::new();
    for segment in normalized.split('/') {
        if segment.is_empty() || segment == "." {
            continue;
        }
        if segment == ".." {
            return Err(PathSafetyError::Traversal);
        }
        let stem = segment
            .split('.')
            .next()
            .unwrap_or_default()
            .to_ascii_uppercase();
        if segment.ends_with([' ', '.'])
            || matches!(
                stem.as_str(),
                "CON"
                    | "PRN"
                    | "AUX"
                    | "NUL"
                    | "COM1"
                    | "COM2"
                    | "COM3"
                    | "COM4"
                    | "COM5"
                    | "COM6"
                    | "COM7"
                    | "COM8"
                    | "COM9"
                    | "LPT1"
                    | "LPT2"
                    | "LPT3"
                    | "LPT4"
                    | "LPT5"
                    | "LPT6"
                    | "LPT7"
                    | "LPT8"
                    | "LPT9"
            )
        {
            return Err(PathSafetyError::ReservedName);
        }
        safe.push(segment);
    }
    if safe.as_os_str().is_empty() {
        return Err(PathSafetyError::Empty);
    }
    Ok(safe)
}
pub fn output_path(output: &Path, entry: &str) -> Result<PathBuf, PathSafetyError> {
    let candidate = output.join(safe_relative_path(entry)?);
    if !candidate.starts_with(output)
        || candidate
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(PathSafetyError::Traversal);
    }
    Ok(candidate)
}

pub fn assess_entries(
    entries: &[ArchiveEntry],
    archive_size: u64,
    available_bytes: Option<u64>,
    policy: &ExtractionSecurityPolicy,
) -> Vec<ArchiveRisk> {
    let mut risks = Vec::new();
    let total = entries.iter().map(|entry| entry.size).sum::<u64>();
    if entries.len() as u64 > policy.limits.max_entries {
        risks.push(risk(
            ArchiveRiskCode::EntryCount,
            "条目数量超过安全限制",
            true,
        ));
    }
    if total > policy.limits.max_total_size {
        risks.push(risk(
            ArchiveRiskCode::TotalSize,
            "预计解压大小超过安全限制",
            true,
        ));
    }
    if archive_size > 0 && total / archive_size.max(1) > policy.limits.max_compression_ratio {
        risks.push(risk(
            ArchiveRiskCode::CompressionRatio,
            "压缩比异常，可能存在压缩炸弹风险",
            true,
        ));
    }
    if let Some(free) = available_bytes {
        let reserve = (512 * 1024 * 1024u64).max(total / 20);
        if free < total.saturating_add(reserve) {
            risks.push(risk(
                ArchiveRiskCode::InsufficientDisk,
                "磁盘可用空间不足",
                false,
            ));
        }
    }
    for entry in entries {
        if entry.size > policy.limits.max_single_file_size {
            risks.push(risk(
                ArchiveRiskCode::SingleFileSize,
                format!("条目 {} 超过单文件安全限制", entry.display_name),
                true,
            ));
        }
        if entry
            .path
            .split('/')
            .filter(|part| !part.is_empty())
            .count()
            > policy.limits.max_directory_depth
        {
            risks.push(risk(
                ArchiveRiskCode::Depth,
                format!("条目 {} 目录层级过深", entry.display_name),
                true,
            ));
        }
        if safe_relative_path(&entry.path).is_err() {
            risks.push(risk(
                ArchiveRiskCode::UnsafePath,
                "压缩包包含不安全路径",
                false,
            ));
        }
        if entry.is_symlink && !policy.allow_symlinks {
            risks.push(risk(ArchiveRiskCode::Symlink, "压缩包包含符号链接", false));
        }
        if entry.is_hardlink && !policy.allow_hardlinks {
            risks.push(risk(ArchiveRiskCode::Hardlink, "压缩包包含硬链接", false));
        }
    }
    risks
}
fn risk(code: ArchiveRiskCode, message: impl Into<String>, overridable: bool) -> ArchiveRisk {
    ArchiveRisk {
        code,
        message: message.into(),
        overridable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn entry(path: &str) -> ArchiveEntry {
        ArchiveEntry {
            path: path.into(),
            display_name: path.into(),
            size: 1,
            compressed_size: Some(1),
            is_directory: false,
            modified_at: None,
            crc: None,
            attributes: None,
            encrypted: false,
            is_symlink: false,
            is_hardlink: false,
        }
    }
    #[test]
    fn accepts_unicode_relative_entry() {
        assert_eq!(
            safe_relative_path("目录/hello 世界.txt").unwrap(),
            PathBuf::from("目录").join("hello 世界.txt")
        );
    }
    #[test]
    fn rejects_unsafe_paths() {
        for entry in ["../x", "C:/x", "/x", "//server/x", "CON.txt", "name. "] {
            assert!(safe_relative_path(entry).is_err(), "{entry}");
        }
    }
    #[test]
    fn reports_non_overridable_path_risk() {
        let risks = assess_entries(
            &[entry("../bad")],
            1,
            None,
            &ExtractionSecurityPolicy::default(),
        );
        assert!(
            risks
                .iter()
                .any(|risk| risk.code == ArchiveRiskCode::UnsafePath && !risk.overridable)
        );
    }
}
