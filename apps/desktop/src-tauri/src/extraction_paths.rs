use archive_core::ArchiveEntry;
use std::path::{Path, PathBuf};

pub(super) fn suggested_output(
    archive: &Path,
    named: bool,
    avoid_duplicate_root: bool,
    entries: &[ArchiveEntry],
) -> Option<PathBuf> {
    let parent = archive.parent()?;
    if !named {
        return Some(parent.to_path_buf());
    }
    let name = archive.file_name()?.to_str()?;
    let lower = name.to_ascii_lowercase();
    let stem = [
        ".tar.gz", ".tar.xz", ".tgz", ".txz", ".7z", ".zip", ".rar", ".tar", ".gz", ".xz", ".bz2",
        ".iso", ".cab", ".wim",
    ]
    .iter()
    .find(|extension| lower.ends_with(**extension))
    .map_or(name, |extension| &name[..name.len() - extension.len()]);
    let already_named = !entries.is_empty()
        && entries.iter().all(|entry| {
            let path = entry.path.replace('\\', "/");
            let path = path.trim_end_matches('/');
            let mut parts = path.splitn(2, '/');
            parts
                .next()
                .is_some_and(|root| root.eq_ignore_ascii_case(stem))
                && (entry.is_directory || parts.next().is_some())
        });
    Some(if avoid_duplicate_root && already_named {
        parent.to_path_buf()
    } else {
        parent.join(stem)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn entry(path: &str, directory: bool) -> ArchiveEntry {
        ArchiveEntry {
            path: path.into(),
            display_name: path.into(),
            size: 1,
            compressed_size: None,
            is_directory: directory,
            modified_at: None,
            crc: None,
            attributes: None,
            encrypted: false,
            is_symlink: false,
            is_hardlink: false,
        }
    }
    #[test]
    fn avoids_only_a_matching_single_root_without_flattening_archive_paths() {
        let archive = Path::new("/downloads/资料.ZIP");
        let entries = vec![entry("资料/", true), entry("资料/附件/文件.txt", false)];
        assert_eq!(
            suggested_output(archive, true, true, &entries).unwrap(),
            Path::new("/downloads")
        );
        assert_eq!(
            suggested_output(archive, true, false, &entries).unwrap(),
            Path::new("/downloads/资料")
        );
        assert_eq!(
            suggested_output(archive, false, false, &entries).unwrap(),
            Path::new("/downloads")
        );
        for entries in [
            vec![entry("其他/file.txt", false)],
            vec![entry("资料/file.txt", false), entry("说明.txt", false)],
            vec![],
        ] {
            assert_eq!(
                suggested_output(archive, true, true, &entries).unwrap(),
                Path::new("/downloads/资料")
            );
        }
        assert_eq!(
            suggested_output(Path::new("/a/backup.TAR.GZ"), true, false, &[]).unwrap(),
            Path::new("/a/backup")
        );
    }
}
