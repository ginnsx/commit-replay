pub mod commands;
pub mod error;
pub mod mapper;
pub mod model;
pub mod vcs;

use commands::{preview::build_preview, reader::*, writer::*};

#[tauri::command]
fn ping() -> &'static str {
    "pong"
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            ping,
            list_commits,
            load_changeset,
            build_preview,
            apply_unit,
            commit_resolved,
            open_file_in_system,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
