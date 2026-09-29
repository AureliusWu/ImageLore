use crate::{
    db,
    models::{RemixDraft,RemixSource,RemixSourceInput},
    state::AppState,
    visual_dna,
};
use rusqlite::{params,Connection,OptionalExtension};
use std::collections::HashSet;
use tauri::State;

const DNA_FIELDS:[&str;12]=[
    "subject","character","outfit","pose","expression","composition",
    "camera","lighting","environment","palette","material","style"
];

fn normalize_fields(values:&[String])->Vec<String>{
    let mut seen=HashSet::new();
    values.iter()
        .map(|x|x.trim())
        .filter(|x|DNA_FIELDS.contains(x))
        .filter(|x|seen.insert((*x).to_string()))
        .map(str::to_string)
        .collect()
}

fn normalize_url(value:&str)->String{
    let clean=value.trim();
    if clean.starts_with("https://")||clean.starts_with("http://"){
        clean.chars().take(2048).collect()
    }else{String::new()}
}

fn sources(conn:&Connection,draft_id:i64)->Result<Vec<RemixSource>,String>{
    let mut st=conn.prepare(
        "SELECT rs.asset_id,a.name,rs.fields_json,rs.source_url
         FROM remix_sources rs JOIN assets a ON a.id=rs.asset_id
         WHERE rs.draft_id=?1 ORDER BY rs.position,rs.asset_id"
    ).map_err(|e|e.to_string())?;
    let rows=st.query_map(params![draft_id],|r|{
        let asset_id:i64=r.get(0)?;
        let asset_name:String=r.get(1)?;
        let fields_json:String=r.get(2)?;
        let source_url:String=r.get(3)?;
        Ok((asset_id,asset_name,fields_json,source_url))
    }).map_err(|e|e.to_string())?;
    let mut out=Vec::new();
    for row in rows{
        let(asset_id,asset_name,fields_json,source_url)=row.map_err(|e|e.to_string())?;
        out.push(RemixSource{
            asset_id,
            asset_name,
            fields:normalize_fields(&serde_json::from_str::<Vec<String>>(&fields_json).unwrap_or_default()),
            source_url,
            visual_dna:visual_dna::get(conn,asset_id)?,
        });
    }
    Ok(out)
}

fn get(conn:&Connection,id:i64)->Result<RemixDraft,String>{
    let(base_asset_id,prompt,created_at,updated_at):(i64,String,i64,i64)=conn.query_row(
        "SELECT base_asset_id,prompt,created_at,updated_at FROM remix_drafts WHERE id=?1",
        params![id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))
    ).map_err(|e|e.to_string())?;
    Ok(RemixDraft{id,base_asset_id,prompt,created_at,updated_at,sources:sources(conn,id)?})
}

fn latest(conn:&Connection,base_asset_id:i64)->Result<Option<RemixDraft>,String>{
    let id:Option<i64>=conn.query_row(
        "SELECT id FROM remix_drafts WHERE base_asset_id=?1 ORDER BY updated_at DESC,id DESC LIMIT 1",
        params![base_asset_id],|r|r.get(0)
    ).optional().map_err(|e|e.to_string())?;
    id.map(|id|get(conn,id)).transpose()
}

fn save(conn:&mut Connection,id:Option<i64>,base_asset_id:i64,prompt:String,source_inputs:Vec<RemixSourceInput>)->Result<RemixDraft,String>{
    let _=db::get_asset(conn,base_asset_id)?;
    let prompt:String=prompt.trim().chars().take(16000).collect();
    if source_inputs.is_empty(){return Err("Remix 至少需要一个参考来源".into())}
    let mut unique=HashSet::new();
    let normalized=source_inputs.into_iter().filter_map(|input|{
        if !unique.insert(input.asset_id){return None}
        Some(RemixSourceInput{
            asset_id:input.asset_id,
            fields:normalize_fields(&input.fields),
            source_url:normalize_url(&input.source_url),
        })
    }).collect::<Vec<_>>();
    if !normalized.iter().any(|x|x.asset_id==base_asset_id){
        return Err("Remix 来源必须包含当前基础图片".into())
    }
    for item in &normalized{let _=db::get_asset(conn,item.asset_id)?;}

    let stamp=db::now();
    let tx=conn.transaction().map_err(|e|e.to_string())?;
    let draft_id=if let Some(id)=id{
        let owner:Option<i64>=tx.query_row("SELECT base_asset_id FROM remix_drafts WHERE id=?1",params![id],|r|r.get(0)).optional().map_err(|e|e.to_string())?;
        if owner!=Some(base_asset_id){return Err("Remix 草稿与当前基础图片不匹配".into())}
        tx.execute("UPDATE remix_drafts SET prompt=?1,updated_at=?2 WHERE id=?3",params![prompt,stamp,id]).map_err(|e|e.to_string())?;
        id
    }else{
        tx.execute("INSERT INTO remix_drafts(base_asset_id,prompt,created_at,updated_at) VALUES(?1,?2,?3,?3)",params![base_asset_id,prompt,stamp]).map_err(|e|e.to_string())?;
        tx.last_insert_rowid()
    };
    tx.execute("DELETE FROM remix_sources WHERE draft_id=?1",params![draft_id]).map_err(|e|e.to_string())?;
    for(position,item)in normalized.iter().enumerate(){
        tx.execute(
            "INSERT INTO remix_sources(draft_id,asset_id,fields_json,source_url,reference_meta_json,position,created_at)
             VALUES(?1,?2,?3,?4,'{}',?5,?6)",
            params![draft_id,item.asset_id,serde_json::to_string(&item.fields).map_err(|e|e.to_string())?,item.source_url,position as i64,stamp]
        ).map_err(|e|e.to_string())?;
    }
    tx.commit().map_err(|e|e.to_string())?;
    get(conn,draft_id)
}

fn field_labels(fields:&[String])->String{
    fields.iter().map(|x|match x.as_str(){
        "subject"=>"主体","character"=>"角色","outfit"=>"服装","pose"=>"姿势","expression"=>"表情",
        "composition"=>"构图","camera"=>"镜头","lighting"=>"光线","environment"=>"环境","palette"=>"色彩",
        "material"=>"材质","style"=>"风格",_=>x.as_str()
    }).collect::<Vec<_>>().join("、")
}

fn apply_lineage(conn:&mut Connection,draft_id:i64,child_id:i64)->Result<bool,String>{
    let draft=get(conn,draft_id)?;
    let child=db::get_asset(conn,child_id)?;
    if draft.sources.iter().any(|x|x.asset_id==child_id){return Err("Remix 结果不能与参考来源是同一条记录".into())}
    let stamp=db::now();
    let tx=conn.transaction().map_err(|e|e.to_string())?;
    for source in &draft.sources{
        let relation_type=if source.asset_id==draft.base_asset_id{"derived_from"}else{"reference"};
        let fields=field_labels(&source.fields);
        let note=if fields.is_empty(){"Remix 参考".to_string()}else{format!("Remix · {}",fields)};
        tx.execute(
            "INSERT OR IGNORE INTO relations(parent_id,child_id,relation_type,note,created_at) VALUES(?1,?2,?3,?4,?5)",
            params![source.asset_id,child_id,relation_type,note,stamp]
        ).map_err(|e|e.to_string())?;
    }
    if child.prompt.trim().is_empty()&&!draft.prompt.trim().is_empty(){
        tx.execute(
            "INSERT INTO prompt_state(asset_id,prompt,negative_prompt,model,updated_at) VALUES(?1,?2,'','',?3)
             ON CONFLICT(asset_id) DO UPDATE SET prompt=CASE WHEN trim(prompt)='' THEN excluded.prompt ELSE prompt END,updated_at=?3",
            params![child_id,draft.prompt,stamp]
        ).map_err(|e|e.to_string())?;
    }
    tx.execute(
        "INSERT OR IGNORE INTO asset_sessions(asset_id,session_id,note,created_at,updated_at)
         SELECT ?1,session_id,'Remix result',?2,?2 FROM asset_sessions WHERE asset_id=?3",
        params![child_id,stamp,draft.base_asset_id]
    ).map_err(|e|e.to_string())?;
    tx.execute("UPDATE assets SET updated_at=?1 WHERE id=?2",params![stamp,child_id]).map_err(|e|e.to_string())?;
    db::reindex_asset(&tx,child_id)?;
    tx.commit().map_err(|e|e.to_string())?;
    Ok(true)
}

#[tauri::command]
pub fn latest_remix_draft(state:State<'_,AppState>,base_asset_id:i64)->Result<Option<RemixDraft>,String>{
    let conn=state.db.lock().map_err(|e|e.to_string())?;
    latest(&conn,base_asset_id)
}

#[tauri::command]
pub fn save_remix_draft(state:State<'_,AppState>,id:Option<i64>,base_asset_id:i64,prompt:String,sources:Vec<RemixSourceInput>)->Result<RemixDraft,String>{
    let mut conn=state.db.lock().map_err(|e|e.to_string())?;
    save(&mut conn,id,base_asset_id,prompt,sources)
}

#[tauri::command]
pub fn delete_remix_draft(state:State<'_,AppState>,id:i64)->Result<bool,String>{
    let conn=state.db.lock().map_err(|e|e.to_string())?;
    conn.execute("DELETE FROM remix_drafts WHERE id=?1",params![id]).map_err(|e|e.to_string())?;
    Ok(true)
}

#[tauri::command]
pub fn apply_remix_lineage(state:State<'_,AppState>,draft_id:i64,child_id:i64)->Result<bool,String>{
    let mut conn=state.db.lock().map_err(|e|e.to_string())?;
    apply_lineage(&mut conn,draft_id,child_id)
}

#[cfg(test)]
mod tests{
    use super::*;

    fn seeded()->Connection{
        let conn=Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("../schema.sql")).unwrap();
        for(id,name)in [(1,"Base.png"),(2,"Light.png"),(3,"Result.png")]{
            conn.execute("INSERT INTO assets(id,path,name,created_at,updated_at) VALUES(?1,?2,?3,1,1)",params![id,format!("{}.png",id),name]).unwrap();
            conn.execute("INSERT INTO prompt_state(asset_id,prompt,negative_prompt,model,updated_at) VALUES(?1,'','','',1)",params![id]).unwrap();
        }
        visual_dna::upsert(&conn,1,&crate::models::VisualDnaPatch{subject:"蓝发角色".into(),style:"写实摄影".into(),..Default::default()}).unwrap();
        visual_dna::upsert(&conn,2,&crate::models::VisualDnaPatch{lighting:"霓虹侧光".into(),environment:"夜间街道".into(),..Default::default()}).unwrap();
        conn
    }

    #[test]
    fn saves_multi_source_draft_and_filters_unknown_fields(){
        let mut conn=seeded();
        let draft=save(&mut conn,None,1,"remix prompt".into(),vec![
            RemixSourceInput{asset_id:1,fields:vec!["subject".into(),"unknown".into()],source_url:String::new()},
            RemixSourceInput{asset_id:2,fields:vec!["lighting".into()],source_url:"https://example.test/ref".into()},
        ]).unwrap();
        assert_eq!(draft.sources.len(),2);
        assert_eq!(draft.sources[0].fields,vec!["subject"]);
        assert_eq!(draft.sources[1].source_url,"https://example.test/ref");
    }

    #[test]
    fn lineage_records_base_and_reference_fields_without_overwriting_existing_prompt(){
        let mut conn=seeded();
        let draft=save(&mut conn,None,1,"remix prompt".into(),vec![
            RemixSourceInput{asset_id:1,fields:vec!["subject".into()],source_url:String::new()},
            RemixSourceInput{asset_id:2,fields:vec!["lighting".into()],source_url:String::new()},
        ]).unwrap();
        apply_lineage(&mut conn,draft.id,3).unwrap();
        let relations:Vec<(String,String)>={
            let mut st=conn.prepare("SELECT relation_type,note FROM relations WHERE child_id=3 ORDER BY relation_type").unwrap();
            st.query_map([],|r|Ok((r.get(0)?,r.get(1)?))).unwrap().filter_map(Result::ok).collect()
        };
        assert_eq!(relations.len(),2);
        assert!(relations.iter().any(|x|x.0=="derived_from"&&x.1.contains("主体")));
        assert!(relations.iter().any(|x|x.0=="reference"&&x.1.contains("光线")));
        let prompt:String=conn.query_row("SELECT prompt FROM prompt_state WHERE asset_id=3",[],|r|r.get(0)).unwrap();
        assert_eq!(prompt,"remix prompt");

        conn.execute("UPDATE prompt_state SET prompt='metadata prompt' WHERE asset_id=3",[]).unwrap();
        apply_lineage(&mut conn,draft.id,3).unwrap();
        let prompt:String=conn.query_row("SELECT prompt FROM prompt_state WHERE asset_id=3",[],|r|r.get(0)).unwrap();
        assert_eq!(prompt,"metadata prompt");
    }
}
