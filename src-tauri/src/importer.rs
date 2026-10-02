use crate::{
    db, generation, generation_index, jobs, metadata,
    models::{ImportProgress, ImportSummary, VisualDnaPatch},
    preview, references, sidecar,
    state::AppState,
    visual_dna,
};
use rusqlite::params;
use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};
use tauri::{AppHandle, Emitter, Manager, State};
use walkdir::WalkDir;

struct PreparedAsset {
    path: String,
    name: String,
    width: Option<i64>,
    height: Option<i64>,
    file_size: Option<i64>,
    format: String,
    mime_type: String,
    metadata_type: String,
    generation_json: String,
    fingerprint: String,
    portable_id: String,
    file_mtime: i64,
    prompt: String,
    negative_prompt: String,
    model: String,
    tags: Vec<String>,
    parents: Vec<sidecar::ParentRef>,
    session: Option<sidecar::SessionRef>,
    visual_dna: Option<VisualDnaPatch>,
    references: Vec<references::ReferenceInput>,
}

enum InsertOutcome {
    Added(i64),
    Existing(i64),
    Duplicate(i64),
}

fn discover_files(
    roots: Vec<String>,
    recursive_dirs: bool,
    cancel: &AtomicBool,
) -> (Vec<PathBuf>, i64) {
    let mut out = Vec::new();
    let mut failed = 0;
    'roots: for raw in roots {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let root = PathBuf::from(raw);
        if root.is_file() {
            if metadata::is_supported(&root) {
                out.push(root)
            }
        } else if root.is_dir() && recursive_dirs {
            for entry in WalkDir::new(root).follow_links(false) {
                if cancel.load(Ordering::Relaxed) {
                    break 'roots;
                }
                let entry = match entry {
                    Ok(entry) => entry,
                    Err(_) => {
                        failed += 1;
                        continue;
                    }
                };
                if entry.file_type().is_file() && metadata::is_supported(entry.path()) {
                    out.push(entry.into_path())
                }
            }
        } else if !root.exists() {
            failed += 1;
        }
    }
    (out, failed)
}

#[cfg(test)]
fn supported_files(roots: Vec<String>, recursive_dirs: bool, cancel: &AtomicBool) -> Vec<PathBuf> {
    discover_files(roots, recursive_dirs, cancel).0
}

fn prepare(path: &Path) -> Option<PreparedAsset> {
    if !path.is_file() || !metadata::is_supported(path) {
        return None;
    }
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let info = metadata::file_info(&canonical);
    let extract = metadata::extract_generation(&canonical);
    let saved = sidecar::read(&canonical).ok()?;
    let prompt = saved
        .as_ref()
        .map(|x| sidecar::text(x, "prompt"))
        .filter(|x| !x.is_empty())
        .unwrap_or(extract.prompt);
    let negative_prompt = saved
        .as_ref()
        .map(|x| sidecar::text(x, "negative_prompt"))
        .filter(|x| !x.is_empty())
        .unwrap_or(extract.negative_prompt);
    let model = saved
        .as_ref()
        .map(|x| sidecar::text(x, "model"))
        .filter(|x| !x.is_empty())
        .unwrap_or(extract.model);
    let tags = saved.as_ref().map(sidecar::tags).unwrap_or_default();
    let parents = saved.as_ref().map(sidecar::parents).unwrap_or_default();
    let session = saved.as_ref().and_then(sidecar::session);
    let visual_dna = saved.as_ref().and_then(sidecar::visual_dna);
    let references = saved.as_ref().map(sidecar::references).unwrap_or_default();
    let fingerprint = metadata::fingerprint(&canonical);
    if fingerprint.is_empty() {
        return None;
    }
    let requested_portable = saved.as_ref().map(sidecar::portable_id).unwrap_or_default();
    let portable_id = if requested_portable.is_empty() {
        db::make_portable_id(&format!(
            "{}:{}:{}:{}",
            fingerprint,
            canonical.to_string_lossy(),
            info.file_mtime,
            info.file_size.unwrap_or(0)
        ))
    } else {
        requested_portable
    };
    let name = canonical
        .file_name()
        .and_then(|x| x.to_str())
        .unwrap_or("image")
        .to_string();
    Some(PreparedAsset {
        path: canonical.to_string_lossy().to_string(),
        name,
        width: info.width,
        height: info.height,
        file_size: info.file_size,
        format: info.format,
        mime_type: info.mime_type,
        metadata_type: extract.metadata_type,
        generation_json: extract.generation_json,
        fingerprint,
        portable_id,
        file_mtime: info.file_mtime,
        prompt,
        negative_prompt,
        model,
        tags,
        parents,
        session,
        visual_dna,
        references,
    })
}

fn merge_context(conn: &rusqlite::Connection, id: i64, item: &PreparedAsset) -> Result<(), String> {
    let portable_id: String = conn
        .query_row(
            "SELECT portable_id FROM assets WHERE id=?1",
            params![id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    generation::assign_session_by_name(conn, id, item.session.as_ref())?;
    generation::queue_parent_refs(conn, &portable_id, &item.parents)?;
    visual_dna::merge_imported(conn, id, item.visual_dna.as_ref())?;
    references::merge_imported(conn, id, &item.references)?;
    Ok(())
}

fn refresh_existing(
    state: &AppState,
    id: i64,
    item: PreparedAsset,
) -> Result<InsertOutcome, String> {
    let mut conn = state.db.lock().map_err(|e| e.to_string())?;
    let old = db::get_asset(&conn, id)?;
    let stamp = db::now();
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    tx.execute(
        "UPDATE assets SET name=?1,width=?2,height=?3,file_size=?4,format=?5,mime_type=?6,metadata_type=?7,generation_json=?8,fingerprint=?9,file_mtime=?10,missing=0,updated_at=?11 WHERE id=?12",
        params![item.name,item.width,item.height,item.file_size,item.format,item.mime_type,item.metadata_type,item.generation_json,item.fingerprint,item.file_mtime,stamp,id]
    ).map_err(|e|e.to_string())?;
    if old.prompt.is_empty() && old.negative_prompt.is_empty() && old.model.is_empty() {
        tx.execute(
            "UPDATE prompt_state SET prompt=?1,negative_prompt=?2,model=?3,updated_at=?4 WHERE asset_id=?5",
            params![item.prompt,item.negative_prompt,item.model,stamp,id]
        ).map_err(|e|e.to_string())?;
    }
    if old.tags.is_empty() && !item.tags.is_empty() {
        db::replace_tags_raw(&tx, id, &item.tags)?;
    }
    merge_context(&tx, id, &item)?;
    generation_index::upsert(&tx, id, &item.generation_json)?;
    db::reindex_asset(&tx, id)?;
    tx.commit().map_err(|e| e.to_string())?;
    drop(conn);
    preview::purge_asset_cache(&state.cache_dir, &old.fingerprint);
    Ok(InsertOutcome::Existing(id))
}

fn insert_new(state: &AppState, item: PreparedAsset) -> Result<InsertOutcome, String> {
    let mut conn = state.db.lock().map_err(|e| e.to_string())?;
    if let Some(id) = db::asset_exists_by_path(&conn, &item.path)? {
        drop(conn);
        return refresh_existing(state, id, item);
    }
    if let Some(id) = db::asset_exists_by_fingerprint(&conn, &item.fingerprint)? {
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        merge_context(&tx, id, &item)?;
        db::reindex_asset(&tx, id)?;
        tx.commit().map_err(|e| e.to_string())?;
        return Ok(InsertOutcome::Duplicate(id));
    }

    let portable_id = if db::asset_by_portable_id(&conn, &item.portable_id)?.is_some() {
        db::make_portable_id(&format!("{}:{}:{}", item.portable_id, item.path, db::now()))
    } else {
        item.portable_id.clone()
    };
    let stamp = db::now();
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT INTO assets(path,name,favorite,width,height,file_size,format,mime_type,metadata_type,generation_json,fingerprint,portable_id,file_mtime,missing,created_at,updated_at) VALUES(?1,?2,0,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,0,?13,?13)",
        params![item.path,item.name,item.width,item.height,item.file_size,item.format,item.mime_type,item.metadata_type,item.generation_json,item.fingerprint,portable_id,item.file_mtime,stamp]
    ).map_err(|e|e.to_string())?;
    let id = tx.last_insert_rowid();
    tx.execute("INSERT INTO prompt_state(asset_id,prompt,negative_prompt,model,updated_at) VALUES(?1,?2,?3,?4,?5)",params![id,item.prompt,item.negative_prompt,item.model,stamp]).map_err(|e|e.to_string())?;
    db::replace_tags_raw(&tx, id, &item.tags)?;
    merge_context(&tx, id, &item)?;
    generation_index::upsert(&tx, id, &item.generation_json)?;
    db::reindex_asset(&tx, id)?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(InsertOutcome::Added(id))
}

pub(crate) fn run_import<F>(
    state: &AppState,
    roots: Vec<String>,
    recursive_dirs: bool,
    cancel: &AtomicBool,
    mut progress: F,
) -> Result<ImportSummary, String>
where
    F: FnMut(ImportProgress),
{
    let (paths, discovery_failed) = discover_files(roots, recursive_dirs, cancel);
    let total = paths.len();
    let mut result = ImportSummary {
        added: 0,
        skipped: 0,
        duplicates: 0,
        failed: discovery_failed,
        last_id: None,
    };

    for (index, path) in paths.into_iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
        let path_text = canonical.to_string_lossy().to_string();
        let name = canonical
            .file_name()
            .and_then(|x| x.to_str())
            .unwrap_or("image")
            .to_string();

        let existing = {
            let conn = state.db.lock().map_err(|e| e.to_string())?;
            db::asset_file_state_by_path(&conn, &path_text)?
        };
        if let Some((id, stored_mtime, stored_size)) = existing {
            let (current_size, current_mtime) = metadata::file_stamp(&canonical);
            let sidecar_present = sidecar::path_for(&canonical).exists();
            if stored_mtime == current_mtime && stored_size == current_size && !sidecar_present {
                let conn = state.db.lock().map_err(|e| e.to_string())?;
                conn.execute("UPDATE assets SET missing=0 WHERE id=?1", params![id])
                    .map_err(|e| e.to_string())?;
                result.skipped += 1;
                result.last_id = Some(id);
                progress(ImportProgress {
                    job_id: 0,
                    processed: index + 1,
                    total,
                    added: result.added,
                    skipped: result.skipped,
                    duplicates: result.duplicates,
                    failed: result.failed,
                    current_name: name,
                    done: false,
                    cancelled: false,
                    last_id: result.last_id,
                });
                continue;
            }
        }

        match prepare(&canonical).map(|item| {
            if let Some((id, _, _)) = existing {
                refresh_existing(state, id, item)
            } else {
                insert_new(state, item)
            }
        }) {
            Some(Ok(InsertOutcome::Added(id))) => {
                result.added += 1;
                result.last_id = Some(id)
            }
            Some(Ok(InsertOutcome::Existing(id))) => {
                result.skipped += 1;
                result.last_id = Some(id)
            }
            Some(Ok(InsertOutcome::Duplicate(id))) => {
                result.duplicates += 1;
                result.last_id = Some(id)
            }
            Some(Err(_)) => result.failed += 1,
            None => result.failed += 1,
        }

        progress(ImportProgress {
            job_id: 0,
            processed: index + 1,
            total,
            added: result.added,
            skipped: result.skipped,
            duplicates: result.duplicates,
            failed: result.failed,
            current_name: name,
            done: false,
            cancelled: false,
            last_id: result.last_id,
        });
    }
    Ok(result)
}

pub(crate) fn start_job_with_finish<F>(
    app: AppHandle,
    roots: Vec<String>,
    recursive_dirs: bool,
    on_finish: F,
) -> Result<u64, String>
where
    F: FnOnce(&AppState, &ImportSummary, bool) -> Result<(), String> + Send + 'static,
{
    let state = app.state::<AppState>();
    let (job_id, cancel) = jobs::register(state.inner())?;

    let app_for_thread = app.clone();
    std::thread::spawn(move || {
        let state = app_for_thread.state::<AppState>();
        let run = run_import(
            state.inner(),
            roots,
            recursive_dirs,
            &cancel,
            |mut event| {
                event.job_id = job_id;
                let _ = app_for_thread.emit("imagelore://import-progress", event);
            },
        );
        let cancelled = cancel.load(Ordering::Relaxed);
        let mut summary = run.unwrap_or(ImportSummary {
            added: 0,
            skipped: 0,
            duplicates: 0,
            failed: 1,
            last_id: None,
        });
        if on_finish(state.inner(), &summary, cancelled).is_err() {
            summary.failed += 1;
        }
        let _ = app_for_thread.emit(
            "imagelore://import-progress",
            ImportProgress {
                job_id,
                processed: 0,
                total: 0,
                added: summary.added,
                skipped: summary.skipped,
                duplicates: summary.duplicates,
                failed: summary.failed,
                current_name: String::new(),
                done: true,
                cancelled,
                last_id: summary.last_id,
            },
        );
        jobs::finish(state.inner(), job_id);
    });

    Ok(job_id)
}

pub(crate) fn start_job(
    app: AppHandle,
    roots: Vec<String>,
    recursive_dirs: bool,
) -> Result<u64, String> {
    start_job_with_finish(app, roots, recursive_dirs, |_, _, _| Ok(()))
}

#[tauri::command]
pub fn start_import_paths(app: AppHandle, paths: Vec<String>) -> Result<u64, String> {
    start_job(app, paths, false)
}

#[tauri::command]
pub fn start_import_folder(app: AppHandle, path: String) -> Result<u64, String> {
    start_job(app, vec![path], true)
}

#[tauri::command]
pub fn start_import_dropped_paths(app: AppHandle, paths: Vec<String>) -> Result<u64, String> {
    start_job(app, paths, true)
}

#[tauri::command]
pub fn cancel_import(state: State<'_, AppState>, job_id: u64) -> Result<bool, String> {
    jobs::cancel(state.inner(), job_id)
}

fn immediate(
    state: &AppState,
    roots: Vec<String>,
    recursive_dirs: bool,
) -> Result<ImportSummary, String> {
    let cancel = AtomicBool::new(false);
    run_import(state, roots, recursive_dirs, &cancel, |_| {})
}

#[tauri::command]
pub async fn import_paths(app: AppHandle, paths: Vec<String>) -> Result<ImportSummary, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        immediate(state.inner(), paths, false)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn import_folder(app: AppHandle, path: String) -> Result<ImportSummary, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        immediate(state.inner(), vec![path], true)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn import_dropped_paths(
    app: AppHandle,
    paths: Vec<String>,
) -> Result<ImportSummary, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        immediate(state.inner(), paths, true)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        collections::HashMap,
        fs,
        sync::{atomic::AtomicU64, Mutex},
        time::{SystemTime, UNIX_EPOCH},
    };

    fn test_state(root: &Path) -> AppState {
        fs::create_dir_all(root).unwrap();
        let database_path = root.join("library.sqlite3");
        AppState {
            db: Mutex::new(db::init_db(&database_path).unwrap()),
            backup_operation: Mutex::new(()),
            data_dir: root.to_path_buf(),
            cache_dir: root.join("cache"),
            database_path,
            backups_dir: root.join("backups"),
            models_dir: root.join("models"),
            jobs: Mutex::new(HashMap::new()),
            next_job_id: AtomicU64::new(1),
            vision_api_key: Mutex::new(String::new()),
        }
    }

    fn image_fixture(root: &Path, name: &str, color: [u8; 3]) -> PathBuf {
        let path = root.join(name);
        image::RgbImage::from_pixel(3, 3, image::Rgb(color))
            .save(&path)
            .unwrap();
        path
    }

    fn import_files(state: &AppState, paths: &[&Path]) -> ImportSummary {
        run_import(
            state,
            paths
                .iter()
                .map(|path| path.to_string_lossy().to_string())
                .collect(),
            false,
            &AtomicBool::new(false),
            |_| {},
        )
        .unwrap()
    }

    fn exported(state: &AppState, id: i64) -> serde_json::Value {
        let path = crate::commands::export_sidecar_for_state(state, id).unwrap();
        serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
    }

    fn portable_content(mut value: serde_json::Value) -> serde_json::Value {
        value.as_object_mut().unwrap().remove("image");
        if let Some(dna) = value
            .get_mut("visual_dna")
            .and_then(serde_json::Value::as_object_mut)
        {
            dna.remove("source");
            dna.remove("updated_at");
        }
        value["parents"]
            .as_array_mut()
            .unwrap()
            .sort_by_key(|parent| parent["portable_id"].as_str().unwrap().to_string());
        value
    }

    #[test]
    fn inbox_remix_sidecar_round_trip_preserves_portable_lineage_and_context_after_restart() {
        use crate::{models::RemixSourceInput, remix};
        use serde_json::json;
        let root = temp_root("cross-library-workflow");
        let images = root.join("images-a");
        fs::create_dir_all(&images).unwrap();
        let first = test_state(&root.join("library-a"));
        let base = image_fixture(&images, "base.png", [20, 40, 60]);
        let light = image_fixture(&images, "light.png", [80, 100, 120]);
        let child = image_fixture(&images, "result.png", [140, 160, 180]);
        sidecar::write_atomic(&sidecar::path_for(&base), &json!({
            "schema":"imagelore.sidecar.v3", "asset":{"portable_id":"portable-base"},
            "prompt":"手工基础 prompt", "negative_prompt":"avoid noise", "model":"model-a",
            "tags":["灵感", "reference"],
            "visual_dna":{"subject":"蓝发角色", "style":"写实摄影"},
            "session":{"name":"合成 Session", "session_note":"session notes", "asset_note":"base notes"},
            "reference":{"source_url":"https://cdn.example/base.png", "page_url":"https://example/one", "page_title":"来源一", "captured_at":10,"metadata":{"purpose":"composition"}}
        }).to_string()).unwrap();
        sidecar::write_atomic(&sidecar::path_for(&light), &json!({
            "schema":"imagelore.sidecar.v3", "asset":{"portable_id":"portable-light"},
            "prompt":"lighting reference", "visual_dna":{"lighting":"霓虹侧光", "environment":"夜间街道"}
        }).to_string()).unwrap();
        assert_eq!(import_files(&first, &[&base, &light]).added, 2);
        let (base_id, light_id) = {
            let conn = first.db.lock().unwrap();
            (
                db::asset_by_portable_id(&conn, "portable-base")
                    .unwrap()
                    .unwrap(),
                db::asset_by_portable_id(&conn, "portable-light")
                    .unwrap()
                    .unwrap(),
            )
        };
        let duplicate = images.join("duplicate.png");
        fs::copy(&base, &duplicate).unwrap();
        sidecar::write_atomic(&sidecar::path_for(&duplicate), &json!({
            "schema":"imagelore.sidecar.v3", "reference":{"source_url":"https://cdn.example/base.png", "page_url":"https://example/two", "page_title":"来源二", "captured_at":20,"metadata":{"purpose":"lighting"}}
        }).to_string()).unwrap();
        for _ in 0..3 {
            let summary = import_files(&first, &[&duplicate]);
            assert_eq!(summary.duplicates, 1);
            assert_eq!(summary.added, 0);
            assert_eq!(summary.failed, 0);
        }
        {
            let conn = first.db.lock().unwrap();
            assert_eq!(references::list(&conn, base_id).unwrap().len(), 2);
            assert_eq!(
                db::get_asset(&conn, base_id).unwrap().prompt,
                "手工基础 prompt"
            );
            assert_eq!(visual_dna::get(&conn, base_id).unwrap().subject, "蓝发角色");
        }
        sidecar::write_atomic(
            &sidecar::path_for(&child),
            &json!({
                "schema":"imagelore.sidecar.v3", "asset":{"portable_id":"portable-result"},
                "prompt":"already supplied result prompt", "visual_dna":{"composition":"中心构图"}
            })
            .to_string(),
        )
        .unwrap();
        assert_eq!(import_files(&first, &[&child]).added, 1);
        let child_id = {
            let mut conn = first.db.lock().unwrap();
            let child_id = db::asset_by_portable_id(&conn, "portable-result")
                .unwrap()
                .unwrap();
            let draft = remix::save(
                &mut conn,
                None,
                base_id,
                "Remix generated prompt".into(),
                vec![
                    RemixSourceInput {
                        asset_id: base_id,
                        fields: vec!["subject".into(), "style".into()],
                        source_url: String::new(),
                    },
                    RemixSourceInput {
                        asset_id: light_id,
                        fields: vec!["lighting".into(), "environment".into()],
                        source_url: "https://example/ref".into(),
                    },
                ],
            )
            .unwrap();
            remix::apply_lineage(&mut conn, draft.id, child_id).unwrap();
            remix::apply_lineage(&mut conn, draft.id, child_id).unwrap();
            assert_eq!(
                db::get_asset(&conn, child_id).unwrap().prompt,
                "already supplied result prompt"
            );
            child_id
        };
        let base_export = exported(&first, base_id);
        let light_export = exported(&first, light_id);
        let child_export = exported(&first, child_id);
        assert_eq!(child_export["parents"].as_array().unwrap().len(), 2);
        assert!(child_export["parents"]
            .as_array()
            .unwrap()
            .iter()
            .any(|parent| parent["relation_type"] == "derived_from"
                && parent["note"] == "Remix · 主体、风格"));
        assert!(child_export["parents"]
            .as_array()
            .unwrap()
            .iter()
            .any(|parent| parent["relation_type"] == "reference"
                && parent["note"] == "Remix · 光线、环境"));
        assert_eq!(child_export["session"]["name"], "合成 Session");
        assert_eq!(child_export["session"]["asset_note"], "Remix result");
        let images_b = root.join("images-b");
        fs::create_dir_all(&images_b).unwrap();
        let mut copied = Vec::new();
        for original in [&child, &light, &base] {
            let destination = images_b.join(original.file_name().unwrap());
            fs::copy(original, &destination).unwrap();
            fs::copy(sidecar::path_for(original), sidecar::path_for(&destination)).unwrap();
            copied.push(destination);
        }
        let second_root = root.join("library-b");
        let second = test_state(&second_root);
        assert_eq!(import_files(&second, &[&copied[0]]).added, 1);
        {
            let conn = second.db.lock().unwrap();
            assert_eq!(
                conn.query_row("SELECT COUNT(*) FROM pending_relations", [], |row| row
                    .get::<_, i64>(0))
                    .unwrap(),
                2
            );
            assert_eq!(
                conn.query_row("SELECT COUNT(*) FROM relations", [], |row| row
                    .get::<_, i64>(0))
                    .unwrap(),
                0
            );
        }
        assert_eq!(import_files(&second, &[&copied[1], &copied[2]]).added, 2);
        for _ in 0..3 {
            let repeated = import_files(&second, &[&copied[0], &copied[1], &copied[2]]);
            assert_eq!(repeated.skipped, 3);
            assert_eq!(repeated.failed, 0);
        }
        drop(second);
        let reopened = test_state(&second_root);
        for (portable, expected) in [
            ("portable-base", base_export),
            ("portable-light", light_export),
            ("portable-result", child_export),
        ] {
            let id = {
                let conn = reopened.db.lock().unwrap();
                db::asset_by_portable_id(&conn, portable).unwrap().unwrap()
            };
            assert_eq!(
                portable_content(exported(&reopened, id)),
                portable_content(expected)
            );
        }
        {
            let conn = reopened.db.lock().unwrap();
            assert_eq!(
                conn.query_row("SELECT COUNT(*) FROM assets", [], |row| row
                    .get::<_, i64>(0))
                    .unwrap(),
                3
            );
            assert_eq!(
                conn.query_row("SELECT COUNT(*) FROM pending_relations", [], |row| row
                    .get::<_, i64>(0))
                    .unwrap(),
                0
            );
            assert_eq!(
                conn.query_row("SELECT COUNT(*) FROM relations", [], |row| row
                    .get::<_, i64>(0))
                    .unwrap(),
                2
            );
            assert_eq!(
                conn.query_row("SELECT COUNT(*) FROM reference_sources", [], |row| row
                    .get::<_, i64>(0))
                    .unwrap(),
                2
            );
        }
        drop(reopened);
        drop(first);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn source_sync_failure_and_cancellation_preserve_success_timestamp_and_committed_data() {
        let root = temp_root("source-scan");
        let images = root.join("images");
        fs::create_dir_all(&images).unwrap();
        let state_root = root.join("library");
        let state = test_state(&state_root);
        let image_a = image_fixture(&images, "a.png", [1, 2, 3]);
        let image_b = image_fixture(&images, "b.png", [4, 5, 6]);
        {
            let conn = state.db.lock().unwrap();
            conn.execute("INSERT INTO source_folders(id,path,name,auto_sync,last_scan_at,created_at,updated_at) VALUES(1,?1,'Synthetic',1,42,1,42)", params![images.to_string_lossy()]).unwrap();
        }
        let cancel = AtomicBool::new(true);
        let pre_cancel = run_import(
            &state,
            vec![images.to_string_lossy().to_string()],
            true,
            &cancel,
            |_| {},
        )
        .unwrap();
        assert_eq!(pre_cancel.added, 0);
        crate::sources::record_completed_scan(
            &state.db.lock().unwrap(),
            &[1],
            &pre_cancel,
            true,
            100,
        )
        .unwrap();
        cancel.store(false, Ordering::Relaxed);
        let partial = run_import(
            &state,
            vec![
                image_a.to_string_lossy().to_string(),
                image_b.to_string_lossy().to_string(),
            ],
            false,
            &cancel,
            |_| cancel.store(true, Ordering::Relaxed),
        )
        .unwrap();
        assert_eq!(partial.added, 1);
        crate::sources::record_completed_scan(&state.db.lock().unwrap(), &[1], &partial, true, 200)
            .unwrap();
        let failure = run_import(
            &state,
            vec![
                images.to_string_lossy().to_string(),
                root.join("unavailable-folder")
                    .to_string_lossy()
                    .to_string(),
            ],
            true,
            &AtomicBool::new(false),
            |_| {},
        )
        .unwrap();
        assert_eq!(failure.added, 1);
        assert_eq!(failure.failed, 1);
        crate::sources::record_completed_scan(
            &state.db.lock().unwrap(),
            &[1],
            &failure,
            false,
            300,
        )
        .unwrap();
        {
            let conn = state.db.lock().unwrap();
            assert_eq!(
                crate::sources::list_from_conn(&conn).unwrap()[0].last_scan_at,
                42
            );
            conn.execute_batch("CREATE TRIGGER synthetic_import_failure BEFORE INSERT ON reference_sources BEGIN SELECT RAISE(ABORT,'synthetic reference failure'); END").unwrap();
        }
        let duplicate = images.join("duplicate.png");
        fs::copy(&image_a, &duplicate).unwrap();
        sidecar::write_atomic(&sidecar::path_for(&duplicate), r#"{"schema":"imagelore.sidecar.v3","reference":{"source_url":"https://example/failed.png","page_url":"https://example/failed"},"session":{"name":"must rollback"},"visual_dna":{"subject":"must rollback"}}"#).unwrap();
        let failed_merge = import_files(&state, &[&duplicate]);
        assert_eq!(failed_merge.failed, 1);
        assert_eq!(failed_merge.duplicates, 0);
        {
            let conn = state.db.lock().unwrap();
            assert_eq!(
                conn.query_row("SELECT COUNT(*) FROM generation_sessions", [], |row| row
                    .get::<_, i64>(0))
                    .unwrap(),
                0
            );
            assert_eq!(
                conn.query_row(
                    "SELECT COUNT(*) FROM visual_dna WHERE subject<>''",
                    [],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
                0
            );
            conn.execute_batch("DROP TRIGGER synthetic_import_failure")
                .unwrap();
        }
        fs::remove_file(sidecar::path_for(&duplicate)).unwrap();
        for invalid_sidecar in ["{truncated", r#"{"schema":"imagelore.sidecar.v99"}"#] {
            fs::write(sidecar::path_for(&duplicate), invalid_sidecar).unwrap();
            let invalid_summary = import_files(&state, &[&duplicate]);
            assert_eq!(invalid_summary.failed, 1);
            assert_eq!(invalid_summary.duplicates, 0);
            assert_eq!(
                fs::read_to_string(sidecar::path_for(&duplicate)).unwrap(),
                invalid_sidecar
            );
        }
        fs::remove_file(sidecar::path_for(&duplicate)).unwrap();
        drop(state);
        let state = test_state(&state_root);
        {
            let conn = state.db.lock().unwrap();
            assert_eq!(
                crate::sources::list_from_conn(&conn).unwrap()[0].last_scan_at,
                42
            );
            assert_eq!(
                conn.query_row("SELECT COUNT(*) FROM assets", [], |row| row
                    .get::<_, i64>(0))
                    .unwrap(),
                2
            );
            assert_eq!(
                conn.query_row("SELECT COUNT(*) FROM generation_sessions", [], |row| row
                    .get::<_, i64>(0))
                    .unwrap(),
                0
            );
        }
        let success = run_import(
            &state,
            vec![images.to_string_lossy().to_string()],
            true,
            &AtomicBool::new(false),
            |_| {},
        )
        .unwrap();
        assert_eq!(success.failed, 0);
        crate::sources::record_completed_scan(
            &state.db.lock().unwrap(),
            &[1],
            &success,
            false,
            400,
        )
        .unwrap();
        drop(state);
        let reopened = test_state(&state_root);
        {
            let conn = reopened.db.lock().unwrap();
            assert_eq!(
                crate::sources::list_from_conn(&conn).unwrap()[0].last_scan_at,
                400
            );
            assert_eq!(
                conn.query_row("SELECT COUNT(*) FROM assets", [], |row| row
                    .get::<_, i64>(0))
                    .unwrap(),
                2
            );
            assert_eq!(
                conn.query_row("SELECT COUNT(*) FROM reference_sources", [], |row| row
                    .get::<_, i64>(0))
                    .unwrap(),
                0
            );
        }
        drop(reopened);
        fs::remove_dir_all(root).unwrap();
    }

    fn temp_root(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "imagelore-importer-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn supported_files_handles_direct_and_recursive_inputs() {
        let root = temp_root("files");
        let nested = root.join("nested");
        fs::create_dir_all(&nested).unwrap();
        let direct = root.join("direct.png");
        let child = nested.join("child.jpg");
        let ignored = nested.join("notes.txt");
        fs::write(&direct, b"not-decoded-here").unwrap();
        fs::write(&child, b"not-decoded-here").unwrap();
        fs::write(&ignored, b"text").unwrap();

        let cancel = AtomicBool::new(false);
        let direct_only = supported_files(
            vec![
                direct.to_string_lossy().to_string(),
                root.to_string_lossy().to_string(),
            ],
            false,
            &cancel,
        );
        assert_eq!(direct_only, vec![direct.clone()]);

        let recursive = supported_files(vec![root.to_string_lossy().to_string()], true, &cancel);
        assert!(recursive.contains(&direct));
        assert!(recursive.contains(&child));
        assert!(!recursive.contains(&ignored));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn supported_files_honors_pre_cancelled_job() {
        let root = temp_root("cancel");
        let image = root.join("image.png");
        fs::write(&image, b"x").unwrap();
        let cancel = AtomicBool::new(true);

        let found = supported_files(vec![image.to_string_lossy().to_string()], false, &cancel);

        assert!(found.is_empty());
        let _ = fs::remove_dir_all(root);
    }
}
