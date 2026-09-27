mod backup;
mod commands;
mod db;
mod importer;
mod migrations;
mod metadata;
mod models;
mod preview;
mod sidecar;
mod state;

use state::AppState;
use std::sync::Mutex;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let (data_dir, cache_dir) = db::data_root().expect("failed to prepare ImageLore data directory");
    let database_path = data_dir.join("library.sqlite3");
    let backups_dir = data_dir.join("backups");
    std::fs::create_dir_all(&backups_dir).expect("failed to prepare ImageLore backup directory");
    backup::apply_pending_restore(&database_path,&data_dir,&backups_dir).expect("failed to apply pending ImageLore restore");
    let connection = db::init_db(&database_path).expect("failed to initialize ImageLore database");
    let cache_cleanup = cache_dir.clone();
    std::thread::spawn(move || preview::prune_cache(&cache_cleanup, 1024 * 1024 * 1024));

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            db: Mutex::new(connection),
            data_dir,
            cache_dir,
            database_path,
            backups_dir,
            import_jobs: Mutex::new(std::collections::HashMap::new()),
            next_job_id: std::sync::atomic::AtomicU64::new(1),
        })
        .invoke_handler(tauri::generate_handler![
            commands::library_page,
            backup::create_backup,
            backup::ensure_auto_backup,
            backup::list_backups,
            backup::stage_restore,
            commands::library_facets,
            commands::get_asset,
            importer::import_paths,
            importer::import_folder,
            importer::import_dropped_paths,
            importer::start_import_paths,
            importer::start_import_folder,
            importer::start_import_dropped_paths,
            importer::cancel_import,
            commands::update_prompt,
            commands::replace_tags,
            commands::batch_add_tags,
            commands::toggle_favorite,
            commands::delete_asset,
            commands::add_revision,
            commands::list_revisions,
            commands::restore_revision,
            commands::add_relation,
            commands::lineage,
            commands::collections,
            commands::duplicate_groups,
            commands::rename_tag,
            commands::merge_tags,
            commands::delete_tag,
            commands::rename_collection,
            commands::delete_collection,
            commands::create_collection,
            commands::add_to_collection,
            commands::rescan_metadata,
            commands::export_sidecar,
            commands::preview_cache_path,
            commands::refresh_missing,
            commands::relocate_missing,
            commands::open_external,
            commands::open_containing_folder
        ])
        .run(tauri::generate_context!())
        .expect("error while running ImageLore");
}
