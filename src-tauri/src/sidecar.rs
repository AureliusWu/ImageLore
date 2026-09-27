use serde_json::Value;
use std::{fs,path::{Path,PathBuf}};

#[derive(Debug,Clone)]
pub struct ParentRef{
    pub portable_id:String,
    pub fingerprint:String,
    pub relation_type:String,
    pub note:String,
}

#[derive(Debug,Clone)]
pub struct SessionRef{
    pub name:String,
    pub note:String,
}

pub fn path_for(path:&Path)->PathBuf{
    PathBuf::from(format!("{}.imagelore.json",path.to_string_lossy()))
}

pub fn read(path:&Path)->Option<Value>{
    let text=fs::read_to_string(path_for(path)).ok()?;
    let value=serde_json::from_str::<Value>(&text).ok()?;
    match value.get("schema").and_then(Value::as_str){
        Some("imagelore.sidecar.v2")|Some("imagelore.sidecar.v3")=>Some(value),
        _=>None,
    }
}

pub fn text(value:&Value,key:&str)->String{
    value.get(key).and_then(Value::as_str).unwrap_or("").to_string()
}

pub fn tags(value:&Value)->Vec<String>{
    value.get("tags").and_then(Value::as_array)
        .map(|items|items.iter().filter_map(Value::as_str).map(str::to_string).collect())
        .unwrap_or_default()
}

pub fn portable_id(value:&Value)->String{
    value.get("asset").and_then(|x|x.get("portable_id")).and_then(Value::as_str)
        .or_else(||value.get("portable_id").and_then(Value::as_str))
        .unwrap_or("").to_string()
}

pub fn parents(value:&Value)->Vec<ParentRef>{
    if value.get("schema").and_then(Value::as_str)!=Some("imagelore.sidecar.v3"){return Vec::new()}
    value.get("parents").and_then(Value::as_array).map(|items|{
        items.iter().filter_map(|item|{
            let portable_id=item.get("portable_id").and_then(Value::as_str).unwrap_or("").to_string();
            let fingerprint=item.get("fingerprint").and_then(Value::as_str).unwrap_or("").to_string();
            if portable_id.is_empty()&&fingerprint.is_empty(){return None}
            Some(ParentRef{
                portable_id,
                fingerprint,
                relation_type:item.get("relation_type").and_then(Value::as_str).unwrap_or("reference").to_string(),
                note:item.get("note").and_then(Value::as_str).unwrap_or("").to_string(),
            })
        }).collect()
    }).unwrap_or_default()
}

pub fn session(value:&Value)->Option<SessionRef>{
    let item=value.get("session")?;
    let name=item.get("name").and_then(Value::as_str)?.trim();
    if name.is_empty(){return None}
    Some(SessionRef{
        name:name.to_string(),
        note:item.get("note").and_then(Value::as_str).unwrap_or("").to_string(),
    })
}
