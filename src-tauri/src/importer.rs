use crate::{db,metadata,models::{ImportProgress,ImportSummary},sidecar,state::AppState};
use rusqlite::params;
use std::{
    path::{Path,PathBuf},
    sync::{atomic::{AtomicBool,Ordering},Arc},
};
use tauri::{AppHandle,Emitter,Manager,State};
use walkdir::WalkDir;

struct PreparedAsset{
    path:String,
    name:String,
    width:Option<i64>,
    height:Option<i64>,
    file_size:Option<i64>,
    format:String,
    mime_type:String,
    metadata_type:String,
    generation_json:String,
    fingerprint:String,
    file_mtime:i64,
    prompt:String,
    negative_prompt:String,
    model:String,
    tags:Vec<String>,
}

enum InsertOutcome{Added(i64),Existing(i64),Duplicate(i64)}

fn supported_files(roots:Vec<String>,recursive_dirs:bool)->Vec<PathBuf>{
    let mut out=Vec::new();
    for raw in roots{
        let root=PathBuf::from(raw);
        if root.is_file(){
            if metadata::is_supported(&root){out.push(root)}
        }else if root.is_dir()&&recursive_dirs{
            out.extend(
                WalkDir::new(root).follow_links(false).into_iter().filter_map(Result::ok)
                    .filter(|e|e.file_type().is_file()&&metadata::is_supported(e.path()))
                    .map(|e|e.into_path())
            );
        }
    }
    out
}

fn prepare(path:&Path)->Option<PreparedAsset>{
    if !path.is_file()||!metadata::is_supported(path){return None}
    let canonical=path.canonicalize().unwrap_or_else(|_|path.to_path_buf());
    let info=metadata::file_info(&canonical);
    let extract=metadata::extract_generation(&canonical);
    let saved=sidecar::read(&canonical);
    let prompt=saved.as_ref().map(|x|sidecar::text(x,"prompt")).filter(|x|!x.is_empty()).unwrap_or(extract.prompt);
    let negative_prompt=saved.as_ref().map(|x|sidecar::text(x,"negative_prompt")).filter(|x|!x.is_empty()).unwrap_or(extract.negative_prompt);
    let model=saved.as_ref().map(|x|sidecar::text(x,"model")).filter(|x|!x.is_empty()).unwrap_or(extract.model);
    let tags=saved.as_ref().map(sidecar::tags).unwrap_or_default();
    let fingerprint=metadata::fingerprint(&canonical);
    let name=canonical.file_name().and_then(|x|x.to_str()).unwrap_or("image").to_string();
    Some(PreparedAsset{
        path:canonical.to_string_lossy().to_string(),name,width:info.width,height:info.height,
        file_size:info.file_size,format:info.format,mime_type:info.mime_type,
        metadata_type:extract.metadata_type,generation_json:extract.generation_json,
        fingerprint,file_mtime:info.file_mtime,prompt,negative_prompt,model,tags,
    })
}

fn insert(state:&AppState,item:PreparedAsset)->Result<InsertOutcome,String>{
    let mut conn=state.db.lock().map_err(|e|e.to_string())?;
    if let Some(id)=db::asset_exists_by_path(&conn,&item.path)?{
        conn.execute("UPDATE assets SET missing=0,updated_at=?1 WHERE id=?2",params![db::now(),id]).map_err(|e|e.to_string())?;
        return Ok(InsertOutcome::Existing(id))
    }
    if let Some(id)=db::asset_exists_by_fingerprint(&conn,&item.fingerprint)?{
        return Ok(InsertOutcome::Duplicate(id))
    }

    let stamp=db::now();
    let tx=conn.transaction().map_err(|e|e.to_string())?;
    tx.execute(
        "INSERT INTO assets(path,name,favorite,width,height,file_size,format,mime_type,metadata_type,generation_json,fingerprint,file_mtime,missing,created_at,updated_at) VALUES(?1,?2,0,?3,?4,?5,?6,?7,?8,?9,?10,?11,0,?12,?12)",
        params![item.path,item.name,item.width,item.height,item.file_size,item.format,item.mime_type,item.metadata_type,item.generation_json,item.fingerprint,item.file_mtime,stamp]
    ).map_err(|e|e.to_string())?;
    let id=tx.last_insert_rowid();
    tx.execute("INSERT INTO prompt_state(asset_id,prompt,negative_prompt,model,updated_at) VALUES(?1,?2,?3,?4,?5)",params![id,item.prompt,item.negative_prompt,item.model,stamp]).map_err(|e|e.to_string())?;
    db::replace_tags_raw(&tx,id,&item.tags)?;
    db::reindex_asset(&tx,id)?;
    tx.commit().map_err(|e|e.to_string())?;
    Ok(InsertOutcome::Added(id))
}

fn run_import<F>(
    state:&AppState,
    roots:Vec<String>,
    recursive_dirs:bool,
    cancel:&AtomicBool,
    mut progress:F,
)->Result<ImportSummary,String>
where F:FnMut(ImportProgress){
    let paths=supported_files(roots,recursive_dirs);
    let total=paths.len();
    let mut result=ImportSummary{added:0,skipped:0,duplicates:0,failed:0,last_id:None};

    for(index,path)in paths.into_iter().enumerate(){
        if cancel.load(Ordering::Relaxed){break}
        let canonical=path.canonicalize().unwrap_or_else(|_|path.clone());
        let path_text=canonical.to_string_lossy().to_string();
        let name=canonical.file_name().and_then(|x|x.to_str()).unwrap_or("image").to_string();

        {
            let conn=state.db.lock().map_err(|e|e.to_string())?;
            if let Some(id)=db::asset_exists_by_path(&conn,&path_text)?{
                conn.execute("UPDATE assets SET missing=0 WHERE id=?1",params![id]).map_err(|e|e.to_string())?;
                result.skipped+=1;result.last_id=Some(id);
                progress(ImportProgress{
                    job_id:0,processed:index+1,total,added:result.added,skipped:result.skipped,
                    duplicates:result.duplicates,failed:result.failed,current_name:name,done:false,cancelled:false,last_id:result.last_id
                });
                continue;
            }
        }

        match prepare(&canonical).map(|item|insert(state,item)){
            Some(Ok(InsertOutcome::Added(id)))=>{result.added+=1;result.last_id=Some(id)}
            Some(Ok(InsertOutcome::Existing(id)))=>{result.skipped+=1;result.last_id=Some(id)}
            Some(Ok(InsertOutcome::Duplicate(id)))=>{result.duplicates+=1;result.last_id=Some(id)}
            Some(Err(_))=>result.failed+=1,
            None=>result.skipped+=1,
        }

        progress(ImportProgress{
            job_id:0,processed:index+1,total,added:result.added,skipped:result.skipped,
            duplicates:result.duplicates,failed:result.failed,current_name:name,done:false,cancelled:false,last_id:result.last_id
        });
    }
    Ok(result)
}

fn start_job(app:AppHandle,roots:Vec<String>,recursive_dirs:bool)->Result<u64,String>{
    let state=app.state::<AppState>();
    let job_id=state.next_job_id.fetch_add(1,Ordering::Relaxed);
    let cancel=Arc::new(AtomicBool::new(false));
    state.import_jobs.lock().map_err(|e|e.to_string())?.insert(job_id,cancel.clone());

    let app_for_thread=app.clone();
    std::thread::spawn(move||{
        let state=app_for_thread.state::<AppState>();
        let run=run_import(state.inner(),roots,recursive_dirs,&cancel,|mut event|{
            event.job_id=job_id;
            let _=app_for_thread.emit("imagelore://import-progress",event);
        });
        let cancelled=cancel.load(Ordering::Relaxed);
        let summary=run.unwrap_or(ImportSummary{added:0,skipped:0,duplicates:0,failed:1,last_id:None});
        let _=app_for_thread.emit("imagelore://import-progress",ImportProgress{
            job_id,processed:0,total:0,added:summary.added,skipped:summary.skipped,duplicates:summary.duplicates,
            failed:summary.failed,current_name:String::new(),done:true,cancelled,last_id:summary.last_id
        });
        if let Ok(mut jobs)=state.import_jobs.lock(){jobs.remove(&job_id);}
    });

    Ok(job_id)
}

#[tauri::command]
pub fn start_import_paths(app:AppHandle,paths:Vec<String>)->Result<u64,String>{start_job(app,paths,false)}

#[tauri::command]
pub fn start_import_folder(app:AppHandle,path:String)->Result<u64,String>{start_job(app,vec![path],true)}

#[tauri::command]
pub fn start_import_dropped_paths(app:AppHandle,paths:Vec<String>)->Result<u64,String>{start_job(app,paths,true)}

#[tauri::command]
pub fn cancel_import(state:State<'_,AppState>,job_id:u64)->Result<bool,String>{
    if let Some(flag)=state.import_jobs.lock().map_err(|e|e.to_string())?.get(&job_id){
        flag.store(true,Ordering::Relaxed);
        return Ok(true)
    }
    Ok(false)
}

// Kept for single-image derivative imports and API compatibility.
fn immediate(state:&AppState,roots:Vec<String>,recursive_dirs:bool)->Result<ImportSummary,String>{
    let cancel=AtomicBool::new(false);
    run_import(state,roots,recursive_dirs,&cancel,|_|{})
}

#[tauri::command]
pub fn import_paths(state:State<'_,AppState>,paths:Vec<String>)->Result<ImportSummary,String>{immediate(state.inner(),paths,false)}

#[tauri::command]
pub fn import_folder(state:State<'_,AppState>,path:String)->Result<ImportSummary,String>{immediate(state.inner(),vec![path],true)}

#[tauri::command]
pub fn import_dropped_paths(state:State<'_,AppState>,paths:Vec<String>)->Result<ImportSummary,String>{immediate(state.inner(),paths,true)}
