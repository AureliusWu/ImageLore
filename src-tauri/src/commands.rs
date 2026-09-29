use crate::{db, generation_index, metadata, models::*, preview, search, sidecar, state::AppState};
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};
use tauri::State;
use walkdir::WalkDir;

#[tauri::command]
pub fn library_page(
    state: State<'_, AppState>,
    filter: LibraryFilter,
    offset: i64,
    limit: i64,
) -> Result<LibraryPage, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    search::library_page(&conn, &filter, offset, limit)
}

#[tauri::command]
pub fn library_facets(state: State<'_, AppState>) -> Result<LibraryFacets, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::facets(&conn)
}

#[tauri::command]
pub fn get_asset(state: State<'_, AppState>, id: i64) -> Result<AssetRecord, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::get_asset(&conn, id)
}

#[tauri::command]
pub fn update_prompt(
    state: State<'_, AppState>,
    id: i64,
    patch: PromptPatch,
) -> Result<AssetRecord, String> {
    let mut conn = state.db.lock().map_err(|e| e.to_string())?;
    let stamp = db::now();
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    tx.execute("INSERT INTO prompt_state(asset_id,prompt,negative_prompt,model,updated_at) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(asset_id) DO UPDATE SET prompt=excluded.prompt,negative_prompt=excluded.negative_prompt,model=excluded.model,updated_at=excluded.updated_at",params![id,patch.prompt,patch.negative_prompt,patch.model,stamp]).map_err(|e|e.to_string())?;
    tx.execute(
        "UPDATE assets SET updated_at=?1 WHERE id=?2",
        params![stamp, id],
    )
    .map_err(|e| e.to_string())?;
    db::reindex_asset(&tx, id)?;
    tx.commit().map_err(|e| e.to_string())?;
    db::get_asset(&conn, id)
}

#[tauri::command]
pub fn replace_tags(
    state: State<'_, AppState>,
    id: i64,
    tags: Vec<String>,
) -> Result<AssetRecord, String> {
    let mut conn = state.db.lock().map_err(|e| e.to_string())?;
    db::set_tags(&mut conn, id, &tags)?;
    db::get_asset(&conn, id)
}

#[tauri::command]
pub fn batch_add_tags(
    state: State<'_, AppState>,
    ids: Vec<i64>,
    tags: Vec<String>,
) -> Result<bool, String> {
    let mut conn = state.db.lock().map_err(|e| e.to_string())?;
    db::add_tags(&mut conn, &ids, &tags)?;
    Ok(true)
}

#[tauri::command]
pub fn toggle_favorite(state: State<'_, AppState>, id: i64) -> Result<AssetRecord, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    conn.execute("UPDATE assets SET favorite=CASE favorite WHEN 0 THEN 1 ELSE 0 END,updated_at=?1 WHERE id=?2",params![db::now(),id]).map_err(|e|e.to_string())?;
    db::get_asset(&conn, id)
}

#[tauri::command]
pub fn delete_asset(state: State<'_, AppState>, id: i64) -> Result<bool, String> {
    let fingerprint = {
        let mut conn = state.db.lock().map_err(|e| e.to_string())?;
        let fingerprint: String = conn
            .query_row(
                "SELECT fingerprint FROM assets WHERE id=?1",
                params![id],
                |r| r.get(0),
            )
            .unwrap_or_default();
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        tx.execute("DELETE FROM assets WHERE id=?1", params![id])
            .map_err(|e| e.to_string())?;
        tx.execute("DELETE FROM asset_search WHERE asset_id=?1", params![id])
            .map_err(|e| e.to_string())?;
        tx.execute("DELETE FROM asset_cjk_search WHERE rowid=?1", params![id])
            .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        fingerprint
    };
    let cache_prefix = if fingerprint.is_empty() {
        format!("asset-{}", id)
    } else {
        fingerprint
    };
    preview::purge_asset_cache(&state.cache_dir, &cache_prefix);
    Ok(true)
}

#[tauri::command]
pub fn add_revision(state: State<'_, AppState>, id: i64, note: String) -> Result<bool, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let asset = db::get_asset(&conn, id)?;
    let tags_json = serde_json::to_string(&asset.tags).map_err(|e| e.to_string())?;
    conn.execute("INSERT INTO prompt_revisions(asset_id,prompt,negative_prompt,model,tags_json,note,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![id,asset.prompt,asset.negative_prompt,asset.model,tags_json,note,db::now()]).map_err(|e|e.to_string())?;
    Ok(true)
}

#[tauri::command]
pub fn save_prompt_revision(
    state: State<'_, AppState>,
    id: i64,
    patch: PromptPatch,
    tags: Vec<String>,
    note: String,
) -> Result<AssetRecord, String> {
    let mut conn = state.db.lock().map_err(|e| e.to_string())?;
    let stamp = db::now();
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT INTO prompt_state(asset_id,prompt,negative_prompt,model,updated_at) VALUES(?1,?2,?3,?4,?5)
         ON CONFLICT(asset_id) DO UPDATE SET prompt=excluded.prompt,negative_prompt=excluded.negative_prompt,model=excluded.model,updated_at=excluded.updated_at",
        params![id,patch.prompt,patch.negative_prompt,patch.model,stamp]
    ).map_err(|e|e.to_string())?;
    db::replace_tags_raw(&tx, id, &tags)?;
    tx.execute(
        "UPDATE assets SET updated_at=?1 WHERE id=?2",
        params![stamp, id],
    )
    .map_err(|e| e.to_string())?;
    db::reindex_asset(&tx, id)?;
    let asset = db::get_asset(&tx, id)?;
    let tags_json = serde_json::to_string(&asset.tags).map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT INTO prompt_revisions(asset_id,prompt,negative_prompt,model,tags_json,note,created_at)
         VALUES(?1,?2,?3,?4,?5,?6,?7)",
        params![id,asset.prompt,asset.negative_prompt,asset.model,tags_json,note,stamp]
    ).map_err(|e|e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    db::get_asset(&conn, id)
}

#[tauri::command]
pub fn list_revisions(state: State<'_, AppState>, id: i64) -> Result<Vec<Revision>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut st=conn.prepare("SELECT id,asset_id,prompt,negative_prompt,model,tags_json,note,created_at FROM prompt_revisions WHERE asset_id=?1 ORDER BY created_at DESC,id DESC").map_err(|e|e.to_string())?;
    let rows = st
        .query_map(params![id], |r| {
            let tags_json: String = r.get(5)?;
            Ok(Revision {
                id: r.get(0)?,
                asset_id: r.get(1)?,
                prompt: r.get(2)?,
                negative_prompt: r.get(3)?,
                model: r.get(4)?,
                tags: serde_json::from_str(&tags_json).unwrap_or_default(),
                note: r.get(6)?,
                created_at: r.get(7)?,
            })
        })
        .map_err(|e| e.to_string())?;
    Ok(rows.filter_map(Result::ok).collect())
}

#[tauri::command]
pub fn restore_revision(
    state: State<'_, AppState>,
    revision_id: i64,
) -> Result<AssetRecord, String> {
    let mut conn = state.db.lock().map_err(|e| e.to_string())?;
    let rev:Revision=conn.query_row("SELECT id,asset_id,prompt,negative_prompt,model,tags_json,note,created_at FROM prompt_revisions WHERE id=?1",params![revision_id],|r|{
        let tags_json:String=r.get(5)?;
        Ok(Revision{id:r.get(0)?,asset_id:r.get(1)?,prompt:r.get(2)?,negative_prompt:r.get(3)?,model:r.get(4)?,tags:serde_json::from_str(&tags_json).unwrap_or_default(),note:r.get(6)?,created_at:r.get(7)?})
    }).map_err(|e|e.to_string())?;
    let current = db::get_asset(&conn, rev.asset_id)?;
    let current_tags = serde_json::to_string(&current.tags).map_err(|e| e.to_string())?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    tx.execute("INSERT INTO prompt_revisions(asset_id,prompt,negative_prompt,model,tags_json,note,created_at) VALUES(?1,?2,?3,?4,?5,'Before restore',?6)",params![current.id,current.prompt,current.negative_prompt,current.model,current_tags,db::now()]).map_err(|e|e.to_string())?;
    tx.execute("UPDATE prompt_state SET prompt=?1,negative_prompt=?2,model=?3,updated_at=?4 WHERE asset_id=?5",params![rev.prompt,rev.negative_prompt,rev.model,db::now(),rev.asset_id]).map_err(|e|e.to_string())?;
    db::replace_tags_raw(&tx, rev.asset_id, &rev.tags)?;
    tx.execute(
        "UPDATE assets SET updated_at=?1 WHERE id=?2",
        params![db::now(), rev.asset_id],
    )
    .map_err(|e| e.to_string())?;
    db::reindex_asset(&tx, rev.asset_id)?;
    tx.commit().map_err(|e| e.to_string())?;
    db::get_asset(&conn, rev.asset_id)
}

fn path_exists(conn: &Connection, start: i64, target: i64) -> bool {
    let mut stack = vec![start];
    let mut seen = HashSet::new();
    while let Some(id) = stack.pop() {
        if id == target {
            return true;
        }
        if !seen.insert(id) {
            continue;
        }
        if let Ok(mut st) = conn.prepare("SELECT child_id FROM relations WHERE parent_id=?1") {
            if let Ok(rows) = st.query_map(params![id], |r| r.get::<_, i64>(0)) {
                for child in rows.filter_map(Result::ok) {
                    stack.push(child)
                }
            }
        }
    }
    false
}

#[tauri::command]
pub fn add_relation(
    state: State<'_, AppState>,
    parent_id: i64,
    child_id: i64,
    relation_type: String,
    note: String,
) -> Result<bool, String> {
    if parent_id == child_id {
        return Err("不能把记录关联到自己".into());
    }
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    if path_exists(&conn, child_id, parent_id) {
        return Err("该关系会形成循环谱系".into());
    }
    conn.execute("INSERT OR IGNORE INTO relations(parent_id,child_id,relation_type,note,created_at) VALUES(?1,?2,?3,?4,?5)",params![parent_id,child_id,relation_type,note,db::now()]).map_err(|e|e.to_string())?;
    Ok(true)
}

fn relations(conn: &Connection, id: i64, parents: bool) -> Result<Vec<RelationRecord>, String> {
    let sql = if parents {
        "SELECT r.id,r.parent_id,r.child_id,r.relation_type,r.note,r.created_at,a.id,a.name FROM relations r JOIN assets a ON a.id=r.parent_id WHERE r.child_id=?1 ORDER BY r.created_at DESC"
    } else {
        "SELECT r.id,r.parent_id,r.child_id,r.relation_type,r.note,r.created_at,a.id,a.name FROM relations r JOIN assets a ON a.id=r.child_id WHERE r.parent_id=?1 ORDER BY r.created_at DESC"
    };
    let mut st = conn.prepare(sql).map_err(|e| e.to_string())?;
    let rows = st
        .query_map(params![id], |r| {
            Ok(RelationRecord {
                id: r.get(0)?,
                parent_id: r.get(1)?,
                child_id: r.get(2)?,
                relation_type: r.get(3)?,
                note: r.get(4)?,
                created_at: r.get(5)?,
                other_id: r.get(6)?,
                other_name: r.get(7)?,
            })
        })
        .map_err(|e| e.to_string())?;
    Ok(rows.filter_map(Result::ok).collect())
}

#[tauri::command]
pub fn lineage(state: State<'_, AppState>, id: i64) -> Result<Lineage, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    Ok(Lineage {
        parents: relations(&conn, id, true)?,
        children: relations(&conn, id, false)?,
    })
}

#[tauri::command]
pub fn collections(state: State<'_, AppState>) -> Result<Vec<CollectionRecord>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::collections(&conn)
}

#[tauri::command]
pub fn create_collection(
    state: State<'_, AppState>,
    name: String,
    description: String,
) -> Result<CollectionRecord, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("集合名称不能为空".into());
    }
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let stamp = db::now();
    conn.execute(
        "INSERT INTO collections(name,description,created_at,updated_at) VALUES(?1,?2,?3,?3)",
        params![name, description.trim(), stamp],
    )
    .map_err(|e| {
        if e.to_string().contains("UNIQUE") {
            "已存在同名集合".into()
        } else {
            e.to_string()
        }
    })?;
    let id = conn.last_insert_rowid();
    Ok(CollectionRecord {
        id,
        name: name.to_string(),
        description: description.trim().to_string(),
        created_at: stamp,
        updated_at: stamp,
        count: 0,
    })
}

#[tauri::command]
pub fn add_to_collection(
    state: State<'_, AppState>,
    collection_id: i64,
    asset_ids: Vec<i64>,
) -> Result<bool, String> {
    let mut conn = state.db.lock().map_err(|e| e.to_string())?;
    let exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM collections WHERE id=?1",
            params![collection_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    if exists == 0 {
        return Err("找不到目标集合".into());
    }
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let stamp = db::now();
    for id in asset_ids {
        tx.execute("INSERT OR IGNORE INTO collection_assets(collection_id,asset_id,created_at) VALUES(?1,?2,?3)",params![collection_id,id,stamp]).map_err(|e|e.to_string())?;
    }
    tx.execute(
        "UPDATE collections SET updated_at=?1 WHERE id=?2",
        params![stamp, collection_id],
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(true)
}

#[tauri::command]
pub fn rescan_metadata(state: State<'_, AppState>, id: i64) -> Result<AssetRecord, String> {
    let old = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        db::get_asset(&conn, id)?
    };
    let path = PathBuf::from(&old.path);
    if !path.exists() {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE assets SET missing=1,updated_at=?1 WHERE id=?2",
            params![db::now(), id],
        )
        .map_err(|e| e.to_string())?;
        return db::get_asset(&conn, id);
    }

    let info = metadata::file_info(&path);
    let fp = metadata::fingerprint(&path);
    let extract = metadata::extract_generation(&path);
    let mut conn = state.db.lock().map_err(|e| e.to_string())?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let stamp = db::now();
    tx.execute(
        "UPDATE assets SET width=?1,height=?2,file_size=?3,format=?4,mime_type=?5,metadata_type=?6,generation_json=?7,fingerprint=?8,file_mtime=?9,missing=0,updated_at=?10 WHERE id=?11",
        params![info.width,info.height,info.file_size,info.format,info.mime_type,extract.metadata_type,extract.generation_json,fp,info.file_mtime,stamp,id]
    ).map_err(|e|e.to_string())?;
    if old.prompt.is_empty() && old.negative_prompt.is_empty() && old.model.is_empty() {
        tx.execute("UPDATE prompt_state SET prompt=?1,negative_prompt=?2,model=?3,updated_at=?4 WHERE asset_id=?5",params![extract.prompt,extract.negative_prompt,extract.model,stamp,id]).map_err(|e|e.to_string())?;
    }
    generation_index::upsert(&tx, id, &extract.generation_json)?;
    db::reindex_asset(&tx, id)?;
    tx.commit().map_err(|e| e.to_string())?;
    let refreshed = db::get_asset(&conn, id)?;
    drop(conn);
    preview::purge_asset_cache(&state.cache_dir, &old.fingerprint);
    Ok(refreshed)
}

#[tauri::command]
pub fn export_sidecar(state: State<'_, AppState>, id: i64) -> Result<String, String> {
    let (asset, out, text) = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        let asset = db::get_asset(&conn, id)?;
        let mut parent_st=conn.prepare(
            "SELECT p.portable_id,p.fingerprint,r.relation_type,r.note FROM relations r JOIN assets p ON p.id=r.parent_id WHERE r.child_id=?1 ORDER BY r.created_at"
        ).map_err(|e|e.to_string())?;
        let parents: Vec<Value> = parent_st
            .query_map(params![id], |r| {
                Ok(json!({
                    "portable_id":r.get::<_,String>(0)?,
                    "fingerprint":r.get::<_,String>(1)?,
                    "relation_type":r.get::<_,String>(2)?,
                    "note":r.get::<_,String>(3)?
                }))
            })
            .map_err(|e| e.to_string())?
            .filter_map(Result::ok)
            .collect();
        let visual_dna = crate::visual_dna::get(&conn, id)?;
        let visual_dna = if crate::visual_dna::record_is_empty(&visual_dna) {
            Value::Null
        } else {
            serde_json::to_value(visual_dna).map_err(|e| e.to_string())?
        };
        let references=crate::references::list(&conn,id)?.into_iter().map(|item|{
            json!({
                "source_url":item.source_url,
                "page_url":item.page_url,
                "page_title":item.page_title,
                "source_type":item.source_type,
                "captured_at":item.captured_at,
                "metadata":serde_json::from_str::<Value>(&item.metadata_json).unwrap_or(Value::Object(Default::default()))
            })
        }).collect::<Vec<_>>();
        let session:Option<Value>=conn.query_row(
            "SELECT s.name,s.note,a.note FROM asset_sessions a JOIN generation_sessions s ON s.id=a.session_id WHERE a.asset_id=?1",
            params![id],
            |r|Ok(json!({"name":r.get::<_,String>(0)?,"session_note":r.get::<_,String>(1)?,"asset_note":r.get::<_,String>(2)?}))
        ).ok();
        let data = json!({
            "schema":"imagelore.sidecar.v3",
            "asset":{"portable_id":asset.portable_id,"fingerprint":asset.fingerprint,"name":asset.name},
            "image":asset.path,
            "prompt":asset.prompt,
            "negative_prompt":asset.negative_prompt,
            "model":asset.model,
            "tags":asset.tags,
            "metadata_type":asset.metadata_type,
            "generation":serde_json::from_str::<Value>(&asset.generation_json).unwrap_or(Value::String(asset.generation_json.clone())),
            "visual_dna":visual_dna,
            "references":references,
            "session":session,
            "parents":parents
        });
        let out = sidecar::path_for(Path::new(&asset.path));
        let text = serde_json::to_string_pretty(&data).map_err(|e| e.to_string())?;
        (asset, out, text)
    };
    if asset.missing != 0 || !Path::new(&asset.path).exists() {
        return Err("原图片文件不存在，无法导出 Sidecar".into());
    }
    fs::write(&out, text).map_err(|e| e.to_string())?;
    Ok(out.to_string_lossy().to_string())
}

#[tauri::command]
pub fn preview_cache_path(
    state: State<'_, AppState>,
    id: i64,
    max_edge: u32,
    thumbnail: bool,
) -> Result<String, String> {
    let asset = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        db::get_asset(&conn, id)?
    };
    let path = Path::new(&asset.path);
    if !path.exists() {
        return Err("原图片文件不存在".into());
    }
    let meta = fs::metadata(path).map_err(|e| e.to_string())?;
    let actual_mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|x| x.as_secs() as i64)
        .unwrap_or(asset.file_mtime);
    let prefix = if asset.fingerprint.is_empty() {
        format!("asset-{}", asset.id)
    } else {
        asset.fingerprint.clone()
    };
    let key = format!("{}-{}-{}", prefix, actual_mtime, meta.len());
    preview::cached_preview_path(path, &state.cache_dir, &key, max_edge, thumbnail)
}

#[tauri::command]
pub fn refresh_missing(state: State<'_, AppState>) -> Result<i64, String> {
    let rows: Vec<(i64, String, i64)> = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        let mut st = conn
            .prepare("SELECT id,path,missing FROM assets")
            .map_err(|e| e.to_string())?;
        let rows = st
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .map_err(|e| e.to_string())?
            .filter_map(Result::ok)
            .collect();
        rows
    };
    let mut missing_count = 0;
    let mut changes = Vec::new();
    for (id, path, old) in rows {
        let missing = if Path::new(&path).exists() { 0 } else { 1 };
        if missing == 1 {
            missing_count += 1
        }
        if missing != old {
            changes.push((id, missing));
        }
    }
    if !changes.is_empty() {
        let mut conn = state.db.lock().map_err(|e| e.to_string())?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        for (id, missing) in changes {
            tx.execute(
                "UPDATE assets SET missing=?1 WHERE id=?2",
                params![missing, id],
            )
            .map_err(|e| e.to_string())?;
        }
        tx.commit().map_err(|e| e.to_string())?;
    }
    Ok(missing_count)
}

#[tauri::command]
pub fn relocate_missing(state: State<'_, AppState>, root: String) -> Result<i64, String> {
    let wanted: Vec<(i64, String)> = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        let mut st = conn
            .prepare("SELECT id,fingerprint FROM assets WHERE missing=1 AND fingerprint<>''")
            .map_err(|e| e.to_string())?;
        let rows = st
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(|e| e.to_string())?
            .filter_map(Result::ok)
            .collect();
        rows
    };
    let fingerprints = wanted.iter().map(|x| x.1.clone()).collect::<HashSet<_>>();
    let mut found = HashMap::<String, PathBuf>::new();
    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
    {
        if !entry.file_type().is_file() || !metadata::is_supported(entry.path()) {
            continue;
        }
        let fp = metadata::fingerprint(entry.path());
        if fingerprints.contains(&fp) {
            found.insert(fp, entry.path().to_path_buf());
            if found.len() == fingerprints.len() {
                break;
            }
        }
    }

    let mut updates = Vec::new();
    for (id, fp) in wanted {
        if let Some(path) = found.get(&fp) {
            let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
            let name = canonical
                .file_name()
                .and_then(|x| x.to_str())
                .unwrap_or("image")
                .to_string();
            updates.push((id, canonical.to_string_lossy().to_string(), name));
        }
    }
    if updates.is_empty() {
        return Ok(0);
    }
    let fixed = updates.len() as i64;
    let mut conn = state.db.lock().map_err(|e| e.to_string())?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let stamp = db::now();
    for (id, path, name) in updates {
        tx.execute(
            "UPDATE assets SET path=?1,name=?2,missing=0,updated_at=?3 WHERE id=?4",
            params![path, name, stamp, id],
        )
        .map_err(|e| e.to_string())?;
        db::reindex_asset(&tx, id)?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(fixed)
}

#[tauri::command]
pub fn duplicate_groups(state: State<'_, AppState>) -> Result<Vec<DuplicateGroup>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::duplicate_groups(&conn)
}

fn reindex_many(conn: &Connection, ids: &[i64]) -> Result<(), String> {
    for id in ids {
        db::reindex_asset(conn, *id)?
    }
    Ok(())
}

#[tauri::command]
pub fn rename_tag(
    state: State<'_, AppState>,
    old_name: String,
    new_name: String,
) -> Result<bool, String> {
    let old = old_name.trim();
    let new = new_name.trim();
    if old.is_empty() || new.is_empty() {
        return Err("标签名称不能为空".into());
    }
    let mut conn = state.db.lock().map_err(|e| e.to_string())?;
    let source: Option<i64> = conn
        .query_row(
            "SELECT id FROM tags WHERE name=?1 COLLATE NOCASE",
            params![old],
            |r| r.get(0),
        )
        .ok();
    let Some(source_id) = source else {
        return Err("找不到原标签".into());
    };
    let ids: Vec<i64> = {
        let mut st = conn
            .prepare("SELECT asset_id FROM asset_tags WHERE tag_id=?1")
            .map_err(|e| e.to_string())?;
        let rows = st
            .query_map(params![source_id], |r| r.get(0))
            .map_err(|e| e.to_string())?
            .filter_map(Result::ok)
            .collect();
        rows
    };
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    if let Ok(target_id) = tx.query_row(
        "SELECT id FROM tags WHERE name=?1 COLLATE NOCASE",
        params![new],
        |r| r.get::<_, i64>(0),
    ) {
        if target_id != source_id {
            tx.execute(
                "INSERT OR IGNORE INTO asset_tags(asset_id,tag_id,created_at) SELECT asset_id,?1,created_at FROM asset_tags WHERE tag_id=?2",
                params![target_id,source_id]
            ).map_err(|e|e.to_string())?;
            tx.execute("DELETE FROM tags WHERE id=?1", params![source_id])
                .map_err(|e| e.to_string())?;
        }
    } else {
        tx.execute(
            "UPDATE tags SET name=?1 WHERE id=?2",
            params![new, source_id],
        )
        .map_err(|e| e.to_string())?;
    }
    reindex_many(&tx, &ids)?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(true)
}

#[tauri::command]
pub fn merge_tags(
    state: State<'_, AppState>,
    source_name: String,
    target_name: String,
) -> Result<bool, String> {
    rename_tag(state, source_name, target_name)
}

#[tauri::command]
pub fn delete_tag(state: State<'_, AppState>, name: String) -> Result<bool, String> {
    let mut conn = state.db.lock().map_err(|e| e.to_string())?;
    let tag_id: i64 = conn
        .query_row(
            "SELECT id FROM tags WHERE name=?1 COLLATE NOCASE",
            params![name.trim()],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let ids: Vec<i64> = {
        let mut st = conn
            .prepare("SELECT asset_id FROM asset_tags WHERE tag_id=?1")
            .map_err(|e| e.to_string())?;
        let rows = st
            .query_map(params![tag_id], |r| r.get(0))
            .map_err(|e| e.to_string())?
            .filter_map(Result::ok)
            .collect();
        rows
    };
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM tags WHERE id=?1", params![tag_id])
        .map_err(|e| e.to_string())?;
    reindex_many(&tx, &ids)?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(true)
}

#[tauri::command]
pub fn rename_collection(
    state: State<'_, AppState>,
    id: i64,
    name: String,
) -> Result<bool, String> {
    let value = name.trim();
    if value.is_empty() {
        return Err("集合名称不能为空".into());
    }
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE collections SET name=?1,updated_at=?2 WHERE id=?3",
        params![value, db::now(), id],
    )
    .map_err(|e| {
        if e.to_string().contains("UNIQUE") {
            "已存在同名集合".into()
        } else {
            e.to_string()
        }
    })?;
    Ok(true)
}

#[tauri::command]
pub fn delete_collection(state: State<'_, AppState>, id: i64) -> Result<bool, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM collections WHERE id=?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(true)
}
