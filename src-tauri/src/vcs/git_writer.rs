use std::io::Write;
use std::path::Path;
use std::process::Stdio;

use super::VcsWriter;
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
        let path = resolve_wc_path(&self.repo_path, rel_path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, content)?;
        Ok(())
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
                    if let Some(after) = fc.after.as_ref() {
                        self.write_file(target, after)
                    } else {
                        Ok(())
                    }
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
                if let Some(after) = fc.after.as_ref() {
                    self.write_file(target, after)
                } else {
                    Ok(())
                }
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
                if let Some(after) = fc.after.as_ref() {
                    self.write_file(target, after)
                } else {
                    Ok(())
                }
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
}

fn equivalent_text(a: &str, b: &str) -> bool {
    let normalize = |s: &str| s.replace("\r\n", "\n").replace('\r', "\n");
    normalize(a).trim_end_matches('\n') == normalize(b).trim_end_matches('\n')
}

impl VcsWriter for GitWriter {
    fn prepare(&self, branch: &str) -> Result<String> {
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
        Ok(head.trim().to_string())
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

    fn rollback(&self, checkpoint: &str) -> Result<()> {
        self.run_git(&["reset", "--hard", checkpoint])?;
        self.run_git(&["clean", "-fd"])?;
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
