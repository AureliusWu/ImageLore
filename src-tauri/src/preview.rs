use image::{DynamicImage, ImageFormat};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
    time::{Duration, Instant, UNIX_EPOCH},
};
use walkdir::WalkDir;

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(1);
const CACHE_MAX_BYTES: u64 = 1024 * 1024 * 1024;
const CACHE_PRUNE_INTERVAL: Duration = Duration::from_secs(60);
static CACHE_PRUNE_LOCK: Mutex<()> = Mutex::new(());
static CACHE_PRUNE_RUNS: std::sync::LazyLock<Mutex<HashMap<PathBuf, (Instant, bool)>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

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
    // A fit request that keeps every source pixel shares the full preview URL.
    // Read the source header rather than potentially stale library dimensions;
    // if it cannot be read, retain the existing requested-cache behavior.
    let max_edge = if !thumbnail
        && max_edge > 0
        && image::image_dimensions(path).is_ok_and(|(w, h)| w <= max_edge && h <= max_edge)
    {
        0
    } else {
        max_edge
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
    request_cache_prune(cache_root, &output);
    Ok(output.to_string_lossy().to_string())
}

fn request_cache_prune(cache_root: &Path, current: &Path) {
    let Ok(mut runs) = CACHE_PRUNE_RUNS.lock() else {
        return;
    };
    if let Some((last, running)) = runs.get(cache_root) {
        if *running || last.elapsed() < CACHE_PRUNE_INTERVAL {
            return;
        }
    }
    runs.insert(cache_root.to_path_buf(), (Instant::now(), true));
    let root = cache_root.to_path_buf();
    let current = current.to_path_buf();
    drop(runs);
    std::thread::spawn(move || {
        prune_cache_preserving(&root, CACHE_MAX_BYTES, Some(&current));
        if let Ok(mut runs) = CACHE_PRUNE_RUNS.lock() {
            runs.insert(root, (Instant::now(), false));
        }
    });
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
    prune_cache_preserving(cache_root, max_bytes, None);
}

fn prune_cache_preserving(cache_root: &Path, max_bytes: u64, current: Option<&Path>) {
    // Startup and ongoing cleanup share one worker; only complete derived WebP
    // files count toward the soft cache budget. Writers' .tmp files stay intact.
    let Ok(_guard) = CACHE_PRUNE_LOCK.lock() else {
        return;
    };
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
        if entry.path().extension().is_none_or(|x| x != "webp") {
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
        if current == Some(path.as_path()) {
            continue;
        }
        if fs::remove_file(path).is_ok() {
            total = total.saturating_sub(size)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "imagelore-preview-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn cached_preview_is_created_reused_and_edge_is_clamped() {
        let root = temp_root("cache");
        let source = root.join("source.png");
        DynamicImage::new_rgb8(400, 200).save(&source).unwrap();

        let first = cached_preview_path(&source, &root, "fingerprint", 1, true).unwrap();
        let first_path = PathBuf::from(&first);
        assert!(first_path.exists());
        assert!(first_path.ends_with("fingerprint-160.webp"));

        let rendered = image::open(&first_path).unwrap();
        assert_eq!((rendered.width(), rendered.height()), (160, 80));

        let second = cached_preview_path(&source, &root, "fingerprint", 1, true).unwrap();
        assert_eq!(first, second);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn small_fit_and_full_share_cache_without_replacing_original_or_legacy_cache() {
        let root = temp_root("small-fit-full");
        let source = root.join("source.png");
        DynamicImage::new_rgb8(128, 96).save(&source).unwrap();
        let original = fs::read(&source).unwrap();
        let legacy = cache_path(&root, "small", 2200, false);
        fs::create_dir_all(legacy.parent().unwrap()).unwrap();
        fs::write(&legacy, b"previous derived cache").unwrap();

        let fit = cached_preview_path(&source, &root, "small", 2200, false).unwrap();
        let full = cached_preview_path(&source, &root, "small", 0, false).unwrap();

        assert_eq!(fit, full);
        assert!(Path::new(&fit).ends_with("small-full.webp"));
        let rendered = image::open(&fit).unwrap();
        assert_eq!((rendered.width(), rendered.height()), (128, 96));
        assert_eq!(fs::read(&source).unwrap(), original);
        assert_eq!(fs::read(&legacy).unwrap(), b"previous derived cache");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn preview_choice_uses_current_source_dimensions_after_replacement() {
        let root = temp_root("current-dimensions");
        let source = root.join("source.png");
        DynamicImage::new_rgb8(400, 200).save(&source).unwrap();
        // Source dimensions, rather than database dimensions from an earlier
        // import, must decide whether the requested edge would resize pixels.
        let small = cached_preview_path(&source, &root, "small-stamp", 2200, false).unwrap();
        assert!(Path::new(&small).ends_with("small-stamp-full.webp"));

        DynamicImage::new_rgb8(2400, 1200).save(&source).unwrap();
        let original = fs::read(&source).unwrap();
        // The command's cache key includes the current file stamp/size.
        let fit = cached_preview_path(&source, &root, "large-stamp", 2200, false).unwrap();
        let full = cached_preview_path(&source, &root, "large-stamp", 0, false).unwrap();
        assert_ne!(fit, full);
        assert!(Path::new(&fit).ends_with("large-stamp-2200.webp"));
        let rendered = image::open(&fit).unwrap();
        assert_eq!((rendered.width(), rendered.height()), (2200, 1100));
        let rendered = image::open(&full).unwrap();
        assert_eq!((rendered.width(), rendered.height()), (2400, 1200));
        assert_eq!(fs::read(&source).unwrap(), original);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn small_thumbnail_keeps_its_requested_edge_and_separate_cache_directory() {
        let root = temp_root("thumbnail-isolation");
        let source = root.join("source.png");
        DynamicImage::new_rgb8(64, 80).save(&source).unwrap();
        let thumbnail = cached_preview_path(&source, &root, "small", 420, true).unwrap();
        let fit = cached_preview_path(&source, &root, "small", 2200, false).unwrap();
        let full = cached_preview_path(&source, &root, "small", 0, false).unwrap();
        assert!(Path::new(&thumbnail).ends_with(Path::new("thumbnails").join("small-420.webp")));
        assert_ne!(thumbnail, full);
        assert_eq!(fit, full);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn unreadable_header_keeps_requested_cache_and_failure_preserves_source() {
        let root = temp_root("header-fallback");
        let source = root.join("source.png");
        fs::write(&source, b"not a supported image").unwrap();
        let requested = cache_path(&root, "cached", 2200, false);
        fs::create_dir_all(requested.parent().unwrap()).unwrap();
        fs::write(&requested, b"previous derived cache").unwrap();

        let returned = cached_preview_path(&source, &root, "cached", 2200, false).unwrap();
        assert_eq!(Path::new(&returned), requested);
        assert_eq!(fs::read(&requested).unwrap(), b"previous derived cache");
        assert!(cached_preview_path(&source, &root, "uncached", 2200, false).is_err());
        assert_eq!(fs::read(&source).unwrap(), b"not a supported image");
        assert!(!cache_path(&root, "uncached", 2200, false).exists());
        assert!(!cache_path(&root, "uncached", 0, false).exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn concurrent_small_fit_and_full_requests_publish_one_complete_cache() {
        let root = temp_root("concurrent-fit-full");
        let source = root.join("source.png");
        DynamicImage::new_rgb8(128, 96).save(&source).unwrap();
        let original = fs::read(&source).unwrap();
        let workers = (0..8)
            .map(|index| {
                let source = source.clone();
                let root = root.clone();
                std::thread::spawn(move || {
                    cached_preview_path(
                        &source,
                        &root,
                        "concurrent",
                        if index % 2 == 0 { 2200 } else { 0 },
                        false,
                    )
                    .unwrap()
                })
            })
            .collect::<Vec<_>>();
        let paths = workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .collect::<Vec<_>>();
        assert!(paths.iter().all(|path| path == &paths[0]));
        let rendered = image::open(&paths[0]).unwrap();
        assert_eq!((rendered.width(), rendered.height()), (128, 96));
        let files = fs::read_dir(root.join("previews"))
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect::<Vec<_>>();
        assert_eq!(
            files,
            vec![std::ffi::OsString::from("concurrent-full.webp")]
        );
        assert_eq!(fs::read(&source).unwrap(), original);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn purge_asset_cache_only_removes_matching_fingerprint_files() {
        let root = temp_root("purge");
        let previews = root.join("previews");
        fs::create_dir_all(&previews).unwrap();
        fs::write(previews.join("abc-160.webp"), b"a").unwrap();
        fs::write(previews.join("other-160.webp"), b"b").unwrap();

        purge_asset_cache(&root, "abc");

        assert!(!previews.join("abc-160.webp").exists());
        assert!(previews.join("other-160.webp").exists());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn prune_cache_can_reduce_cache_to_zero_bytes() {
        let root = temp_root("prune");
        fs::write(root.join("a.webp"), b"1234").unwrap();
        fs::write(root.join("b.webp"), b"5678").unwrap();

        prune_cache(&root, 0);

        assert!(!root.join("a.webp").exists());
        assert!(!root.join("b.webp").exists());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn prune_preserves_inflight_encoding_and_noncache_files() {
        let root = temp_root("inflight");
        fs::write(root.join("old.webp"), b"1234").unwrap();
        fs::write(root.join("writing.webp.12.1.tmp"), b"5678").unwrap();
        fs::write(root.join("source.png"), b"source").unwrap();
        prune_cache(&root, 0);
        assert!(!root.join("old.webp").exists());
        assert!(root.join("writing.webp.12.1.tmp").exists());
        assert!(root.join("source.png").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn ongoing_cleanup_preserves_the_just_returned_preview() {
        let root = temp_root("current");
        let current = root.join("current.webp");
        fs::write(&current, b"1234").unwrap();
        fs::write(root.join("old.webp"), b"5678").unwrap();
        prune_cache_preserving(&root, 4, Some(&current));
        assert!(current.exists());
        assert!(!root.join("old.webp").exists());
        let _ = fs::remove_dir_all(root);
    }
}
