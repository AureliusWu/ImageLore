use crate::generation_index;
use rusqlite::{params,Connection,OptionalExtension};
use sha2::{Digest,Sha256};

const LATEST:i64=11;

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

fn has_table(conn:&Connection,table:&str)->Result<bool,String>{
    let count:i64=conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE name=?1",
        params![table],
        |r|r.get(0)
    ).map_err(|e|e.to_string())?;
    Ok(count>0)
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

fn migrate_v4(conn:&Connection)->Result<(),String>{
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS source_folders (
           id INTEGER PRIMARY KEY AUTOINCREMENT,
           path TEXT NOT NULL UNIQUE COLLATE NOCASE,
           name TEXT NOT NULL,
           auto_sync INTEGER NOT NULL DEFAULT 1 CHECK (auto_sync IN (0,1)),
           last_scan_at INTEGER NOT NULL DEFAULT 0,
           created_at INTEGER NOT NULL,
           updated_at INTEGER NOT NULL
         );
         CREATE INDEX IF NOT EXISTS idx_source_folders_auto_sync ON source_folders(auto_sync,name COLLATE NOCASE);"
    ).map_err(|e|e.to_string())
}

fn migrate_v5(conn:&Connection)->Result<(),String>{
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS generation_index (
           asset_id INTEGER PRIMARY KEY,
           seed TEXT NOT NULL DEFAULT '',
           steps INTEGER,
           sampler TEXT NOT NULL DEFAULT '',
           scheduler TEXT NOT NULL DEFAULT '',
           cfg_scale REAL,
           denoise REAL,
           FOREIGN KEY(asset_id) REFERENCES assets(id) ON DELETE CASCADE
         );
         CREATE INDEX IF NOT EXISTS idx_generation_seed ON generation_index(seed) WHERE seed<>'';
         CREATE INDEX IF NOT EXISTS idx_generation_sampler ON generation_index(sampler COLLATE NOCASE) WHERE sampler<>'';
         CREATE INDEX IF NOT EXISTS idx_generation_scheduler ON generation_index(scheduler COLLATE NOCASE) WHERE scheduler<>'';
         CREATE INDEX IF NOT EXISTS idx_generation_steps ON generation_index(steps) WHERE steps IS NOT NULL;
         CREATE INDEX IF NOT EXISTS idx_generation_cfg ON generation_index(cfg_scale) WHERE cfg_scale IS NOT NULL;"
    ).map_err(|e|e.to_string())?;
    generation_index::rebuild_all(conn)?;
    Ok(())
}

fn migrate_v6(conn:&Connection)->Result<(),String>{
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS semantic_embeddings (
           asset_id INTEGER PRIMARY KEY,
           model_id TEXT NOT NULL,
           dimensions INTEGER NOT NULL,
           vector BLOB NOT NULL,
           fingerprint TEXT NOT NULL DEFAULT '',
           indexed_at INTEGER NOT NULL,
           FOREIGN KEY(asset_id) REFERENCES assets(id) ON DELETE CASCADE
         );
         CREATE INDEX IF NOT EXISTS idx_semantic_model ON semantic_embeddings(model_id,asset_id);
         CREATE INDEX IF NOT EXISTS idx_semantic_fingerprint ON semantic_embeddings(fingerprint) WHERE fingerprint<>'';
         CREATE TABLE IF NOT EXISTS semantic_settings (
           key TEXT PRIMARY KEY,
           value TEXT NOT NULL
         );
         INSERT OR IGNORE INTO semantic_settings(key,value) VALUES('enabled','0');
         INSERT OR IGNORE INTO semantic_settings(key,value) VALUES('model_id','clip-vit-b32-qdrant-v1');"
    ).map_err(|e|e.to_string())
}

fn migrate_v7(conn:&Connection)->Result<(),String>{
    conn.execute_batch(
        "CREATE VIRTUAL TABLE IF NOT EXISTS asset_cjk_search USING fts5(
           asset_id UNINDEXED,
           text,
           tokenize='trigram'
         );"
    ).map_err(|e|e.to_string())?;

    let can_backfill=
        has_column(conn,"assets","name")? &&
        has_table(conn,"prompt_state")? &&
        has_table(conn,"tags")? &&
        has_table(conn,"asset_tags")?;
    if can_backfill{
        conn.execute("DELETE FROM asset_cjk_search",[]).map_err(|e|e.to_string())?;
        conn.execute(
            "INSERT INTO asset_cjk_search(rowid,asset_id,text)
             SELECT a.id,a.id,
                    a.name || char(31) ||
                    COALESCE(ps.prompt,'') || char(31) ||
                    COALESCE(ps.negative_prompt,'') || char(31) ||
                    COALESCE(ps.model,'') || char(31) ||
                    COALESCE((
                      SELECT GROUP_CONCAT(t.name,' ')
                      FROM asset_tags at
                      JOIN tags t ON t.id=at.tag_id
                      WHERE at.asset_id=a.id
                    ),'')
             FROM assets a
             LEFT JOIN prompt_state ps ON ps.asset_id=a.id",
            []
        ).map_err(|e|e.to_string())?;
    }
    Ok(())
}

fn migrate_v8(conn:&Connection)->Result<(),String>{
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS visual_dna (
           asset_id INTEGER PRIMARY KEY,
           subject TEXT NOT NULL DEFAULT '',
           character_name TEXT NOT NULL DEFAULT '',
           outfit TEXT NOT NULL DEFAULT '',
           pose TEXT NOT NULL DEFAULT '',
           expression TEXT NOT NULL DEFAULT '',
           composition TEXT NOT NULL DEFAULT '',
           camera TEXT NOT NULL DEFAULT '',
           lighting TEXT NOT NULL DEFAULT '',
           environment TEXT NOT NULL DEFAULT '',
           palette TEXT NOT NULL DEFAULT '',
           material TEXT NOT NULL DEFAULT '',
           style TEXT NOT NULL DEFAULT '',
           search_text TEXT NOT NULL DEFAULT '',
           source TEXT NOT NULL DEFAULT 'manual',
           updated_at INTEGER NOT NULL DEFAULT 0,
           FOREIGN KEY(asset_id) REFERENCES assets(id) ON DELETE CASCADE
         );
         DROP TABLE IF EXISTS asset_search;
         CREATE VIRTUAL TABLE asset_search USING fts5(
           asset_id UNINDEXED,
           name,
           prompt,
           negative_prompt,
           model,
           tags,
           visual_dna,
           tokenize='unicode61 remove_diacritics 2'
         );"
    ).map_err(|e|e.to_string())?;

    let can_backfill=
        has_column(conn,"assets","name")? &&
        has_table(conn,"prompt_state")? &&
        has_table(conn,"tags")? &&
        has_table(conn,"asset_tags")?;
    if can_backfill{
        conn.execute(
            "INSERT INTO asset_search(asset_id,name,prompt,negative_prompt,model,tags,visual_dna)
             SELECT a.id,a.name,COALESCE(ps.prompt,''),COALESCE(ps.negative_prompt,''),COALESCE(ps.model,''),
                    COALESCE((SELECT GROUP_CONCAT(t.name,' ') FROM asset_tags at JOIN tags t ON t.id=at.tag_id WHERE at.asset_id=a.id),''),
                    COALESCE(vd.search_text,'')
             FROM assets a
             LEFT JOIN prompt_state ps ON ps.asset_id=a.id
             LEFT JOIN visual_dna vd ON vd.asset_id=a.id",
            []
        ).map_err(|e|e.to_string())?;

        if has_table(conn,"asset_cjk_search")?{
            conn.execute("DELETE FROM asset_cjk_search",[]).map_err(|e|e.to_string())?;
            conn.execute(
                "INSERT INTO asset_cjk_search(rowid,asset_id,text)
                 SELECT a.id,a.id,
                        a.name || char(31) || COALESCE(ps.prompt,'') || char(31) || COALESCE(ps.negative_prompt,'') ||
                        char(31) || COALESCE(ps.model,'') || char(31) ||
                        COALESCE((SELECT GROUP_CONCAT(t.name,' ') FROM asset_tags at JOIN tags t ON t.id=at.tag_id WHERE at.asset_id=a.id),'') ||
                        char(31) || COALESCE(vd.search_text,'')
                 FROM assets a
                 LEFT JOIN prompt_state ps ON ps.asset_id=a.id
                 LEFT JOIN visual_dna vd ON vd.asset_id=a.id",
                []
            ).map_err(|e|e.to_string())?;
        }
    }
    Ok(())
}


fn migrate_v9(conn:&Connection)->Result<(),String>{
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS vision_settings (
           id INTEGER PRIMARY KEY CHECK (id=1),
           base_url TEXT NOT NULL DEFAULT 'https://api.openai.com/v1',
           model TEXT NOT NULL DEFAULT '',
           updated_at INTEGER NOT NULL DEFAULT 0
         );
         INSERT OR IGNORE INTO vision_settings(id,base_url,model,updated_at) VALUES(1,'https://api.openai.com/v1','',0);
         CREATE TABLE IF NOT EXISTS image_prompt_analyses (
           id INTEGER PRIMARY KEY AUTOINCREMENT,
           asset_id INTEGER NOT NULL,
           provider TEXT NOT NULL DEFAULT 'openai-compatible',
           model TEXT NOT NULL,
           summary TEXT NOT NULL DEFAULT '',
           prompt TEXT NOT NULL DEFAULT '',
           visual_dna_json TEXT NOT NULL DEFAULT '{}',
           created_at INTEGER NOT NULL,
           FOREIGN KEY(asset_id) REFERENCES assets(id) ON DELETE CASCADE
         );
         CREATE INDEX IF NOT EXISTS idx_image_prompt_asset ON image_prompt_analyses(asset_id,created_at DESC,id DESC);"
    ).map_err(|e|e.to_string())
}

fn migrate_v10(conn:&Connection)->Result<(),String>{
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS remix_drafts (
           id INTEGER PRIMARY KEY AUTOINCREMENT,
           base_asset_id INTEGER NOT NULL,
           prompt TEXT NOT NULL DEFAULT '',
           created_at INTEGER NOT NULL,
           updated_at INTEGER NOT NULL,
           FOREIGN KEY(base_asset_id) REFERENCES assets(id) ON DELETE CASCADE
         );
         CREATE INDEX IF NOT EXISTS idx_remix_base ON remix_drafts(base_asset_id,updated_at DESC,id DESC);
         CREATE TABLE IF NOT EXISTS remix_sources (
           draft_id INTEGER NOT NULL,
           asset_id INTEGER NOT NULL,
           fields_json TEXT NOT NULL DEFAULT '[]',
           source_url TEXT NOT NULL DEFAULT '',
           reference_meta_json TEXT NOT NULL DEFAULT '{}',
           position INTEGER NOT NULL DEFAULT 0,
           created_at INTEGER NOT NULL,
           PRIMARY KEY(draft_id,asset_id),
           FOREIGN KEY(draft_id) REFERENCES remix_drafts(id) ON DELETE CASCADE,
           FOREIGN KEY(asset_id) REFERENCES assets(id) ON DELETE CASCADE
         );
         CREATE INDEX IF NOT EXISTS idx_remix_sources_asset ON remix_sources(asset_id,draft_id);"
    ).map_err(|e|e.to_string())
}


fn migrate_v11(conn:&Connection)->Result<(),String>{
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS reference_sources (
           id INTEGER PRIMARY KEY AUTOINCREMENT,
           asset_id INTEGER NOT NULL,
           source_url TEXT NOT NULL DEFAULT '',
           page_url TEXT NOT NULL DEFAULT '',
           page_title TEXT NOT NULL DEFAULT '',
           source_type TEXT NOT NULL DEFAULT 'web',
           metadata_json TEXT NOT NULL DEFAULT '{}',
           captured_at INTEGER NOT NULL DEFAULT 0,
           created_at INTEGER NOT NULL,
           updated_at INTEGER NOT NULL,
           UNIQUE(asset_id,source_url,page_url),
           FOREIGN KEY(asset_id) REFERENCES assets(id) ON DELETE CASCADE
         );
         CREATE INDEX IF NOT EXISTS idx_reference_asset ON reference_sources(asset_id,captured_at DESC,id DESC);
         DROP TABLE IF EXISTS asset_search;
         CREATE VIRTUAL TABLE asset_search USING fts5(
           asset_id UNINDEXED,name,prompt,negative_prompt,model,tags,visual_dna,reference,
           tokenize='unicode61 remove_diacritics 2'
         );"
    ).map_err(|e|e.to_string())?;
    let can_reindex=
        has_column(conn,"assets","name")? &&
        has_table(conn,"prompt_state")? &&
        has_table(conn,"tags")? &&
        has_table(conn,"asset_tags")?;
    if can_reindex{
        let ids:Vec<i64>={
            let mut st=conn.prepare("SELECT id FROM assets ORDER BY id").map_err(|e|e.to_string())?;
            let rows=st.query_map([],|r|r.get(0)).map_err(|e|e.to_string())?;
            let collected=rows.filter_map(Result::ok).collect();
            collected
        };
        for id in ids{crate::db::reindex_asset(conn,id)?;}
    }
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

    if version<4{
        migrate_v4(conn)?;
        version=4;
        set_version(conn,version)?;
    }

    if version<5{
        migrate_v5(conn)?;
        version=5;
        set_version(conn,version)?;
    }

    if version<6{
        migrate_v6(conn)?;
        version=6;
        set_version(conn,version)?;
    }

    if version<7{
        migrate_v7(conn)?;
        version=7;
        set_version(conn,version)?;
    }

    if version<8{
        migrate_v8(conn)?;
        version=8;
        set_version(conn,version)?;
    }

    if version<9{
        migrate_v9(conn)?;
        version=9;
        set_version(conn,version)?;
    }

    if version<10{
        migrate_v10(conn)?;
        version=10;
        set_version(conn,version)?;
    }
    if version<11{
        migrate_v11(conn)?;
        version=11;
        set_version(conn,version)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests{

    #[test]
    fn migration_v11_adds_reference_sources(){
        let conn=Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE app_meta(key TEXT PRIMARY KEY,value TEXT NOT NULL);
             INSERT INTO app_meta VALUES('schema_version','10');
             CREATE TABLE assets(id INTEGER PRIMARY KEY);
             INSERT INTO assets(id) VALUES(1);"
        ).unwrap();
        apply(&conn).unwrap();
        let version:String=conn.query_row("SELECT value FROM app_meta WHERE key='schema_version'",[],|r|r.get(0)).unwrap();
        assert_eq!(version,"11");
        conn.execute(
            "INSERT INTO reference_sources(asset_id,source_url,page_url,page_title,source_type,metadata_json,captured_at,created_at,updated_at)
             VALUES(1,'https://cdn.example/a.png','https://example.test/post','Inspiration','browser-extension','{}',1,1,1)",[]
        ).unwrap();
        let title:String=conn.query_row("SELECT page_title FROM reference_sources WHERE asset_id=1",[],|r|r.get(0)).unwrap();
        assert_eq!(title,"Inspiration");
    }

    #[test]
    fn migration_v10_adds_remix_tables(){
        let conn=Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE app_meta(key TEXT PRIMARY KEY,value TEXT NOT NULL);
             INSERT INTO app_meta VALUES('schema_version','9');
             CREATE TABLE assets(id INTEGER PRIMARY KEY);
             INSERT INTO assets(id) VALUES(1);"
        ).unwrap();
        apply(&conn).unwrap();
        let version:String=conn.query_row("SELECT value FROM app_meta WHERE key='schema_version'",[],|r|r.get(0)).unwrap();
        assert_eq!(version,"11");
        conn.execute("INSERT INTO remix_drafts(base_asset_id,prompt,created_at,updated_at) VALUES(1,'p',1,1)",[]).unwrap();
        let draft=conn.last_insert_rowid();
        conn.execute("INSERT INTO remix_sources(draft_id,asset_id,fields_json,created_at) VALUES(?1,1,'[\"subject\"]',1)",params![draft]).unwrap();
        assert_eq!(conn.query_row("SELECT COUNT(*) FROM remix_sources",[],|r|r.get::<_,i64>(0)).unwrap(),1);
    }


    #[test]
    fn migration_v9_adds_image_to_prompt_tables(){
        let conn=Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE app_meta(key TEXT PRIMARY KEY,value TEXT NOT NULL);
             INSERT INTO app_meta VALUES('schema_version','8');
             CREATE TABLE assets(id INTEGER PRIMARY KEY);"
        ).unwrap();
        apply(&conn).unwrap();
        let version:String=conn.query_row("SELECT value FROM app_meta WHERE key='schema_version'",[],|r|r.get(0)).unwrap();
        assert_eq!(version,"11");
        let base:String=conn.query_row("SELECT base_url FROM vision_settings WHERE id=1",[],|r|r.get(0)).unwrap();
        assert_eq!(base,"https://api.openai.com/v1");
        conn.execute("INSERT INTO assets(id) VALUES(1)",[]).unwrap();
        conn.execute(
            "INSERT INTO image_prompt_analyses(asset_id,model,prompt,created_at) VALUES(1,'vision-model','prompt',1)",[]
        ).unwrap();
        let count:i64=conn.query_row("SELECT COUNT(*) FROM image_prompt_analyses",[],|r|r.get(0)).unwrap();
        assert_eq!(count,1);
    }


    use super::*;

    #[test]
    fn migration_v7_adds_cjk_trigram_index(){
        let conn=Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE app_meta(key TEXT PRIMARY KEY,value TEXT NOT NULL);
             INSERT INTO app_meta VALUES('schema_version','6');
             CREATE TABLE assets(id INTEGER PRIMARY KEY,name TEXT NOT NULL);
             CREATE TABLE prompt_state(
               asset_id INTEGER PRIMARY KEY,
               prompt TEXT NOT NULL DEFAULT '',
               negative_prompt TEXT NOT NULL DEFAULT '',
               model TEXT NOT NULL DEFAULT ''
             );
             CREATE TABLE tags(id INTEGER PRIMARY KEY,name TEXT NOT NULL);
             CREATE TABLE asset_tags(asset_id INTEGER NOT NULL,tag_id INTEGER NOT NULL);
             INSERT INTO assets(id,name) VALUES(1,'蓝色大肥鱼.png');
             INSERT INTO prompt_state(asset_id,prompt,negative_prompt,model)
             VALUES(1,'夜晚卧室中的蓝发角色','','GPT Image');
             INSERT INTO tags(id,name) VALUES(1,'海洋少女');
             INSERT INTO asset_tags(asset_id,tag_id) VALUES(1,1);"
        ).unwrap();
        apply(&conn).unwrap();
        let version:String=conn.query_row("SELECT value FROM app_meta WHERE key='schema_version'",[],|r|r.get(0)).unwrap();
        assert_eq!(version,"11");
        let hit:i64=conn.query_row(
            "SELECT asset_id FROM asset_cjk_search WHERE asset_cjk_search MATCH ?1",
            params!["\"蓝色大肥鱼\""],
            |r|r.get(0)
        ).unwrap();
        assert_eq!(hit,1);
    }

    #[test]
    fn migration_v6_adds_semantic_tables(){
        let conn=Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE app_meta(key TEXT PRIMARY KEY,value TEXT NOT NULL);
             INSERT INTO app_meta VALUES('schema_version','5');
             CREATE TABLE assets(id INTEGER PRIMARY KEY);"
        ).unwrap();
        apply(&conn).unwrap();
        let version:String=conn.query_row("SELECT value FROM app_meta WHERE key='schema_version'",[],|r|r.get(0)).unwrap();
        assert_eq!(version,"11");
        let enabled:String=conn.query_row("SELECT value FROM semantic_settings WHERE key='enabled'",[],|r|r.get(0)).unwrap();
        assert_eq!(enabled,"0");
        conn.execute("INSERT INTO assets(id) VALUES(1)",[]).unwrap();
        conn.execute(
            "INSERT INTO semantic_embeddings(asset_id,model_id,dimensions,vector,fingerprint,indexed_at) VALUES(1,'clip',1,X'00000000','abc',1)",[]
        ).unwrap();
    }

    #[test]
    fn migration_v5_backfills_generation_index(){
        let conn=Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE app_meta(key TEXT PRIMARY KEY,value TEXT NOT NULL);
             INSERT INTO app_meta VALUES('schema_version','4');
             CREATE TABLE assets(
               id INTEGER PRIMARY KEY,path TEXT NOT NULL,generation_json TEXT NOT NULL DEFAULT '{}'
             );
             INSERT INTO assets(id,path,generation_json) VALUES(1,'x.png','{\"seed\":\"42\",\"steps\":\"30\",\"sampler\":\"Euler\",\"cfg_scale\":\"6.5\"}');"
        ).unwrap();
        apply(&conn).unwrap();
        let version:String=conn.query_row("SELECT value FROM app_meta WHERE key='schema_version'",[],|r|r.get(0)).unwrap();
        assert_eq!(version,"11");
        let row:(String,i64,String,f64)=conn.query_row(
            "SELECT seed,steps,sampler,cfg_scale FROM generation_index WHERE asset_id=1",[],
            |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))
        ).unwrap();
        assert_eq!(row,("42".into(),30,"Euler".into(),6.5));
    }

    #[test]
    fn migration_v4_adds_source_folders(){
        let conn=Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE app_meta(key TEXT PRIMARY KEY,value TEXT NOT NULL);
             INSERT INTO app_meta VALUES('schema_version','3');
             CREATE TABLE assets(
               id INTEGER PRIMARY KEY,path TEXT NOT NULL,generation_json TEXT NOT NULL DEFAULT '{}'
             );"
        ).unwrap();
        apply(&conn).unwrap();
        let version:String=conn.query_row("SELECT value FROM app_meta WHERE key='schema_version'",[],|r|r.get(0)).unwrap();
        assert_eq!(version,"11");
        conn.execute(
            "INSERT INTO source_folders(path,name,created_at,updated_at) VALUES('D:/AI','AI',1,1)",[]
        ).unwrap();
        let enabled:i64=conn.query_row("SELECT auto_sync FROM source_folders LIMIT 1",[],|r|r.get(0)).unwrap();
        assert_eq!(enabled,1);
    }

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
    #[test]
    fn migration_v8_adds_visual_dna_and_search_column(){
        let conn=Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE app_meta(key TEXT PRIMARY KEY,value TEXT NOT NULL);
             INSERT INTO app_meta VALUES('schema_version','7');
             CREATE TABLE assets(id INTEGER PRIMARY KEY,name TEXT NOT NULL);
             CREATE TABLE prompt_state(asset_id INTEGER PRIMARY KEY,prompt TEXT NOT NULL DEFAULT '',negative_prompt TEXT NOT NULL DEFAULT '',model TEXT NOT NULL DEFAULT '');
             CREATE TABLE tags(id INTEGER PRIMARY KEY,name TEXT NOT NULL);
             CREATE TABLE asset_tags(asset_id INTEGER NOT NULL,tag_id INTEGER NOT NULL);
             CREATE VIRTUAL TABLE asset_search USING fts5(asset_id UNINDEXED,name,prompt,negative_prompt,model,tags);
             CREATE VIRTUAL TABLE asset_cjk_search USING fts5(asset_id UNINDEXED,text,tokenize='trigram');
             INSERT INTO assets(id,name) VALUES(1,'reference.png');
             INSERT INTO prompt_state(asset_id,prompt,negative_prompt,model) VALUES(1,'portrait','','GPT Image');"
        ).unwrap();
        apply(&conn).unwrap();
        let version:String=conn.query_row("SELECT value FROM app_meta WHERE key='schema_version'",[],|r|r.get(0)).unwrap();
        assert_eq!(version,"11");
        conn.execute(
            "INSERT INTO visual_dna(asset_id,environment,style,search_text,source,updated_at) VALUES(1,'千禧年电脑房','日系写实','千禧年电脑房 日系写实','manual',1)",[]
        ).unwrap();
        let columns:i64=conn.query_row("SELECT COUNT(*) FROM pragma_table_info('visual_dna')",[],|r|r.get(0)).unwrap();
        assert!(columns>=16);
    }

    #[test]
    fn migration_v11_adds_reference_sources_and_search(){
        let conn=Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("../schema.sql")).unwrap();
        assert_eq!(conn.query_row("SELECT value FROM app_meta WHERE key='schema_version'",[],|r|r.get::<_,String>(0)).unwrap(),"11");
        conn.execute("INSERT INTO assets(id,path,name,created_at,updated_at) VALUES(1,'a.png','a.png',1,1)",[]).unwrap();
        conn.execute("INSERT INTO prompt_state(asset_id,updated_at) VALUES(1,1)",[]).unwrap();
        conn.execute("INSERT INTO reference_sources(asset_id,source_url,page_url,page_title,source_type,metadata_json,captured_at,created_at,updated_at) VALUES(1,'https://cdn.example/a.png','https://example/post','灵感页面','browser-extension','{}',1,1,1)",[]).unwrap();
        crate::db::reindex_asset(&conn,1).unwrap();
        assert_eq!(conn.query_row("SELECT COUNT(*) FROM asset_search WHERE asset_search MATCH '灵感'",[],|r|r.get::<_,i64>(0)).unwrap(),1);
    }

}
