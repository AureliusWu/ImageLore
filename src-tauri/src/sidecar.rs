use crate::{models::VisualDnaPatch,references::ReferenceInput};
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
    pub session_note:String,
    pub asset_note:String,
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
        session_note:item.get("session_note").and_then(Value::as_str)
            .or_else(||item.get("note").and_then(Value::as_str)).unwrap_or("").to_string(),
        asset_note:item.get("asset_note").and_then(Value::as_str).unwrap_or("").to_string(),
    })
}

pub fn references(value:&Value)->Vec<ReferenceInput>{
    let mut items=Vec::<Value>::new();
    if let Some(array)=value.get("references").and_then(Value::as_array){items.extend(array.iter().cloned());}
    if let Some(single)=value.get("reference").filter(|x|x.is_object()){items.push(single.clone());}
    items.into_iter().filter_map(|item|{
        let text=|key:&str|item.get(key).and_then(Value::as_str).unwrap_or("").to_string();
        let source_url=text("source_url");
        let page_url=text("page_url");
        if source_url.trim().is_empty()&&page_url.trim().is_empty(){return None}
        let metadata_json=item.get("metadata").or_else(||item.get("reference_meta"))
            .map(|x|serde_json::to_string(x).unwrap_or_else(|_|"{}".into())).unwrap_or_else(||"{}".into());
        Some(ReferenceInput{
            source_url,page_url,page_title:text("page_title"),source_type:text("source_type"),
            metadata_json,captured_at:item.get("captured_at").and_then(Value::as_i64).unwrap_or(0),
        })
    }).collect()
}

pub fn visual_dna(value:&Value)->Option<VisualDnaPatch>{
    let item=value.get("visual_dna")?;
    let field=|key:&str|item.get(key).and_then(Value::as_str).unwrap_or("").to_string();
    let patch=VisualDnaPatch{
        subject:field("subject"),
        character:field("character"),
        outfit:field("outfit"),
        pose:field("pose"),
        expression:field("expression"),
        composition:field("composition"),
        camera:field("camera"),
        lighting:field("lighting"),
        environment:field("environment"),
        palette:field("palette"),
        material:field("material"),
        style:field("style"),
        source:"sidecar".into(),
    };
    if crate::visual_dna::patch_is_empty(&patch){None}else{Some(patch)}
}

#[cfg(test)]
mod tests{
    use super::*;

    #[test]
    fn reads_web_reference_from_v3_sidecar(){
        let value:Value=serde_json::from_str(r#"{
          "schema":"imagelore.sidecar.v3",
          "reference":{"source_url":"https://cdn.example/a.png","page_url":"https://example/post","page_title":"Inspiration","source_type":"browser-extension","captured_at":42,"metadata":{"host":"example"}}
        }"#).unwrap();
        let refs=references(&value);
        assert_eq!(refs.len(),1);
        assert_eq!(refs[0].page_title,"Inspiration");
        assert!(refs[0].metadata_json.contains("example"));
    }

    #[test]
    fn reads_optional_visual_dna_from_v3_sidecar(){
        let value:Value=serde_json::from_str(r#"{
          "schema":"imagelore.sidecar.v3",
          "visual_dna":{"subject":"成年女性","lighting":"窗边柔光","style":"写实摄影"}
        }"#).unwrap();
        let dna=visual_dna(&value).unwrap();
        assert_eq!(dna.subject,"成年女性");
        assert_eq!(dna.lighting,"窗边柔光");
        assert_eq!(dna.source,"sidecar");
    }
}
