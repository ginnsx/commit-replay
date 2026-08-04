pub mod commands;
pub mod commit_message;
pub mod diff;
pub mod error;
pub mod process;
pub mod mapper;
pub mod model;
pub mod preview;
pub mod relay;
pub mod store;
pub mod vcs;

use commands::{
    migrate::execute_migration,
    preview::{
        build_integration_plan_cmd, build_preview, build_preview_meta, build_source_preview_meta,
        detect_conflicts, get_file_diff,
    },
    reader::{
        get_repo_mappings, get_repo_pair_mappings, list_relayed_commits_cmd, list_repo_commits,
        load_changeset, save_repo_pair_mappings, validate_migration_combo,
    },
    settings::*,
    writer::{open_file_in_editor, open_file_in_system},
};
use preview::PreviewCache;
use store::db::{init_db, DbState};
use tauri::Manager;

#[tauri::command]
fn ping() -> &'static str {
    "pong"
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            #[cfg(desktop)]
            if let Some(public_key) = option_env!("RELAY_UPDATER_PUBLIC_KEY") {
                app.handle()
                    .plugin(tauri_plugin_updater::Builder::new().pubkey(public_key).build())?;
            }
            let conn = init_db(&app.handle())?;
            app.manage(DbState(std::sync::Mutex::new(conn)));
            app.manage(PreviewCache::new());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ping,
            relay_list_repos,
            relay_save_repo,
            relay_delete_repo,
            relay_list_editors,
            relay_save_editor,
            relay_delete_editor,
            relay_get_default_editor,
            relay_set_default_editor,
            relay_list_migrations,
            relay_get_migration,
            relay_save_migration_record,
            relay_get_update_check_state,
            relay_save_update_check_state,
            relay_pick_folder,
            relay_probe_svn_wc,
            relay_probe_git_repo,
            list_repo_commits,
            list_relayed_commits_cmd,
            load_changeset,
            validate_migration_combo,
            get_repo_mappings,
            get_repo_pair_mappings,
            save_repo_pair_mappings,
            build_preview,
            build_preview_meta,
            build_source_preview_meta,
            get_file_diff,
            build_integration_plan_cmd,
            detect_conflicts,
            execute_migration,
            open_file_in_editor,
            open_file_in_system,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
