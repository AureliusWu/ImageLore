export type LibraryView = "all" | "favorites" | "recent" | "missing";
export type RelationType = "derived_from" | "variation" | "edit" | "upscale" | "reference";
export type SearchMode = "keyword" | "semantic";

export interface LibraryFilter {
  query:string;view:LibraryView;tag?:string|null;model?:string|null;collection_id?:number|null;
  metadata_type?:string|null;sampler?:string|null;scheduler?:string|null;seed?:string|null;
  steps_min?:number|null;steps_max?:number|null;cfg_min?:number|null;cfg_max?:number|null;
  denoise_min?:number|null;denoise_max?:number|null;orientation?:string|null;sort?:string;
}
export interface AssetSummary {
  id:number;path:string;name:string;favorite:number;width:number|null;height:number|null;format:string;
  metadata_type:string;fingerprint:string;file_mtime:number;missing:number;updated_at:number;
}
export interface AssetRecord extends AssetSummary {
  prompt:string;negative_prompt:string;model:string;tags:string[];file_size:number|null;mime_type:string;
  generation_json:string;portable_id:string;created_at:number;
}
export interface LibraryPage { items:AssetSummary[];total:number;offset:number;limit:number; }
export interface FacetCount { name:string;count:number; }
export interface CollectionRecord { id:number;name:string;description:string;created_at:number;updated_at:number;count:number; }
export interface LibraryFacets {
  tags:FacetCount[];models:FacetCount[];collections:CollectionRecord[];
  metadata_types:FacetCount[];samplers:FacetCount[];schedulers:FacetCount[];
}
export interface ImportSummary { added:number;skipped:number;duplicates:number;failed:number;last_id:number|null; }
export interface ImportProgress extends ImportSummary {
  job_id:number;processed:number;total:number;current_name:string;done:boolean;cancelled:boolean;
}
export interface BackupRecord { name:string;path:string;size:number;created_at:number; }
export interface DuplicateGroup { fingerprint:string;count:number;asset_ids:number[];names:string[]; }
export interface PromptPatch { prompt:string;negative_prompt:string;model:string; }
export interface Revision { id:number;asset_id:number;prompt:string;negative_prompt:string;model:string;tags:string[];note:string;created_at:number; }
export interface RelationRecord { id:number;parent_id:number;child_id:number;relation_type:string;note:string;created_at:number;other_id:number;other_name:string; }
export interface Lineage { parents:RelationRecord[];children:RelationRecord[]; }
export interface GenerationInfo { seed?:string;steps?:string;sampler?:string;cfg_scale?:string;size?:string;model?:string;scheduler?:string;denoise?:string;[key:string]:unknown; }

export interface GenerationSession { id:number;name:string;note:string;count:number;created_at:number;updated_at:number; }
export interface AssetSession { session_id:number;session_name:string;session_note:string;asset_note:string; }
export interface ModelAlias { alias:string;canonical:string; }
export interface SavedFilter { id:number;name:string;filter:LibraryFilter;created_at:number;updated_at:number; }
export interface LibraryHealth {
  total:number;missing:number;duplicate_groups:number;without_metadata:number;without_fingerprint:number;
  pending_relations:number;unassigned_session:number;cache_bytes:number;
}

export interface SourceFolder {
  id:number;path:string;name:string;auto_sync:boolean;last_scan_at:number;created_at:number;updated_at:number;
}


export interface SemanticStatus {
  model_id:string;enabled:boolean;model_ready:boolean;vision_ready:boolean;text_ready:boolean;
  indexed:number;total:number;stale:number;model_bytes:number;index_bytes:number;
}
export interface SemanticHit { asset:AssetSummary;score:number; }
export interface SemanticProgress {
  job_id:number;processed:number;total:number;indexed:number;skipped:number;failed:number;
  current_name:string;done:boolean;cancelled:boolean;
}
