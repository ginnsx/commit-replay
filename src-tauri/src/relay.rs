use crate::error::{AppError, Result};
use crate::model::{FileChange, FileChangeKind};
use crate::store::models::{DiffLine, FileChangeView, FileStatus};

use crate::diff::line_diff::{count_line_stats, count_patch_stats, lines_to_diff, patch_to_diff_lines};

pub fn file_kind_to_status(kind: &FileChangeKind) -> FileStatus {
    match kind {
        FileChangeKind::Add => FileStatus::Add,
        FileChangeKind::Delete => FileStatus::Del,
        _ => FileStatus::Mod,
    }
}

pub fn file_change_to_view(fc: &FileChange, include_diff: bool) -> FileChangeView {
    let path = fc
        .target_path
        .clone()
        .unwrap_or_else(|| fc.path.clone());
    let status = file_kind_to_status(&fc.kind);
    let diff = if include_diff {
        Some(if let Some(patch) = fc.patch.as_deref() {
            patch_to_diff_lines(patch)
        } else {
            lines_to_diff(fc.before.as_deref(), fc.after.as_deref())
        })
    } else {
        None
    };
    let (additions, deletions) = if let Some(patch) = fc.patch.as_deref() {
        count_patch_stats(patch)
    } else {
        diff.as_ref().map(|d| count_line_stats(d)).unwrap_or((0, 0))
    };
    FileChangeView {
        id: path.clone(),
        path,
        status,
        additions,
        deletions,
        diff,
    }
}

pub fn preview_file_meta(fc: &FileChange) -> FileChangeView {
    file_change_to_view(fc, false)
}

pub fn preview_file_with_diff(fc: &FileChange) -> FileChangeView {
    file_change_to_view(fc, true)
}

pub fn truncate_diff(mut lines: Vec<DiffLine>, max: usize) -> (Vec<DiffLine>, bool) {
    if lines.len() <= max {
        return (lines, false);
    }
    lines.truncate(max);
    (lines, true)
}

pub fn validate_repo_path(path: &str) -> Result<()> {
    let p = std::path::Path::new(path);
    if !p.exists() {
        return Err(AppError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("path not found: {path}"),
        )));
    }
    Ok(())
}

pub fn is_supported_migration(source_type: &str, target_type: &str) -> bool {
    matches!(
        (source_type, target_type),
        ("svn" | "git", "svn" | "git")
    )
}

pub fn unsupported_combo_message(source_type: &str, target_type: &str) -> String {
    format!("暂不支持 {source_type} → {target_type} 迁移组合")
}
