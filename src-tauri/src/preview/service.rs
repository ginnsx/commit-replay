use crate::{
    error::{AppError, Result},
    mapper::{Mapper, PathMapping},
    model::{
        ConflictRisk, DiffStats, FileChange, FileChangeKind, PreviewResult, PreviewUnit,
    },
    vcs::{svn_reader::SvnReader, VcsReader},
};

use super::git_wc::{check_apply, derive_after, read_wc_file, resolve_wc_path};

#[derive(Debug, Clone, serde::Deserialize)]
pub struct MappingInput {
    pub from: String,
    pub to: String,
}

pub fn build_preview_plan(
    source_wc_path: &str,
    source_refs: &[String],
    target_wc_path: &str,
    mappings: &[MappingInput],
    username: Option<String>,
    password: Option<String>,
) -> Result<PreviewResult> {
    if source_refs.is_empty() {
        return Err(AppError::Vcs("no source revisions selected".into()));
    }
    if target_wc_path.trim().is_empty() {
        return Err(AppError::Vcs("target working copy path is empty".into()));
    }

    let mapper = Mapper::new(
        mappings
            .iter()
            .map(|m| PathMapping {
                from: m.from.clone(),
                to: m.to.clone(),
            })
            .collect(),
    );

    let reader = SvnReader {
        wc_path: source_wc_path.to_string(),
        username,
        password,
    };

    let mut units = Vec::with_capacity(source_refs.len());
    for source_ref in source_refs {
        let mut changeset = reader.load_changeset(source_ref)?;
        mapper.apply_to_files(&mut changeset.files)?;
        enrich_files(target_wc_path, &mut changeset.files)?;
        units.push(PreviewUnit {
            meta: changeset.meta,
            files: changeset.files,
        });
    }

    let aggregated = aggregate_by_target_path(&units);
    let stats = compute_stats(&aggregated);

    Ok(PreviewResult {
        units,
        aggregated,
        stats,
    })
}

pub fn build_preview_plan_parallel(
    source_wc_path: &str,
    source_refs: &[String],
    target_wc_path: &str,
    mappings: &[MappingInput],
    username: Option<String>,
    password: Option<String>,
) -> Result<PreviewResult> {
    use rayon::prelude::*;

    if source_refs.is_empty() {
        return Err(AppError::Vcs("no source revisions selected".into()));
    }
    if target_wc_path.trim().is_empty() {
        return Err(AppError::Vcs("target working copy path is empty".into()));
    }

    let mapper = Mapper::new(
        mappings
            .iter()
            .map(|m| PathMapping {
                from: m.from.clone(),
                to: m.to.clone(),
            })
            .collect(),
    );

    let reader = SvnReader {
        wc_path: source_wc_path.to_string(),
        username: username.clone(),
        password: password.clone(),
    };

    let mut units: Vec<PreviewUnit> = source_refs
        .par_iter()
        .map(|source_ref| {
            let mut changeset = reader.load_changeset(source_ref)?;
            mapper.apply_to_files(&mut changeset.files)?;
            enrich_files(target_wc_path, &mut changeset.files)?;
            Ok(PreviewUnit {
                meta: changeset.meta,
                files: changeset.files,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    units.sort_by(|a, b| a.meta.source_ref.cmp(&b.meta.source_ref));

    let aggregated = aggregate_by_target_path(&units);
    let stats = compute_stats(&aggregated);

    Ok(PreviewResult {
        units,
        aggregated,
        stats,
    })
}

pub fn get_aggregated_files(
    source_wc_path: &str,
    source_refs: &[String],
    target_wc_path: &str,
    mappings: &[MappingInput],
    username: Option<String>,
    password: Option<String>,
) -> Result<Vec<FileChange>> {
    let preview = build_preview_plan_parallel(
        source_wc_path,
        source_refs,
        target_wc_path,
        mappings,
        username,
        password,
    )?;
    Ok(preview.aggregated)
}

fn enrich_files(wc_root: &str, files: &mut [FileChange]) -> Result<()> {
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
            _ => check_apply(wc_root, fc),
        });
    }
    Ok(())
}

/// Later units override earlier ones for the same target path (replay order).
fn aggregate_by_target_path(units: &[PreviewUnit]) -> Vec<FileChange> {
    let mut by_path: std::collections::BTreeMap<String, FileChange> =
        std::collections::BTreeMap::new();

    for unit in units {
        for f in &unit.files {
            if let Some(tp) = &f.target_path {
                by_path.insert(tp.clone(), f.clone());
            }
        }
    }

    by_path.into_values().collect()
}

fn compute_stats(files: &[FileChange]) -> DiffStats {
    let mut lines_added = 0usize;
    let mut lines_removed = 0usize;
    let mut binary_count = 0usize;
    let mut conflict_risk_count = 0usize;

    for f in files {
        if f.kind == FileChangeKind::Binary {
            binary_count += 1;
        }
        if f.conflict_risk == Some(ConflictRisk::High) {
            conflict_risk_count += 1;
        }
        if let Some(patch) = &f.patch {
            for line in patch.lines() {
                if let Some(c) = line.chars().next() {
                    match c {
                        '+' if !line.starts_with("+++") => lines_added += 1,
                        '-' if !line.starts_with("---") => lines_removed += 1,
                        _ => {}
                    }
                }
            }
        }
    }

    DiffStats {
        files_changed: files.len(),
        lines_added,
        lines_removed,
        binary_count,
        conflict_risk_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{FileChange, FileChangeKind, ReplayUnitMeta};

    fn sample_meta() -> ReplayUnitMeta {
        ReplayUnitMeta {
            source_ref: "svn:1".into(),
            author: String::new(),
            date: String::new(),
            message: String::new(),
            changed_paths_count: 1,
        }
    }

    #[test]
    fn aggregate_later_unit_overrides() {
        let units = vec![
            PreviewUnit {
                meta: sample_meta(),
                files: vec![FileChange {
                    path: "/trunk/a.txt".into(),
                    target_path: Some("a.txt".into()),
                    kind: FileChangeKind::Modify,
                    old_path: None,
                    before: Some("v1".into()),
                    after: Some("v2".into()),
                    patch: None,
                    conflict_risk: None,
                }],
            },
            PreviewUnit {
                meta: sample_meta(),
                files: vec![FileChange {
                    path: "/trunk/a.txt".into(),
                    target_path: Some("a.txt".into()),
                    kind: FileChangeKind::Modify,
                    old_path: None,
                    before: Some("v2".into()),
                    after: Some("v3".into()),
                    patch: None,
                    conflict_risk: None,
                }],
            },
        ];
        let agg = aggregate_by_target_path(&units);
        assert_eq!(agg.len(), 1);
        assert_eq!(agg[0].after.as_deref(), Some("v3"));
    }
}
