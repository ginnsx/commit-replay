use crate::{error::AppError, model::PreviewResult};

/// Build a full preview for the given source refs.
#[tauri::command]
pub fn build_preview(
    source_vcs: String,
    source_url: String,
    source_refs: Vec<String>,
    target_wc_path: String,
) -> Result<PreviewResult, AppError> {
    // TODO(M2): load changesets, apply mapping, read `before`, derive `after`, check apply
    let _ = (source_vcs, source_url, source_refs, target_wc_path);
    Ok(PreviewResult {
        units: vec![],
        aggregated: vec![],
        stats: crate::model::DiffStats {
            files_changed: 0,
            lines_added: 0,
            lines_removed: 0,
            binary_count: 0,
            conflict_risk_count: 0,
        },
    })
}
