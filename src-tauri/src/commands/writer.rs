use crate::{error::AppError, model::ValidationResult};

/// Apply and commit a single replay unit. Emits `unit-status` events.
#[tauri::command]
pub fn apply_unit(
    source_ref: String,
    target_vcs: String,
    target_wc_path: String,
    target_branch: String,
    message_template: String,
) -> Result<String, AppError> {
    // TODO(M3): prepare → apply → validate → commit; on fail → rollback + emit event
    let _ = (source_ref, target_vcs, target_wc_path, target_branch, message_template);
    Ok(String::new())
}

/// Re-validate the working copy after manual resolution, then commit.
#[tauri::command]
pub fn commit_resolved(
    source_ref: String,
    target_wc_path: String,
    message_template: String,
) -> Result<ValidationResult, AppError> {
    // TODO(M4): validate → if passed commit → return result
    let _ = (source_ref, target_wc_path, message_template);
    Ok(ValidationResult { passed: false, issues: vec![] })
}

/// Open a file in the system default application.
#[tauri::command]
pub fn open_file_in_system(path: String) -> Result<(), AppError> {
    // TODO(M4): use tauri-plugin-opener or shell::open
    let _ = path;
    Ok(())
}
