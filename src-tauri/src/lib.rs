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
    let connection = db::init_db(&database_path).expect("failed to initialize ImageLore database");

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState { db: Mutex::new(connection), data_dir, cache_dir })
        .invoke_handler(tauri::generate_handler![
            commands::library_page,
            commands::library_facets,
            commands::get_asset,
            importer::import_paths,
            importer::import_folder,
            importer::import_dropped_paths,
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
            commands::create_collection,
            commands::add_to_collection,
            commands::rescan_metadata,
            commands::export_sidecar,
            commands::preview_data_url,
            commands::refresh_missing,
            commands::relocate_missing,
            commands::open_external,
            commands::open_containing_folder
        ])
        .run(tauri::generate_context!())
        .expect("error while running ImageLore");
}
