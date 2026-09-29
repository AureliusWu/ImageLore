use image::{DynamicImage, ImageFormat};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::UNIX_EPOCH,
};
use walkdir::WalkDir;

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(1);

fn load_scaled(path: &Path, max_edge: u32) -> Result<DynamicImage, String> {
    let image = image::open(path).map_err(|e| e.to_string())?;
    if max_edge == 0 {
        return Ok(image);
    }
    let (w, h) = (image.width(), image.height());
    if w <= max_edge && h <= max_edge {
        return Ok(image);
    }
    Ok(image.thumbnail(max_edge, max_edge))
}

fn cache_path(cache_root: &Path, cache_key: &str, max_edge: u32, thumbnail: bool) -> PathBuf {
    let kind = if thumbnail { "thumbnails" } else { "previews" };
    let edge = if max_edge == 0 {
        "full".into()
    } else {
        max_edge.to_string()
    };
    cache_root
        .join(kind)
        .join(format!("{}-{}.webp", cache_key, edge))
}

pub fn cached_preview_path(
    path: &Path,
    cache_root: &Path,
    cache_key: &str,
    max_edge: u32,
    thumbnail: bool,
) -> Result<String, String> {
    let max_edge = if max_edge == 0 {
        0
    } else {
        max_edge.clamp(160, 4800)
    };
    let output = cache_path(cache_root, cache_key, max_edge, thumbnail);
    if output.exists() {
        return Ok(output.to_string_lossy().to_string());
    }

    let parent = output.parent().ok_or("无法定位缓存目录")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let image = load_scaled(path, max_edge)?;

    let temp = output.with_extension(format!(
        "webp.{}.{}.tmp",
        std::process::id(),
        TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    {
        let mut file = fs::File::create(&temp).map_err(|e| e.to_string())?;
        image
            .write_to(&mut file, ImageFormat::WebP)
            .map_err(|e| e.to_string())?;
    }
    if let Err(error) = fs::rename(&temp, &output) {
        if !output.exists() {
            let _ = fs::remove_file(&temp);
            return Err(error.to_string());
        }
        let _ = fs::remove_file(&temp);
    }
    Ok(output.to_string_lossy().to_string())
}

pub fn purge_asset_cache(cache_root: &Path, fingerprint: &str) {
    if fingerprint.is_empty() {
        return;
    }
    for entry in WalkDir::new(cache_root)
        .min_depth(1)
        .into_iter()
        .filter_map(Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy();
        if name.starts_with(fingerprint) {
            let _ = fs::remove_file(entry.path());
        }
    }
}

pub fn prune_cache(cache_root: &Path, max_bytes: u64) {
    let mut files = Vec::<(PathBuf, u64, u64)>::new();
    let mut total = 0u64;
    for entry in WalkDir::new(cache_root)
        .min_depth(1)
        .into_iter()
        .filter_map(Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        let size = meta.len();
        let modified = meta
            .modified()
            .ok()
            .and_then(|x| x.duration_since(UNIX_EPOCH).ok())
            .map(|x| x.as_secs())
            .unwrap_or(0);
        total = total.saturating_add(size);
        files.push((entry.into_path(), size, modified));
    }
    if total <= max_bytes {
        return;
    }
    files.sort_by_key(|x| x.2);
    for (path, size, _) in files {
        if total <= max_bytes {
            break;
        }
        if fs::remove_file(path).is_ok() {
            total = total.saturating_sub(size)
        }
    }
}
