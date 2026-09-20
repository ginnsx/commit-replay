use crate::error::{AppError, Result};
use crate::model::{ConflictRisk, FileChange, FileChangeKind};
use crate::store::models::RepoType;
use crate::vcs::factory::SourceReader;
use crate::vcs::svn_reader::attach_source_after_file;
use crate::vcs::svn::parse_svn_revision;

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

pub(crate) fn ensure_source_after(reader: &SourceReader, fc: &mut FileChange) -> Result<()> {
    if fc.source_after.is_some() || fc.after_bytes.is_some() {
        return Ok(());
    }
    if matches!(fc.kind, FileChangeKind::Delete) {
        return Ok(());
    }
    if let SourceReader::Svn(svn) = reader {
        if let Some(source_ref) = fc.source_ref.as_deref() {
            let revision = parse_svn_revision(source_ref)?;
            attach_source_after_file(svn, revision, fc);
        }
    }
    Ok(())
}

fn enrich_one(
    kind: TargetWcKind,
    wc_root: &str,
    fc: &mut FileChange,
    reader: Option<&SourceReader>,
) -> Result<()> {
    if let Some(r) = reader {
        ensure_source_after(r, fc)?;
    }

    let target = fc
        .target_path
        .clone()
        .ok_or_else(|| AppError::Mapping("file missing target_path after mapping".into()))?;

    let wc_path = resolve_wc_path(wc_root, &target);
    fc.before = read_wc_file(&wc_path);

    if fc.patch.is_some() || fc.after.is_none() {
        fc.after = derive_after(
            fc.before.as_deref(),
            fc.patch.as_deref(),
            &fc.kind,
            fc.source_after.as_deref(),
        )?;
    }

    if fc.conflict_risk.is_none() {
        fc.conflict_risk = Some(match fc.kind {
            FileChangeKind::Binary => ConflictRisk::Low,
            FileChangeKind::Delete if fc.before.is_none() => ConflictRisk::High,
            FileChangeKind::Add if fc.before.is_some() => ConflictRisk::High,
            _ => check_apply(kind, wc_root, fc),
        });
    }
    Ok(())
}

pub fn enrich_file(
    kind: TargetWcKind,
    wc_root: &str,
    fc: &mut FileChange,
    reader: &SourceReader,
) -> Result<()> {
    enrich_one(kind, wc_root, fc, Some(reader))
}

pub fn enrich_files(
    kind: TargetWcKind,
    wc_root: &str,
    files: &mut [FileChange],
    reader: &SourceReader,
) -> Result<()> {
    use rayon::prelude::*;

    files
        .par_iter_mut()
        .try_for_each(|fc| enrich_one(kind, wc_root, fc, Some(reader)))
}
