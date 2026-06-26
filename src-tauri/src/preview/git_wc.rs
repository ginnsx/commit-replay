use crate::error::{AppError, Result};
use crate::model::{ConflictRisk, FileChange, FileChangeKind};
use std::path::{Path, PathBuf};

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

    let Ok(status) = run_git_apply_check(wc_root, target, patch, &fc.kind) else {
        return ConflictRisk::High;
    };

    if status {
        ConflictRisk::Low
    } else {
        ConflictRisk::High
    }
}

fn run_git_apply_check(
    wc_root: &str,
    target_path: &str,
    patch_body: &str,
    kind: &FileChangeKind,
) -> Result<bool> {
    use std::io::Write;
    use std::process::Stdio;

    let clean_target = target_path.trim_start_matches("./");
    let header = match kind {
        FileChangeKind::Add => format!("--- /dev/null\n+++ b/{clean_target}\n"),
        FileChangeKind::Delete => format!("--- a/{clean_target}\n+++ /dev/null\n"),
        _ => format!("--- a/{clean_target}\n+++ b/{clean_target}\n"),
    };
    let full_patch = format!("{header}{patch_body}");

    let mut child = crate::process::command("git")
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
    source_after: Option<&str>,
) -> Result<Option<String>> {
    match kind {
        FileChangeKind::Delete => Ok(None),
        FileChangeKind::Binary => Ok(None),
        FileChangeKind::Add => {
            let patch = patch.ok_or_else(|| AppError::Vcs("add change missing patch".into()))?;
            Ok(Some(apply_patch_or_lines(before, patch, source_after)?))
        }
        FileChangeKind::Modify | FileChangeKind::Rename => {
            let patch = patch.ok_or_else(|| AppError::Vcs("modify change missing patch".into()))?;
            Ok(Some(apply_patch_or_lines(before, patch, source_after)?))
        }
    }
}

fn apply_patch_or_lines(
    before: Option<&str>,
    patch: &str,
    _source_after: Option<&str>,
) -> Result<String> {
    let expected_new = super::patch_apply::reconstruct_new_from_patch(patch);
    let old = super::patch_apply::reconstruct_old_from_patch(patch);

    if before.is_some_and(|b| normalize_lines(b) == normalize_lines(&expected_new)) {
        return Ok(expected_new);
    }
    if before.is_some_and(|b| normalize_lines(b) == normalize_lines(&old)) {
        return super::patch_apply::apply_unified_patch(before, patch);
    }
    if let Ok(next) = super::patch_apply::apply_unified_patch(before, patch) {
        return Ok(next);
    }
    if let Some(b) = before {
        if let Ok(next) = super::patch_apply::apply_unified_patch_by_search(b, patch) {
            if normalize_lines(&next) != normalize_lines(b) {
                return Ok(next);
            }
        }
    }
    Ok(before.map(str::to_string).unwrap_or(expected_new))
}

fn normalize_lines(s: &str) -> String {
    s.replace("\r\n", "\n").replace('\r', "\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_wc_path_joins() {
        let p = resolve_wc_path("C:/repo", "./src/a.rs");
        assert!(p.to_string_lossy().contains("src"));
    }

    #[test]
    fn derive_after_keeps_target_when_patch_context_mismatches() {
        let patch = "@@ -381,8 +381,7 @@\n \
             \t\t\t\"    ii.brand AS brand, \\n\" +\n \
             -\"    pb.bin as bin,\\n\" +\n \
             -\"    pb.remark as remark,\\n\" +\n \
             +\"    pb.bin as binTwo,\\n\" +\n \
             \t\t\t\"    ii.OC_OR_SCREEN_TYPE AS ocOrScreenType  \\n\" +\n";
        let git_before =
            "line380\n    private Integer inventoryItemId;\n    private String itemCode;\n";
        let after = derive_after(
            Some(git_before),
            Some(patch),
            &FileChangeKind::Modify,
            Some("unrelated full svn snapshot\n"),
        )
        .unwrap()
        .unwrap();
        assert_eq!(after, git_before);
    }
}
