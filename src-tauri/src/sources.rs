use crate::{
    db, importer,
    models::{ImportSummary, SourceFolder},
    state::AppState,
};
use rusqlite::{params, Connection, OptionalExtension};
use std::{collections::HashSet, path::PathBuf};
use tauri::{AppHandle, Manager, State};

fn row_source(row: &rusqlite::Row<'_>) -> rusqlite::Result<SourceFolder> {
    Ok(SourceFolder {
        id: row.get(0)?,
        path: row.get(1)?,
        name: row.get(2)?,
        auto_sync: row.get::<_, i64>(3)? != 0,
        last_scan_at: row.get(4)?,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
    })
}

pub(crate) fn list_from_conn(conn: &Connection) -> Result<Vec<SourceFolder>, String> {
    let mut st=conn.prepare(
        "SELECT id,path,name,auto_sync,last_scan_at,created_at,updated_at FROM source_folders ORDER BY name COLLATE NOCASE,path COLLATE NOCASE"
    ).map_err(|e|e.to_string())?;
    let rows = st.query_map([], row_source).map_err(|e| e.to_string())?;
    Ok(rows.filter_map(Result::ok).collect())
}

fn get_from_conn(conn: &Connection, id: i64) -> Result<SourceFolder, String> {
    conn.query_row(
        "SELECT id,path,name,auto_sync,last_scan_at,created_at,updated_at FROM source_folders WHERE id=?1",
        params![id],row_source
    ).map_err(|e|e.to_string())
}

#[tauri::command]
pub fn source_folders(state: State<'_, AppState>) -> Result<Vec<SourceFolder>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    list_from_conn(&conn)
}

#[tauri::command]
pub fn add_source_folder(state: State<'_, AppState>, path: String) -> Result<SourceFolder, String> {
    let raw = PathBuf::from(path.trim());
    if !raw.is_dir() {
        return Err("来源目录不存在或无法访问".into());
    }
    let canonical = raw.canonicalize().unwrap_or(raw);
    let path_text = canonical.to_string_lossy().to_string();
    let name = canonical
        .file_name()
        .and_then(|x| x.to_str())
        .filter(|x| !x.trim().is_empty())
        .unwrap_or(&path_text)
        .to_string();

    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let existing: Option<i64> = conn
        .query_row(
            "SELECT id FROM source_folders WHERE path=?1 COLLATE NOCASE",
            params![path_text],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    if let Some(id) = existing {
        return get_from_conn(&conn, id);
    }

    let stamp = db::now();
    conn.execute(
        "INSERT INTO source_folders(path,name,auto_sync,last_scan_at,created_at,updated_at) VALUES(?1,?2,1,0,?3,?3)",
        params![path_text,name,stamp]
    ).map_err(|e|e.to_string())?;
    get_from_conn(&conn, conn.last_insert_rowid())
}

#[tauri::command]
pub fn remove_source_folder(state: State<'_, AppState>, id: i64) -> Result<bool, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM source_folders WHERE id=?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(true)
}

#[tauri::command]
pub fn set_source_auto_sync(
    state: State<'_, AppState>,
    id: i64,
    enabled: bool,
) -> Result<SourceFolder, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE source_folders SET auto_sync=?1,updated_at=?2 WHERE id=?3",
        params![if enabled { 1 } else { 0 }, db::now(), id],
    )
    .map_err(|e| e.to_string())?;
    get_from_conn(&conn, id)
}

#[tauri::command]
pub fn start_sync_sources(app: AppHandle, ids: Vec<i64>) -> Result<u64, String> {
    if ids.is_empty() {
        return Err("没有可同步的来源目录".into());
    }
    let wanted: HashSet<i64> = ids.into_iter().collect();
    let state = app.state::<AppState>();
    let (roots, matched_ids) = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        let all = list_from_conn(&conn)?;
        let selected: Vec<SourceFolder> =
            all.into_iter().filter(|x| wanted.contains(&x.id)).collect();
        let roots = selected.iter().map(|x| x.path.clone()).collect::<Vec<_>>();
        let matched_ids = selected.iter().map(|x| x.id).collect::<Vec<_>>();
        (roots, matched_ids)
    };
    if roots.is_empty() {
        return Err("找不到要同步的来源目录".into());
    }

    importer::start_job_with_finish(app, roots, true, move |state, summary, cancelled| {
        let conn = state.db.lock().map_err(|error| error.to_string())?;
        record_completed_scan(&conn, &matched_ids, summary, cancelled, db::now())
    })
}

pub(crate) fn record_completed_scan(
    conn: &Connection,
    ids: &[i64],
    summary: &ImportSummary,
    cancelled: bool,
    stamp: i64,
) -> Result<(), String> {
    // The importer reports one batch summary. Until it can certify individual roots,
    // retain every previous successful scan timestamp if any part of the batch failed.
    if cancelled || summary.failed != 0 {
        return Ok(());
    }
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    for id in ids {
        tx.execute(
            "UPDATE source_folders SET last_scan_at=?1,updated_at=?1 WHERE id=?2",
            params![stamp, id],
        )
        .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_folder_rows_round_trip() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("../schema.sql")).unwrap();
        conn.execute(
            "INSERT INTO source_folders(path,name,auto_sync,last_scan_at,created_at,updated_at) VALUES('D:/AI','AI',1,42,1,2)",[]
        ).unwrap();
        let rows = list_from_conn(&conn).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].path, "D:/AI");
        assert!(rows[0].auto_sync);
        assert_eq!(rows[0].last_scan_at, 42);
    }

    #[test]
    fn failed_or_cancelled_batch_never_marks_any_root_successful() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("../schema.sql")).unwrap();
        for id in [1, 2] {
            conn.execute("INSERT INTO source_folders(id,path,name,auto_sync,last_scan_at,created_at,updated_at) VALUES(?1,?2,?2,1,42,1,42)", params![id, format!("synthetic-{id}")]).unwrap();
        }
        let mut summary = ImportSummary {
            added: 1,
            skipped: 0,
            duplicates: 0,
            failed: 1,
            last_id: Some(1),
        };
        record_completed_scan(&conn, &[1, 2], &summary, false, 100).unwrap();
        assert!(list_from_conn(&conn)
            .unwrap()
            .iter()
            .all(|source| source.last_scan_at == 42));
        summary.failed = 0;
        record_completed_scan(&conn, &[1, 2], &summary, true, 200).unwrap();
        assert!(list_from_conn(&conn)
            .unwrap()
            .iter()
            .all(|source| source.last_scan_at == 42));
        record_completed_scan(&conn, &[1, 2], &summary, false, 300).unwrap();
        assert!(list_from_conn(&conn)
            .unwrap()
            .iter()
            .all(|source| source.last_scan_at == 300));
        conn.execute_batch("CREATE TRIGGER synthetic_stamp_failure BEFORE UPDATE ON source_folders WHEN old.id=2 BEGIN SELECT RAISE(ABORT,'synthetic scan failure'); END").unwrap();
        assert!(record_completed_scan(&conn, &[1, 2], &summary, false, 400).is_err());
        assert!(list_from_conn(&conn)
            .unwrap()
            .iter()
            .all(|source| source.last_scan_at == 300));
    }
}
