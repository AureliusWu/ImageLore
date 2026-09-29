use crate::{db,models::{VisualDna,VisualDnaPatch},state::AppState};
use rusqlite::{params,Connection,OptionalExtension};
use tauri::State;

fn normalize(value:&str)->String{
    value.trim().chars().take(1200).collect()
}

pub fn normalize_patch(mut value:VisualDnaPatch)->VisualDnaPatch{
    value.subject=normalize(&value.subject);
    value.character=normalize(&value.character);
    value.outfit=normalize(&value.outfit);
    value.pose=normalize(&value.pose);
    value.expression=normalize(&value.expression);
    value.composition=normalize(&value.composition);
    value.camera=normalize(&value.camera);
    value.lighting=normalize(&value.lighting);
    value.environment=normalize(&value.environment);
    value.palette=normalize(&value.palette);
    value.material=normalize(&value.material);
    value.style=normalize(&value.style);
    value.source=match value.source.trim(){
        "ai"=>"ai".into(),
        "sidecar"=>"sidecar".into(),
        _=>"manual".into(),
    };
    value
}

pub fn patch_is_empty(value:&VisualDnaPatch)->bool{
    [
        &value.subject,&value.character,&value.outfit,&value.pose,&value.expression,&value.composition,
        &value.camera,&value.lighting,&value.environment,&value.palette,&value.material,&value.style
    ].iter().all(|x|x.trim().is_empty())
}

pub fn record_is_empty(value:&VisualDna)->bool{
    [
        &value.subject,&value.character,&value.outfit,&value.pose,&value.expression,&value.composition,
        &value.camera,&value.lighting,&value.environment,&value.palette,&value.material,&value.style
    ].iter().all(|x|x.trim().is_empty())
}

fn row(r:&rusqlite::Row<'_>)->rusqlite::Result<VisualDna>{
    Ok(VisualDna{
        subject:r.get(0)?,character:r.get(1)?,outfit:r.get(2)?,pose:r.get(3)?,expression:r.get(4)?,
        composition:r.get(5)?,camera:r.get(6)?,lighting:r.get(7)?,environment:r.get(8)?,palette:r.get(9)?,
        material:r.get(10)?,style:r.get(11)?,source:r.get(12)?,updated_at:r.get(13)?,
    })
}

pub fn get(conn:&Connection,asset_id:i64)->Result<VisualDna,String>{
    conn.query_row(
        "SELECT subject,character_name,outfit,pose,expression,composition,camera,lighting,environment,palette,material,style,source,updated_at
         FROM visual_dna WHERE asset_id=?1",
        params![asset_id],row
    ).optional().map_err(|e|e.to_string()).map(|value|value.unwrap_or_default())
}

pub fn search_text(conn:&Connection,asset_id:i64)->Result<String,String>{
    conn.query_row("SELECT search_text FROM visual_dna WHERE asset_id=?1",params![asset_id],|r|r.get(0))
        .optional().map_err(|e|e.to_string()).map(|x:Option<String>|x.unwrap_or_default())
}

pub fn combined_text(value:&VisualDnaPatch)->String{
    [
        &value.subject,&value.character,&value.outfit,&value.pose,&value.expression,&value.composition,
        &value.camera,&value.lighting,&value.environment,&value.palette,&value.material,&value.style
    ].iter().map(|x|x.trim()).filter(|x|!x.is_empty()).collect::<Vec<_>>().join(" ")
}

pub fn upsert(conn:&Connection,asset_id:i64,value:&VisualDnaPatch)->Result<VisualDna,String>{
    let value=normalize_patch(value.clone());
    let search=combined_text(&value);
    let stamp=db::now();
    conn.execute(
        "INSERT INTO visual_dna(
           asset_id,subject,character_name,outfit,pose,expression,composition,camera,lighting,environment,palette,material,style,search_text,source,updated_at
         ) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)
         ON CONFLICT(asset_id) DO UPDATE SET
           subject=excluded.subject,character_name=excluded.character_name,outfit=excluded.outfit,pose=excluded.pose,
           expression=excluded.expression,composition=excluded.composition,camera=excluded.camera,lighting=excluded.lighting,
           environment=excluded.environment,palette=excluded.palette,material=excluded.material,style=excluded.style,
           search_text=excluded.search_text,source=excluded.source,updated_at=excluded.updated_at",
        params![
            asset_id,value.subject,value.character,value.outfit,value.pose,value.expression,value.composition,value.camera,
            value.lighting,value.environment,value.palette,value.material,value.style,search,value.source,stamp
        ]
    ).map_err(|e|e.to_string())?;
    get(conn,asset_id)
}

pub fn merge_imported(conn:&Connection,asset_id:i64,value:Option<&VisualDnaPatch>)->Result<(),String>{
    let Some(value)=value else{return Ok(())};
    if patch_is_empty(value){return Ok(())}
    let current=get(conn,asset_id)?;
    if record_is_empty(&current){upsert(conn,asset_id,value)?;}
    Ok(())
}

#[tauri::command]
pub fn get_visual_dna(state:State<'_,AppState>,id:i64)->Result<VisualDna,String>{
    let conn=state.db.lock().map_err(|e|e.to_string())?;
    get(&conn,id)
}

#[tauri::command]
pub fn update_visual_dna(state:State<'_,AppState>,id:i64,value:VisualDnaPatch)->Result<VisualDna,String>{
    let mut conn=state.db.lock().map_err(|e|e.to_string())?;
    let tx=conn.transaction().map_err(|e|e.to_string())?;
    let saved=upsert(&tx,id,&value)?;
    tx.execute("UPDATE assets SET updated_at=?1 WHERE id=?2",params![db::now(),id]).map_err(|e|e.to_string())?;
    db::reindex_asset(&tx,id)?;
    tx.commit().map_err(|e|e.to_string())?;
    Ok(saved)
}

#[cfg(test)]
mod tests{
    use super::*;

    #[test]
    fn normalizes_and_bounds_visual_dna(){
        let patch=VisualDnaPatch{
            subject:format!("  {}  ","x".repeat(1400)),
            source:"unknown".into(),
            ..Default::default()
        };
        let value=normalize_patch(patch);
        assert_eq!(value.subject.chars().count(),1200);
        assert_eq!(value.source,"manual");
    }

    #[test]
    fn combined_text_contains_nonempty_fields(){
        let patch=VisualDnaPatch{
            subject:"成年东亚女性".into(),
            lighting:"暖色室内光".into(),
            style:"写实摄影".into(),
            ..Default::default()
        };
        let text=combined_text(&patch);
        assert!(text.contains("成年东亚女性"));
        assert!(text.contains("暖色室内光"));
        assert!(text.contains("写实摄影"));
    }
}
