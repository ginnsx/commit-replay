use crate::{

    error::{AppError, Result},

    mapper::{Mapper, PathMapping},

    model::{

        ConflictRisk, DiffStats, FileChange, FileChangeKind, PreviewResult, PreviewUnit,

    },

    preview::target_wc::{enrich_files, TargetWcKind},

    store::models::RepoType,

    vcs::factory::SourceReader,

};



#[derive(Debug, Clone, serde::Deserialize)]

pub struct MappingInput {

    pub from: String,

    pub to: String,

}



pub struct PreviewContext {

    pub reader: SourceReader,

    pub target_wc_path: String,

    pub target_kind: TargetWcKind,

    pub mappings: Vec<MappingInput>,

}



impl PreviewContext {

    pub fn new(

        reader: SourceReader,

        target_wc_path: String,

        target_kind: TargetWcKind,

        mappings: Vec<MappingInput>,

    ) -> Self {

        Self {

            reader,

            target_wc_path,

            target_kind,

            mappings,

        }

    }



    pub fn from_repos(

        source: &crate::store::models::RepoRecord,

        target: &crate::store::models::RepoRecord,

        password: Option<String>,

        mappings: Vec<MappingInput>,

    ) -> Result<Self> {

        let reader = SourceReader::from_repo(source, password)?;

        let target_kind = TargetWcKind::from(&target.repo_type);

        Ok(Self::new(

            reader,

            target.path.clone(),

            target_kind,

            mappings,

        ))

    }

}



pub fn target_kind_from_repo_type(repo_type: &RepoType) -> TargetWcKind {

    TargetWcKind::from(repo_type)

}



pub fn build_preview_plan(

    ctx: &PreviewContext,

    source_refs: &[String],

) -> Result<PreviewResult> {

    if source_refs.is_empty() {

        return Err(AppError::Vcs("no source revisions selected".into()));

    }

    if ctx.target_wc_path.trim().is_empty() {

        return Err(AppError::Vcs("target working copy path is empty".into()));

    }



    let mapper = Mapper::new(

        ctx.mappings

            .iter()

            .map(|m| PathMapping {

                from: m.from.clone(),

                to: m.to.clone(),

            })

            .collect(),

    );



    let mut units = Vec::with_capacity(source_refs.len());

    for source_ref in source_refs {

        let mut changeset = ctx.reader.load_changeset(source_ref)?;

        mapper.apply_to_files(&mut changeset.files)?;

        enrich_files(
            ctx.target_kind,
            &ctx.target_wc_path,
            &mut changeset.files,
            &ctx.reader,
        )?;

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

    ctx: &PreviewContext,

    source_refs: &[String],

) -> Result<PreviewResult> {

    use rayon::prelude::*;



    if source_refs.is_empty() {

        return Err(AppError::Vcs("no source revisions selected".into()));

    }

    if ctx.target_wc_path.trim().is_empty() {

        return Err(AppError::Vcs("target working copy path is empty".into()));

    }



    let mapper = Mapper::new(

        ctx.mappings

            .iter()

            .map(|m| PathMapping {

                from: m.from.clone(),

                to: m.to.clone(),

            })

            .collect(),

    );



    let target_wc_path = ctx.target_wc_path.clone();

    let target_kind = ctx.target_kind;

    let reader = ctx.reader.clone();



    let mut units: Vec<PreviewUnit> = source_refs

        .par_iter()

        .map(|source_ref| {

            let mut changeset = reader.load_changeset(source_ref)?;

            mapper.apply_to_files(&mut changeset.files)?;

            enrich_files(target_kind, &target_wc_path, &mut changeset.files, &reader)?;

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



pub fn get_aggregated_files(ctx: &PreviewContext, source_refs: &[String]) -> Result<Vec<FileChange>> {

    let preview = build_preview_plan_parallel(ctx, source_refs)?;

    Ok(preview.aggregated)

}

pub fn get_aggregated_files_meta(ctx: &PreviewContext, source_refs: &[String]) -> Result<Vec<FileChange>> {
    build_preview_plan_meta(ctx, source_refs)
}

pub fn build_preview_plan_meta(ctx: &PreviewContext, source_refs: &[String]) -> Result<Vec<FileChange>> {
    use rayon::prelude::*;

    if source_refs.is_empty() {
        return Err(AppError::Vcs("no source revisions selected".into()));
    }
    if ctx.target_wc_path.trim().is_empty() {
        return Err(AppError::Vcs("target working copy path is empty".into()));
    }

    let mapper = Mapper::new(
        ctx.mappings
            .iter()
            .map(|m| PathMapping {
                from: m.from.clone(),
                to: m.to.clone(),
            })
            .collect(),
    );

    let reader = ctx.reader.clone();

    let mut units: Vec<PreviewUnit> = source_refs
        .par_iter()
        .map(|source_ref| {
            let mut changeset = reader.load_changeset_meta(source_ref)?;
            mapper.apply_to_files(&mut changeset.files)?;
            Ok(PreviewUnit {
                meta: changeset.meta,
                files: changeset.files,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    units.sort_by(|a, b| a.meta.source_ref.cmp(&b.meta.source_ref));
    Ok(aggregate_by_target_path(&units))
}



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

                    source_after: None,
                    source_ref: None,

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

                    source_after: None,
                    source_ref: None,

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


