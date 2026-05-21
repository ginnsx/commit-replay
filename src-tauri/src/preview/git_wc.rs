use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::{AppError, Result};
use crate::model::{ConflictRisk, FileChange, FileChangeKind};

/// Join target working copy root with a mapped relative path.
pub fn resolve_wc_path(wc_root: &str, target_path: &str) -> PathBuf {
    let rel = target_path.trim_start_matches("./").trim_start_matches('/');
    Path::new(wc_root).join(rel)
}

pub fn read_wc_file(path: &Path) -> Option<String> {
    if !path.is_file() {
        return None;
    }
    std::fs::read_to_string(path).ok()
}

pub fn check_apply(wc_root: &str, fc: &FileChange) -> ConflictRisk {
    let Some(patch) = fc.patch.as_ref() else {
        return ConflictRisk::Low;
    };
    let Some(target) = fc.target_path.as_ref() else {
        return ConflictRisk::High;
    };

    let Ok(status) = run_git_apply_check(wc_root, target, patch) else {
        return ConflictRisk::High;
    };

    if status {
        ConflictRisk::Low
    } else {
        ConflictRisk::High
    }
}

fn run_git_apply_check(wc_root: &str, target_path: &str, patch_body: &str) -> Result<bool> {
    use std::io::Write;
    use std::process::Stdio;

    let header = format!(
        "--- a/{target_path}\n+++ b/{target_path}\n",
        target_path = target_path.trim_start_matches("./")
    );
    let full_patch = format!("{header}{patch_body}");

    let mut child = Command::new("git")
        .current_dir(wc_root)
        .args(["apply", "--check", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| AppError::Vcs(format!("failed to spawn git apply: {e}")))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(full_patch.as_bytes())
            .map_err(|e| AppError::Vcs(format!("git apply stdin: {e}")))?;
    }

    let out = child
        .wait_with_output()
        .map_err(|e| AppError::Vcs(format!("git apply wait: {e}")))?;

    Ok(out.status.success())
}

pub fn derive_after(
    before: Option<&str>,
    patch: Option<&str>,
    kind: &FileChangeKind,
) -> Result<Option<String>> {
    match kind {
        FileChangeKind::Delete => Ok(None),
        FileChangeKind::Binary => Ok(None),
        FileChangeKind::Add => {
            let patch = patch.ok_or_else(|| {
                AppError::Vcs("add change missing patch".into())
            })?;
            Ok(Some(apply_patch_or_lines(before, patch)?))
        }
        FileChangeKind::Modify | FileChangeKind::Rename => {
            let patch = patch.ok_or_else(|| {
                AppError::Vcs("modify change missing patch".into())
            })?;
            Ok(Some(apply_patch_or_lines(before, patch)?))
        }
    }
}

fn apply_patch_or_lines(before: Option<&str>, patch: &str) -> Result<String> {
    super::patch_apply::apply_unified_patch(before, patch)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_wc_path_joins() {
        let p = resolve_wc_path("C:/repo", "./src/a.rs");
        assert!(p.to_string_lossy().contains("src"));
    }
}
