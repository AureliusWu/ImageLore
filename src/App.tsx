import { useCallback,useEffect,useMemo,useRef,useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { open } from "@tauri-apps/plugin-dialog";
import { api,isTauri } from "./api";
import { APP_VERSION } from "./version";
import type { AssetRecord,AssetSummary,BackupRecord,DuplicateGroup,ImportSummary,LibraryFacets,LibraryFilter,Lineage } from "./types";
import { useDebouncedValue } from "./hooks/useDebouncedValue";
import { useEditorDraft } from "./hooks/useEditorDraft";
import { useNativeDrop } from "./hooks/useNativeDrop";
import { useWorkspaceLayout } from "./hooks/useWorkspaceLayout";
import { AppHeader } from "./components/AppHeader";
import { LibraryPane } from "./components/LibraryPane";
import { PreviewPane } from "./components/PreviewPane";
import { InspectorPane,type InspectorTab } from "./components/InspectorPane";
import { AppDialogs,type AppModalState } from "./components/AppDialogs";
import { LibraryManager } from "./components/LibraryManager";
import { ParentPicker } from "./components/ParentPicker";
import { useImportJob } from "./hooks/useImportJob";

const PAGE_SIZE=240;
const emptyFacets:LibraryFacets={tags:[],models:[],collections:[]};
const emptyLineage:Lineage={parents:[],children:[]};
const parseTags=(text:string)=>[...new Set(text.split(/[,，]/).map(x=>x.trim()).filter(Boolean))];

export default function App(){
  const[version,setVersion]=useState(APP_VERSION);
  const[status,setStatus]=useState("就绪");
  const[filter,setFilter]=useState<LibraryFilter>({query:"",view:"all",tag:null,model:null,collection_id:null});
  const debouncedQuery=useDebouncedValue(filter.query,180);
  const effectiveFilter=useMemo(()=>({...filter,query:debouncedQuery}),[filter,debouncedQuery]);

  const[assets,setAssets]=useState<AssetSummary[]>([]);
  const[total,setTotal]=useState(0);
  const[loading,setLoading]=useState(false);
  const[facets,setFacets]=useState<LibraryFacets>(emptyFacets);
  const[current,setCurrent]=useState<AssetRecord|null>(null);
  const[selected,setSelected]=useState<Set<number>>(new Set());
  const[preview,setPreview]=useState("");
  const[tab,setTab]=useState<InspectorTab>("prompt");
  const[lineage,setLineage]=useState<Lineage>(emptyLineage);
  const[compareRecord,setCompareRecord]=useState<AssetRecord|null>(null);
  const[managerOpen,setManagerOpen]=useState(false);
  const[backups,setBackups]=useState<BackupRecord[]>([]);
  const[duplicates,setDuplicates]=useState<DuplicateGroup[]>([]);
  const[parentOpen,setParentOpen]=useState(false);
  const[parentQuery,setParentQuery]=useState("");
  const[parentChoice,setParentChoice]=useState<number|null>(null);
  const[parentResults,setParentResults]=useState<AssetSummary[]>([]);
  const[parentLoading,setParentLoading]=useState(false);
  const debouncedParentQuery=useDebouncedValue(parentQuery,180);

  const[modal,setModal]=useState<AppModalState>(null);
  const[dialogText,setDialogText]=useState("");
  const[dialogChoice,setDialogChoice]=useState("");
  const promptRef=useRef<HTMLTextAreaElement>(null);
  const{previewMode,setPreviewMode,leftWidth,rightWidth,drag}=useWorkspaceLayout();

  const refreshFacets=useCallback(()=>api.facets().then(setFacets).catch(()=>setFacets(emptyFacets)),[]);
  const onEditorSaved=useCallback((a:AssetRecord)=>setCurrent(prev=>prev?.id===a.id?a:prev),[]);
  const editor=useEditorDraft(current,onEditorSaved,refreshFacets,setStatus);
  const{prompt,negative,model,tagsText,setPrompt,setNegative,setModel,setTagsText,flush:flushEditor,load:loadEditor}=editor;

  const selectRecord=useCallback(async(id:number,selection?:Set<number>)=>{
    await flushEditor();
    const record=await api.get(id);
    setCurrent(record);setSelected(selection??new Set([id]));
  },[flushEditor]);

  const refresh=useCallback(async(preferId?:number)=>{
    await flushEditor();setLoading(true);
    try{
      const page=await api.page(effectiveFilter,0,PAGE_SIZE);
      setAssets(page.items);setTotal(page.total);
      const nextId=preferId??(current?.id&&page.items.some(x=>x.id===current.id)?current.id:page.items[0]?.id);
      const next=nextId?await api.get(nextId).catch(()=>null):null;
      setCurrent(next);loadEditor(next);setSelected(next?new Set([next.id]):new Set());
    }catch(e){setStatus(`图库加载失败：${String(e)}`)}finally{setLoading(false)}
  },[effectiveFilter,current?.id,flushEditor,loadEditor]);

  const loadMore=useCallback(async()=>{
    if(loading||assets.length>=total)return;
    setLoading(true);
    try{
      const page=await api.page(effectiveFilter,assets.length,PAGE_SIZE);
      setAssets(prev=>{const ids=new Set(prev.map(x=>x.id));return[...prev,...page.items.filter(x=>!ids.has(x.id))]});
      setTotal(page.total);
    }catch(e){setStatus(String(e))}finally{setLoading(false)}
  },[loading,assets,effectiveFilter,total]);

  useEffect(()=>{if(isTauri)getVersion().then(setVersion).catch(()=>setVersion(APP_VERSION))},[]);
  useEffect(()=>{void refresh()},[debouncedQuery,filter.view,filter.tag,filter.model,filter.collection_id]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(()=>{void refreshFacets()},[refreshFacets]);
  useEffect(()=>{if(isTauri)void api.ensureAutoBackup().catch(()=>{})},[]);
  useEffect(()=>{
    if(!parentOpen)return;
    setParentLoading(true);
    api.page({query:debouncedParentQuery,view:"all",tag:null,model:null,collection_id:null},0,100)
      .then(page=>setParentResults(page.items.filter(x=>x.id!==current?.id)))
      .catch(()=>setParentResults([]))
      .finally(()=>setParentLoading(false));
  },[parentOpen,debouncedParentQuery,current?.id]);
  useEffect(()=>{
    if(!current){setPreview("");setLineage(emptyLineage);setCompareRecord(null);return}
    setStatus("正在加载预览…");
    api.preview(current.id,previewMode==="fit"?2200:0,false).then(src=>{setPreview(src);setStatus("就绪")}).catch(e=>{setPreview("");setStatus(`预览失败：${String(e)}`)});
    api.lineage(current.id).then(async x=>{setLineage(x);const p=x.parents[0];setCompareRecord(p?await api.get(p.other_id).catch(()=>null):null)}).catch(()=>setLineage(emptyLineage));
  },[current?.id,previewMode]);

  const importDone=useCallback(async(result:ImportSummary)=>{
    await refreshFacets();await refresh(result.last_id??undefined);
  },[refreshFacets,refresh]);
  const importJob=useImportJob(importDone,setStatus);
  const importImmediate=useCallback(async(label:string,task:()=>Promise<ImportSummary>)=>{
    await flushEditor();setStatus(label);
    try{
      const result=await task();
      setStatus("导入完成：新增 "+result.added+"，重复 "+result.duplicates+"，跳过 "+result.skipped+"，失败 "+result.failed);
      await importDone(result);
    }catch(e){setStatus("导入失败："+String(e))}
  },[flushEditor,importDone]);
  const startBackgroundImport=useCallback(async(label:string,starter:()=>Promise<number>,fallback:()=>Promise<ImportSummary>)=>{
    await flushEditor();
    if(isTauri){await importJob.start(label,starter)}else{await importImmediate(label,fallback)}
  },[flushEditor,importJob,importImmediate]);

  const chooseImages=async()=>{
    if(!isTauri){setStatus("文件选择器仅在桌面版中可用");return}
    const picked=await open({multiple:true,filters:[{name:"图片",extensions:["png","jpg","jpeg","webp","bmp","gif"]}]});
    if(!picked)return;const paths=Array.isArray(picked)?picked:[picked];
    await startBackgroundImport("正在准备导入…",()=>api.startImportPaths(paths),()=>api.importPaths(paths));
  };
  const chooseFolder=async()=>{
    if(!isTauri){setStatus("文件夹选择器仅在桌面版中可用");return}
    const picked=await open({directory:true,multiple:false});if(!picked||Array.isArray(picked))return;
    await startBackgroundImport("正在扫描文件夹…",()=>api.startImportFolder(picked),()=>api.importFolder(picked));
  };
  const handleDrop=useCallback((paths:string[])=>startBackgroundImport("正在扫描拖入内容…",()=>api.startImportDroppedPaths(paths),()=>api.importDroppedPaths(paths)),[startBackgroundImport]);
  const drop=useNativeDrop(handleDrop,setStatus);

  const importDerivative=async()=>{
    if(!current)return;if(!isTauri){setStatus("此功能仅在桌面版中可用");return}
    const picked=await open({multiple:false,filters:[{name:"图片",extensions:["png","jpg","jpeg","webp","bmp","gif"]}]});
    if(!picked||Array.isArray(picked))return;
    await flushEditor();const result=await api.importPaths([picked]);
    if(result.last_id){await api.addRelation(current.id,result.last_id,"derived_from","");await refresh(result.last_id);setTab("lineage");setStatus("派生图已关联")}
  };

  const onAsset=async(asset:AssetSummary,e:React.MouseEvent)=>{
    let next=new Set([asset.id]);
    if(e.ctrlKey||e.metaKey){next=new Set(selected);next.has(asset.id)?next.delete(asset.id):next.add(asset.id)}
    await selectRecord(asset.id,next);
  };
  const toggleFavorite=async()=>{if(!current)return;const a=await api.toggleFavorite(current.id);setCurrent(a);setAssets(xs=>xs.map(x=>x.id===a.id?{...x,favorite:a.favorite,updated_at:a.updated_at}:x))};
  const copyPrompt=()=>{if(current)void navigator.clipboard.writeText(prompt).then(()=>setStatus("提示词已复制"))};
  const saveRevision=async()=>{if(!current)return;await flushEditor();await api.addRevision(current.id,"");setStatus("提示词版本已保存")};
  const openHistory=async()=>{if(current){setDialogChoice("");setModal({kind:"history",revisions:await api.revisions(current.id)})}};
  const openCollection=async()=>{setDialogChoice("");setDialogText("");setModal({kind:"collection",collections:await api.collections()})};
  const rescan=async()=>{if(!current)return;await flushEditor();const a=await api.rescan(current.id);setCurrent(a);loadEditor(a);setStatus("元数据已刷新")};
  const exportSidecar=async()=>{if(current)setStatus(`Sidecar 已导出：${await api.exportSidecar(current.id)}`)};
  const refreshMissing=async()=>{setStatus("正在检查文件位置…");const n=await api.refreshMissing();setStatus(`发现 ${n} 个缺失文件`);await refresh()};
  const repairMissing=async()=>{if(!isTauri){setStatus("此功能仅在桌面版中可用");return}const root=await open({directory:true,multiple:false});if(!root||Array.isArray(root))return;const n=await api.relocateMissing(root);setStatus(`已重新定位 ${n} 条记录`);await refresh()};
  const compare=async(id:number)=>setCompareRecord(await api.get(id).catch(()=>null));
  const refreshManager=useCallback(async()=>{
    const[b,d]=await Promise.all([api.backups(),api.duplicateGroups()]);
    setBackups(b);setDuplicates(d);await refreshFacets();
  },[refreshFacets]);
  const openManager=async()=>{await flushEditor();await refreshManager();setManagerOpen(true)};
  const createBackup=async()=>{setStatus("正在备份资料库…");await api.createBackup();await refreshManager();setStatus("资料库备份完成")};
  const restoreBackup=async(name:string)=>{if(!window.confirm("确定恢复到这份备份吗？当前资料库会在重启时先自动保留一份安全副本。"))return;await api.stageRestore(name);setStatus("恢复已准备完成，请重启 ImageLore 后生效")};
  const renameTag=async(oldName:string,newName:string)=>{await api.renameTag(oldName,newName);await refreshManager();await refresh(current?.id)};
  const mergeTag=async(source:string,target:string)=>{await api.mergeTags(source,target);await refreshManager();await refresh(current?.id)};
  const deleteTag=async(name:string)=>{if(!window.confirm("删除标签“"+name+"”？图片记录本身不会被删除。"))return;await api.deleteTag(name);await refreshManager();await refresh(current?.id)};
  const renameCollection=async(id:number,name:string)=>{await api.renameCollection(id,name);await refreshManager()};
  const deleteCollection=async(id:number)=>{if(!window.confirm("删除这个集合？集合内的图片记录不会被删除。"))return;await api.deleteCollection(id);await refreshManager();setFilter(f=>f.collection_id===id?{...f,collection_id:null}:f)};
  const openParentPicker=()=>{setParentQuery("");setParentChoice(null);setParentOpen(true)};
  const confirmParent=async()=>{if(!current||!parentChoice)return;await api.addRelation(parentChoice,current.id,"reference","");setParentOpen(false);setLineage(await api.lineage(current.id));setCompareRecord(await api.get(parentChoice));setStatus("父图已关联")};

  const confirmModal=async()=>{
    if(!modal)return;
    if(modal.kind==="batch-tags"){const ids=selected.size?[...selected]:current?[current.id]:[];await api.batchAddTags(ids,parseTags(dialogText));setModal(null);await refreshFacets();await refresh(current?.id);return}
    if(modal.kind==="history"){if(!dialogChoice)return;const a=await api.restoreRevision(Number(dialogChoice));setCurrent(a);loadEditor(a);setModal(null);setStatus("历史版本已恢复");return}
    if(modal.kind==="collection"){let id=Number(dialogChoice);if(dialogChoice==="new"){if(!dialogText.trim())return;id=(await api.createCollection(dialogText.trim(),"")).id}if(!id)return;const ids=selected.size?[...selected]:current?[current.id]:[];await api.addToCollection(id,ids);setModal(null);await refreshFacets();setStatus("已加入集合");return}
    if(modal.kind==="remove"){if(!current)return;await flushEditor();await api.deleteAsset(current.id);setModal(null);setCurrent(null);await refreshFacets();await refresh();setStatus("记录已移除")}
  };

  useEffect(()=>{
    const handler=(e:KeyboardEvent)=>{
      if(e.key==="Escape"){if(modal){setModal(null);return}if(parentOpen){setParentOpen(false);return}if(managerOpen){setManagerOpen(false);return}}
      if(e.ctrlKey&&e.key.toLowerCase()==="f"){e.preventDefault();document.querySelector<HTMLInputElement>("#search")?.focus()}
      if(e.key==="F6"){e.preventDefault();promptRef.current?.focus()}
      if(e.key==="F7"){e.preventDefault();setPreviewMode(x=>x==="fit"?"actual":"fit")}
      if(e.ctrlKey&&e.shiftKey&&e.key.toLowerCase()==="c"){e.preventDefault();copyPrompt()}
      if(e.ctrlKey&&e.key.toLowerCase()==="s"){e.preventDefault();void saveRevision()}
      if(e.ctrlKey&&e.key.toLowerCase()==="i"){e.preventDefault();void chooseImages()}
      if(e.altKey&&(e.key==="ArrowUp"||e.key==="ArrowDown")){
        e.preventDefault();if(!current)return;
        const i=assets.findIndex(x=>x.id===current.id),n=e.key==="ArrowUp"?Math.max(0,i-1):Math.min(assets.length-1,i+1);
        if(assets[n])void selectRecord(assets[n].id);
      }
    };
    window.addEventListener("keydown",handler);return()=>window.removeEventListener("keydown",handler);
  },[modal,current,assets,prompt,selectRecord]); // eslint-disable-line react-hooks/exhaustive-deps

  return <div className="app-shell">
    <div className="aero-background" aria-hidden="true"><i className="cloud a"/><i className="cloud b"/><i className="bubble a"/><i className="bubble b"/></div>
    {drop.active?<div className="drop-overlay" aria-live="polite"><div className="drop-card"><span className="drop-orb">⇩</span><strong>松开鼠标即可导入</strong><p>{drop.count?`检测到 ${drop.count} 个项目`:"正在识别拖入内容"} · 支持图片和文件夹</p><small>文件夹会递归扫描；已存在的图片会自动跳过</small></div></div>:null}
    <AppHeader version={version} query={filter.query} onQuery={query=>setFilter(f=>({...f,query}))} onImport={chooseImages} onFolder={chooseFolder}/>
    <main className="workspace" style={{gridTemplateColumns:`${leftWidth}px 8px minmax(360px,1fr) 8px ${rightWidth}px`}}>
      <LibraryPane assets={assets} total={total} currentId={current?.id} selected={selected} loading={loading} filter={filter} facets={facets} onFilter={setFilter} onAsset={onAsset} onLoadMore={loadMore} onBatchTags={()=>{setDialogText("");setModal({kind:"batch-tags"})}} onCollection={openCollection} onClearSelection={()=>setSelected(new Set())} onRefreshMissing={refreshMissing} onManage={openManager}/>
      <div className="splitter" onPointerDown={drag("left")}/>
      <PreviewPane asset={current} src={preview} mode={previewMode} onMode={setPreviewMode} onImport={chooseImages} onOpen={()=>current&&api.openExternal(current.id)} onFolder={()=>current&&api.openFolder(current.id)}/>
      <div className="splitter" onPointerDown={drag("right")}/>
      <InspectorPane asset={current} tab={tab} onTab={setTab} prompt={prompt} onPrompt={setPrompt} negative={negative} onNegative={setNegative} model={model} onModel={setModel} tagsText={tagsText} onTagsText={setTagsText} lineage={lineage} compareRecord={compareRecord} onCompare={compare} onCopy={copyPrompt} onSaveRevision={saveRevision} onHistory={openHistory} onFavorite={toggleFavorite} onRescan={rescan} onSidecar={exportSidecar} onCollection={openCollection} onRemove={()=>setModal({kind:"remove"})} onImportDerivative={importDerivative} onLinkParent={openParentPicker} promptRef={promptRef}/>
    </main>
    <footer className="statusbar glass-surface"><span className={`runtime-dot ${isTauri?"native":"preview"}`}/><strong>{isTauri?"桌面版":"浏览器预览"}</strong><span>v{version}</span><span className="status-message">{status}</span>{importJob.active?<><progress max={Math.max(1,importJob.progress.total)} value={importJob.progress.processed}/><button onClick={importJob.cancel}>取消导入</button></>:null}<span>已加载 {assets.length}/{total}</span><button onClick={repairMissing} title="根据文件指纹查找移动后的文件">修复缺失文件</button><span className="shortcut">F6 提示词 · F7 预览 · Ctrl+S 保存版本</span></footer>
    <LibraryManager open={managerOpen} backups={backups} tags={facets.tags} collections={facets.collections} duplicates={duplicates} onClose={()=>setManagerOpen(false)} onBackup={createBackup} onRestore={restoreBackup} onRenameTag={renameTag} onDeleteTag={deleteTag} onRenameCollection={renameCollection} onDeleteCollection={deleteCollection}/>
    <ParentPicker open={parentOpen} query={parentQuery} results={parentResults} choice={parentChoice} loading={parentLoading} onQuery={setParentQuery} onChoice={setParentChoice} onClose={()=>setParentOpen(false)} onConfirm={confirmParent}/>
    <AppDialogs modal={modal} assets={assets} currentId={current?.id} text={dialogText} setText={setDialogText} choice={dialogChoice} setChoice={setDialogChoice} onClose={()=>setModal(null)} onConfirm={confirmModal}/>
  </div>
}
