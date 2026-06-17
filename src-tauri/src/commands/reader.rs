use tauri::State;

use crate::error::AppError;
use crate::model::ReplayUnitMeta;
use crate::relay::{validate_repo_path, is_supported_migration, unsupported_combo_message};
use crate::store::db::{decrypt_repo_pass, get_repo, get_repo_pair_mapping, repo_path_mappings, save_repo_pair_mapping, touch_repo, DbState};
use crate::store::models::RepoPairMappingInput;
use crate::vcs::{ensure_different_repos, repo_type_str, SourceReader};

#[derive(Debug, serde::Serialize)]
pub struct CommitListItem {
    pub id: String,
    pub hash: String,
    pub msg: String,
    pub author: String,
    pub date: String,
    pub files: usize,
    pub source_ref: String,
}

fn meta_to_item(meta: &ReplayUnitMeta) -> CommitListItem {
    let rev = meta
        .source_ref
        .strip_prefix("svn:")
        .or_else(|| meta.source_ref.strip_prefix("git:"))
        .unwrap_or(&meta.source_ref);
    CommitListItem {
        id: meta.source_ref.clone(),
        hash: rev.chars().take(7).collect(),
        msg: meta.message.clone(),
        author: meta.author.clone(),
        date: format_date_display(&meta.date),
        files: meta.changed_paths_count,
        source_ref: meta.source_ref.clone(),
    }
}

fn format_date_display(iso: &str) -> String {
    use chrono::{DateTime, Local, NaiveDateTime, TimeZone, Utc};

    if let Ok(dt) = DateTime::parse_from_rfc3339(iso) {
        return dt.with_timezone(&Local).format("%Y-%m-%d %H:%M").to_string();
    }
    if let Ok(naive) = NaiveDateTime::parse_from_str(iso, "%Y-%m-%dT%H:%M:%S%.f") {
        return Utc
            .from_utc_datetime(&naive)
            .with_timezone(&Local)
            .format("%Y-%m-%d %H:%M")
            .to_string();
    }
    if iso.len() >= 16 {
        iso.replace('T', " ").chars().take(16).collect()
    } else {
        iso.to_string()
    }
}

fn reader_from_repo(state: &DbState, repo_id: &str) -> Result<(SourceReader, String), AppError> {
    let conn = state
        .0
        .lock()
        .map_err(|_| AppError::Other(anyhow::anyhow!("db lock")))?;
    let repo = get_repo(&conn, repo_id)?
        .ok_or_else(|| AppError::Vcs(format!("repo not found: {repo_id}")))?;
    validate_repo_path(&repo.path)?;
    let password = decrypt_repo_pass(&repo)?;
    let reader = SourceReader::from_repo(&repo, password)?;
    Ok((reader, repo.id.clone()))
}

#[tauri::command]
pub async fn list_repo_commits(
    state: State<'_, DbState>,
    repo_id: String,
    limit: usize,
    before_cursor: Option<String>,
) -> Result<Vec<CommitListItem>, AppError> {
    let (reader, id) = reader_from_repo(&state, &repo_id)?;
    let limit = limit.clamp(1, 500);
    let cursor = before_cursor;
    let metas = tokio::task::spawn_blocking(move || {
        reader.list_recent_paged(limit, cursor.as_deref())
    })
        .await
        .map_err(|e| AppError::Other(anyhow::anyhow!("list commits task: {e}")))??;

    let conn = state
        .0
        .lock()
        .map_err(|_| AppError::Other(anyhow::anyhow!("db lock")))?;
    let _ = touch_repo(&conn, &id);
    Ok(metas.iter().map(meta_to_item).collect())
}

#[tauri::command]
pub fn load_changeset(
    state: State<DbState>,
    repo_id: String,
    source_ref: String,
) -> Result<crate::model::ChangeSet, AppError> {
    let (reader, _) = reader_from_repo(&state, &repo_id)?;
    reader.load_changeset(&source_ref).map_err(Into::into)
}

#[tauri::command]
pub fn validate_migration_combo(
    state: State<DbState>,
    source_id: String,
    target_id: String,
) -> Result<bool, AppError> {
    let conn = state
        .0
        .lock()
        .map_err(|_| AppError::Other(anyhow::anyhow!("db lock")))?;
    let source = get_repo(&conn, &source_id)?
        .ok_or_else(|| AppError::Vcs("source repo not found".into()))?;
    let target = get_repo(&conn, &target_id)?
        .ok_or_else(|| AppError::Vcs("target repo not found".into()))?;
    let st = repo_type_str(&source.repo_type);
    let tt = repo_type_str(&target.repo_type);
    if !is_supported_migration(st, tt) {
        return Err(AppError::Validation(unsupported_combo_message(st, tt)));
    }
    ensure_different_repos(&source, &target)?;
    Ok(true)
}

#[tauri::command]
pub fn get_repo_mappings(
    state: State<DbState>,
    repo_id: String,
) -> Result<Vec<crate::mapper::PathMapping>, AppError> {
    let conn = state
        .0
        .lock()
        .map_err(|_| AppError::Other(anyhow::anyhow!("db lock")))?;
    let repo = get_repo(&conn, &repo_id)?
        .ok_or_else(|| AppError::Vcs("repo not found".into()))?;
    Ok(repo_path_mappings(&repo))
}

#[tauri::command]
pub fn get_repo_pair_mappings(
    state: State<DbState>,
    source_id: String,
    target_id: String,
) -> Result<Option<crate::store::models::RepoPairMappingView>, AppError> {
    let conn = state
        .0
        .lock()
        .map_err(|_| AppError::Other(anyhow::anyhow!("db lock")))?;
    get_repo_pair_mapping(&conn, &source_id, &target_id)
}

#[tauri::command]
pub fn save_repo_pair_mappings(
    state: State<DbState>,
    input: RepoPairMappingInput,
) -> Result<crate::store::models::RepoPairMappingView, AppError> {
    let conn = state
        .0
        .lock()
        .map_err(|_| AppError::Other(anyhow::anyhow!("db lock")))?;
    save_repo_pair_mapping(&conn, input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_date_converts_utc_to_local() {
        let local = format_date_display("2026-06-16T09:07:24.953037Z");
        let expected = chrono::DateTime::parse_from_rfc3339("2026-06-16T09:07:24.953037Z")
            .unwrap()
            .with_timezone(&chrono::Local)
            .format("%Y-%m-%d %H:%M")
            .to_string();
        assert_eq!(local, expected);
    }
}
