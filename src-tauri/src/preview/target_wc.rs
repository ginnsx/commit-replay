use crate::error::{AppError, Result};
use crate::model::{ConflictRisk, FileChange, FileChangeKind};
use crate::store::models::RepoType;

use super::git_wc::{check_apply as git_check_apply, derive_after, read_wc_file, resolve_wc_path};
use super::patch_apply::apply_unified_patch;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetWcKind {
    Git,
    Svn,
}

impl From<&RepoType> for TargetWcKind {
    fn from(value: &RepoType) -> Self {
        match value {
            RepoType::Git => TargetWcKind::Git,
            RepoType::Svn => TargetWcKind::Svn,
        }
    }
}

pub fn check_apply(kind: TargetWcKind, wc_root: &str, fc: &FileChange) -> ConflictRisk {
    match kind {
        TargetWcKind::Git => git_check_apply(wc_root, fc),
        TargetWcKind::Svn => svn_check_apply(wc_root, fc),
    }
}

fn svn_check_apply(_wc_root: &str, fc: &FileChange) -> ConflictRisk {
    let Some(patch) = fc.patch.as_ref() else {
        return ConflictRisk::Low;
    };
    if fc.target_path.is_none() {
        return ConflictRisk::High;
    }
    match apply_unified_patch(fc.before.as_deref(), patch) {
        Ok(_) => ConflictRisk::Low,
        Err(_) => ConflictRisk::High,
    }
}

pub fn enrich_files(kind: TargetWcKind, wc_root: &str, files: &mut [FileChange]) -> Result<()> {
    for fc in files.iter_mut() {
        let target = fc
            .target_path
            .clone()
            .ok_or_else(|| AppError::Mapping("file missing target_path after mapping".into()))?;

        let wc_path = resolve_wc_path(wc_root, &target);
        fc.before = match fc.kind {
            FileChangeKind::Add => None,
            _ => read_wc_file(&wc_path),
        };

        fc.after = derive_after(fc.before.as_deref(), fc.patch.as_deref(), &fc.kind)?;

        fc.conflict_risk = Some(match fc.kind {
            FileChangeKind::Binary => ConflictRisk::Low,
            FileChangeKind::Delete if fc.before.is_none() => ConflictRisk::High,
            FileChangeKind::Add if fc.before.is_some() => ConflictRisk::High,
            _ => check_apply(kind, wc_root, fc),
        });
    }
    Ok(())
}
