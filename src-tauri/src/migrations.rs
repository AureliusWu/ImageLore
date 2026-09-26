use rusqlite::{params,Connection,OptionalExtension};

const LATEST:i64=1;

pub fn apply(conn:&Connection)->Result<(),String>{
    conn.execute_batch("CREATE TABLE IF NOT EXISTS app_meta(key TEXT PRIMARY KEY,value TEXT NOT NULL);").map_err(|e|e.to_string())?;
    let current:Option<String>=conn.query_row("SELECT value FROM app_meta WHERE key='schema_version'",[],|r|r.get(0)).optional().map_err(|e|e.to_string())?;
    let version=current.and_then(|x|x.parse::<i64>().ok()).unwrap_or(0);
    if version>LATEST{return Err(format!("数据库版本 {} 高于当前程序支持的 {}",version,LATEST))}
    if version==0{
        conn.execute_batch(include_str!("../schema.sql")).map_err(|e|e.to_string())?;
        conn.execute("INSERT INTO app_meta(key,value) VALUES('schema_version',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",params![LATEST.to_string()]).map_err(|e|e.to_string())?;
    }
    Ok(())
}
