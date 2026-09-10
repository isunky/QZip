use std::collections::HashMap;

use archive_core::ArchiveEntry;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct EntryPage {
    pub(super) entries: Vec<ArchiveEntry>,
    pub(super) total: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) next_offset: Option<usize>,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) enum EntrySortKey {
    Name,
    Size,
    Type,
    Modified,
}

#[derive(Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) enum SortDirection {
    Ascending,
    Descending,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct EntrySortDto {
    pub(super) key: EntrySortKey,
    pub(super) direction: SortDirection,
}

pub(super) fn entries_in_directory(
    archive_entries: &[ArchiveEntry],
    directory: &str,
    search: &str,
) -> Vec<ArchiveEntry> {
    let directory = directory.replace('\\', "/");
    let directory = directory.trim_matches('/');
    let prefix = if directory.is_empty() {
        String::new()
    } else {
        format!("{directory}/")
    };
    let needle = search.to_ascii_lowercase();
    let mut entries = HashMap::<String, ArchiveEntry>::new();

    for entry in archive_entries {
        let Some(relative_path) = entry.path.strip_prefix(&prefix) else {
            continue;
        };
        if relative_path.is_empty() {
            continue;
        }

        if let Some((folder_name, _)) = relative_path.split_once('/') {
            if folder_name.is_empty() {
                continue;
            }
            let path = format!("{prefix}{folder_name}");
            entries.entry(path.clone()).or_insert_with(|| ArchiveEntry {
                path,
                display_name: folder_name.to_owned(),
                size: 0,
                compressed_size: None,
                is_directory: true,
                modified_at: None,
                crc: None,
                attributes: Some("D".to_owned()),
                encrypted: entry.encrypted,
                is_symlink: false,
                is_hardlink: false,
            });
        } else {
            entries.insert(entry.path.clone(), entry.clone());
        }
    }

    let mut entries = entries
        .into_values()
        .filter(|entry| {
            needle.is_empty() || entry.display_name.to_ascii_lowercase().contains(&needle)
        })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| {
        left.is_directory
            .cmp(&right.is_directory)
            .reverse()
            .then_with(|| left.display_name.cmp(&right.display_name))
    });
    entries
}

pub(super) fn sort_archive_entries(
    entries: &mut [ArchiveEntry],
    sort_key: EntrySortKey,
    direction: SortDirection,
) {
    entries.sort_by(|left, right| {
        let folder_order = right.is_directory.cmp(&left.is_directory);
        if !folder_order.is_eq() {
            return folder_order;
        }

        let field_order = match sort_key {
            EntrySortKey::Name => left
                .display_name
                .to_lowercase()
                .cmp(&right.display_name.to_lowercase()),
            EntrySortKey::Size => left.size.cmp(&right.size),
            EntrySortKey::Type => {
                archive_entry_extension(left).cmp(&archive_entry_extension(right))
            }
            EntrySortKey::Modified => left
                .modified_at
                .as_deref()
                .unwrap_or_default()
                .cmp(right.modified_at.as_deref().unwrap_or_default()),
        };
        let field_order = if direction == SortDirection::Descending {
            field_order.reverse()
        } else {
            field_order
        };
        field_order.then_with(|| {
            left.display_name
                .to_lowercase()
                .cmp(&right.display_name.to_lowercase())
        })
    });
}

fn archive_entry_extension(entry: &ArchiveEntry) -> String {
    entry
        .display_name
        .rsplit_once('.')
        .filter(|(stem, extension)| !stem.is_empty() && !extension.is_empty())
        .map(|(_, extension)| extension.to_ascii_lowercase())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str) -> ArchiveEntry {
        ArchiveEntry {
            path: path.to_owned(),
            display_name: path.rsplit('/').next().unwrap_or(path).to_owned(),
            size: 12,
            compressed_size: Some(9),
            is_directory: false,
            modified_at: None,
            crc: None,
            attributes: Some("A".to_owned()),
            encrypted: false,
            is_symlink: false,
            is_hardlink: false,
        }
    }

    #[test]
    fn creates_navigable_folders_for_archives_without_directory_entries() {
        let archive_entries = vec![
            file("附件/封面.docx"),
            file("附件/投标人承诺函.docx"),
            file("招标文件.pdf"),
        ];

        let root = entries_in_directory(&archive_entries, "", "");
        assert_eq!(root.len(), 2);
        assert_eq!(root[0].path, "附件");
        assert!(root[0].is_directory);
        assert_eq!(root[1].path, "招标文件.pdf");

        let attachment = entries_in_directory(&archive_entries, "附件", "");
        assert_eq!(attachment.len(), 2);
        assert!(attachment.iter().all(|entry| !entry.is_directory));
        assert!(
            attachment
                .iter()
                .any(|entry| entry.display_name == "封面.docx")
        );
    }

    #[test]
    fn directory_matching_does_not_include_similar_prefixes() {
        let archive_entries = vec![file("附件/inside.txt"), file("附件二/outside.txt")];

        let entries = entries_in_directory(&archive_entries, "附件", "");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path, "附件/inside.txt");
    }

    #[test]
    fn sorts_the_full_entry_set_while_keeping_folders_first() {
        let mut entries = vec![
            file("small.txt"),
            ArchiveEntry {
                path: "文件夹".into(),
                display_name: "文件夹".into(),
                size: 0,
                compressed_size: None,
                is_directory: true,
                modified_at: None,
                crc: None,
                attributes: Some("D".into()),
                encrypted: false,
                is_symlink: false,
                is_hardlink: false,
            },
            ArchiveEntry {
                size: 99,
                ..file("large.pdf")
            },
        ];

        sort_archive_entries(&mut entries, EntrySortKey::Size, SortDirection::Descending);
        assert!(entries[0].is_directory);
        assert_eq!(entries[1].display_name, "large.pdf");
        assert_eq!(entries[2].display_name, "small.txt");
    }
}
