use crate::{db,metadata,models::ImportSummary,sidecar,state::AppState};
use rusqlite::params;
use std::path::{Path,PathBuf};
use tauri::State;
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

fn insert(state:&AppState,item:PreparedAsset)->Result<(i64,bool),String>{
    let mut conn=state.db.lock().map_err(|e|e.to_string())?;
    if let Some(id)=db::asset_exists_by_path(&conn,&item.path)?{
        conn.execute("UPDATE assets SET missing=0,updated_at=?1 WHERE id=?2",params![db::now(),id]).map_err(|e|e.to_string())?;
        return Ok((id,false))
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
    Ok((id,true))
}

fn import(state:&AppState,roots:Vec<String>,recursive_dirs:bool)->Result<ImportSummary,String>{
    let mut result=ImportSummary{added:0,skipped:0,failed:0,last_id:None};
    for path in supported_files(roots,recursive_dirs){
        let canonical=path.canonicalize().unwrap_or_else(|_|path.clone());
        let path_text=canonical.to_string_lossy().to_string();

        // Fast path: avoid metadata parsing and hashing for an already-known path.
        {
            let conn=state.db.lock().map_err(|e|e.to_string())?;
            if let Some(id)=db::asset_exists_by_path(&conn,&path_text)?{
                conn.execute("UPDATE assets SET missing=0 WHERE id=?1",params![id]).map_err(|e|e.to_string())?;
                result.skipped+=1;result.last_id=Some(id);continue;
            }
        }

        let Some(item)=prepare(&canonical)else{result.skipped+=1;continue};
        match insert(state,item){
            Ok((id,true))=>{result.added+=1;result.last_id=Some(id)}
            Ok((id,false))=>{result.skipped+=1;result.last_id=Some(id)}
            Err(_)=>result.failed+=1,
        }
    }
    Ok(result)
}

#[tauri::command]
pub fn import_paths(state:State<'_,AppState>,paths:Vec<String>)->Result<ImportSummary,String>{
    import(state.inner(),paths,false)
}

#[tauri::command]
pub fn import_folder(state:State<'_,AppState>,path:String)->Result<ImportSummary,String>{
    import(state.inner(),vec![path],true)
}

#[tauri::command]
pub fn import_dropped_paths(state:State<'_,AppState>,paths:Vec<String>)->Result<ImportSummary,String>{
    import(state.inner(),paths,true)
}
