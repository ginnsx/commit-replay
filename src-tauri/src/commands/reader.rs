use crate::{error::AppError, model::ReplayUnitMeta};

/// List recent commits from a source VCS.
/// `source_vcs`: "svn" | "git"
#[tauri::command]
pub fn list_commits(
    source_vcs: String,
    url: String,
    limit: usize,
) -> Result<Vec<ReplayUnitMeta>, AppError> {
    // TODO(M1): instantiate the correct VcsReader and call list_recent
    let _ = (source_vcs, url, limit);
    Ok(vec![])
}
