use crate::{models::BackupRecord, state::AppState};
use rusqlite::{Connection, OpenFlags};
use std::{
    fs::{self, File, OpenOptions},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{Manager, State};

const MAX_BACKUPS: usize = 10;
const AUTO_INTERVAL: i64 = 24 * 3600;
// Full integrity checks revisit index pages. A bounded, connection-local cache
// avoids repeatedly reading them; it is released when the probe closes and
// does not alter the live library's cache or any persistent database setting.
const VALIDATION_CACHE_KIB: i64 = 32 * 1024;

// Test-only timing around the unchanged expression: no closure, duplicate
// evaluation, altered ?/return behavior, or production logging/clock access.
macro_rules! storage_phase {
    ($name:literal, $path:expr, $expression:expr, result) => {{
        #[cfg(test)]
        let mut storage_phase = tests::phase_profile::enter($name, $path);
        let outcome = $expression;
        #[cfg(test)]
        storage_phase.observe_result(outcome.is_ok());
        outcome
    }};
    ($name:literal, $path:expr, $expression:expr) => {{
        #[cfg(test)]
        let _storage_phase = tests::phase_profile::enter($name, $path);
        $expression
    }};
}

// Sibling DB initialization reuses the same opt-in, thread-local recorder.
#[cfg(test)]
pub(crate) type StorageProfileScope = tests::phase_profile::Scope;
#[cfg(test)]
pub(crate) fn storage_profile_span(
    phase: &'static str,
    path: Option<&Path>,
) -> StorageProfileScope {
    tests::phase_profile::enter(phase, path)
}

fn configure_validation(conn: &Connection) -> rusqlite::Result<()> {
    conn.busy_timeout(std::time::Duration::ZERO)?;
    conn.pragma_update(None, "cache_size", -VALIDATION_CACHE_KIB)
}

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
    storage_phase!(
        "file.sync",
        Some(path),
        OpenOptions::new()
            .write(true)
            .open(path)
            .and_then(|file| file.sync_all())
            .map_err(|e| e.to_string())
    )
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
    #[cfg(test)]
    let _validation = tests::phase_profile::enter("validation.total", Some(path));
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
    let conn = storage_phase!(
        "validation.open",
        None,
        Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
    )
    .map_err(|e| e.to_string())?;
    storage_phase!("validation.configure", None, configure_validation(&conn))
        .map_err(|e| e.to_string())?;
    let result: String = storage_phase!(
        "validation.integrity_check",
        None,
        conn.query_row("PRAGMA integrity_check", [], |r| r.get(0))
    )
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
        let duplicate: i64 = storage_phase!("validation.portable_ids", None, conn.query_row("SELECT COUNT(*) FROM (SELECT portable_id FROM assets WHERE portable_id<>'' GROUP BY portable_id HAVING COUNT(*)>1)", [], |r| r.get(0))).map_err(|e| e.to_string())?;
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
    storage_phase!("validation.schema_tables", None, {
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
    });
    let fk_error: bool = storage_phase!(
        "validation.foreign_keys",
        None,
        conn.prepare("PRAGMA foreign_key_check")
            .map_err(|e| e.to_string())?
            .exists([])
    )
    .map_err(|e| e.to_string())?;
    if fk_error {
        return Err("备份外键检查失败".into());
    }
    // Also reject orphan business records in files whose FK declarations were removed.
    storage_phase!("validation.business_relations", None, {
        for (table, column, parent) in relations {
            let orphan: bool = conn.query_row(&format!("SELECT EXISTS(SELECT 1 FROM {table} c LEFT JOIN {parent} p ON p.id=c.{column} WHERE p.id IS NULL)"), [], |r| r.get(0)).map_err(|e| e.to_string())?;
            if orphan {
                return Err(format!("备份业务关系检查失败：{}.{}", table, column));
            }
        }
    });
    Ok(())
}

fn records(dir: &Path) -> Result<Vec<BackupRecord>, String> {
    #[cfg(test)]
    let _records = tests::phase_profile::enter("records.enumerate", None);
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
    #[cfg(test)]
    let _rotation = tests::phase_profile::enter("rotation.total", None);
    let mut valid_count = 0;
    for item in records(dir)? {
        if storage_phase!(
            "rotation.candidate",
            Some(Path::new(&item.path)),
            validate(Path::new(&item.path))
        )
        .is_err()
        {
            // Invalid candidates remain available as evidence and cannot evict
            // usable recovery points or suppress the next automatic backup.
            let rejected = dir.join("rejected");
            fs::create_dir_all(&rejected).map_err(|e| e.to_string())?;
            let target = rejected.join(format!("{}-{}.raw", item.name, now_nanos()));
            storage_phase!("rotation.quarantine", None, fs::rename(&item.path, target))
                .map_err(|e| e.to_string())?;
            continue;
        }
        valid_count += 1;
        if valid_count > MAX_BACKUPS {
            storage_phase!("rotation.remove", None, fs::remove_file(item.path))
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn vacuum_snapshot(source: &Path, target: &Path) -> Result<(), String> {
    #[cfg(test)]
    let _snapshot = tests::phase_profile::enter("snapshot.total", Some(source));
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
    storage_phase!(
        "snapshot.vacuum_into",
        None,
        conn.execute_batch(&format!("VACUUM INTO '{}';", sql_path(target)))
    )
    .map_err(|e| e.to_string())?;
    drop(conn);
    validate(target)?;
    sync_file(target)
}

fn create(state: &AppState, prefix: &str) -> Result<BackupRecord, String> {
    let _operation = storage_phase!("backup.mutex_wait", None, state.backup_operation.lock())
        .map_err(|e| e.to_string())?;
    create_locked(state, prefix)
}

// Caller holds backup_operation throughout the decision, snapshot, validation
// and rotation. Only VACUUM needs the live library connection.
fn create_locked(state: &AppState, prefix: &str) -> Result<BackupRecord, String> {
    fs::create_dir_all(&state.backups_dir).map_err(|e| e.to_string())?;
    let path = unique_path(&state.backups_dir, prefix);
    let result = (|| {
        {
            let conn = storage_phase!("backup.db_mutex_wait", None, state.db.lock())
                .map_err(|e| e.to_string())?;
            storage_phase!(
                "backup.vacuum_into",
                None,
                conn.execute_batch(&format!("VACUUM INTO '{}';", sql_path(&path)))
            )
            .map_err(|e| e.to_string())?;
        }
        storage_phase!("backup.new_validation", Some(&path), validate(&path))?;
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
    // Decision and snapshot share the backup operation mutex, including
    // concurrent timer, focus and manual requests. Validate every candidate
    // again without blocking the live library; invalid files cannot defer a
    // real backup. All paths acquire backup_operation before db.
    let _operation = storage_phase!("auto.mutex_wait", None, state.backup_operation.lock())
        .map_err(|e| e.to_string())?;
    let latest = records(&state.backups_dir)?.into_iter().find(|item| {
        storage_phase!(
            "auto.candidate",
            Some(Path::new(&item.path)),
            validate(Path::new(&item.path))
        )
        .is_ok()
    });
    if latest
        .as_ref()
        .map(|x| timestamp.saturating_sub(x.created_at) < AUTO_INTERVAL)
        .unwrap_or(false)
    {
        return Ok(None);
    }
    Ok(Some(create_locked(state, "auto")?))
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
    #[cfg(test)]
    let _preserve = tests::phase_profile::enter("restore.preserve_originals", Some(database));
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
            storage_phase!(
                "restore.preserve_copy",
                Some(source),
                std::io::copy(&mut input, &mut output)
            )
            .map_err(|e| e.to_string())?;
            storage_phase!("restore.preserve_sync", None, output.sync_all())
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn rollback_dir(database: &Path) -> Result<PathBuf, String> {
    let parent = database.parent().ok_or("资料库路径缺少父目录")?;
    Ok(parent.join("restore.rollback"))
}

fn finish_or_rollback(database: &Path, data_dir: &Path) -> Result<(), String> {
    #[cfg(test)]
    let _finish = tests::phase_profile::enter("restore.finish_or_rollback", None);
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
    #[cfg(test)]
    let _copy = tests::phase_profile::enter("probe.copy_db_wal", Some(database));
    static NEXT_PROBE: AtomicU64 = AtomicU64::new(0);
    for _ in 0..8 {
        let sequence = NEXT_PROBE.fetch_add(1, Ordering::Relaxed);
        let probe_dir = std::env::temp_dir().join(format!(
            "imagelore-integrity-probe-{}-{}-{}",
            std::process::id(),
            now_nanos(),
            sequence
        ));
        match copy_probe_files_into(database, probe_dir) {
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            result => return result,
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "无法创建独立的完整性检查临时目录",
    ))
}

fn copy_probe_files_into(database: &Path, probe_dir: PathBuf) -> std::io::Result<PathBuf> {
    let probe = probe_dir.join("library.sqlite3");
    // Only a successful atomic create gives this call ownership. If another
    // probe already occupies the path, return without cleaning up its files.
    fs::create_dir(&probe_dir)?;
    let result = (|| {
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
    #[cfg(test)]
    let _probe = tests::phase_profile::enter("probe.integrity_total", Some(database));
    // READ_ONLY can rebuild SHM. Inspect disposable DB/WAL copies,
    // including committed WAL, without opening the original through SQLite.
    let probe_dir = copy_probe_files(database)
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
    let opened = storage_phase!(
        "probe.open",
        None,
        Connection::open_with_flags(
            probe_dir.join("library.sqlite3"),
            OpenFlags::SQLITE_OPEN_READ_ONLY,
        ),
        result
    );
    let result = opened.and_then(|conn| {
        configure_validation(&conn)?;
        storage_phase!(
            "probe.integrity_check",
            None,
            conn.query_row("PRAGMA integrity_check(1)", [], |r| r.get::<_, String>(0))
        )
    });
    let _ = storage_phase!(
        "probe.cleanup",
        None,
        fs::remove_dir_all(&probe_dir),
        result
    );
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
    #[cfg(test)]
    let _schema = tests::phase_profile::enter("restore.schema_probe", Some(database));
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
    #[cfg(test)]
    let _access = tests::phase_profile::enter("restore.write_access", Some(database));
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
        // cache_size can read a damaged schema. Preserve the same corruption
        // classification as the actual integrity probe; neither cache setup
        // nor a permission/locking failure can authorize replacement.
        match conn.pragma_update(None, "cache_size", -VALIDATION_CACHE_KIB) {
            Ok(()) => (),
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
    #[cfg(test)]
    let _replace = tests::phase_profile::enter("restore.replace_total", None);
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
            let corrupt = storage_phase!(
                "restore.active_health_probe",
                None,
                integrity_is_corrupt(database)
            );
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
        storage_phase!("restore.reserve_originals", None, {
            for (original, label) in companions(database).iter().zip(["database", "wal", "shm"]) {
                if original.exists() {
                    fs::rename(original, reservation.join(label)).map_err(|e| e.to_string())?;
                }
            }
        });
        hook(RestoreStep::Reserved)?;
        // No copy fallback over a live path: failed rename rolls the exact originals back.
        storage_phase!(
            "restore.install_rename",
            None,
            fs::rename(&staged, database)
        )
        .map_err(|e| e.to_string())?;
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
    #[cfg(test)]
    let _apply = tests::phase_profile::enter("restore.apply_pending_total", None);
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
pub async fn create_backup(app: tauri::AppHandle) -> Result<BackupRecord, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        create(state.inner(), "imagelore")
    })
    .await
    .map_err(|e| e.to_string())?
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
pub async fn stage_restore(app: tauri::AppHandle, name: String) -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        stage_restore_at(state.inner(), &name)
    })
    .await
    .map_err(|e| e.to_string())?
}

fn stage_restore_at(state: &AppState, name: &str) -> Result<bool, String> {
    // Protect candidates and pending files from creation/rotation. Staging
    // reads a backup, so it does not need the active library connection.
    let _operation = storage_phase!("stage.mutex_wait", None, state.backup_operation.lock())
        .map_err(|e| e.to_string())?;
    finish_pending_stage(&state.data_dir)?;
    let file = PathBuf::from(name);
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
    if let Err(error) = storage_phase!("stage.install_pending", None, fs::rename(&temp, &pending)) {
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

    // The production spans are inert unless this one thread explicitly captures
    // an ignored profile operation. No global environment switch enables them.
    pub(super) mod phase_profile {
        use serde_json::{json, Value};
        use std::{
            cell::RefCell,
            collections::BTreeMap,
            path::{Path, PathBuf},
            time::Instant,
        };

        struct Session {
            origin: Instant,
            next_id: u64,
            stack: Vec<u64>,
            files: BTreeMap<PathBuf, u64>,
            events: Vec<Value>,
        }
        thread_local! { static CURRENT: RefCell<Option<Session>> = const { RefCell::new(None) }; }
        struct ActiveScope {
            id: u64,
            parent: Option<u64>,
            phase: &'static str,
            started: Instant,
            started_ms: f64,
            ordinal: Option<u64>,
            bytes: Option<u64>,
            result_ok: Option<bool>,
        }
        pub(crate) struct Scope(Option<ActiveScope>);
        impl Scope {
            pub(crate) fn observe_result(&mut self, success: bool) {
                if let Some(active) = self.0.as_mut() {
                    active.result_ok = Some(success);
                }
            }
        }
        pub(crate) fn enter(phase: &'static str, path: Option<&Path>) -> Scope {
            CURRENT.with(|slot| {
                let Ok(mut current) = slot.try_borrow_mut() else {
                    return Scope(None);
                };
                let Some(session) = current.as_mut() else {
                    return Scope(None);
                };
                let ordinal = path.map(|path| {
                    let next = session.files.len() as u64 + 1;
                    *session.files.entry(path.to_path_buf()).or_insert(next)
                });
                let bytes = path
                    .and_then(|path| std::fs::metadata(path).ok())
                    .map(|m| m.len());
                session.next_id += 1;
                let id = session.next_id;
                let parent = session.stack.last().copied();
                session.stack.push(id);
                Scope(Some(ActiveScope {
                    id,
                    parent,
                    phase,
                    started: Instant::now(),
                    started_ms: session.origin.elapsed().as_secs_f64() * 1000.0,
                    ordinal,
                    bytes,
                    result_ok: None,
                }))
            })
        }
        impl Drop for Scope {
            fn drop(&mut self) {
                let Some(ActiveScope {
                    id,
                    parent,
                    phase,
                    started,
                    started_ms,
                    ordinal,
                    bytes,
                    result_ok,
                }) = self.0.take()
                else {
                    return;
                };
                let duration_ms = started.elapsed().as_secs_f64() * 1000.0;
                CURRENT.with(|slot| {
                    let Ok(mut current) = slot.try_borrow_mut() else { return; };
                    let Some(session) = current.as_mut() else { return; };
                    if session.stack.last() == Some(&id) { session.stack.pop(); }
                    session.events.push(json!({"id":id,"parentId":parent,"phase":phase,"thread":format!("{:?}",std::thread::current().id()),"startedMs":started_ms,"endedMs":started_ms+duration_ms,"durationMs":duration_ms,"fileOrdinal":ordinal,"bytes":bytes,"resultOk":result_ok,"panicking":std::thread::panicking(),"endReason":"scope_drop_not_a_success_assertion"}));
                });
            }
        }
        pub(super) fn capture<T>(
            operation: &'static str,
            action: impl FnOnce() -> Result<T, String>,
        ) -> (Result<T, String>, Vec<Value>) {
            CURRENT.with(|slot| {
                let mut current = slot.borrow_mut();
                assert!(current.is_none(), "Nested capture is unsupported");
                *current = Some(Session {
                    origin: Instant::now(),
                    next_id: 0,
                    stack: Vec::new(),
                    files: BTreeMap::new(),
                    events: Vec::new(),
                });
            });
            let mut total = enter(operation, None);
            let result = action();
            total.observe_result(result.is_ok());
            drop(total);
            let mut events = CURRENT.with(|slot| slot.borrow_mut().take().unwrap().events);
            let child_sums: BTreeMap<u64, f64> = events
                .iter()
                .filter_map(|event| {
                    event["parentId"]
                        .as_u64()
                        .map(|id| (id, event["durationMs"].as_f64().unwrap()))
                })
                .fold(BTreeMap::new(), |mut sums, (id, ms)| {
                    *sums.entry(id).or_default() += ms;
                    sums
                });
            for event in &mut events {
                let inclusive = event["durationMs"].as_f64().unwrap();
                let children = child_sums
                    .get(&event["id"].as_u64().unwrap())
                    .copied()
                    .unwrap_or(0.0);
                event["inclusiveMs"] = json!(inclusive);
                event["exclusiveMs"] = json!((inclusive - children).max(0.0));
            }
            (result, events)
        }
    }

    fn profile_plain_path(path: &Path) {
        let metadata = fs::symlink_metadata(path).unwrap();
        assert!(
            !metadata.file_type().is_symlink(),
            "Profile fixture symlinks are forbidden"
        );
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            assert_eq!(
                metadata.file_attributes() & 0x400,
                0,
                "Profile fixture reparse points are forbidden"
            );
        }
    }

    // Type and length encoding plus sorted row hashes bind all logical rows,
    // including vector BLOBs and virtual FTS rows, while excluding only SQLite
    // internals and physical shadow tables. VACUUM may change file bytes.
    fn profile_business(path: &Path) -> std::collections::BTreeMap<String, (usize, String)> {
        use rusqlite::types::ValueRef;
        use sha2::{Digest, Sha256};
        let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        let mut table_list = conn.prepare("PRAGMA table_list").unwrap();
        let tables = table_list
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let mut business = std::collections::BTreeMap::new();
        for (schema, table, kind) in tables {
            if schema != "main" || table.starts_with("sqlite_") || kind == "shadow" {
                continue;
            }
            let escaped = table.replace('"', "\"\"");
            let mut statement = conn
                .prepare(&format!("SELECT * FROM \"{}\"", escaped))
                .unwrap();
            let columns = statement.column_count();
            let mut rows = statement.query([]).unwrap();
            let mut row_hashes = Vec::<[u8; 32]>::new();
            while let Some(row) = rows.next().unwrap() {
                let mut hash = Sha256::new();
                hash.update((columns as u64).to_le_bytes());
                for column in 0..columns {
                    match row.get_ref(column).unwrap() {
                        ValueRef::Null => hash.update([0]),
                        ValueRef::Integer(value) => {
                            hash.update([1]);
                            hash.update(value.to_le_bytes());
                        }
                        ValueRef::Real(value) => {
                            hash.update([2]);
                            hash.update(value.to_bits().to_le_bytes());
                        }
                        ValueRef::Text(value) => {
                            hash.update([3]);
                            hash.update((value.len() as u64).to_le_bytes());
                            hash.update(value);
                        }
                        ValueRef::Blob(value) => {
                            hash.update([4]);
                            hash.update((value.len() as u64).to_le_bytes());
                            hash.update(value);
                        }
                    }
                }
                row_hashes.push(hash.finalize().into());
            }
            row_hashes.sort_unstable();
            let mut hash = Sha256::new();
            for row in &row_hashes {
                hash.update(row);
            }
            business.insert(table, (row_hashes.len(), format!("{:x}", hash.finalize())));
        }
        business
    }

    fn profile_operation<T>(
        root: &Path,
        operation: &'static str,
        action: impl FnOnce() -> Result<T, String>,
    ) -> T {
        use std::io::Write;
        let (result, events) = phase_profile::capture(operation, action);
        let validations = events
            .iter()
            .filter(|event| event["phase"] == "validation.total")
            .count();
        let rotation_candidates = events
            .iter()
            .filter(|event| event["phase"] == "rotation.candidate")
            .count();
        let report = serde_json::json!({"operation":operation,"ok":result.is_ok(),"fullValidationCalls":validations,"rotationCandidates":rotation_candidates,"cacheKiB":VALIDATION_CACHE_KIB,"timing":"inclusive nested spans; do not sum parents and children","events":events});
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(root.join(format!("{}.json", operation)))
            .unwrap();
        serde_json::to_writer_pretty(&mut output, &report).unwrap();
        output.flush().unwrap();
        println!(
            "{}",
            serde_json::json!({"operation":operation,"ok":result.is_ok(),"fullValidationCalls":validations,"rotationCandidates":rotation_candidates})
        );
        result.unwrap_or_else(|error| panic!("Profile operation {} failed: {}", operation, error))
    }

    fn profile_file_sha256(path: &Path) -> String {
        use sha2::{Digest, Sha256};
        use std::io::Read;
        let mut input = File::open(path).unwrap();
        let mut hash = Sha256::new();
        let mut buffer = vec![0_u8; 64 * 1024];
        loop {
            let count = input.read(&mut buffer).unwrap();
            if count == 0 {
                break;
            }
            hash.update(&buffer[..count]);
        }
        format!("{:x}", hash.finalize()).to_ascii_uppercase()
    }

    // Manifest v2 preserves a closed source's raw DB/WAL/SHM bytes separately
    // from its clone-only checkpoint. This guard never opens the source DB.
    fn profile_v2_bundle_guard(root: &Path, manifest: &serde_json::Value) {
        use sha2::{Digest, Sha256};
        assert_eq!(manifest["sourceBeforeAfterHashesPreserved"], true);
        assert_eq!(manifest["cloneSidecarsAbsent"], true);
        profile_plain_path(&root.join("raw-source-bundle"));
        profile_plain_path(&root.join("raw-source-bundle/backups"));
        profile_plain_path(&root.join(".clone-checkpoint-started"));
        let mut originals = std::collections::BTreeMap::new();
        for file in manifest["sourceBundle"].as_array().unwrap() {
            let name = file["relativePath"].as_str().unwrap();
            let relative = Path::new(name);
            assert!(name.is_ascii() && !relative.is_absolute());
            assert!(relative
                .components()
                .all(|part| matches!(part, std::path::Component::Normal(_))));
            assert!(
                matches!(
                    name,
                    "library.sqlite3" | "library.sqlite3-wal" | "library.sqlite3-shm"
                ) || (relative.parent() == Some(Path::new("backups"))
                    && relative
                        .extension()
                        .and_then(|extension| extension.to_str())
                        == Some("sqlite3"))
            );
            assert_eq!(file["rawRelativePath"], format!("raw-source-bundle/{name}"));
            let before = file["sourceSha256Before"].as_str().unwrap();
            assert_eq!(before.len(), 64);
            assert!(before.bytes().all(|byte| byte.is_ascii_hexdigit()));
            assert_eq!(before, before.to_ascii_uppercase());
            assert_eq!(file["sourceSha256After"], before);
            assert_eq!(file["rawSha256"], before);
            let modified = file["sourceModifiedUtcBefore"].as_str().unwrap();
            assert!(!modified.is_empty());
            assert_eq!(file["sourceModifiedUtcAfter"], modified);
            assert_eq!(file["copiedFromRetainedReadHandle"], true);
            let raw = root.join("raw-source-bundle").join(relative);
            profile_plain_path(&raw);
            assert_eq!(
                fs::metadata(&raw).unwrap().len(),
                file["bytes"].as_u64().unwrap()
            );
            assert_eq!(profile_file_sha256(&raw), before);
            assert!(originals.insert(name.to_string(), file).is_none());
        }
        assert!((2..=13).contains(&originals.len()));
        assert!(originals.contains_key("library.sqlite3"));
        let source_sidecars_absent = !originals.contains_key("library.sqlite3-wal")
            && !originals.contains_key("library.sqlite3-shm");
        assert_eq!(
            manifest["sourceSidecarsAbsent"].as_bool().unwrap(),
            source_sidecars_absent
        );
        let mut bundle = Sha256::new();
        for (name, file) in &originals {
            bundle.update((name.len() as u64).to_le_bytes());
            bundle.update(name.as_bytes());
            bundle.update(file["bytes"].as_u64().unwrap().to_le_bytes());
            bundle.update(file["sourceSha256Before"].as_str().unwrap().as_bytes());
        }
        assert_eq!(
            manifest["sourceBundleSha256"],
            format!("{:x}", bundle.finalize()).to_ascii_uppercase()
        );
        let mut clones = std::collections::BTreeSet::new();
        for file in manifest["cloneFiles"].as_array().unwrap() {
            let name = file["relativePath"].as_str().unwrap();
            assert!(clones.insert(name));
            let original = originals.get(name).unwrap();
            assert_eq!(file["checkpointApplied"], name == "library.sqlite3");
            if name != "library.sqlite3" {
                assert_eq!(file["cloneSha256"], original["sourceSha256Before"]);
                assert_eq!(file["bytes"], original["bytes"]);
            }
            assert!(!root.join(format!("{name}-journal")).exists());
        }
        let expected: std::collections::BTreeSet<_> = originals
            .keys()
            .filter(|name| !matches!(name.as_str(), "library.sqlite3-wal" | "library.sqlite3-shm"))
            .map(String::as_str)
            .collect();
        assert_eq!(clones, expected);
        let checkpoint = &manifest["cloneCheckpoint"];
        assert_eq!(
            checkpoint["reportRelativePath"],
            "clone-checkpoint-result.json"
        );
        let report_path = root.join("clone-checkpoint-result.json");
        profile_plain_path(&report_path);
        assert_eq!(
            profile_file_sha256(&report_path),
            checkpoint["reportSha256"].as_str().unwrap()
        );
        let report: serde_json::Value =
            serde_json::from_slice(&fs::read(report_path).unwrap()).unwrap();
        assert_eq!(report["state"], "PASS_CLONE_ONLY_CHECKPOINT_VALIDATED");
        assert_eq!(report["sourceSQLiteOpened"], false);
        assert_eq!(report["rawLibraryUnchanged"], true);
        assert_eq!(report["cloneSidecarsAbsent"], true);
        assert_eq!(report["checkpointResult"], serde_json::json!([0, 0, 0]));
        assert_eq!(report["before"], report["after"]);
        assert_eq!(report["before"]["schemaVersion"], 11);
        assert_eq!(report["before"]["integrity"], serde_json::json!(["ok"]));
        assert_eq!(report["before"]["foreignKeyErrors"], 0);
        assert_eq!(report["before"]["journalMode"], "wal");
        assert!(report["before"]["business"].as_object().is_some());
        let library = manifest["cloneFiles"]
            .as_array()
            .unwrap()
            .iter()
            .find(|file| file["relativePath"] == "library.sqlite3")
            .unwrap();
        assert_eq!(report["checkpointedCloneSha256"], library["cloneSha256"]);
        assert_eq!(report["checkpointedCloneBytes"], library["bytes"]);
        assert_eq!(
            checkpoint["checkpointedCloneSha256"],
            library["cloneSha256"]
        );
        assert_eq!(checkpoint["schemaVersion"], 11);
    }

    #[test]
    #[ignore = "heavy I/O; requires a newly prepared storage-phase-profile fixture; never run with formal memory acceptance"]
    fn profile_file_backed_backup_phases() {
        use sha2::{Digest, Sha256};
        use std::io::{Read, Write};
        let supplied = PathBuf::from(
            std::env::var_os("IMAGELORE_BACKUP_PHASE_PROFILE_DIR")
                .expect("Explicit fresh profile fixture is required"),
        );
        assert!(supplied.is_absolute());
        profile_plain_path(&supplied);
        let root = fs::canonicalize(&supplied).unwrap();
        let project = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let evidence = fs::canonicalize(project.join("<private-evidence>")).unwrap();
        assert_eq!(root.parent(), Some(evidence.as_path()));
        assert!(root
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("storage-phase-profile-"));
        assert!(
            !root.join(".imagelore-acceptance-root").exists(),
            "A profile fixture must not be a GUI acceptance data root"
        );
        let marker = root.join(".imagelore-storage-phase-profile");
        profile_plain_path(&marker);
        let manifest_path = root.join("fixture-manifest.json");
        profile_plain_path(&manifest_path);
        let manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
        let fixture_version = manifest["fixtureVersion"].as_u64().unwrap();
        assert!(
            matches!(fixture_version, 1 | 2),
            "Future fixture versions are unsupported"
        );
        assert_eq!(
            fs::read_to_string(&marker).unwrap().trim(),
            format!("ImageLore storage phase profile fixture v{fixture_version}")
        );
        let profile_source_sha256 =
            format!("{:x}", Sha256::digest(include_bytes!("backup.rs"))).to_ascii_uppercase();
        assert_eq!(
            manifest["profileSourceSha256"].as_str().unwrap(),
            profile_source_sha256,
            "Fixture must bind the exact compiled backup.rs source"
        );
        let profile_db_source_sha256 =
            format!("{:x}", Sha256::digest(include_bytes!("db.rs"))).to_ascii_uppercase();
        assert_eq!(
            manifest["profileDbSourceSha256"].as_str().unwrap(),
            profile_db_source_sha256,
            "Fixture must bind the exact compiled db.rs source"
        );
        assert_eq!(manifest["fixtureType"], "storage-phase-profile");
        assert_eq!(manifest["sourceSQLiteOpened"], false);
        assert_eq!(manifest["exclusiveReadHandles"], true);
        if fixture_version == 1 {
            assert_eq!(manifest["sourceSidecarsAbsent"], true);
        }
        assert_eq!(manifest["singleUse"], true);
        assert!(
            !root.join(".profile-run-reserved").exists(),
            "This fixture has already been used; refuse before hash preflight"
        );
        assert_eq!(
            fs::canonicalize(Path::new(manifest["sourceRoot"].as_str().unwrap())).unwrap(),
            fs::canonicalize(evidence.join("native-gui/data")).unwrap()
        );
        assert_eq!(
            fs::canonicalize(Path::new(manifest["destination"].as_str().unwrap())).unwrap(),
            root
        );
        if fixture_version == 2 {
            profile_v2_bundle_guard(&root, &manifest);
        }
        let files = manifest[if fixture_version == 1 {
            "files"
        } else {
            "cloneFiles"
        }]
        .as_array()
        .unwrap();
        let hash_field = if fixture_version == 1 {
            "sourceSha256"
        } else {
            "cloneSha256"
        };
        assert!((2..=11).contains(&files.len()));
        let initial_backups = files
            .iter()
            .filter(|file| {
                file["relativePath"]
                    .as_str()
                    .unwrap()
                    .starts_with("backups/")
            })
            .count();
        assert!((1..=MAX_BACKUPS).contains(&initial_backups));
        assert_eq!(
            manifest["initialBackupCount"].as_u64().unwrap(),
            initial_backups as u64
        );
        let mut unique_files = std::collections::BTreeSet::new();
        for file in files {
            let relative = Path::new(file["relativePath"].as_str().unwrap());
            assert!(
                !relative.is_absolute()
                    && relative
                        .components()
                        .all(|part| matches!(part, std::path::Component::Normal(_)))
            );
            assert_eq!(file[hash_field].as_str().unwrap().len(), 64);
            assert!(file[hash_field]
                .as_str()
                .unwrap()
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit()));
            assert_eq!(file["copiedFromRetainedReadHandle"], true);
            assert!(unique_files.insert(relative.to_path_buf()));
            assert!(
                relative == Path::new("library.sqlite3")
                    || (relative.parent() == Some(Path::new("backups"))
                        && relative
                            .extension()
                            .and_then(|extension| extension.to_str())
                            == Some("sqlite3"))
            );
            let copy = root.join(relative);
            profile_plain_path(&copy);
            assert_eq!(
                fs::metadata(&copy).unwrap().len(),
                file["bytes"].as_u64().unwrap()
            );
            // Verify prepared bytes before init_db can change WAL/layout. This
            // streaming preflight warms every clone and is outside all timings.
            let mut input = File::open(&copy).unwrap();
            let mut hash = Sha256::new();
            let mut buffer = vec![0_u8; 64 * 1024];
            loop {
                let count = input.read(&mut buffer).unwrap();
                if count == 0 {
                    break;
                }
                hash.update(&buffer[..count]);
            }
            assert_eq!(
                format!("{:x}", hash.finalize()).to_ascii_uppercase(),
                file[hash_field].as_str().unwrap()
            );
            assert!(!companions(&copy)
                .iter()
                .skip(1)
                .any(|sidecar| sidecar.exists()));
        }
        assert!(unique_files.contains(Path::new("library.sqlite3")));
        profile_plain_path(&root.join("backups"));
        let mut reservation = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(root.join(".profile-run-reserved"))
            .expect("This fixture has already been used; prepare a fresh one");
        reservation
            .write_all(b"single use: profile run started\n")
            .unwrap();
        drop(reservation);
        let database = root.join("library.sqlite3");
        profile_plain_path(&database);
        let startup_backups = records(&root.join("backups")).unwrap();
        let conn = profile_operation(&root, "startup_normal", || {
            crate::open_library_connection(&database, &root, &root.join("backups"))
        });
        assert!(
            !root.join("recovery").exists()
                && !crate::diagnostics::recovery_notice_path(&root).exists(),
            "A normal-startup sample must not recover or replace the library"
        );
        assert_eq!(
            records(&root.join("backups"))
                .unwrap()
                .iter()
                .map(|record| &record.name)
                .collect::<Vec<_>>(),
            startup_backups
                .iter()
                .map(|record| &record.name)
                .collect::<Vec<_>>()
        );
        let sqlite_version: String = conn
            .query_row("SELECT sqlite_version()", [], |row| row.get(0))
            .unwrap();
        let asset_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM assets", [], |row| row.get(0))
            .unwrap();
        assert_eq!(
            asset_count, 50_000,
            "The explicit scale fixture must contain 50k assets"
        );
        let mutation_id: i64 = conn
            .query_row(
                "SELECT asset_id FROM prompt_state ORDER BY asset_id LIMIT 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let state = AppState {
            db: std::sync::Mutex::new(conn),
            backup_operation: std::sync::Mutex::new(()),
            data_dir: root.clone(),
            database_path: database.clone(),
            cache_dir: root.join("cache"),
            backups_dir: root.join("backups"),
            models_dir: root.join("models"),
            jobs: std::sync::Mutex::new(std::collections::HashMap::new()),
            next_job_id: std::sync::atomic::AtomicU64::new(1),
            vision_api_key: std::sync::Mutex::new(String::new()),
        };
        let baseline = profile_business(&database);
        validate(&database).unwrap();
        let before_auto = records(&state.backups_dir).unwrap();
        assert_eq!(before_auto.len(), initial_backups);
        let recent_time = before_auto[0].created_at + 1;
        assert!(
            profile_operation(&root, "auto_recent", || ensure_auto_at(&state, recent_time))
                .is_none()
        );
        assert_eq!(
            records(&state.backups_dir)
                .unwrap()
                .iter()
                .map(|record| &record.name)
                .collect::<Vec<_>>(),
            before_auto
                .iter()
                .map(|record| &record.name)
                .collect::<Vec<_>>()
        );
        let created = profile_operation(&root, "manual_create", || create(&state, "imagelore"));
        validate(Path::new(&created.path)).unwrap();
        assert_eq!(profile_business(Path::new(&created.path)), baseline);
        assert_eq!(profile_business(&database), baseline);
        let due_time = records(&state.backups_dir).unwrap()[0].created_at + AUTO_INTERVAL;
        let automatic = profile_operation(&root, "auto_due_create", || {
            ensure_auto_at(&state, due_time)
        })
        .expect("Due automatic operation must create a backup");
        validate(Path::new(&automatic.path)).unwrap();
        assert_eq!(profile_business(Path::new(&automatic.path)), baseline);
        assert!(profile_operation(&root, "stage_restore", || {
            stage_restore_at(&state, &created.name)
        }));
        let pending = root.join("restore.pending.sqlite3");
        validate(&pending).unwrap();
        assert_eq!(profile_business(&pending), baseline);
        assert_eq!(
            profile_business(&database),
            baseline,
            "Staging cannot change the active library"
        );
        state
            .db
            .lock()
            .unwrap()
            .execute(
                "UPDATE prompt_state SET prompt=?1 WHERE asset_id=?2",
                rusqlite::params!["Synthetic storage phase profile mutation", mutation_id],
            )
            .unwrap();
        drop(state); // The real replacement guard requires a closed active DB.
        let mutated = profile_business(&database);
        assert_ne!(mutated, baseline);
        profile_operation(&root, "apply_pending_restore", || {
            apply_pending_restore(&database, &root, &root.join("backups"))
        });
        let reopened = crate::db::init_db(&database).unwrap();
        drop(reopened);
        validate(&database).unwrap();
        assert_eq!(profile_business(&database), baseline);
        assert!(!pending.exists() && !root.join("restore.rollback").exists());
        let preserved: Vec<_> = fs::read_dir(root.join("backups"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| {
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("pre-restore-")
                    && path
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .ends_with(".sqlite3.raw")
            })
            .collect();
        assert!(
            !preserved.is_empty(),
            "Original forensic copies must survive restore"
        );
        for path in &preserved {
            profile_plain_path(path);
            validate(path).unwrap();
        }
        assert!(preserved
            .iter()
            .any(|path| profile_business(path) == mutated));
        let final_backups = records(&root.join("backups")).unwrap();
        assert!(final_backups.len() <= MAX_BACKUPS);
        for backup in &final_backups {
            validate(Path::new(&backup.path)).unwrap();
        }
        let summary = serde_json::json!({"state":"PASS_PROFILE_WORKFLOW_ASSERTIONS","packageVersion":env!("CARGO_PKG_VERSION"),"profileSourceSha256":profile_source_sha256,"profileDbSourceSha256":profile_db_source_sha256,"sqliteVersion":sqlite_version,"assetCount":asset_count,"initialBackups":initial_backups,"finalBackups":final_backups.len(),"business":baseline,"preservedRawCount":preserved.len(),"cacheBoundary":"OS cache uncontrolled: fixture copying/hash, streaming hash preflight of every clone, startup and untimed business/integrity assertions warm pages; fixed operation order; not a cold-cache claim","order":["startup_normal","auto_recent","manual_create","auto_due_create","stage_restore","apply_pending_restore"],"validationCacheKiB":VALIDATION_CACHE_KIB,"scope":"open_library_connection backend-only and synchronous production backup helpers; excludes global prepare_state (data_root/dirs/apply_pending_restore/cache-prune/diagnostics), Tauri/WebView/FCP/library-ready/blocking-pool queue/UI; uncontended mutex waits; inclusive and exclusive span durations","sourceLibraryOpenedByTest":false});
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(root.join("profile-summary.json"))
            .unwrap();
        serde_json::to_writer_pretty(&mut output, &summary).unwrap();
        output.flush().unwrap();
        println!(
            "{}",
            serde_json::json!({"state":"PASS_PROFILE_WORKFLOW_ASSERTIONS","assetCount":asset_count,"initialBackups":initial_backups,"finalBackups":final_backups.len(),"coldCacheProven":false})
        );
    }

    #[test]
    #[ignore = "requires an explicitly marked, isolated file-backed acceptance library"]
    fn profile_file_backed_validation() {
        use std::time::Instant;
        let root = PathBuf::from(std::env::var_os("IMAGELORE_STORAGE_PROFILE_DIR").unwrap());
        assert!(root.is_absolute());
        assert_eq!(
            fs::read_to_string(root.join(".imagelore-acceptance-root"))
                .unwrap()
                .trim(),
            "ImageLore acceptance fixture"
        );
        let database = root.join("library.sqlite3");
        let before = fs::read(&database).unwrap();
        for sample in 0..3 {
            let start = Instant::now();
            assert_eq!(integrity_probe_copy(&database).unwrap(), "ok");
            println!(
                "{{\"sample\":{sample},\"operation\":\"production_copy_integrity\",\"ms\":{}}}",
                start.elapsed().as_secs_f64() * 1000.0
            );
            let start = Instant::now();
            validate(&database).unwrap();
            println!(
                "{{\"sample\":{sample},\"operation\":\"production_validate\",\"ms\":{}}}",
                start.elapsed().as_secs_f64() * 1000.0
            );
            // Same bundled SQLite and full integrity check. This experiment
            // changes only the temporary connection's page-cache allowance.
            for cache_kib in [2000, 32768, 65536] {
                let conn = Connection::open_with_flags(&database, OpenFlags::SQLITE_OPEN_READ_ONLY)
                    .unwrap();
                conn.pragma_update(None, "cache_size", -cache_kib).unwrap();
                let start = Instant::now();
                assert_eq!(
                    conn.query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))
                        .unwrap(),
                    "ok"
                );
                println!("{{\"sample\":{sample},\"operation\":\"full_integrity\",\"cache_kib\":{cache_kib},\"ms\":{}}}", start.elapsed().as_secs_f64() * 1000.0);
            }
        }
        assert_eq!(fs::read(database).unwrap(), before);
    }

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

    #[test]
    fn probe_collision_keeps_preexisting_directory() {
        let root = test_root("probe-collision");
        let probe = root.join("occupied-probe");
        fs::create_dir(&probe).unwrap();
        let marker = probe.join("another-owner.bin");
        let original = b"another operation owns this directory\0\xff";
        fs::write(&marker, original).unwrap();

        let result = copy_probe_files_into(&root.join("library.sqlite3"), probe.clone());
        assert_eq!(
            result.unwrap_err().kind(),
            std::io::ErrorKind::AlreadyExists
        );
        assert_eq!(fs::read(&marker).unwrap(), original);
        assert_eq!(fs::read_dir(&probe).unwrap().count(), 1);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn probe_cleanup_after_owned_copy_failure_preserves_source() {
        let root = test_root("probe-copy-failure");
        // A directory cannot be copied as database bytes. The cleanup may only
        // remove the new probe, never the source or its contents.
        let source = root.join("library.sqlite3");
        fs::create_dir(&source).unwrap();
        let marker = source.join("retained-source.bin");
        let original = b"source must survive failed probe copy\0\xff";
        fs::write(&marker, original).unwrap();
        let probe = root.join("owned-probe");

        assert!(copy_probe_files_into(&source, probe.clone()).is_err());
        assert!(!probe.exists());
        assert_eq!(fs::read(&marker).unwrap(), original);
        fs::remove_dir_all(root).unwrap();
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
            backup_operation: std::sync::Mutex::new(()),
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
    fn staging_rejects_invalid_candidates_without_consuming_pending_and_can_retry() {
        let root = test_root("stage-retry");
        let state = test_state(&root);
        let original = root.join("backups/original.sqlite3");
        drop(seed(&original, "first restore"));
        stage_restore_at(&state, "original.sqlite3").unwrap();
        let pending = root.join("restore.pending.sqlite3");
        let staged_bytes = fs::read(&pending).unwrap();
        for kind in ["empty", "foreign", "future", "orphan", "truncated"] {
            let name = format!("{kind}.sqlite3");
            let candidate = state.backups_dir.join(&name);
            let conn = if kind == "empty" || kind == "foreign" {
                Connection::open(&candidate).unwrap()
            } else {
                seed(&candidate, kind)
            };
            match kind {
                "foreign" => conn
                    .execute_batch("CREATE TABLE foreign_data(value TEXT)")
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
                    conn.execute("INSERT INTO prompt_state(asset_id,prompt,updated_at) VALUES(999,'orphan',1)", []).unwrap();
                }
                _ => (),
            }
            drop(conn);
            if kind == "truncated" {
                let bytes = fs::read(&candidate).unwrap();
                fs::write(&candidate, &bytes[..bytes.len() / 2]).unwrap();
            }
            let candidate_bytes = fs::read(&candidate).unwrap();
            assert!(
                stage_restore_at(&state, &name).is_err(),
                "{kind} must be rejected"
            );
            assert_eq!(fs::read(&candidate).unwrap(), candidate_bytes);
            assert_eq!(fs::read(&pending).unwrap(), staged_bytes);
            assert_eq!(read_marker(&state.database_path), "current");
        }
        let retry = state.backups_dir.join("retry.sqlite3");
        drop(seed(&retry, "retried restore"));
        stage_restore_at(&state, "retry.sqlite3").unwrap();
        assert_eq!(read_marker(&pending), "retried restore");
        assert_eq!(read_marker(&original), "first restore");
        assert_eq!(read_marker(&state.database_path), "current");
        assert!(!root.join("restore.pending.previous.sqlite3").exists());
        drop(state);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn staging_does_not_wait_for_the_active_library_connection() {
        use std::sync::{mpsc, Arc};
        let root = test_root("stage-independent");
        let state = Arc::new(test_state(&root));
        drop(seed(
            &state.backups_dir.join("candidate.sqlite3"),
            "candidate",
        ));
        let library_guard = state.db.lock().unwrap();
        let worker_state = Arc::clone(&state);
        let (sent, received) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            sent.send(stage_restore_at(&worker_state, "candidate.sqlite3"))
                .unwrap();
        });
        let result = received.recv_timeout(std::time::Duration::from_secs(2));
        drop(library_guard);
        worker.join().unwrap();
        assert!(
            matches!(result, Ok(Ok(true))),
            "Staging only needs the backup-operation lock: {result:?}"
        );
        assert_eq!(
            read_marker(&root.join("restore.pending.sqlite3")),
            "candidate"
        );
        assert_eq!(read_marker(&state.database_path), "current");
        drop(state);
        fs::remove_dir_all(root).unwrap();
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
    fn recent_valid_auto_backup_does_not_wait_for_library_connection() {
        let root = test_root("recent-auto-library-lock");
        let state = std::sync::Arc::new(test_state(&root));
        let recent = create(&state, "manual").unwrap();
        let library = state.db.lock().unwrap();
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let (result_tx, result_rx) = std::sync::mpsc::channel();
        let worker_state = std::sync::Arc::clone(&state);
        let worker = std::thread::spawn(move || {
            started_tx.send(()).unwrap();
            result_tx
                .send(ensure_auto_at(&worker_state, recent.created_at + 1))
                .unwrap();
        });
        started_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        let completed_while_library_busy =
            result_rx.recv_timeout(std::time::Duration::from_secs(1));
        // Release and join even on the old blocking implementation, so a failed
        // assertion never leaves a worker or open fixture behind.
        drop(library);
        worker.join().unwrap();
        drop(state);
        fs::remove_dir_all(root).unwrap();
        assert!(
            matches!(completed_while_library_busy, Ok(Ok(None))),
            "A valid recent backup check must finish without the library connection: {completed_while_library_busy:?}"
        );
    }

    #[test]
    fn auto_backup_revalidates_a_recent_backup_after_external_corruption() {
        let root = test_root("recent-auto-external-corruption");
        let state = test_state(&root);
        let recent = create(&state, "manual").unwrap();
        assert!(ensure_auto_at(&state, recent.created_at + 1)
            .unwrap()
            .is_none());
        let damaged = b"externally damaged after a successful check";
        fs::write(&recent.path, damaged).unwrap();
        let replacement = ensure_auto_at(&state, recent.created_at + 2)
            .unwrap()
            .expect("A previously valid but now corrupt backup cannot suppress a new snapshot");
        validate(Path::new(&replacement.path)).unwrap();
        assert!(!Path::new(&recent.path).exists());
        let rejected = fs::read_dir(root.join("backups/rejected"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        assert_eq!(rejected.len(), 1);
        assert_eq!(fs::read(&rejected[0]).unwrap(), damaged);
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
