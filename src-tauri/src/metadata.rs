use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::{collections::HashMap, fs, io::Read, path::Path, time::UNIX_EPOCH};

pub const SUPPORTED: &[&str] = &["png", "jpg", "jpeg", "webp", "bmp", "gif"];

pub fn is_supported(path: &Path) -> bool {
    path.extension()
        .and_then(|x| x.to_str())
        .map(|x| SUPPORTED.contains(&x.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

pub fn fingerprint(path: &Path) -> String {
    let mut hasher = Sha256::new();
    let Ok(mut file) = fs::File::open(path) else { return String::new() };
    let mut buf = [0u8; 1024 * 64];
    loop {
        match file.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => hasher.update(&buf[..n]),
            Err(_) => return String::new(),
        }
    }
    format!("{:x}", hasher.finalize())
}

pub struct FileInfo {
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub file_size: Option<i64>,
    pub format: String,
    pub mime_type: String,
    pub file_mtime: i64,
}

pub fn file_info(path: &Path) -> FileInfo {
    let meta = fs::metadata(path).ok();
    let file_size = meta.as_ref().map(|m| m.len() as i64);
    let file_mtime = meta
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|x| x.as_secs() as i64)
        .unwrap_or(0);
    let (width, height) = image::image_dimensions(path)
        .map(|(w, h)| (Some(w as i64), Some(h as i64)))
        .unwrap_or((None, None));
    let format = path
        .extension()
        .and_then(|x| x.to_str())
        .unwrap_or("")
        .to_ascii_uppercase();
    let mime_type = mime_guess::from_path(path)
        .first_or_octet_stream()
        .essence_str()
        .to_string();
    FileInfo { width, height, file_size, format, mime_type, file_mtime }
}

fn read_png_text(path: &Path) -> HashMap<String, String> {
    let data = match fs::read(path) { Ok(x) => x, Err(_) => return HashMap::new() };
    if data.len() < 8 || &data[..8] != b"\x89PNG\r\n\x1a\n" { return HashMap::new(); }

    let mut out = HashMap::new();
    let mut i = 8usize;
    while i + 12 <= data.len() {
        let len = u32::from_be_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]) as usize;
        let typ = &data[i + 4..i + 8];
        if i + 12 + len > data.len() { break; }
        let body = &data[i + 8..i + 8 + len];

        if typ == b"tEXt" {
            if let Some(z) = body.iter().position(|b| *b == 0) {
                out.insert(
                    String::from_utf8_lossy(&body[..z]).to_string(),
                    String::from_utf8_lossy(&body[z + 1..]).to_string(),
                );
            }
        } else if typ == b"iTXt" {
            if let Some(z) = body.iter().position(|b| *b == 0) {
                let key = String::from_utf8_lossy(&body[..z]).to_string();
                let rest = &body[z + 1..];
                if rest.len() > 4 && rest[0] == 0 {
                    let mut p = 2usize;
                    for _ in 0..2 {
                        if let Some(k) = rest[p..].iter().position(|b| *b == 0) { p += k + 1; } else { break; }
                    }
                    if p <= rest.len() {
                        out.insert(key, String::from_utf8_lossy(&rest[p..]).to_string());
                    }
                }
            }
        }
        i += 12 + len;
        if typ == b"IEND" { break; }
    }
    out
}

fn parse_a1111_parameters(input: &str) -> (String, String, Map<String, Value>) {
    let mut prompt = input.to_string();
    let mut negative = String::new();
    let mut generation = Map::new();

    if let Some(pos) = input.rfind("\nSteps:") {
        let settings = input[pos + 1..].trim();
        prompt = input[..pos].trim().to_string();
        for part in settings.split(',') {
            let mut kv = part.trim().splitn(2, ':');
            let (Some(k), Some(v)) = (kv.next(), kv.next()) else { continue };
            let key = match k.trim() {
                "CFG scale" => "cfg_scale",
                "Seed" => "seed",
                "Steps" => "steps",
                "Sampler" => "sampler",
                "Size" => "size",
                "Model" => "model",
                other => other,
            };
            generation.insert(key.to_string(), Value::String(v.trim().to_string()));
        }
    }
    if let Some(pos) = prompt.find("\nNegative prompt:") {
        negative = prompt[pos + "\nNegative prompt:".len()..].trim().to_string();
        prompt = prompt[..pos].trim().to_string();
    }
    (prompt, negative, generation)
}

#[derive(Default)]
pub struct GenerationExtract {
    pub prompt: String,
    pub negative_prompt: String,
    pub model: String,
    pub metadata_type: String,
    pub generation_json: String,
}

pub fn extract_generation(path: &Path) -> GenerationExtract {
    if path.extension().and_then(|x| x.to_str()).unwrap_or("").to_ascii_lowercase() != "png" {
        return GenerationExtract { metadata_type: "none".into(), generation_json: "{}".into(), ..Default::default() };
    }

    let text = read_png_text(path);
    if let Some(parameters) = text.get("parameters") {
        let (prompt, negative_prompt, generation) = parse_a1111_parameters(parameters);
        let model = generation.get("model").and_then(Value::as_str).unwrap_or("").to_string();
        return GenerationExtract {
            prompt,
            negative_prompt,
            model,
            metadata_type: "a1111".into(),
            generation_json: Value::Object(generation).to_string(),
        };
    }

    let prompt_json = text.get("prompt").and_then(|x| serde_json::from_str::<Value>(x).ok());
    let workflow_json = text.get("workflow").and_then(|x| serde_json::from_str::<Value>(x).ok());
    if prompt_json.is_some() || workflow_json.is_some() {
        let payload = serde_json::json!({"prompt": prompt_json, "workflow": workflow_json});
        return GenerationExtract {
            metadata_type: "comfyui".into(),
            generation_json: payload.to_string(),
            ..Default::default()
        };
    }

    GenerationExtract { metadata_type: "none".into(), generation_json: "{}".into(), ..Default::default() }
}
