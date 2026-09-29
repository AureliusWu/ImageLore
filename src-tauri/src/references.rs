use crate::{db,models::{ReferenceSource,SourceFolder},sources,state::AppState};
use rusqlite::{params,Connection,OptionalExtension};
use serde_json::Value;
use std::{fs,process::Command};
use tauri::State;

#[derive(Debug,Clone)]
pub struct ReferenceInput{
    pub source_url:String,
    pub page_url:String,
    pub page_title:String,
    pub source_type:String,
    pub metadata_json:String,
    pub captured_at:i64,
}

fn http_url(value:&str)->String{
    let clean=value.trim();
    if clean.starts_with("https://")||clean.starts_with("http://"){clean.chars().take(2048).collect()}else{String::new()}
}
fn title(value:&str)->String{value.trim().chars().take(512).collect()}
fn metadata(value:&str)->String{
    let parsed=serde_json::from_str::<Value>(value).unwrap_or(Value::Object(Default::default()));
    let text=serde_json::to_string(&parsed).unwrap_or_else(|_|"{}".into());
    text.chars().take(16384).collect()
}
fn row(row:&rusqlite::Row<'_>)->rusqlite::Result<ReferenceSource>{
    Ok(ReferenceSource{
        id:row.get(0)?,asset_id:row.get(1)?,source_url:row.get(2)?,page_url:row.get(3)?,
        page_title:row.get(4)?,source_type:row.get(5)?,metadata_json:row.get(6)?,
        captured_at:row.get(7)?,created_at:row.get(8)?,updated_at:row.get(9)?,
    })
}
pub(crate) fn list(conn:&Connection,asset_id:i64)->Result<Vec<ReferenceSource>,String>{
    let mut st=conn.prepare(
        "SELECT id,asset_id,source_url,page_url,page_title,source_type,metadata_json,captured_at,created_at,updated_at FROM reference_sources WHERE asset_id=?1 ORDER BY captured_at DESC,id DESC"
    ).map_err(|e|e.to_string())?;
    let rows=st.query_map(params![asset_id],row).map_err(|e|e.to_string())?;
    Ok(rows.filter_map(Result::ok).collect())
}
pub(crate) fn search_text(conn:&Connection,asset_id:i64)->Result<String,String>{
    let mut st=conn.prepare("SELECT page_title,page_url,source_url FROM reference_sources WHERE asset_id=?1 ORDER BY id").map_err(|e|e.to_string())?;
    let rows=st.query_map(params![asset_id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?))).map_err(|e|e.to_string())?;
    Ok(rows.filter_map(Result::ok).flat_map(|(a,b,c)|[a,b,c]).filter(|x|!x.is_empty()).collect::<Vec<_>>().join(" "))
}
pub(crate) fn merge_imported(conn:&Connection,asset_id:i64,items:&[ReferenceInput])->Result<(),String>{
    if items.is_empty(){return Ok(())}
    let stamp=db::now();
    for item in items{
        let source_url=http_url(&item.source_url);
        let page_url=http_url(&item.page_url);
        if source_url.is_empty()&&page_url.is_empty(){continue}
        let page_title=title(&item.page_title);
        let source_type=if item.source_type.trim().is_empty(){"web".to_string()}else{item.source_type.trim().chars().take(64).collect()};
        let metadata_json=metadata(&item.metadata_json);
        let captured_at=if item.captured_at>0{item.captured_at}else{stamp};
        conn.execute(
            "INSERT INTO reference_sources(asset_id,source_url,page_url,page_title,source_type,metadata_json,captured_at,created_at,updated_at)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?8)
             ON CONFLICT(asset_id,source_url,page_url) DO UPDATE SET
               page_title=CASE WHEN excluded.page_title<>'' THEN excluded.page_title ELSE page_title END,
               source_type=excluded.source_type,metadata_json=excluded.metadata_json,
               captured_at=MAX(captured_at,excluded.captured_at),updated_at=excluded.updated_at",
            params![asset_id,source_url,page_url,page_title,source_type,metadata_json,captured_at,stamp]
        ).map_err(|e|e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn reference_sources(state:State<'_,AppState>,asset_id:i64)->Result<Vec<ReferenceSource>,String>{
    let conn=state.db.lock().map_err(|e|e.to_string())?;
    list(&conn,asset_id)
}

#[tauri::command]
pub fn ensure_reference_inbox(state:State<'_,AppState>)->Result<SourceFolder,String>{
    let root=dirs::download_dir().ok_or("无法定位系统下载目录")?.join("ImageLore Inbox");
    fs::create_dir_all(&root).map_err(|e|e.to_string())?;
    let canonical=root.canonicalize().unwrap_or(root);
    let path=canonical.to_string_lossy().to_string();
    let conn=state.db.lock().map_err(|e|e.to_string())?;
    let exists:Option<i64>=conn.query_row("SELECT id FROM source_folders WHERE path=?1 COLLATE NOCASE",params![path],|r|r.get(0)).optional().map_err(|e|e.to_string())?;
    if exists.is_none(){
        let stamp=db::now();
        conn.execute("INSERT INTO source_folders(path,name,auto_sync,last_scan_at,created_at,updated_at) VALUES(?1,'ImageLore Inbox',1,0,?2,?2)",params![path,stamp]).map_err(|e|e.to_string())?;
    }
    sources::list_from_conn(&conn)?.into_iter().find(|x|x.path.eq_ignore_ascii_case(&path)).ok_or("无法创建 ImageLore Inbox".into())
}

#[tauri::command]
pub fn open_reference_url(url:String)->Result<bool,String>{
    let url=http_url(&url);
    if url.is_empty(){return Err("只允许打开 http / https 来源链接".into())}
    #[cfg(target_os="windows")]
    {Command::new("rundll32.exe").arg("url.dll,FileProtocolHandler").arg(&url).spawn().map_err(|e|e.to_string())?;}
    #[cfg(target_os="macos")]
    {Command::new("open").arg(&url).spawn().map_err(|e|e.to_string())?;}
    #[cfg(all(unix,not(target_os="macos")))]
    {Command::new("xdg-open").arg(&url).spawn().map_err(|e|e.to_string())?;}
    Ok(true)
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn duplicate_asset_can_merge_multiple_web_sources(){
        let conn=Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("../schema.sql")).unwrap();
        conn.execute("INSERT INTO assets(id,path,name,created_at,updated_at) VALUES(1,'a.png','a.png',1,1)",[]).unwrap();
        conn.execute("INSERT INTO prompt_state(asset_id,updated_at) VALUES(1,1)",[]).unwrap();
        merge_imported(&conn,1,&[
            ReferenceInput{source_url:"https://img.example/a.png".into(),page_url:"https://one.example/post".into(),page_title:"One".into(),source_type:"browser-extension".into(),metadata_json:"{}".into(),captured_at:10},
            ReferenceInput{source_url:"https://img.example/a.png".into(),page_url:"https://two.example/post".into(),page_title:"Two".into(),source_type:"browser-extension".into(),metadata_json:"{}".into(),captured_at:20},
        ]).unwrap();
        let rows=list(&conn,1).unwrap();
        assert_eq!(rows.len(),2);
        assert_eq!(rows[0].page_title,"Two");
    }
    #[test]
    fn unsafe_reference_urls_are_ignored(){
        let conn=Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("../schema.sql")).unwrap();
        conn.execute("INSERT INTO assets(id,path,name,created_at,updated_at) VALUES(1,'a.png','a.png',1,1)",[]).unwrap();
        conn.execute("INSERT INTO prompt_state(asset_id,updated_at) VALUES(1,1)",[]).unwrap();
        merge_imported(&conn,1,&[ReferenceInput{source_url:"file:///secret".into(),page_url:"javascript:alert(1)".into(),page_title:"Bad".into(),source_type:"web".into(),metadata_json:"{}".into(),captured_at:1}]).unwrap();
        assert!(list(&conn,1).unwrap().is_empty());
    }
}
