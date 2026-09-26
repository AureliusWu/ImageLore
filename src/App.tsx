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
  const[status,setStatus]=useState("Ready");const[modal,setModal]=useState<ModalState>(null);const[dialogText,setDialogText]=useState("");const[dialogChoice,setDialogChoice]=useState("");
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
    }catch(e){setStatus(`Library error: ${String(e)}`)}finally{setLoading(false)}
  },[effectiveFilter,current?.id]);
  const loadMore=useCallback(async()=>{if(loading||assets.length>=total)return;setLoading(true);try{const page=await api.page(effectiveFilter,assets.length,PAGE_SIZE);setAssets(prev=>{const ids=new Set(prev.map(x=>x.id));return[...prev,...page.items.filter(x=>!ids.has(x.id))]});setTotal(page.total)}catch(e){setStatus(String(e))}finally{setLoading(false)}},[loading,assets,effectiveFilter,total]);

  useEffect(()=>{refresh().catch(()=>{})},[debouncedQuery,filter.view,filter.tag,filter.model,filter.collection_id]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(()=>{refreshFacets()},[refreshFacets]);
  useEffect(()=>{
    if(!current){setPreview("");setLineage(emptyLineage);setCompareRecord(null);setPrompt("");setNegative("");setModel("");setTagsText("");return}
    setPrompt(current.prompt);setNegative(current.negative_prompt);setModel(current.model);setTagsText(current.tags.join(", "));
    setStatus("Loading preview…");api.preview(current.id,previewMode==="fit"?2200:4200,false).then(src=>{setPreview(src);setStatus("Ready")}).catch(e=>{setPreview("");setStatus(`Preview error: ${String(e)}`)});
    api.lineage(current.id).then(async x=>{setLineage(x);const parent=x.parents[0];setCompareRecord(parent?await api.get(parent.other_id).catch(()=>null):null)}).catch(()=>setLineage(emptyLineage));
  },[current?.id,previewMode]);

  useDebouncedEffect(()=>{if(!current)return;setStatus("Saving prompt…");api.updatePrompt(current.id,{prompt,negative_prompt:negative,model}).then(a=>{setCurrent(a);setAssets(xs=>xs.map(x=>x.id===a.id?a:x));setStatus("Saved")}).catch(e=>setStatus(`Save failed: ${String(e)}`))},[prompt,negative,model,current?.id],650);
  useDebouncedEffect(()=>{if(!current)return;const tags=parseTags(tagsText);if(JSON.stringify(tags)===JSON.stringify(current.tags))return;api.replaceTags(current.id,tags).then(a=>{setCurrent(a);setAssets(xs=>xs.map(x=>x.id===a.id?a:x));refreshFacets();setStatus("Tags saved")}).catch(e=>setStatus(`Tag save failed: ${String(e)}`))},[tagsText,current?.id],800);

  const chooseImages=async()=>{if(!isTauri){setStatus("File picker is available in the native app");return}const picked=await open({multiple:true,filters:[{name:"Images",extensions:["png","jpg","jpeg","webp","bmp","gif"]}]});if(!picked)return;const paths=Array.isArray(picked)?picked:[picked];setStatus("Importing…");const result=await api.importPaths(paths);setStatus(`Imported ${result.added}; skipped ${result.skipped}; failed ${result.failed}`);await refreshFacets();await refresh(result.last_id??undefined)};
  const chooseFolder=async()=>{if(!isTauri){setStatus("Folder picker is available in the native app");return}const picked=await open({directory:true,multiple:false});if(!picked||Array.isArray(picked))return;setStatus("Scanning folder…");const result=await api.importFolder(picked);setStatus(`Imported ${result.added}; skipped ${result.skipped}; failed ${result.failed}`);await refreshFacets();await refresh(result.last_id??undefined)};
  const importDerivative=async()=>{if(!current)return;if(!isTauri){setStatus("Native app required");return}const picked=await open({multiple:false,filters:[{name:"Images",extensions:["png","jpg","jpeg","webp","bmp","gif"]}]});if(!picked||Array.isArray(picked))return;const result=await api.importPaths([picked]);if(result.last_id){await api.addRelation(current.id,result.last_id,"derived_from","");await refresh(result.last_id);setTab("lineage");setStatus("Derivative linked")}};

  const onAsset=(asset:AssetRecord,e:React.MouseEvent)=>{if(e.ctrlKey||e.metaKey){setSelected(prev=>{const next=new Set(prev);next.has(asset.id)?next.delete(asset.id):next.add(asset.id);return next})}else{setSelected(new Set([asset.id]))}setCurrent(asset)};
  const toggleFavorite=async()=>{if(!current)return;const a=await api.toggleFavorite(current.id);setCurrent(a);setAssets(xs=>xs.map(x=>x.id===a.id?a:x));};
  const copyPrompt=()=>{if(!current)return;navigator.clipboard.writeText(prompt).then(()=>setStatus("Prompt copied"))};
  const saveRevision=async()=>{if(!current)return;await api.addRevision(current.id,"");setStatus("Prompt version saved")};
  const openHistory=async()=>{if(!current)return;setDialogChoice("");setModal({kind:"history",revisions:await api.revisions(current.id)})};
  const openCollection=async()=>{setDialogChoice("");setDialogText("");setModal({kind:"collection",collections:await api.collections()})};
  const openBatchTags=()=>{setDialogText("");setModal({kind:"batch-tags"})};
  const openLinkParent=()=>{setDialogChoice("");setModal({kind:"link-parent"})};
  const remove=()=>setModal({kind:"remove"});
  const rescan=async()=>{if(!current)return;const a=await api.rescan(current.id);setCurrent(a);setAssets(xs=>xs.map(x=>x.id===a.id?a:x));setStatus("Metadata refreshed")};
  const exportSidecar=async()=>{if(!current)return;const path=await api.exportSidecar(current.id);setStatus(`Sidecar: ${path}`)};
  const refreshMissing=async()=>{setStatus("Checking file locations…");const n=await api.refreshMissing();setStatus(`${n} missing file${n===1?"":"s"}`);await refresh()};
  const repairMissing=async()=>{if(!isTauri){setStatus("Native app required");return}const root=await open({directory:true,multiple:false});if(!root||Array.isArray(root))return;const n=await api.relocateMissing(root);setStatus(`Relocated ${n} record${n===1?"":"s"}`);await refresh()};
  const compare=async(id:number)=>setCompareRecord(await api.get(id).catch(()=>null));

  const confirmModal=async()=>{if(!modal)return;
    if(modal.kind==="batch-tags"){const ids=selected.size?[...selected]:current?[current.id]:[];await api.batchAddTags(ids,parseTags(dialogText));setModal(null);await refreshFacets();await refresh(current?.id);return}
    if(modal.kind==="history"){if(!dialogChoice)return;const a=await api.restoreRevision(Number(dialogChoice));setCurrent(a);setAssets(xs=>xs.map(x=>x.id===a.id?a:x));setModal(null);setStatus("Revision restored");return}
    if(modal.kind==="collection"){let id=Number(dialogChoice);if(dialogChoice==="new"){if(!dialogText.trim())return;id=(await api.createCollection(dialogText.trim(),"")).id}if(!id)return;const ids=selected.size?[...selected]:current?[current.id]:[];await api.addToCollection(id,ids);setModal(null);await refreshFacets();setStatus("Added to collection");return}
    if(modal.kind==="link-parent"){if(!current||!dialogChoice)return;await api.addRelation(Number(dialogChoice),current.id,"reference","");setModal(null);setLineage(await api.lineage(current.id));setCompareRecord(await api.get(Number(dialogChoice)));setStatus("Parent linked");return}
    if(modal.kind==="remove"){if(!current)return;await api.deleteAsset(current.id);setModal(null);setCurrent(null);await refreshFacets();await refresh();setStatus("Record removed")}
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
    <footer className="statusbar glass-surface"><span className={`runtime-dot ${isTauri?"native":"preview"}`}/><strong>{isTauri?"Native Desktop":"Browser Preview"}</strong><span>v{version}</span><span className="status-message">{status}</span><span>{assets.length}/{total} loaded</span><button onClick={repairMissing} title="Find moved files by fingerprint">Relocate missing</button><span className="shortcut">F6 Prompt · F7 Preview · Ctrl+S Version</span></footer>
    {modal?<Modal title={modal.kind==="batch-tags"?"Add tags":modal.kind==="history"?"Prompt history":modal.kind==="collection"?"Add to collection":modal.kind==="link-parent"?"Link parent":"Remove record"} description={modal.kind==="batch-tags"?"Add the same tags to every selected record.":modal.kind==="history"?"Restore a saved Prompt snapshot. Current content is preserved as a revision first.":modal.kind==="collection"?"Group the current selection into a project collection.":modal.kind==="link-parent"?"Choose another record as a source or reference parent.":"This removes ImageLore metadata only. The original image stays on disk."} onClose={()=>setModal(null)} onConfirm={confirmModal} confirmLabel={modal.kind==="remove"?"Remove record":modal.kind==="history"?"Restore":"Confirm"} danger={modal.kind==="remove"}>
      {modal.kind==="batch-tags"?<label className="modal-field"><span>Tags</span><input autoFocus value={dialogText} onChange={e=>setDialogText(e.target.value)} placeholder="character, bedroom, reference"/></label>:null}
      {modal.kind==="history"?<div className="choice-list">{modal.revisions.length?modal.revisions.map(r=><label className={dialogChoice===String(r.id)?"selected":""} key={r.id}><input type="radio" name="revision" value={r.id} checked={dialogChoice===String(r.id)} onChange={e=>setDialogChoice(e.target.value)}/><div><strong>{new Date(r.created_at*1000).toLocaleString()}</strong><span>{r.note||"Prompt snapshot"}</span><p>{r.prompt.slice(0,150)||"(empty prompt)"}</p></div></label>):<p className="muted">No saved revisions yet.</p>}</div>:null}
      {modal.kind==="collection"?<><div className="choice-grid">{modal.collections.map(c=><label className={dialogChoice===String(c.id)?"selected":""} key={c.id}><input type="radio" name="collection" value={c.id} checked={dialogChoice===String(c.id)} onChange={e=>setDialogChoice(e.target.value)}/><div><strong>{c.name}</strong><span>{c.count} records</span></div></label>)}<label className={dialogChoice==="new"?"selected":""}><input type="radio" name="collection" value="new" checked={dialogChoice==="new"} onChange={e=>setDialogChoice(e.target.value)}/><div><strong>＋ New collection</strong><span>Create a project group</span></div></label></div>{dialogChoice==="new"?<label className="modal-field"><span>Name</span><input autoFocus value={dialogText} onChange={e=>setDialogText(e.target.value)} placeholder="Character study"/></label>:null}</>:null}
      {modal.kind==="link-parent"?<div className="choice-list">{assets.filter(x=>x.id!==current?.id).slice(0,150).map(a=><label className={dialogChoice===String(a.id)?"selected":""} key={a.id}><input type="radio" name="parent" value={a.id} checked={dialogChoice===String(a.id)} onChange={e=>setDialogChoice(e.target.value)}/><div><strong>{a.name}</strong><span>{a.model||a.metadata_type}</span></div></label>)}</div>:null}
      {modal.kind==="remove"?<div className="warning-card"><span>!</span><div><strong>Original image will not be deleted</strong><p>Only this ImageLore record, revisions, relationships and collection memberships are removed.</p></div></div>:null}
    </Modal>:null}
  </div>
}
