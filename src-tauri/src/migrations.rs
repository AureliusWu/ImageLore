use rusqlite::{params,Connection,OptionalExtension};
use sha2::{Digest,Sha256};

const LATEST:i64=3;

fn set_version(conn:&Connection,version:i64)->Result<(),String>{
    conn.execute(
        "INSERT INTO app_meta(key,value) VALUES('schema_version',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        params![version.to_string()]
    ).map_err(|e|e.to_string())?;
    Ok(())
}

fn legacy_portable_id(id:i64,path:&str,fingerprint:&str,created_at:i64)->String{
    let mut hash=Sha256::new();
    hash.update(format!("ImageLore:v3:{}:{}:{}:{}",id,created_at,fingerprint,path).as_bytes());
    let hex=format!("{:x}",hash.finalize());
    format!("il-{}",&hex[..32])
}

fn has_column(conn:&Connection,table:&str,column:&str)->Result<bool,String>{
    let sql=format!("PRAGMA table_info({})",table);
    let mut st=conn.prepare(&sql).map_err(|e|e.to_string())?;
    let mut rows=st.query([]).map_err(|e|e.to_string())?;
    while let Some(row)=rows.next().map_err(|e|e.to_string())?{
        let name:String=row.get(1).map_err(|e|e.to_string())?;
        if name==column{return Ok(true)}
    }
    Ok(false)
}

fn migrate_v3(conn:&Connection)->Result<(),String>{
    if !has_column(conn,"assets","portable_id")?{
        conn.execute("ALTER TABLE assets ADD COLUMN portable_id TEXT NOT NULL DEFAULT ''",[]).map_err(|e|e.to_string())?;
    }
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS generation_sessions (
           id INTEGER PRIMARY KEY AUTOINCREMENT,
           name TEXT NOT NULL UNIQUE COLLATE NOCASE,
           note TEXT NOT NULL DEFAULT '',
           created_at INTEGER NOT NULL,
           updated_at INTEGER NOT NULL
         );
         CREATE TABLE IF NOT EXISTS asset_sessions (
           asset_id INTEGER PRIMARY KEY,
           session_id INTEGER NOT NULL,
           note TEXT NOT NULL DEFAULT '',
           created_at INTEGER NOT NULL,
           updated_at INTEGER NOT NULL,
           FOREIGN KEY(asset_id) REFERENCES assets(id) ON DELETE CASCADE,
           FOREIGN KEY(session_id) REFERENCES generation_sessions(id) ON DELETE CASCADE
         );
         CREATE INDEX IF NOT EXISTS idx_asset_sessions_session ON asset_sessions(session_id,asset_id);
         CREATE TABLE IF NOT EXISTS model_aliases (
           alias TEXT PRIMARY KEY COLLATE NOCASE,
           canonical TEXT NOT NULL COLLATE NOCASE,
           created_at INTEGER NOT NULL,
           updated_at INTEGER NOT NULL
         );
         CREATE TABLE IF NOT EXISTS saved_filters (
           id INTEGER PRIMARY KEY AUTOINCREMENT,
           name TEXT NOT NULL UNIQUE COLLATE NOCASE,
           filter_json TEXT NOT NULL,
           created_at INTEGER NOT NULL,
           updated_at INTEGER NOT NULL
         );
         CREATE TABLE IF NOT EXISTS pending_relations (
           child_portable_id TEXT NOT NULL,
           parent_portable_id TEXT NOT NULL DEFAULT '',
           parent_fingerprint TEXT NOT NULL DEFAULT '',
           relation_type TEXT NOT NULL DEFAULT 'reference',
           note TEXT NOT NULL DEFAULT '',
           created_at INTEGER NOT NULL,
           UNIQUE(child_portable_id,parent_portable_id,parent_fingerprint,relation_type)
         );
         CREATE INDEX IF NOT EXISTS idx_pending_relations_child ON pending_relations(child_portable_id);"
    ).map_err(|e|e.to_string())?;

    let rows:Vec<(i64,String,String,i64)>={
        let mut st=conn.prepare("SELECT id,path,fingerprint,created_at FROM assets WHERE portable_id=''").map_err(|e|e.to_string())?;
        let mapped=st.query_map([],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).map_err(|e|e.to_string())?;
        let collected=mapped.filter_map(Result::ok).collect();
        collected
    };
    for(id,path,fingerprint,created_at)in rows{
        let portable_id=legacy_portable_id(id,&path,&fingerprint,created_at);
        conn.execute("UPDATE assets SET portable_id=?1 WHERE id=?2",params![portable_id,id]).map_err(|e|e.to_string())?;
    }
    conn.execute_batch("CREATE UNIQUE INDEX IF NOT EXISTS idx_assets_portable_id_nonempty ON assets(portable_id) WHERE portable_id<>'';").map_err(|e|e.to_string())?;
    Ok(())
}

pub fn apply(conn:&Connection)->Result<(),String>{
    conn.execute_batch("CREATE TABLE IF NOT EXISTS app_meta(key TEXT PRIMARY KEY,value TEXT NOT NULL);").map_err(|e|e.to_string())?;
    let current:Option<String>=conn.query_row(
        "SELECT value FROM app_meta WHERE key='schema_version'",[],|r|r.get(0)
    ).optional().map_err(|e|e.to_string())?;
    let mut version=current.and_then(|x|x.parse::<i64>().ok()).unwrap_or(0);

    if version>LATEST{return Err(format!("数据库版本 {} 高于当前程序支持的 {}",version,LATEST))}

    if version==0{
        conn.execute_batch(include_str!("../schema.sql")).map_err(|e|e.to_string())?;
        set_version(conn,LATEST)?;
        return Ok(())
    }

    if version<2{
        conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_assets_fingerprint_nonempty ON assets(fingerprint) WHERE fingerprint<>'';"
        ).map_err(|e|e.to_string())?;
        version=2;
        set_version(conn,version)?;
    }

    if version<3{
        migrate_v3(conn)?;
        version=3;
        set_version(conn,version)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests{
    use super::*;

    #[test]
    fn migration_v3_is_rerunnable_after_column_exists(){
        let conn=Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE app_meta(key TEXT PRIMARY KEY,value TEXT NOT NULL);
             INSERT INTO app_meta VALUES('schema_version','2');
             CREATE TABLE assets(id INTEGER PRIMARY KEY,path TEXT NOT NULL,fingerprint TEXT NOT NULL DEFAULT '',portable_id TEXT NOT NULL DEFAULT '',created_at INTEGER NOT NULL);
             INSERT INTO assets(id,path,fingerprint,portable_id,created_at) VALUES(1,'x.png','abc','',10);"
        ).unwrap();
        migrate_v3(&conn).unwrap();
        migrate_v3(&conn).unwrap();
        let value:String=conn.query_row("SELECT portable_id FROM assets WHERE id=1",[],|r|r.get(0)).unwrap();
        assert!(value.starts_with("il-"));
    }

    #[test]
    fn migrates_v2_assets_to_portable_ids(){
        let conn=Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE app_meta(key TEXT PRIMARY KEY,value TEXT NOT NULL);
             INSERT INTO app_meta VALUES('schema_version','2');
             CREATE TABLE assets(id INTEGER PRIMARY KEY,path TEXT NOT NULL,fingerprint TEXT NOT NULL DEFAULT '',created_at INTEGER NOT NULL);
             INSERT INTO assets(id,path,fingerprint,created_at) VALUES(1,'x.png','abc',10);"
        ).unwrap();
        migrate_v3(&conn).unwrap();
        let value:String=conn.query_row("SELECT portable_id FROM assets WHERE id=1",[],|r|r.get(0)).unwrap();
        assert!(value.starts_with("il-"));
        assert_eq!(value.len(),35);
    }
}
