use tauri::State;



use crate::{

    error::AppError,

    preview::{

        build_integration_plan, build_preview_plan_meta, build_preview_plan_parallel,
        build_source_preview_plan_meta, enrich_file, get_merged_file_change, preview_cache_key,
        MappingInput, PreviewCache, PreviewContext,

        TargetWcKind,

    },

    relay::{preview_file_with_diff},

    store::{

        db::{decrypt_repo_pass, get_repo, repo_path_mappings, DbState},

        models::{FileChangeView, IntegrationPlanResult, MigrationMode},

    },

    vcs::ensure_different_repos,

};



#[derive(Debug, serde::Serialize)]

pub struct PreviewMetaResult {

    pub files: Vec<FileChangeView>,

    pub adds: u32,

    pub mods: u32,

    pub dels: u32,

    pub total_additions: u32,

    pub total_deletions: u32,

}



fn resolve_mappings(

    source: &crate::store::models::RepoRecord,

    path_mappings: Option<Vec<MappingInput>>,

) -> Vec<MappingInput> {

    if let Some(m) = path_mappings.filter(|m| !m.is_empty()) {

        return m;

    }

    repo_path_mappings(source)

        .into_iter()

        .map(|m| MappingInput {

            from: m.from,

            to: m.to,

        })

        .collect()

}



fn preview_context(

    state: &DbState,

    source_id: &str,

    target_id: &str,

    path_mappings: Option<Vec<MappingInput>>,

) -> Result<PreviewContext, AppError> {

    let conn = state

        .0

        .lock()

        .map_err(|_| AppError::Other(anyhow::anyhow!("db lock")))?;

    let source = get_repo(&conn, source_id)?

        .ok_or_else(|| AppError::Vcs("source repo not found".into()))?;

    let target = get_repo(&conn, target_id)?

        .ok_or_else(|| AppError::Vcs("target repo not found".into()))?;

    ensure_different_repos(&source, &target)?;

    let mappings = resolve_mappings(&source, path_mappings);

    let password = decrypt_repo_pass(&source)?;

    PreviewContext::from_repos(&source, &target, password, mappings).map_err(Into::into)

}

fn source_reader_context(
    state: &DbState,
    source_id: &str,
) -> Result<crate::vcs::SourceReader, AppError> {
    let conn = state
        .0
        .lock()
        .map_err(|_| AppError::Other(anyhow::anyhow!("db lock")))?;
    let source = get_repo(&conn, source_id)?
        .ok_or_else(|| AppError::Vcs("source repo not found".into()))?;
    let password = decrypt_repo_pass(&source)?;
    crate::vcs::SourceReader::from_repo(&source, password).map_err(Into::into)
}



fn target_kind_for_context(ctx: &PreviewContext) -> TargetWcKind {

    ctx.target_kind

}



fn meta_from_aggregated(aggregated: Vec<crate::model::FileChange>) -> PreviewMetaResult {

    let files: Vec<FileChangeView> = aggregated.iter().map(preview_file_with_diff).collect();

    let adds = files

        .iter()

        .filter(|f| matches!(f.status, crate::store::models::FileStatus::Add))

        .count() as u32;

    let mods = files

        .iter()

        .filter(|f| matches!(f.status, crate::store::models::FileStatus::Mod))

        .count() as u32;

    let dels = files

        .iter()

        .filter(|f| matches!(f.status, crate::store::models::FileStatus::Del))

        .count() as u32;

    let total_additions = files.iter().map(|f| f.additions).sum();

    let total_deletions = files.iter().map(|f| f.deletions).sum();

    PreviewMetaResult {

        files,

        adds,

        mods,

        dels,

        total_additions,

        total_deletions,

    }

}



#[tauri::command]
pub async fn build_preview_meta(
    state: State<'_, DbState>,
    cache: State<'_, PreviewCache>,
    source_id: String,
    target_id: String,
    source_refs: Vec<String>,
    path_mappings: Option<Vec<MappingInput>>,
) -> Result<PreviewMetaResult, AppError> {
    let ctx = preview_context(&state, &source_id, &target_id, path_mappings.clone())?;
    let key = preview_cache_key(&source_id, &target_id, &source_refs, &ctx.mappings);
    if let Some(cached) = cache.get(key) {
        return Ok(meta_from_aggregated(cached));
    }

    let aggregated = tokio::task::spawn_blocking(move || build_preview_plan_meta(&ctx, &source_refs))
        .await
        .map_err(|e| AppError::Other(anyhow::anyhow!("preview meta task: {e}")))??;

    cache.set(key, aggregated.clone());
    Ok(meta_from_aggregated(aggregated))
}

#[tauri::command]
pub async fn build_source_preview_meta(
    state: State<'_, DbState>,
    source_id: String,
    source_refs: Vec<String>,
) -> Result<PreviewMetaResult, AppError> {
    let reader = source_reader_context(&state, &source_id)?;
    let aggregated =
        tokio::task::spawn_blocking(move || build_source_preview_plan_meta(&reader, &source_refs))
            .await
            .map_err(|e| AppError::Other(anyhow::anyhow!("source preview task: {e}")))??;
    Ok(meta_from_aggregated(aggregated))
}



#[tauri::command]

pub fn get_file_diff(

    state: State<DbState>,

    cache: State<PreviewCache>,

    source_id: String,

    target_id: String,

    source_refs: Vec<String>,

    file_path: String,

    path_mappings: Option<Vec<MappingInput>>,

) -> Result<FileChangeView, AppError> {

    let ctx = preview_context(&state, &source_id, &target_id, path_mappings.clone())?;

    let _ = cache;

    let mut fc = get_merged_file_change(&ctx, &source_refs, &file_path)?;

    enrich_file(

        ctx.target_kind,

        &ctx.target_wc_path,

        &mut fc,

        &ctx.reader,

    )?;

    Ok(preview_file_with_diff(&fc))

}



fn integration_plan_for_repos(

    source: &crate::store::models::RepoRecord,

    target: &crate::store::models::RepoRecord,

    source_refs: &[String],

    path_mappings: Option<Vec<MappingInput>>,

    mode: MigrationMode,

) -> Result<IntegrationPlanResult, AppError> {

    ensure_different_repos(source, target)?;

    let mappings = resolve_mappings(source, path_mappings);

    let password = decrypt_repo_pass(source)?;

    let ctx = PreviewContext::from_repos(source, target, password, mappings)?;

    let preview = build_preview_plan_parallel(&ctx, source_refs)?;

    Ok(build_integration_plan(

        &preview.aggregated,

        &ctx.target_wc_path,

        target_kind_for_context(&ctx),

        mode,

    ))

}



#[tauri::command]

pub async fn build_integration_plan_cmd(

    state: State<'_, DbState>,

    source_id: String,

    target_id: String,

    source_refs: Vec<String>,

    path_mappings: Option<Vec<MappingInput>>,

    mode: Option<MigrationMode>,

) -> Result<IntegrationPlanResult, AppError> {

    let (source, target, mode) = {

        let conn = state

            .0

            .lock()

            .map_err(|_| AppError::Other(anyhow::anyhow!("db lock")))?;

        let source = get_repo(&conn, &source_id)?

            .ok_or_else(|| AppError::Vcs("source repo not found".into()))?;

        let target = get_repo(&conn, &target_id)?

            .ok_or_else(|| AppError::Vcs("target repo not found".into()))?;

        (source, target, mode.unwrap_or(MigrationMode::IncrementalFirst))

    };



    tokio::task::spawn_blocking(move || {

        integration_plan_for_repos(&source, &target, &source_refs, path_mappings, mode)

    })

    .await

    .map_err(|e| AppError::Other(anyhow::anyhow!("integration plan task: {e}")))?

}



// Legacy alias: returns review + blocked items only.

#[tauri::command]

pub fn detect_conflicts(

    state: State<DbState>,

    source_id: String,

    target_id: String,

    source_refs: Vec<String>,

    path_mappings: Option<Vec<MappingInput>>,

) -> Result<Vec<crate::store::models::IntegrationItemView>, AppError> {

    use crate::store::models::IntegrationStatus;

    let conn = state

        .0

        .lock()

        .map_err(|_| AppError::Other(anyhow::anyhow!("db lock")))?;

    let source = get_repo(&conn, &source_id)?

        .ok_or_else(|| AppError::Vcs("source repo not found".into()))?;

    let target = get_repo(&conn, &target_id)?

        .ok_or_else(|| AppError::Vcs("target repo not found".into()))?;

    let plan = integration_plan_for_repos(

        &source,

        &target,

        &source_refs,

        path_mappings,

        MigrationMode::IncrementalFirst,

    )?;

    Ok(plan

        .items

        .into_iter()

        .filter(|i| {

            matches!(

                i.integration_status,

                IntegrationStatus::Review | IntegrationStatus::Blocked

            )

        })

        .collect())

}



// Legacy preview command kept for tests.

#[tauri::command]

pub fn build_preview(

    source_wc_path: String,

    source_refs: Vec<String>,

    target_wc_path: String,

    mappings: Vec<MappingInput>,

    username: Option<String>,

    password: Option<String>,

) -> Result<crate::model::PreviewResult, AppError> {

    use crate::store::models::RepoRecord;



    let source = RepoRecord {

        id: String::new(),

        name: String::new(),

        path: source_wc_path,

        repo_type: crate::store::models::RepoType::Svn,

        branch: String::new(),

        last_used: None,

        svn_user: username,

        svn_pass_encrypted: None,

        path_mappings: vec![],

    };

    let target = RepoRecord {

        id: String::new(),

        name: String::new(),

        path: target_wc_path.clone(),

        repo_type: crate::store::models::RepoType::Git,

        branch: String::new(),

        last_used: None,

        svn_user: None,

        svn_pass_encrypted: None,

        path_mappings: vec![],

    };

    let ctx = PreviewContext::from_repos(&source, &target, password, mappings)?;

    build_preview_plan_parallel(&ctx, &source_refs).map_err(Into::into)

}

