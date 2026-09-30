use crate::{models::BackupRecord, state::AppState};
use rusqlite::{Connection, OpenFlags};
use std::{
    fs::{self, File, OpenOptions},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{Manager, State};

const MAX_BACKUPS: usize = 10;
const AUTO_INTERVAL: i64 = 24 * 3600;

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn now_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

fn sql_path(path: &Path) -> String {
    path.to_string_lossy().replace("'", "''")
}

fn sync_file(path: &Path) -> Result<(), String> {
    OpenOptions::new()
        .write(true)
        .open(path)
        .and_then(|file| file.sync_all())
        .map_err(|e| e.to_string())
}

fn unique_path(dir: &Path, prefix: &str) -> PathBuf {
    for suffix in 0..1000u16 {
        let token = now_nanos();
        let name = if suffix == 0 {
            format!("{}-{}.sqlite3", prefix, token)
        } else {
            format!("{}-{}-{}.sqlite3", prefix, token, suffix)
        };
        let candidate = dir.join(name);
        if !candidate.exists() {
            return candidate;
        }
    }
    dir.join(format!("{}-{}-fallback.sqlite3", prefix, now_nanos()))
}

fn validate(path: &Path) -> Result<(), String> {
    if companions(path)
        .iter()
        .skip(1)
        .any(|sidecar| sidecar.exists())
    {
        let probe_dir = copy_probe_files(path).map_err(|e| e.to_string())?;
        let result = validate_in_place(&probe_dir.join("library.sqlite3"));
        let _ = fs::remove_dir_all(probe_dir);
        return result;
    }
    validate_in_place(path)
}

fn validate_in_place(path: &Path) -> Result<(), String> {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| e.to_string())?;
    conn.busy_timeout(std::time::Duration::ZERO)
        .map_err(|e| e.to_string())?;
    let result: String = conn
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    if result != "ok" {
        return Err(format!("备份完整性检查失败：{}", result));
    }
    let version: String = conn
        .query_row(
            "SELECT value FROM app_meta WHERE key='schema_version'",
            [],
            |r| r.get(0),
        )
        .map_err(|_| "备份不是 ImageLore 资料库：缺少 schema 标记".to_string())?;
    let version = version
        .parse::<i64>()
        .ok()
        .filter(|v| (1..=crate::migrations::LATEST).contains(v))
        .ok_or_else(|| format!("备份数据库版本 {} 不受当前程序支持", version))?;
    // Historical databases have no application_id. Recognize their complete
    // business schema, rather than trusting a forgeable version marker alone.
    let mut tables: Vec<(&str, &[&str])> = vec![
        (
            "assets",
            &[
                "id",
                "path",
                "name",
                "favorite",
                "width",
                "height",
                "file_size",
                "format",
                "mime_type",
                "metadata_type",
                "generation_json",
                "fingerprint",
                "file_mtime",
                "missing",
                "created_at",
                "updated_at",
            ],
        ),
        (
            "prompt_state",
            &[
                "asset_id",
                "prompt",
                "negative_prompt",
                "model",
                "updated_at",
            ],
        ),
        ("tags", &["id", "name", "created_at"]),
        ("asset_tags", &["asset_id", "tag_id", "created_at"]),
        (
            "prompt_revisions",
            &[
                "id",
                "asset_id",
                "prompt",
                "negative_prompt",
                "model",
                "tags_json",
                "note",
                "created_at",
            ],
        ),
        (
            "relations",
            &[
                "id",
                "parent_id",
                "child_id",
                "relation_type",
                "note",
                "created_at",
            ],
        ),
        (
            "collections",
            &["id", "name", "description", "created_at", "updated_at"],
        ),
        (
            "collection_assets",
            &["collection_id", "asset_id", "created_at"],
        ),
        (
            "asset_search",
            &[
                "asset_id",
                "name",
                "prompt",
                "negative_prompt",
                "model",
                "tags",
            ],
        ),
    ];
    let mut relations = vec![
        ("prompt_state", "asset_id", "assets"),
        ("prompt_revisions", "asset_id", "assets"),
        ("asset_tags", "asset_id", "assets"),
        ("asset_tags", "tag_id", "tags"),
        ("relations", "parent_id", "assets"),
        ("relations", "child_id", "assets"),
        ("collection_assets", "asset_id", "assets"),
        ("collection_assets", "collection_id", "collections"),
    ];
    if version >= 3 {
        tables.extend([
            ("assets", &["portable_id"][..]),
            (
                "generation_sessions",
                &["id", "name", "note", "created_at", "updated_at"][..],
            ),
            (
                "asset_sessions",
                &["asset_id", "session_id", "note", "created_at", "updated_at"][..],
            ),
            (
                "model_aliases",
                &["alias", "canonical", "created_at", "updated_at"][..],
            ),
            (
                "saved_filters",
                &["id", "name", "filter_json", "created_at", "updated_at"][..],
            ),
            (
                "pending_relations",
                &[
                    "child_portable_id",
                    "parent_portable_id",
                    "parent_fingerprint",
                    "relation_type",
                    "note",
                    "created_at",
                ][..],
            ),
        ]);
        relations.extend([
            ("asset_sessions", "asset_id", "assets"),
            ("asset_sessions", "session_id", "generation_sessions"),
        ]);
        let duplicate: i64 = conn.query_row("SELECT COUNT(*) FROM (SELECT portable_id FROM assets WHERE portable_id<>'' GROUP BY portable_id HAVING COUNT(*)>1)", [], |r| r.get(0)).map_err(|e| e.to_string())?;
        if duplicate != 0 {
            return Err("备份存在重复 portable ID".into());
        }
    }
    if version >= 4 {
        tables.push((
            "source_folders",
            &[
                "id",
                "path",
                "name",
                "auto_sync",
                "last_scan_at",
                "created_at",
                "updated_at",
            ],
        ));
    }
    if version >= 5 {
        tables.push((
            "generation_index",
            &[
                "asset_id",
                "seed",
                "steps",
                "sampler",
                "scheduler",
                "cfg_scale",
                "denoise",
            ],
        ));
        relations.push(("generation_index", "asset_id", "assets"));
    }
    if version >= 6 {
        tables.push((
            "semantic_embeddings",
            &[
                "asset_id",
                "model_id",
                "dimensions",
                "vector",
                "fingerprint",
                "indexed_at",
            ],
        ));
        tables.push(("semantic_settings", &["key", "value"]));
        relations.push(("semantic_embeddings", "asset_id", "assets"));
    }
    if version >= 7 {
        tables.push(("asset_cjk_search", &["asset_id", "text"]));
    }
    if version >= 8 {
        tables.push((
            "visual_dna",
            &[
                "asset_id",
                "subject",
                "character_name",
                "outfit",
                "pose",
                "expression",
                "composition",
                "camera",
                "lighting",
                "environment",
                "palette",
                "material",
                "style",
                "search_text",
                "source",
                "updated_at",
            ],
        ));
        tables.push(("asset_search", &["visual_dna"]));
        relations.push(("visual_dna", "asset_id", "assets"));
    }
    if version >= 9 {
        tables.push((
            "vision_settings",
            &["id", "base_url", "model", "updated_at"],
        ));
        tables.push((
            "image_prompt_analyses",
            &[
                "id",
                "asset_id",
                "provider",
                "model",
                "summary",
                "prompt",
                "visual_dna_json",
                "created_at",
            ],
        ));
        relations.push(("image_prompt_analyses", "asset_id", "assets"));
    }
    if version >= 10 {
        tables.push((
            "remix_drafts",
            &["id", "base_asset_id", "prompt", "created_at", "updated_at"],
        ));
        tables.push((
            "remix_sources",
            &[
                "draft_id",
                "asset_id",
                "fields_json",
                "source_url",
                "reference_meta_json",
                "position",
                "created_at",
            ],
        ));
        relations.extend([
            ("remix_drafts", "base_asset_id", "assets"),
            ("remix_sources", "asset_id", "assets"),
            ("remix_sources", "draft_id", "remix_drafts"),
        ]);
    }
    if version >= 11 {
        tables.push((
            "reference_sources",
            &[
                "id",
                "asset_id",
                "source_url",
                "page_url",
                "page_title",
                "source_type",
                "metadata_json",
                "captured_at",
                "created_at",
                "updated_at",
            ],
        ));
        tables.push(("asset_search", &["reference"]));
        relations.push(("reference_sources", "asset_id", "assets"));
    }
    for (table, columns) in tables {
        let mut st = conn
            .prepare(&format!("PRAGMA table_info({})", table))
            .map_err(|e| e.to_string())?;
        let actual = st
            .query_map([], |r| r.get::<_, String>(1))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        if columns
            .iter()
            .any(|column| !actual.iter().any(|name| name == column))
        {
            return Err(format!("备份缺少 ImageLore {} 表的必要字段", table));
        }
    }
    let fk_error: bool = conn
        .prepare("PRAGMA foreign_key_check")
        .map_err(|e| e.to_string())?
        .exists([])
        .map_err(|e| e.to_string())?;
    if fk_error {
        return Err("备份外键检查失败".into());
    }
    // Also reject orphan business records in files whose FK declarations were removed.
    for (table, column, parent) in relations {
        let orphan: bool = conn.query_row(&format!("SELECT EXISTS(SELECT 1 FROM {table} c LEFT JOIN {parent} p ON p.id=c.{column} WHERE p.id IS NULL)"), [], |r| r.get(0)).map_err(|e| e.to_string())?;
        if orphan {
            return Err(format!("备份业务关系检查失败：{}.{}", table, column));
        }
    }
    Ok(())
}

fn records(dir: &Path) -> Result<Vec<BackupRecord>, String> {
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for entry in fs::read_dir(dir)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if path.extension().and_then(|x| x.to_str()) != Some("sqlite3") {
            continue;
        }
        let meta = entry.metadata().map_err(|e| e.to_string())?;
        let created = meta
            .modified()
            .ok()
            .and_then(|x| x.duration_since(UNIX_EPOCH).ok())
            .map(|x| x.as_secs() as i64)
            .unwrap_or(0);
        out.push(BackupRecord {
            name: path
                .file_name()
                .and_then(|x| x.to_str())
                .unwrap_or("backup.sqlite3")
                .to_string(),
            path: path.to_string_lossy().to_string(),
            size: meta.len(),
            created_at: created,
        });
    }
    out.sort_by(|a, b| {
        b.created_at
            .cmp(&a.created_at)
            .then_with(|| b.name.cmp(&a.name))
    });
    Ok(out)
}

fn rotate(dir: &Path) -> Result<(), String> {
    let mut valid_count = 0;
    for item in records(dir)? {
        if validate(Path::new(&item.path)).is_err() {
            // Invalid candidates remain available as evidence and cannot evict
            // usable recovery points or suppress the next automatic backup.
            let rejected = dir.join("rejected");
            fs::create_dir_all(&rejected).map_err(|e| e.to_string())?;
            let target = rejected.join(format!("{}-{}.raw", item.name, now_nanos()));
            fs::rename(&item.path, target).map_err(|e| e.to_string())?;
            continue;
        }
        valid_count += 1;
        if valid_count > MAX_BACKUPS {
            fs::remove_file(item.path).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn vacuum_snapshot(source: &Path, target: &Path) -> Result<(), String> {
    if companions(source)
        .iter()
        .skip(1)
        .any(|sidecar| sidecar.exists())
    {
        let probe_dir = copy_probe_files(source).map_err(|e| e.to_string())?;
        let result = vacuum_snapshot_in_place(&probe_dir.join("library.sqlite3"), target);
        let _ = fs::remove_dir_all(probe_dir);
        return result;
    }
    vacuum_snapshot_in_place(source, target)
}

fn vacuum_snapshot_in_place(source: &Path, target: &Path) -> Result<(), String> {
    let conn = Connection::open_with_flags(source, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| e.to_string())?;
    conn.busy_timeout(std::time::Duration::ZERO)
        .map_err(|e| e.to_string())?;
    // VACUUM INTO reads committed WAL pages directly; checkpointing a source can
    // mutate evidence and is unnecessary for a consistent standalone snapshot.
    conn.execute_batch(&format!("VACUUM INTO '{}';", sql_path(target)))
        .map_err(|e| e.to_string())?;
    drop(conn);
    validate(target)?;
    sync_file(target)
}

fn create(state: &AppState, prefix: &str) -> Result<BackupRecord, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    create_locked(&conn, state, prefix)
}

fn create_locked(
    conn: &Connection,
    state: &AppState,
    prefix: &str,
) -> Result<BackupRecord, String> {
    fs::create_dir_all(&state.backups_dir).map_err(|e| e.to_string())?;
    let path = unique_path(&state.backups_dir, prefix);
    let result = (|| {
        conn.execute_batch(&format!("VACUUM INTO '{}';", sql_path(&path)))
            .map_err(|e| e.to_string())?;
        validate(&path)?;
        sync_file(&path)?;
        Ok::<_, String>(())
    })();
    if let Err(error) = result {
        let _ = fs::remove_file(&path);
        return Err(error);
    }
    rotate(&state.backups_dir)?;
    records(&state.backups_dir)?
        .into_iter()
        .find(|x| Path::new(&x.path) == path)
        .ok_or("无法读取刚创建的备份".into())
}

fn ensure_auto_at(state: &AppState, timestamp: i64) -> Result<Option<BackupRecord>, String> {
    // Decision and snapshot share the write mutex, including concurrent timer,
    // focus and manual requests. Invalid files cannot defer a real backup.
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let latest = records(&state.backups_dir)?
        .into_iter()
        .find(|item| validate(Path::new(&item.path)).is_ok());
    if latest
        .as_ref()
        .map(|x| timestamp.saturating_sub(x.created_at) < AUTO_INTERVAL)
        .unwrap_or(false)
    {
        return Ok(None);
    }
    Ok(Some(create_locked(&conn, state, "auto")?))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RestoreStep {
    Staged,
    Preserved,
    Probed,
    Reserved,
    Installed,
}

fn companions(database: &Path) -> [PathBuf; 3] {
    [
        database.to_path_buf(),
        database.with_extension("sqlite3-wal"),
        database.with_extension("sqlite3-shm"),
    ]
}

fn preserve_originals(database: &Path, dir: &Path, prefix: &str) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let token = now_nanos();
    for (source, label) in companions(database).iter().zip(["sqlite3", "wal", "shm"]) {
        if source.exists() {
            let target = dir.join(format!("{}-{}.{}.raw", prefix, token, label));
            // Any failed forensic copy aborts before moving or replacing the active library.
            let mut output = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&target)
                .map_err(|e| format!("保全原件 {} 失败：{}", source.display(), e))?;
            let mut input = File::open(source).map_err(|e| e.to_string())?;
            std::io::copy(&mut input, &mut output).map_err(|e| e.to_string())?;
            output.sync_all().map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn rollback_dir(database: &Path) -> Result<PathBuf, String> {
    let parent = database.parent().ok_or("资料库路径缺少父目录")?;
    Ok(parent.join("restore.rollback"))
}

fn finish_or_rollback(database: &Path, data_dir: &Path) -> Result<(), String> {
    let dir = rollback_dir(database)?;
    if !dir.exists() {
        return Ok(());
    }
    // An interrupted older restore cannot downgrade a readable future library.
    if database.exists() {
        reject_unsupported_readable_schema(database)?;
    }
    let committed = dir.join("COMMITTED");
    if committed.exists() {
        let state = fs::read_to_string(&committed).map_err(|e| e.to_string())?;
        if (state == "pending" || state == "recovery") && validate(database).is_ok() {
            if state == "pending" {
                let pending = data_dir.join("restore.pending.sqlite3");
                if pending.exists() {
                    fs::remove_file(pending).map_err(|e| e.to_string())?;
                }
            }
            return fs::remove_dir_all(&dir).map_err(|e| e.to_string());
        }
        // A torn commit marker or damaged installation is still an interrupted
        // transaction. Keep the pending source and restore the reserved originals.
        fs::remove_file(committed).map_err(|e| e.to_string())?;
    }
    if dir.join("NO_DATABASE").exists() && database.exists() {
        let failed_dir = data_dir.join("recovery");
        fs::create_dir_all(&failed_dir).map_err(|e| e.to_string())?;
        fs::rename(
            database,
            failed_dir.join(format!("failed-install-{}.sqlite3.raw", now_nanos())),
        )
        .map_err(|e| e.to_string())?;
    }
    for (target, label) in companions(database).iter().zip(["database", "wal", "shm"]) {
        let original = dir.join(label);
        if original.exists() {
            // Never destroy either original or a partially installed candidate.
            if target.exists() {
                fs::rename(
                    target,
                    dir.join(format!("failed-{}-{}", label, now_nanos())),
                )
                .map_err(|e| format!("恢复回退无法预留 {}：{}", target.display(), e))?;
            }
            fs::rename(&original, target)
                .map_err(|e| format!("恢复原件回退失败：{}；原件位于 {}", e, original.display()))?;
        }
    }
    fs::remove_dir_all(&dir).map_err(|e| e.to_string())
}

fn copy_probe_files(database: &Path) -> std::io::Result<PathBuf> {
    let probe_dir = std::env::temp_dir().join(format!("imagelore-integrity-probe-{}", now_nanos()));
    let probe = probe_dir.join("library.sqlite3");
    let result = (|| {
        fs::create_dir(&probe_dir)?;
        // SHM is a derived WAL index and may contain active Windows byte-range
        // locks. Rebuild it in the disposable copy instead of copying a locked
        // index; DB + committed WAL carry all durable content.
        for (source, target) in companions(database).iter().take(2).zip(companions(&probe)) {
            if source.exists() {
                fs::copy(source, target)?;
            }
        }
        Ok::<_, std::io::Error>(())
    })();
    if let Err(error) = result {
        let _ = fs::remove_dir_all(&probe_dir);
        return Err(error);
    }
    Ok(probe_dir)
}

pub(crate) fn integrity_probe_copy(database: &Path) -> rusqlite::Result<String> {
    // READ_ONLY can rebuild SHM. Inspect disposable DB/WAL copies,
    // including committed WAL, without opening the original through SQLite.
    let probe_dir = copy_probe_files(database)
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
    let result = Connection::open_with_flags(
        probe_dir.join("library.sqlite3"),
        OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .and_then(|conn| {
        conn.busy_timeout(std::time::Duration::ZERO)?;
        conn.query_row("PRAGMA integrity_check(1)", [], |r| r.get::<_, String>(0))
    });
    let _ = fs::remove_dir_all(&probe_dir);
    result
}

fn integrity_is_corrupt(database: &Path) -> bool {
    match integrity_probe_copy(database) {
        Ok(status) => status != "ok",
        Err(rusqlite::Error::SqliteFailure(error, _)) => matches!(
            error.code,
            rusqlite::ErrorCode::DatabaseCorrupt | rusqlite::ErrorCode::NotADatabase
        ),
        Err(_) => false,
    }
}

fn reject_unsupported_readable_schema(database: &Path) -> Result<(), String> {
    // Integrity damage does not authorize a downgrade. Read the marker from a
    // disposable copy before deciding whether corrupt content can be replaced.
    let probe_dir = copy_probe_files(database).map_err(|e| e.to_string())?;
    let result = Connection::open_with_flags(
        probe_dir.join("library.sqlite3"),
        OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .and_then(|conn| {
        conn.busy_timeout(std::time::Duration::ZERO)?;
        conn.query_row(
            "SELECT value FROM app_meta WHERE key='schema_version'",
            [],
            |r| r.get::<_, String>(0),
        )
    });
    let _ = fs::remove_dir_all(probe_dir);
    match result {
        Ok(value)
            if value
                .parse::<i64>()
                .ok()
                .map(|v| (1..=crate::migrations::LATEST).contains(&v))
                .unwrap_or(false) =>
        {
            Ok(())
        }
        Ok(value) => Err(format!("活动资料库版本 {} 不受支持，拒绝恢复覆盖", value)),
        Err(rusqlite::Error::SqliteFailure(error, _))
            if matches!(
                error.code,
                rusqlite::ErrorCode::DatabaseCorrupt | rusqlite::ErrorCode::NotADatabase
            ) =>
        {
            Ok(())
        }
        Err(error) => Err(format!("无法复核活动资料库版本，拒绝恢复覆盖：{}", error)),
    }
}

fn known_sqlite_damage(error: &rusqlite::Error) -> bool {
    matches!(error, rusqlite::Error::SqliteFailure(code, _) if matches!(code.code,
        rusqlite::ErrorCode::DatabaseCorrupt | rusqlite::ErrorCode::NotADatabase))
}

fn supported_marker(value: &str) -> bool {
    value
        .parse::<i64>()
        .ok()
        .map(|v| (1..=crate::migrations::LATEST).contains(&v))
        .unwrap_or(false)
}

fn check_restore_write_access(database: &Path, allow_corrupt: bool) -> Result<bool, String> {
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // NOTADB can precede SQLite's locking attempt. Refuse outstanding
        // Windows handles even when SQLite cannot parse the damaged header.
        let _exclusive = OpenOptions::new()
            .read(true)
            .write(true)
            .share_mode(0)
            .open(database)
            .map_err(|e| e.to_string())?;
    }
    let conn = match Connection::open_with_flags(database, OpenFlags::SQLITE_OPEN_READ_WRITE) {
        Ok(conn) => conn,
        Err(error) if allow_corrupt && known_sqlite_damage(&error) => return Ok(true),
        Err(error) => return Err(error.to_string()),
    };
    conn.busy_timeout(std::time::Duration::ZERO)
        .map_err(|e| e.to_string())?;
    match conn.execute_batch("BEGIN IMMEDIATE;") {
        Ok(()) => (),
        Err(error) if allow_corrupt && known_sqlite_damage(&error) => return Ok(true),
        Err(error) => return Err(error.to_string()),
    }
    // Recheck the actual source under one write lock. A disposable copy may
    // have raced a WAL writer which exited before this connection acquired it.
    let result = (|| {
        match conn.query_row(
            "SELECT value FROM app_meta WHERE key='schema_version'",
            [],
            |r| r.get::<_, String>(0),
        ) {
            Ok(value) if supported_marker(&value) => (),
            Ok(value) => return Err(format!("活动资料库版本 {} 不受支持，拒绝恢复覆盖", value)),
            Err(error) if allow_corrupt && known_sqlite_damage(&error) => return Ok(true),
            Err(error) => return Err(error.to_string()),
        }
        match conn.query_row("PRAGMA integrity_check(1)", [], |r| r.get::<_, String>(0)) {
            Ok(status) => Ok(status != "ok"),
            Err(error) if allow_corrupt && known_sqlite_damage(&error) => Ok(true),
            Err(error) => Err(error.to_string()),
        }
    })();
    conn.execute_batch("ROLLBACK;").map_err(|e| e.to_string())?;
    result
}

fn replace_library(
    database: &Path,
    data_dir: &Path,
    source: &Path,
    preserve_dir: &Path,
    prefix: &str,
    is_pending: bool,
    hook: impl Fn(RestoreStep) -> Result<(), String>,
) -> Result<(), String> {
    finish_or_rollback(database, data_dir)?;
    validate(source)?;
    // Always stage on the destination filesystem. Snapshot includes any committed WAL.
    let parent = database.parent().ok_or("资料库路径缺少父目录")?;
    let staged = unique_path(parent, "restore.next");
    let reservation = rollback_dir(database)?;
    let result = (|| {
        vacuum_snapshot(source, &staged)?;
        hook(RestoreStep::Staged)?;
        preserve_originals(database, preserve_dir, prefix)?;
        hook(RestoreStep::Preserved)?;
        if database.exists() {
            reject_unsupported_readable_schema(database)?;
            let corrupt = integrity_is_corrupt(database);
            if !is_pending && !corrupt {
                return Err("自动恢复最终复核未确认损坏，已保留当前资料库与备份候选".into());
            }
            hook(RestoreStep::Probed)?;
            let actual_corrupt = check_restore_write_access(database, corrupt)?;
            if !is_pending && !actual_corrupt {
                return Err("自动恢复原库锁内复核未确认损坏，已保留当前资料库与备份候选".into());
            }
            if !actual_corrupt {
                validate(database)?;
                vacuum_snapshot(database, &unique_path(preserve_dir, "pre-restore"))?;
            }
        } else if !is_pending {
            return Err("自动恢复时活动资料库已不存在，已保留备份候选".into());
        }
        fs::create_dir(&reservation).map_err(|e| e.to_string())?;
        if !database.exists() {
            fs::write(reservation.join("NO_DATABASE"), b"").map_err(|e| e.to_string())?;
        }
        for (original, label) in companions(database).iter().zip(["database", "wal", "shm"]) {
            if original.exists() {
                fs::rename(original, reservation.join(label)).map_err(|e| e.to_string())?;
            }
        }
        hook(RestoreStep::Reserved)?;
        // No copy fallback over a live path: failed rename rolls the exact originals back.
        fs::rename(&staged, database).map_err(|e| e.to_string())?;
        hook(RestoreStep::Installed)?;
        validate(database)?;
        let commit = reservation.join("COMMITTED");
        fs::write(&commit, if is_pending { "pending" } else { "recovery" })
            .map_err(|e| e.to_string())?;
        sync_file(&commit)?;
        finish_or_rollback(database, data_dir)
    })();
    if let Err(error) = result {
        let rollback = finish_or_rollback(database, data_dir);
        let _ = fs::remove_file(&staged);
        return match rollback {
            Ok(()) => Err(error),
            Err(rollback_error) => Err(format!("{}；{}", error, rollback_error)),
        };
    }
    Ok(())
}

pub fn apply_pending_restore(
    database_path: &Path,
    data_dir: &Path,
    backups_dir: &Path,
) -> Result<(), String> {
    finish_or_rollback(database_path, data_dir)?;
    finish_pending_stage(data_dir)?;
    let pending = data_dir.join("restore.pending.sqlite3");
    if !pending.exists() {
        return Ok(());
    }
    replace_library(
        database_path,
        data_dir,
        &pending,
        backups_dir,
        "pre-restore",
        true,
        |_| Ok(()),
    )?;
    rotate(backups_dir)
}

fn finish_pending_stage(data_dir: &Path) -> Result<(), String> {
    let pending = data_dir.join("restore.pending.sqlite3");
    let previous = data_dir.join("restore.pending.previous.sqlite3");
    if !previous.exists() {
        return Ok(());
    }
    if !pending.exists() {
        return fs::rename(previous, pending).map_err(|e| e.to_string());
    }
    validate(&pending)?;
    fs::remove_file(previous).map_err(|e| e.to_string())
}

pub fn recover_latest_valid_backup(
    database_path: &Path,
    data_dir: &Path,
    backups_dir: &Path,
    reason: &str,
) -> Result<Option<BackupRecord>, String> {
    finish_or_rollback(database_path, data_dir)?;
    let selected = records(backups_dir)?
        .into_iter()
        .find(|item| validate(Path::new(&item.path)).is_ok());
    let Some(selected) = selected else {
        return Ok(None);
    };
    let recovery_dir = data_dir.join("recovery");
    replace_library(
        database_path,
        data_dir,
        Path::new(&selected.path),
        &recovery_dir,
        "startup-corrupt",
        false,
        |_| Ok(()),
    )?;
    let notice = format!(
        "time={}\nbackup={}\nreason={}\nrecovery_dir={}\n",
        now(),
        selected.name,
        reason.replace('\n', " | "),
        recovery_dir.to_string_lossy()
    );
    fs::write(crate::diagnostics::recovery_notice_path(data_dir), notice)
        .map_err(|e| e.to_string())?;
    rotate(backups_dir)?;
    Ok(Some(selected))
}

#[tauri::command]
pub fn create_backup(state: State<'_, AppState>) -> Result<BackupRecord, String> {
    create(state.inner(), "imagelore")
}

#[tauri::command]
pub async fn ensure_auto_backup(app: tauri::AppHandle) -> Result<Option<BackupRecord>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        ensure_auto_at(state.inner(), now())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn list_backups(state: State<'_, AppState>) -> Result<Vec<BackupRecord>, String> {
    records(&state.backups_dir)
}

#[tauri::command]
pub fn stage_restore(state: State<'_, AppState>, name: String) -> Result<bool, String> {
    let _lock = state.db.lock().map_err(|e| e.to_string())?;
    finish_pending_stage(&state.data_dir)?;
    let file = PathBuf::from(&name);
    let base = file
        .file_name()
        .and_then(|x| x.to_str())
        .ok_or("无效备份名称")?;
    let source = state.backups_dir.join(base);
    if !source.exists() {
        return Err("找不到该备份".into());
    }
    validate(&source)?;

    let pending = state.data_dir.join("restore.pending.sqlite3");
    let temp = unique_path(&state.data_dir, "restore.pending.next");
    vacuum_snapshot(&source, &temp)?;
    let previous = state.data_dir.join("restore.pending.previous.sqlite3");
    if pending.exists() {
        fs::rename(&pending, &previous).map_err(|e| e.to_string())?;
    }
    if let Err(error) = fs::rename(&temp, &pending) {
        if previous.exists() {
            fs::rename(&previous, &pending)
                .map_err(|e| format!("{}；暂存回退失败：{}", error, e))?;
        }
        return Err(error.to_string());
    }
    if previous.exists() {
        let _ = fs::remove_file(previous);
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backup_names_do_not_collide() {
        let root = std::env::temp_dir().join(format!("imagelore-backup-test-{}", now_nanos()));
        fs::create_dir_all(&root).unwrap();
        let first = unique_path(&root, "test");
        fs::write(&first, b"x").unwrap();
        let second = unique_path(&root, "test");
        assert_ne!(first, second);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn sql_path_escapes_single_quotes() {
        assert_eq!(
            sql_path(Path::new("C:/O'Brien/library.sqlite3")),
            "C:/O''Brien/library.sqlite3"
        );
    }

    #[test]
    fn valid_pending_backup_restores_over_corrupt_active_db() {
        let root = std::env::temp_dir().join(format!("imagelore-restore-test-{}", now_nanos()));
        let backups = root.join("backups");
        fs::create_dir_all(&backups).unwrap();
        let database = root.join("library.sqlite3");
        let pending = root.join("restore.pending.sqlite3");

        {
            let conn = Connection::open(&pending).unwrap();
            conn.execute_batch(include_str!("../schema.sql")).unwrap();
            conn.execute_batch(
                "CREATE TABLE marker(value TEXT); INSERT INTO marker(value) VALUES('restored');",
            )
            .unwrap();
        }
        fs::write(&database, b"this is not a sqlite database").unwrap();

        apply_pending_restore(&database, &root, &backups).unwrap();

        let conn = Connection::open(&database).unwrap();
        let value: String = conn
            .query_row("SELECT value FROM marker", [], |r| r.get(0))
            .unwrap();
        assert_eq!(value, "restored");
        assert!(!pending.exists());
        assert!(fs::read_dir(&backups)
            .unwrap()
            .filter_map(Result::ok)
            .any(|e| e.path().extension().and_then(|x| x.to_str()) == Some("raw")));
        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn startup_recovery_uses_valid_backup_and_preserves_corrupt_active() {
        let root =
            std::env::temp_dir().join(format!("imagelore-auto-recovery-test-{}", now_nanos()));
        let backups = root.join("backups");
        fs::create_dir_all(&backups).unwrap();
        let database = root.join("library.sqlite3");
        fs::write(&database, b"not a sqlite database").unwrap();

        let valid = backups.join("valid.sqlite3");
        {
            let conn = Connection::open(&valid).unwrap();
            conn.execute_batch(include_str!("../schema.sql")).unwrap();
            conn.execute_batch(
                "CREATE TABLE marker(value TEXT); INSERT INTO marker(value) VALUES('from-backup');",
            )
            .unwrap();
        }
        fs::write(backups.join("zz-new-corrupt.sqlite3"), b"broken backup").unwrap();
        assert_eq!(records(&backups).unwrap()[0].name, "zz-new-corrupt.sqlite3");

        let selected =
            recover_latest_valid_backup(&database, &root, &backups, "database open failed")
                .unwrap()
                .unwrap();
        assert_eq!(selected.name, "valid.sqlite3");

        let conn = Connection::open(&database).unwrap();
        let value: String = conn
            .query_row("SELECT value FROM marker", [], |r| r.get(0))
            .unwrap();
        assert_eq!(value, "from-backup");
        assert!(root.join("recovery-last.txt").exists());
        assert!(fs::read_dir(root.join("recovery"))
            .unwrap()
            .filter_map(Result::ok)
            .any(|e| e.path().extension().and_then(|x| x.to_str()) == Some("raw")));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn startup_recovery_returns_none_without_valid_backup() {
        let root = std::env::temp_dir().join(format!(
            "imagelore-auto-recovery-empty-test-{}",
            now_nanos()
        ));
        let backups = root.join("backups");
        fs::create_dir_all(&backups).unwrap();
        fs::write(backups.join("corrupt.sqlite3"), b"broken backup").unwrap();
        let result =
            recover_latest_valid_backup(&root.join("library.sqlite3"), &root, &backups, "failed")
                .unwrap();
        assert!(result.is_none());
        let _ = fs::remove_dir_all(root);
    }

    fn test_root(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("imagelore-{}-{}", label, now_nanos()));
        fs::create_dir_all(root.join("backups")).unwrap();
        root
    }

    fn seed(path: &Path, value: &str) -> Connection {
        let conn = Connection::open(path).unwrap();
        conn.execute_batch(include_str!("../schema.sql")).unwrap();
        conn.execute_batch("CREATE TABLE marker(value TEXT NOT NULL);")
            .unwrap();
        conn.execute("INSERT INTO marker VALUES(?1)", [value])
            .unwrap();
        conn.execute("INSERT INTO assets(id,path,name,portable_id,created_at,updated_at) VALUES(1,'synthetic/image.png','sentinel image','sentinel-id',1,1)", []).unwrap();
        conn.execute(
            "INSERT INTO prompt_state(asset_id,prompt,updated_at) VALUES(1,'sentinel prompt',1)",
            [],
        )
        .unwrap();
        conn
    }

    fn read_marker(path: &Path) -> String {
        let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        conn.query_row("SELECT value FROM marker", [], |r| r.get(0))
            .unwrap()
    }

    fn test_state(root: &Path) -> AppState {
        let database_path = root.join("library.sqlite3");
        let conn = seed(&database_path, "current");
        AppState {
            db: std::sync::Mutex::new(conn),
            data_dir: root.to_path_buf(),
            database_path,
            cache_dir: root.join("cache"),
            backups_dir: root.join("backups"),
            models_dir: root.join("models"),
            jobs: std::sync::Mutex::new(std::collections::HashMap::new()),
            next_job_id: std::sync::atomic::AtomicU64::new(1),
            vision_api_key: std::sync::Mutex::new(String::new()),
        }
    }

    #[test]
    fn readonly_validation_rejects_missing_empty_foreign_future_and_orphan_databases() {
        let root = test_root("validation");
        let missing = root.join("missing.sqlite3");
        assert!(validate(&missing).is_err());
        assert!(!missing.exists());
        for kind in ["empty", "foreign", "future", "orphan", "truncated"] {
            let path = root.join(format!("{}.sqlite3", kind));
            let conn = if kind == "empty" || kind == "foreign" {
                Connection::open(&path).unwrap()
            } else {
                seed(&path, kind)
            };
            match kind {
                "foreign" => conn.execute_batch("CREATE TABLE ordinary(value TEXT); INSERT INTO ordinary VALUES('ordinary');").unwrap(),
                "future" => { conn.execute("UPDATE app_meta SET value='12' WHERE key='schema_version'", []).unwrap(); },
                "orphan" => { conn.pragma_update(None, "foreign_keys", "OFF").unwrap(); conn.execute("INSERT INTO prompt_state(asset_id,prompt,updated_at) VALUES(999,'orphan',1)", []).unwrap(); },
                _ => (),
            }
            drop(conn);
            if kind == "truncated" {
                let bytes = fs::read(&path).unwrap();
                fs::write(&path, &bytes[..bytes.len() / 2]).unwrap();
            }
            let before = fs::read(&path).unwrap();
            assert!(validate(&path).is_err(), "{} must be rejected", kind);
            assert_eq!(fs::read(&path).unwrap(), before, "validation is read only");
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn invalid_pending_never_replaces_active_or_consumes_candidate() {
        for kind in ["empty", "foreign", "future", "orphan", "truncated"] {
            let root = test_root("invalid-pending");
            let database = root.join("library.sqlite3");
            drop(seed(&database, "original"));
            let pending = root.join("restore.pending.sqlite3");
            let conn = if kind == "empty" || kind == "foreign" {
                Connection::open(&pending).unwrap()
            } else {
                seed(&pending, kind)
            };
            match kind {
                "foreign" => conn
                    .execute_batch("CREATE TABLE ordinary(value TEXT);")
                    .unwrap(),
                "future" => {
                    conn.execute(
                        "UPDATE app_meta SET value='12' WHERE key='schema_version'",
                        [],
                    )
                    .unwrap();
                }
                "orphan" => {
                    conn.pragma_update(None, "foreign_keys", "OFF").unwrap();
                    conn.execute(
                        "INSERT INTO relations(parent_id,child_id,created_at) VALUES(1,999,1)",
                        [],
                    )
                    .unwrap();
                }
                _ => (),
            }
            drop(conn);
            if kind == "truncated" {
                fs::write(&pending, b"truncated").unwrap();
            }
            let before = fs::read(&database).unwrap();
            let candidate = fs::read(&pending).unwrap();
            assert!(apply_pending_restore(&database, &root, &root.join("backups")).is_err());
            assert_eq!(fs::read(&database).unwrap(), before);
            assert_eq!(fs::read(&pending).unwrap(), candidate);
            assert_eq!(read_marker(&database), "original");
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn snapshot_captures_committed_wal_without_checkpoint_and_restores_business_records() {
        let root = test_root("wal-restore");
        let state = test_state(&root);
        {
            let conn = state.db.lock().unwrap();
            conn.pragma_update(None, "journal_mode", "WAL").unwrap();
            conn.pragma_update(None, "wal_autocheckpoint", 0).unwrap();
            conn.execute("UPDATE marker SET value='wal-sentinel'", [])
                .unwrap();
            conn.execute(
                "UPDATE prompt_state SET prompt='wal-edit' WHERE asset_id=1",
                [],
            )
            .unwrap();
        }
        let wal = state.database_path.with_extension("sqlite3-wal");
        assert!(fs::metadata(&wal).unwrap().len() > 0);
        let backup = create(&state, "manual").unwrap();
        assert_eq!(read_marker(Path::new(&backup.path)), "wal-sentinel");
        state
            .db
            .lock()
            .unwrap()
            .execute("UPDATE marker SET value='post-backup'", [])
            .unwrap();
        let database = state.database_path.clone();
        vacuum_snapshot(
            Path::new(&backup.path),
            &root.join("restore.pending.sqlite3"),
        )
        .unwrap();
        drop(state);
        apply_pending_restore(&database, &root, &root.join("backups")).unwrap();
        assert_eq!(read_marker(&database), "wal-sentinel");
        let conn = crate::db::init_db(&database).unwrap();
        assert_eq!(
            conn.query_row(
                "SELECT prompt FROM prompt_state WHERE asset_id=1",
                [],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
            "wal-edit"
        );
        drop(conn);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn all_restore_failure_steps_keep_original_readable_and_pending_retryable() {
        for failpoint in [
            RestoreStep::Staged,
            RestoreStep::Preserved,
            RestoreStep::Probed,
            RestoreStep::Reserved,
            RestoreStep::Installed,
        ] {
            let root = test_root("restore-failure");
            let database = root.join("library.sqlite3");
            let pending = root.join("restore.pending.sqlite3");
            drop(seed(&database, "original"));
            drop(seed(&pending, "candidate"));
            let before = fs::read(&database).unwrap();
            let result = replace_library(
                &database,
                &root,
                &pending,
                &root.join("backups"),
                "fault-test",
                true,
                |step| {
                    if step == failpoint {
                        Err(format!("injected {:?} IO/rename/disk failure", step))
                    } else {
                        Ok(())
                    }
                },
            );
            assert!(result.unwrap_err().contains("injected"));
            assert_eq!(fs::read(&database).unwrap(), before, "{:?}", failpoint);
            assert_eq!(read_marker(&database), "original");
            assert_eq!(read_marker(&pending), "candidate");
            apply_pending_restore(&database, &root, &root.join("backups")).unwrap();
            assert_eq!(read_marker(&database), "candidate");
            assert!(!pending.exists());
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn interrupted_reservation_is_rolled_back_before_retrying_pending() {
        let root = test_root("interrupted-restore");
        let database = root.join("library.sqlite3");
        let pending = root.join("restore.pending.sqlite3");
        drop(seed(&database, "original"));
        drop(seed(&pending, "candidate"));
        let reservation = rollback_dir(&database).unwrap();
        fs::create_dir(&reservation).unwrap();
        fs::rename(&database, reservation.join("database")).unwrap();
        fs::copy(&pending, &database).unwrap(); // Process died after installation, before commit.
        finish_or_rollback(&database, &root).unwrap();
        assert_eq!(read_marker(&database), "original");
        assert_eq!(read_marker(&pending), "candidate");
        apply_pending_restore(&database, &root, &root.join("backups")).unwrap();
        assert_eq!(read_marker(&database), "candidate");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn corrupt_original_db_wal_shm_preservation_failure_aborts_replacement() {
        let root = test_root("raw-preservation-failure");
        let database = root.join("library.sqlite3");
        let pending = root.join("restore.pending.sqlite3");
        fs::write(&database, b"corrupt original sentinel").unwrap();
        fs::write(
            database.with_extension("sqlite3-wal"),
            b"corrupt wal sentinel",
        )
        .unwrap();
        fs::write(
            database.with_extension("sqlite3-shm"),
            b"corrupt shm sentinel",
        )
        .unwrap();
        drop(seed(&pending, "candidate"));
        let blocked = root.join("blocked-destination");
        fs::write(&blocked, b"file blocks directory creation").unwrap();
        let originals = companions(&database).map(|path| fs::read(path).unwrap());
        assert!(
            replace_library(&database, &root, &pending, &blocked, "fault", true, |_| Ok(
                ()
            ))
            .is_err()
        );
        for (path, bytes) in companions(&database).iter().zip(originals) {
            assert_eq!(fs::read(path).unwrap(), bytes);
        }
        assert!(pending.exists());
        apply_pending_restore(&database, &root, &root.join("backups")).unwrap();
        assert_eq!(read_marker(&database), "candidate");
        assert_eq!(
            fs::read_dir(root.join("backups"))
                .unwrap()
                .filter_map(Result::ok)
                .filter(|entry| entry.path().extension().and_then(|s| s.to_str()) == Some("raw"))
                .count(),
            3
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn locked_active_and_future_active_are_preserved_with_pending() {
        for future in [false, true] {
            let root = test_root("guard-active");
            let database = root.join("library.sqlite3");
            let pending = root.join("restore.pending.sqlite3");
            let conn = seed(&database, "original");
            drop(seed(&pending, "candidate"));
            if future {
                conn.execute(
                    "UPDATE app_meta SET value='12' WHERE key='schema_version'",
                    [],
                )
                .unwrap();
            } else {
                conn.execute_batch("BEGIN EXCLUSIVE;").unwrap();
            }
            assert!(apply_pending_restore(&database, &root, &root.join("backups")).is_err());
            if !future {
                conn.execute_batch("ROLLBACK;").unwrap();
            }
            drop(conn);
            assert_eq!(read_marker(&database), "original");
            assert_eq!(read_marker(&pending), "candidate");
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn active_wal_writer_prevents_restore_even_when_read_probes_succeed() {
        let root = test_root("wal-writer-guard");
        let database = root.join("library.sqlite3");
        let pending = root.join("restore.pending.sqlite3");
        let conn = seed(&database, "original");
        conn.pragma_update(None, "journal_mode", "WAL").unwrap();
        conn.execute_batch("BEGIN IMMEDIATE;").unwrap();
        drop(seed(&pending, "candidate"));
        validate(&database).unwrap();
        assert!(apply_pending_restore(&database, &root, &root.join("backups")).is_err());
        conn.execute_batch("ROLLBACK;").unwrap();
        drop(conn);
        assert_eq!(read_marker(&database), "original");
        assert_eq!(read_marker(&pending), "candidate");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn auto_backup_uses_valid_files_time_boundary_and_serializes_parallel_triggers() {
        let root = test_root("auto-concurrency");
        let state = std::sync::Arc::new(test_state(&root));
        fs::write(root.join("backups/zzz-broken.sqlite3"), b"new but invalid").unwrap();
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let state = std::sync::Arc::clone(&state);
                std::thread::spawn(move || ensure_auto_at(&state, now()).unwrap().is_some())
            })
            .collect();
        assert_eq!(
            handles
                .into_iter()
                .map(|handle| handle.join().unwrap())
                .filter(|created| *created)
                .count(),
            1
        );
        let created = records(&state.backups_dir)
            .unwrap()
            .into_iter()
            .find(|item| validate(Path::new(&item.path)).is_ok())
            .unwrap();
        assert!(
            ensure_auto_at(&state, created.created_at + AUTO_INTERVAL - 1)
                .unwrap()
                .is_none()
        );
        assert!(ensure_auto_at(&state, created.created_at + AUTO_INTERVAL)
            .unwrap()
            .is_some());
        let manual = create(&state, "manual").unwrap();
        assert!(Path::new(&manual.path).exists());
        for _ in 0..12 {
            create(&state, "manual").unwrap();
        }
        assert_eq!(records(&state.backups_dir).unwrap().len(), MAX_BACKUPS);
        assert!(records(&state.backups_dir)
            .unwrap()
            .iter()
            .all(|item| validate(Path::new(&item.path)).is_ok()));
        drop(state);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_auto_backup_can_retry_after_destination_is_repaired() {
        let root = test_root("auto-retry");
        let state = test_state(&root);
        fs::remove_dir(&state.backups_dir).unwrap();
        fs::write(&state.backups_dir, b"blocked").unwrap();
        assert!(ensure_auto_at(&state, now()).is_err());
        fs::remove_file(&state.backups_dir).unwrap();
        assert!(ensure_auto_at(&state, now()).unwrap().is_some());
        drop(state);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn every_supported_historical_schema_passes_backup_identity_validation() {
        let root = test_root("backup-historical");
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/migrations");
        for version in 1..=11 {
            let path = root.join(format!("v{}.sqlite3", version));
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                &fs::read_to_string(fixtures.join(format!("schema-{}.sql", version))).unwrap(),
            )
            .unwrap();
            // v2's untouched static source is correctly version 1 here. Its real
            // historical transition to 2 is exercised by the migration matrix.
            drop(conn);
            validate(&path).unwrap_or_else(|e| panic!("historical v{}: {}", version, e));
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn wal_candidate_validation_and_snapshot_never_rewrite_source_sidecars() {
        let root = test_root("candidate-readonly-wal");
        let source = root.join("source.sqlite3");
        let candidate = root.join("candidate.sqlite3");
        let snapshot = root.join("snapshot.sqlite3");
        let conn = seed(&source, "initial");
        conn.pragma_update(None, "journal_mode", "WAL").unwrap();
        conn.pragma_update(None, "wal_autocheckpoint", 0).unwrap();
        conn.execute("UPDATE marker SET value='committed-wal'", [])
            .unwrap();
        for (source, target) in companions(&source).iter().zip(companions(&candidate)) {
            if source.exists() {
                fs::copy(source, target).unwrap();
            }
        }
        fs::write(
            candidate.with_extension("sqlite3-shm"),
            b"invalid original shm index",
        )
        .unwrap();
        let originals = companions(&candidate).map(|path| fs::read(path).unwrap());
        validate(&candidate).unwrap();
        vacuum_snapshot(&candidate, &snapshot).unwrap();
        for (path, original) in companions(&candidate).iter().zip(originals) {
            assert!(
                fs::read(path).unwrap() == original,
                "candidate {} changed",
                path.display()
            );
        }
        assert_eq!(read_marker(&snapshot), "committed-wal");
        drop(conn);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failure_installing_first_library_quarantines_candidate_and_retries_pending() {
        let root = test_root("first-library-failure");
        let database = root.join("library.sqlite3");
        let pending = root.join("restore.pending.sqlite3");
        drop(seed(&pending, "candidate"));
        assert!(replace_library(
            &database,
            &root,
            &pending,
            &root.join("backups"),
            "first",
            true,
            |step| {
                if step == RestoreStep::Installed {
                    Err("injected failed first install".into())
                } else {
                    Ok(())
                }
            }
        )
        .is_err());
        assert!(!database.exists());
        assert_eq!(read_marker(&pending), "candidate");
        apply_pending_restore(&database, &root, &root.join("backups")).unwrap();
        assert_eq!(read_marker(&database), "candidate");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn interrupted_pending_stage_retains_previous_candidate() {
        let root = test_root("pending-stage-interrupt");
        let previous = root.join("restore.pending.previous.sqlite3");
        let pending = root.join("restore.pending.sqlite3");
        drop(seed(&previous, "previous-candidate"));
        finish_pending_stage(&root).unwrap();
        assert_eq!(read_marker(&pending), "previous-candidate");
        assert!(!previous.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn occupied_corrupt_database_is_never_replaced_even_when_sqlite_reports_notadb() {
        let root = test_root("occupied-corrupt-db");
        let database = root.join("library.sqlite3");
        let pending = root.join("restore.pending.sqlite3");
        let original = b"damaged and still occupied sentinel";
        fs::write(&database, original).unwrap();
        drop(seed(&pending, "candidate"));
        let occupied = File::open(&database).unwrap();
        assert!(integrity_is_corrupt(&database));
        assert!(apply_pending_restore(&database, &root, &root.join("backups")).is_err());
        assert_eq!(fs::read(&database).unwrap(), original);
        assert_eq!(read_marker(&pending), "candidate");
        drop(occupied);
        apply_pending_restore(&database, &root, &root.join("backups")).unwrap();
        assert_eq!(read_marker(&database), "candidate");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn automatic_recovery_recheck_keeps_a_library_repaired_since_initial_probe() {
        let root = test_root("auto-recovery-race");
        let database = root.join("library.sqlite3");
        let backup = root.join("backups/candidate.sqlite3");
        fs::write(&database, b"initial confirmed corruption").unwrap();
        drop(seed(&backup, "old-backup"));
        let original_backup = fs::read(&backup).unwrap();
        assert!(integrity_is_corrupt(&database));
        let result = replace_library(
            &database,
            &root,
            &backup,
            &root.join("recovery"),
            "race",
            false,
            |step| {
                if step == RestoreStep::Staged {
                    fs::remove_file(&database).unwrap();
                    let conn = seed(&database, "new-repaired-current");
                    conn.execute("INSERT INTO assets(id,path,name,portable_id,created_at,updated_at) VALUES(2,'synthetic/new.png','new-write','new-portable',2,2)", []).unwrap();
                }
                Ok(())
            },
        );
        assert!(result.unwrap_err().contains("最终复核"));
        assert_eq!(read_marker(&database), "new-repaired-current");
        let conn = Connection::open(&database).unwrap();
        assert_eq!(
            conn.query_row("SELECT name FROM assets WHERE id=2", [], |r| r
                .get::<_, String>(0))
                .unwrap(),
            "new-write"
        );
        drop(conn);
        assert_eq!(fs::read(&backup).unwrap(), original_backup);
        assert!(!rollback_dir(&database).unwrap().exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn readable_future_schema_cannot_be_recovered_even_when_integrity_is_bad() {
        let root = test_root("corrupt-future-schema");
        let database = root.join("library.sqlite3");
        let backup = root.join("backups/candidate.sqlite3");
        let conn = seed(&database, "future-original");
        conn.execute_batch("CREATE TABLE future_integrity_sentinel(value INTEGER); INSERT INTO future_integrity_sentinel VALUES(123); CREATE INDEX future_integrity_index ON future_integrity_sentinel(value); UPDATE app_meta SET value='12' WHERE key='schema_version';").unwrap();
        let page_size: usize = conn
            .query_row("PRAGMA page_size", [], |r| r.get(0))
            .unwrap();
        let root_page: usize = conn
            .query_row(
                "SELECT rootpage FROM sqlite_master WHERE name='future_integrity_index'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        drop(conn);
        // Change only the synthetic single-row index leaf payload. The table's
        // value and readable schema marker remain intact; readonly integrity
        // must report the index/table mismatch (CHECK errors are skipped in ro).
        let mut damaged = fs::read(&database).unwrap();
        let page = (root_page - 1) * page_size;
        assert_eq!(damaged[page], 10); // SQLite index leaf page.
        let cell = u16::from_be_bytes([damaged[page + 8], damaged[page + 9]]) as usize;
        assert_eq!(&damaged[page + cell..page + cell + 5], &[4, 3, 1, 9, 123]);
        damaged[page + cell + 4] = 124;
        fs::write(&database, damaged).unwrap();
        drop(seed(&backup, "old-backup"));
        assert!(integrity_is_corrupt(&database));
        let original = fs::read(&database).unwrap();
        let candidate = fs::read(&backup).unwrap();
        let error =
            recover_latest_valid_backup(&database, &root, &root.join("backups"), "bad integrity")
                .unwrap_err();
        assert!(error.contains("版本 12"));
        assert_eq!(fs::read(&database).unwrap(), original);
        assert_eq!(fs::read(&backup).unwrap(), candidate);
        assert_eq!(read_marker(&database), "future-original");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn locked_source_recheck_rejects_a_stale_corrupt_copy_after_writer_exit() {
        let root = test_root("auto-recovery-stale-probe");
        let database = root.join("library.sqlite3");
        let backup = root.join("backups/candidate.sqlite3");
        fs::write(&database, b"confirmed copied corruption").unwrap();
        drop(seed(&backup, "old-backup"));
        let candidate = fs::read(&backup).unwrap();
        let result = replace_library(
            &database,
            &root,
            &backup,
            &root.join("recovery"),
            "stale-probe",
            false,
            |step| {
                if step == RestoreStep::Probed {
                    fs::remove_file(&database).unwrap();
                    let conn = seed(&database, "repaired-after-copy-probe");
                    conn.execute("INSERT INTO assets(id,path,name,portable_id,created_at,updated_at) VALUES(2,'synthetic/latest.png','latest-write','latest-id',2,2)", []).unwrap();
                }
                Ok(())
            },
        );
        assert!(result.unwrap_err().contains("锁内复核"));
        assert_eq!(read_marker(&database), "repaired-after-copy-probe");
        let conn = Connection::open(&database).unwrap();
        assert_eq!(
            conn.query_row("SELECT name FROM assets WHERE id=2", [], |r| r
                .get::<_, String>(0))
                .unwrap(),
            "latest-write"
        );
        drop(conn);
        assert_eq!(fs::read(&backup).unwrap(), candidate);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn interrupted_restore_never_rolls_back_a_readable_future_current_library() {
        for committed in [false, true] {
            let root = test_root("future-interrupted-restore");
            let database = root.join("library.sqlite3");
            let reservation = rollback_dir(&database).unwrap();
            fs::create_dir(&reservation).unwrap();
            drop(seed(&reservation.join("database"), "reserved-old-library"));
            let current = seed(&database, "future-current");
            current
                .execute(
                    "UPDATE app_meta SET value='12' WHERE key='schema_version'",
                    [],
                )
                .unwrap();
            drop(current);
            if committed {
                fs::write(reservation.join("COMMITTED"), "recovery").unwrap();
            }
            let original = fs::read(&database).unwrap();
            let reserved = fs::read(reservation.join("database")).unwrap();
            assert!(finish_or_rollback(&database, &root)
                .unwrap_err()
                .contains("版本 12"));
            assert_eq!(fs::read(&database).unwrap(), original);
            assert_eq!(fs::read(reservation.join("database")).unwrap(), reserved);
            assert_eq!(reservation.join("COMMITTED").exists(), committed);
            assert_eq!(read_marker(&database), "future-current");
            fs::remove_dir_all(root).unwrap();
        }
    }
}
