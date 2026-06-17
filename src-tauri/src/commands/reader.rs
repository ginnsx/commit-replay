use tauri::State;

use crate::error::AppError;
use crate::model::ReplayUnitMeta;
use crate::relay::{validate_repo_path};
use crate::store::db::{decrypt_repo_pass, get_repo, repo_path_mappings, touch_repo, DbState};
use crate::store::models::RepoType;
use crate::vcs::{svn_reader::SvnReader, VcsReader};

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
    let rev = meta.source_ref.strip_prefix("svn:").unwrap_or(&meta.source_ref);
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

fn svn_reader_from_repo(
    state: &DbState,
    repo_id: &str,
) -> Result<(SvnReader, String), AppError> {
    let conn = state.0.lock().map_err(|_| AppError::Other(anyhow::anyhow!("db lock")))?;
    let repo = get_repo(&conn, repo_id)?
        .ok_or_else(|| AppError::Vcs(format!("repo not found: {repo_id}")))?;
    if !matches!(repo.repo_type, RepoType::Svn) {
        return Err(AppError::Vcs("repo must be SVN for this operation".into()));
    }
    validate_repo_path(&repo.path)?;
    let password = decrypt_repo_pass(&repo)?;
    let reader = SvnReader {
        wc_path: repo.path.clone(),
        username: repo.svn_user.clone(),
        password,
    };
    Ok((reader, repo.id.clone()))
}

#[tauri::command]
pub async fn list_repo_commits(
    state: State<'_, DbState>,
    repo_id: String,
    limit: usize,
    before_revision: Option<u64>,
) -> Result<Vec<CommitListItem>, AppError> {
    let (reader, id) = svn_reader_from_repo(&state, &repo_id)?;
    let limit = limit.clamp(1, 500);

    let metas = tokio::task::spawn_blocking(move || reader.list_recent_paged(limit, before_revision))
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
    let (reader, _) = svn_reader_from_repo(&state, &repo_id)?;
    reader.load_changeset(&source_ref).map_err(Into::into)
}

#[tauri::command]
pub fn validate_migration_combo(
    state: State<DbState>,
    source_id: String,
    target_id: String,
) -> Result<bool, AppError> {
    let conn = state.0.lock().map_err(|_| AppError::Other(anyhow::anyhow!("db lock")))?;
    let source = get_repo(&conn, &source_id)?
        .ok_or_else(|| AppError::Vcs("source repo not found".into()))?;
    let target = get_repo(&conn, &target_id)?
        .ok_or_else(|| AppError::Vcs("target repo not found".into()))?;
    let st = match source.repo_type {
        RepoType::Svn => "svn",
        RepoType::Git => "git",
    };
    let tt = match target.repo_type {
        RepoType::Svn => "svn",
        RepoType::Git => "git",
    };
    if !crate::relay::is_supported_migration(st, tt) {
        return Err(AppError::Validation(crate::relay::unsupported_combo_message(
            st, tt,
        )));
    }
    Ok(true)
}

#[tauri::command]
pub fn get_repo_mappings(
    state: State<DbState>,
    repo_id: String,
) -> Result<Vec<crate::mapper::PathMapping>, AppError> {
    let conn = state.0.lock().map_err(|_| AppError::Other(anyhow::anyhow!("db lock")))?;
    let repo = get_repo(&conn, &repo_id)?
        .ok_or_else(|| AppError::Vcs("repo not found".into()))?;
    Ok(repo_path_mappings(&repo))
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
