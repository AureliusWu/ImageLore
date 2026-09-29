use crate::{
    db,
    models::{AssetSummary, LibraryFilter, LibraryPage},
};
use rusqlite::{params_from_iter, types::Value as SqlValue, Connection};

fn fts_query(input: &str) -> String {
    input
        .split_whitespace()
        .filter(|x| !x.is_empty())
        .map(|x| format!("\"{}\"*", x.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" AND ")
}

fn cjk_trigram_query(input: &str) -> Option<String> {
    let terms = input
        .split_whitespace()
        .filter(|term| term.chars().count() >= 3)
        .map(|term| format!("\"{}\"", term.replace('"', "\"\"")))
        .collect::<Vec<_>>();
    if terms.is_empty() {
        None
    } else {
        Some(terms.join(" AND "))
    }
}

fn filter_parts(filter: &LibraryFilter) -> (String, String, Vec<SqlValue>) {
    let mut joins = " LEFT JOIN generation_index gi ON gi.asset_id=a.id LEFT JOIN visual_dna vd ON vd.asset_id=a.id ".to_string();
    let mut where_parts = vec!["1=1".to_string()];
    let mut args = Vec::<SqlValue>::new();

    let query = filter.query.trim();
    if !query.is_empty() {
        let has_cjk = query.chars().any(
            |c| matches!(c,'\u{3400}'..='\u{9fff}'|'\u{3040}'..='\u{30ff}'|'\u{ac00}'..='\u{d7af}'),
        );
        if has_cjk {
            if let Some(trigram) = cjk_trigram_query(query) {
                joins.push_str(" JOIN asset_cjk_search ON asset_cjk_search.asset_id=a.id ");
                where_parts.push("asset_cjk_search MATCH ?".into());
                args.push(SqlValue::Text(trigram));
            }
            for term in query.split_whitespace().filter(|x| !x.is_empty()) {
                where_parts.push("(a.name LIKE ? OR COALESCE(ps.prompt,'') LIKE ? OR COALESCE(ps.negative_prompt,'') LIKE ? OR COALESCE(ps.model,'') LIKE ? OR EXISTS(SELECT 1 FROM asset_tags sq_at JOIN tags sq_t ON sq_t.id=sq_at.tag_id WHERE sq_at.asset_id=a.id AND sq_t.name LIKE ?) OR COALESCE(vd.search_text,'') LIKE ? OR EXISTS(SELECT 1 FROM reference_sources rs WHERE rs.asset_id=a.id AND (rs.page_title LIKE ? OR rs.page_url LIKE ? OR rs.source_url LIKE ? OR rs.source_type LIKE ? OR rs.metadata_json LIKE ?)))".into());
                let needle = SqlValue::Text(format!("%{}%", term));
                for _ in 0..11 {
                    args.push(needle.clone())
                }
            }
        } else {
            joins.push_str(" JOIN asset_search ON asset_search.asset_id=a.id ");
            where_parts.push("asset_search MATCH ?".into());
            args.push(SqlValue::Text(fts_query(query)));
        }
    }
    match filter.view.as_str() {
        "favorites" => where_parts.push("a.favorite=1".into()),
        "missing" => where_parts.push("a.missing=1".into()),
        "recent" => {
            where_parts.push("a.updated_at>=?".into());
            args.push(SqlValue::Integer(db::now() - 30 * 24 * 3600));
        }
        _ => {}
    }
    if let Some(tag) = filter.tag.as_ref().filter(|x| !x.trim().is_empty()) {
        where_parts.push("EXISTS(SELECT 1 FROM asset_tags fx JOIN tags ft ON ft.id=fx.tag_id WHERE fx.asset_id=a.id AND ft.name=? COLLATE NOCASE)".into());
        args.push(SqlValue::Text(tag.clone()));
    }
    if let Some(model) = filter.model.as_ref().filter(|x| !x.trim().is_empty()) {
        where_parts.push("COALESCE((SELECT ma.canonical FROM model_aliases ma WHERE ma.alias=ps.model COLLATE NOCASE LIMIT 1),COALESCE(ps.model,''))=? COLLATE NOCASE".into());
        args.push(SqlValue::Text(model.clone()));
    }
    if let Some(collection_id) = filter.collection_id {
        where_parts.push("EXISTS(SELECT 1 FROM collection_assets ca WHERE ca.asset_id=a.id AND ca.collection_id=?)".into());
        args.push(SqlValue::Integer(collection_id));
    }
    if let Some(value) = filter
        .metadata_type
        .as_ref()
        .filter(|x| !x.trim().is_empty())
    {
        where_parts.push("a.metadata_type=? COLLATE NOCASE".into());
        args.push(SqlValue::Text(value.clone()));
    }
    if let Some(value) = filter.sampler.as_ref().filter(|x| !x.trim().is_empty()) {
        where_parts.push("COALESCE(gi.sampler,'')=? COLLATE NOCASE".into());
        args.push(SqlValue::Text(value.clone()));
    }
    if let Some(value) = filter.scheduler.as_ref().filter(|x| !x.trim().is_empty()) {
        where_parts.push("COALESCE(gi.scheduler,'')=? COLLATE NOCASE".into());
        args.push(SqlValue::Text(value.clone()));
    }
    if let Some(value) = filter.seed.as_ref().filter(|x| !x.trim().is_empty()) {
        where_parts.push("COALESCE(gi.seed,'')=?".into());
        args.push(SqlValue::Text(value.trim().to_string()));
    }
    if let Some(value) = filter.steps_min {
        where_parts.push("gi.steps>=?".into());
        args.push(SqlValue::Integer(value))
    }
    if let Some(value) = filter.steps_max {
        where_parts.push("gi.steps<=?".into());
        args.push(SqlValue::Integer(value))
    }
    if let Some(value) = filter.cfg_min {
        where_parts.push("gi.cfg_scale>=?".into());
        args.push(SqlValue::Real(value))
    }
    if let Some(value) = filter.cfg_max {
        where_parts.push("gi.cfg_scale<=?".into());
        args.push(SqlValue::Real(value))
    }
    if let Some(value) = filter.denoise_min {
        where_parts.push("gi.denoise>=?".into());
        args.push(SqlValue::Real(value))
    }
    if let Some(value) = filter.denoise_max {
        where_parts.push("gi.denoise<=?".into());
        args.push(SqlValue::Real(value))
    }
    match filter.orientation.as_deref() {
        Some("landscape") => where_parts
            .push("a.width IS NOT NULL AND a.height IS NOT NULL AND a.width>a.height".into()),
        Some("portrait") => where_parts
            .push("a.width IS NOT NULL AND a.height IS NOT NULL AND a.height>a.width".into()),
        Some("square") => where_parts
            .push("a.width IS NOT NULL AND a.height IS NOT NULL AND a.width=a.height".into()),
        _ => {}
    }
    (joins, where_parts.join(" AND "), args)
}

pub(crate) fn filtered_summaries(
    conn: &Connection,
    filter: &LibraryFilter,
    limit: i64,
) -> Result<Vec<AssetSummary>, String> {
    let (joins, where_sql, mut args) = filter_parts(filter);
    let limit = limit.clamp(1, 100_000);
    let sql = format!(
        "{} {} WHERE {} GROUP BY a.id LIMIT ?",
        db::SELECT_SUMMARY, joins, where_sql
    );
    args.push(SqlValue::Integer(limit));
    let mut st = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = st
        .query_map(params_from_iter(args), db::row_summary)
        .map_err(|e| e.to_string())?;
    Ok(rows.filter_map(Result::ok).collect())
}

pub fn library_page(
    conn: &Connection,
    filter: &LibraryFilter,
    offset: i64,
    limit: i64,
) -> Result<LibraryPage, String> {
    let offset = offset.max(0);
    let limit = limit.clamp(20, 500);
    let (joins, where_sql, args) = filter_parts(filter);

    let count_sql=format!("SELECT COUNT(DISTINCT a.id) FROM assets a LEFT JOIN prompt_state ps ON ps.asset_id=a.id {} WHERE {}",joins,where_sql);
    let total: i64 = conn
        .query_row(&count_sql, params_from_iter(args.clone()), |r| r.get(0))
        .map_err(|e| e.to_string())?;

    let order = match filter.sort.as_str() {
        "updated_desc" => "a.updated_at DESC,a.id DESC",
        "updated_asc" => "a.updated_at ASC,a.id ASC",
        "created_desc" => "a.created_at DESC,a.id DESC",
        "created_asc" => "a.created_at ASC,a.id ASC",
        "name_asc" => "a.name COLLATE NOCASE ASC,a.id ASC",
        "name_desc" => "a.name COLLATE NOCASE DESC,a.id DESC",
        "resolution_desc" => {
            "(COALESCE(a.width,0)*COALESCE(a.height,0)) DESC,a.updated_at DESC,a.id DESC"
        }
        "size_desc" => "COALESCE(a.file_size,0) DESC,a.updated_at DESC,a.id DESC",
        _ if filter.view == "recent" => "a.updated_at DESC,a.id DESC",
        _ => "a.favorite DESC,a.updated_at DESC,a.id DESC",
    };
    let sql = format!(
        "{} {} WHERE {} ORDER BY {} LIMIT ? OFFSET ?",
        db::SELECT_SUMMARY, joins, where_sql, order
    );
    let mut page_args = args;
    page_args.push(SqlValue::Integer(limit));
    page_args.push(SqlValue::Integer(offset));
    let mut st = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let items = st
        .query_map(params_from_iter(page_args), db::row_summary)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .collect();
    Ok(LibraryPage {
        items,
        total,
        offset,
        limit,
    })
}
