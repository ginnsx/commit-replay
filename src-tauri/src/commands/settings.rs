use tauri::State;

use crate::error::AppError;
use crate::store::db::{
    delete_editor, delete_repo, get_default_editor_id, get_migration, list_editors, list_migrations,
    list_repos, save_editor, save_migration, save_repo, set_default_editor, DbState,
};
use crate::store::models::{EditorRecord, MigrationRecord, RepoInput, RepoView};

#[tauri::command]
pub fn relay_list_repos(state: State<DbState>) -> Result<Vec<RepoView>, AppError> {
    let conn = state.0.lock().map_err(|_| AppError::Other(anyhow::anyhow!("db lock")))?;
    list_repos(&conn)
}

#[tauri::command]
pub fn relay_save_repo(state: State<DbState>, input: RepoInput) -> Result<RepoView, AppError> {
    let conn = state.0.lock().map_err(|_| AppError::Other(anyhow::anyhow!("db lock")))?;
    save_repo(&conn, input)
}

#[tauri::command]
pub fn relay_delete_repo(state: State<DbState>, id: String) -> Result<(), AppError> {
    let conn = state.0.lock().map_err(|_| AppError::Other(anyhow::anyhow!("db lock")))?;
    delete_repo(&conn, &id)
}

#[tauri::command]
pub fn relay_list_editors(state: State<DbState>) -> Result<Vec<EditorRecord>, AppError> {
    let conn = state.0.lock().map_err(|_| AppError::Other(anyhow::anyhow!("db lock")))?;
    list_editors(&conn)
}

#[tauri::command]
pub fn relay_save_editor(state: State<DbState>, editor: EditorRecord) -> Result<EditorRecord, AppError> {
    let conn = state.0.lock().map_err(|_| AppError::Other(anyhow::anyhow!("db lock")))?;
    save_editor(&conn, editor)
}

#[tauri::command]
pub fn relay_delete_editor(state: State<DbState>, id: String) -> Result<(), AppError> {
    let conn = state.0.lock().map_err(|_| AppError::Other(anyhow::anyhow!("db lock")))?;
    delete_editor(&conn, &id)
}

#[tauri::command]
pub fn relay_get_default_editor(state: State<DbState>) -> Result<String, AppError> {
    let conn = state.0.lock().map_err(|_| AppError::Other(anyhow::anyhow!("db lock")))?;
    get_default_editor_id(&conn)
}

#[tauri::command]
pub fn relay_set_default_editor(state: State<DbState>, id: String) -> Result<(), AppError> {
    let conn = state.0.lock().map_err(|_| AppError::Other(anyhow::anyhow!("db lock")))?;
    set_default_editor(&conn, &id)
}

#[tauri::command]
pub fn relay_list_migrations(state: State<DbState>) -> Result<Vec<MigrationRecord>, AppError> {
    let conn = state.0.lock().map_err(|_| AppError::Other(anyhow::anyhow!("db lock")))?;
    list_migrations(&conn)
}

#[tauri::command]
pub fn relay_get_migration(
    state: State<DbState>,
    id: String,
) -> Result<Option<MigrationRecord>, AppError> {
    let conn = state.0.lock().map_err(|_| AppError::Other(anyhow::anyhow!("db lock")))?;
    get_migration(&conn, &id)
}

#[tauri::command]
pub fn relay_save_migration_record(
    state: State<DbState>,
    record: MigrationRecord,
) -> Result<MigrationRecord, AppError> {
    let conn = state.0.lock().map_err(|_| AppError::Other(anyhow::anyhow!("db lock")))?;
    save_migration(&conn, record)
}

#[tauri::command]
pub async fn relay_pick_folder(app: tauri::AppHandle) -> Result<Option<String>, AppError> {
    use tauri_plugin_dialog::DialogExt;
    let path = app.dialog().file().blocking_pick_folder();
    Ok(path.map(|p| p.to_string()))
}

#[tauri::command]
pub fn relay_probe_svn_wc(path: String) -> Result<crate::vcs::svn::SvnWcInfo, AppError> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation("path is required".into()));
    }
    if !std::path::Path::new(trimmed).exists() {
        return Err(AppError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("path not found: {trimmed}"),
        )));
    }
    crate::vcs::svn::probe_svn_wc(trimmed)
}
