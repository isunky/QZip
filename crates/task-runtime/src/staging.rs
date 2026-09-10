use std::{
    fs,
    path::{Path, PathBuf},
};

use archive_core::{ArchiveError, ArchiveErrorCode, ConflictPolicy};
use uuid::Uuid;

pub(super) fn validate_create_destination(
    inputs: &[PathBuf],
    output: &Path,
) -> Result<(), ArchiveError> {
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

pub(super) struct CreateStaging {
    pub(super) directory: PathBuf,
    pub(super) archive: PathBuf,
}

pub(super) fn prepare_create_staging(output: &Path) -> Result<CreateStaging, ArchiveError> {
    let parent = output
        .parent()
        .ok_or_else(|| ArchiveError::new(ArchiveErrorCode::InvalidRequest, "压缩包保存位置无效"))?;
    fs::create_dir_all(parent).map_err(|_| {
        ArchiveError::new(ArchiveErrorCode::PermissionDenied, "无法创建压缩包保存目录")
    })?;
    let parent = parent.canonicalize().map_err(|_| {
        ArchiveError::new(ArchiveErrorCode::PermissionDenied, "无法访问压缩包保存目录")
    })?;
    let name = output
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| ArchiveError::new(ArchiveErrorCode::InvalidRequest, "压缩包文件名无效"))?;
    let directory = parent.join(format!(".qzip-create-{}", Uuid::new_v4()));
    fs::create_dir(&directory).map_err(|_| {
        ArchiveError::new(
            ArchiveErrorCode::PermissionDenied,
            "无法创建安全压缩暂存目录",
        )
    })?;
    Ok(CreateStaging {
        archive: directory.join(name),
        directory,
    })
}

pub(super) fn cleanup_create_staging(directory: &Path) -> Result<(), ArchiveError> {
    if directory
        .file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.starts_with(".qzip-create-"))
        && directory.exists()
    {
        fs::remove_dir_all(directory).map_err(|_| {
            ArchiveError::new(ArchiveErrorCode::CleanupFailed, "无法清理创建任务临时目录")
        })?;
    }
    Ok(())
}

pub(super) fn commit_created_archive(temporary: &Path, output: &Path) -> Result<(), ArchiveError> {
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

pub(super) fn prepare_extraction_staging(output: &Path) -> Result<PathBuf, ArchiveError> {
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

pub(super) fn cleanup_staging(staging: &Path) -> Result<(), ArchiveError> {
    if staging
        .file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.starts_with(".qzip-extract-"))
        && staging.exists()
    {
        fs::remove_dir_all(staging).map_err(|_| {
            ArchiveError::new(ArchiveErrorCode::CleanupFailed, "无法清理解压任务临时目录")
        })?;
    }
    Ok(())
}

pub(super) fn commit_extraction(
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
    if !path_exists(output) {
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
    let mut journal = ExtractionJournal::default();
    if let Err(error) = merge_directory(staging, output, policy, &mut journal) {
        journal.rollback()?;
        return Err(error);
    }
    journal.finalize();
    Ok(())
}

#[derive(Default)]
struct ExtractionJournal {
    created_files: Vec<PathBuf>,
    created_directories: Vec<PathBuf>,
    replaced_files: Vec<ReplacedFile>,
}

struct ReplacedFile {
    target: PathBuf,
    backup: PathBuf,
}

impl ExtractionJournal {
    fn rollback(self) -> Result<(), ArchiveError> {
        let mut rollback_error = None;
        for path in self.created_files.into_iter().rev() {
            if path_exists(&path)
                && let Err(error) = fs::remove_file(&path)
            {
                rollback_error
                    .get_or_insert_with(|| map_file_write_error(error, "无法回滚新建的解压文件"));
            }
        }
        for path in self.created_directories.into_iter().rev() {
            if path_exists(&path)
                && let Err(error) = fs::remove_dir(&path)
            {
                rollback_error.get_or_insert_with(|| {
                    ArchiveError::new(
                        ArchiveErrorCode::CleanupFailed,
                        format!("无法回滚新建的解压目录: {error}"),
                    )
                });
            }
        }
        for replacement in self.replaced_files.into_iter().rev() {
            if path_exists(&replacement.target)
                && let Err(error) = fs::remove_file(&replacement.target)
            {
                rollback_error
                    .get_or_insert_with(|| map_file_write_error(error, "无法回滚被替换的解压文件"));
                continue;
            }
            if let Err(error) = fs::rename(&replacement.backup, &replacement.target) {
                rollback_error.get_or_insert_with(|| {
                    ArchiveError::new(
                        ArchiveErrorCode::CleanupFailed,
                        format!("无法恢复原有解压文件: {error}"),
                    )
                });
            }
        }
        rollback_error.map_or(Ok(()), Err)
    }

    fn finalize(self) {
        for replacement in self.replaced_files {
            let _ = fs::remove_file(replacement.backup);
        }
    }
}

fn merge_directory(
    source: &Path,
    target: &Path,
    policy: ConflictPolicy,
    journal: &mut ExtractionJournal,
) -> Result<(), ArchiveError> {
    let mut entries = fs::read_dir(source)
        .map_err(|_| ArchiveError::new(ArchiveErrorCode::PermissionDenied, "无法读取解压暂存目录"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| {
            ArchiveError::new(ArchiveErrorCode::PermissionDenied, "无法读取解压暂存条目")
        })?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let source_path = entry.path();
        if path_is_link_or_reparse(&source_path)? {
            return Err(ArchiveError::new(
                ArchiveErrorCode::UnsafePath,
                "解压结果包含不安全链接",
            ));
        }
        let is_directory = entry
            .file_type()
            .map_err(|_| {
                ArchiveError::new(ArchiveErrorCode::PermissionDenied, "无法读取解压条目类型")
            })?
            .is_dir();
        let mut target_path = target.join(entry.file_name());
        if path_exists(&target_path) {
            if path_is_link_or_reparse(&target_path)? {
                return Err(ArchiveError::new(
                    ArchiveErrorCode::UnsafePath,
                    "解压目标包含不安全链接",
                ));
            }
            if is_directory && target_path.is_dir() {
                merge_directory(&source_path, &target_path, policy, journal)?;
                continue;
            }
            match policy {
                ConflictPolicy::Rename => target_path = renamed_path(&target_path),
                ConflictPolicy::Overwrite => {
                    if is_directory || target_path.is_dir() {
                        return Err(ArchiveError::new(
                            ArchiveErrorCode::ConflictRequiresDecision,
                            "文件与目录同名，无法安全覆盖",
                        ));
                    }
                    replace_existing_file(&source_path, &target_path, journal)?;
                    continue;
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
            journal.created_directories.push(target_path.clone());
            merge_directory(&source_path, &target_path, policy, journal)?;
        } else {
            fs::rename(&source_path, &target_path)
                .map_err(|error| map_file_write_error(error, "无法写入解压文件"))?;
            journal.created_files.push(target_path);
        }
    }
    Ok(())
}

fn replace_existing_file(
    source: &Path,
    target: &Path,
    journal: &mut ExtractionJournal,
) -> Result<(), ArchiveError> {
    let backup = target
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!(".qzip-overwrite-{}", Uuid::new_v4()));
    fs::rename(target, &backup)
        .map_err(|error| map_file_write_error(error, "无法准备覆盖现有文件"))?;
    if let Err(error) = fs::rename(source, target) {
        if fs::rename(&backup, target).is_err() {
            return Err(ArchiveError::new(
                ArchiveErrorCode::CleanupFailed,
                "无法提交解压文件，原文件已保留在临时备份中",
            ));
        }
        return Err(map_file_write_error(error, "无法写入解压文件"));
    }
    journal.replaced_files.push(ReplacedFile {
        target: target.to_owned(),
        backup,
    });
    Ok(())
}

fn map_file_write_error(error: std::io::Error, message: &str) -> ArchiveError {
    #[cfg(target_os = "windows")]
    if matches!(error.raw_os_error(), Some(32 | 33)) {
        return ArchiveError::new(ArchiveErrorCode::FileInUse, "目标文件正在被其他程序占用");
    }
    #[cfg(not(target_os = "windows"))]
    let _ = error;
    ArchiveError::new(ArchiveErrorCode::PermissionDenied, message)
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
        if !path_exists(&candidate) {
            return candidate;
        }
    }
    parent.join(format!("{stem} ({})", Uuid::new_v4()))
}

fn path_exists(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
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
