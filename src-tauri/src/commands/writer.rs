use std::process::Command;

use tauri::State;

use crate::error::AppError;
use crate::store::db::{list_editors, DbState};

#[tauri::command]
pub fn open_file_in_editor(
    state: State<DbState>,
    editor_id: String,
    file_path: String,
) -> Result<(), AppError> {
    let conn = state.0.lock().map_err(|_| AppError::Other(anyhow::anyhow!("db lock")))?;
    let editors = list_editors(&conn)?;
    let editor = editors
        .iter()
        .find(|e| e.id == editor_id)
        .ok_or_else(|| AppError::Validation("editor not found".into()))?;
    let exe = crate::store::editor_paths::resolve_editor_exe(&editor.kind, &editor.exe);

    let path = std::path::Path::new(&file_path);
    if !path.exists() {
        return Err(AppError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("file not found: {file_path}"),
        )));
    }

    let status = match editor.kind.as_str() {
        "explorer" => Command::new(&exe)
            .arg("/select,")
            .arg(&file_path)
            .status(),
        "gitbash" => Command::new(&exe)
            .args(["-c", &format!("start '' '{file_path}'")])
            .status(),
        "terminal" => Command::new(&exe)
            .args(["-d", std::path::Path::new(&file_path).parent().unwrap_or(path).to_str().unwrap_or(".")])
            .status(),
        _ => Command::new(&exe).arg(&file_path).status(),
    }
    .map_err(|e| AppError::Io(e))?;

    if status.success() {
        Ok(())
    } else {
        Err(AppError::Other(anyhow::anyhow!(
            "failed to open editor {}",
            editor.name
        )))
    }
}

#[tauri::command]
pub fn open_file_in_system(app: tauri::AppHandle, path: String) -> Result<(), AppError> {
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_path(path, None::<&str>)
        .map_err(|e| AppError::Io(std::io::Error::other(e.to_string())))
}
