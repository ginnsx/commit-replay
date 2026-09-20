use std::io::Write;
use std::path::Path;
use std::process::Stdio;

use super::{VcsCheckpoint, VcsWriter};
use crate::{
    error::{AppError, Result},
    model::{
        ApplyResult, ApplyStatus, ChangeSet, FileChange, FileChangeKind, ReplayUnitMeta,
        ValidationIssue, ValidationResult,
    },
    preview::git_wc::resolve_wc_path,
    store::models::IntegrationStrategy,
};

pub struct GitWriter {
    pub repo_path: String,
}

impl GitWriter {
    fn run_git(&self, args: &[&str]) -> Result<String> {
        let output = crate::process::output(
            crate::process::command("git")
                .current_dir(&self.repo_path)
                .args(args),
        )
        .map_err(|e| AppError::Vcs(format!("failed to spawn git: {e}")))?;
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        if output.status.success() {
            return Ok(stdout);
        }
        Err(AppError::Vcs(format!(
            "git {} failed: {stderr}{stdout}",
            args.first().copied().unwrap_or("")
        )))
    }

    fn write_file(&self, rel_path: &str, content: &str) -> Result<()> {
        self.write_file_bytes(rel_path, content.as_bytes())
    }

    fn write_file_bytes(&self, rel_path: &str, content: &[u8]) -> Result<()> {
        let path = resolve_wc_path(&self.repo_path, rel_path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, content)?;
        Ok(())
    }

    fn write_binary_file(&self, fc: &FileChange, target: &str) -> Result<()> {
        let content = fc
            .after_bytes
            .as_deref()
            .ok_or_else(|| AppError::Apply(format!("binary content missing for {target}")))?;
        self.write_file_bytes(target, content)
    }

    fn delete_file(&self, rel_path: &str) -> Result<()> {
        let path = resolve_wc_path(&self.repo_path, rel_path);
        if path.is_file() {
            std::fs::remove_file(path)?;
        }
        Ok(())
    }

    fn write_rename_after(&self, fc: &FileChange, target: &str, after: &str) -> Result<()> {
        if let Some(old_path) = fc.old_path.as_deref().filter(|old| *old != target) {
            self.delete_file(old_path)?;
        }
        self.write_file(target, after)
    }

    fn apply_patch(&self, fc: &FileChange) -> Result<()> {
        let Some(patch) = fc.patch.as_ref() else {
            return Ok(());
        };
        let target = fc
            .target_path
            .as_deref()
            .ok_or_else(|| AppError::Mapping("missing target_path".into()))?;
        let clean_target = target.trim_start_matches("./");
        let header = match fc.kind {
            FileChangeKind::Add => format!("--- /dev/null\n+++ b/{clean_target}\n"),
            FileChangeKind::Delete => format!("--- a/{clean_target}\n+++ /dev/null\n"),
            _ => format!("--- a/{clean_target}\n+++ b/{clean_target}\n"),
        };
        let full_patch = format!("{header}{patch}");
        let mut child = crate::process::command("git")
            .current_dir(&self.repo_path)
            .args(["apply", "-"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| AppError::Vcs(format!("git apply spawn: {e}")))?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(full_patch.as_bytes())
                .map_err(|e| AppError::Vcs(format!("git apply stdin: {e}")))?;
        }
        let out = crate::process::wait_with_output(child)
            .map_err(|e| AppError::Vcs(format!("git apply wait: {e}")))?;
        if out.status.success() {
            if let Some(after) = fc.after.as_ref() {
                let path = resolve_wc_path(&self.repo_path, target);
                if let Ok(current) = std::fs::read_to_string(&path) {
                    if equivalent_text(&current, after) {
                        self.write_file(target, after)?;
                    }
                }
            }
            return Ok(());
        }
        Err(AppError::Apply(format!(
            "git apply failed for {target}: {}",
            String::from_utf8_lossy(&out.stderr)
        )))
    }

    fn apply_patch_with_fallback(&self, fc: &FileChange) -> Result<()> {
        let target = fc
            .target_path
            .as_deref()
            .ok_or_else(|| AppError::Mapping("missing target_path".into()))?;
        match self.apply_patch(fc) {
            Ok(()) => Ok(()),
            Err(_) => {
                if let Some(after) = fc.after.as_ref() {
                    self.write_file(target, after)
                } else {
                    Err(AppError::Apply("no content to apply".into()))
                }
            }
        }
    }

    pub fn apply_file_with_strategy(
        &self,
        fc: &FileChange,
        strategy: IntegrationStrategy,
    ) -> Result<()> {
        let target = fc
            .target_path
            .as_deref()
            .ok_or_else(|| AppError::Mapping("missing target_path".into()))?;

        match strategy {
            IntegrationStrategy::Skip | IntegrationStrategy::ManualMerge => Ok(()),
            IntegrationStrategy::ApplyPatch => self.apply_patch_only(fc),
            IntegrationStrategy::WriteAfter => match fc.kind {
                FileChangeKind::Delete => self.delete_file(target),
                FileChangeKind::Add | FileChangeKind::Modify => {
                    if let Some(after) = fc.after.as_ref() {
                        self.write_file(target, after)
                    } else {
                        Ok(())
                    }
                }
                FileChangeKind::Rename => {
                    if let Some(after) = fc.after.as_ref() {
                        self.write_rename_after(fc, target, after)
                    } else {
                        Ok(())
                    }
                }
                FileChangeKind::Binary => {
                    self.write_binary_file(fc, target)
                }
            },
        }
    }

    fn apply_patch_only(&self, fc: &FileChange) -> Result<()> {
        let target = fc
            .target_path
            .as_deref()
            .ok_or_else(|| AppError::Mapping("missing target_path".into()))?;
        match fc.kind {
            FileChangeKind::Delete => self.delete_file(target),
            FileChangeKind::Add => {
                if let Some(after) = fc.after.as_ref() {
                    self.write_file(target, after)
                } else {
                    Ok(())
                }
            }
            FileChangeKind::Modify => {
                if fc.patch.is_some() {
                    self.apply_patch(fc)
                } else if let Some(after) = fc.after.as_ref() {
                    self.write_file(target, after)
                } else {
                    Ok(())
                }
            }
            FileChangeKind::Rename => {
                if let Some(after) = fc.after.as_ref() {
                    self.write_rename_after(fc, target, after)
                } else {
                    Ok(())
                }
            }
            FileChangeKind::Binary => {
                self.write_binary_file(fc, target)
            }
        }
    }

    fn apply_file_default(&self, fc: &FileChange) -> Result<()> {
        let target = fc
            .target_path
            .as_deref()
            .ok_or_else(|| AppError::Mapping("missing target_path".into()))?;
        match fc.kind {
            FileChangeKind::Add => {
                if let Some(after) = fc.after.as_ref() {
                    self.write_file(target, after)
                } else if fc.patch.is_some() {
                    self.apply_patch_with_fallback(fc)
                } else {
                    Ok(())
                }
            }
            FileChangeKind::Modify => {
                if fc.patch.is_some() {
                    self.apply_patch_with_fallback(fc)
                } else if let Some(after) = fc.after.as_ref() {
                    self.write_file(target, after)
                } else {
                    Ok(())
                }
            }
            FileChangeKind::Rename => {
                if let Some(after) = fc.after.as_ref() {
                    self.write_rename_after(fc, target, after)
                } else if fc.patch.is_some() {
                    self.apply_patch_with_fallback(fc)
                } else {
                    Ok(())
                }
            }
            FileChangeKind::Delete => self.delete_file(target),
            FileChangeKind::Binary => {
                self.write_binary_file(fc, target)
            }
        }
    }

    pub fn apply_changeset_with_strategies(
        &self,
        changeset: &ChangeSet,
        strategies: &std::collections::HashMap<String, IntegrationStrategy>,
    ) -> Result<ApplyResult> {
        let mut failed = Vec::new();
        for fc in &changeset.files {
            let target = match fc.target_path.as_deref() {
                Some(t) => t,
                None => {
                    failed.push(fc.path.clone());
                    continue;
                }
            };
            let strategy = strategies
                .get(target)
                .copied()
                .unwrap_or(IntegrationStrategy::ApplyPatch);
            if self.apply_file_with_strategy(fc, strategy).is_err() {
                failed.push(target.to_string());
            }
        }
        if failed.is_empty() {
            Ok(ApplyResult {
                status: ApplyStatus::Ok,
                message: None,
                failed_paths: vec![],
            })
        } else {
            Ok(ApplyResult {
                status: ApplyStatus::Conflict,
                message: Some("some files failed to apply".into()),
                failed_paths: failed,
            })
        }
    }

    pub fn commit_with_message(&self, message: &str) -> Result<String> {
        self.commit_with_message_inner(message, false)
    }

    pub fn commit_with_message_allow_empty(&self, message: &str) -> Result<String> {
        self.commit_with_message_inner(message, true)
    }

    fn commit_with_message_inner(&self, message: &str, allow_empty: bool) -> Result<String> {
        self.run_git(&["add", "-A"])?;
        let status = self.run_git(&["status", "--porcelain"])?;
        if status.trim().is_empty() {
            if allow_empty {
                self.run_git(&["commit", "--allow-empty", "-m", message])?;
            } else {
                let sha = self.run_git(&["rev-parse", "--short", "HEAD"])?;
                return Ok(sha.trim().to_string());
            }
        } else {
            self.run_git(&["commit", "-m", message])?;
        }
        let sha = self.run_git(&["rev-parse", "--short", "HEAD"])?;
        Ok(sha.trim().to_string())
    }

    pub fn commit_allow_empty(
        &self,
        meta: &ReplayUnitMeta,
        message_template: &str,
    ) -> Result<String> {
        let msg = if message_template.is_empty() {
            format!("relay: {}", meta.message)
        } else {
            message_template.replace("{message}", &meta.message)
        };
        self.commit_with_message_allow_empty(&msg)
    }

    pub fn create_migration_branch(&self) -> Result<String> {
        let base_name = migration_branch_base_name(chrono::Local::now().naive_local());
        let branch_name = select_available_branch_name(&base_name, |candidate| {
            self.run_git(&["branch", "--list", candidate])
                .map(|output| !output.trim().is_empty())
        })?;
        self.run_git(&["checkout", "-b", &branch_name])?;
        Ok(branch_name)
    }
}

fn migration_branch_base_name(now: chrono::NaiveDateTime) -> String {
    format!("relay/{}", now.format("%Y%m%d-%H%M%S-%3f"))
}

fn select_available_branch_name<F>(base_name: &str, mut exists: F) -> Result<String>
where
    F: FnMut(&str) -> Result<bool>,
{
    let mut suffix = 1usize;
    loop {
        let candidate = if suffix == 1 {
            base_name.to_string()
        } else {
            format!("{base_name}-{suffix}")
        };
        if !exists(&candidate)? {
            return Ok(candidate);
        }
        suffix += 1;
    }
}

fn equivalent_text(a: &str, b: &str) -> bool {
    let normalize = |s: &str| s.replace("\r\n", "\n").replace('\r', "\n");
    normalize(a).trim_end_matches('\n') == normalize(b).trim_end_matches('\n')
}

impl VcsWriter for GitWriter {
    fn prepare(&self, branch: &str) -> Result<VcsCheckpoint> {
        let status = self.run_git(&["status", "--porcelain"])?;
        if !status.trim().is_empty() {
            return Err(AppError::Validation(
                "target working copy is not clean".into(),
            ));
        }
        if !branch.is_empty() {
            self.run_git(&["checkout", branch])?;
        }
        let head = self.run_git(&["rev-parse", "HEAD"])?;
        let base_branch = self.run_git(&["branch", "--show-current"])?;
        let base_branch = match base_branch.trim() {
            "" => None,
            branch => Some(branch.to_string()),
        };
        Ok(VcsCheckpoint {
            reference: head.trim().to_string(),
            base_branch,
        })
    }

    fn apply(&self, changeset: &ChangeSet) -> Result<ApplyResult> {
        let mut failed = Vec::new();
        for fc in &changeset.files {
            let target = match fc.target_path.as_deref() {
                Some(t) => t,
                None => {
                    failed.push(fc.path.clone());
                    continue;
                }
            };
            if self.apply_file_default(fc).is_err() {
                failed.push(target.to_string());
            }
        }
        if failed.is_empty() {
            Ok(ApplyResult {
                status: ApplyStatus::Ok,
                message: None,
                failed_paths: vec![],
            })
        } else {
            Ok(ApplyResult {
                status: ApplyStatus::Conflict,
                message: Some("some files failed to apply".into()),
                failed_paths: failed,
            })
        }
    }

    fn validate(&self) -> Result<ValidationResult> {
        let mut issues = Vec::new();
        for entry in walkdir_simple(&self.repo_path)? {
            if entry.ends_with(".rs") || entry.ends_with(".java") || entry.ends_with(".ts") {
                if let Ok(content) = std::fs::read_to_string(&entry) {
                    if content.contains("<<<<<<<") {
                        issues.push(ValidationIssue {
                            path: entry,
                            reason: "conflict markers found".into(),
                        });
                    }
                }
            }
        }
        Ok(ValidationResult {
            passed: issues.is_empty(),
            issues,
        })
    }

    fn commit(&self, meta: &ReplayUnitMeta, message_template: &str) -> Result<String> {
        let msg = if message_template.is_empty() {
            format!("relay: {}", meta.message)
        } else {
            message_template.replace("{message}", &meta.message)
        };
        self.commit_with_message(&msg)
    }

    fn rollback(&self, checkpoint: &VcsCheckpoint, created_branch: Option<&str>) -> Result<()> {
        self.run_git(&["reset", "--hard", &checkpoint.reference])?;
        self.run_git(&["clean", "-fd"])?;
        if let Some(base_branch) = checkpoint.base_branch.as_deref() {
            self.run_git(&["checkout", base_branch])?;
        } else {
            self.run_git(&["checkout", "--detach", &checkpoint.reference])?;
        }
        if let Some(created_branch) = created_branch {
            self.run_git(&["branch", "-D", created_branch])?;
        }
        Ok(())
    }
}

fn walkdir_simple(root: &str) -> Result<Vec<String>> {
    let mut out = Vec::new();
    walk(Path::new(root), &mut out)?;
    Ok(out)
}

fn walk(path: &Path, out: &mut Vec<String>) -> Result<()> {
    if path.is_file() {
        out.push(path.to_string_lossy().into_owned());
        return Ok(());
    }
    if path.is_dir() {
        if path.file_name().and_then(|n| n.to_str()) == Some(".git") {
            return Ok(());
        }
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            walk(&entry.path(), out)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::*;

    fn binary_change(payload: Option<Vec<u8>>) -> FileChange {
        FileChange {
            path: "report.xlsx".into(),
            target_path: Some("report.xlsx".into()),
            kind: FileChangeKind::Binary,
            old_path: None,
            before: None,
            after: None,
            source_after: None,
            after_bytes: payload,
            source_ref: Some("svn:42".into()),
            patch: None,
            conflict_risk: None,
            analysis: None,
        }
    }

    #[test]
    fn writes_binary_content_without_text_decoding() {
        let dir = std::env::temp_dir().join(format!("relay-binary-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let writer = GitWriter {
            repo_path: dir.to_string_lossy().into_owned(),
        };
        let payload = vec![0x50, 0x4b, 0x03, 0x04, 0xff, 0x00, 0x80];

        writer
            .apply_file_with_strategy(
                &binary_change(Some(payload.clone())),
                IntegrationStrategy::WriteAfter,
            )
            .unwrap();

        assert_eq!(std::fs::read(dir.join("report.xlsx")).unwrap(), payload);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn rejects_binary_change_without_content() {
        let writer = GitWriter {
            repo_path: std::env::temp_dir().to_string_lossy().into_owned(),
        };

        let err = writer
            .apply_file_with_strategy(
                &binary_change(None),
                IntegrationStrategy::WriteAfter,
            )
            .unwrap_err();

        assert!(err.to_string().contains("binary content missing"));
    }

    #[test]
    fn formats_migration_branch_name_with_milliseconds() {
        let now = NaiveDate::from_ymd_opt(2026, 8, 10)
            .and_then(|date| date.and_hms_milli_opt(15, 30, 12, 123))
            .expect("valid test timestamp");

        assert_eq!(migration_branch_base_name(now), "relay/20260810-153012-123");
    }

    #[test]
    fn appends_incrementing_suffix_when_branch_name_exists() {
        let existing = ["relay/20260810-153012-123", "relay/20260810-153012-123-2"];

        let selected = select_available_branch_name("relay/20260810-153012-123", |candidate| {
            Ok(existing.contains(&candidate))
        })
        .expect("branch selection should succeed");

        assert_eq!(selected, "relay/20260810-153012-123-3");
    }
}
