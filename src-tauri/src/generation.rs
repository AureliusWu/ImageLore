use crate::{db, models::*, sidecar, state::AppState};
use rusqlite::{params, Connection, OptionalExtension};
use tauri::State;
use walkdir::WalkDir;

fn path_exists(conn: &Connection, start: i64, target: i64) -> bool {
    let mut stack = vec![start];
    let mut seen = std::collections::HashSet::new();
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

pub(crate) fn resolve_pending_relations(conn: &Connection) -> Result<usize, String> {
    let rows: Vec<(i64, String, String, String, String, String)> = {
        let mut st=conn.prepare(
            "SELECT rowid,child_portable_id,parent_portable_id,parent_fingerprint,relation_type,note FROM pending_relations ORDER BY created_at,rowid"
        ).map_err(|e|e.to_string())?;
        let mapped = st
            .query_map([], |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        mapped.filter_map(Result::ok).collect()
    };
    let mut resolved = 0;
    for (rowid, child_portable, parent_portable, parent_fingerprint, relation_type, note) in rows {
        let child = db::asset_by_portable_id(conn, &child_portable)?;
        let by_portable = if parent_portable.is_empty() {
            None
        } else {
            db::asset_by_portable_id(conn, &parent_portable)?
        };
        let parent = match by_portable {
            Some(id) => Some(id),
            None => db::asset_exists_by_fingerprint(conn, &parent_fingerprint)?,
        };
        let (Some(child_id), Some(parent_id)) = (child, parent) else {
            continue;
        };
        if child_id != parent_id && !path_exists(conn, child_id, parent_id) {
            conn.execute(
                "INSERT OR IGNORE INTO relations(parent_id,child_id,relation_type,note,created_at) VALUES(?1,?2,?3,?4,?5)",
                params![parent_id,child_id,relation_type,note,db::now()]
            ).map_err(|e|e.to_string())?;
        }
        conn.execute(
            "DELETE FROM pending_relations WHERE rowid=?1",
            params![rowid],
        )
        .map_err(|e| e.to_string())?;
        resolved += 1;
    }
    Ok(resolved)
}

pub(crate) fn queue_parent_refs(
    conn: &Connection,
    child_portable_id: &str,
    parents: &[sidecar::ParentRef],
) -> Result<(), String> {
    for parent in parents {
        conn.execute(
            "INSERT OR IGNORE INTO pending_relations(child_portable_id,parent_portable_id,parent_fingerprint,relation_type,note,created_at) VALUES(?1,?2,?3,?4,?5,?6)",
            params![child_portable_id,parent.portable_id,parent.fingerprint,parent.relation_type,parent.note,db::now()]
        ).map_err(|e|e.to_string())?;
    }
    resolve_pending_relations(conn)?;
    Ok(())
}

pub(crate) fn assign_session_by_name(
    conn: &Connection,
    asset_id: i64,
    session: Option<&sidecar::SessionRef>,
) -> Result<(), String> {
    let Some(session) = session else {
        return Ok(());
    };
    let stamp = db::now();
    conn.execute(
        "INSERT INTO generation_sessions(name,note,created_at,updated_at) VALUES(?1,?2,?3,?3) ON CONFLICT(name) DO UPDATE SET note=CASE WHEN generation_sessions.note='' THEN excluded.note ELSE generation_sessions.note END,updated_at=excluded.updated_at",
        params![session.name,session.session_note,stamp]
    ).map_err(|e|e.to_string())?;
    let session_id: i64 = conn
        .query_row(
            "SELECT id FROM generation_sessions WHERE name=?1 COLLATE NOCASE",
            params![session.name],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO asset_sessions(asset_id,session_id,note,created_at,updated_at) VALUES(?1,?2,?3,?4,?4) ON CONFLICT(asset_id) DO UPDATE SET session_id=excluded.session_id,note=CASE WHEN asset_sessions.note='' THEN excluded.note ELSE asset_sessions.note END,updated_at=excluded.updated_at",
        params![asset_id,session_id,session.asset_note,stamp]
    ).map_err(|e|e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn generation_sessions(state: State<'_, AppState>) -> Result<Vec<GenerationSession>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut st=conn.prepare(
        "SELECT s.id,s.name,s.note,COUNT(a.asset_id),s.created_at,s.updated_at FROM generation_sessions s LEFT JOIN asset_sessions a ON a.session_id=s.id GROUP BY s.id ORDER BY s.updated_at DESC,s.name COLLATE NOCASE"
    ).map_err(|e|e.to_string())?;
    let mapped = st
        .query_map([], |r| {
            Ok(GenerationSession {
                id: r.get(0)?,
                name: r.get(1)?,
                note: r.get(2)?,
                count: r.get(3)?,
                created_at: r.get(4)?,
                updated_at: r.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?;
    let items = mapped.filter_map(Result::ok).collect();
    Ok(items)
}

#[tauri::command]
pub fn create_generation_session(
    state: State<'_, AppState>,
    name: String,
    note: String,
) -> Result<GenerationSession, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("会话名称不能为空".into());
    }
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let stamp = db::now();
    conn.execute(
        "INSERT INTO generation_sessions(name,note,created_at,updated_at) VALUES(?1,?2,?3,?3)",
        params![name, note.trim(), stamp],
    )
    .map_err(|e| {
        if e.to_string().contains("UNIQUE") {
            "已存在同名会话".into()
        } else {
            e.to_string()
        }
    })?;
    let id = conn.last_insert_rowid();
    Ok(GenerationSession {
        id,
        name: name.to_string(),
        note: note.trim().to_string(),
        count: 0,
        created_at: stamp,
        updated_at: stamp,
    })
}

#[tauri::command]
pub fn asset_session(
    state: State<'_, AppState>,
    asset_id: i64,
) -> Result<Option<AssetSession>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    conn.query_row(
        "SELECT s.id,s.name,s.note,a.note FROM asset_sessions a JOIN generation_sessions s ON s.id=a.session_id WHERE a.asset_id=?1",
        params![asset_id],
        |r|Ok(AssetSession{session_id:r.get(0)?,session_name:r.get(1)?,session_note:r.get(2)?,asset_note:r.get(3)?})
    ).optional().map_err(|e|e.to_string())
}

#[tauri::command]
pub fn set_asset_session(
    state: State<'_, AppState>,
    asset_id: i64,
    session_id: Option<i64>,
    note: String,
) -> Result<bool, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    if let Some(session_id) = session_id {
        let exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM generation_sessions WHERE id=?1",
                params![session_id],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        if exists == 0 {
            return Err("找不到目标生成会话".into());
        }
        let stamp = db::now();
        conn.execute(
            "INSERT INTO asset_sessions(asset_id,session_id,note,created_at,updated_at) VALUES(?1,?2,?3,?4,?4) ON CONFLICT(asset_id) DO UPDATE SET session_id=excluded.session_id,note=excluded.note,updated_at=excluded.updated_at",
            params![asset_id,session_id,note.trim(),stamp]
        ).map_err(|e|e.to_string())?;
    } else {
        conn.execute(
            "DELETE FROM asset_sessions WHERE asset_id=?1",
            params![asset_id],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(true)
}

#[tauri::command]
pub fn update_relation_note(
    state: State<'_, AppState>,
    relation_id: i64,
    note: String,
) -> Result<bool, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let changed = conn
        .execute(
            "UPDATE relations SET note=?1 WHERE id=?2",
            params![note.trim(), relation_id],
        )
        .map_err(|e| e.to_string())?;
    if changed == 0 {
        return Err("找不到谱系关系".into());
    }
    Ok(true)
}

#[tauri::command]
pub fn model_aliases(state: State<'_, AppState>) -> Result<Vec<ModelAlias>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut st=conn.prepare("SELECT alias,canonical FROM model_aliases ORDER BY canonical COLLATE NOCASE,alias COLLATE NOCASE").map_err(|e|e.to_string())?;
    let mapped = st
        .query_map([], |r| {
            Ok(ModelAlias {
                alias: r.get(0)?,
                canonical: r.get(1)?,
            })
        })
        .map_err(|e| e.to_string())?;
    let items = mapped.filter_map(Result::ok).collect();
    Ok(items)
}

#[tauri::command]
pub fn upsert_model_alias(
    state: State<'_, AppState>,
    alias: String,
    canonical: String,
) -> Result<bool, String> {
    let alias = alias.trim();
    let canonical = canonical.trim();
    if alias.is_empty() || canonical.is_empty() {
        return Err("模型别名和规范名称不能为空".into());
    }
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let stamp = db::now();
    conn.execute(
        "INSERT INTO model_aliases(alias,canonical,created_at,updated_at) VALUES(?1,?2,?3,?3) ON CONFLICT(alias) DO UPDATE SET canonical=excluded.canonical,updated_at=excluded.updated_at",
        params![alias,canonical,stamp]
    ).map_err(|e|e.to_string())?;
    Ok(true)
}

#[tauri::command]
pub fn delete_model_alias(state: State<'_, AppState>, alias: String) -> Result<bool, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "DELETE FROM model_aliases WHERE alias=?1 COLLATE NOCASE",
        params![alias.trim()],
    )
    .map_err(|e| e.to_string())?;
    Ok(true)
}

#[tauri::command]
pub fn saved_filters(state: State<'_, AppState>) -> Result<Vec<SavedFilter>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut st=conn.prepare("SELECT id,name,filter_json,created_at,updated_at FROM saved_filters ORDER BY name COLLATE NOCASE").map_err(|e|e.to_string())?;
    let rows = st
        .query_map([], |r| {
            let raw: String = r.get(2)?;
            let filter = serde_json::from_str::<LibraryFilter>(&raw).unwrap_or_default();
            Ok(SavedFilter {
                id: r.get(0)?,
                name: r.get(1)?,
                filter,
                created_at: r.get(3)?,
                updated_at: r.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?;
    Ok(rows.filter_map(Result::ok).collect())
}

#[tauri::command]
pub fn save_filter(
    state: State<'_, AppState>,
    name: String,
    filter: LibraryFilter,
) -> Result<SavedFilter, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("保存视图名称不能为空".into());
    }
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let stamp = db::now();
    let raw = serde_json::to_string(&filter).map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO saved_filters(name,filter_json,created_at,updated_at) VALUES(?1,?2,?3,?3) ON CONFLICT(name) DO UPDATE SET filter_json=excluded.filter_json,updated_at=excluded.updated_at",
        params![name,raw,stamp]
    ).map_err(|e|e.to_string())?;
    let (id, created_at): (i64, i64) = conn
        .query_row(
            "SELECT id,created_at FROM saved_filters WHERE name=?1 COLLATE NOCASE",
            params![name],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|e| e.to_string())?;
    Ok(SavedFilter {
        id,
        name: name.to_string(),
        filter,
        created_at,
        updated_at: stamp,
    })
}

#[tauri::command]
pub fn delete_saved_filter(state: State<'_, AppState>, id: i64) -> Result<bool, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM saved_filters WHERE id=?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(true)
}

fn dir_size(path: &std::path::Path) -> u64 {
    WalkDir::new(path)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| e.metadata().ok().map(|m| m.len()))
        .sum()
}

#[tauri::command]
pub fn library_health(state: State<'_, AppState>) -> Result<LibraryHealth, String> {
    let (
        total,
        missing,
        duplicate_groups,
        without_metadata,
        without_fingerprint,
        pending_relations,
        unassigned_session,
    ) = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        let scalar = |sql: &str| conn.query_row(sql, [], |r| r.get::<_, i64>(0)).unwrap_or(0);
        (
            scalar("SELECT COUNT(*) FROM assets"),
            scalar("SELECT COUNT(*) FROM assets WHERE missing=1"),
            scalar("SELECT COUNT(*) FROM (SELECT fingerprint FROM assets WHERE fingerprint<>'' GROUP BY fingerprint HAVING COUNT(*)>1)"),
            scalar("SELECT COUNT(*) FROM assets WHERE metadata_type='none'"),
            scalar("SELECT COUNT(*) FROM assets WHERE fingerprint=''"),
            scalar("SELECT COUNT(*) FROM pending_relations"),
            scalar("SELECT COUNT(*) FROM assets a LEFT JOIN asset_sessions s ON s.asset_id=a.id WHERE s.asset_id IS NULL")
        )
    };
    Ok(LibraryHealth {
        total,
        missing,
        duplicate_groups,
        without_metadata,
        without_fingerprint,
        pending_relations,
        unassigned_session,
        cache_bytes: dir_size(&state.cache_dir),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("../schema.sql")).unwrap();
        conn
    }

    #[test]
    fn pending_relation_recovers_by_fingerprint() {
        let conn = conn();
        let stamp = db::now();
        conn.execute(
            "INSERT INTO assets(path,name,fingerprint,portable_id,created_at,updated_at) VALUES('parent.png','parent','same-fingerprint','il-local-parent',?1,?1)",
            params![stamp]
        ).unwrap();
        let parent_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO assets(path,name,fingerprint,portable_id,created_at,updated_at) VALUES('child.png','child','child-fingerprint','il-child',?1,?1)",
            params![stamp]
        ).unwrap();
        let child_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO pending_relations(child_portable_id,parent_portable_id,parent_fingerprint,relation_type,note,created_at) VALUES('il-child','il-foreign-parent','same-fingerprint','reference','portable fallback',?1)",
            params![stamp]
        ).unwrap();

        assert_eq!(resolve_pending_relations(&conn).unwrap(), 1);
        let relation: (i64, i64, String) = conn
            .query_row(
                "SELECT parent_id,child_id,note FROM relations LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(relation, (parent_id, child_id, "portable fallback".into()));
        let pending: i64 = conn
            .query_row("SELECT COUNT(*) FROM pending_relations", [], |r| r.get(0))
            .unwrap();
        assert_eq!(pending, 0);
    }
}
