use tauri::State;

use crate::{
    error::AppError,
    preview::{build_preview_plan_parallel, get_aggregated_files, build_integration_plan, MappingInput},
    relay::{preview_file_meta, preview_file_with_diff},
    store::{
        db::{decrypt_repo_pass, get_repo, repo_path_mappings, DbState},
        models::{FileChangeView, IntegrationPlanResult, MigrationMode, RepoType},
    },
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

fn load_repos(
    state: &DbState,
    source_id: &str,
    target_id: &str,
    path_mappings: Option<Vec<MappingInput>>,
) -> Result<
    (
        String,
        Option<String>,
        Option<String>,
        Vec<MappingInput>,
        String,
    ),
    AppError,
> {
    let conn = state.0.lock().map_err(|_| AppError::Other(anyhow::anyhow!("db lock")))?;
    let source = get_repo(&conn, source_id)?
        .ok_or_else(|| AppError::Vcs("source repo not found".into()))?;
    let target = get_repo(&conn, target_id)?
        .ok_or_else(|| AppError::Vcs("target repo not found".into()))?;
    if !matches!(source.repo_type, RepoType::Svn) {
        return Err(AppError::Vcs("source must be SVN".into()));
    }
    if !matches!(target.repo_type, RepoType::Git) {
        return Err(AppError::Vcs("target must be Git".into()));
    }
    let mappings = resolve_mappings(&source, path_mappings);
    let password = decrypt_repo_pass(&source)?;
    Ok((
        source.path.clone(),
        source.svn_user.clone(),
        password,
        mappings,
        target.path.clone(),
    ))
}

#[tauri::command]
pub fn build_preview_meta(
    state: State<DbState>,
    source_id: String,
    target_id: String,
    source_refs: Vec<String>,
    path_mappings: Option<Vec<MappingInput>>,
) -> Result<PreviewMetaResult, AppError> {
    let (source_path, username, password, mappings, target_path) =
        load_repos(&state, &source_id, &target_id, path_mappings)?;
    let aggregated = get_aggregated_files(
        &source_path,
        &source_refs,
        &target_path,
        &mappings,
        username,
        password,
    )?;
    let files: Vec<FileChangeView> = aggregated.iter().map(preview_file_meta).collect();
    let adds = files.iter().filter(|f| matches!(f.status, crate::store::models::FileStatus::Add)).count() as u32;
    let mods = files.iter().filter(|f| matches!(f.status, crate::store::models::FileStatus::Mod)).count() as u32;
    let dels = files.iter().filter(|f| matches!(f.status, crate::store::models::FileStatus::Del)).count() as u32;
    let total_additions = files.iter().map(|f| f.additions).sum();
    let total_deletions = files.iter().map(|f| f.deletions).sum();
    Ok(PreviewMetaResult {
        files,
        adds,
        mods,
        dels,
        total_additions,
        total_deletions,
    })
}

#[tauri::command]
pub fn get_file_diff(
    state: State<DbState>,
    source_id: String,
    target_id: String,
    source_refs: Vec<String>,
    file_path: String,
    path_mappings: Option<Vec<MappingInput>>,
) -> Result<FileChangeView, AppError> {
    let (source_path, username, password, mappings, target_path) =
        load_repos(&state, &source_id, &target_id, path_mappings)?;
    let aggregated = get_aggregated_files(
        &source_path,
        &source_refs,
        &target_path,
        &mappings,
        username,
        password,
    )?;
    let fc = aggregated
        .iter()
        .find(|f| f.target_path.as_deref() == Some(file_path.as_str()) || f.path == file_path)
        .ok_or_else(|| AppError::Vcs(format!("file not found in preview: {file_path}")))?;
    Ok(preview_file_with_diff(fc))
}

fn integration_plan_for_repos(
    state: &DbState,
    source_id: &str,
    target_id: &str,
    source_refs: &[String],
    path_mappings: Option<Vec<MappingInput>>,
    mode: MigrationMode,
) -> Result<IntegrationPlanResult, AppError> {
    let (source_path, username, password, mappings, target_path) =
        load_repos(state, source_id, target_id, path_mappings)?;
    let preview = build_preview_plan_parallel(
        &source_path,
        source_refs,
        &target_path,
        &mappings,
        username,
        password,
    )?;
    Ok(build_integration_plan(
        &preview.aggregated,
        &target_path,
        mode,
    ))
}

#[tauri::command]
pub fn build_integration_plan_cmd(
    state: State<DbState>,
    source_id: String,
    target_id: String,
    source_refs: Vec<String>,
    path_mappings: Option<Vec<MappingInput>>,
    mode: Option<MigrationMode>,
) -> Result<IntegrationPlanResult, AppError> {
    integration_plan_for_repos(
        &state,
        &source_id,
        &target_id,
        &source_refs,
        path_mappings,
        mode.unwrap_or(MigrationMode::IncrementalFirst),
    )
}

/// Legacy alias — returns review + blocked items only.
#[tauri::command]
pub fn detect_conflicts(
    state: State<DbState>,
    source_id: String,
    target_id: String,
    source_refs: Vec<String>,
    path_mappings: Option<Vec<MappingInput>>,
) -> Result<Vec<crate::store::models::IntegrationItemView>, AppError> {
    use crate::store::models::IntegrationStatus;
    let plan = integration_plan_for_repos(
        &state,
        &source_id,
        &target_id,
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

/// Legacy preview command kept for tests.
#[tauri::command]
pub fn build_preview(
    source_wc_path: String,
    source_refs: Vec<String>,
    target_wc_path: String,
    mappings: Vec<MappingInput>,
    username: Option<String>,
    password: Option<String>,
) -> Result<crate::model::PreviewResult, AppError> {
    build_preview_plan_parallel(
        &source_wc_path,
        &source_refs,
        &target_wc_path,
        &mappings,
        username,
        password,
    )
    .map_err(Into::into)
}
