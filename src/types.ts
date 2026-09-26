export type LibraryView = "all" | "favorites" | "recent" | "missing";
export type RelationType = "derived_from" | "variation" | "edit" | "upscale" | "reference";

export interface LibraryFilter {
  query: string;
  view: LibraryView;
  tag?: string | null;
  model?: string | null;
  collection_id?: number | null;
}

export interface AssetSummary {
  id: number;
  path: string;
  name: string;
  favorite: number;
  width: number | null;
  height: number | null;
  format: string;
  metadata_type: string;
  fingerprint: string;
  file_mtime: number;
  missing: number;
  updated_at: number;
}

export interface AssetRecord extends AssetSummary {
  prompt: string;
  negative_prompt: string;
  model: string;
  tags: string[];
  file_size: number | null;
  mime_type: string;
  generation_json: string;
  created_at: number;
}

export interface LibraryPage { items: AssetSummary[]; total: number; offset: number; limit: number; }
export interface FacetCount { name: string; count: number; }
export interface CollectionRecord { id:number; name:string; description:string; created_at:number; updated_at:number; count:number; }
export interface LibraryFacets { tags:FacetCount[]; models:FacetCount[]; collections:CollectionRecord[]; }
export interface ImportSummary { added:number; skipped:number; failed:number; last_id:number|null; }
export interface PromptPatch { prompt:string; negative_prompt:string; model:string; }
export interface Revision { id:number; asset_id:number; prompt:string; negative_prompt:string; model:string; tags:string[]; note:string; created_at:number; }
export interface RelationRecord { id:number; parent_id:number; child_id:number; relation_type:string; note:string; created_at:number; other_id:number; other_name:string; }
export interface Lineage { parents:RelationRecord[]; children:RelationRecord[]; }
export interface GenerationInfo { seed?:string; steps?:string; sampler?:string; cfg_scale?:string; size?:string; model?:string; [key:string]:unknown; }
