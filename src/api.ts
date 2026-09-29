import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import type { AssetRecord, AssetSession, AssetSummary, BackupRecord, CollectionRecord, DiagnosticStatus, DuplicateGroup, GenerationSession, ImportSummary, LibraryFacets, LibraryFilter, LibraryHealth, LibraryPage, Lineage, ModelAlias, PromptPatch, Revision, SavedFilter, SemanticHit, SemanticStatus, SourceFolder, VisualDna, VisualDnaPatch } from "./types";

export const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

const demoSvg = (title:string, a:string, b:string) => `data:image/svg+xml;charset=utf-8,${encodeURIComponent(`<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="900"><defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop stop-color="${a}"/><stop offset="1" stop-color="${b}"/></linearGradient></defs><rect width="100%" height="100%" fill="url(#g)"/><circle cx="880" cy="180" r="180" fill="rgba(255,255,255,.22)"/><circle cx="260" cy="690" r="260" fill="rgba(255,255,255,.12)"/><text x="70" y="110" font-family="Segoe UI" font-size="54" font-weight="700" fill="white">${title}</text><text x="72" y="165" font-family="Segoe UI" font-size="24" fill="rgba(255,255,255,.78)">ImageLore Generation Record</text></svg>`)}`;

let mockAssets:AssetRecord[] = [
  {id:1,path:"D:/AI/character-origin.png",name:"Character Origin.png",prompt:"adult blue-haired marine fantasy character, calm expression, clean character design, pearl ornaments",negative_prompt:"low quality, extra fingers",model:"GPT Image",tags:["character","blue hair","origin"],favorite:1,width:1536,height:2048,file_size:2840000,format:"PNG",mime_type:"image/png",metadata_type:"manual",generation_json:'{"size":"1536x2048"}',fingerprint:"demo-01",portable_id:"il-demo-01",file_mtime:0,missing:0,created_at:1760000000,updated_at:1760000500},
  {id:2,path:"D:/AI/bedroom-variation.png",name:"Bedroom Variation.png",prompt:"adult blue-haired marine fantasy character, sitting beside a bed at night, warm bedside lamp, quiet mood",negative_prompt:"low quality",model:"GPT Image",tags:["character","bedroom","variation"],favorite:0,width:1536,height:2048,file_size:2310000,format:"PNG",mime_type:"image/png",metadata_type:"manual",generation_json:'{"size":"1536x2048"}',fingerprint:"demo-02",portable_id:"il-demo-02",file_mtime:0,missing:0,created_at:1760000100,updated_at:1760000600},
  {id:3,path:"D:/AI/photo-study.png",name:"Photoreal Study.png",prompt:"photorealistic adult East Asian woman, blue hair, marine inspired jewelry, editorial portrait",negative_prompt:"plastic skin",model:"Flux",tags:["photoreal","study"],favorite:0,width:1024,height:1536,file_size:1900000,format:"PNG",mime_type:"image/png",metadata_type:"a1111",generation_json:'{"steps":"28","sampler":"DPM++ 2M","cfg_scale":"5","seed":"4120039"}',fingerprint:"demo-03",portable_id:"il-demo-03",file_mtime:0,missing:0,created_at:1760000200,updated_at:1760000400},
  {id:4,path:"D:/AI/poster.png",name:"Aero Poster.png",prompt:"frutiger aero inspired poster, blue sky, clear water, lush green hills, glossy bubbles",negative_prompt:"dark UI, gray background",model:"Flux",tags:["poster","aero"],favorite:1,width:1400,height:1800,file_size:2100000,format:"PNG",mime_type:"image/png",metadata_type:"manual",generation_json:'{}',fingerprint:"demo-04",portable_id:"il-demo-04",file_mtime:0,missing:0,created_at:1760000300,updated_at:1760000700},
  {id:5,path:"D:/AI/closeup.png",name:"Close-up Test.png",prompt:"close-up character portrait, clean light, glassy blue eyes",negative_prompt:"blur",model:"GPT Image",tags:["portrait","test"],favorite:0,width:1200,height:1200,file_size:1200000,format:"PNG",mime_type:"image/png",metadata_type:"manual",generation_json:'{}',fingerprint:"demo-05",portable_id:"il-demo-05",file_mtime:0,missing:0,created_at:1760000400,updated_at:1760000300},
  {id:6,path:"D:/AI/missing.png",name:"Moved Reference.png",prompt:"reference image",negative_prompt:"",model:"",tags:["reference"],favorite:0,width:1024,height:1024,file_size:900000,format:"PNG",mime_type:"image/png",metadata_type:"manual",generation_json:'{}',fingerprint:"demo-06",portable_id:"il-demo-06",file_mtime:0,missing:1,created_at:1760000500,updated_at:1760000200}
];
const demoImages = new Map<number,string>([
  [1,demoSvg("Origin","#5ac8eb","#78be70")],[2,demoSvg("Variation","#4fbbe4","#96ca71")],[3,demoSvg("Photo Study","#74b6d8","#5e8ea6")],
  [4,demoSvg("Aero Poster","#45c8e8","#72c359")],[5,demoSvg("Close-up","#71d0e3","#6a9fc4")],[6,demoSvg("Missing","#b7c9d0","#9aadb4")]
]);
let mockCollections:CollectionRecord[]=[{id:1,name:"Character Study",description:"Main character experiments",created_at:0,updated_at:0,count:3}];
let mockSources:SourceFolder[]=[{id:1,path:"D:/AI/outputs",name:"outputs",auto_sync:true,last_scan_at:0,created_at:0,updated_at:0}];

const toSummary=(a:AssetRecord):AssetSummary=>({id:a.id,path:a.path,name:a.name,favorite:a.favorite,width:a.width,height:a.height,format:a.format,metadata_type:a.metadata_type,fingerprint:a.fingerprint,file_mtime:a.file_mtime,missing:a.missing,updated_at:a.updated_at});

const generationOf=(a:AssetRecord)=>{try{return JSON.parse(a.generation_json||"{}") as Record<string,unknown>}catch{return{}}};
const numeric=(value:unknown)=>{const n=Number(value);return Number.isFinite(n)?n:null};

function filtered(filter:LibraryFilter){
  const q=filter.query.trim().toLowerCase();
  const rows=mockAssets.filter(a=>{
    const g=generationOf(a),steps=numeric(g.steps),cfg=numeric(g.cfg_scale),denoise=numeric(g.denoise);
    const orientation=a.width&&a.height?(a.width===a.height?"square":a.width>a.height?"landscape":"portrait"):null;
    return (!q||[a.name,a.prompt,a.negative_prompt,a.model,...a.tags].join(" ").toLowerCase().includes(q))
      &&(filter.view!=="favorites"||!!a.favorite)&&(filter.view!=="missing"||!!a.missing)
      &&(!filter.tag||a.tags.includes(filter.tag))&&(!filter.model||a.model===filter.model)
      &&(!filter.metadata_type||a.metadata_type===filter.metadata_type)
      &&(!filter.sampler||String(g.sampler||"").toLowerCase()===filter.sampler.toLowerCase())
      &&(!filter.scheduler||String(g.scheduler||"").toLowerCase()===filter.scheduler.toLowerCase())
      &&(!filter.seed||String(g.seed||"")===filter.seed)
      &&(filter.steps_min==null||steps!=null&&steps>=filter.steps_min)
      &&(filter.steps_max==null||steps!=null&&steps<=filter.steps_max)
      &&(filter.cfg_min==null||cfg!=null&&cfg>=filter.cfg_min)
      &&(filter.cfg_max==null||cfg!=null&&cfg<=filter.cfg_max)
      &&(filter.denoise_min==null||denoise!=null&&denoise>=filter.denoise_min)
      &&(filter.denoise_max==null||denoise!=null&&denoise<=filter.denoise_max)
      &&(!filter.orientation||orientation===filter.orientation);
  });
  const sort=filter.sort||"smart";
  return rows.sort((a,b)=>{
    if(sort==="updated_desc")return b.updated_at-a.updated_at;
    if(sort==="updated_asc")return a.updated_at-b.updated_at;
    if(sort==="created_desc")return b.created_at-a.created_at;
    if(sort==="created_asc")return a.created_at-b.created_at;
    if(sort==="name_asc")return a.name.localeCompare(b.name);
    if(sort==="name_desc")return b.name.localeCompare(a.name);
    if(sort==="resolution_desc")return (b.width||0)*(b.height||0)-(a.width||0)*(a.height||0);
    if(sort==="size_desc")return (b.file_size||0)-(a.file_size||0);
    return b.favorite-a.favorite||b.updated_at-a.updated_at;
  });
}

async function mockCall<T>(command:string,args:Record<string,unknown>):Promise<T>{
  await new Promise(r=>setTimeout(r,20));
  switch(command){
    case "library_page":{const filter=args.filter as LibraryFilter;const offset=Number(args.offset||0),limit=Number(args.limit||200);const all=filtered(filter);return {items:all.slice(offset,offset+limit).map(toSummary),total:all.length,offset,limit} as T;}
    case "library_facets":return {
      tags:[{name:"character",count:2},{name:"aero",count:1},{name:"study",count:1}],
      models:[{name:"GPT Image",count:3},{name:"Flux",count:2}],collections:mockCollections,
      metadata_types:[{name:"manual",count:5},{name:"a1111",count:1}],
      samplers:[{name:"DPM++ 2M",count:1}],schedulers:[]
    } as T;
    case "get_asset":return mockAssets.find(x=>x.id===Number(args.id)) as T;
    case "preview_cache_path":return (demoImages.get(Number(args.id))||demoSvg("Preview","#73c7e5","#71bb6a")) as T;
    case "update_prompt":{const id=Number(args.id),patch=args.patch as PromptPatch;mockAssets=mockAssets.map(x=>x.id===id?{...x,...patch,updated_at:Math.floor(Date.now()/1000)}:x);return mockAssets.find(x=>x.id===id) as T;}
    case "replace_tags":{const id=Number(args.id),tags=args.tags as string[];mockAssets=mockAssets.map(x=>x.id===id?{...x,tags}:x);return mockAssets.find(x=>x.id===id) as T;}
    case "toggle_favorite":{const a=mockAssets.find(x=>x.id===Number(args.id));if(a)a.favorite=a.favorite?0:1;return a as T;}
    case "import_paths":case "import_folder":case "import_dropped_paths":return {added:0,skipped:0,duplicates:0,failed:0,last_id:null} as T;
    case "start_import_paths":case "start_import_folder":case "start_import_dropped_paths":return 1 as T;
    case "cancel_import":return true as T;
    case "batch_add_tags":return true as T;
    case "delete_asset":{mockAssets=mockAssets.filter(x=>x.id!==Number(args.id));return true as T;}
    case "add_revision":return true as T;
    case "save_prompt_revision":{
      const id=Number(args.id),patch=args.patch as PromptPatch,tags=args.tags as string[];
      mockAssets=mockAssets.map(x=>x.id===id?{...x,...patch,tags,updated_at:Math.floor(Date.now()/1000)}:x);
      return mockAssets.find(x=>x.id===id) as T;
    }
    case "list_revisions":return [] as T;
    case "restore_revision":return mockAssets[0] as T;
    case "lineage":return {parents:[],children:[]} as T;
    case "add_relation":return true as T;
    case "collections":return mockCollections as T;
    case "duplicate_groups":return [] as T;
    case "create_backup":return {name:"demo-backup.sqlite3",path:"demo",size:123456,created_at:Math.floor(Date.now()/1000)} as T;
    case "ensure_auto_backup":return null as T;
    case "list_backups":return [] as T;
    case "stage_restore":return true as T;
    case "rename_tag":case "merge_tags":case "delete_tag":case "rename_collection":case "delete_collection":return true as T;
    case "create_collection":{const c={id:mockCollections.length+1,name:String(args.name),description:String(args.description||""),created_at:0,updated_at:0,count:0};mockCollections=[...mockCollections,c];return c as T;}
    case "add_to_collection":return true as T;
    case "rescan_metadata":return mockAssets.find(x=>x.id===Number(args.id)) as T;
    case "export_sidecar":return "demo.png.imagelore.json" as T;
    case "refresh_missing":return mockAssets.filter(x=>x.missing).length as T;
    case "relocate_missing":return 0 as T;
    case "generation_sessions":return [{id:1,name:"Blue Character Study",note:"Main iteration session",count:2,created_at:0,updated_at:0}] as T;
    case "create_generation_session":return {id:2,name:String(args.name),note:String(args.note||""),count:0,created_at:0,updated_at:0} as T;
    case "asset_session":return null as T;
    case "set_asset_session":case "update_relation_note":return true as T;
    case "model_aliases":return [{alias:"flux1-dev-fp8.safetensors",canonical:"Flux.1 Dev"}] as T;
    case "upsert_model_alias":case "delete_model_alias":return true as T;
    case "saved_filters":return [] as T;
    case "save_filter":return {id:1,name:String(args.name),filter:args.filter as LibraryFilter,created_at:0,updated_at:0} as T;
    case "delete_saved_filter":return true as T;
    case "library_health":return {total:mockAssets.length,missing:mockAssets.filter(x=>x.missing).length,duplicate_groups:0,without_metadata:1,without_fingerprint:0,pending_relations:0,unassigned_session:mockAssets.length,cache_bytes:10485760} as T;
    case "source_folders":return mockSources as T;
    case "add_source_folder":{const path=String(args.path);const existing=mockSources.find(x=>x.path===path);if(existing)return existing as T;const item={id:mockSources.length+1,path,name:path.split(/[\\/]/).filter(Boolean).at(-1)||path,auto_sync:true,last_scan_at:0,created_at:0,updated_at:0};mockSources=[...mockSources,item];return item as T;}
    case "remove_source_folder":{mockSources=mockSources.filter(x=>x.id!==Number(args.id));return true as T;}
    case "set_source_auto_sync":{const id=Number(args.id),enabled=Boolean(args.enabled);mockSources=mockSources.map(x=>x.id===id?{...x,auto_sync:enabled}:x);return mockSources.find(x=>x.id===id) as T;}
    case "start_sync_sources":return 1 as T;
    case "get_visual_dna":return {subject:"",character:"",outfit:"",pose:"",expression:"",composition:"",camera:"",lighting:"",environment:"",palette:"",material:"",style:"",source:"manual",updated_at:0} as T;
    case "update_visual_dna":return {...(args.value as VisualDnaPatch),updated_at:Math.floor(Date.now()/1000)} as T;
    case "semantic_status":return {model_id:"clip-vit-b32-qdrant-v1",enabled:true,model_ready:true,vision_ready:true,text_ready:true,indexed:mockAssets.length,total:mockAssets.length,stale:0,model_bytes:620000000,index_bytes:mockAssets.length*2048} as T;
    case "start_semantic_index":return 2 as T;
    case "cancel_semantic_index":return true as T;
    case "semantic_search_text":{
      const query=String(args.query||"").toLowerCase();
      const filter=args.filter as LibraryFilter;
      const ranked=filtered({...filter,query:""}).map((asset,index)=>{
        const hay=[asset.name,asset.prompt,asset.negative_prompt,asset.model,...asset.tags].join(" ").toLowerCase();
        const score=hay.includes(query)?0.96:0.82-index*0.03;
        return {asset:toSummary(asset),score};
      }).sort((a,b)=>b.score-a.score);
      return ranked.slice(0,Number(args.limit||240)) as T;
    }
    case "semantic_search_similar":{
      const id=Number(args.assetId),filter=args.filter as LibraryFilter;
      const ranked=filtered({...filter,query:""}).filter(a=>a.id!==id).map((asset,index)=>({asset:toSummary(asset),score:0.94-index*0.04}));
      return ranked.slice(0,Number(args.limit||240)) as T;
    }
    case "clear_semantic_index":case "delete_semantic_models":return true as T;
    case "diagnostics_status":return {
      data_dir:"C:/Users/Demo/AppData/Local/app.imagelore.desktop",
      database_path:"C:/Users/Demo/AppData/Local/app.imagelore.desktop/library.sqlite3",
      backups_dir:"C:/Users/Demo/AppData/Local/app.imagelore.desktop/backups",
      logs_dir:"C:/Users/Demo/AppData/Local/app.imagelore.desktop/logs",
      active_log:"C:/Users/Demo/AppData/Local/app.imagelore.desktop/logs/imagelore.log",
      active_log_size:4096,recovery_notice:null
    } as T;
    case "open_external":case "open_containing_folder":case "copy_asset_to":case "open_data_folder":case "open_logs_folder":return true as T;
    default:throw new Error(`Unknown mock command: ${command}`);
  }
}

async function call<T>(command:string,args:Record<string,unknown>={}):Promise<T>{return isTauri?invoke<T>(command,args):mockCall<T>(command,args)}

export const api={
  page:(filter:LibraryFilter,offset=0,limit=240)=>call<LibraryPage>("library_page",{filter,offset,limit}),
  facets:()=>call<LibraryFacets>("library_facets"),
  get:(id:number)=>call<AssetRecord>("get_asset",{id}),
  preview:async(id:number,maxEdge:number,thumbnail=false)=>{
    const source=await call<string>("preview_cache_path",{id,maxEdge,thumbnail});
    return isTauri?convertFileSrc(source):source;
  },
  importPaths:(paths:string[])=>call<ImportSummary>("import_paths",{paths}),
  importFolder:(path:string)=>call<ImportSummary>("import_folder",{path}),
  importDroppedPaths:(paths:string[])=>call<ImportSummary>("import_dropped_paths",{paths}),
  startImportPaths:(paths:string[])=>call<number>("start_import_paths",{paths}),
  startImportFolder:(path:string)=>call<number>("start_import_folder",{path}),
  startImportDroppedPaths:(paths:string[])=>call<number>("start_import_dropped_paths",{paths}),
  cancelImport:(jobId:number)=>call<boolean>("cancel_import",{jobId}),
  updatePrompt:(id:number,patch:PromptPatch)=>call<AssetRecord>("update_prompt",{id,patch}),
  replaceTags:(id:number,tags:string[])=>call<AssetRecord>("replace_tags",{id,tags}),
  batchAddTags:(ids:number[],tags:string[])=>call<boolean>("batch_add_tags",{ids,tags}),
  toggleFavorite:(id:number)=>call<AssetRecord>("toggle_favorite",{id}),
  deleteAsset:(id:number)=>call<boolean>("delete_asset",{id}),
  addRevision:(id:number,note="")=>call<boolean>("add_revision",{id,note}),
  savePromptRevision:(id:number,patch:PromptPatch,tags:string[],note="")=>call<AssetRecord>("save_prompt_revision",{id,patch,tags,note}),
  revisions:(id:number)=>call<Revision[]>("list_revisions",{id}),
  restoreRevision:(revisionId:number)=>call<AssetRecord>("restore_revision",{revisionId}),
  lineage:(id:number)=>call<Lineage>("lineage",{id}),
  addRelation:(parentId:number,childId:number,relationType:string,note="")=>call<boolean>("add_relation",{parentId,childId,relationType,note}),
  sessions:()=>call<GenerationSession[]>("generation_sessions"),
  createSession:(name:string,note="")=>call<GenerationSession>("create_generation_session",{name,note}),
  assetSession:(assetId:number)=>call<AssetSession|null>("asset_session",{assetId}),
  setAssetSession:(assetId:number,sessionId:number|null,note="")=>call<boolean>("set_asset_session",{assetId,sessionId,note}),
  updateRelationNote:(relationId:number,note:string)=>call<boolean>("update_relation_note",{relationId,note}),
  modelAliases:()=>call<ModelAlias[]>("model_aliases"),
  upsertModelAlias:(alias:string,canonical:string)=>call<boolean>("upsert_model_alias",{alias,canonical}),
  deleteModelAlias:(alias:string)=>call<boolean>("delete_model_alias",{alias}),
  savedFilters:()=>call<SavedFilter[]>("saved_filters"),
  saveFilter:(name:string,filter:LibraryFilter)=>call<SavedFilter>("save_filter",{name,filter}),
  deleteSavedFilter:(id:number)=>call<boolean>("delete_saved_filter",{id}),
  libraryHealth:()=>call<LibraryHealth>("library_health"),
  sourceFolders:()=>call<SourceFolder[]>("source_folders"),
  addSourceFolder:(path:string)=>call<SourceFolder>("add_source_folder",{path}),
  removeSourceFolder:(id:number)=>call<boolean>("remove_source_folder",{id}),
  setSourceAutoSync:(id:number,enabled:boolean)=>call<SourceFolder>("set_source_auto_sync",{id,enabled}),
  startSyncSources:(ids:number[])=>call<number>("start_sync_sources",{ids}),
  collections:()=>call<CollectionRecord[]>("collections"),
  duplicateGroups:()=>call<DuplicateGroup[]>("duplicate_groups"),
  createBackup:()=>call<BackupRecord>("create_backup"),
  ensureAutoBackup:()=>call<BackupRecord|null>("ensure_auto_backup"),
  backups:()=>call<BackupRecord[]>("list_backups"),
  stageRestore:(name:string)=>call<boolean>("stage_restore",{name}),
  renameTag:(oldName:string,newName:string)=>call<boolean>("rename_tag",{oldName,newName}),
  mergeTags:(sourceName:string,targetName:string)=>call<boolean>("merge_tags",{sourceName,targetName}),
  deleteTag:(name:string)=>call<boolean>("delete_tag",{name}),
  renameCollection:(id:number,name:string)=>call<boolean>("rename_collection",{id,name}),
  deleteCollection:(id:number)=>call<boolean>("delete_collection",{id}),
  createCollection:(name:string,description="")=>call<CollectionRecord>("create_collection",{name,description}),
  addToCollection:(collectionId:number,assetIds:number[])=>call<boolean>("add_to_collection",{collectionId,assetIds}),
  rescan:(id:number)=>call<AssetRecord>("rescan_metadata",{id}),
  exportSidecar:(id:number)=>call<string>("export_sidecar",{id}),
  refreshMissing:()=>call<number>("refresh_missing"),
  relocateMissing:(root:string)=>call<number>("relocate_missing",{root}),
  openExternal:(id:number)=>call<boolean>("open_external",{id}),
  openFolder:(id:number)=>call<boolean>("open_containing_folder",{id}),
  copyAssetTo:(id:number,destination:string)=>call<boolean>("copy_asset_to",{id,destination}),
  visualDna:(id:number)=>call<VisualDna>("get_visual_dna",{id}),
  updateVisualDna:(id:number,value:VisualDnaPatch)=>call<VisualDna>("update_visual_dna",{id,value}),
  diagnosticsStatus:()=>call<DiagnosticStatus>("diagnostics_status"),
  openDataFolder:()=>call<boolean>("open_data_folder"),
  openLogsFolder:()=>call<boolean>("open_logs_folder"),
  semanticStatus:()=>call<SemanticStatus>("semantic_status"),
  startSemanticIndex:()=>call<number>("start_semantic_index"),
  cancelSemanticIndex:(jobId:number)=>call<boolean>("cancel_semantic_index",{jobId}),
  semanticSearchText:(query:string,filter:LibraryFilter,limit=240)=>call<SemanticHit[]>("semantic_search_text",{query,filter,limit}),
  semanticSearchSimilar:(assetId:number,filter:LibraryFilter,limit=240)=>call<SemanticHit[]>("semantic_search_similar",{assetId,filter,limit}),
  clearSemanticIndex:()=>call<boolean>("clear_semantic_index"),
  deleteSemanticModels:()=>call<boolean>("delete_semantic_models")
};
