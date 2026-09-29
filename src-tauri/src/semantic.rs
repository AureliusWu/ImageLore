use crate::{
    db, jobs, search,
    models::{LibraryFilter, SemanticHit, SemanticProgress, SemanticStatus},
    state::AppState,
};
use fastembed::{
    ImageEmbedding, ImageInitOptionsUserDefined, InitOptionsUserDefined, Pooling, TextEmbedding,
    TokenizerFiles, UserDefinedEmbeddingModel, UserDefinedImageEmbeddingModel,
};
use hf_hub::{api::sync::ApiBuilder, Repo, RepoType};
use rusqlite::{params, OptionalExtension};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    sync::atomic::Ordering,
};
use tauri::{AppHandle, Emitter, Manager, State};
use walkdir::WalkDir;

pub const MODEL_ID: &str = "clip-vit-b32-qdrant-v1";
const DIMENSIONS: usize = 512;
const TRUST_MANIFEST_VERSION: u32 = 1;

#[derive(Clone, Copy)]
struct SupportFile {
    path: &'static str,
    max_bytes: u64,
}

#[derive(Clone, Copy)]
struct TrustedModelSpec {
    name: &'static str,
    repo: &'static str,
    revision: &'static str,
    onnx_sha256: &'static str,
    onnx_size: u64,
    support: &'static [SupportFile],
}

const VISION_SUPPORT: &[SupportFile] = &[SupportFile {
    path: "preprocessor_config.json",
    max_bytes: 64 * 1024,
}];
const TEXT_SUPPORT: &[SupportFile] = &[
    SupportFile {
        path: "tokenizer.json",
        max_bytes: 8 * 1024 * 1024,
    },
    SupportFile {
        path: "config.json",
        max_bytes: 64 * 1024,
    },
    SupportFile {
        path: "special_tokens_map.json",
        max_bytes: 64 * 1024,
    },
    SupportFile {
        path: "tokenizer_config.json",
        max_bytes: 64 * 1024,
    },
];

const VISION_SPEC: TrustedModelSpec = TrustedModelSpec {
    name: "vision",
    repo: "Qdrant/clip-ViT-B-32-vision",
    revision: "e0c24ed0fa57fa3e4f97f30de74c51d944036ace",
    onnx_sha256: "c68d3d9a200ddd2a8c8a5510b576d4c94d1ae383bf8b36dd8c084f94e1fb4d63",
    onnx_size: 351_686_194,
    support: VISION_SUPPORT,
};
const TEXT_SPEC: TrustedModelSpec = TrustedModelSpec {
    name: "text",
    repo: "Qdrant/clip-ViT-B-32-text",
    revision: "48ca1db27cb4063eb311ec2aa7f087a808112876",
    onnx_sha256: "4dbe762b11e36488304471e439cde89da053ad7acaddbf9e096745d142ec8d8b",
    onnx_size: 254_102_519,
    support: TEXT_SUPPORT,
};

fn cache_dir(state: &AppState) -> PathBuf {
    state.models_dir.join("semantic")
}
fn trusted_dir(cache: &Path, spec: TrustedModelSpec) -> PathBuf {
    cache.join("trusted").join(spec.name)
}

fn dir_size(path: &Path) -> u64 {
    if !path.exists() {
        return 0;
    }
    WalkDir::new(path)
        .into_iter()
        .filter_map(Result::ok)
        .filter_map(|entry| {
            if entry.file_type().is_file() {
                entry.metadata().ok().map(|m| m.len())
            } else {
                None
            }
        })
        .sum()
}

fn setting(conn: &rusqlite::Connection, key: &str) -> String {
    conn.query_row(
        "SELECT value FROM semantic_settings WHERE key=?1",
        params![key],
        |r| r.get(0),
    )
    .unwrap_or_default()
}

fn set_setting(conn: &rusqlite::Connection, key: &str, value: &str) -> Result<(), String> {
    conn.execute(
        "INSERT INTO semantic_settings(key,value) VALUES(?1,?2)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        params![key, value],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn normalize(mut vector: Vec<f32>) -> Vec<f32> {
    let norm = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for value in &mut vector {
            *value /= norm
        }
    }
    vector
}

fn to_blob(vector: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(vector.len() * 4);
    for value in vector {
        out.extend_from_slice(&value.to_le_bytes())
    }
    out
}

fn from_blob(blob: &[u8]) -> Vec<f32> {
    blob.chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

fn score(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return -1.0;
    }
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 1024 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

fn validate_json(path: &Path, max_bytes: u64) -> Result<(), String> {
    let meta =
        fs::metadata(path).map_err(|e| format!("模型配套文件缺失 {}：{e}", path.display()))?;
    if meta.len() == 0 || meta.len() > max_bytes {
        return Err(format!(
            "模型配套文件大小异常：{} ({} bytes)",
            path.display(),
            meta.len()
        ));
    }
    let data = fs::read(path).map_err(|e| e.to_string())?;
    serde_json::from_slice::<serde_json::Value>(&data)
        .map_err(|e| format!("模型配套 JSON 无效 {}：{e}", path.display()))?;
    Ok(())
}

fn validate_trusted(dir: &Path, spec: TrustedModelSpec) -> Result<(), String> {
    let onnx = dir.join("model.onnx");
    let meta = fs::metadata(&onnx).map_err(|e| format!("可信模型缺失 {}：{e}", onnx.display()))?;
    if meta.len() != spec.onnx_size {
        return Err(format!(
            "模型大小校验失败：{}，期望 {} bytes，实际 {} bytes",
            spec.name,
            spec.onnx_size,
            meta.len()
        ));
    }
    let digest = sha256_file(&onnx)?;
    if digest != spec.onnx_sha256 {
        return Err(format!(
            "模型 SHA-256 校验失败：{}。已拒绝加载。",
            spec.name
        ));
    }
    for file in spec.support {
        validate_json(&dir.join(file.path), file.max_bytes)?
    }
    Ok(())
}

fn install_trusted(cache: &Path, spec: TrustedModelSpec) -> Result<PathBuf, String> {
    let target = trusted_dir(cache, spec);
    if validate_trusted(&target, spec).is_ok() {
        return Ok(target);
    }

    fs::create_dir_all(cache.join("trusted")).map_err(|e| e.to_string())?;
    let hub_cache = cache.join("hf");
    let api = ApiBuilder::new()
        .with_cache_dir(hub_cache.clone())
        .with_progress(false)
        .build()
        .map_err(|e| format!("无法初始化固定版本模型下载器：{e}"))?;
    let repo = api.repo(Repo::with_revision(
        spec.repo.to_string(),
        RepoType::Model,
        spec.revision.to_string(),
    ));

    let stage = cache.join(format!(".trusted-{}-{}", spec.name, std::process::id()));
    if stage.exists() {
        fs::remove_dir_all(&stage).map_err(|e| e.to_string())?
    }
    fs::create_dir_all(&stage).map_err(|e| e.to_string())?;

    let install_result = (|| -> Result<(), String> {
        let source_model = repo
            .get("model.onnx")
            .map_err(|e| format!("固定版本模型下载失败 {}@{}：{e}", spec.repo, spec.revision))?;
        let source_meta = fs::metadata(&source_model).map_err(|e| e.to_string())?;
        if source_meta.len() != spec.onnx_size {
            return Err(format!(
                "下载模型大小不符合可信清单：期望 {} bytes，实际 {} bytes",
                spec.onnx_size,
                source_meta.len()
            ));
        }
        let source_digest = sha256_file(&source_model)?;
        if source_digest != spec.onnx_sha256 {
            return Err(format!(
                "下载模型 SHA-256 与可信清单不一致：{}。已拒绝安装。",
                spec.name
            ));
        }
        fs::copy(&source_model, stage.join("model.onnx")).map_err(|e| e.to_string())?;

        for support in spec.support {
            let source = repo
                .get(support.path)
                .map_err(|e| format!("固定版本配套文件下载失败 {}：{e}", support.path))?;
            let meta = fs::metadata(&source).map_err(|e| e.to_string())?;
            if meta.len() == 0 || meta.len() > support.max_bytes {
                return Err(format!(
                    "配套文件大小超过安全上限：{} ({} bytes)",
                    support.path,
                    meta.len()
                ));
            }
            fs::copy(&source, stage.join(support.path)).map_err(|e| e.to_string())?;
            validate_json(&stage.join(support.path), support.max_bytes)?;
        }

        let manifest = serde_json::json!({
            "manifest_version":TRUST_MANIFEST_VERSION,
            "model_id":MODEL_ID,
            "component":spec.name,
            "repository":spec.repo,
            "revision":spec.revision,
            "onnx_sha256":spec.onnx_sha256,
            "onnx_size":spec.onnx_size
        });
        fs::write(
            stage.join("trusted-manifest.json"),
            serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        validate_trusted(&stage, spec)
    })();

    if let Err(error) = install_result {
        let _ = fs::remove_dir_all(&stage);
        let _ = fs::remove_dir_all(&hub_cache);
        return Err(error);
    }

    let backup = cache.join(format!(".trusted-{}-old", spec.name));
    if backup.exists() {
        let _ = fs::remove_dir_all(&backup);
    }
    if target.exists() {
        fs::rename(&target, &backup).map_err(|e| format!("无法替换旧模型缓存：{e}"))?
    }
    if let Err(error) = fs::rename(&stage, &target) {
        if backup.exists() {
            let _ = fs::rename(&backup, &target);
        }
        let _ = fs::remove_dir_all(&stage);
        return Err(format!("可信模型原子安装失败：{error}"));
    }
    if backup.exists() {
        let _ = fs::remove_dir_all(&backup);
    }
    let _ = fs::remove_dir_all(&hub_cache);
    Ok(target)
}

fn image_model(cache: PathBuf) -> Result<ImageEmbedding, String> {
    let dir = install_trusted(&cache, VISION_SPEC)?;
    let onnx = fs::read(dir.join("model.onnx")).map_err(|e| e.to_string())?;
    let preprocessor = fs::read(dir.join("preprocessor_config.json")).map_err(|e| e.to_string())?;
    let model = UserDefinedImageEmbeddingModel::new(onnx, preprocessor);
    ImageEmbedding::try_new_from_user_defined(
        model,
        ImageInitOptionsUserDefined::new().with_intra_threads(2),
    )
    .map_err(|e| format!("可信视觉模型加载失败：{e}"))
}

fn text_vector(cache: PathBuf, query: String) -> Result<Vec<f32>, String> {
    let dir = install_trusted(&cache, TEXT_SPEC)?;
    let tokenizer = TokenizerFiles {
        tokenizer_file: fs::read(dir.join("tokenizer.json")).map_err(|e| e.to_string())?,
        config_file: fs::read(dir.join("config.json")).map_err(|e| e.to_string())?,
        special_tokens_map_file: fs::read(dir.join("special_tokens_map.json"))
            .map_err(|e| e.to_string())?,
        tokenizer_config_file: fs::read(dir.join("tokenizer_config.json"))
            .map_err(|e| e.to_string())?,
    };
    let model = UserDefinedEmbeddingModel::new(
        fs::read(dir.join("model.onnx")).map_err(|e| e.to_string())?,
        tokenizer,
    )
    .with_pooling(Pooling::Mean);
    let mut model = TextEmbedding::try_new_from_user_defined(
        model,
        InitOptionsUserDefined::new()
            .with_max_length(77)
            .with_intra_threads(2),
    )
    .map_err(|e| format!("可信文本模型加载失败：{e}"))?;
    let mut rows = model
        .embed(vec![query], None)
        .map_err(|e| format!("语义查询编码失败：{e}"))?;
    let vector = rows.pop().ok_or("语义模型没有返回查询向量")?;
    Ok(normalize(vector))
}

fn emit(app: &AppHandle, progress: SemanticProgress) {
    let _ = app.emit("imagelore://semantic-progress", progress);
}

fn index_failure(app: &AppHandle, job_id: u64, total: usize, message: String) {
    emit(
        app,
        SemanticProgress {
            job_id,
            processed: 0,
            total,
            indexed: 0,
            skipped: 0,
            failed: 1,
            current_name: message,
            done: true,
            cancelled: false,
        },
    );
}

#[tauri::command]
pub fn semantic_status(state: State<'_, AppState>) -> Result<SemanticStatus, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let enabled = setting(&conn, "enabled") == "1";
    let vision_ready = setting(&conn, "vision_ready") == "1";
    let text_ready = setting(&conn, "text_ready") == "1";
    let total: i64 = conn
        .query_row("SELECT COUNT(*) FROM assets WHERE missing=0", [], |r| {
            r.get(0)
        })
        .unwrap_or(0);
    let indexed: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM semantic_embeddings WHERE model_id=?1",
            params![MODEL_ID],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let stale:i64=conn.query_row(
        "SELECT COUNT(*) FROM assets a
         LEFT JOIN semantic_embeddings se ON se.asset_id=a.id
         WHERE a.missing=0 AND (se.asset_id IS NULL OR se.model_id<>?1 OR se.fingerprint<>a.fingerprint)",
        params![MODEL_ID],|r|r.get(0)
    ).unwrap_or(total);
    let index_bytes: i64 = conn
        .query_row(
            "SELECT COALESCE(SUM(LENGTH(vector)),0) FROM semantic_embeddings WHERE model_id=?1",
            params![MODEL_ID],
            |r| r.get(0),
        )
        .unwrap_or(0);
    drop(conn);
    let model_bytes = dir_size(&cache_dir(state.inner()));
    Ok(SemanticStatus {
        model_id: MODEL_ID.into(),
        enabled,
        model_ready: vision_ready && text_ready,
        vision_ready,
        text_ready,
        indexed,
        total,
        stale,
        model_bytes,
        index_bytes: index_bytes.max(0) as u64,
    })
}

#[tauri::command]
pub fn start_semantic_index(app: AppHandle) -> Result<u64, String> {
    let state = app.state::<AppState>();
    let (job_id, cancel) = jobs::register(state.inner())?;
    let cache = cache_dir(state.inner());

    let rows: Vec<(i64, String, String, String)> = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO semantic_settings(key,value) VALUES('enabled','1')
             ON CONFLICT(key) DO UPDATE SET value='1'",
            [],
        )
        .map_err(|e| e.to_string())?;
        let mut st=conn.prepare(
            "SELECT a.id,a.path,a.name,a.fingerprint
             FROM assets a
             LEFT JOIN semantic_embeddings se ON se.asset_id=a.id
             WHERE a.missing=0 AND (se.asset_id IS NULL OR se.model_id<>?1 OR se.fingerprint<>a.fingerprint)
             ORDER BY a.id"
        ).map_err(|e|e.to_string())?;
        let mapped = st
            .query_map(params![MODEL_ID], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
            })
            .map_err(|e| e.to_string())?;
        mapped.filter_map(Result::ok).collect()
    };
    let total = rows.len();
    let app_for_thread = app.clone();

    std::thread::spawn(move || {
        emit(
            &app_for_thread,
            SemanticProgress {
                job_id,
                processed: 0,
                total,
                indexed: 0,
                skipped: 0,
                failed: 0,
                current_name: "正在准备本地视觉模型…".into(),
                done: false,
                cancelled: false,
            },
        );
        let mut model = match image_model(cache) {
            Ok(model) => {
                let state = app_for_thread.state::<AppState>();
                if let Ok(conn) = state.db.lock() {
                    let _ = set_setting(&conn, "vision_ready", "1");
                }
                model
            }
            Err(error) => {
                let state = app_for_thread.state::<AppState>();
                if let Ok(conn) = state.db.lock() {
                    let _ = set_setting(&conn, "vision_ready", "0");
                }
                index_failure(&app_for_thread, job_id, total, error);
                jobs::finish(state.inner(), job_id);
                return;
            }
        };

        let mut indexed = 0i64;
        let skipped = 0i64;
        let mut failed = 0i64;
        let mut processed = 0usize;

        for (id, path, name, fingerprint) in rows {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            let image_path = PathBuf::from(&path);
            if !image_path.exists() {
                failed += 1;
                processed += 1;
                emit(
                    &app_for_thread,
                    SemanticProgress {
                        job_id,
                        processed,
                        total,
                        indexed,
                        skipped,
                        failed,
                        current_name: name,
                        done: false,
                        cancelled: false,
                    },
                );
                continue;
            }
            match model.embed(vec![image_path], None) {
                Ok(mut vectors) if !vectors.is_empty() => {
                    let vector = normalize(vectors.remove(0));
                    if vector.len() != DIMENSIONS {
                        failed += 1;
                    } else {
                        let blob = to_blob(&vector);
                        let state = app_for_thread.state::<AppState>();
                        let result=state.db.lock().map_err(|e|e.to_string()).and_then(|conn|{
                            conn.execute(
                                "INSERT INTO semantic_embeddings(asset_id,model_id,dimensions,vector,fingerprint,indexed_at)
                                 VALUES(?1,?2,?3,?4,?5,?6)
                                 ON CONFLICT(asset_id) DO UPDATE SET
                                   model_id=excluded.model_id,dimensions=excluded.dimensions,vector=excluded.vector,
                                   fingerprint=excluded.fingerprint,indexed_at=excluded.indexed_at",
                                params![id,MODEL_ID,DIMENSIONS as i64,blob,fingerprint,db::now()]
                            ).map_err(|e|e.to_string())?;
                            Ok(())
                        });
                        if result.is_ok() {
                            indexed += 1
                        } else {
                            failed += 1
                        }
                    }
                }
                Ok(_) => failed += 1,
                Err(_) => failed += 1,
            }
            processed += 1;
            emit(
                &app_for_thread,
                SemanticProgress {
                    job_id,
                    processed,
                    total,
                    indexed,
                    skipped,
                    failed,
                    current_name: name,
                    done: false,
                    cancelled: false,
                },
            );
        }

        let cancelled = cancel.load(Ordering::Relaxed);
        let state = app_for_thread.state::<AppState>();
        if let Ok(conn) = state.db.lock() {
            let _ = conn.execute(
                "DELETE FROM semantic_embeddings
                 WHERE asset_id IN (SELECT id FROM assets WHERE missing<>0) OR model_id<>?1",
                params![MODEL_ID],
            );
        }
        emit(
            &app_for_thread,
            SemanticProgress {
                job_id,
                processed,
                total,
                indexed,
                skipped,
                failed,
                current_name: String::new(),
                done: true,
                cancelled,
            },
        );
        jobs::finish(state.inner(), job_id);
    });

    Ok(job_id)
}

#[tauri::command]
pub fn cancel_semantic_index(state: State<'_, AppState>, job_id: u64) -> Result<bool, String> {
    jobs::cancel(state.inner(), job_id)
}

fn ranked(
    state: &AppState,
    query: &[f32],
    mut filter: LibraryFilter,
    limit: i64,
    exclude: Option<i64>,
) -> Result<Vec<SemanticHit>, String> {
    filter.query.clear();
    let candidates = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        search::filtered_summaries(&conn, &filter, 100_000)?
    };
    let allowed: HashSet<i64> = candidates.iter().map(|x| x.id).collect();
    let assets: HashMap<i64, _> = candidates.into_iter().map(|x| (x.id, x)).collect();

    let rows: Vec<(i64, Vec<u8>)> = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        let mut st=conn.prepare(
            "SELECT asset_id,vector FROM semantic_embeddings WHERE model_id=?1 AND dimensions=?2"
        ).map_err(|e|e.to_string())?;
        let mapped = st
            .query_map(params![MODEL_ID, DIMENSIONS as i64], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .map_err(|e| e.to_string())?;
        mapped.filter_map(Result::ok).collect()
    };

    let mut hits = Vec::new();
    for (id, blob) in rows {
        if exclude == Some(id) || !allowed.contains(&id) {
            continue;
        }
        let vector = from_blob(&blob);
        let similarity = score(query, &vector);
        if !similarity.is_finite() {
            continue;
        }
        if let Some(asset) = assets.get(&id) {
            hits.push(SemanticHit {
                asset: asset.clone(),
                score: similarity,
            })
        }
    }
    hits.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| b.asset.id.cmp(&a.asset.id))
    });
    hits.truncate(limit.clamp(1, 500) as usize);
    Ok(hits)
}

#[tauri::command]
pub async fn semantic_search_text(
    state: State<'_, AppState>,
    query: String,
    filter: LibraryFilter,
    limit: i64,
) -> Result<Vec<SemanticHit>, String> {
    let query = query.trim().to_string();
    if query.is_empty() {
        return Ok(Vec::new());
    }
    let indexed: i64 = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        conn.query_row(
            "SELECT COUNT(*) FROM semantic_embeddings WHERE model_id=?1",
            params![MODEL_ID],
            |r| r.get(0),
        )
        .unwrap_or(0)
    };
    if indexed == 0 {
        return Err("请先在资料库管理中建立语义索引".into());
    }
    let cache = cache_dir(state.inner());
    let vector = tauri::async_runtime::spawn_blocking(move || text_vector(cache, query))
        .await
        .map_err(|e| e.to_string())??;
    {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        set_setting(&conn, "text_ready", "1")?;
    }
    ranked(state.inner(), &vector, filter, limit, None)
}

#[tauri::command]
pub fn semantic_search_similar(
    state: State<'_, AppState>,
    asset_id: i64,
    filter: LibraryFilter,
    limit: i64,
) -> Result<Vec<SemanticHit>, String> {
    let blob: Option<Vec<u8>> = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        conn.query_row(
            "SELECT vector FROM semantic_embeddings WHERE asset_id=?1 AND model_id=?2 AND dimensions=?3",
            params![asset_id,MODEL_ID,DIMENSIONS as i64],|r|r.get(0)
        ).optional().map_err(|e|e.to_string())?
    };
    let blob = blob.ok_or("当前图片尚未建立语义索引")?;
    ranked(
        state.inner(),
        &from_blob(&blob),
        filter,
        limit,
        Some(asset_id),
    )
}

#[tauri::command]
pub fn clear_semantic_index(state: State<'_, AppState>) -> Result<bool, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM semantic_embeddings", [])
        .map_err(|e| e.to_string())?;
    set_setting(&conn, "enabled", "0")?;
    Ok(true)
}

#[tauri::command]
pub fn delete_semantic_models(state: State<'_, AppState>) -> Result<bool, String> {
    let path = cache_dir(state.inner());
    if path.exists() {
        fs::remove_dir_all(&path).map_err(|e| e.to_string())?
    }
    fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    set_setting(&conn, "vision_ready", "0")?;
    set_setting(&conn, "text_ready", "0")?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_file_matches_known_digest() {
        let path =
            std::env::temp_dir().join(format!("imagelore-semantic-sha-{}.bin", std::process::id()));
        fs::write(&path, b"abc").unwrap();
        let digest = sha256_file(&path).unwrap();
        let _ = fs::remove_file(&path);
        assert_eq!(
            digest,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn trusted_model_rejects_wrong_digest() {
        static EMPTY: &[SupportFile] = &[];
        let root =
            std::env::temp_dir().join(format!("imagelore-semantic-trust-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("model.onnx"), b"abc").unwrap();
        let spec = TrustedModelSpec {
            name: "test",
            repo: "unused",
            revision: "unused",
            onnx_sha256: "0000000000000000000000000000000000000000000000000000000000000000",
            onnx_size: 3,
            support: EMPTY,
        };
        assert!(validate_trusted(&root, spec)
            .unwrap_err()
            .contains("SHA-256"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn vector_blob_round_trip_and_cosine() {
        let source = normalize(vec![1.0, 2.0, 3.0, 4.0]);
        let decoded = from_blob(&to_blob(&source));
        assert_eq!(decoded.len(), source.len());
        assert!((score(&source, &decoded) - 1.0).abs() < 0.0001);
    }
}
