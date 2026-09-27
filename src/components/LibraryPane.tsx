import { AssetGrid } from "./AssetGrid";
import type { AssetSummary,LibraryFacets,LibraryFilter,LibraryView,SavedFilter } from "../types";

const views:Array<[LibraryView,string,string]>=[["all","全部","▦"],["favorites","收藏","★"],["recent","最近","◷"],["missing","缺失","!"]];

export function LibraryPane({assets,total,currentId,selected,loading,filter,facets,savedFilters,onFilter,onAsset,onLoadMore,onBatchTags,onCollection,onClearSelection,onRefreshMissing,onManage,onSaveView,onApplySavedView}:{
  assets:AssetSummary[];total:number;currentId?:number;selected:Set<number>;loading:boolean;filter:LibraryFilter;facets:LibraryFacets;savedFilters:SavedFilter[];
  onFilter:(next:LibraryFilter)=>void;onAsset:(asset:AssetSummary,e:React.MouseEvent)=>void;onLoadMore:()=>void;
  onBatchTags:()=>void;onCollection:()=>void;onClearSelection:()=>void;onRefreshMissing:()=>void;onManage:()=>void;onSaveView:()=>void;onApplySavedView:(view:SavedFilter)=>void;
}){
  const activeFilters=Number(!!filter.tag)+Number(!!filter.model)+Number(!!filter.collection_id);
  return <aside className="library-pane panel glass-surface">
    <div className="pane-heading"><div><span className="eyebrow">图库</span><div className="heading-line"><strong>生成记录</strong><span className="count-pill">{total}</span></div></div><div className="pane-actions"><button className="icon-button" title="资料库管理" onClick={onManage}>⚙</button><button className="icon-button" title="刷新缺失文件状态" onClick={onRefreshMissing}>↻</button></div></div>
    <div className="view-switch" role="tablist">{views.map(([value,label,icon])=><button key={value} className={filter.view===value?"active":""} onClick={()=>onFilter({...filter,view:value})}><span>{icon}</span>{label}</button>)}</div>
    <div className="saved-view-bar"><select defaultValue="" onChange={e=>{const item=savedFilters.find(x=>x.id===Number(e.target.value));if(item)onApplySavedView(item);e.currentTarget.value=""}}><option value="">保存视图…</option>{savedFilters.map(x=><option key={x.id} value={x.id}>{x.name}</option>)}</select><button onClick={onSaveView}>＋ 保存当前筛选</button></div>
    <div className="filter-strip">
      <label><span>集合</span><select value={filter.collection_id??""} onChange={e=>onFilter({...filter,collection_id:e.target.value?Number(e.target.value):null})}><option value="">全部集合</option>{facets.collections.map(c=><option key={c.id} value={c.id}>{c.name} ({c.count})</option>)}</select></label>
      <label><span>标签</span><select value={filter.tag??""} onChange={e=>onFilter({...filter,tag:e.target.value||null})}><option value="">全部标签</option>{facets.tags.map(x=><option key={x.name} value={x.name}>{x.name} ({x.count})</option>)}</select></label>
      <label><span>模型</span><select value={filter.model??""} onChange={e=>onFilter({...filter,model:e.target.value||null})}><option value="">全部模型</option>{facets.models.map(x=><option key={x.name} value={x.name}>{x.name} ({x.count})</option>)}</select></label>
      {activeFilters>0?<button className="clear-filter" onClick={()=>onFilter({...filter,tag:null,model:null,collection_id:null})}>清除 {activeFilters} 个筛选条件</button>:null}
    </div>
    {selected.size>1?<div className="selection-bar"><strong>已选择 {selected.size} 项</strong><button onClick={onBatchTags}>添加标签</button><button onClick={onCollection}>加入集合</button><button onClick={onClearSelection}>取消选择</button></div>:null}
    {assets.length?<AssetGrid assets={assets} total={total} currentId={currentId} selected={selected} loading={loading} onAsset={onAsset} onLoadMore={onLoadMore}/>:<div className="library-empty"><span className="empty-orb">✦</span><strong>还没有生成记录</strong><p>导入图片后，ImageLore 会帮你保存并检索提示词、参数和生成关系。</p></div>}
  </aside>
}
