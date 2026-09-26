use serde_json::Value;
use std::{fs,path::{Path,PathBuf}};

pub fn path_for(path:&Path)->PathBuf{
    PathBuf::from(format!("{}.imagelore.json",path.to_string_lossy()))
}

pub fn read(path:&Path)->Option<Value>{
    let text=fs::read_to_string(path_for(path)).ok()?;
    let value=serde_json::from_str::<Value>(&text).ok()?;
    (value.get("schema").and_then(Value::as_str)==Some("imagelore.sidecar.v2")).then_some(value)
}

pub fn text(value:&Value,key:&str)->String{
    value.get(key).and_then(Value::as_str).unwrap_or("").to_string()
}

pub fn tags(value:&Value)->Vec<String>{
    value.get("tags").and_then(Value::as_array)
        .map(|items|items.iter().filter_map(Value::as_str).map(str::to_string).collect())
        .unwrap_or_default()
}
