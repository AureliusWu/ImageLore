use rusqlite::{params,Connection};
use serde_json::Value;

fn text(value:&Value,key:&str)->String{
    value.get(key).and_then(|v|{
        if let Some(s)=v.as_str(){Some(s.trim().to_string())}
        else if v.is_number()||v.is_boolean(){Some(v.to_string())}
        else{None}
    }).unwrap_or_default()
}

fn int_value(value:&Value,key:&str)->Option<i64>{
    let v=value.get(key)?;
    v.as_i64().or_else(||v.as_u64().and_then(|x|i64::try_from(x).ok()))
        .or_else(||v.as_f64().map(|x|x.round() as i64))
        .or_else(||v.as_str().and_then(|x|x.trim().parse::<f64>().ok()).map(|x|x.round() as i64))
}

fn real_value(value:&Value,key:&str)->Option<f64>{
    let v=value.get(key)?;
    v.as_f64().or_else(||v.as_i64().map(|x|x as f64))
        .or_else(||v.as_u64().map(|x|x as f64))
        .or_else(||v.as_str().and_then(|x|x.trim().parse::<f64>().ok()))
}

pub fn upsert(conn:&Connection,asset_id:i64,generation_json:&str)->Result<(),String>{
    let value=serde_json::from_str::<Value>(generation_json).unwrap_or(Value::Null);
    let seed=text(&value,"seed");
    let sampler=text(&value,"sampler");
    let scheduler=text(&value,"scheduler");
    let steps=int_value(&value,"steps");
    let cfg_scale=real_value(&value,"cfg_scale");
    let denoise=real_value(&value,"denoise");
    conn.execute(
        "INSERT INTO generation_index(asset_id,seed,steps,sampler,scheduler,cfg_scale,denoise)
         VALUES(?1,?2,?3,?4,?5,?6,?7)
         ON CONFLICT(asset_id) DO UPDATE SET
           seed=excluded.seed,steps=excluded.steps,sampler=excluded.sampler,
           scheduler=excluded.scheduler,cfg_scale=excluded.cfg_scale,denoise=excluded.denoise",
        params![asset_id,seed,steps,sampler,scheduler,cfg_scale,denoise]
    ).map_err(|e|e.to_string())?;
    Ok(())
}

pub fn rebuild_all(conn:&Connection)->Result<usize,String>{
    let rows:Vec<(i64,String)>={
        let mut st=conn.prepare("SELECT id,generation_json FROM assets ORDER BY id").map_err(|e|e.to_string())?;
        let mapped=st.query_map([],|r|Ok((r.get(0)?,r.get(1)?))).map_err(|e|e.to_string())?;
        let collected=mapped.filter_map(Result::ok).collect();
        collected
    };
    for(id,json)in &rows{upsert(conn,*id,json)?}
    Ok(rows.len())
}

#[cfg(test)]
mod tests{
    use super::*;

    #[test]
    fn normalizes_string_and_numeric_generation_fields(){
        let conn=Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE generation_index(
              asset_id INTEGER PRIMARY KEY,seed TEXT NOT NULL DEFAULT '',steps INTEGER,
              sampler TEXT NOT NULL DEFAULT '',scheduler TEXT NOT NULL DEFAULT '',
              cfg_scale REAL,denoise REAL
            );"
        ).unwrap();
        upsert(&conn,7,r#"{"seed":4120039,"steps":"28","sampler":"DPM++ 2M","scheduler":"karras","cfg_scale":"5.5","denoise":0.72}"#).unwrap();
        let row:(String,i64,String,String,f64,f64)=conn.query_row(
            "SELECT seed,steps,sampler,scheduler,cfg_scale,denoise FROM generation_index WHERE asset_id=7",[],
            |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))
        ).unwrap();
        assert_eq!(row.0,"4120039");
        assert_eq!(row.1,28);
        assert_eq!(row.2,"DPM++ 2M");
        assert_eq!(row.3,"karras");
        assert!((row.4-5.5).abs()<0.001);
        assert!((row.5-0.72).abs()<0.001);
    }
}
