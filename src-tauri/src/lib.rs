mod backup;
mod commands;
mod db;
mod diagnostics;
mod importer;
mod generation;
mod generation_index;
mod jobs;
mod migrations;
mod metadata;
mod models;
mod preview;
mod remix;
mod references;
mod semantic;
mod sidecar;
mod visual_dna;
mod vision;
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
    let models_dir=data_dir.join("models");
    fs::create_dir_all(&backups_dir).map_err(|e|e.to_string())?;
    fs::create_dir_all(&models_dir).map_err(|e|e.to_string())?;
    diagnostics::log(&data_dir,"INFO","startup: preparing local library");
    backup::apply_pending_restore(&database_path,&data_dir,&backups_dir)
        .map_err(|e|{diagnostics::log(&data_dir,"ERROR",&format!("pending restore failed: {}",e));e})?;
    let had_database=database_path.exists();
    let connection=match db::init_db(&database_path){
        Ok(conn)=>conn,
        Err(initial_error)=>{
            diagnostics::log(&data_dir,"ERROR",&format!("database initialization failed: {}",initial_error));
            if !had_database{return Err(initial_error)}
            match backup::recover_latest_valid_backup(&database_path,&data_dir,&backups_dir,&initial_error)?{
                Some(record)=>{
                    diagnostics::log(&data_dir,"WARN",&format!("startup recovery applied backup {}",record.name));
                    db::init_db(&database_path).map_err(|retry|format!("资料库自动恢复后仍无法打开：{}；原始错误：{}",retry,initial_error))?
                }
                None=>return Err(format!("资料库无法打开，且没有找到可验证的备份。原始错误：{}",initial_error)),
            }
        }
    };
    let cache_cleanup=cache_dir.clone();
    std::thread::spawn(move||preview::prune_cache(&cache_cleanup,1024*1024*1024));
    diagnostics::log(&data_dir,"INFO","startup: library ready");
    Ok(AppState{
        db:Mutex::new(connection),
        data_dir,
        cache_dir,
        database_path,
        backups_dir,
        models_dir,
        jobs:Mutex::new(std::collections::HashMap::new()),
        next_job_id:std::sync::atomic::AtomicU64::new(1),
        vision_api_key:Mutex::new(std::env::var("IMAGELORE_VISION_API_KEY").unwrap_or_default()),
    })
}

fn write_startup_error(message:&str){
    let root=db::error_log_root();
    let _=fs::create_dir_all(&root);
    diagnostics::log(&root,"ERROR",message);
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
            visual_dna::get_visual_dna,
            visual_dna::update_visual_dna,
            vision::vision_settings,
            vision::save_vision_settings,
            vision::set_vision_api_key,
            vision::latest_image_prompt_analysis,
            vision::analyze_image_to_prompt,
            vision::apply_image_prompt_dna,
            vision::save_image_prompt_revision,
            remix::latest_remix_draft,
            remix::save_remix_draft,
            remix::delete_remix_draft,
            remix::apply_remix_lineage,
            references::reference_sources,
            references::ensure_reference_inbox,
            references::open_reference_url,
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
            commands::save_prompt_revision,
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
            commands::copy_asset_to,
            diagnostics::diagnostics_status,
            diagnostics::open_data_folder,
            diagnostics::open_logs_folder,
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
            sources::start_sync_sources,
            semantic::semantic_status,
            semantic::start_semantic_index,
            semantic::cancel_semantic_index,
            semantic::semantic_search_text,
            semantic::semantic_search_similar,
            semantic::clear_semantic_index,
            semantic::delete_semantic_models
        ])
        .run(tauri::generate_context!())
        .expect("error while running ImageLore");
}
