use std::path::Path;

use super::{VcsCheckpoint, VcsWriter};
use crate::{
    error::{AppError, Result},
    model::{
        ApplyResult, ApplyStatus, ChangeSet, FileChange, FileChangeKind, ReplayUnitMeta,
        ValidationIssue, ValidationResult,
    },
    preview::git_wc::resolve_wc_path,
    preview::patch_apply::apply_unified_patch,
    store::models::IntegrationStrategy,
    vcs::svn::{decode_svn_output, run_svn, SvnCredentials},
};

pub struct SvnWriter {
    pub wc_path: String,
    pub username: Option<String>,
    pub password: Option<String>,
}

impl SvnWriter {
    fn creds(&self) -> SvnCredentials {
        SvnCredentials {
            username: self.username.clone(),
            password: self.password.clone(),
        }
    }

    fn run_svn(&self, args: &[&str]) -> Result<String> {
        run_svn(&self.wc_path, &self.creds(), args)
    }

    fn run_svn_wc(&self, args: &[&str]) -> Result<String> {
        let mut cmd = crate::process::command("svn");
        cmd.current_dir(&self.wc_path).arg("--non-interactive");
        if let Some(user) = &self.username {
            cmd.arg("--username").arg(user);
        }
        if let Some(pass) = &self.password {
            cmd.arg("--password").arg(pass);
        }
        cmd.args(args);
        let output = crate::process::output(&mut cmd)
            .map_err(|e| AppError::Vcs(format!("failed to spawn svn: {e}")))?;
        let stdout = decode_svn_output(&output.stdout);
        let stderr = decode_svn_output(&output.stderr);
        if output.status.success() {
            Ok(stdout)
        } else {
            Err(AppError::Vcs(format!(
                "svn {} failed (exit {:?}): {stderr}{stdout}",
                args.first().copied().unwrap_or(""),
                output.status.code(),
            )))
        }
    }

    fn write_file(&self, rel_path: &str, content: &str) -> Result<()> {
        self.write_file_bytes(rel_path, content.as_bytes())
    }

    fn write_file_bytes(&self, rel_path: &str, content: &[u8]) -> Result<()> {
        let path = resolve_wc_path(&self.wc_path, rel_path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, content)?;
        self.schedule_add_if_needed(rel_path)?;
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
        let path = resolve_wc_path(&self.wc_path, rel_path);
        if path.exists() {
            let arg = rel_path.replace('\\', "/");
            if self.run_svn_wc(&["delete", "--force", &arg]).is_err() && path.is_file() {
                std::fs::remove_file(path)?;
            }
        }
        Ok(())
    }

    fn schedule_add_if_needed(&self, _rel_path: &str) -> Result<()> {
        // SlikSVN 会按系统代码页损坏中文命令行参数；只传递 ASCII 的当前目录。
        // prepare 已确保开始时没有未跟踪文件，因此整体扫描只会登记本次写入的文件。
        self.run_svn_wc(&["add", "--force", "."])?;
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
        let wc_path = resolve_wc_path(&self.wc_path, target);
        let before = if wc_path.is_file() {
            std::fs::read_to_string(&wc_path).ok()
        } else {
            None
        };
        let after = apply_unified_patch(before.as_deref(), patch)?;
        self.write_file(target, &after)
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
                FileChangeKind::Binary => self.write_binary_file(fc, target),
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
            FileChangeKind::Binary => self.write_binary_file(fc, target),
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
            FileChangeKind::Binary => self.write_binary_file(fc, target),
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
            if let Err(err) = self.apply_file_with_strategy(fc, strategy) {
                failed.push(format!("{target}: {err}"));
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
        let message_path = std::env::temp_dir().join(format!(
            "relay-svn-commit-{}-{}.txt",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        std::fs::write(&message_path, message.as_bytes())?;
        let message_arg = message_path.to_string_lossy().into_owned();
        let result = self.run_svn(&["commit", "--file", &message_arg, "--encoding", "UTF-8"]);
        let _ = std::fs::remove_file(&message_path);
        let output = result?;
        let revision = parse_commit_revision(&output)?;
        let _ = self.run_svn(&["update"]);
        Ok(revision)
    }
}

impl VcsWriter for SvnWriter {
    fn prepare(&self, _branch: &str) -> Result<VcsCheckpoint> {
        let status = self.run_svn(&["status", "--ignore-externals"])?;
        ensure_clean_status(&status)?;
        let rev = self
            .run_svn(&["info", "--show-item", "revision"])?
            .trim()
            .to_string();
        Ok(VcsCheckpoint {
            reference: rev,
            base_branch: None,
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
            if let Err(err) = self.apply_file_default(fc) {
                failed.push(format!("{target}: {err}"));
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
        for entry in walkdir_simple(&self.wc_path)? {
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

    fn rollback(&self, checkpoint: &VcsCheckpoint, _created_branch: Option<&str>) -> Result<()> {
        self.run_svn(&["revert", "-R", "."])?;
        if !checkpoint.reference.is_empty() {
            self.run_svn(&["update", "-r", &checkpoint.reference])?;
        }
        Ok(())
    }
}

fn ensure_clean_status(status: &str) -> Result<()> {
    if status.lines().any(|line| !line.trim().is_empty()) {
        return Err(AppError::Validation(
            "target working copy is not clean, including untracked files".into(),
        ));
    }
    Ok(())
}

fn parse_commit_revision(output: &str) -> Result<String> {
    for line in output.lines() {
        if let Some(rest) = line.strip_prefix("Committed revision ") {
            if let Some(rev) = rest.strip_suffix('.') {
                return Ok(rev.to_string());
            }
            return Ok(rest.trim().to_string());
        }
    }
    Ok(String::new())
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
        if path.file_name().and_then(|n| n.to_str()) == Some(".svn") {
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
    use super::*;

    #[test]
    fn clean_status_accepts_empty_output() {
        assert!(ensure_clean_status("\r\n").is_ok());
    }

    #[test]
    fn clean_status_rejects_untracked_files() {
        let error = ensure_clean_status("?       local-only.txt\r\n").unwrap_err();
        assert!(error.to_string().contains("not clean"));
    }
}
