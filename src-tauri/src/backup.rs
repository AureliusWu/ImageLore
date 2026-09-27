use crate::{models::BackupRecord,state::AppState};
use rusqlite::Connection;
use std::{fs,path::{Path,PathBuf},time::{SystemTime,UNIX_EPOCH}};
use tauri::State;

const MAX_BACKUPS:usize=10;
const AUTO_INTERVAL:i64=24*3600;

fn now()->i64{
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64
}

fn now_ms()->u128{
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis()
}

fn validate(path:&Path)->Result<(),String>{
    let conn=Connection::open(path).map_err(|e|e.to_string())?;
    let result:String=conn.query_row("PRAGMA integrity_check",[],|r|r.get(0)).map_err(|e|e.to_string())?;
    if result!="ok"{return Err(format!("备份完整性检查失败：{}",result))}
    Ok(())
}

fn records(dir:&Path)->Result<Vec<BackupRecord>,String>{
    fs::create_dir_all(dir).map_err(|e|e.to_string())?;
    let mut out=Vec::new();
    for entry in fs::read_dir(dir).map_err(|e|e.to_string())?.filter_map(Result::ok){
        let path=entry.path();
        if path.extension().and_then(|x|x.to_str())!=Some("sqlite3"){continue}
        let meta=entry.metadata().map_err(|e|e.to_string())?;
        let created=meta.modified().ok()
            .and_then(|x|x.duration_since(UNIX_EPOCH).ok())
            .map(|x|x.as_secs() as i64).unwrap_or(0);
        out.push(BackupRecord{
            name:path.file_name().and_then(|x|x.to_str()).unwrap_or("backup.sqlite3").to_string(),
            path:path.to_string_lossy().to_string(),
            size:meta.len(),
            created_at:created,
        });
    }
    out.sort_by(|a,b|b.created_at.cmp(&a.created_at).then_with(||b.name.cmp(&a.name)));
    Ok(out)
}

fn rotate(dir:&Path)->Result<(),String>{
    let all=records(dir)?;
    for item in all.into_iter().skip(MAX_BACKUPS){let _=fs::remove_file(item.path);}
    Ok(())
}

fn unique_path(dir:&Path,prefix:&str)->PathBuf{
    dir.join(format!("{}-{}.sqlite3",prefix,now_ms()))
}

fn vacuum_snapshot(source:&Path,target:&Path)->Result<(),String>{
    if target.exists(){fs::remove_file(target).map_err(|e|e.to_string())?;}
    let conn=Connection::open(source).map_err(|e|e.to_string())?;
    conn.execute_batch("PRAGMA wal_checkpoint(FULL);").map_err(|e|e.to_string())?;
    let escaped=target.to_string_lossy().replace(''',"''");
    conn.execute_batch(&format!("VACUUM INTO '{}';",escaped)).map_err(|e|e.to_string())?;
    drop(conn);
    validate(target)
}

fn create(state:&AppState,prefix:&str)->Result<BackupRecord,String>{
    fs::create_dir_all(&state.backups_dir).map_err(|e|e.to_string())?;
    let path=unique_path(&state.backups_dir,prefix);
    {
        let conn=state.db.lock().map_err(|e|e.to_string())?;
        conn.execute_batch("PRAGMA wal_checkpoint(FULL);").map_err(|e|e.to_string())?;
        let escaped=path.to_string_lossy().replace(''',"''");
        conn.execute_batch(&format!("VACUUM INTO '{}';",escaped)).map_err(|e|e.to_string())?;
    }
    validate(&path)?;
    rotate(&state.backups_dir)?;
    records(&state.backups_dir)?.into_iter().find(|x|x.path==path.to_string_lossy()).ok_or("无法读取刚创建的备份".into())
}

pub fn apply_pending_restore(database_path:&Path,data_dir:&Path,backups_dir:&Path)->Result<(),String>{
    let pending=data_dir.join("restore.pending.sqlite3");
    if !pending.exists(){return Ok(())}
    validate(&pending)?;
    fs::create_dir_all(backups_dir).map_err(|e|e.to_string())?;

    let safety=if database_path.exists(){
        let path=unique_path(backups_dir,"pre-restore");
        vacuum_snapshot(database_path,&path)?;
        Some(path)
    }else{None};

    let staged=data_dir.join("restore.next.sqlite3");
    if staged.exists(){fs::remove_file(&staged).map_err(|e|e.to_string())?;}
    fs::copy(&pending,&staged).map_err(|e|e.to_string())?;
    validate(&staged)?;

    let wal=database_path.with_extension("sqlite3-wal");
    let shm=database_path.with_extension("sqlite3-shm");
    let _=fs::remove_file(&wal);
    let _=fs::remove_file(&shm);
    if database_path.exists(){fs::remove_file(database_path).map_err(|e|e.to_string())?;}

    if let Err(error)=fs::rename(&staged,database_path){
        if let Some(safety)=safety.as_ref(){let _=fs::copy(safety,database_path);}
        return Err(format!("应用恢复备份失败：{}",error))
    }

    validate(database_path)?;
    fs::remove_file(&pending).map_err(|e|e.to_string())?;
    rotate(backups_dir)?;
    Ok(())
}

#[tauri::command]
pub fn create_backup(state:State<'_,AppState>)->Result<BackupRecord,String>{create(state.inner(),"imagelore")}

#[tauri::command]
pub fn ensure_auto_backup(state:State<'_,AppState>)->Result<Option<BackupRecord>,String>{
    let latest=records(&state.backups_dir)?.into_iter().next();
    if latest.as_ref().map(|x|now()-x.created_at<AUTO_INTERVAL).unwrap_or(false){return Ok(None)}
    Ok(Some(create(state.inner(),"auto")?))
}

#[tauri::command]
pub fn list_backups(state:State<'_,AppState>)->Result<Vec<BackupRecord>,String>{records(&state.backups_dir)}

#[tauri::command]
pub fn stage_restore(state:State<'_,AppState>,name:String)->Result<bool,String>{
    let file=PathBuf::from(&name);
    let base=file.file_name().and_then(|x|x.to_str()).ok_or("无效备份名称")?;
    let source=state.backups_dir.join(base);
    if !source.exists(){return Err("找不到该备份".into())}
    validate(&source)?;
    let pending=state.data_dir.join("restore.pending.sqlite3");
    fs::copy(source,pending).map_err(|e|e.to_string())?;
    validate(&pending)?;
    Ok(true)
}
