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
    export_sidecar_for_state(state.inner(), id)
}

pub(crate) fn export_sidecar_for_state(state: &AppState, id: i64) -> Result<String, String> {
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
    sidecar::write_atomic(&out, &text)?;
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
    let key = preview_cache_key(
        &asset.fingerprint,
        asset.id,
        meta.len(),
        meta.modified().ok(),
        asset.file_mtime,
    );
    preview::cached_preview_path(path, &state.cache_dir, &key, max_edge, thumbnail)
}

fn preview_cache_key(
    fingerprint: &str,
    asset_id: i64,
    source_size: u64,
    modified: Option<std::time::SystemTime>,
    fallback_mtime_ms: i64,
) -> String {
    // Source stamps retain filesystem precision. The library stores milliseconds,
    // so normalize that fallback to nanoseconds too; i128 also keeps signed
    // fallback values without multiplication overflow. A unit label prevents
    // reuse of cache names written with the former second-based key.
    let actual_mtime_ns = modified
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|x| x.as_nanos() as i128)
        .unwrap_or(i128::from(fallback_mtime_ms) * 1_000_000);
    let prefix = if fingerprint.is_empty() {
        format!("asset-{}", asset_id)
    } else {
        fingerprint.to_string()
    };
    format!("{}-mtime-ns-{}-{}", prefix, actual_mtime_ns, source_size)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    #[test]
    fn preview_cache_fallback_uses_library_milliseconds_and_keeps_key_namespace() {
        let library_mtime_ms = 1_790_752_500_123;
        let source_modified = UNIX_EPOCH + Duration::from_millis(library_mtime_ms as u64);
        let actual = preview_cache_key("fingerprint", 7, 822, Some(source_modified), 0);
        let fallback = preview_cache_key("fingerprint", 7, 822, None, library_mtime_ms);
        assert_eq!(actual, fallback);
        assert!(actual.starts_with("fingerprint-"));
        let legacy = format!("fingerprint-{}-822", library_mtime_ms / 1000);
        assert_ne!(actual, legacy);

        let without_fingerprint = preview_cache_key("", 7, 822, None, i64::MIN);
        assert!(without_fingerprint.starts_with("asset-7-mtime-ns-"));
        assert_ne!(
            without_fingerprint,
            preview_cache_key("", 7, 822, None, i64::MAX)
        );
    }

    #[test]
    fn same_size_image_replaced_within_one_second_gets_fresh_preview_cache() {
        let root = std::env::temp_dir().join(format!(
            "imagelore-preview-cache-key-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let source = root.join("source.bmp");
        let cache = root.join("cache");
        let fingerprint = "same-library-fingerprint";
        let second = 1_790_752_500;

        image::RgbImage::from_pixel(16, 16, image::Rgb([255, 0, 0]))
            .save(&source)
            .unwrap();
        fs::File::options()
            .write(true)
            .open(&source)
            .unwrap()
            .set_times(
                fs::FileTimes::new().set_modified(UNIX_EPOCH + Duration::new(second, 123_400_000)),
            )
            .unwrap();
        let first_meta = fs::metadata(&source).unwrap();
        let first_stamp = first_meta
            .modified()
            .unwrap()
            .duration_since(UNIX_EPOCH)
            .unwrap();
        let first_key = preview_cache_key(
            fingerprint,
            7,
            first_meta.len(),
            first_meta.modified().ok(),
            0,
        );
        let first_cache =
            preview::cached_preview_path(&source, &cache, &first_key, 2200, false).unwrap();
        let first_cache_bytes = fs::read(&first_cache).unwrap();

        image::RgbImage::from_pixel(16, 16, image::Rgb([0, 255, 0]))
            .save(&source)
            .unwrap();
        fs::File::options()
            .write(true)
            .open(&source)
            .unwrap()
            .set_times(
                fs::FileTimes::new().set_modified(UNIX_EPOCH + Duration::new(second, 800_900_000)),
            )
            .unwrap();
        let replacement_bytes = fs::read(&source).unwrap();
        let second_meta = fs::metadata(&source).unwrap();
        let second_stamp = second_meta
            .modified()
            .unwrap()
            .duration_since(UNIX_EPOCH)
            .unwrap();
        let second_key = preview_cache_key(
            fingerprint,
            7,
            second_meta.len(),
            second_meta.modified().ok(),
            0,
        );
        let second_cache =
            preview::cached_preview_path(&source, &cache, &second_key, 2200, false).unwrap();
        let first_pixel = image::open(&first_cache)
            .unwrap()
            .to_rgb8()
            .get_pixel(0, 0)
            .0;
        let second_pixel = image::open(&second_cache)
            .unwrap()
            .to_rgb8()
            .get_pixel(0, 0)
            .0;
        let source_preserved = fs::read(&source).unwrap() == replacement_bytes;
        let previous_cache_preserved = fs::read(&first_cache).unwrap() == first_cache_bytes;
        fs::remove_dir_all(&root).unwrap();

        assert_eq!(first_meta.len(), second_meta.len());
        assert_eq!(first_stamp.as_secs(), second_stamp.as_secs());
        assert_ne!(first_stamp.subsec_nanos(), second_stamp.subsec_nanos());
        assert_ne!(
            first_key, second_key,
            "Subsecond source changes need a fresh cache key"
        );
        assert_ne!(first_cache, second_cache);
        assert_eq!(first_pixel, [255, 0, 0]);
        assert_eq!(second_pixel, [0, 255, 0]);
        assert!(source_preserved);
        assert!(previous_cache_preserved);
    }
}
