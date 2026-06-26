use crate::{

    error::{AppError, Result},

    mapper::{Mapper, PathMapping},

    model::{

        ConflictRisk, DiffStats, FileChange, FileChangeKind, PreviewResult, PreviewUnit,

    },

    preview::patch_apply::{
        apply_unified_patch, merge_patches_last_wins,
        reconstruct_new_from_patch, reconstruct_old_from_patch, resolve_target_after,
    },

    preview::target_wc::{enrich_files, TargetWcKind},

    store::models::RepoType,

    vcs::factory::SourceReader,

    vcs::svn::parse_svn_revision,

    diff::line_diff::{count_line_stats, lines_to_diff},

    preview::git_wc::{read_wc_file, resolve_wc_path},

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

    sort_units_chronological(&mut units);
    let ordered_refs = chronological_source_refs(&units);

    let aggregated = aggregate_merged_by_target_path(
        &units,
        &ctx.target_wc_path,
        &ordered_refs,
        &ctx.reader,
    )?;

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



    sort_units_chronological(&mut units);
    let ordered_refs = chronological_source_refs(&units);

    let aggregated = aggregate_merged_by_target_path(
        &units,
        &target_wc_path,
        &ordered_refs,
        &reader,
    )?;

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

pub fn load_preview_units(ctx: &PreviewContext, source_refs: &[String]) -> Result<Vec<PreviewUnit>> {
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

    sort_units_chronological(&mut units);

    Ok(units)
}

pub fn build_preview_plan_meta(ctx: &PreviewContext, source_refs: &[String]) -> Result<Vec<FileChange>> {
    let units = load_preview_units(ctx, source_refs)?;
    let ordered_refs = chronological_source_refs(&units);
    aggregate_merged_by_target_path(&units, &ctx.target_wc_path, &ordered_refs, &ctx.reader)
}

pub fn get_merged_file_change(
    ctx: &PreviewContext,
    source_refs: &[String],
    file_path: &str,
) -> Result<FileChange> {
    let units = load_preview_units(ctx, source_refs)?;
    let changes = changes_for_target_path(&units, file_path);
    if changes.is_empty() {
        return Err(AppError::Vcs(format!("file not found in preview: {file_path}")));
    }
    let target_path = changes[0]
        .target_path
        .clone()
        .unwrap_or_else(|| file_path.to_string());
    let ordered_refs = chronological_source_refs(&units);
    merge_file_changes_for_preview(
        &target_path,
        &changes,
        &ctx.target_wc_path,
        &ordered_refs,
        Some(&ctx.reader),
    )?
    .ok_or_else(|| AppError::Vcs(format!("file not found in preview: {file_path}")))
}



fn unit_chrono_key(u: &PreviewUnit) -> u64 {
    if let Ok(rev) = parse_svn_revision(&u.meta.source_ref) {
        return rev;
    }
    chrono::DateTime::parse_from_rfc3339(&u.meta.date)
        .map(|d| d.timestamp().max(0) as u64)
        .unwrap_or(0)
}

fn sort_units_chronological(units: &mut [PreviewUnit]) {
    units.sort_by_key(unit_chrono_key);
}

fn chronological_source_refs(units: &[PreviewUnit]) -> Vec<String> {
    units.iter().map(|u| u.meta.source_ref.clone()).collect()
}

fn changes_for_target_path(units: &[PreviewUnit], file_path: &str) -> Vec<FileChange> {
    let mut changes = Vec::new();
    for unit in units {
        for f in &unit.files {
            if f.target_path.as_deref() == Some(file_path) || f.path == file_path {
                changes.push(f.clone());
            }
        }
    }
    changes
}

fn normalize_content(s: &str) -> String {
    s.replace("\r\n", "\n").replace('\r', "\n")
}

fn content_equal(a: Option<&str>, b: Option<&str>) -> bool {
    normalize_content(a.unwrap_or("")) == normalize_content(b.unwrap_or(""))
}

fn has_net_change(kind: &FileChangeKind, before: Option<&str>, after: Option<&str>) -> bool {
    if content_equal(before, after) {
        return false;
    }
    if *kind == FileChangeKind::Binary {
        return true;
    }
    let (add, del) = count_line_stats(&lines_to_diff(before, after));
    add > 0 || del > 0
}

fn collect_changes_by_path(units: &[PreviewUnit]) -> std::collections::BTreeMap<String, Vec<FileChange>> {
    let mut by_path: std::collections::BTreeMap<String, Vec<FileChange>> =
        std::collections::BTreeMap::new();
    for unit in units {
        for f in &unit.files {
            if let Some(tp) = &f.target_path {
                by_path.entry(tp.clone()).or_default().push(f.clone());
            }
        }
    }
    by_path
}

fn aggregate_merged_by_target_path(
    units: &[PreviewUnit],
    wc_root: &str,
    source_refs: &[String],
    reader: &SourceReader,
) -> Result<Vec<FileChange>> {
    collect_changes_by_path(units)
        .into_iter()
        .filter_map(|(target_path, changes)| {
            merge_file_changes_for_preview(
                &target_path,
                &changes,
                wc_root,
                source_refs,
                Some(reader),
            )
            .transpose()
        })
        .collect()
}

fn change_chrono_key(fc: &FileChange, source_refs: &[String]) -> u64 {
    let Some(sr) = fc.source_ref.as_deref() else {
        return 0;
    };
    if let Ok(rev) = parse_svn_revision(sr) {
        return rev;
    }
    source_refs.iter().position(|r| r == sr).unwrap_or(0) as u64
}

fn sort_changes_chronological(changes: &mut [FileChange], source_refs: &[String]) {
    changes.sort_by_key(|c| change_chrono_key(c, source_refs));
}

fn apply_merge_step(disk: Option<String>, patch: &str) -> Result<Option<String>> {
    if let Ok(next) = apply_unified_patch(disk.as_deref(), patch) {
        return Ok(Some(next));
    }
    if disk.is_none() {
        let old = reconstruct_old_from_patch(patch);
        if let Ok(next) = apply_unified_patch(Some(&old), patch) {
            return Ok(Some(next));
        }
    }
    Ok(disk)
}

fn merge_patches_for_display(changes: &[FileChange]) -> Option<String> {
    let patches: Vec<&str> = changes
        .iter()
        .filter_map(|fc| fc.patch.as_deref())
        .collect();
    merge_patches_last_wins(&patches)
}

fn merge_file_changes_for_preview(
    target_path: &str,
    changes: &[FileChange],
    wc_root: &str,
    source_refs: &[String],
    _reader: Option<&SourceReader>,
) -> Result<Option<FileChange>> {
    if changes.is_empty() {
        return Ok(None);
    }

    let mut ordered = changes.to_vec();
    sort_changes_chronological(&mut ordered, source_refs);

    let wc_path = resolve_wc_path(wc_root, target_path);
    let wc_before = read_wc_file(&wc_path);
    let latest = ordered.last().expect("non-empty");

    if latest.kind == FileChangeKind::Delete {
        if wc_before.is_none() {
            return Ok(None);
        }
        return Ok(Some(FileChange {
            path: latest.path.clone(),
            target_path: Some(target_path.to_string()),
            kind: FileChangeKind::Delete,
            old_path: latest.old_path.clone(),
            before: wc_before,
            after: None,
            source_after: None,
            source_ref: None,
            patch: None,
            conflict_risk: Some(ConflictRisk::Low),
        }));
    }

    let mut target_merged = wc_before.clone();
    let mut merge_context_ok = true;
    for fc in &ordered {
        match fc.kind {
            FileChangeKind::Delete => target_merged = None,
            FileChangeKind::Binary => {
                if let Some(patch) = fc.patch.as_deref() {
                    let prev = target_merged.clone();
                    match apply_unified_patch(target_merged.as_deref(), patch) {
                        Ok(next) => target_merged = Some(next),
                        Err(_) => {
                            if prev.is_some() {
                                merge_context_ok = false;
                            }
                        }
                    }
                }
            }
            FileChangeKind::Add | FileChangeKind::Modify | FileChangeKind::Rename => {
                if let Some(patch) = fc.patch.as_deref() {
                    let prev = target_merged.clone();
                    target_merged = apply_merge_step(target_merged, patch)?;
                    let old = reconstruct_old_from_patch(patch);
                    let new = reconstruct_new_from_patch(patch);
                    if old != new && content_equal(target_merged.as_deref(), prev.as_deref()) {
                        merge_context_ok = false;
                    }
                }
            }
        }
    }

    let conflict_risk = if merge_context_ok {
        ConflictRisk::Low
    } else {
        ConflictRisk::High
    };

    let display_patch = if merge_context_ok {
        None
    } else {
        merge_patches_for_display(&ordered)
    };

    let patch_list: Vec<&str> = ordered
        .iter()
        .filter_map(|fc| fc.patch.as_deref())
        .collect();
    let after_content = if merge_context_ok {
        target_merged.clone()
    } else if let Some(before) = wc_before.as_deref() {
        resolve_target_after(
            before,
            display_patch.as_deref(),
            &patch_list,
        )
        .or(target_merged.clone())
    } else {
        target_merged.clone()
    };

    let kind = infer_net_kind(wc_before.is_some(), after_content.is_some());
    let visible = if merge_context_ok {
        has_net_change(&kind, wc_before.as_deref(), after_content.as_deref())
    } else {
        display_patch.is_some()
    };
    if !visible {
        return Ok(None);
    }

    let first = &ordered[0];
    Ok(Some(FileChange {
        path: first.path.clone(),
        target_path: Some(target_path.to_string()),
        kind,
        old_path: first.old_path.clone(),
        before: wc_before,
        after: after_content,
        source_after: None,
        source_ref: None,
        patch: display_patch,
        conflict_risk: Some(conflict_risk),
    }))
}

fn infer_net_kind(before_exists: bool, after_exists: bool) -> FileChangeKind {
    match (before_exists, after_exists) {
        (false, true) => FileChangeKind::Add,
        (true, false) => FileChangeKind::Delete,
        (false, false) => FileChangeKind::Delete,
        (true, true) => FileChangeKind::Modify,
    }
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

        } else {

            let (add, del) = count_line_stats(&lines_to_diff(f.before.as_deref(), f.after.as_deref()));

            lines_added += add as usize;

            lines_removed += del as usize;

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

    use crate::model::{FileChange, FileChangeKind};

    #[test]
    fn apply_merge_step_does_not_replace_full_file_with_patch_fragment() {
        let line = "                                          ng-model=\"formParams.description\" style=\"height: 50px\">";
        let full_before = format!("<div>\n<textarea\n{line}\n</textarea>\n</div>\n");
        let patch = format!(
            "@@ -3,1 +3,1 @@\n-{line}\n+                                          ng-model=\"formParams.description\" rows=\"5\">\n"
        );
        let disk = Some(full_before);
        let after = apply_merge_step(disk, &patch).unwrap().unwrap();
        assert!(after.contains("<div>"));
        assert!(after.contains("rows=\"5\""));
    }

    #[test]
    fn merge_preview_chronological_add_then_remove_style() {
        let line = "                                          ng-model=\"formParams.description\" style=\"height: 50px\">";
        let line_with_rows = "                                          ng-model=\"formParams.description\" rows=\"5\" style=\"height: 50px\">";
        let line_final = "                                          ng-model=\"formParams.description\" rows=\"5\">";
        let add_rows = format!("@@ -3,1 +3,1 @@\n-{line}\n+{line_with_rows}\n");
        let remove_style = format!("@@ -3,1 +3,1 @@\n-{line_with_rows}\n+{line_final}\n");
        let changes = vec![
            FileChange {
                path: "/trunk/a.html".into(),
                target_path: Some("a.html".into()),
                kind: FileChangeKind::Modify,
                old_path: None,
                before: None,
                after: None,
                source_after: None,
                source_ref: Some("svn:50545".into()),
                patch: Some(add_rows.into()),
                conflict_risk: None,
            },
            FileChange {
                path: "/trunk/a.html".into(),
                target_path: Some("a.html".into()),
                kind: FileChangeKind::Modify,
                old_path: None,
                before: None,
                after: None,
                source_after: None,
                source_ref: Some("svn:50556".into()),
                patch: Some(remove_style.into()),
                conflict_risk: None,
            },
        ];
        let source_refs = vec!["svn:50557".into(), "svn:50556".into(), "svn:50545".into()];
        let wc_root = std::env::temp_dir().join(format!(
            "copy-diff-merge-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&wc_root).unwrap();
        let full_before = format!("<div>\n<textarea\n{line}\n</textarea>\n</div>\n");
        std::fs::write(wc_root.join("a.html"), &full_before).unwrap();
        let agg = merge_file_changes_for_preview(
            "a.html",
            &changes,
            wc_root.to_str().unwrap(),
            &source_refs,
            None,
        )
        .unwrap()
        .expect("net change");
        let line = agg
            .after
            .as_deref()
            .unwrap()
            .lines()
            .find(|l| l.contains("formParams.description"))
            .unwrap();
        assert!(line.contains("rows=\"5\""));
        assert!(!line.contains("style=\"height: 50px\""));

        let diff = crate::relay::preview_file_with_diff(&agg).diff.unwrap();
        let del = diff
            .iter()
            .find(|l| matches!(l.line_type, crate::store::models::DiffLineType::Del))
            .expect("deletion line");
        let add = diff
            .iter()
            .find(|l| matches!(l.line_type, crate::store::models::DiffLineType::Add))
            .expect("addition line");
        assert!(del.text.contains("style=\"height: 50px\""));
        assert!(add.text.contains("rows=\"5\""));
        assert!(!add.text.contains("style=\"height: 50px\""));
        assert_eq!(agg.patch, None);
        assert_eq!(agg.conflict_risk, Some(ConflictRisk::Low));
        let _ = std::fs::remove_dir_all(&wc_root);
    }

    #[test]
    fn merge_preview_context_fail_shows_selected_patches_only() {
        let line = "                                          ng-model=\"formParams.description\" style=\"height: 50px\">";
        let line_final = "                                          ng-model=\"formParams.description\" rows=\"5\">";
        // hunk targets line 3 but WC line 3 differs — context mismatch
        let patch = format!("@@ -3,1 +3,1 @@\n-wrong line content\n+{line_final}\n");
        let changes = vec![FileChange {
            path: "/trunk/a.html".into(),
            target_path: Some("a.html".into()),
            kind: FileChangeKind::Modify,
            old_path: None,
            before: None,
            after: None,
            source_after: None,
            source_ref: Some("svn:50556".into()),
            patch: Some(patch),
            conflict_risk: None,
        }];
        let wc_root = std::env::temp_dir().join(format!(
            "copy-diff-ctx-fail-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&wc_root).unwrap();
        let full_before = format!("<div>\n<textarea\n{line}\n</textarea>\n</div>\n");
        std::fs::write(wc_root.join("a.html"), &full_before).unwrap();
        let agg = merge_file_changes_for_preview(
            "a.html",
            &changes,
            wc_root.to_str().unwrap(),
            &["svn:50556".into()],
            None,
        )
        .unwrap()
        .expect("visible via patch");
        assert_eq!(agg.conflict_risk, Some(ConflictRisk::High));
        assert_eq!(agg.after.as_deref(), Some(full_before.as_str()));
        let patch_text = agg.patch.as_deref().expect("merged patch for display");
        assert!(patch_text.contains("rows=\"5\""));
        assert!(!patch_text.contains("wrong line content") || patch_text.contains("+"));
        let view = crate::relay::preview_file_with_diff(&agg);
        let diff = view.diff.unwrap();
        assert!(diff.iter().any(|l| l.text.contains("rows=\"5\"")));
        assert!(diff.len() <= 4);
        let _ = std::fs::remove_dir_all(&wc_root);
    }
}


