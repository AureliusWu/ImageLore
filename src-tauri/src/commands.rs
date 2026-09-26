use crate::{db, metadata, models::*, preview, state::AppState};
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::{collections::{HashMap, HashSet}, fs, path::{Path, PathBuf}, process::Command};
use tauri::State;
use walkdir::WalkDir;

fn sidecar_path(path: &Path) -> PathBuf {
    PathBuf::from(format!("{}.imagelore.json", path.to_string_lossy()))
}

fn read_sidecar(path: &Path) -> Option<Value> {
    let text = fs::read_to_string(sidecar_path(path)).ok()?;
    let value = serde_json::from_str::<Value>(&text).ok()?;
    if value.get("schema").and_then(Value::as_str) == Some("imagelore.sidecar.v2") { Some(value) } else { None }
}

fn sidecar_text(value: &Value, key: &str) -> String {
    value.get(key).and_then(Value::as_str).unwrap_or("").to_string()
}

fn sidecar_tags(value: &Value) -> Vec<String> {
    value.get("tags").and_then(Value::as_array).map(|items| {
        items.iter().filter_map(Value::as_str).map(str::to_string).collect()
    }).unwrap_or_default()
}

fn add_one(conn: &mut Connection, path: &Path) -> Result<Option<i64>, String> {
    if !path.is_file() || !metadata::is_supported(path) { return Ok(None); }
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let path_text = canonical.to_string_lossy().to_string();
    if let Some(id) = db::asset_exists_by_path(conn, &path_text)? {
        conn.execute("UPDATE assets SET missing=0,updated_at=?1 WHERE id=?2", params![db::now(), id]).map_err(|e| e.to_string())?;
        return Ok(Some(id));
    }

    let info = metadata::file_info(&canonical);
    let extract = metadata::extract_generation(&canonical);
    let sidecar = read_sidecar(&canonical);
    let prompt = sidecar.as_ref().map(|x| sidecar_text(x,"prompt")).filter(|x| !x.is_empty()).unwrap_or(extract.prompt);
    let negative = sidecar.as_ref().map(|x| sidecar_text(x,"negative_prompt")).filter(|x| !x.is_empty()).unwrap_or(extract.negative_prompt);
    let model = sidecar.as_ref().map(|x| sidecar_text(x,"model")).filter(|x| !x.is_empty()).unwrap_or(extract.model);
    let tags = sidecar.as_ref().map(sidecar_tags).unwrap_or_default();
    let fingerprint = metadata::fingerprint(&canonical);
    let timestamp = db::now();
    let name = canonical.file_name().and_then(|x| x.to_str()).unwrap_or("image").to_string();

    conn.execute(
        "INSERT INTO assets(path,name,favorite,width,height,file_size,format,mime_type,metadata_type,generation_json,fingerprint,file_mtime,missing,created_at,updated_at) VALUES(?1,?2,0,?3,?4,?5,?6,?7,?8,?9,?10,?11,0,?12,?12)",
        params![path_text,name,info.width,info.height,info.file_size,info.format,info.mime_type,extract.metadata_type,extract.generation_json,fingerprint,info.file_mtime,timestamp]
    ).map_err(|e| e.to_string())?;
    let id = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO prompt_state(asset_id,prompt,negative_prompt,model,updated_at) VALUES(?1,?2,?3,?4,?5)",
        params![id,prompt,negative,model,timestamp]
    ).map_err(|e| e.to_string())?;
    db::set_tags(conn,id,&tags)?;
    Ok(Some(id))
}

#[tauri::command]
pub fn library_page(state: State<'_,AppState>, filter: LibraryFilter, offset:i64, limit:i64) -> Result<LibraryPage,String> {
    let conn = state.db.lock().map_err(|e|e.to_string())?;
    db::library_page(&conn,&filter,offset,limit)
}

#[tauri::command]
pub fn library_facets(state: State<'_,AppState>) -> Result<LibraryFacets,String> {
    let conn = state.db.lock().map_err(|e|e.to_string())?;
    db::facets(&conn)
}

#[tauri::command]
pub fn get_asset(state: State<'_,AppState>, id:i64) -> Result<AssetRecord,String> {
    let conn = state.db.lock().map_err(|e|e.to_string())?;
    db::get_asset(&conn,id)
}

#[tauri::command]
pub fn import_paths(state: State<'_,AppState>, paths:Vec<String>) -> Result<ImportSummary,String> {
    let mut conn = state.db.lock().map_err(|e|e.to_string())?;
    let mut result = ImportSummary{added:0,skipped:0,failed:0,last_id:None};
    for raw in paths {
        let existed = db::asset_exists_by_path(&conn,&PathBuf::from(&raw).canonicalize().unwrap_or_else(|_|PathBuf::from(&raw)).to_string_lossy())?.is_some();
        match add_one(&mut conn,Path::new(&raw)) {
            Ok(Some(id)) => { if existed { result.skipped+=1 } else { result.added+=1 }; result.last_id=Some(id); },
            Ok(None) => result.skipped+=1,
            Err(_) => result.failed+=1,
        }
    }
    Ok(result)
}

#[tauri::command]
pub fn import_folder(state: State<'_,AppState>, path:String) -> Result<ImportSummary,String> {
    let mut conn = state.db.lock().map_err(|e|e.to_string())?;
    let mut result = ImportSummary{added:0,skipped:0,failed:0,last_id:None};
    for entry in WalkDir::new(&path).follow_links(false).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() || !metadata::is_supported(entry.path()) { continue; }
        let canonical = entry.path().canonicalize().unwrap_or_else(|_|entry.path().to_path_buf());
        let existed = db::asset_exists_by_path(&conn,&canonical.to_string_lossy())?.is_some();
        match add_one(&mut conn,entry.path()) {
            Ok(Some(id)) => { if existed { result.skipped+=1 } else { result.added+=1 }; result.last_id=Some(id); },
            Ok(None) => result.skipped+=1,
            Err(_) => result.failed+=1,
        }
    }
    Ok(result)
}

#[tauri::command]
pub fn update_prompt(state:State<'_,AppState>, id:i64, patch:PromptPatch) -> Result<AssetRecord,String> {
    let conn = state.db.lock().map_err(|e|e.to_string())?;
    let stamp=db::now();
    conn.execute("INSERT INTO prompt_state(asset_id,prompt,negative_prompt,model,updated_at) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(asset_id) DO UPDATE SET prompt=excluded.prompt,negative_prompt=excluded.negative_prompt,model=excluded.model,updated_at=excluded.updated_at", params![id,patch.prompt,patch.negative_prompt,patch.model,stamp]).map_err(|e|e.to_string())?;
    conn.execute("UPDATE assets SET updated_at=?1 WHERE id=?2",params![stamp,id]).map_err(|e|e.to_string())?;
    db::reindex_asset(&conn,id)?;
    db::get_asset(&conn,id)
}

#[tauri::command]
pub fn replace_tags(state:State<'_,AppState>, id:i64, tags:Vec<String>) -> Result<AssetRecord,String> {
    let mut conn = state.db.lock().map_err(|e|e.to_string())?;
    db::set_tags(&mut conn,id,&tags)?;
    db::get_asset(&conn,id)
}

#[tauri::command]
pub fn batch_add_tags(state:State<'_,AppState>, ids:Vec<i64>, tags:Vec<String>) -> Result<bool,String> {
    let mut conn = state.db.lock().map_err(|e|e.to_string())?;
    db::add_tags(&mut conn,&ids,&tags)?;
    Ok(true)
}

#[tauri::command]
pub fn toggle_favorite(state:State<'_,AppState>, id:i64) -> Result<AssetRecord,String> {
    let conn = state.db.lock().map_err(|e|e.to_string())?;
    conn.execute("UPDATE assets SET favorite=CASE favorite WHEN 0 THEN 1 ELSE 0 END,updated_at=?1 WHERE id=?2",params![db::now(),id]).map_err(|e|e.to_string())?;
    db::get_asset(&conn,id)
}

#[tauri::command]
pub fn delete_asset(state:State<'_,AppState>, id:i64) -> Result<bool,String> {
    let conn = state.db.lock().map_err(|e|e.to_string())?;
    let fingerprint:String = conn.query_row("SELECT fingerprint FROM assets WHERE id=?1",params![id],|r|r.get(0)).unwrap_or_default();
    conn.execute("DELETE FROM assets WHERE id=?1",params![id]).map_err(|e|e.to_string())?;
    conn.execute("DELETE FROM asset_search WHERE asset_id=?1",params![id]).map_err(|e|e.to_string())?;
    preview::purge_asset_cache(&state.cache_dir,&fingerprint);
    Ok(true)
}

#[tauri::command]
pub fn add_revision(state:State<'_,AppState>, id:i64, note:String) -> Result<bool,String> {
    let conn=state.db.lock().map_err(|e|e.to_string())?;
    let asset=db::get_asset(&conn,id)?;
    let tags_json=serde_json::to_string(&asset.tags).map_err(|e|e.to_string())?;
    conn.execute("INSERT INTO prompt_revisions(asset_id,prompt,negative_prompt,model,tags_json,note,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![id,asset.prompt,asset.negative_prompt,asset.model,tags_json,note,db::now()]).map_err(|e|e.to_string())?;
    Ok(true)
}

#[tauri::command]
pub fn list_revisions(state:State<'_,AppState>, id:i64) -> Result<Vec<Revision>,String> {
    let conn=state.db.lock().map_err(|e|e.to_string())?;
    let mut st=conn.prepare("SELECT id,asset_id,prompt,negative_prompt,model,tags_json,note,created_at FROM prompt_revisions WHERE asset_id=?1 ORDER BY created_at DESC,id DESC").map_err(|e|e.to_string())?;
    let rows=st.query_map(params![id],|r|{
        let tags_json:String=r.get(5)?;
        Ok(Revision{id:r.get(0)?,asset_id:r.get(1)?,prompt:r.get(2)?,negative_prompt:r.get(3)?,model:r.get(4)?,tags:serde_json::from_str(&tags_json).unwrap_or_default(),note:r.get(6)?,created_at:r.get(7)?})
    }).map_err(|e|e.to_string())?;
    Ok(rows.filter_map(Result::ok).collect())
}

#[tauri::command]
pub fn restore_revision(state:State<'_,AppState>, revision_id:i64) -> Result<AssetRecord,String> {
    let mut conn=state.db.lock().map_err(|e|e.to_string())?;
    let rev:Revision=conn.query_row("SELECT id,asset_id,prompt,negative_prompt,model,tags_json,note,created_at FROM prompt_revisions WHERE id=?1",params![revision_id],|r|{
        let tags_json:String=r.get(5)?;
        Ok(Revision{id:r.get(0)?,asset_id:r.get(1)?,prompt:r.get(2)?,negative_prompt:r.get(3)?,model:r.get(4)?,tags:serde_json::from_str(&tags_json).unwrap_or_default(),note:r.get(6)?,created_at:r.get(7)?})
    }).map_err(|e|e.to_string())?;
    let current=db::get_asset(&conn,rev.asset_id)?;
    let current_tags=serde_json::to_string(&current.tags).map_err(|e|e.to_string())?;
    conn.execute("INSERT INTO prompt_revisions(asset_id,prompt,negative_prompt,model,tags_json,note,created_at) VALUES(?1,?2,?3,?4,?5,'Before restore',?6)",params![current.id,current.prompt,current.negative_prompt,current.model,current_tags,db::now()]).map_err(|e|e.to_string())?;
    conn.execute("UPDATE prompt_state SET prompt=?1,negative_prompt=?2,model=?3,updated_at=?4 WHERE asset_id=?5",params![rev.prompt,rev.negative_prompt,rev.model,db::now(),rev.asset_id]).map_err(|e|e.to_string())?;
    db::set_tags(&mut conn,rev.asset_id,&rev.tags)?;
    db::reindex_asset(&conn,rev.asset_id)?;
    db::get_asset(&conn,rev.asset_id)
}

fn path_exists(conn:&Connection,start:i64,target:i64)->bool {
    let mut stack=vec![start]; let mut seen=HashSet::new();
    while let Some(id)=stack.pop() {
        if id==target { return true; }
        if !seen.insert(id) { continue; }
        if let Ok(mut st)=conn.prepare("SELECT child_id FROM relations WHERE parent_id=?1") {
            if let Ok(rows)=st.query_map(params![id],|r|r.get::<_,i64>(0)) { for child in rows.filter_map(Result::ok){stack.push(child)} }
        }
    }
    false
}

#[tauri::command]
pub fn add_relation(state:State<'_,AppState>, parent_id:i64, child_id:i64, relation_type:String, note:String)->Result<bool,String>{
    if parent_id==child_id{return Err("不能把记录关联到自己".into())}
    let conn=state.db.lock().map_err(|e|e.to_string())?;
    if path_exists(&conn,child_id,parent_id){return Err("该关系会形成循环谱系".into())}
    conn.execute("INSERT OR IGNORE INTO relations(parent_id,child_id,relation_type,note,created_at) VALUES(?1,?2,?3,?4,?5)",params![parent_id,child_id,relation_type,note,db::now()]).map_err(|e|e.to_string())?;
    Ok(true)
}

fn relations(conn:&Connection,id:i64,parents:bool)->Result<Vec<RelationRecord>,String>{
    let sql=if parents{
        "SELECT r.id,r.parent_id,r.child_id,r.relation_type,r.note,r.created_at,a.id,a.name FROM relations r JOIN assets a ON a.id=r.parent_id WHERE r.child_id=?1 ORDER BY r.created_at DESC"
    }else{
        "SELECT r.id,r.parent_id,r.child_id,r.relation_type,r.note,r.created_at,a.id,a.name FROM relations r JOIN assets a ON a.id=r.child_id WHERE r.parent_id=?1 ORDER BY r.created_at DESC"
    };
    let mut st=conn.prepare(sql).map_err(|e|e.to_string())?;
    let rows=st.query_map(params![id],|r|Ok(RelationRecord{id:r.get(0)?,parent_id:r.get(1)?,child_id:r.get(2)?,relation_type:r.get(3)?,note:r.get(4)?,created_at:r.get(5)?,other_id:r.get(6)?,other_name:r.get(7)?})).map_err(|e|e.to_string())?;
    Ok(rows.filter_map(Result::ok).collect())
}

#[tauri::command]
pub fn lineage(state:State<'_,AppState>,id:i64)->Result<Lineage,String>{
    let conn=state.db.lock().map_err(|e|e.to_string())?;
    Ok(Lineage{parents:relations(&conn,id,true)?,children:relations(&conn,id,false)?})
}

#[tauri::command]
pub fn collections(state:State<'_,AppState>)->Result<Vec<CollectionRecord>,String>{
    let conn=state.db.lock().map_err(|e|e.to_string())?;
    db::collections(&conn)
}

#[tauri::command]
pub fn create_collection(state:State<'_,AppState>,name:String,description:String)->Result<CollectionRecord,String>{
    let conn=state.db.lock().map_err(|e|e.to_string())?;
    let stamp=db::now();
    conn.execute("INSERT INTO collections(name,description,created_at,updated_at) VALUES(?1,?2,?3,?3)",params![name.trim(),description.trim(),stamp]).map_err(|e|e.to_string())?;
    let id=conn.last_insert_rowid();
    Ok(CollectionRecord{id,name:name.trim().to_string(),description:description.trim().to_string(),created_at:stamp,updated_at:stamp,count:0})
}

#[tauri::command]
pub fn add_to_collection(state:State<'_,AppState>,collection_id:i64,asset_ids:Vec<i64>)->Result<bool,String>{
    let conn=state.db.lock().map_err(|e|e.to_string())?;
    for id in asset_ids { conn.execute("INSERT OR IGNORE INTO collection_assets(collection_id,asset_id,created_at) VALUES(?1,?2,?3)",params![collection_id,id,db::now()]).map_err(|e|e.to_string())?; }
    conn.execute("UPDATE collections SET updated_at=?1 WHERE id=?2",params![db::now(),collection_id]).map_err(|e|e.to_string())?;
    Ok(true)
}

#[tauri::command]
pub fn rescan_metadata(state:State<'_,AppState>,id:i64)->Result<AssetRecord,String>{
    let conn=state.db.lock().map_err(|e|e.to_string())?;
    let old=db::get_asset(&conn,id)?; let path=Path::new(&old.path);
    if !path.exists(){conn.execute("UPDATE assets SET missing=1,updated_at=?1 WHERE id=?2",params![db::now(),id]).map_err(|e|e.to_string())?;return db::get_asset(&conn,id)}
    let info=metadata::file_info(path); let fp=metadata::fingerprint(path); let extract=metadata::extract_generation(path);
    conn.execute("UPDATE assets SET width=?1,height=?2,file_size=?3,format=?4,mime_type=?5,metadata_type=?6,generation_json=?7,fingerprint=?8,file_mtime=?9,missing=0,updated_at=?10 WHERE id=?11",params![info.width,info.height,info.file_size,info.format,info.mime_type,extract.metadata_type,extract.generation_json,fp,info.file_mtime,db::now(),id]).map_err(|e|e.to_string())?;
    if old.prompt.is_empty() && old.negative_prompt.is_empty() && old.model.is_empty(){conn.execute("UPDATE prompt_state SET prompt=?1,negative_prompt=?2,model=?3,updated_at=?4 WHERE asset_id=?5",params![extract.prompt,extract.negative_prompt,extract.model,db::now(),id]).map_err(|e|e.to_string())?;}
    preview::purge_asset_cache(&state.cache_dir,&old.fingerprint); db::reindex_asset(&conn,id)?; db::get_asset(&conn,id)
}

#[tauri::command]
pub fn export_sidecar(state:State<'_,AppState>,id:i64)->Result<String,String>{
    let conn=state.db.lock().map_err(|e|e.to_string())?; let asset=db::get_asset(&conn,id)?; let lin=Lineage{parents:relations(&conn,id,true)?,children:relations(&conn,id,false)?};
    let data=json!({"schema":"imagelore.sidecar.v2","image":asset.path,"fingerprint":asset.fingerprint,"prompt":asset.prompt,"negative_prompt":asset.negative_prompt,"model":asset.model,"tags":asset.tags,"metadata_type":asset.metadata_type,"generation":serde_json::from_str::<Value>(&asset.generation_json).unwrap_or(Value::String(asset.generation_json)),"parents":lin.parents});
    let out=sidecar_path(Path::new(&asset.path)); fs::write(&out,serde_json::to_string_pretty(&data).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?; Ok(out.to_string_lossy().to_string())
}

#[tauri::command]
pub fn preview_data_url(state:State<'_,AppState>,id:i64,max_edge:u32,thumbnail:bool)->Result<String,String>{
    let conn=state.db.lock().map_err(|e|e.to_string())?; let asset=db::get_asset(&conn,id)?; drop(conn);
    let path=Path::new(&asset.path); if !path.exists(){return Err("原图片文件不存在".into())}
    let key=if asset.fingerprint.is_empty(){format!("{}-{}",asset.id,asset.file_mtime)}else{asset.fingerprint.clone()};
    preview::preview_data_url(path,&state.cache_dir,&key,max_edge,thumbnail)
}

#[tauri::command]
pub fn refresh_missing(state:State<'_,AppState>)->Result<i64,String>{
    let conn=state.db.lock().map_err(|e|e.to_string())?;
    let rows:Vec<(i64,String,i64)>={let mut st=conn.prepare("SELECT id,path,missing FROM assets").map_err(|e|e.to_string())?;st.query_map([],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(|e|e.to_string())?.filter_map(Result::ok).collect()};
    let mut missing_count=0;
    for(id,path,old)in rows{let missing=if Path::new(&path).exists(){0}else{1};if missing==1{missing_count+=1}if missing!=old{conn.execute("UPDATE assets SET missing=?1 WHERE id=?2",params![missing,id]).map_err(|e|e.to_string())?;}}
    Ok(missing_count)
}

#[tauri::command]
pub fn relocate_missing(state:State<'_,AppState>,root:String)->Result<i64,String>{
    let conn=state.db.lock().map_err(|e|e.to_string())?;
    let wanted:Vec<(i64,String)>={let mut st=conn.prepare("SELECT id,fingerprint FROM assets WHERE missing=1 AND fingerprint<>''").map_err(|e|e.to_string())?;st.query_map([],|r|Ok((r.get(0)?,r.get(1)?))).map_err(|e|e.to_string())?.filter_map(Result::ok).collect()};
    let fingerprints=wanted.iter().map(|x|x.1.clone()).collect::<HashSet<_>>(); let mut found=HashMap::<String,PathBuf>::new();
    for entry in WalkDir::new(root).follow_links(false).into_iter().filter_map(Result::ok){if !entry.file_type().is_file()||!metadata::is_supported(entry.path()){continue}let fp=metadata::fingerprint(entry.path());if fingerprints.contains(&fp){found.insert(fp,entry.path().to_path_buf());if found.len()==fingerprints.len(){break}}}
    let mut fixed=0;
    for(id,fp)in wanted{if let Some(path)=found.get(&fp){let canonical=path.canonicalize().unwrap_or_else(|_|path.clone());let name=canonical.file_name().and_then(|x|x.to_str()).unwrap_or("image");conn.execute("UPDATE assets SET path=?1,name=?2,missing=0,updated_at=?3 WHERE id=?4",params![canonical.to_string_lossy(),name,db::now(),id]).map_err(|e|e.to_string())?;db::reindex_asset(&conn,id)?;fixed+=1}}
    Ok(fixed)
}

fn open_path(path:&Path,select:bool)->Result<(),String>{
    #[cfg(target_os="windows")]{let mut c=Command::new("explorer");if select{c.arg("/select,").arg(path);}else{c.arg(path);}c.spawn().map_err(|e|e.to_string())?;}
    #[cfg(target_os="macos")]{let mut c=Command::new("open");if select{c.arg("-R");}c.arg(path).spawn().map_err(|e|e.to_string())?;}
    #[cfg(all(unix,not(target_os="macos")))]{let p=if select{path.parent().unwrap_or(path)}else{path};Command::new("xdg-open").arg(p).spawn().map_err(|e|e.to_string())?;}
    Ok(())
}

#[tauri::command]
pub fn open_external(state:State<'_,AppState>,id:i64)->Result<bool,String>{let conn=state.db.lock().map_err(|e|e.to_string())?;let asset=db::get_asset(&conn,id)?;open_path(Path::new(&asset.path),false)?;Ok(true)}
#[tauri::command]
pub fn open_containing_folder(state:State<'_,AppState>,id:i64)->Result<bool,String>{let conn=state.db.lock().map_err(|e|e.to_string())?;let asset=db::get_asset(&conn,id)?;open_path(Path::new(&asset.path),true)?;Ok(true)}
