#![forbid(unsafe_code)]

//! Minimal extraction-path guard for M1. Link handling and archive-bomb limits
//! are deliberately postponed to M3.

use std::path::{Component, Path, PathBuf};

use thiserror::Error;

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

/// Validates an entry name and returns its safe, relative Windows path.
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

/// Joins a validated entry beneath `output` and verifies the component boundary.
pub fn output_path(output: &Path, entry: &str) -> Result<PathBuf, PathSafetyError> {
    let candidate = output.join(safe_relative_path(entry)?);
    if !candidate.starts_with(output) {
        return Err(PathSafetyError::Traversal);
    }
    // This catches platform-specific absolute components if a future parser hands
    // us an already-normalized PathBuf instead of a string.
    if candidate
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(PathSafetyError::Traversal);
    }
    Ok(candidate)
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn keeps_output_boundary() {
        assert!(output_path(Path::new("out"), "../x").is_err());
    }
}
