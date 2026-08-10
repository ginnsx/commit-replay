use tauri::State;

use crate::{
    comparison::{compare_record_files, legacy_identity_pairs, unavailable_comparison},
    error::AppError,
    preview::build_source_preview_plan_meta,
    store::{
        db::{decrypt_repo_pass, get_migration, get_repo, list_repos, DbState},
        models::{
            MigrationComparisonResult, MigrationFilePair, MigrationRecord, RepoRecord, RepoType,
        },
    },
    vcs::SourceReader,
};

#[tauri::command]
pub async fn compare_migration_files(
    state: State<'_, DbState>,
    migration_id: String,
) -> Result<MigrationComparisonResult, AppError> {
    let (record, legacy_source) = {
        let conn = state
            .0
            .lock()
            .map_err(|_| AppError::Other(anyhow::anyhow!("db lock")))?;
        let record = get_migration(&conn, &migration_id)?
            .ok_or_else(|| AppError::Validation("迁移记录不存在".into()))?;
        let legacy_source = if record.comparison_files.is_empty() {
            legacy_source_repo(&conn, &record)?
        } else {
            None
        };
        (record, legacy_source)
    };

    tokio::task::spawn_blocking(move || {
        let pairs = if record.comparison_files.is_empty() {
            match legacy_pairs(&record, legacy_source) {
                Ok(pairs) => pairs,
                Err(reason) => return Ok(unavailable_comparison(&record, reason)),
            }
        } else {
            record.comparison_files.clone()
        };
        Ok(compare_record_files(&record, &pairs))
    })
    .await
    .map_err(|err| AppError::Other(anyhow::anyhow!("comparison task: {err}")))?
}

fn legacy_source_repo(
    conn: &rusqlite::Connection,
    record: &MigrationRecord,
) -> Result<Option<(RepoRecord, Option<String>)>, AppError> {
    let source_id = list_repos(conn)?
        .into_iter()
        .find(|repo| same_repo_snapshot(repo, record))
        .map(|repo| repo.id);
    let Some(source_id) = source_id else {
        return Ok(None);
    };
    let Some(source) = get_repo(conn, &source_id)? else {
        return Ok(None);
    };
    let password = decrypt_repo_pass(&source)?;
    Ok(Some((source, password)))
}

fn same_repo_snapshot(repo: &crate::store::models::RepoView, record: &MigrationRecord) -> bool {
    repo_path_key(&repo.path) == repo_path_key(&record.source.path)
        && matches!(
            (&repo.repo_type, &record.source.repo_type),
            (RepoType::Git, RepoType::Git) | (RepoType::Svn, RepoType::Svn)
        )
}

fn repo_path_key(path: &str) -> String {
    path.replace('\\', "/").trim_end_matches('/').to_lowercase()
}

fn legacy_pairs(
    record: &MigrationRecord,
    source: Option<(RepoRecord, Option<String>)>,
) -> Result<Vec<MigrationFilePair>, String> {
    let Some((source, password)) = source else {
        return Err("旧记录缺少已保存的源仓库信息，无法安全对比".into());
    };
    if record.commits.is_empty() {
        return Err("旧记录没有可用于恢复路径的提交信息，无法安全对比".into());
    }
    let reader = SourceReader::from_repo(&source, password)
        .map_err(|_| "无法创建旧记录的源仓库读取器".to_string())?;
    let refs: Vec<String> = record
        .commits
        .iter()
        .map(|commit| commit.id.clone())
        .collect();
    let source_files = build_source_preview_plan_meta(&reader, &refs)
        .map_err(|_| "无法重新读取旧记录的源提交，无法安全对比".to_string())?;
    legacy_identity_pairs(record, &source_files)
}
