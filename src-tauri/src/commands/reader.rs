use crate::{
    error::AppError,
    model::{ChangeSet, ReplayUnitMeta},
    vcs::{svn_reader::SvnReader, VcsReader},
};

fn svn_reader(
    url: String,
    username: Option<String>,
    password: Option<String>,
) -> SvnReader {
    SvnReader {
        url,
        username,
        password,
    }
}

/// List recent commits from a source VCS.
/// `source_vcs`: `"svn"` | `"git"` (git not yet implemented).
#[tauri::command]
pub fn list_commits(
    source_vcs: String,
    url: String,
    limit: usize,
    username: Option<String>,
    password: Option<String>,
) -> Result<Vec<ReplayUnitMeta>, AppError> {
    match source_vcs.as_str() {
        "svn" => svn_reader(url, username, password).list_recent(limit),
        other => Err(AppError::Vcs(format!(
            "unsupported source VCS: {other}. Use 'svn'."
        ))),
    }
}

/// Load a single commit's file-level changes from the source VCS.
#[tauri::command]
pub fn load_changeset(
    source_vcs: String,
    url: String,
    source_ref: String,
    username: Option<String>,
    password: Option<String>,
) -> Result<ChangeSet, AppError> {
    match source_vcs.as_str() {
        "svn" => svn_reader(url, username, password).load_changeset(&source_ref),
        other => Err(AppError::Vcs(format!(
            "unsupported source VCS: {other}. Use 'svn'."
        ))),
    }
}
