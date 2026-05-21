use crate::{
    error::AppError,
    model::PreviewResult,
    preview::{build_preview_plan, MappingInput},
};

/// Build a full preview for the given source refs.
#[tauri::command]
pub fn build_preview(
    source_vcs: String,
    source_url: String,
    source_refs: Vec<String>,
    target_wc_path: String,
    mappings: Vec<MappingInput>,
    username: Option<String>,
    password: Option<String>,
) -> Result<PreviewResult, AppError> {
    build_preview_plan(
        &source_vcs,
        &source_url,
        &source_refs,
        &target_wc_path,
        &mappings,
        username,
        password,
    )
}
