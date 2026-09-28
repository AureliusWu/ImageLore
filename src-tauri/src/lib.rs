mod backup;
mod commands;
mod db;
mod importer;
mod generation;
mod generation_index;
mod migrations;
mod metadata;
mod models;
mod preview;
mod sidecar;
mod sources;
mod state;

use state::AppState;
use std::{fs,sync::Mutex};
use tauri::Manager;
use tauri_plugin_dialog::{DialogExt,MessageDialogKind};

fn prepare_state()->Result<AppState,String>{
    let(data_dir,cache_dir)=db::data_root()?;
    let database_path=data_dir.join("library.sqlite3");
    let backups_dir=data_dir.join("backups");
    fs::create_dir_all(&backups_dir).map_err(|e|e.to_string())?;
    backup::apply_pending_restore(&database_path,&data_dir,&backups_dir)?;
    let connection=db::init_db(&database_path)?;
    let cache_cleanup=cache_dir.clone();
    std::thread::spawn(move||preview::prune_cache(&cache_cleanup,1024*1024*1024));
    Ok(AppState{
        db:Mutex::new(connection),
        data_dir,
        cache_dir,
        database_path,
        backups_dir,
        import_jobs:Mutex::new(std::collections::HashMap::new()),
        next_job_id:std::sync::atomic::AtomicU64::new(1),
    })
}

fn write_startup_error(message:&str){
    let root=dirs::data_local_dir().map(|x|x.join("ImageLore")).unwrap_or_else(std::env::temp_dir);
    let _=fs::create_dir_all(&root);
    let _=fs::write(root.join("startup-error.log"),message);
}

#[cfg_attr(mobile,tauri::mobile_entry_point)]
pub fn run(){
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app|{
            match prepare_state(){
                Ok(state)=>{
                    app.manage(state);
                }
                Err(error)=>{
                    let message=format!("ImageLore 无法安全打开资料库。\n\n{}\n\n错误详情已写入 startup-error.log。",error);
                    write_startup_error(&message);
                    if let Some(window)=app.get_webview_window("main"){let _=window.hide();}
                    let handle=app.handle().clone();
                    app.dialog()
                        .message(message)
                        .kind(MessageDialogKind::Error)
                        .title("ImageLore 启动失败")
                        .show(move |_|handle.exit(1));
                }
            }
            Ok(())
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
            commands::open_containing_folder,
            generation::generation_sessions,
            generation::create_generation_session,
            generation::asset_session,
            generation::set_asset_session,
            generation::update_relation_note,
            generation::model_aliases,
            generation::upsert_model_alias,
            generation::delete_model_alias,
            generation::saved_filters,
            generation::save_filter,
            generation::delete_saved_filter,
            generation::library_health,
            sources::source_folders,
            sources::add_source_folder,
            sources::remove_source_folder,
            sources::set_source_auto_sync,
            sources::start_sync_sources
        ])
        .run(tauri::generate_context!())
        .expect("error while running ImageLore");
}
