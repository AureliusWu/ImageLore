use base64::{engine::general_purpose::STANDARD, Engine as _};
use image::{DynamicImage, ImageFormat};
use std::{fs, io::Cursor, path::{Path, PathBuf}};

fn load_scaled(path: &Path, max_edge: u32) -> Result<DynamicImage, String> {
    let image = image::open(path).map_err(|e| e.to_string())?;
    if max_edge == 0 { return Ok(image); }
    let w = image.width();
    let h = image.height();
    if w <= max_edge && h <= max_edge { return Ok(image); }
    Ok(image.thumbnail(max_edge, max_edge))
}

fn encode_webp(image: &DynamicImage) -> Result<Vec<u8>, String> {
    let mut cursor = Cursor::new(Vec::<u8>::new());
    image.write_to(&mut cursor, ImageFormat::WebP).map_err(|e| e.to_string())?;
    Ok(cursor.into_inner())
}

pub fn preview_data_url(path: &Path, cache_dir: &Path, cache_key: &str, max_edge: u32, cache: bool) -> Result<String, String> {
    let max_edge = max_edge.clamp(160, 4800);
    let cache_path: PathBuf = cache_dir.join(format!("{}-{}.webp", cache_key, max_edge));
    let bytes = if cache && cache_path.exists() {
        fs::read(&cache_path).map_err(|e| e.to_string())?
    } else {
        let image = load_scaled(path, max_edge)?;
        let bytes = encode_webp(&image)?;
        if cache {
            let _ = fs::create_dir_all(cache_dir);
            let _ = fs::write(&cache_path, &bytes);
        }
        bytes
    };
    Ok(format!("data:image/webp;base64,{}", STANDARD.encode(bytes)))
}

pub fn purge_asset_cache(cache_dir: &Path, fingerprint: &str) {
    if fingerprint.is_empty() { return; }
    if let Ok(entries) = fs::read_dir(cache_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with(fingerprint) { let _ = fs::remove_file(entry.path()); }
        }
    }
}
