use std::path::Path;

use crate::{
    error::{AppError, Result},
    model::{
        ApplyResult, ApplyStatus, ChangeSet, FileChange, FileChangeKind, ReplayUnitMeta,
        ValidationIssue, ValidationResult,
    },
    preview::git_wc::resolve_wc_path,
    preview::patch_apply::apply_unified_patch,
    store::models::IntegrationStrategy,
    vcs::svn::{run_svn, SvnCredentials},
};
use super::VcsWriter;

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

    fn write_file(&self, rel_path: &str, content: &str) -> Result<()> {
        let path = resolve_wc_path(&self.wc_path, rel_path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, content)?;
        Ok(())
    }

    fn delete_file(&self, rel_path: &str) -> Result<()> {
        let path = resolve_wc_path(&self.wc_path, rel_path);
        if path.is_file() {
            std::fs::remove_file(path)?;
        }
        Ok(())
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

    pub fn apply_file_with_strategy(&self, fc: &FileChange, strategy: IntegrationStrategy) -> Result<()> {
        let target = fc
            .target_path
            .as_deref()
            .ok_or_else(|| AppError::Mapping("missing target_path".into()))?;

        match strategy {
            IntegrationStrategy::Skip | IntegrationStrategy::ManualMerge => Ok(()),
            IntegrationStrategy::ApplyPatch => self.apply_patch_only(fc),
            IntegrationStrategy::WriteAfter => match fc.kind {
                FileChangeKind::Delete => self.delete_file(target),
                FileChangeKind::Add | FileChangeKind::Modify | FileChangeKind::Rename => {
                    if let Some(after) = fc.after.as_ref() {
                        self.write_file(target, after)
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
        if fc.patch.is_some() {
            return self.apply_patch(fc);
        }
        let target = fc
            .target_path
            .as_deref()
            .ok_or_else(|| AppError::Mapping("missing target_path".into()))?;
        match fc.kind {
            FileChangeKind::Delete => self.delete_file(target),
            FileChangeKind::Add | FileChangeKind::Modify | FileChangeKind::Rename => {
                if let Some(after) = fc.after.as_ref() {
                    self.write_file(target, after)
                } else {
                    Ok(())
                }
            }
            FileChangeKind::Binary => Ok(()),
        }
    }

    fn apply_file_default(&self, fc: &FileChange) -> Result<()> {
        let target = fc
            .target_path
            .as_deref()
            .ok_or_else(|| AppError::Mapping("missing target_path".into()))?;
        match fc.kind {
            FileChangeKind::Add | FileChangeKind::Modify | FileChangeKind::Rename => {
                if fc.patch.is_some() {
                    self.apply_patch_with_fallback(fc)
                } else if let Some(after) = fc.after.as_ref() {
                    self.write_file(target, after)
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
}

impl VcsWriter for SvnWriter {
    fn prepare(&self, _branch: &str) -> Result<String> {
        let status = self.run_svn(&["status", "--ignore-externals"])?;
        let dirty: Vec<&str> = status
            .lines()
            .filter(|l| {
                let l = l.trim();
                !l.is_empty() && !l.starts_with('?')
            })
            .collect();
        if !dirty.is_empty() {
            return Err(AppError::Validation(
                "target working copy is not clean".into(),
            ));
        }
        let rev = self
            .run_svn(&["info", "--show-item", "revision"])?
            .trim()
            .to_string();
        Ok(rev)
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
        let output = self.run_svn(&["commit", "-m", &msg])?;
        parse_commit_revision(&output)
    }

    fn rollback(&self, checkpoint: &str) -> Result<()> {
        self.run_svn(&["revert", "-R", "."])?;
        if !checkpoint.is_empty() {
            self.run_svn(&["update", "-r", checkpoint])?;
        }
        Ok(())
    }
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
