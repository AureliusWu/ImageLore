import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { open } from "@tauri-apps/plugin-dialog";
import { api, isTauri } from "./api";
import { APP_VERSION } from "./version";
import type { AssetRecord, CollectionRecord, LibraryFacets, LibraryFilter, Lineage, Revision } from "./types";
import { useDebouncedEffect } from "./hooks/useDebouncedEffect";
import { useDebouncedValue } from "./hooks/useDebouncedValue";
import { AppHeader } from "./components/AppHeader";
import { LibraryPane } from "./components/LibraryPane";
import { PreviewPane } from "./components/PreviewPane";
import { InspectorPane, type InspectorTab } from "./components/InspectorPane";
import { Modal } from "./components/Modal";

type PreviewMode="fit"|"actual";
type ModalState=
  |{kind:"batch-tags"}
  |{kind:"history";revisions:Revision[]}
  |{kind:"collection";collections:CollectionRecord[]}
  |{kind:"link-parent"}
  |{kind:"remove"}
  |null;

const PAGE_SIZE=240;
const emptyFacets:LibraryFacets={tags:[],models:[],collections:[]};
const emptyLineage:Lineage={parents:[],children:[]};
const parseTags=(text:string)=>[...new Set(text.split(/[,，]/).map(x=>x.trim()).filter(Boolean))];

export default function App(){
  const[version,setVersion]=useState(APP_VERSION);
  const[filter,setFilter]=useState<LibraryFilter>({query:"",view:"all",tag:null,model:null,collection_id:null});
  const debouncedQuery=useDebouncedValue(filter.query,180);
  const effectiveFilter=useMemo(()=>({...filter,query:debouncedQuery}),[filter,debouncedQuery]);
  const[assets,setAssets]=useState<AssetRecord[]>([]);const[total,setTotal]=useState(0);const[loading,setLoading]=useState(false);const[facets,setFacets]=useState<LibraryFacets>(emptyFacets);
  const[current,setCurrent]=useState<AssetRecord|null>(null);const[selected,setSelected]=useState<Set<number>>(new Set());
  const[preview,setPreview]=useState("");const[previewMode,setPreviewMode]=useState<PreviewMode>(()=>(localStorage.getItem("imagelore.preview")||"fit") as PreviewMode);
  const[tab,setTab]=useState<InspectorTab>("prompt");const[lineage,setLineage]=useState<Lineage>(emptyLineage);const[compareRecord,setCompareRecord]=useState<AssetRecord|null>(null);
  const[prompt,setPrompt]=useState("");const[negative,setNegative]=useState("");const[model,setModel]=useState("");const[tagsText,setTagsText]=useState("");
  const[status,setStatus]=useState("就绪");const[modal,setModal]=useState<ModalState>(null);const[dialogText,setDialogText]=useState("");const[dialogChoice,setDialogChoice]=useState("");
  const[leftWidth,setLeftWidth]=useState(Number(localStorage.getItem("imagelore.left")||348));const[rightWidth,setRightWidth]=useState(Number(localStorage.getItem("imagelore.right")||510));
  const promptRef=useRef<HTMLTextAreaElement>(null);

  useEffect(()=>{if(isTauri)getVersion().then(setVersion).catch(()=>setVersion(APP_VERSION))},[]);
  useEffect(()=>{localStorage.setItem("imagelore.preview",previewMode)},[previewMode]);
  useEffect(()=>{localStorage.setItem("imagelore.left",String(leftWidth));localStorage.setItem("imagelore.right",String(rightWidth))},[leftWidth,rightWidth]);

  const refreshFacets=useCallback(()=>api.facets().then(setFacets).catch(()=>setFacets(emptyFacets)),[]);
  const refresh=useCallback(async(preferId?:number)=>{
    setLoading(true);
    try{
      const page=await api.page(effectiveFilter,0,PAGE_SIZE);setAssets(page.items);setTotal(page.total);
      let next:AssetRecord|null=null;
      if(preferId)next=page.items.find(x=>x.id===preferId)??await api.get(preferId).catch(()=>null);
      if(!next&&current?.id)next=page.items.find(x=>x.id===current.id)??null;
      if(!next)next=page.items[0]??null;
      setCurrent(next);if(next)setSelected(new Set([next.id]));else setSelected(new Set());
    }catch(e){setStatus(`图库加载失败：${String(e)}`)}finally{setLoading(false)}
  },[effectiveFilter,current?.id]);
  const loadMore=useCallback(async()=>{if(loading||assets.length>=total)return;setLoading(true);try{const page=await api.page(effectiveFilter,assets.length,PAGE_SIZE);setAssets(prev=>{const ids=new Set(prev.map(x=>x.id));return[...prev,...page.items.filter(x=>!ids.has(x.id))]});setTotal(page.total)}catch(e){setStatus(String(e))}finally{setLoading(false)}},[loading,assets,effectiveFilter,total]);

  useEffect(()=>{refresh().catch(()=>{})},[debouncedQuery,filter.view,filter.tag,filter.model,filter.collection_id]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(()=>{refreshFacets()},[refreshFacets]);
  useEffect(()=>{
    if(!current){setPreview("");setLineage(emptyLineage);setCompareRecord(null);setPrompt("");setNegative("");setModel("");setTagsText("");return}
    setPrompt(current.prompt);setNegative(current.negative_prompt);setModel(current.model);setTagsText(current.tags.join(", "));
    setStatus("正在加载预览…");api.preview(current.id,previewMode==="fit"?2200:4200,false).then(src=>{setPreview(src);setStatus("就绪")}).catch(e=>{setPreview("");setStatus(`预览失败：${String(e)}`)});
    api.lineage(current.id).then(async x=>{setLineage(x);const parent=x.parents[0];setCompareRecord(parent?await api.get(parent.other_id).catch(()=>null):null)}).catch(()=>setLineage(emptyLineage));
  },[current?.id,previewMode]);

  useDebouncedEffect(()=>{if(!current)return;setStatus("正在保存提示词…");api.updatePrompt(current.id,{prompt,negative_prompt:negative,model}).then(a=>{setCurrent(a);setAssets(xs=>xs.map(x=>x.id===a.id?a:x));setStatus("已保存")}).catch(e=>setStatus(`保存失败：${String(e)}`))},[prompt,negative,model,current?.id],650);
  useDebouncedEffect(()=>{if(!current)return;const tags=parseTags(tagsText);if(JSON.stringify(tags)===JSON.stringify(current.tags))return;api.replaceTags(current.id,tags).then(a=>{setCurrent(a);setAssets(xs=>xs.map(x=>x.id===a.id?a:x));refreshFacets();setStatus("标签已保存")}).catch(e=>setStatus(`标签保存失败：${String(e)}`))},[tagsText,current?.id],800);

  const chooseImages=async()=>{if(!isTauri){setStatus("文件选择器仅在桌面版中可用");return}const picked=await open({multiple:true,filters:[{name:"图片",extensions:["png","jpg","jpeg","webp","bmp","gif"]}]});if(!picked)return;const paths=Array.isArray(picked)?picked:[picked];setStatus("正在导入…");const result=await api.importPaths(paths);setStatus(`已导入 ${result.added} 张；跳过 ${result.skipped} 张；失败 ${result.failed} 张`);await refreshFacets();await refresh(result.last_id??undefined)};
  const chooseFolder=async()=>{if(!isTauri){setStatus("文件夹选择器仅在桌面版中可用");return}const picked=await open({directory:true,multiple:false});if(!picked||Array.isArray(picked))return;setStatus("正在扫描文件夹…");const result=await api.importFolder(picked);setStatus(`已导入 ${result.added} 张；跳过 ${result.skipped} 张；失败 ${result.failed} 张`);await refreshFacets();await refresh(result.last_id??undefined)};
  const importDerivative=async()=>{if(!current)return;if(!isTauri){setStatus("此功能仅在桌面版中可用");return}const picked=await open({multiple:false,filters:[{name:"图片",extensions:["png","jpg","jpeg","webp","bmp","gif"]}]});if(!picked||Array.isArray(picked))return;const result=await api.importPaths([picked]);if(result.last_id){await api.addRelation(current.id,result.last_id,"derived_from","");await refresh(result.last_id);setTab("lineage");setStatus("派生图已关联")}};

  const onAsset=(asset:AssetRecord,e:React.MouseEvent)=>{if(e.ctrlKey||e.metaKey){setSelected(prev=>{const next=new Set(prev);next.has(asset.id)?next.delete(asset.id):next.add(asset.id);return next})}else{setSelected(new Set([asset.id]))}setCurrent(asset)};
  const toggleFavorite=async()=>{if(!current)return;const a=await api.toggleFavorite(current.id);setCurrent(a);setAssets(xs=>xs.map(x=>x.id===a.id?a:x));};
  const copyPrompt=()=>{if(!current)return;navigator.clipboard.writeText(prompt).then(()=>setStatus("提示词已复制"))};
  const saveRevision=async()=>{if(!current)return;await api.addRevision(current.id,"");setStatus("提示词版本已保存")};
  const openHistory=async()=>{if(!current)return;setDialogChoice("");setModal({kind:"history",revisions:await api.revisions(current.id)})};
  const openCollection=async()=>{setDialogChoice("");setDialogText("");setModal({kind:"collection",collections:await api.collections()})};
  const openBatchTags=()=>{setDialogText("");setModal({kind:"batch-tags"})};
  const openLinkParent=()=>{setDialogChoice("");setModal({kind:"link-parent"})};
  const remove=()=>setModal({kind:"remove"});
  const rescan=async()=>{if(!current)return;const a=await api.rescan(current.id);setCurrent(a);setAssets(xs=>xs.map(x=>x.id===a.id?a:x));setStatus("元数据已刷新")};
  const exportSidecar=async()=>{if(!current)return;const path=await api.exportSidecar(current.id);setStatus(`Sidecar 已导出：${path}`)};
  const refreshMissing=async()=>{setStatus("正在检查文件位置…");const n=await api.refreshMissing();setStatus(`发现 ${n} 个缺失文件`);await refresh()};
  const repairMissing=async()=>{if(!isTauri){setStatus("此功能仅在桌面版中可用");return}const root=await open({directory:true,multiple:false});if(!root||Array.isArray(root))return;const n=await api.relocateMissing(root);setStatus(`已重新定位 ${n} 条记录`);await refresh()};
  const compare=async(id:number)=>setCompareRecord(await api.get(id).catch(()=>null));

  const confirmModal=async()=>{if(!modal)return;
    if(modal.kind==="batch-tags"){const ids=selected.size?[...selected]:current?[current.id]:[];await api.batchAddTags(ids,parseTags(dialogText));setModal(null);await refreshFacets();await refresh(current?.id);return}
    if(modal.kind==="history"){if(!dialogChoice)return;const a=await api.restoreRevision(Number(dialogChoice));setCurrent(a);setAssets(xs=>xs.map(x=>x.id===a.id?a:x));setModal(null);setStatus("历史版本已恢复");return}
    if(modal.kind==="collection"){let id=Number(dialogChoice);if(dialogChoice==="new"){if(!dialogText.trim())return;id=(await api.createCollection(dialogText.trim(),"")).id}if(!id)return;const ids=selected.size?[...selected]:current?[current.id]:[];await api.addToCollection(id,ids);setModal(null);await refreshFacets();setStatus("已加入集合");return}
    if(modal.kind==="link-parent"){if(!current||!dialogChoice)return;await api.addRelation(Number(dialogChoice),current.id,"reference","");setModal(null);setLineage(await api.lineage(current.id));setCompareRecord(await api.get(Number(dialogChoice)));setStatus("父图已关联");return}
    if(modal.kind==="remove"){if(!current)return;await api.deleteAsset(current.id);setModal(null);setCurrent(null);await refreshFacets();await refresh();setStatus("记录已移除")}
  };

  useEffect(()=>{const handler=(e:KeyboardEvent)=>{if(e.key==="Escape"&&modal){setModal(null);return}if(e.ctrlKey&&e.key.toLowerCase()==="f"){e.preventDefault();document.querySelector<HTMLInputElement>("#search")?.focus()}if(e.key==="F6"){e.preventDefault();promptRef.current?.focus()}if(e.key==="F7"){e.preventDefault();setPreviewMode(x=>x==="fit"?"actual":"fit")}if(e.ctrlKey&&e.shiftKey&&e.key.toLowerCase()==="c"){e.preventDefault();copyPrompt()}if(e.ctrlKey&&e.key.toLowerCase()==="s"){e.preventDefault();saveRevision()}if(e.ctrlKey&&e.key.toLowerCase()==="i"){e.preventDefault();chooseImages()}if(e.altKey&&(e.key==="ArrowUp"||e.key==="ArrowDown")){e.preventDefault();if(!current)return;const i=assets.findIndex(x=>x.id===current.id);const n=e.key==="ArrowUp"?Math.max(0,i-1):Math.min(assets.length-1,i+1);if(assets[n]){setCurrent(assets[n]);setSelected(new Set([assets[n].id]))}}};window.addEventListener("keydown",handler);return()=>window.removeEventListener("keydown",handler)},[current,assets,prompt,modal]); // eslint-disable-line react-hooks/exhaustive-deps

  const drag=(side:"left"|"right")=>(e:React.PointerEvent)=>{e.currentTarget.setPointerCapture(e.pointerId);const start=e.clientX,initial=side==="left"?leftWidth:rightWidth;const move=(ev:PointerEvent)=>{const delta=ev.clientX-start;if(side==="left")setLeftWidth(Math.min(590,Math.max(280,initial+delta)));else setRightWidth(Math.min(780,Math.max(410,initial-delta)))};const up=()=>{window.removeEventListener("pointermove",move);window.removeEventListener("pointerup",up)};window.addEventListener("pointermove",move);window.addEventListener("pointerup",up)};

  return <div className="app-shell"><div className="aero-background" aria-hidden="true"><i className="cloud a"/><i className="cloud b"/><i className="bubble a"/><i className="bubble b"/></div>
    <AppHeader version={version} query={filter.query} onQuery={query=>setFilter(f=>({...f,query}))} onImport={chooseImages} onFolder={chooseFolder}/>
    <main className="workspace" style={{gridTemplateColumns:`${leftWidth}px 8px minmax(360px,1fr) 8px ${rightWidth}px`}}>
      <LibraryPane assets={assets} total={total} currentId={current?.id} selected={selected} loading={loading} filter={filter} facets={facets} onFilter={setFilter} onAsset={onAsset} onLoadMore={loadMore} onBatchTags={openBatchTags} onCollection={openCollection} onClearSelection={()=>setSelected(new Set())} onRefreshMissing={refreshMissing}/>
      <div className="splitter" onPointerDown={drag("left")}/>
      <PreviewPane asset={current} src={preview} mode={previewMode} onMode={setPreviewMode} onImport={chooseImages} onOpen={()=>current&&api.openExternal(current.id)} onFolder={()=>current&&api.openFolder(current.id)}/>
      <div className="splitter" onPointerDown={drag("right")}/>
      <InspectorPane asset={current} tab={tab} onTab={setTab} prompt={prompt} onPrompt={setPrompt} negative={negative} onNegative={setNegative} model={model} onModel={setModel} tagsText={tagsText} onTagsText={setTagsText} lineage={lineage} compareRecord={compareRecord} onCompare={compare} onCopy={copyPrompt} onSaveRevision={saveRevision} onHistory={openHistory} onFavorite={toggleFavorite} onRescan={rescan} onSidecar={exportSidecar} onCollection={openCollection} onRemove={remove} onImportDerivative={importDerivative} onLinkParent={openLinkParent} promptRef={promptRef}/>
    </main>
    <footer className="statusbar glass-surface"><span className={`runtime-dot ${isTauri?"native":"preview"}`}/><strong>{isTauri?"桌面版":"浏览器预览"}</strong><span>v{version}</span><span className="status-message">{status}</span><span>已加载 {assets.length}/{total}</span><button onClick={repairMissing} title="根据文件指纹查找移动后的文件">修复缺失文件</button><span className="shortcut">F6 提示词 · F7 预览 · Ctrl+S 保存版本</span></footer>
    {modal?<Modal title={modal.kind==="batch-tags"?"添加标签":modal.kind==="history"?"提示词历史":modal.kind==="collection"?"加入集合":modal.kind==="link-parent"?"关联父图":"移除记录"} description={modal.kind==="batch-tags"?"为所有已选记录添加相同标签。":modal.kind==="history"?"恢复一个已保存的提示词快照；恢复前会先保留当前内容。":modal.kind==="collection"?"把当前选择加入一个项目集合。":modal.kind==="link-parent"?"选择另一条记录作为来源或参考父图。":"仅移除 ImageLore 中的记录与元数据，原始图片仍保留在磁盘上。"} onClose={()=>setModal(null)} onConfirm={confirmModal} confirmLabel={modal.kind==="remove"?"移除记录":modal.kind==="history"?"恢复":"确认"} danger={modal.kind==="remove"}>
      {modal.kind==="batch-tags"?<label className="modal-field"><span>标签</span><input autoFocus value={dialogText} onChange={e=>setDialogText(e.target.value)} placeholder="角色, 卧室, 参考图"/></label>:null}
      {modal.kind==="history"?<div className="choice-list">{modal.revisions.length?modal.revisions.map(r=><label className={dialogChoice===String(r.id)?"selected":""} key={r.id}><input type="radio" name="revision" value={r.id} checked={dialogChoice===String(r.id)} onChange={e=>setDialogChoice(e.target.value)}/><div><strong>{new Date(r.created_at*1000).toLocaleString("zh-CN")}</strong><span>{r.note||"提示词快照"}</span><p>{r.prompt.slice(0,150)||"（空提示词）"}</p></div></label>):<p className="muted">还没有保存过历史版本。</p>}</div>:null}
      {modal.kind==="collection"?<><div className="choice-grid">{modal.collections.map(c=><label className={dialogChoice===String(c.id)?"selected":""} key={c.id}><input type="radio" name="collection" value={c.id} checked={dialogChoice===String(c.id)} onChange={e=>setDialogChoice(e.target.value)}/><div><strong>{c.name}</strong><span>{c.count} 条记录</span></div></label>)}<label className={dialogChoice==="new"?"selected":""}><input type="radio" name="collection" value="new" checked={dialogChoice==="new"} onChange={e=>setDialogChoice(e.target.value)}/><div><strong>＋ 新建集合</strong><span>创建一个项目分组</span></div></label></div>{dialogChoice==="new"?<label className="modal-field"><span>名称</span><input autoFocus value={dialogText} onChange={e=>setDialogText(e.target.value)} placeholder="角色研究"/></label>:null}</>:null}
      {modal.kind==="link-parent"?<div className="choice-list">{assets.filter(x=>x.id!==current?.id).slice(0,150).map(a=><label className={dialogChoice===String(a.id)?"selected":""} key={a.id}><input type="radio" name="parent" value={a.id} checked={dialogChoice===String(a.id)} onChange={e=>setDialogChoice(e.target.value)}/><div><strong>{a.name}</strong><span>{a.model||a.metadata_type}</span></div></label>)}</div>:null}
      {modal.kind==="remove"?<div className="warning-card"><span>!</span><div><strong>原始图片不会被删除</strong><p>只会移除这条 ImageLore 记录、历史版本、关系和集合归属。</p></div></div>:null}
    </Modal>:null}
  </div>
}
