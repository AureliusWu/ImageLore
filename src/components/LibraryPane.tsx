import { AssetGrid } from "./AssetGrid";
import type { AssetRecord, LibraryFacets, LibraryFilter, LibraryView } from "../types";

const views:Array<[LibraryView,string,string]>=[["all","All","▦"],["favorites","Favorites","★"],["recent","Recent","◷"],["missing","Missing","!"]];

export function LibraryPane({assets,total,currentId,selected,loading,filter,facets,onFilter,onAsset,onLoadMore,onBatchTags,onCollection,onClearSelection,onRefreshMissing}:{
  assets:AssetRecord[];total:number;currentId?:number;selected:Set<number>;loading:boolean;filter:LibraryFilter;facets:LibraryFacets;
  onFilter:(next:LibraryFilter)=>void;onAsset:(asset:AssetRecord,e:React.MouseEvent)=>void;onLoadMore:()=>void;onBatchTags:()=>void;onCollection:()=>void;onClearSelection:()=>void;onRefreshMissing:()=>void;
}){
  const activeFilters=Number(!!filter.tag)+Number(!!filter.model)+Number(!!filter.collection_id);
  return <aside className="library-pane panel glass-surface">
    <div className="pane-heading"><div><span className="eyebrow">LIBRARY</span><div className="heading-line"><strong>Generation Records</strong><span className="count-pill">{total}</span></div></div><button className="icon-button" title="Refresh missing-file status" onClick={onRefreshMissing}>↻</button></div>
    <div className="view-switch" role="tablist">{views.map(([value,label,icon])=><button key={value} className={filter.view===value?"active":""} onClick={()=>onFilter({...filter,view:value})}><span>{icon}</span>{label}</button>)}</div>
    <div className="filter-strip">
      <label><span>Collection</span><select value={filter.collection_id??""} onChange={e=>onFilter({...filter,collection_id:e.target.value?Number(e.target.value):null})}><option value="">All collections</option>{facets.collections.map(c=><option key={c.id} value={c.id}>{c.name} ({c.count})</option>)}</select></label>
      <label><span>Tag</span><select value={filter.tag??""} onChange={e=>onFilter({...filter,tag:e.target.value||null})}><option value="">All tags</option>{facets.tags.map(x=><option key={x.name} value={x.name}>{x.name} ({x.count})</option>)}</select></label>
      <label><span>Model</span><select value={filter.model??""} onChange={e=>onFilter({...filter,model:e.target.value||null})}><option value="">All models</option>{facets.models.map(x=><option key={x.name} value={x.name}>{x.name} ({x.count})</option>)}</select></label>
      {activeFilters>0?<button className="clear-filter" onClick={()=>onFilter({...filter,tag:null,model:null,collection_id:null})}>Clear {activeFilters} filter{activeFilters>1?"s":""}</button>:null}
    </div>
    {selected.size>1?<div className="selection-bar"><strong>{selected.size} selected</strong><button onClick={onBatchTags}>Add tags</button><button onClick={onCollection}>Collection</button><button onClick={onClearSelection}>Clear</button></div>:null}
    {assets.length?<AssetGrid assets={assets} total={total} currentId={currentId} selected={selected} loading={loading} onAsset={onAsset} onLoadMore={onLoadMore}/>:<div className="library-empty"><span className="empty-orb">✦</span><strong>No records yet</strong><p>Import images to build a searchable memory of prompts, parameters and generation relationships.</p></div>}
  </aside>
}
