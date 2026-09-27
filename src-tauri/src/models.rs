use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetSummary {
    pub id: i64,
    pub path: String,
    pub name: String,
    pub favorite: i64,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub format: String,
    pub metadata_type: String,
    pub fingerprint: String,
    pub file_mtime: i64,
    pub missing: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetRecord {
    pub id: i64,
    pub path: String,
    pub name: String,
    pub prompt: String,
    pub negative_prompt: String,
    pub model: String,
    pub tags: Vec<String>,
    pub favorite: i64,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub file_size: Option<i64>,
    pub format: String,
    pub mime_type: String,
    pub metadata_type: String,
    pub generation_json: String,
    pub fingerprint: String,
    pub portable_id: String,
    pub file_mtime: i64,
    pub missing: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LibraryFilter {
    #[serde(default)]
    pub query: String,
    #[serde(default = "default_view")]
    pub view: String,
    #[serde(default)]
    pub tag: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub collection_id: Option<i64>,
}

fn default_view() -> String { "all".to_string() }

#[derive(Debug, Clone, Serialize)]
pub struct LibraryPage {
    pub items: Vec<AssetSummary>,
    pub total: i64,
    pub offset: i64,
    pub limit: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct FacetCount {
    pub name: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct CollectionRecord {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct LibraryFacets {
    pub tags: Vec<FacetCount>,
    pub models: Vec<FacetCount>,
    pub collections: Vec<CollectionRecord>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PromptPatch {
    pub prompt: String,
    pub negative_prompt: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Revision {
    pub id: i64,
    pub asset_id: i64,
    pub prompt: String,
    pub negative_prompt: String,
    pub model: String,
    pub tags: Vec<String>,
    pub note: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RelationRecord {
    pub id: i64,
    pub parent_id: i64,
    pub child_id: i64,
    pub relation_type: String,
    pub note: String,
    pub created_at: i64,
    pub other_id: i64,
    pub other_name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Lineage {
    pub parents: Vec<RelationRecord>,
    pub children: Vec<RelationRecord>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportSummary {
    pub added: i64,
    pub skipped: i64,
    pub duplicates: i64,
    pub failed: i64,
    pub last_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BackupRecord {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DuplicateGroup {
    pub fingerprint: String,
    pub count: i64,
    pub asset_ids: Vec<i64>,
    pub names: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportProgress {
    pub job_id: u64,
    pub processed: usize,
    pub total: usize,
    pub added: i64,
    pub skipped: i64,
    pub duplicates: i64,
    pub failed: i64,
    pub current_name: String,
    pub done: bool,
    pub cancelled: bool,
    pub last_id: Option<i64>,
}


#[derive(Debug,Clone,Serialize)]
pub struct GenerationSession {
    pub id:i64,
    pub name:String,
    pub note:String,
    pub count:i64,
    pub created_at:i64,
    pub updated_at:i64,
}

#[derive(Debug,Clone,Serialize)]
pub struct AssetSession {
    pub session_id:i64,
    pub session_name:String,
    pub session_note:String,
    pub asset_note:String,
}

#[derive(Debug,Clone,Serialize)]
pub struct ModelAlias {
    pub alias:String,
    pub canonical:String,
}

#[derive(Debug,Clone,Serialize)]
pub struct SavedFilter {
    pub id:i64,
    pub name:String,
    pub filter:LibraryFilter,
    pub created_at:i64,
    pub updated_at:i64,
}

#[derive(Debug,Clone,Serialize)]
pub struct LibraryHealth {
    pub total:i64,
    pub missing:i64,
    pub duplicate_groups:i64,
    pub without_metadata:i64,
    pub without_fingerprint:i64,
    pub pending_relations:i64,
    pub unassigned_session:i64,
    pub cache_bytes:u64,
}
