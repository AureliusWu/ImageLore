use crate::models::{
    AssetRecord, AssetSummary, CollectionRecord, FacetCount, LibraryFacets,
};
use rusqlite::{
    params, Connection, OptionalExtension, Row,
};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

const DATA_DIR_NAME: &str = "app.imagelore.desktop";
const LEGACY_DATA_DIR_NAME: &str = "ImageLore";

fn copy_dir_all(source: &Path, target: &Path) -> Result<(), String> {
    fs::create_dir_all(target).map_err(|e| e.to_string())?;
    for entry in fs::read_dir(source).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let src = entry.path();
        let dst = target.join(entry.file_name());
        if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            copy_dir_all(&src, &dst)?;
        } else {
            if let Some(parent) = dst.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            fs::copy(&src, &dst).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn move_entry(source: &Path, target: &Path) -> Result<(), String> {
    if !source.exists() {
        return Ok(());
    }
    if target.exists() {
        if source.is_dir() && target.is_dir() {
            for entry in fs::read_dir(source).map_err(|e| e.to_string())? {
                let entry = entry.map_err(|e| e.to_string())?;
                move_entry(&entry.path(), &target.join(entry.file_name()))?;
            }
            let _ = fs::remove_dir(source);
        }
        // Existing destination data is always authoritative. Never overwrite it.
        return Ok(());
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    if fs::rename(source, target).is_ok() {
        return Ok(());
    }
    if source.is_dir() {
        copy_dir_all(source, target)?;
        fs::remove_dir_all(source).map_err(|e| e.to_string())?;
    } else {
        fs::copy(source, target).map_err(|e| e.to_string())?;
        fs::remove_file(source).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn migrate_legacy_data(base: &Path, root: &Path) -> Result<(), String> {
    let legacy = base.join(LEGACY_DATA_DIR_NAME);
    if !legacy.exists() || legacy == root {
        return Ok(());
    }
    fs::create_dir_all(root).map_err(|e| e.to_string())?;

    if !root.join("library.sqlite3").exists() {
        for name in [
            "library.sqlite3",
            "library.sqlite3-wal",
            "library.sqlite3-shm",
        ] {
            move_entry(&legacy.join(name), &root.join(name))?;
        }
    }

    for name in [
        "backups",
        "models",
        "restore.pending.sqlite3",
        "restore.next.sqlite3",
        "startup-error.log",
    ] {
        move_entry(&legacy.join(name), &root.join(name))?;
    }

    // Thumbnail/preview cache is fully derived. Rebuild it in the new data root
    // instead of carrying stale cache files through an installer upgrade.
    let legacy_cache = legacy.join("cache");
    if legacy_cache.exists() {
        let _ = fs::remove_dir_all(legacy_cache);
    }
    Ok(())
}

pub fn data_root() -> Result<(PathBuf, PathBuf), String> {
    let base = dirs::data_local_dir().ok_or("无法定位本地应用数据目录")?;
    let root = base.join(DATA_DIR_NAME);
    migrate_legacy_data(&base, &root)?;
    let cache = root.join("cache");
    fs::create_dir_all(cache.join("thumbnails")).map_err(|e| e.to_string())?;
    fs::create_dir_all(cache.join("previews")).map_err(|e| e.to_string())?;
    Ok((root, cache))
}

pub fn error_log_root() -> PathBuf {
    dirs::data_local_dir()
        .map(|x| x.join(DATA_DIR_NAME))
        .unwrap_or_else(std::env::temp_dir)
}

pub fn init_db(path: &Path) -> Result<Connection, String> {
    let conn = Connection::open(path).map_err(|e| e.to_string())?;
    conn.pragma_update(None, "foreign_keys", "ON")
        .map_err(|e| e.to_string())?;
    conn.pragma_update(None, "journal_mode", "WAL")
        .map_err(|e| e.to_string())?;
    conn.pragma_update(None, "synchronous", "NORMAL")
        .map_err(|e| e.to_string())?;
    crate::migrations::apply(&conn)?;
    Ok(conn)
}

fn row_asset(row: &Row<'_>) -> rusqlite::Result<AssetRecord> {
    let tag_blob: String = row.get(21)?;
    let tags = if tag_blob.is_empty() {
        Vec::new()
    } else {
        tag_blob.split('\u{1f}').map(str::to_string).collect()
    };
    Ok(AssetRecord {
        id: row.get(0)?,
        path: row.get(1)?,
        name: row.get(2)?,
        prompt: row.get(3)?,
        negative_prompt: row.get(4)?,
        model: row.get(5)?,
        favorite: row.get(6)?,
        width: row.get(7)?,
        height: row.get(8)?,
        file_size: row.get(9)?,
        format: row.get(10)?,
        mime_type: row.get(11)?,
        metadata_type: row.get(12)?,
        generation_json: row.get(13)?,
        fingerprint: row.get(14)?,
        portable_id: row.get(15)?,
        file_mtime: row.get(16)?,
        missing: row.get(17)?,
        created_at: row.get(18)?,
        updated_at: row.get(19)?,
        tags,
    })
}

const SELECT_ASSET: &str = r#"
SELECT a.id,a.path,a.name,
       COALESCE(ps.prompt,''),COALESCE(ps.negative_prompt,''),COALESCE(ps.model,''),
       a.favorite,a.width,a.height,a.file_size,a.format,a.mime_type,a.metadata_type,a.generation_json,
       a.fingerprint,a.portable_id,a.file_mtime,a.missing,a.created_at,a.updated_at,
       a.id AS asset_marker,
       COALESCE(GROUP_CONCAT(t.name, char(31)),'') AS tags
FROM assets a
LEFT JOIN prompt_state ps ON ps.asset_id=a.id
LEFT JOIN asset_tags at ON at.asset_id=a.id
LEFT JOIN tags t ON t.id=at.tag_id
"#;

pub(crate) fn row_summary(row: &Row<'_>) -> rusqlite::Result<AssetSummary> {
    Ok(AssetSummary {
        id: row.get(0)?,
        path: row.get(1)?,
        name: row.get(2)?,
        favorite: row.get(3)?,
        width: row.get(4)?,
        height: row.get(5)?,
        format: row.get(6)?,
        metadata_type: row.get(7)?,
        fingerprint: row.get(8)?,
        file_mtime: row.get(9)?,
        missing: row.get(10)?,
        updated_at: row.get(11)?,
    })
}

pub(crate) const SELECT_SUMMARY: &str = r#"SELECT a.id,a.path,a.name,a.favorite,a.width,a.height,a.format,a.metadata_type,a.fingerprint,a.file_mtime,a.missing,a.updated_at FROM assets a LEFT JOIN prompt_state ps ON ps.asset_id=a.id"#;

pub fn get_asset(conn: &Connection, id: i64) -> Result<AssetRecord, String> {
    let sql = format!("{} WHERE a.id=?1 GROUP BY a.id", SELECT_ASSET);
    conn.query_row(&sql, params![id], row_asset)
        .map_err(|e| e.to_string())
}

pub fn tags_for(conn: &Connection, asset_id: i64) -> Result<Vec<String>, String> {
    let mut st = conn.prepare("SELECT t.name FROM tags t JOIN asset_tags at ON at.tag_id=t.id WHERE at.asset_id=?1 ORDER BY t.name COLLATE NOCASE").map_err(|e| e.to_string())?;
    let rows = st
        .query_map(params![asset_id], |r| r.get::<_, String>(0))
        .map_err(|e| e.to_string())?;
    Ok(rows.filter_map(Result::ok).collect())
}

fn normalize_tags(tags: &[String]) -> Vec<String> {
    let mut out = Vec::<String>::new();
    for raw in tags {
        for piece in raw.split([',', '，']) {
            let tag = piece.trim();
            if !tag.is_empty() && !out.iter().any(|x| x.eq_ignore_ascii_case(tag)) {
                out.push(tag.to_string())
            }
        }
    }
    out
}

pub(crate) fn replace_tags_raw(
    conn: &Connection,
    asset_id: i64,
    tags: &[String],
) -> Result<(), String> {
    conn.execute(
        "DELETE FROM asset_tags WHERE asset_id=?1",
        params![asset_id],
    )
    .map_err(|e| e.to_string())?;
    for tag in normalize_tags(tags) {
        conn.execute(
            "INSERT OR IGNORE INTO tags(name,created_at) VALUES(?1,?2)",
            params![tag, now()],
        )
        .map_err(|e| e.to_string())?;
        let tag_id: i64 = conn
            .query_row(
                "SELECT id FROM tags WHERE name=?1 COLLATE NOCASE",
                params![tag],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT OR IGNORE INTO asset_tags(asset_id,tag_id,created_at) VALUES(?1,?2,?3)",
            params![asset_id, tag_id, now()],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn set_tags(conn: &mut Connection, asset_id: i64, tags: &[String]) -> Result<(), String> {
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    replace_tags_raw(&tx, asset_id, tags)?;
    tx.execute(
        "UPDATE assets SET updated_at=?1 WHERE id=?2",
        params![now(), asset_id],
    )
    .map_err(|e| e.to_string())?;
    reindex_asset(&tx, asset_id)?;
    tx.commit().map_err(|e| e.to_string())
}

pub fn add_tags(conn: &mut Connection, asset_ids: &[i64], tags: &[String]) -> Result<(), String> {
    let addition = normalize_tags(tags);
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    for id in asset_ids {
        let mut merged = tags_for(&tx, *id)?;
        for tag in &addition {
            if !merged.iter().any(|x| x.eq_ignore_ascii_case(tag)) {
                merged.push(tag.clone())
            }
        }
        replace_tags_raw(&tx, *id, &merged)?;
        tx.execute(
            "UPDATE assets SET updated_at=?1 WHERE id=?2",
            params![now(), id],
        )
        .map_err(|e| e.to_string())?;
        reindex_asset(&tx, *id)?;
    }
    tx.commit().map_err(|e| e.to_string())
}

pub fn reindex_asset(conn: &Connection, asset_id: i64) -> Result<(), String> {
    let asset = get_asset(conn, asset_id)?;
    let tags = asset.tags.join(" ");
    conn.execute(
        "DELETE FROM asset_search WHERE asset_id=?1",
        params![asset_id],
    )
    .map_err(|e| e.to_string())?;
    let visual_text = crate::visual_dna::search_text(conn, asset_id)?;
    let reference_text = crate::references::search_text(conn, asset_id)?;
    conn.execute(
        "INSERT INTO asset_search(asset_id,name,prompt,negative_prompt,model,tags,visual_dna,reference) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
        params![asset_id, asset.name, asset.prompt, asset.negative_prompt, asset.model, tags, visual_text, reference_text],
    ).map_err(|e| e.to_string())?;
    conn.execute(
        "DELETE FROM asset_cjk_search WHERE rowid=?1",
        params![asset_id],
    )
    .map_err(|e| e.to_string())?;
    let trigram_text = format!(
        "{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}",
        asset.name,
        asset.prompt,
        asset.negative_prompt,
        asset.model,
        asset.tags.join(" "),
        visual_text,
        reference_text
    );
    conn.execute(
        "INSERT INTO asset_cjk_search(rowid,asset_id,text) VALUES(?1,?1,?2)",
        params![asset_id, trigram_text],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn collections(conn: &Connection) -> Result<Vec<CollectionRecord>, String> {
    let mut st = conn.prepare(
        "SELECT c.id,c.name,c.description,c.created_at,c.updated_at,COUNT(ca.asset_id) FROM collections c LEFT JOIN collection_assets ca ON ca.collection_id=c.id GROUP BY c.id ORDER BY c.name COLLATE NOCASE"
    ).map_err(|e| e.to_string())?;
    let rows = st
        .query_map([], |r| {
            Ok(CollectionRecord {
                id: r.get(0)?,
                name: r.get(1)?,
                description: r.get(2)?,
                created_at: r.get(3)?,
                updated_at: r.get(4)?,
                count: r.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?;
    Ok(rows.filter_map(Result::ok).collect())
}

pub fn facets(conn: &Connection) -> Result<LibraryFacets, String> {
    let mut tag_st = conn.prepare("SELECT t.name,COUNT(at.asset_id) FROM tags t JOIN asset_tags at ON at.tag_id=t.id GROUP BY t.id ORDER BY COUNT(at.asset_id) DESC,t.name COLLATE NOCASE LIMIT 100").map_err(|e| e.to_string())?;
    let tags = tag_st
        .query_map([], |r| {
            Ok(FacetCount {
                name: r.get(0)?,
                count: r.get(1)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .collect();

    let mut model_st=conn.prepare(
        "SELECT normalized,COUNT(*) FROM (
           SELECT COALESCE((SELECT ma.canonical FROM model_aliases ma WHERE ma.alias=ps.model COLLATE NOCASE LIMIT 1),ps.model) AS normalized
           FROM prompt_state ps WHERE ps.model<>''
         ) GROUP BY normalized ORDER BY COUNT(*) DESC,normalized COLLATE NOCASE LIMIT 100"
    ).map_err(|e|e.to_string())?;
    let models = model_st
        .query_map([], |r| {
            Ok(FacetCount {
                name: r.get(0)?,
                count: r.get(1)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .collect();

    let mut metadata_st=conn.prepare(
        "SELECT metadata_type,COUNT(*) FROM assets GROUP BY metadata_type ORDER BY COUNT(*) DESC,metadata_type COLLATE NOCASE"
    ).map_err(|e|e.to_string())?;
    let metadata_types = metadata_st
        .query_map([], |r| {
            Ok(FacetCount {
                name: r.get(0)?,
                count: r.get(1)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .collect();

    let mut sampler_st=conn.prepare(
        "SELECT sampler,COUNT(*) FROM generation_index WHERE sampler<>'' GROUP BY sampler COLLATE NOCASE ORDER BY COUNT(*) DESC,sampler COLLATE NOCASE LIMIT 100"
    ).map_err(|e|e.to_string())?;
    let samplers = sampler_st
        .query_map([], |r| {
            Ok(FacetCount {
                name: r.get(0)?,
                count: r.get(1)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .collect();

    let mut scheduler_st=conn.prepare(
        "SELECT scheduler,COUNT(*) FROM generation_index WHERE scheduler<>'' GROUP BY scheduler COLLATE NOCASE ORDER BY COUNT(*) DESC,scheduler COLLATE NOCASE LIMIT 100"
    ).map_err(|e|e.to_string())?;
    let schedulers = scheduler_st
        .query_map([], |r| {
            Ok(FacetCount {
                name: r.get(0)?,
                count: r.get(1)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .collect();

    Ok(LibraryFacets {
        tags,
        models,
        collections: collections(conn)?,
        metadata_types,
        samplers,
        schedulers,
    })
}

pub fn asset_exists_by_path(conn: &Connection, path: &str) -> Result<Option<i64>, String> {
    conn.query_row("SELECT id FROM assets WHERE path=?1", params![path], |r| {
        r.get(0)
    })
    .optional()
    .map_err(|e| e.to_string())
}

pub fn asset_file_state_by_path(
    conn: &Connection,
    path: &str,
) -> Result<Option<(i64, i64, Option<i64>)>, String> {
    conn.query_row(
        "SELECT id,file_mtime,file_size FROM assets WHERE path=?1",
        params![path],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )
    .optional()
    .map_err(|e| e.to_string())
}

pub fn asset_exists_by_fingerprint(
    conn: &Connection,
    fingerprint: &str,
) -> Result<Option<i64>, String> {
    if fingerprint.is_empty() {
        return Ok(None);
    }
    conn.query_row(
        "SELECT id FROM assets WHERE fingerprint=?1 ORDER BY id LIMIT 1",
        params![fingerprint],
        |r| r.get(0),
    )
    .optional()
    .map_err(|e| e.to_string())
}

pub fn duplicate_groups(conn: &Connection) -> Result<Vec<crate::models::DuplicateGroup>, String> {
    let mut st = conn
        .prepare(
            "SELECT a.fingerprint,d.n,a.id,a.name
         FROM assets a
         JOIN (
           SELECT fingerprint,COUNT(*) AS n
           FROM assets
           WHERE fingerprint<>''
           GROUP BY fingerprint
           HAVING COUNT(*)>1
         ) d ON d.fingerprint=a.fingerprint
         ORDER BY d.n DESC,a.fingerprint,a.id",
        )
        .map_err(|e| e.to_string())?;
    let rows: Vec<(String, i64, i64, String)> = st
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .collect();

    let mut groups: Vec<crate::models::DuplicateGroup> = Vec::new();
    for (fingerprint, count, id, name) in rows {
        match groups.last_mut() {
            Some(group) if group.fingerprint == fingerprint => {
                group.asset_ids.push(id);
                group.names.push(name);
            }
            _ => groups.push(crate::models::DuplicateGroup {
                fingerprint,
                count,
                asset_ids: vec![id],
                names: vec![name],
            }),
        }
    }
    Ok(groups)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("../schema.sql")).unwrap();
        conn
    }

    #[test]
    fn legacy_data_migration_moves_persistent_state_out_of_install_root() {
        let token = format!("imagelore-upgrade-test-{}-{}", std::process::id(), now());
        let base = std::env::temp_dir().join(token);
        let legacy = base.join(LEGACY_DATA_DIR_NAME);
        let root = base.join(DATA_DIR_NAME);
        fs::create_dir_all(legacy.join("backups")).unwrap();
        fs::create_dir_all(legacy.join("models")).unwrap();
        fs::create_dir_all(legacy.join("cache").join("thumbnails")).unwrap();
        fs::create_dir_all(root.join("backups")).unwrap();
        fs::create_dir_all(root.join("models")).unwrap();
        fs::write(root.join("backups").join("newer.sqlite3"), b"newer").unwrap();
        fs::write(root.join("models").join("newer.onnx"), b"newer-model").unwrap();
        fs::write(legacy.join("library.sqlite3"), b"db").unwrap();
        fs::write(legacy.join("library.sqlite3-wal"), b"wal").unwrap();
        fs::write(legacy.join("backups").join("keep.sqlite3"), b"backup").unwrap();
        fs::write(legacy.join("models").join("model.onnx"), b"model").unwrap();
        fs::write(
            legacy.join("cache").join("thumbnails").join("old.webp"),
            b"cache",
        )
        .unwrap();

        migrate_legacy_data(&base, &root).unwrap();

        assert_eq!(fs::read(root.join("library.sqlite3")).unwrap(), b"db");
        assert_eq!(fs::read(root.join("library.sqlite3-wal")).unwrap(), b"wal");
        assert!(root.join("backups").join("keep.sqlite3").exists());
        assert!(root.join("backups").join("newer.sqlite3").exists());
        assert!(root.join("models").join("model.onnx").exists());
        assert!(root.join("models").join("newer.onnx").exists());
        assert!(!legacy.join("library.sqlite3").exists());
        assert!(!legacy.join("backups").exists());
        assert!(!legacy.join("models").exists());
        assert!(!legacy.join("cache").exists());
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn cjk_search_supports_multiple_terms() {
        let conn = test_conn();
        let now = now();
        conn.execute(
            "INSERT INTO assets(path,name,created_at,updated_at) VALUES('x.png','海洋角色',?1,?1)",
            params![now],
        )
        .unwrap();
        let id = conn.last_insert_rowid();
        conn.execute("INSERT INTO prompt_state(asset_id,prompt,negative_prompt,model,updated_at) VALUES(?1,'蓝色长发 成年女性','','Flux',?2)",params![id,now]).unwrap();
        conn.execute("INSERT INTO asset_search(asset_id,name,prompt,negative_prompt,model,tags) VALUES(?1,'海洋角色','蓝色长发 成年女性','','Flux','')",params![id]).unwrap();
        let filter = LibraryFilter {
            query: "蓝色 女性".into(),
            view: "all".into(),
            ..Default::default()
        };
        let page = crate::search::library_page(&conn, &filter, 0, 20).unwrap();
        assert_eq!(page.total, 1);
    }
}

pub fn make_portable_id(seed: &str) -> String {
    let mut hash = Sha256::new();
    hash.update(seed.as_bytes());
    let hex = format!("{:x}", hash.finalize());
    format!("il-{}", &hex[..32])
}

pub fn asset_by_portable_id(conn: &Connection, portable_id: &str) -> Result<Option<i64>, String> {
    if portable_id.trim().is_empty() {
        return Ok(None);
    }
    conn.query_row(
        "SELECT id FROM assets WHERE portable_id=?1",
        params![portable_id],
        |r| r.get(0),
    )
    .optional()
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod visual_dna_search_tests {
    use super::*;

    #[test]
    fn keyword_search_finds_visual_dna() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("../schema.sql")).unwrap();
        let id = 1i64;
        conn.execute(
            "INSERT INTO assets(id,path,name,created_at,updated_at) VALUES(?1,'dna.png','DNA Test',1,1)",params![id]
        ).unwrap();
        conn.execute(
            "INSERT INTO prompt_state(asset_id,prompt,negative_prompt,model,updated_at) VALUES(?1,'','','',1)",params![id]
        ).unwrap();
        crate::visual_dna::upsert(
            &conn,
            id,
            &crate::models::VisualDnaPatch {
                environment: "千禧年电脑房".into(),
                lighting: "CRT 蓝绿色冷光".into(),
                style: "日系写实摄影".into(),
                ..Default::default()
            },
        )
        .unwrap();
        reindex_asset(&conn, id).unwrap();
        let filter = LibraryFilter {
            query: "千禧年电脑房".into(),
            view: "all".into(),
            ..Default::default()
        };
        let page = crate::search::library_page(&conn, &filter, 0, 20).unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].id, id);
    }
}
