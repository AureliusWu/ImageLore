use crate::{
    db, jobs,
    models::{LibraryFilter, SemanticHit, SemanticProgress, SemanticStatus},
    search,
    state::AppState,
};
use fastembed::{
    ImageEmbedding, ImageInitOptionsUserDefined, InitOptionsUserDefined, Pooling, TextEmbedding,
    TokenizerFiles, UserDefinedEmbeddingModel, UserDefinedImageEmbeddingModel,
};
mod download;
use download::{Control, ModelError, ModelResult};
use rusqlite::{params, types::ValueRef, OptionalExtension};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex, MutexGuard, OnceLock, Weak,
    },
    time::Duration,
};
use tauri::{AppHandle, Emitter, Manager, State};
use walkdir::WalkDir;

pub const MODEL_ID: &str = "clip-vit-b32-qdrant-v1";
const DIMENSIONS: usize = 512;
// Allow float32 rounding while rejecting zero/nonfinite/unnormalized data.
// Actual model output is checked independently during native acceptance.
const VECTOR_NORM_TOLERANCE: f64 = 0.001;
const TRUST_MANIFEST_VERSION: u32 = 1;
static MODEL_LOCKS: OnceLock<Mutex<HashMap<PathBuf, Weak<Mutex<()>>>>> = OnceLock::new();
static STAGE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn model_operation(cache: &Path) -> ModelResult<Arc<Mutex<()>>> {
    let mut locks = MODEL_LOCKS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .map_err(|e| ModelError::Failed(e.to_string()))?;
    locks.retain(|_, value| value.strong_count() > 0);
    if let Some(lock) = locks.get(cache).and_then(Weak::upgrade) {
        return Ok(lock);
    }
    let lock = Arc::new(Mutex::new(()));
    locks.insert(cache.to_path_buf(), Arc::downgrade(&lock));
    Ok(lock)
}

fn model_guard<'a, T>(lock: &'a Mutex<T>, control: &Control<'_>) -> ModelResult<MutexGuard<'a, T>> {
    loop {
        control.check()?;
        match lock.try_lock() {
            Ok(guard) => {
                control.check()?;
                return Ok(guard);
            }
            Err(std::sync::TryLockError::WouldBlock) => {
                std::thread::sleep(Duration::from_millis(50))
            }
            Err(error) => return Err(ModelError::Failed(error.to_string())),
        }
    }
}

fn unique_stage(cache: &Path, label: &str) -> ModelResult<PathBuf> {
    loop {
        let path = cache.join(format!(
            ".trusted-{}-{}-{}",
            label,
            std::process::id(),
            STAGE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        match fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
}

fn discard_stage(stage: &Path, error: ModelError) -> ModelError {
    if let Err(cleanup) = fs::remove_dir_all(stage) {
        if matches!(error, ModelError::Cancelled) {
            // Preserve the cancellation result. An OS cleanup failure leaves
            // only this unique, unpublished stage; it cannot be loaded.
            eprintln!(
                "cancelled model stage cleanup failed {}: {cleanup}",
                stage.display()
            );
        } else {
            return ModelError::Failed(format!("{error}；临时模型文件清理失败，请重试：{cleanup}"));
        }
    }
    error
}

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

fn cache_generation(conn: &rusqlite::Connection) -> Result<u64, String> {
    let value: Option<String> = conn
        .query_row(
            "SELECT value FROM semantic_settings WHERE key='cache_generation'",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    match value {
        None => Ok(0),
        Some(value) => value
            .parse()
            .map_err(|_| "语义模型状态版本无效，已保留模型缓存".into()),
    }
}

fn publish_readiness(
    conn: &mut rusqlite::Connection,
    key: &str,
    generation: u64,
    ready: bool,
) -> Result<(), String> {
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    if cache_generation(&tx)? == generation {
        set_setting(&tx, key, if ready { "1" } else { "0" })?;
    }
    tx.commit().map_err(|e| e.to_string())
}

fn invalidate_model_readiness(conn: &mut rusqlite::Connection) -> Result<(), String> {
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let next = cache_generation(&tx)?
        .checked_add(1)
        .ok_or("语义模型状态版本已达上限，已保留模型缓存")?;
    set_setting(&tx, "vision_ready", "0")?;
    set_setting(&tx, "text_ready", "0")?;
    set_setting(&tx, "cache_generation", &next.to_string())?;
    tx.commit().map_err(|e| e.to_string())
}

fn validate_vector(vector: &[f32]) -> Result<(), String> {
    if vector.len() != DIMENSIONS {
        return Err("语义向量必须包含 512 个数值，请重新建立语义索引".into());
    }
    if vector.iter().any(|value| !value.is_finite()) {
        return Err("语义向量包含非有限数值，请重新建立语义索引".into());
    }
    let norm = vector
        .iter()
        .map(|&value| {
            let value = f64::from(value);
            value * value
        })
        .sum::<f64>()
        .sqrt();
    if norm == 0.0 || !norm.is_finite() || (norm - 1.0).abs() >= VECTOR_NORM_TOLERANCE {
        return Err("语义向量未正确归一化，请重新建立语义索引".into());
    }
    Ok(())
}

fn normalize(mut vector: Vec<f32>) -> Result<Vec<f32>, String> {
    if vector.len() != DIMENSIONS || vector.iter().any(|value| !value.is_finite()) {
        return Err("语义模型返回了无效向量".into());
    }
    // Cast before multiplication: large finite float32 outputs must not
    // overflow a float32 sum and silently normalize into an all-zero vector.
    let norm = vector
        .iter()
        .map(|&value| {
            let value = f64::from(value);
            value * value
        })
        .sum::<f64>()
        .sqrt();
    if norm == 0.0 || !norm.is_finite() {
        return Err("语义模型返回了零向量或无效范数".into());
    }
    for value in &mut vector {
        *value = (f64::from(*value) / norm) as f32;
    }
    validate_vector(&vector)?;
    Ok(vector)
}

fn to_blob(vector: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(vector.len() * 4);
    for value in vector {
        out.extend_from_slice(&value.to_le_bytes())
    }
    out
}

fn from_blob(blob: &[u8]) -> Result<[f32; DIMENSIONS], String> {
    if blob.len() != DIMENSIONS * 4 {
        return Err("语义索引向量必须为 2048 字节，请重新建立语义索引".into());
    }
    let mut vector = [0.0; DIMENSIONS];
    for (value, chunk) in vector.iter_mut().zip(blob.as_chunks::<4>().0) {
        *value = f32::from_le_bytes(*chunk);
    }
    validate_vector(&vector)?;
    Ok(vector)
}

fn score(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return -1.0;
    }
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

#[cfg(test)]
fn sha256_file(path: &Path) -> Result<String, String> {
    download::digest(
        path,
        &Control {
            cancel: None,
            report: &|_| {},
        },
    )
    .map_err(|e| e.to_string())
}

fn validate_json(path: &Path, max_bytes: u64, control: &Control<'_>) -> ModelResult<()> {
    control.check()?;
    let meta = fs::metadata(path)
        .map_err(|e| ModelError::Failed(format!("模型配套文件缺失 {}：{e}", path.display())))?;
    if meta.len() == 0 || meta.len() > max_bytes {
        return Err(ModelError::Failed(format!(
            "模型配套文件大小异常：{} ({} bytes)",
            path.display(),
            meta.len()
        )));
    }
    let data = download::read(path, max_bytes, control)?;
    serde_json::from_slice::<serde_json::Value>(&data)
        .map_err(|e| ModelError::Failed(format!("模型配套 JSON 无效 {}：{e}", path.display())))?;
    control.check()
}

fn validate_trusted_controlled(
    dir: &Path,
    spec: TrustedModelSpec,
    control: &Control<'_>,
) -> ModelResult<()> {
    control.check()?;
    let onnx = dir.join("model.onnx");
    let meta = fs::metadata(&onnx)
        .map_err(|e| ModelError::Failed(format!("可信模型缺失 {}：{e}", onnx.display())))?;
    if meta.len() != spec.onnx_size {
        return Err(ModelError::Failed(format!(
            "模型大小校验失败：{}，期望 {} bytes，实际 {} bytes",
            spec.name,
            spec.onnx_size,
            meta.len()
        )));
    }
    control.phase("正在准备本地模型：完整核验 SHA-256…")?;
    if download::digest(&onnx, control)? != spec.onnx_sha256 {
        return Err(ModelError::Failed(format!(
            "模型 SHA-256 校验失败：{}。已拒绝加载。",
            spec.name
        )));
    }
    for file in spec.support {
        validate_json(&dir.join(file.path), file.max_bytes, control)?;
    }
    control.check()
}

#[cfg(test)]
fn validate_trusted(dir: &Path, spec: TrustedModelSpec) -> Result<(), String> {
    validate_trusted_controlled(
        dir,
        spec,
        &Control {
            cancel: None,
            report: &|_| {},
        },
    )
    .map_err(|e| e.to_string())
}

fn install_trusted_with(
    cache: &Path,
    spec: TrustedModelSpec,
    control: &Control<'_>,
    fetch: impl Fn(&str, &Path, u64, Option<u64>) -> ModelResult<()>,
) -> ModelResult<PathBuf> {
    control.check()?;
    let target = trusted_dir(cache, spec);
    match validate_trusted_controlled(&target, spec, control) {
        Ok(()) => return Ok(target),
        Err(ModelError::Cancelled) => return Err(ModelError::Cancelled),
        Err(ModelError::Failed(_)) => {}
    }
    control.check()?;
    fs::create_dir_all(cache.join("trusted"))?;
    let stage = unique_stage(cache, spec.name)?;
    let result = (|| -> ModelResult<()> {
        control.phase("正在准备本地模型：下载固定公开版本…")?;
        fetch(
            "model.onnx",
            &stage.join("model.onnx"),
            spec.onnx_size,
            Some(spec.onnx_size),
        )?;
        for support in spec.support {
            control.check()?;
            fetch(
                support.path,
                &stage.join(support.path),
                support.max_bytes,
                None,
            )?;
            validate_json(&stage.join(support.path), support.max_bytes, control)?;
        }
        let manifest = serde_json::json!({"manifest_version":TRUST_MANIFEST_VERSION,"model_id":MODEL_ID,"component":spec.name,"repository":spec.repo,"revision":spec.revision,"onnx_sha256":spec.onnx_sha256,"onnx_size":spec.onnx_size});
        fs::write(
            stage.join("trusted-manifest.json"),
            serde_json::to_vec_pretty(&manifest).map_err(|e| ModelError::Failed(e.to_string()))?,
        )?;
        validate_trusted_controlled(&stage, spec, control)
    })();
    if let Err(error) = result {
        // The fetch future and its output file have already been dropped. Only
        // this invocation's unique stage is removed, never another download.
        return Err(discard_stage(&stage, error));
    }
    if let Err(error) = control.check() {
        return Err(discard_stage(&stage, error));
    }
    let backup = match unique_stage(cache, &format!("{}-old", spec.name)) {
        Ok(path) => path,
        Err(error) => return Err(discard_stage(&stage, error)),
    };
    if let Err(error) = fs::remove_dir(&backup) {
        return Err(discard_stage(&stage, error.into()));
    }
    if target.exists() {
        if let Err(error) = fs::rename(&target, &backup) {
            let _ = fs::remove_dir_all(&stage);
            return Err(ModelError::Failed(format!("无法替换旧模型缓存：{error}")));
        }
    }
    // Do not interrupt the rename/rollback critical section. A cancel arriving
    // here may keep a completely verified cache, but cannot start encoding.
    if let Err(error) = fs::rename(&stage, &target) {
        if backup.exists() {
            if let Err(rollback) = fs::rename(&backup, &target) {
                return Err(ModelError::Failed(format!(
                    "可信模型安装失败：{error}；旧模型保留于 {}，回滚失败：{rollback}",
                    backup.display()
                )));
            }
        }
        let _ = fs::remove_dir_all(&stage);
        return Err(ModelError::Failed(format!("可信模型原子安装失败：{error}")));
    }
    if backup.exists() {
        let _ = fs::remove_dir_all(&backup);
    }
    control.check()?;
    Ok(target)
}

fn install_trusted(
    cache: &Path,
    spec: TrustedModelSpec,
    control: &Control<'_>,
) -> ModelResult<PathBuf> {
    // A valid private trusted cache takes the network-free branch before the
    // fetch closure is invoked. No HF builder, global cache or token is read.
    let client = std::cell::OnceCell::new();
    install_trusted_with(
        cache,
        spec,
        control,
        |name, destination, max_bytes, exact_size| {
            if client.get().is_none() {
                client
                    .set(download::client()?)
                    .map_err(|_| ModelError::Failed("模型下载器初始化失败".into()))?;
            }
            let url = format!(
                "https://huggingface.co/{}/resolve/{}/{}",
                spec.repo, spec.revision, name
            );
            tauri::async_runtime::block_on(download::file(
                client.get().unwrap(),
                &url,
                destination,
                max_bytes,
                exact_size,
                control,
            ))
        },
    )
}

fn image_model(cache: PathBuf, control: &Control<'_>) -> ModelResult<ImageEmbedding> {
    let operation = model_operation(&cache)?;
    let guard = model_guard(&operation, control)?;
    let dir = install_trusted(&cache, VISION_SPEC, control)?;
    control.phase("正在准备本地视觉模型：读取可信文件…")?;
    let onnx = download::read(&dir.join("model.onnx"), VISION_SPEC.onnx_size, control)?;
    let preprocessor = download::read(
        &dir.join("preprocessor_config.json"),
        VISION_SUPPORT[0].max_bytes,
        control,
    )?;
    drop(guard);
    control.phase("正在准备本地视觉模型：初始化 ONNX，取消将在本阶段结束后生效…")?;
    let model = ImageEmbedding::try_new_from_user_defined(
        UserDefinedImageEmbeddingModel::new(onnx, preprocessor),
        ImageInitOptionsUserDefined::new().with_intra_threads(2),
    )
    .map_err(|e| ModelError::Failed(format!("可信视觉模型加载失败：{e}")))?;
    control.check()?;
    Ok(model)
}

fn text_vector(cache: PathBuf, query: String) -> Result<Vec<f32>, String> {
    text_vector_controlled(
        cache,
        query,
        &Control {
            cancel: None,
            report: &|_| {},
        },
    )
    .map_err(|e| e.to_string())
}

fn text_vector_controlled(
    cache: PathBuf,
    query: String,
    control: &Control<'_>,
) -> ModelResult<Vec<f32>> {
    let operation = model_operation(&cache)?;
    let guard = model_guard(&operation, control)?;
    let dir = install_trusted(&cache, TEXT_SPEC, control)?;
    let tokenizer = TokenizerFiles {
        tokenizer_file: download::read(
            &dir.join("tokenizer.json"),
            TEXT_SUPPORT[0].max_bytes,
            control,
        )?,
        config_file: download::read(&dir.join("config.json"), TEXT_SUPPORT[1].max_bytes, control)?,
        special_tokens_map_file: download::read(
            &dir.join("special_tokens_map.json"),
            TEXT_SUPPORT[2].max_bytes,
            control,
        )?,
        tokenizer_config_file: download::read(
            &dir.join("tokenizer_config.json"),
            TEXT_SUPPORT[3].max_bytes,
            control,
        )?,
    };
    let onnx = download::read(&dir.join("model.onnx"), TEXT_SPEC.onnx_size, control)?;
    drop(guard);
    control.check()?;
    let mut model = TextEmbedding::try_new_from_user_defined(
        UserDefinedEmbeddingModel::new(onnx, tokenizer).with_pooling(Pooling::Mean),
        InitOptionsUserDefined::new()
            .with_max_length(77)
            .with_intra_threads(2),
    )
    .map_err(|e| ModelError::Failed(format!("可信文本模型加载失败：{e}")))?;
    let mut rows = model
        .embed(vec![query], None)
        .map_err(|e| ModelError::Failed(format!("语义查询编码失败：{e}")))?;
    let vector = rows
        .pop()
        .ok_or_else(|| ModelError::Failed("语义模型没有返回查询向量".into()))?;
    normalize(vector).map_err(ModelError::Failed)
}

fn emit(app: &AppHandle, progress: SemanticProgress) {
    let _ = app.emit("imagelore://semantic-progress", progress);
}

fn index_failure(app: &AppHandle, job_id: u64, total: usize, error: ModelError) {
    let cancelled = matches!(error, ModelError::Cancelled);
    let state = app.state::<AppState>();
    jobs::finish(state.inner(), job_id);
    emit(
        app,
        SemanticProgress {
            job_id,
            processed: 0,
            total,
            indexed: 0,
            skipped: 0,
            failed: if cancelled { 0 } else { 1 },
            current_name: error.to_string(),
            done: true,
            cancelled,
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
        .map_err(|e| e.to_string())?;
    let indexed: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM semantic_embeddings WHERE model_id=?1",
            params![MODEL_ID],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let stale = stale_count(&conn)?;
    let index_bytes: i64 = conn
        .query_row(
            "SELECT COALESCE(SUM(LENGTH(vector)),0) FROM semantic_embeddings WHERE model_id=?1",
            params![MODEL_ID],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
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

type IndexRow = (i64, String, String, String);

fn current_embedding(
    row: &rusqlite::Row<'_>,
    fingerprint: &str,
    first_column: usize,
) -> rusqlite::Result<bool> {
    if fingerprint.is_empty()
        || !matches!(row.get_ref(first_column)?,ValueRef::Text(value) if value==MODEL_ID.as_bytes())
        || !matches!(row.get_ref(first_column+1)?,ValueRef::Integer(value) if value==DIMENSIONS as i64)
        || !matches!(row.get_ref(first_column+3)?,ValueRef::Text(value) if value==fingerprint.as_bytes())
    {
        return Ok(false);
    }
    Ok(matches!(row.get_ref(first_column+2)?,ValueRef::Blob(blob) if from_blob(blob).is_ok()))
}

fn stale_count(conn: &rusqlite::Connection) -> Result<i64, String> {
    // Correct freshness requires reading every present vector, including
    // finite/unit norm checks. Borrow each row's blob; do not copy the whole
    // index or collect an unbounded list of invalid IDs.
    let mut statement = conn
        .prepare(
            "SELECT a.fingerprint,se.model_id,se.dimensions,se.vector,se.fingerprint
         FROM assets a LEFT JOIN semantic_embeddings se ON se.asset_id=a.id
         WHERE a.missing=0",
        )
        .map_err(|e| e.to_string())?;
    let mut rows = statement.query([]).map_err(|e| e.to_string())?;
    let mut stale = 0;
    while let Some(row) = rows.next().map_err(|e| e.to_string())? {
        let fingerprint: String = row.get(0).map_err(|e| e.to_string())?;
        if !current_embedding(row, &fingerprint, 1).map_err(|e| e.to_string())? {
            stale += 1;
        }
    }
    Ok(stale)
}

fn index_rows(state: &AppState, cancel: &AtomicBool) -> ModelResult<(Vec<IndexRow>, u64)> {
    let control = Control {
        cancel: Some(cancel),
        report: &|_| {},
    };
    let conn = model_guard(&state.db, &control)?;
    let generation = cache_generation(&conn).map_err(ModelError::Failed)?;
    conn.execute("INSERT INTO semantic_settings(key,value) VALUES('enabled','1') ON CONFLICT(key) DO UPDATE SET value='1'",[]).map_err(|e|ModelError::Failed(e.to_string()))?;
    control.check()?;
    let mut statement=conn.prepare(
        "SELECT a.id,a.path,a.name,a.fingerprint,se.model_id,se.dimensions,se.vector,se.fingerprint
         FROM assets a LEFT JOIN semantic_embeddings se ON se.asset_id=a.id
         WHERE a.missing=0 ORDER BY a.id"
    ).map_err(|e|ModelError::Failed(e.to_string()))?;
    let mut mapped = statement
        .query([])
        .map_err(|e| ModelError::Failed(e.to_string()))?;
    let mut rows = Vec::new();
    while let Some(row) = mapped
        .next()
        .map_err(|e| ModelError::Failed(e.to_string()))?
    {
        control.check()?;
        let fingerprint: String = row.get(3).map_err(|e| ModelError::Failed(e.to_string()))?;
        if !current_embedding(row, &fingerprint, 4)
            .map_err(|e| ModelError::Failed(e.to_string()))?
        {
            rows.push((
                row.get(0).map_err(|e| ModelError::Failed(e.to_string()))?,
                row.get(1).map_err(|e| ModelError::Failed(e.to_string()))?,
                row.get(2).map_err(|e| ModelError::Failed(e.to_string()))?,
                fingerprint,
            ));
        }
    }
    control.check()?;
    Ok((rows, generation))
}

fn store_index_vector(
    state: &AppState,
    id: i64,
    blob: &[u8],
    fingerprint: &str,
    control: &Control<'_>,
) -> ModelResult<()> {
    control.check()?;
    from_blob(blob).map_err(ModelError::Failed)?;
    if fingerprint.is_empty() {
        return Err(ModelError::Failed("语义索引缺少当前图片指纹".into()));
    }
    let conn = model_guard(&state.db, control)?;
    control.check()?;
    conn.execute(
        "INSERT INTO semantic_embeddings(asset_id,model_id,dimensions,vector,fingerprint,indexed_at)
         VALUES(?1,?2,?3,?4,?5,?6)
         ON CONFLICT(asset_id) DO UPDATE SET
           model_id=excluded.model_id,dimensions=excluded.dimensions,vector=excluded.vector,
           fingerprint=excluded.fingerprint,indexed_at=excluded.indexed_at",
        params![id, MODEL_ID, DIMENSIONS as i64, blob, fingerprint, db::now()],
    )
    .map_err(|e| ModelError::Failed(e.to_string()))?;
    // A completed SQLite write counts as indexed even if cancellation arrives
    // during it. The next boundary stops work; no terminal is emitted early.
    Ok(())
}

#[tauri::command]
pub fn start_semantic_index(app: AppHandle) -> Result<u64, String> {
    let state = app.state::<AppState>();
    let (job_id, cancel) = jobs::register(state.inner())?;
    let cache = cache_dir(state.inner());

    let app_for_thread = app.clone();

    std::thread::spawn(move || {
        emit(
            &app_for_thread,
            SemanticProgress {
                job_id,
                processed: 0,
                total: 0,
                indexed: 0,
                skipped: 0,
                failed: 0,
                current_name: "正在准备语义索引：读取待编码资产…".into(),
                done: false,
                cancelled: false,
            },
        );
        let (rows, generation) =
            match index_rows(app_for_thread.state::<AppState>().inner(), &cancel) {
                Ok(prepared) => prepared,
                Err(error) => {
                    index_failure(&app_for_thread, job_id, 0, error);
                    return;
                }
            };
        let total = rows.len();

        if total == 0 {
            let state = app_for_thread.state::<AppState>();
            jobs::finish(state.inner(), job_id);
            emit(
                &app_for_thread,
                SemanticProgress {
                    job_id,
                    processed: 0,
                    total: 0,
                    indexed: 0,
                    skipped: 0,
                    failed: 0,
                    current_name: String::new(),
                    done: true,
                    cancelled: cancel.load(Ordering::Relaxed),
                },
            );
            return;
        }
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
        let report = |message: &str| {
            emit(
                &app_for_thread,
                SemanticProgress {
                    job_id,
                    processed: 0,
                    total,
                    indexed: 0,
                    skipped: 0,
                    failed: 0,
                    current_name: message.into(),
                    done: false,
                    cancelled: false,
                },
            )
        };
        let control = Control {
            cancel: Some(&cancel),
            report: &report,
        };
        let mut model = match image_model(cache, &control) {
            Ok(model) => {
                let state = app_for_thread.state::<AppState>();
                match model_guard(&state.db, &control) {
                    Ok(mut conn) => {
                        let _ = publish_readiness(&mut conn, "vision_ready", generation, true);
                    }
                    Err(ModelError::Cancelled) => {
                        drop(model);
                        index_failure(&app_for_thread, job_id, total, ModelError::Cancelled);
                        return;
                    }
                    Err(ModelError::Failed(_)) => {}
                }
                model
            }
            Err(error) => {
                let state = app_for_thread.state::<AppState>();
                if !matches!(error, ModelError::Cancelled) {
                    match model_guard(&state.db, &control) {
                        Ok(mut conn) => {
                            let _ = publish_readiness(&mut conn, "vision_ready", generation, false);
                        }
                        Err(ModelError::Cancelled) => {
                            index_failure(&app_for_thread, job_id, total, ModelError::Cancelled);
                            return;
                        }
                        Err(ModelError::Failed(_)) => {}
                    }
                }
                index_failure(&app_for_thread, job_id, total, error);
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
            emit(
                &app_for_thread,
                SemanticProgress {
                    job_id,
                    processed,
                    total,
                    indexed,
                    skipped,
                    failed,
                    current_name: format!(
                        "{} · 正在编码当前图片，取消将在本张处理结束后生效",
                        name
                    ),
                    done: false,
                    cancelled: false,
                },
            );
            match model.embed(vec![image_path], None) {
                Ok(mut vectors) if !vectors.is_empty() => {
                    if cancel.load(Ordering::Relaxed) {
                        break;
                    }
                    if let Ok(vector) = normalize(vectors.remove(0)) {
                        let blob = to_blob(&vector);
                        let state = app_for_thread.state::<AppState>();
                        match store_index_vector(state.inner(), id, &blob, &fingerprint, &control) {
                            Ok(()) => indexed += 1,
                            Err(ModelError::Cancelled) => break,
                            Err(ModelError::Failed(_)) => failed += 1,
                        }
                    } else {
                        failed += 1;
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

        drop(model);
        let state = app_for_thread.state::<AppState>();
        if let Ok(conn) = model_guard(&state.db, &control) {
            let _ = conn.execute(
                "DELETE FROM semantic_embeddings
                 WHERE asset_id IN (SELECT id FROM assets WHERE missing<>0) OR model_id<>?1",
                params![MODEL_ID],
            );
        }
        jobs::finish(state.inner(), job_id);
        let cancelled = cancel.load(Ordering::Relaxed);
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
    });

    Ok(job_id)
}

#[tauri::command]
pub fn cancel_semantic_index(state: State<'_, AppState>, job_id: u64) -> Result<bool, String> {
    jobs::cancel(state.inner(), job_id)
}

pub(crate) fn ranked(
    state: &AppState,
    query: &[f32],
    mut filter: LibraryFilter,
    limit: i64,
    exclude: Option<i64>,
) -> Result<Vec<SemanticHit>, String> {
    validate_vector(query)?;
    filter.query.clear();
    let mut hits = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        let candidates = search::filtered_summaries(&conn, &filter, i64::MAX)?;
        let assets: HashMap<i64, _> = candidates
            .into_iter()
            .map(|asset| (asset.id, asset))
            .collect();
        let mut st=conn.prepare(
            "SELECT se.asset_id,se.vector,se.dimensions FROM semantic_embeddings se
             JOIN assets a ON a.id=se.asset_id
             WHERE se.model_id=?1 AND se.fingerprint=a.fingerprint AND a.fingerprint<>'' AND a.missing=0"
        ).map_err(|e|e.to_string())?;
        let mut rows = st.query(params![MODEL_ID]).map_err(|e| e.to_string())?;
        let mut hits = Vec::new();
        // Validate and score a borrowed row before advancing it. Keep only
        // hits, not an additional full-index copy of 2048-byte blobs.
        while let Some(row) = rows.next().map_err(|e| e.to_string())? {
            let id: i64 = row.get(0).map_err(|e| e.to_string())?;
            if exclude == Some(id) {
                continue;
            }
            let Some(asset) = assets.get(&id) else {
                continue;
            };
            let dimensions: i64 = row.get(2).map_err(|e| e.to_string())?;
            if dimensions != DIMENSIONS as i64 {
                return Err(format!("图片 {id} 的语义索引维度无效，请重新建立语义索引"));
            }
            let ValueRef::Blob(blob) = row.get_ref(1).map_err(|e| e.to_string())? else {
                return Err(format!(
                    "图片 {id} 的语义索引存储类型无效，请重新建立语义索引"
                ));
            };
            let vector = from_blob(blob).map_err(|error| format!("图片 {id}：{error}"))?;
            let similarity = score(query, &vector);
            if !similarity.is_finite() {
                return Err(format!("图片 {id} 的语义匹配分数无效，请重新建立语义索引"));
            }
            hits.push(SemanticHit {
                asset: asset.clone(),
                score: similarity,
            })
        }
        hits
    };
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
    let (indexed, generation): (i64, u64) = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        let indexed = conn
            .query_row(
                "SELECT COUNT(*) FROM semantic_embeddings WHERE model_id=?1",
                params![MODEL_ID],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        (indexed, cache_generation(&conn)?)
    };
    if indexed == 0 {
        return Err("请先在资料库管理中建立语义索引".into());
    }
    let cache = cache_dir(state.inner());
    let encoded = tauri::async_runtime::spawn_blocking(move || text_vector(cache, query))
        .await
        .map_err(|e| e.to_string())
        .and_then(|result| result);
    let vector = complete_text_encoding(state.inner(), generation, encoded)?;
    ranked(state.inner(), &vector, filter, limit, None)
}

fn complete_text_encoding(
    state: &AppState,
    generation: u64,
    encoded: Result<Vec<f32>, String>,
) -> Result<Vec<f32>, String> {
    let encoded = encoded.and_then(|vector| {
        validate_vector(&vector)?;
        Ok(vector)
    });
    let mut conn = state.db.lock().map_err(|e| e.to_string())?;
    publish_readiness(&mut conn, "text_ready", generation, encoded.is_ok())?;
    encoded
}

#[tauri::command]
pub fn semantic_search_similar(
    state: State<'_, AppState>,
    asset_id: i64,
    filter: LibraryFilter,
    limit: i64,
) -> Result<Vec<SemanticHit>, String> {
    let vector = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        reference_vector(&conn, asset_id)?
    };
    let vector = vector.ok_or("当前图片尚未建立语义索引")?;
    ranked(state.inner(), &vector, filter, limit, Some(asset_id))
}

fn reference_vector(
    conn: &rusqlite::Connection,
    asset_id: i64,
) -> Result<Option<[f32; DIMENSIONS]>, String> {
    let mut statement=conn.prepare(
        "SELECT se.vector,se.dimensions FROM semantic_embeddings se JOIN assets a ON a.id=se.asset_id
         WHERE se.asset_id=?1 AND se.model_id=?2
           AND se.fingerprint=a.fingerprint AND a.fingerprint<>'' AND a.missing=0",
    ).map_err(|e|e.to_string())?;
    let mut rows = statement
        .query(params![asset_id, MODEL_ID])
        .map_err(|e| e.to_string())?;
    let Some(row) = rows.next().map_err(|e| e.to_string())? else {
        return Ok(None);
    };
    let dimensions: i64 = row.get(1).map_err(|e| e.to_string())?;
    if dimensions != DIMENSIONS as i64 {
        return Err("当前图片语义索引维度无效，请重新建立语义索引".into());
    }
    let ValueRef::Blob(blob) = row.get_ref(0).map_err(|e| e.to_string())? else {
        return Err("当前图片语义索引存储类型无效，请重新建立语义索引".into());
    };
    from_blob(blob).map(Some)
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
    let operation = model_operation(&path).map_err(|e| e.to_string())?;
    let _guard = model_guard(
        &operation,
        &Control {
            cancel: None,
            report: &|_| {},
        },
    )
    .map_err(|e| e.to_string())?;
    {
        let mut conn = state.db.lock().map_err(|e| e.to_string())?;
        // Commit invalidation before deleting files. If filesystem deletion
        // fails, an already-running query still cannot reassert readiness.
        invalidate_model_readiness(&mut conn)?;
    }
    if path.exists() {
        fs::remove_dir_all(&path).map_err(|e| e.to_string())?
    }
    fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn indexed_state() -> AppState {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("../schema.sql")).unwrap();
        let vector = to_blob(&normalize(vec![1.0; DIMENSIONS]).unwrap());
        for (id, missing) in [(1, 0), (2, 0), (3, 1)] {
            conn.execute(
                "INSERT INTO assets(id,path,name,fingerprint,missing,created_at,updated_at) VALUES(?1,?2,?3,'fresh',?4,1,1)",
                params![id, format!("/synthetic/{id}.png"), format!("asset {id}"), missing],
            ).unwrap();
            conn.execute(
                "INSERT INTO semantic_embeddings(asset_id,model_id,dimensions,vector,fingerprint,indexed_at) VALUES(?1,?2,?3,?4,'fresh',1)",
                params![id, MODEL_ID, DIMENSIONS as i64, vector],
            ).unwrap();
        }
        AppState {
            db: std::sync::Mutex::new(conn),
            backup_operation: std::sync::Mutex::new(()),
            data_dir: PathBuf::new(),
            cache_dir: PathBuf::new(),
            database_path: PathBuf::new(),
            backups_dir: PathBuf::new(),
            models_dir: PathBuf::new(),
            jobs: std::sync::Mutex::new(HashMap::new()),
            next_job_id: std::sync::atomic::AtomicU64::new(1),
            vision_api_key: std::sync::Mutex::new(String::new()),
        }
    }

    #[test]
    fn ranked_omits_stale_and_missing_embeddings_until_rebuilt() {
        let state = indexed_state();
        {
            let conn = state.db.lock().unwrap();
            conn.execute("UPDATE assets SET fingerprint='changed' WHERE id=2", [])
                .unwrap();
        }
        let query = normalize(vec![1.0; DIMENSIONS]).unwrap();
        let hits = ranked(&state, &query, LibraryFilter::default(), 10, None).unwrap();
        assert_eq!(
            hits.iter().map(|hit| hit.asset.id).collect::<Vec<_>>(),
            vec![1]
        );
        {
            let conn = state.db.lock().unwrap();
            conn.execute(
                "UPDATE semantic_embeddings SET fingerprint='changed' WHERE asset_id=2",
                [],
            )
            .unwrap();
        }
        let rebuilt = ranked(&state, &query, LibraryFilter::default(), 10, None).unwrap();
        assert_eq!(
            rebuilt.iter().map(|hit| hit.asset.id).collect::<Vec<_>>(),
            vec![2, 1]
        );
    }

    #[test]
    fn ranked_includes_a_fresh_match_beyond_one_hundred_thousand_assets() {
        let state = indexed_state();
        {
            let mut conn = state.db.lock().unwrap();
            let tx = conn.transaction().unwrap();
            tx.execute("DELETE FROM semantic_embeddings", []).unwrap();
            tx.execute("DELETE FROM assets", []).unwrap();
            tx.execute_batch("WITH RECURSIVE ids(id) AS (SELECT 1 UNION ALL SELECT id+1 FROM ids WHERE id<100001) INSERT INTO assets(id,path,name,fingerprint,created_at,updated_at) SELECT id,'/synthetic/'||id||'.png','asset '||id,'fresh',1,1 FROM ids;").unwrap();
            tx.execute(
                "INSERT INTO semantic_embeddings VALUES(100001,?1,512,?2,'fresh',1)",
                params![
                    MODEL_ID,
                    to_blob(&normalize(vec![1.0; DIMENSIONS]).unwrap())
                ],
            )
            .unwrap();
            tx.commit().unwrap();
        }
        let hits = ranked(
            &state,
            &normalize(vec![1.0; DIMENSIONS]).unwrap(),
            LibraryFilter::default(),
            10,
            None,
        )
        .unwrap();
        assert_eq!(
            hits.iter().map(|hit| hit.asset.id).collect::<Vec<_>>(),
            vec![100001]
        );
    }

    #[test]
    fn similar_reference_requires_current_nonmissing_embedding() {
        let state = indexed_state();
        let conn = state.db.lock().unwrap();
        assert!(reference_vector(&conn, 1).unwrap().is_some());
        conn.execute("UPDATE assets SET fingerprint='changed' WHERE id=1", [])
            .unwrap();
        assert!(reference_vector(&conn, 1).unwrap().is_none());
        assert!(reference_vector(&conn, 3).unwrap().is_none());
    }

    fn malformed_vectors() -> Vec<(&'static str, i64, rusqlite::types::Value)> {
        use rusqlite::types::Value;
        let mut unit = vec![0.0; DIMENSIONS];
        unit[0] = 1.0;
        let valid = to_blob(&unit);
        let mut extra = valid.clone();
        extra.push(0);
        let mut nan = unit.clone();
        nan[1] = f32::NAN;
        let mut infinite = unit.clone();
        infinite[1] = f32::INFINITY;
        let mut nonunit = unit;
        nonunit[0] = 2.0;
        vec![
            ("empty", 512, Value::Blob(Vec::new())),
            ("short", 512, Value::Blob(valid[..2044].to_vec())),
            ("extra-byte", 512, Value::Blob(extra)),
            ("wrong-dimensions", 511, Value::Blob(valid)),
            ("zero", 512, Value::Blob(vec![0; 2048])),
            ("nan", 512, Value::Blob(to_blob(&nan))),
            ("infinite", 512, Value::Blob(to_blob(&infinite))),
            ("nonunit", 512, Value::Blob(to_blob(&nonunit))),
            ("text-storage", 512, Value::Text("not a blob".into())),
        ]
    }

    #[test]
    fn malformed_index_rows_are_rejected_and_rebuilt_without_changing_valid_rows() {
        let mut query = vec![0.0; DIMENSIONS];
        query[0] = 1.0;
        for (label, dimensions, vector) in malformed_vectors() {
            let state = indexed_state();
            let unchanged: Vec<u8> = {
                let conn = state.db.lock().unwrap();
                conn.execute(
                    "UPDATE semantic_embeddings SET dimensions=?1,vector=?2 WHERE asset_id=1",
                    params![dimensions, vector],
                )
                .unwrap();
                assert_eq!(stale_count(&conn).unwrap(), 1, "{label}");
                assert!(reference_vector(&conn, 1).is_err(), "{label}");
                conn.query_row(
                    "SELECT vector FROM semantic_embeddings WHERE asset_id=2",
                    [],
                    |row| row.get(0),
                )
                .unwrap()
            };
            assert!(
                ranked(&state, &query, LibraryFilter::default(), 10, None).is_err(),
                "{label}"
            );
            let (pending, _) = index_rows(&state, &AtomicBool::new(false)).unwrap();
            assert_eq!(
                pending.iter().map(|row| row.0).collect::<Vec<_>>(),
                vec![1],
                "{label}"
            );
            store_index_vector(
                &state,
                1,
                &to_blob(&query),
                "fresh",
                &Control {
                    cancel: None,
                    report: &|_| {},
                },
            )
            .unwrap();
            assert!(
                index_rows(&state, &AtomicBool::new(false))
                    .unwrap()
                    .0
                    .is_empty(),
                "{label}"
            );
            let conn = state.db.lock().unwrap();
            assert_eq!(stale_count(&conn).unwrap(), 0, "{label}");
            assert!(reference_vector(&conn, 1).unwrap().is_some(), "{label}");
            let retained: Vec<u8> = conn
                .query_row(
                    "SELECT vector FROM semantic_embeddings WHERE asset_id=2",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(retained, unchanged, "{label}");
            drop(conn);
            let hits = ranked(&state, &query, LibraryFilter::default(), 10, None).unwrap();
            assert_eq!(
                hits.iter().map(|hit| hit.asset.id).collect::<Vec<_>>(),
                vec![1, 2],
                "{label}"
            );
        }
    }

    #[test]
    fn malformed_query_vectors_cannot_match_valid_embeddings() {
        let state = indexed_state();
        for (label, _, value) in malformed_vectors() {
            if let rusqlite::types::Value::Blob(blob) = value {
                if label == "wrong-dimensions" {
                    continue;
                }
                let (chunks, _) = blob.as_chunks::<4>();
                let query: Vec<f32> = chunks
                    .iter()
                    .map(|chunk| f32::from_le_bytes(*chunk))
                    .collect();
                if label == "extra-byte" {
                    continue;
                }
                assert!(
                    ranked(&state, &query, LibraryFilter::default(), 10, None).is_err(),
                    "{label}"
                );
            }
        }
        assert!(ranked(
            &state,
            &vec![1.0; DIMENSIONS + 1],
            LibraryFilter::default(),
            10,
            None
        )
        .is_err());
    }

    #[test]
    fn malformed_model_blobs_cannot_overwrite_a_valid_embedding() {
        let state = indexed_state();
        let previous: Vec<u8> = state
            .db
            .lock()
            .unwrap()
            .query_row(
                "SELECT vector FROM semantic_embeddings WHERE asset_id=1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        for (label, _, value) in malformed_vectors() {
            if let rusqlite::types::Value::Blob(blob) = value {
                if label == "wrong-dimensions" {
                    continue;
                }
                assert!(
                    store_index_vector(
                        &state,
                        1,
                        &blob,
                        "fresh",
                        &Control {
                            cancel: None,
                            report: &|_| {}
                        }
                    )
                    .is_err(),
                    "{label}"
                );
                let retained: Vec<u8> = state
                    .db
                    .lock()
                    .unwrap()
                    .query_row(
                        "SELECT vector FROM semantic_embeddings WHERE asset_id=1",
                        [],
                        |row| row.get(0),
                    )
                    .unwrap();
                assert_eq!(retained, previous, "{label}");
            }
        }
    }

    #[test]
    fn embedding_sql_errors_are_reported_instead_of_omitted() {
        let state = indexed_state();
        state
            .db
            .lock()
            .unwrap()
            .execute(
                "UPDATE semantic_embeddings SET dimensions=x'ff' WHERE asset_id=1",
                [],
            )
            .unwrap();
        let mut query = vec![0.0; DIMENSIONS];
        query[0] = 1.0;
        assert!(ranked(&state, &query, LibraryFilter::default(), 10, None).is_err());
        let conn = state.db.lock().unwrap();
        assert_eq!(stale_count(&conn).unwrap(), 1);
        assert!(reference_vector(&conn, 1).is_err());
        drop(conn);
        assert_eq!(
            index_rows(&state, &AtomicBool::new(false))
                .unwrap()
                .0
                .iter()
                .map(|row| row.0)
                .collect::<Vec<_>>(),
            vec![1]
        );
        let conn = state.db.lock().unwrap();
        conn.execute("DROP TABLE semantic_embeddings", []).unwrap();
        assert!(stale_count(&conn).is_err());
        drop(conn);
        assert!(ranked(&state, &query, LibraryFilter::default(), 10, None).is_err());
        assert!(matches!(
            index_rows(&state, &AtomicBool::new(false)),
            Err(ModelError::Failed(_))
        ));
    }

    #[test]
    fn ranked_reports_a_candidate_summary_conversion_error() {
        let state = indexed_state();
        state
            .db
            .lock()
            .unwrap()
            .execute("UPDATE assets SET path=x'ff' WHERE id=1", [])
            .unwrap();
        let query = normalize(vec![1.0; DIMENSIONS]).unwrap();
        assert!(ranked(&state, &query, LibraryFilter::default(), 10, None).is_err());
    }

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
        let mut raw = vec![0.0; DIMENSIONS];
        raw[..4].copy_from_slice(&[1.0, 2.0, 3.0, 4.0]);
        let source = normalize(raw).unwrap();
        let decoded = from_blob(&to_blob(&source)).unwrap();
        assert_eq!(decoded.len(), source.len());
        assert!((score(&source, &decoded) - 1.0).abs() < 0.0001);
    }

    #[test]
    fn model_normalization_rejects_invalid_outputs_and_handles_large_finite_values() {
        for raw in [
            Vec::new(),
            vec![0.0; DIMENSIONS],
            vec![f32::NAN; DIMENSIONS],
            vec![f32::INFINITY; DIMENSIONS],
            vec![1.0; DIMENSIONS - 1],
        ] {
            assert!(normalize(raw).is_err());
        }
        let normalized = normalize(vec![f32::MAX; DIMENSIONS]).unwrap();
        assert!(normalized
            .iter()
            .all(|value| value.is_finite() && *value > 0.0));
        validate_vector(&normalized).unwrap();
        from_blob(&to_blob(&normalized)).unwrap();
    }

    #[test]
    fn invalid_text_output_clears_readiness_without_returning_a_bad_query_vector() {
        let state = indexed_state();
        for raw in [
            Vec::new(),
            vec![0.0; DIMENSIONS],
            vec![f32::NAN; DIMENSIONS],
            vec![f32::INFINITY; DIMENSIONS],
            vec![1.0; DIMENSIONS],
        ] {
            set_setting(&state.db.lock().unwrap(), "text_ready", "1").unwrap();
            assert!(complete_text_encoding(&state, 0, Ok(raw)).is_err());
            assert_eq!(setting(&state.db.lock().unwrap(), "text_ready"), "0");
        }
    }

    #[test]
    fn vector_norm_tolerance_accepts_rounding_and_rejects_scaled_vectors() {
        let mut unit = vec![0.0; DIMENSIONS];
        unit[0] = 1.0005;
        validate_vector(&unit).unwrap();
        from_blob(&to_blob(&unit)).unwrap();
        unit[0] = 1.002;
        assert!(validate_vector(&unit).is_err());
        assert!(from_blob(&to_blob(&unit)).is_err());
    }

    #[test]
    fn an_excluded_bad_index_does_not_block_other_valid_candidates() {
        let state = indexed_state();
        state
            .db
            .lock()
            .unwrap()
            .execute(
                "UPDATE semantic_embeddings SET vector=x'' WHERE asset_id=1",
                [],
            )
            .unwrap();
        let query = normalize(vec![1.0; DIMENSIONS]).unwrap();
        let hits = ranked(&state, &query, LibraryFilter::default(), 10, Some(1)).unwrap();
        assert_eq!(
            hits.iter().map(|hit| hit.asset.id).collect::<Vec<_>>(),
            vec![2]
        );
    }

    #[test]
    fn empty_fingerprints_are_stale_in_status_preparation_and_search() {
        let state = indexed_state();
        let conn = state.db.lock().unwrap();
        conn.execute("UPDATE assets SET fingerprint='' WHERE id=1", [])
            .unwrap();
        conn.execute(
            "UPDATE semantic_embeddings SET fingerprint='' WHERE asset_id=1",
            [],
        )
        .unwrap();
        assert_eq!(stale_count(&conn).unwrap(), 1);
        assert!(reference_vector(&conn, 1).unwrap().is_none());
        drop(conn);
        let pending = index_rows(&state, &AtomicBool::new(false)).unwrap().0;
        assert_eq!(pending.iter().map(|row| row.0).collect::<Vec<_>>(), vec![1]);
        let query = normalize(vec![1.0; DIMENSIONS]).unwrap();
        let hits = ranked(&state, &query, LibraryFilter::default(), 10, None).unwrap();
        assert_eq!(
            hits.iter().map(|hit| hit.asset.id).collect::<Vec<_>>(),
            vec![2]
        );
    }

    fn model_test_root() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "imagelore-model-operation-{}-{}",
            std::process::id(),
            STAGE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        path
    }

    fn small_spec() -> TrustedModelSpec {
        static SUPPORT: &[SupportFile] = &[SupportFile {
            path: "config.json",
            max_bytes: 64,
        }];
        TrustedModelSpec {
            name: "test",
            repo: "unused",
            revision: "fixed",
            onnx_sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
            onnx_size: 3,
            support: SUPPORT,
        }
    }

    #[test]
    fn trusted_cache_is_network_free_and_installs_only_complete_verified_files() {
        let root = model_test_root();
        let spec = small_spec();
        let control = Control {
            cancel: None,
            report: &|_| {},
        };
        let target = install_trusted_with(&root, spec, &control, |name, path, _, _| {
            fs::write(
                path,
                if name == "model.onnx" {
                    b"abc".as_slice()
                } else {
                    b"{}".as_slice()
                },
            )?;
            Ok(())
        })
        .unwrap();
        let manifest = fs::read(target.join("trusted-manifest.json")).unwrap();
        assert_eq!(
            install_trusted_with(&root, spec, &control, |_, _, _, _| panic!(
                "valid private cache must not contact a downloader"
            ))
            .unwrap(),
            target
        );
        assert_eq!(
            fs::read(target.join("trusted-manifest.json")).unwrap(),
            manifest
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_or_cancelled_download_keeps_old_cache_and_other_staging_files() {
        for mode in ["cancel", "hash", "json", "network"] {
            let root = model_test_root();
            let spec = small_spec();
            let target = trusted_dir(&root, spec);
            fs::create_dir_all(&target).unwrap();
            fs::write(target.join("model.onnx"), b"old").unwrap();
            fs::write(target.join("config.json"), b"{}").unwrap();
            let other = root.join(".trusted-other-owner");
            fs::create_dir(&other).unwrap();
            fs::write(other.join("keep"), b"keep").unwrap();
            let flag = AtomicBool::new(false);
            let result = install_trusted_with(
                &root,
                spec,
                &Control {
                    cancel: Some(&flag),
                    report: &|_| {},
                },
                |name, path, _, _| {
                    if mode == "network" {
                        return Err(ModelError::Failed("HTTP 503".into()));
                    }
                    fs::write(
                        path,
                        if name == "model.onnx" {
                            if mode == "hash" {
                                b"abd".as_slice()
                            } else {
                                b"abc".as_slice()
                            }
                        } else if mode == "json" {
                            b"not-json".as_slice()
                        } else {
                            b"{}".as_slice()
                        },
                    )?;
                    if mode == "cancel" {
                        flag.store(true, Ordering::Relaxed);
                    }
                    Ok(())
                },
            );
            if mode == "cancel" {
                assert!(matches!(result, Err(ModelError::Cancelled)));
            } else {
                assert!(matches!(result, Err(ModelError::Failed(_))));
            }
            assert_eq!(fs::read(target.join("model.onnx")).unwrap(), b"old");
            assert_eq!(fs::read(other.join("keep")).unwrap(), b"keep");
            assert_eq!(
                fs::read_dir(&root).unwrap().count(),
                2,
                "only the old cache and another owner's stage may remain"
            );
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn cancellation_during_cached_hash_is_not_reinterpreted_as_a_download_request() {
        let root = model_test_root();
        let spec = small_spec();
        let target = trusted_dir(&root, spec);
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("model.onnx"), b"abc").unwrap();
        fs::write(target.join("config.json"), b"{}").unwrap();
        let flag = AtomicBool::new(false);
        let report = |_: &str| flag.store(true, Ordering::Relaxed);
        let result = install_trusted_with(
            &root,
            spec,
            &Control {
                cancel: Some(&flag),
                report: &report,
            },
            |_, _, _, _| panic!("cancelled validation must not start downloading"),
        );
        assert!(matches!(result, Err(ModelError::Cancelled)));
        assert_eq!(fs::read(target.join("model.onnx")).unwrap(), b"abc");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn an_external_cache_mutation_is_revalidated_and_never_loaded_when_download_is_offline() {
        let root = model_test_root();
        let spec = small_spec();
        let target = trusted_dir(&root, spec);
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("model.onnx"), b"abc").unwrap();
        fs::write(target.join("config.json"), b"{}").unwrap();
        let control = Control {
            cancel: None,
            report: &|_| {},
        };
        assert_eq!(
            install_trusted_with(&root, spec, &control, |_, _, _, _| panic!(
                "verified cache is offline-ready"
            ))
            .unwrap(),
            target
        );
        fs::write(target.join("model.onnx"), b"abd").unwrap();
        let attempted = AtomicBool::new(false);
        let result = install_trusted_with(&root, spec, &control, |_, _, _, _| {
            attempted.store(true, Ordering::Relaxed);
            Err(ModelError::Failed("offline".into()))
        });
        assert!(matches!(result, Err(ModelError::Failed(_))));
        assert!(attempted.load(Ordering::Relaxed));
        assert_eq!(fs::read(target.join("model.onnx")).unwrap(), b"abd");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn model_locks_are_scoped_by_cache_and_waiting_cancellation_does_not_take_ownership() {
        let root = model_test_root();
        let first = model_operation(&root).unwrap();
        assert!(Arc::ptr_eq(&first, &model_operation(&root).unwrap()));
        assert!(!Arc::ptr_eq(
            &first,
            &model_operation(&root.join("other")).unwrap()
        ));
        let guard = first.lock().unwrap();
        let flag = Arc::new(AtomicBool::new(false));
        let child_flag = flag.clone();
        let worker_lock = first.clone();
        let worker = std::thread::spawn(move || {
            matches!(
                model_guard(
                    &worker_lock,
                    &Control {
                        cancel: Some(&child_flag),
                        report: &|_| {}
                    }
                ),
                Err(ModelError::Cancelled)
            )
        });
        std::thread::sleep(Duration::from_millis(60));
        flag.store(true, Ordering::Relaxed);
        assert!(worker.join().unwrap());
        drop(guard);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn text_encoding_failure_clears_previous_readiness_and_preserves_the_error() {
        let state = indexed_state();
        set_setting(&state.db.lock().unwrap(), "text_ready", "1").unwrap();
        assert_eq!(
            complete_text_encoding(&state, 0, Err("trusted model digest rejected".into()))
                .unwrap_err(),
            "trusted model digest rejected"
        );
        assert_eq!(setting(&state.db.lock().unwrap(), "text_ready"), "0");
        assert_eq!(
            complete_text_encoding(&state, 0, normalize(vec![1.0; DIMENSIONS])).unwrap(),
            normalize(vec![1.0; DIMENSIONS]).unwrap()
        );
        assert_eq!(setting(&state.db.lock().unwrap(), "text_ready"), "1");
    }

    #[test]
    fn stale_text_success_after_deletion_cannot_reassert_readiness() {
        let state = indexed_state();
        {
            let mut conn = state.db.lock().unwrap();
            set_setting(&conn, "text_ready", "1").unwrap();
            invalidate_model_readiness(&mut conn).unwrap();
        }
        assert_eq!(
            complete_text_encoding(&state, 0, normalize(vec![1.0; DIMENSIONS])).unwrap(),
            normalize(vec![1.0; DIMENSIONS]).unwrap()
        );
        assert_eq!(setting(&state.db.lock().unwrap(), "text_ready"), "0");
    }

    #[test]
    fn stale_text_failure_cannot_clear_a_new_generation_success() {
        let state = indexed_state();
        {
            let mut conn = state.db.lock().unwrap();
            invalidate_model_readiness(&mut conn).unwrap();
        }
        complete_text_encoding(&state, 1, normalize(vec![1.0; DIMENSIONS])).unwrap();
        assert_eq!(
            complete_text_encoding(&state, 0, Err("old model failed".into())).unwrap_err(),
            "old model failed"
        );
        assert_eq!(setting(&state.db.lock().unwrap(), "text_ready"), "1");
    }

    #[test]
    fn current_text_generation_success_and_failure_publish_their_actual_status() {
        let state = indexed_state();
        {
            let mut conn = state.db.lock().unwrap();
            invalidate_model_readiness(&mut conn).unwrap();
        }
        complete_text_encoding(&state, 1, normalize(vec![1.0; DIMENSIONS])).unwrap();
        assert_eq!(setting(&state.db.lock().unwrap(), "text_ready"), "1");
        assert_eq!(
            complete_text_encoding(&state, 1, Err("current model failed".into())).unwrap_err(),
            "current model failed"
        );
        assert_eq!(setting(&state.db.lock().unwrap(), "text_ready"), "0");
    }

    #[test]
    fn readiness_invalidation_rolls_back_all_settings_on_an_interrupted_write() {
        let state = indexed_state();
        let mut conn = state.db.lock().unwrap();
        set_setting(&conn, "vision_ready", "1").unwrap();
        set_setting(&conn, "text_ready", "1").unwrap();
        set_setting(&conn, "cache_generation", "0").unwrap();
        conn.execute_batch("CREATE TRIGGER fail_generation BEFORE UPDATE ON semantic_settings WHEN NEW.key='cache_generation' BEGIN SELECT RAISE(ABORT,'synthetic generation write failure'); END;").unwrap();
        assert!(invalidate_model_readiness(&mut conn).is_err());
        assert_eq!(setting(&conn, "vision_ready"), "1");
        assert_eq!(setting(&conn, "text_ready"), "1");
        assert_eq!(cache_generation(&conn).unwrap(), 0);
    }

    #[test]
    fn invalid_or_overflowing_generation_preserves_previous_settings() {
        for generation in ["invalid", "18446744073709551615"] {
            let state = indexed_state();
            let mut conn = state.db.lock().unwrap();
            set_setting(&conn, "vision_ready", "1").unwrap();
            set_setting(&conn, "text_ready", "1").unwrap();
            set_setting(&conn, "cache_generation", generation).unwrap();
            assert!(invalidate_model_readiness(&mut conn).is_err());
            assert_eq!(setting(&conn, "vision_ready"), "1");
            assert_eq!(setting(&conn, "text_ready"), "1");
            assert_eq!(setting(&conn, "cache_generation"), generation);
        }
    }

    fn assert_db_wait_can_cancel(
        state: Arc<AppState>,
        operation: impl FnOnce(&AppState, &AtomicBool) -> ModelResult<()> + Send + 'static,
    ) {
        let guard = state.db.lock().unwrap();
        let flag = Arc::new(AtomicBool::new(false));
        let worker_state = state.clone();
        let worker_flag = flag.clone();
        let (send, receive) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            let result = operation(&worker_state, &worker_flag);
            send.send(matches!(result, Err(ModelError::Cancelled)))
                .unwrap();
        });
        std::thread::sleep(Duration::from_millis(60));
        flag.store(true, Ordering::Relaxed);
        assert!(receive.recv_timeout(Duration::from_millis(700)).unwrap());
        // Keep the DB locked until the cancelled worker has returned. A
        // blocking DB lock or a post-cancel write fails this assertion.
        drop(guard);
        worker.join().unwrap();
    }

    #[test]
    fn index_preparation_db_wait_cancels_without_enabling_or_mutating_the_library() {
        let state = Arc::new(indexed_state());
        set_setting(&state.db.lock().unwrap(), "enabled", "0").unwrap();
        assert_db_wait_can_cancel(state.clone(), |state, cancel| {
            index_rows(state, cancel).map(|_| ())
        });
        assert_eq!(setting(&state.db.lock().unwrap(), "enabled"), "0");
    }

    #[test]
    fn index_write_db_wait_cancels_without_overwriting_a_previous_embedding() {
        let state = Arc::new(indexed_state());
        assert_db_wait_can_cancel(state.clone(), |state, cancel| {
            store_index_vector(
                state,
                1,
                &to_blob(&normalize(vec![2.0; DIMENSIONS]).unwrap()),
                "cancelled-new-fingerprint",
                &Control {
                    cancel: Some(cancel),
                    report: &|_| {},
                },
            )
        });
        let conn = state.db.lock().unwrap();
        let fingerprint: String = conn
            .query_row(
                "SELECT fingerprint FROM semantic_embeddings WHERE asset_id=1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(fingerprint, "fresh");
    }

    #[test]
    fn index_preparation_reports_a_malformed_row_instead_of_silently_skipping_it() {
        let state = indexed_state();
        state
            .db
            .lock()
            .unwrap()
            .execute(
                "UPDATE assets SET path=x'ff',fingerprint='changed' WHERE id=1",
                [],
            )
            .unwrap();
        assert!(matches!(
            index_rows(&state, &AtomicBool::new(false)),
            Err(ModelError::Failed(_))
        ));
    }

    #[test]
    fn a_stage_cleanup_failure_preserves_the_cancelled_result_and_other_files() {
        let root = model_test_root();
        let invalid_stage = root.join("already-removed-stage");
        let other = root.join("other-owner-file");
        fs::write(&other, b"keep").unwrap();
        assert!(matches!(
            discard_stage(&invalid_stage, ModelError::Cancelled),
            ModelError::Cancelled
        ));
        assert_eq!(fs::read(&other).unwrap(), b"keep");
        fs::remove_dir_all(root).unwrap();
    }
}
