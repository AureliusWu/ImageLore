use crate::{
    db,
    models::{ImagePromptAnalysis, VisionSettings, VisualDna, VisualDnaPatch},
    state::AppState,
    visual_dna,
};
use base64::Engine;
use image::GenericImageView;
use reqwest::Client;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{path::Path, time::Duration};
use tauri::State;

const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";
const PROVIDER: &str = "openai-compatible";

#[derive(Debug, Deserialize)]
struct RawAnalysis {
    #[serde(default)]
    summary: String,
    #[serde(default)]
    prompt: String,
    #[serde(default)]
    visual_dna: VisualDnaPatch,
}

fn normalize_text(value: &str, limit: usize) -> String {
    value.trim().chars().take(limit).collect()
}

fn normalize_base_url(value: &str) -> Result<String, String> {
    let clean = value.trim().trim_end_matches('/').to_string();
    if clean.is_empty() {
        return Err("Vision Base URL 不能为空".into());
    }
    if !(clean.starts_with("https://") || clean.starts_with("http://")) {
        return Err("Vision Base URL 必须使用 http:// 或 https://".into());
    }
    Ok(clean)
}

fn chat_endpoint(base: &str) -> String {
    let base = base.trim().trim_end_matches('/');
    if base.ends_with("/chat/completions") {
        base.to_string()
    } else {
        format!("{}/chat/completions", base)
    }
}

fn clean_json_payload(input: &str) -> Result<Value, String> {
    let mut text = input.trim();
    if text.starts_with("```") {
        if let Some(pos) = text.find('\n') {
            text = &text[pos + 1..];
        }
        if let Some(pos) = text.rfind("```") {
            text = &text[..pos];
        }
        text = text.trim();
    }
    let start = text.find('{').ok_or("视觉模型没有返回 JSON 对象")?;
    let end = text.rfind('}').ok_or("视觉模型返回的 JSON 不完整")?;
    serde_json::from_str::<Value>(&text[start..=end])
        .map_err(|e| format!("无法解析视觉模型 JSON：{}", e))
}

fn parse_analysis(input: &str) -> Result<RawAnalysis, String> {
    let value = clean_json_payload(input)?;
    let mut result: RawAnalysis = serde_json::from_value(value)
        .map_err(|e| format!("视觉模型 JSON 字段不符合约定：{}", e))?;
    result.summary = normalize_text(&result.summary, 2400);
    result.prompt = normalize_text(&result.prompt, 12000);
    result.visual_dna.source = "ai".into();
    result.visual_dna = visual_dna::normalize_patch(result.visual_dna);
    if result.prompt.is_empty() && visual_dna::patch_is_empty(&result.visual_dna) {
        return Err("视觉模型返回内容为空".into());
    }
    Ok(result)
}

fn image_data_url(path: &Path) -> Result<String, String> {
    if !path.exists() {
        return Err("原图片文件不存在，无法分析".into());
    }
    let image = image::ImageReader::open(path)
        .map_err(|e| e.to_string())?
        .with_guessed_format()
        .map_err(|e| e.to_string())?
        .decode()
        .map_err(|e| format!("读取图片失败：{}", e))?;
    let (w, h) = image.dimensions();
    let resized = if w.max(h) > 1600 {
        image.thumbnail(1600, 1600)
    } else {
        image
    };
    let rgb = resized.to_rgb8();
    let mut bytes = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 88)
        .encode(
            &rgb,
            rgb.width(),
            rgb.height(),
            image::ExtendedColorType::Rgb8,
        )
        .map_err(|e| format!("准备视觉分析图片失败：{}", e))?;
    Ok(format!(
        "data:image/jpeg;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

fn settings_from_db(conn: &Connection, key_configured: bool) -> Result<VisionSettings, String> {
    let row: Option<(String, String)> = conn
        .query_row(
            "SELECT base_url,model FROM vision_settings WHERE id=1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let (base_url, model) = row.unwrap_or_else(|| (DEFAULT_BASE_URL.into(), String::new()));
    Ok(VisionSettings {
        base_url,
        model,
        api_key_configured: key_configured,
    })
}

fn analysis_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<ImagePromptAnalysis> {
    let dna_json: String = r.get(6)?;
    Ok(ImagePromptAnalysis {
        id: r.get(0)?,
        asset_id: r.get(1)?,
        provider: r.get(2)?,
        model: r.get(3)?,
        summary: r.get(4)?,
        prompt: r.get(5)?,
        visual_dna: serde_json::from_str(&dna_json).unwrap_or_default(),
        created_at: r.get(7)?,
    })
}

fn get_analysis(conn: &Connection, id: i64) -> Result<ImagePromptAnalysis, String> {
    conn.query_row(
        "SELECT id,asset_id,provider,model,summary,prompt,visual_dna_json,created_at
         FROM image_prompt_analyses WHERE id=?1",
        params![id],
        analysis_row,
    )
    .map_err(|e| e.to_string())
}

fn merged_dna(current: &VisualDna, incoming: &VisualDnaPatch, overwrite: bool) -> VisualDnaPatch {
    let pick = |old: &str, new: &str| {
        if overwrite || old.trim().is_empty() {
            new.to_string()
        } else {
            old.to_string()
        }
    };
    let had_current = !visual_dna::record_is_empty(current);
    let adds_ai = [
        (&current.subject, &incoming.subject),
        (&current.character, &incoming.character),
        (&current.outfit, &incoming.outfit),
        (&current.pose, &incoming.pose),
        (&current.expression, &incoming.expression),
        (&current.composition, &incoming.composition),
        (&current.camera, &incoming.camera),
        (&current.lighting, &incoming.lighting),
        (&current.environment, &incoming.environment),
        (&current.palette, &incoming.palette),
        (&current.material, &incoming.material),
        (&current.style, &incoming.style),
    ]
    .iter()
    .any(|(old, new)| !new.trim().is_empty() && (overwrite || old.trim().is_empty()));
    VisualDnaPatch {
        subject: pick(&current.subject, &incoming.subject),
        character: pick(&current.character, &incoming.character),
        outfit: pick(&current.outfit, &incoming.outfit),
        pose: pick(&current.pose, &incoming.pose),
        expression: pick(&current.expression, &incoming.expression),
        composition: pick(&current.composition, &incoming.composition),
        camera: pick(&current.camera, &incoming.camera),
        lighting: pick(&current.lighting, &incoming.lighting),
        environment: pick(&current.environment, &incoming.environment),
        palette: pick(&current.palette, &incoming.palette),
        material: pick(&current.material, &incoming.material),
        style: pick(&current.style, &incoming.style),
        source: if had_current && adds_ai && !overwrite {
            "mixed".into()
        } else if adds_ai {
            "ai".into()
        } else {
            current.source.clone()
        },
    }
}

#[tauri::command]
pub fn vision_settings(state: State<'_, AppState>) -> Result<VisionSettings, String> {
    let key_configured = !state
        .vision_api_key
        .lock()
        .map_err(|e| e.to_string())?
        .trim()
        .is_empty();
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    settings_from_db(&conn, key_configured)
}

#[tauri::command]
pub fn save_vision_settings(
    state: State<'_, AppState>,
    base_url: String,
    model: String,
) -> Result<VisionSettings, String> {
    let base_url = normalize_base_url(&base_url)?;
    let model = normalize_text(&model, 200);
    let key_configured = !state
        .vision_api_key
        .lock()
        .map_err(|e| e.to_string())?
        .trim()
        .is_empty();
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO vision_settings(id,base_url,model,updated_at) VALUES(1,?1,?2,?3)
         ON CONFLICT(id) DO UPDATE SET base_url=excluded.base_url,model=excluded.model,updated_at=excluded.updated_at",
        params![base_url,model,db::now()]
    ).map_err(|e|e.to_string())?;
    settings_from_db(&conn, key_configured)
}

#[tauri::command]
pub fn set_vision_api_key(state: State<'_, AppState>, api_key: String) -> Result<bool, String> {
    let mut key = state.vision_api_key.lock().map_err(|e| e.to_string())?;
    *key = api_key.trim().chars().take(4096).collect();
    Ok(!key.is_empty())
}

#[tauri::command]
pub fn latest_image_prompt_analysis(
    state: State<'_, AppState>,
    id: i64,
) -> Result<Option<ImagePromptAnalysis>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    conn.query_row(
        "SELECT id,asset_id,provider,model,summary,prompt,visual_dna_json,created_at
         FROM image_prompt_analyses WHERE asset_id=?1 ORDER BY created_at DESC,id DESC LIMIT 1",
        params![id],
        analysis_row,
    )
    .optional()
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn analyze_image_to_prompt(
    state: State<'_, AppState>,
    id: i64,
) -> Result<ImagePromptAnalysis, String> {
    let (path, settings, key) = {
        let key = state
            .vision_api_key
            .lock()
            .map_err(|e| e.to_string())?
            .clone();
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        let asset = db::get_asset(&conn, id)?;
        let settings = settings_from_db(&conn, !key.trim().is_empty())?;
        (asset.path, settings, key)
    };
    let model = settings.model.trim();
    if model.is_empty() {
        return Err("请先在「资料库管理 → 图像分析」配置视觉模型名称".into());
    }
    let image_url = image_data_url(Path::new(&path))?;
    let endpoint = chat_endpoint(&normalize_base_url(&settings.base_url)?);
    let system = r#"你是 ImageLore 的图像分析器。只分析可见内容，不猜测看不见的身份、品牌或拍摄背景。返回且只返回一个 JSON 对象：
{
  "summary":"用简体中文概括画面的视觉逻辑，80-240字",
  "visual_dna":{
    "subject":"","character":"","outfit":"","pose":"","expression":"",
    "composition":"","camera":"","lighting":"","environment":"",
    "palette":"","material":"","style":"","source":"ai"
  },
  "prompt":"可直接用于图像生成的高质量提示词。使用简体中文，必要的摄影/渲染术语可保留英文。忠实描述画面，不添加不可见事实。"
}
Visual DNA 每个字段尽量简洁、具体、可复用；无法判断就留空。"#;
    let request = json!({
        "model":model,
        "temperature":0.2,
        "messages":[
            {"role":"system","content":system},
            {"role":"user","content":[
                {"type":"text","text":"分析这张图片，提取 Visual DNA，并给出可复现画面视觉特征的生成 Prompt。"},
                {"type":"image_url","image_url":{"url":image_url}}
            ]}
        ]
    });
    let client = Client::builder()
        .timeout(Duration::from_secs(90))
        .build()
        .map_err(|e| e.to_string())?;
    let mut builder = client.post(endpoint).json(&request);
    if !key.trim().is_empty() {
        builder = builder.bearer_auth(key.trim());
    }
    let response = builder
        .send()
        .await
        .map_err(|e| format!("视觉模型请求失败：{}", e))?;
    let status = response.status();
    let body = response.text().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        let short: String = body.chars().take(500).collect();
        return Err(format!("视觉模型返回 HTTP {}：{}", status.as_u16(), short));
    }
    let envelope: Value =
        serde_json::from_str(&body).map_err(|e| format!("无法解析视觉模型响应：{}", e))?;
    let content = envelope
        .pointer("/choices/0/message/content")
        .and_then(Value::as_str)
        .ok_or("视觉模型响应缺少 choices[0].message.content")?;
    let parsed = parse_analysis(content)?;
    let dna_json = serde_json::to_string(&parsed.visual_dna).map_err(|e| e.to_string())?;
    let analysis_id = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO image_prompt_analyses(asset_id,provider,model,summary,prompt,visual_dna_json,created_at)
             VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![id,PROVIDER,model,parsed.summary,parsed.prompt,dna_json,db::now()]
        ).map_err(|e|e.to_string())?;
        conn.last_insert_rowid()
    };
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    get_analysis(&conn, analysis_id)
}

#[tauri::command]
pub fn apply_image_prompt_dna(
    state: State<'_, AppState>,
    id: i64,
    analysis_id: i64,
    overwrite: bool,
) -> Result<VisualDna, String> {
    let mut conn = state.db.lock().map_err(|e| e.to_string())?;
    let analysis = get_analysis(&conn, analysis_id)?;
    if analysis.asset_id != id {
        return Err("分析记录与当前图片不匹配".into());
    }
    let current = visual_dna::get(&conn, id)?;
    let merged = merged_dna(&current, &analysis.visual_dna, overwrite);
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let saved = visual_dna::upsert(&tx, id, &merged)?;
    tx.execute(
        "UPDATE assets SET updated_at=?1 WHERE id=?2",
        params![db::now(), id],
    )
    .map_err(|e| e.to_string())?;
    db::reindex_asset(&tx, id)?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(saved)
}

#[tauri::command]
pub fn save_image_prompt_revision(
    state: State<'_, AppState>,
    id: i64,
    analysis_id: i64,
) -> Result<bool, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let analysis = get_analysis(&conn, analysis_id)?;
    if analysis.asset_id != id {
        return Err("分析记录与当前图片不匹配".into());
    }
    if analysis.prompt.trim().is_empty() {
        return Err("分析记录没有可保存的 Prompt".into());
    }
    let asset = db::get_asset(&conn, id)?;
    let tags_json = serde_json::to_string(&asset.tags).map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO prompt_revisions(asset_id,prompt,negative_prompt,model,tags_json,note,created_at)
         VALUES(?1,?2,?3,?4,?5,?6,?7)",
        params![id,analysis.prompt,asset.negative_prompt,asset.model,tags_json,format!("Image to Prompt · {}",analysis.model),db::now()]
    ).map_err(|e|e.to_string())?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_accepts_base_or_full_chat_url() {
        assert_eq!(
            chat_endpoint("https://example.test/v1"),
            "https://example.test/v1/chat/completions"
        );
        assert_eq!(
            chat_endpoint("https://example.test/v1/chat/completions"),
            "https://example.test/v1/chat/completions"
        );
    }

    #[test]
    fn parses_fenced_json_and_normalizes_ai_source() {
        let raw="```json\n{\"summary\":\"test\",\"visual_dna\":{\"subject\":\" adult woman \",\"lighting\":\"soft light\"},\"prompt\":\"portrait prompt\"}\n```";
        let parsed = parse_analysis(raw).unwrap();
        assert_eq!(parsed.visual_dna.subject, "adult woman");
        assert_eq!(parsed.visual_dna.source, "ai");
        assert_eq!(parsed.prompt, "portrait prompt");
    }

    #[test]
    fn merge_preserves_existing_manual_fields_by_default() {
        let current = VisualDna {
            subject: "手工主体".into(),
            lighting: "".into(),
            source: "manual".into(),
            ..Default::default()
        };
        let incoming = VisualDnaPatch {
            subject: "AI主体".into(),
            lighting: "窗边柔光".into(),
            source: "ai".into(),
            ..Default::default()
        };
        let merged = merged_dna(&current, &incoming, false);
        assert_eq!(merged.subject, "手工主体");
        assert_eq!(merged.lighting, "窗边柔光");
        assert_eq!(merged.source, "mixed");
        let overwritten = merged_dna(&current, &incoming, true);
        assert_eq!(overwritten.subject, "AI主体");
        assert_eq!(overwritten.source, "ai");
    }
}
