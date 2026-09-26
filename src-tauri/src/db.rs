use crate::models::{AssetRecord, CollectionRecord, FacetCount, LibraryFacets, LibraryFilter, LibraryPage};
use rusqlite::{params, params_from_iter, types::Value as SqlValue, Connection, OptionalExtension, Row};
use std::{fs, path::{Path, PathBuf}, time::{SystemTime, UNIX_EPOCH}};

pub fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64
}

pub fn data_root() -> Result<(PathBuf, PathBuf), String> {
    let base = dirs::data_local_dir().ok_or("无法定位本地应用数据目录")?;
    let root = base.join("ImageLore");
    let cache = root.join("cache").join("thumbnails");
    fs::create_dir_all(&cache).map_err(|e| e.to_string())?;
    Ok((root, cache))
}

pub fn init_db(path: &Path) -> Result<Connection, String> {
    let conn = Connection::open(path).map_err(|e| e.to_string())?;
    conn.pragma_update(None, "foreign_keys", "ON").map_err(|e| e.to_string())?;
    conn.pragma_update(None, "journal_mode", "WAL").map_err(|e| e.to_string())?;
    conn.pragma_update(None, "synchronous", "NORMAL").map_err(|e| e.to_string())?;
    conn.execute_batch(include_str!("../schema.sql")).map_err(|e| e.to_string())?;
    Ok(conn)
}

fn row_asset(row: &Row<'_>) -> rusqlite::Result<AssetRecord> {
    let tag_blob: String = row.get(20)?;
    let tags = if tag_blob.is_empty() { Vec::new() } else { tag_blob.split('\u{1f}').map(str::to_string).collect() };
    Ok(AssetRecord {
        id: row.get(0)?, path: row.get(1)?, name: row.get(2)?,
        prompt: row.get(3)?, negative_prompt: row.get(4)?, model: row.get(5)?,
        favorite: row.get(6)?, width: row.get(7)?, height: row.get(8)?, file_size: row.get(9)?,
        format: row.get(10)?, mime_type: row.get(11)?, metadata_type: row.get(12)?, generation_json: row.get(13)?,
        fingerprint: row.get(14)?, file_mtime: row.get(15)?, missing: row.get(16)?,
        created_at: row.get(17)?, updated_at: row.get(18)?,
        tags,
    })
}

const SELECT_ASSET: &str = r#"
SELECT a.id,a.path,a.name,
       COALESCE(ps.prompt,''),COALESCE(ps.negative_prompt,''),COALESCE(ps.model,''),
       a.favorite,a.width,a.height,a.file_size,a.format,a.mime_type,a.metadata_type,a.generation_json,
       a.fingerprint,a.file_mtime,a.missing,a.created_at,a.updated_at,
       a.id AS asset_marker,
       COALESCE(GROUP_CONCAT(t.name, char(31)),'') AS tags
FROM assets a
LEFT JOIN prompt_state ps ON ps.asset_id=a.id
LEFT JOIN asset_tags at ON at.asset_id=a.id
LEFT JOIN tags t ON t.id=at.tag_id
"#;

pub fn get_asset(conn: &Connection, id: i64) -> Result<AssetRecord, String> {
    let sql = format!("{} WHERE a.id=?1 GROUP BY a.id", SELECT_ASSET);
    conn.query_row(&sql, params![id], row_asset).map_err(|e| e.to_string())
}

pub fn tags_for(conn: &Connection, asset_id: i64) -> Result<Vec<String>, String> {
    let mut st = conn.prepare("SELECT t.name FROM tags t JOIN asset_tags at ON at.tag_id=t.id WHERE at.asset_id=?1 ORDER BY t.name COLLATE NOCASE").map_err(|e| e.to_string())?;
    let rows = st.query_map(params![asset_id], |r| r.get::<_, String>(0)).map_err(|e| e.to_string())?;
    Ok(rows.filter_map(Result::ok).collect())
}

fn normalize_tags(tags: &[String]) -> Vec<String> {
    let mut out = Vec::<String>::new();
    for raw in tags {
        for piece in raw.split([',', '，']) {
            let tag = piece.trim();
            if tag.is_empty() { continue; }
            if !out.iter().any(|x| x.eq_ignore_ascii_case(tag)) { out.push(tag.to_string()); }
        }
    }
    out
}

pub fn set_tags(conn: &mut Connection, asset_id: i64, tags: &[String]) -> Result<(), String> {
    let tags = normalize_tags(tags);
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM asset_tags WHERE asset_id=?1", params![asset_id]).map_err(|e| e.to_string())?;
    for tag in tags {
        tx.execute("INSERT OR IGNORE INTO tags(name,created_at) VALUES(?1,?2)", params![tag, now()]).map_err(|e| e.to_string())?;
        let tag_id: i64 = tx.query_row("SELECT id FROM tags WHERE name=?1 COLLATE NOCASE", params![tag], |r| r.get(0)).map_err(|e| e.to_string())?;
        tx.execute("INSERT OR IGNORE INTO asset_tags(asset_id,tag_id,created_at) VALUES(?1,?2,?3)", params![asset_id, tag_id, now()]).map_err(|e| e.to_string())?;
    }
    tx.execute("UPDATE assets SET updated_at=?1 WHERE id=?2", params![now(), asset_id]).map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    reindex_asset(conn, asset_id)
}

pub fn add_tags(conn: &mut Connection, asset_ids: &[i64], tags: &[String]) -> Result<(), String> {
    let addition = normalize_tags(tags);
    for id in asset_ids {
        let mut merged = tags_for(conn, *id)?;
        for tag in &addition {
            if !merged.iter().any(|x| x.eq_ignore_ascii_case(tag)) { merged.push(tag.clone()); }
        }
        set_tags(conn, *id, &merged)?;
    }
    Ok(())
}

pub fn reindex_asset(conn: &Connection, asset_id: i64) -> Result<(), String> {
    let asset = get_asset(conn, asset_id)?;
    conn.execute("DELETE FROM asset_search WHERE asset_id=?1", params![asset_id]).map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO asset_search(asset_id,name,prompt,negative_prompt,model,tags) VALUES(?1,?2,?3,?4,?5,?6)",
        params![asset_id, asset.name, asset.prompt, asset.negative_prompt, asset.model, asset.tags.join(" ")],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

fn fts_query(input: &str) -> String {
    input.split_whitespace()
        .filter(|x| !x.is_empty())
        .map(|x| format!("\"{}\"*", x.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" AND ")
}

fn filter_parts(filter: &LibraryFilter) -> (String, String, Vec<SqlValue>) {
    let mut joins = String::new();
    let mut where_parts = vec!["1=1".to_string()];
    let mut args = Vec::<SqlValue>::new();

    if !filter.query.trim().is_empty() {
        joins.push_str(" JOIN asset_search ON asset_search.asset_id=a.id ");
        where_parts.push("asset_search MATCH ?".into());
        args.push(SqlValue::Text(fts_query(&filter.query)));
    }
    match filter.view.as_str() {
        "favorites" => where_parts.push("a.favorite=1".into()),
        "missing" => where_parts.push("a.missing=1".into()),
        "recent" => {
            where_parts.push("a.updated_at>=?".into());
            args.push(SqlValue::Integer(now() - 30 * 24 * 3600));
        }
        _ => {}
    }
    if let Some(tag) = filter.tag.as_ref().filter(|x| !x.trim().is_empty()) {
        where_parts.push("EXISTS(SELECT 1 FROM asset_tags fx JOIN tags ft ON ft.id=fx.tag_id WHERE fx.asset_id=a.id AND ft.name=? COLLATE NOCASE)".into());
        args.push(SqlValue::Text(tag.clone()));
    }
    if let Some(model) = filter.model.as_ref().filter(|x| !x.trim().is_empty()) {
        where_parts.push("COALESCE(ps.model,'')=?".into());
        args.push(SqlValue::Text(model.clone()));
    }
    if let Some(collection_id) = filter.collection_id {
        where_parts.push("EXISTS(SELECT 1 FROM collection_assets ca WHERE ca.asset_id=a.id AND ca.collection_id=?)".into());
        args.push(SqlValue::Integer(collection_id));
    }
    (joins, where_parts.join(" AND "), args)
}

pub fn library_page(conn: &Connection, filter: &LibraryFilter, offset: i64, limit: i64) -> Result<LibraryPage, String> {
    let offset = offset.max(0);
    let limit = limit.clamp(20, 500);
    let (joins, where_sql, args) = filter_parts(filter);

    let count_sql = format!("SELECT COUNT(DISTINCT a.id) FROM assets a LEFT JOIN prompt_state ps ON ps.asset_id=a.id {} WHERE {}", joins, where_sql);
    let total: i64 = conn.query_row(&count_sql, params_from_iter(args.clone()), |r| r.get(0)).map_err(|e| e.to_string())?;

    let order = if filter.view == "recent" { "a.updated_at DESC,a.id DESC" } else { "a.favorite DESC,a.updated_at DESC,a.id DESC" };
    let sql = format!("{} {} WHERE {} GROUP BY a.id ORDER BY {} LIMIT ? OFFSET ?", SELECT_ASSET, joins, where_sql, order);
    let mut page_args = args;
    page_args.push(SqlValue::Integer(limit));
    page_args.push(SqlValue::Integer(offset));
    let mut st = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let items = st.query_map(params_from_iter(page_args), row_asset).map_err(|e| e.to_string())?.filter_map(Result::ok).collect();
    Ok(LibraryPage { items, total, offset, limit })
}

pub fn collections(conn: &Connection) -> Result<Vec<CollectionRecord>, String> {
    let mut st = conn.prepare(
        "SELECT c.id,c.name,c.description,c.created_at,c.updated_at,COUNT(ca.asset_id) FROM collections c LEFT JOIN collection_assets ca ON ca.collection_id=c.id GROUP BY c.id ORDER BY c.name COLLATE NOCASE"
    ).map_err(|e| e.to_string())?;
    let rows = st.query_map([], |r| Ok(CollectionRecord {
        id:r.get(0)?, name:r.get(1)?, description:r.get(2)?, created_at:r.get(3)?, updated_at:r.get(4)?, count:r.get(5)?
    })).map_err(|e| e.to_string())?;
    Ok(rows.filter_map(Result::ok).collect())
}

pub fn facets(conn: &Connection) -> Result<LibraryFacets, String> {
    let mut tag_st = conn.prepare("SELECT t.name,COUNT(at.asset_id) FROM tags t JOIN asset_tags at ON at.tag_id=t.id GROUP BY t.id ORDER BY COUNT(at.asset_id) DESC,t.name COLLATE NOCASE LIMIT 100").map_err(|e| e.to_string())?;
    let tags = tag_st.query_map([], |r| Ok(FacetCount{name:r.get(0)?,count:r.get(1)?})).map_err(|e| e.to_string())?.filter_map(Result::ok).collect();

    let mut model_st = conn.prepare("SELECT model,COUNT(*) FROM prompt_state WHERE model<>'' GROUP BY model ORDER BY COUNT(*) DESC,model COLLATE NOCASE LIMIT 100").map_err(|e| e.to_string())?;
    let models = model_st.query_map([], |r| Ok(FacetCount{name:r.get(0)?,count:r.get(1)?})).map_err(|e| e.to_string())?.filter_map(Result::ok).collect();

    Ok(LibraryFacets { tags, models, collections: collections(conn)? })
}

pub fn asset_exists_by_path(conn: &Connection, path: &str) -> Result<Option<i64>, String> {
    conn.query_row("SELECT id FROM assets WHERE path=?1", params![path], |r| r.get(0)).optional().map_err(|e| e.to_string())
}
