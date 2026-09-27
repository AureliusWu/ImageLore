import { useCallback,useEffect,useMemo,useRef,useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { open } from "@tauri-apps/plugin-dialog";
import { api,isTauri } from "./api";
import { APP_VERSION } from "./version";
import type { AssetRecord,AssetSession,AssetSummary,BackupRecord,DuplicateGroup,GenerationSession,ImportSummary,LibraryFacets,LibraryFilter,LibraryHealth,Lineage,ModelAlias,SavedFilter } from "./types";
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
import { useCloseGuard } from "./hooks/useCloseGuard";

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
  const[compareParentSrc,setCompareParentSrc]=useState("");
  const[compareCurrentSrc,setCompareCurrentSrc]=useState("");
  const[sessions,setSessions]=useState<GenerationSession[]>([]);
  const[assetSession,setAssetSession]=useState<AssetSession|null>(null);
  const[modelAliases,setModelAliases]=useState<ModelAlias[]>([]);
  const[savedFilters,setSavedFilters]=useState<SavedFilter[]>([]);
  const[health,setHealth]=useState<LibraryHealth|null>(null);
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
  const refreshSeq=useRef(0);
  const selectSeq=useRef(0);
  const{previewMode,setPreviewMode,leftWidth,rightWidth,drag}=useWorkspaceLayout();

  const refreshFacets=useCallback(()=>api.facets().then(setFacets).catch(()=>setFacets(emptyFacets)),[]);
  const refreshSessions=useCallback(()=>api.sessions().then(setSessions).catch(()=>setSessions([])),[]);
  const refreshSavedFilters=useCallback(()=>api.savedFilters().then(setSavedFilters).catch(()=>setSavedFilters([])),[]);
  const onEditorSaved=useCallback((a:AssetRecord)=>setCurrent(prev=>prev?.id===a.id?a:prev),[]);
  const editor=useEditorDraft(current,onEditorSaved,refreshFacets,setStatus);
  const{prompt,negative,model,tagsText,setPrompt,setNegative,setModel,setTagsText,flush:flushEditor,load:loadEditor}=editor;

  const selectRecord=useCallback(async(id:number,selection?:Set<number>)=>{
    const seq=++selectSeq.current;
    ++refreshSeq.current;
    setLoading(false);
    await flushEditor();
    try{
      const record=await api.get(id);
      if(seq!==selectSeq.current)return;
      setCurrent(record);setSelected(selection??new Set([id]));
    }catch(e){
      if(seq===selectSeq.current)setStatus("记录加载失败："+String(e));
    }
  },[flushEditor]);

  const refresh=useCallback(async(preferId?:number)=>{
    const seq=++refreshSeq.current;
    await flushEditor();
    if(seq!==refreshSeq.current)return;
    setLoading(true);
    try{
      const page=await api.page(effectiveFilter,0,PAGE_SIZE);
      if(seq!==refreshSeq.current)return;
      const preferredVisible=preferId&&page.items.some(x=>x.id===preferId)?preferId:undefined;
      const nextId=preferredVisible??(current?.id&&page.items.some(x=>x.id===current.id)?current.id:page.items[0]?.id);
      const next=nextId?await api.get(nextId).catch(()=>null):null;
      if(seq!==refreshSeq.current)return;
      setAssets(page.items);setTotal(page.total);
      setCurrent(next);loadEditor(next);setSelected(next?new Set([next.id]):new Set());
    }catch(e){
      if(seq===refreshSeq.current)setStatus(`图库加载失败：${String(e)}`);
    }finally{
      if(seq===refreshSeq.current)setLoading(false);
    }
  },[effectiveFilter,current?.id,flushEditor,loadEditor]);

  const loadMore=useCallback(async()=>{
    if(loading||assets.length>=total)return;
    const seq=refreshSeq.current;
    setLoading(true);
    try{
      const page=await api.page(effectiveFilter,assets.length,PAGE_SIZE);
      if(seq!==refreshSeq.current)return;
      setAssets(prev=>{const ids=new Set(prev.map(x=>x.id));return[...prev,...page.items.filter(x=>!ids.has(x.id))]});
      setTotal(page.total);
    }catch(e){
      if(seq===refreshSeq.current)setStatus(String(e));
    }finally{
      if(seq===refreshSeq.current)setLoading(false);
    }
  },[loading,assets,effectiveFilter,total]);

  useEffect(()=>{if(isTauri)getVersion().then(setVersion).catch(()=>setVersion(APP_VERSION))},[]);
  useEffect(()=>{void refresh()},[debouncedQuery,filter.view,filter.tag,filter.model,filter.collection_id]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(()=>{void refreshFacets()},[refreshFacets]);
  useEffect(()=>{void refreshSessions();void refreshSavedFilters()},[refreshSessions,refreshSavedFilters]);
  useEffect(()=>{if(isTauri)void api.ensureAutoBackup().catch(e=>setStatus("自动备份失败："+String(e)))},[]);
  useEffect(()=>{
    if(!parentOpen)return;
    let cancelled=false;
    setParentLoading(true);
    api.page({query:debouncedParentQuery,view:"all",tag:null,model:null,collection_id:null},0,100)
      .then(page=>{if(!cancelled)setParentResults(page.items.filter(x=>x.id!==current?.id))})
      .catch(()=>{if(!cancelled)setParentResults([])})
      .finally(()=>{if(!cancelled)setParentLoading(false)});
    return()=>{cancelled=true};
  },[parentOpen,debouncedParentQuery,current?.id]);
  useEffect(()=>{
    if(!current){setPreview("");setLineage(emptyLineage);setCompareRecord(null);setAssetSession(null);return}
    let cancelled=false;
    const assetId=current.id;
    setPreview("");
    setLineage(emptyLineage);
    setCompareRecord(null);
    setStatus("正在加载预览…");
    api.preview(assetId,previewMode==="fit"?2200:0,false)
      .then(src=>{if(!cancelled){setPreview(src);setStatus("就绪")}})
      .catch(e=>{if(!cancelled){setPreview("");setStatus(`预览失败：${String(e)}`)}});
    void Promise.all([
      api.lineage(assetId),
      api.assetSession(assetId).catch(()=>null)
    ]).then(async([x,session])=>{
      const p=x.parents[0];
      const parent=p?await api.get(p.other_id).catch(()=>null):null;
      if(!cancelled){setLineage(x);setCompareRecord(parent);setAssetSession(session)}
    }).catch(()=>{if(!cancelled){setLineage(emptyLineage);setAssetSession(null)}});
    return()=>{cancelled=true};
  },[current?.id,previewMode]);

  useEffect(()=>{
    if(!current||!compareRecord){setCompareParentSrc("");setCompareCurrentSrc("");return}
    let cancelled=false;
    setCompareParentSrc("");setCompareCurrentSrc("");
    Promise.all([
      api.preview(compareRecord.id,1200,false).catch(()=>""),api.preview(current.id,1200,false).catch(()=>"")
    ]).then(([parentSrc,currentSrc])=>{if(!cancelled){setCompareParentSrc(parentSrc);setCompareCurrentSrc(currentSrc)}});
    return()=>{cancelled=true};
  },[current?.id,compareRecord?.id]);

  const importDone=useCallback(async(result:ImportSummary)=>{
    await refreshFacets();await refresh(result.last_id??undefined);
  },[refreshFacets,refresh]);
  const importJob=useImportJob(importDone,setStatus);
  const{start:startImportJob,cancel:cancelImportJob,active:importActive,progress:importProgress}=importJob;
  useCloseGuard(flushEditor,importActive?cancelImportJob:undefined);
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
    if(isTauri){await startImportJob(label,starter)}else{await importImmediate(label,fallback)}
  },[flushEditor,startImportJob,importImmediate]);

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
    if(result.last_id===current.id){setStatus("所选图片与当前记录内容完全相同，未建立自引用关系");return}
    if(result.last_id){
      await api.addRelation(current.id,result.last_id,"derived_from","");
      if(assetSession)await api.setAssetSession(result.last_id,assetSession.session_id,"");
      await refreshSessions();await refresh(result.last_id);setTab("lineage");setStatus("派生图已关联");return
    }
    if(result.duplicates){setStatus("该派生图与资料库中的现有图片内容完全相同，未建立重复谱系")}
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
    const[b,d,a,s,h]=await Promise.all([api.backups(),api.duplicateGroups(),api.modelAliases(),api.savedFilters(),api.libraryHealth()]);
    setBackups(b);setDuplicates(d);setModelAliases(a);setSavedFilters(s);setHealth(h);await refreshFacets();
  },[refreshFacets]);
  const openManager=async()=>{try{await flushEditor();await refreshManager();setManagerOpen(true)}catch(e){setStatus("打开资料库管理失败："+String(e))}};
  const createBackup=async()=>{try{setStatus("正在备份资料库…");await api.createBackup();await refreshManager();setStatus("资料库备份完成")}catch(e){setStatus("资料库备份失败："+String(e))}};
  const restoreBackup=async(name:string)=>{if(!window.confirm("确定恢复到这份备份吗？当前资料库会在重启时先自动保留一份安全副本。"))return;try{await api.stageRestore(name);setStatus("恢复已准备完成，请重启 ImageLore 后生效")}catch(e){setStatus("准备恢复失败："+String(e))}};
  const renameTag=async(oldName:string,newName:string)=>{try{await api.renameTag(oldName,newName);setFilter(f=>f.tag?.toLocaleLowerCase()===oldName.toLocaleLowerCase()?{...f,tag:newName}:f);await refreshManager();await refresh(current?.id)}catch(e){setStatus("标签修改失败："+String(e))}};
  const deleteTag=async(name:string)=>{if(!window.confirm("删除标签“"+name+"”？图片记录本身不会被删除。"))return;try{await api.deleteTag(name);setFilter(f=>f.tag?.toLocaleLowerCase()===name.toLocaleLowerCase()?{...f,tag:null}:f);await refreshManager();await refresh(current?.id)}catch(e){setStatus("删除标签失败："+String(e))}};
  const renameCollection=async(id:number,name:string)=>{try{await api.renameCollection(id,name);await refreshManager()}catch(e){setStatus("集合重命名失败："+String(e))}};
  const deleteCollection=async(id:number)=>{if(!window.confirm("删除这个集合？集合内的图片记录不会被删除。"))return;try{await api.deleteCollection(id);await refreshManager();setFilter(f=>f.collection_id===id?{...f,collection_id:null}:f)}catch(e){setStatus("删除集合失败："+String(e))}};
  const upsertModelAlias=async(alias:string,canonical:string)=>{try{await api.upsertModelAlias(alias,canonical);setFilter(f=>f.model?.toLocaleLowerCase()===alias.toLocaleLowerCase()?{...f,model:canonical}:f);await refreshManager();await refreshFacets();await refresh(current?.id)}catch(e){setStatus("模型别名保存失败："+String(e))}};
  const deleteModelAlias=async(alias:string)=>{try{await api.deleteModelAlias(alias);await refreshManager();await refreshFacets();await refresh(current?.id)}catch(e){setStatus("模型别名删除失败："+String(e))}};
  const saveCurrentView=async()=>{const name=window.prompt("保存当前筛选为","");if(!name?.trim())return;try{await api.saveFilter(name.trim(),filter);await refreshSavedFilters();setStatus("筛选视图已保存")}catch(e){setStatus("保存筛选失败："+String(e))}};
  const applySavedView=(view:SavedFilter)=>{setFilter({...view.filter});setStatus("已应用保存视图："+view.name)};
  const deleteSavedView=async(id:number)=>{try{await api.deleteSavedFilter(id);await refreshSavedFilters();await refreshManager()}catch(e){setStatus("删除保存视图失败："+String(e))}};
  const setSession=async(sessionId:number|null)=>{if(!current)return;try{await api.setAssetSession(current.id,sessionId,assetSession?.asset_note||"");setAssetSession(await api.assetSession(current.id));await refreshSessions();setStatus(sessionId?"已加入生成会话":"已移出生成会话")}catch(e){setStatus("生成会话更新失败："+String(e))}};
  const createSession=async()=>{if(!current)return;const name=window.prompt("新建 Generation Session","");if(!name?.trim())return;const note=window.prompt("会话说明（可留空）","")||"";try{const session=await api.createSession(name.trim(),note);await api.setAssetSession(current.id,session.id,"");await refreshSessions();setAssetSession(await api.assetSession(current.id));setStatus("Generation Session 已创建")}catch(e){setStatus("新建会话失败："+String(e))}};
  const editSessionNote=async()=>{if(!current||!assetSession)return;const note=window.prompt("当前图片在此会话中的备注",assetSession.asset_note||"");if(note===null)return;try{await api.setAssetSession(current.id,assetSession.session_id,note);setAssetSession(await api.assetSession(current.id));setStatus("会话备注已保存")}catch(e){setStatus("会话备注保存失败："+String(e))}};
  const editRelationNote=async(relationId:number,currentNote:string)=>{const note=window.prompt("Branch Note",currentNote);if(note===null||!current)return;try{await api.updateRelationNote(relationId,note);setLineage(await api.lineage(current.id));setStatus("分支备注已保存")}catch(e){setStatus("分支备注保存失败："+String(e))}};
  const openParentPicker=()=>{setParentQuery("");setParentChoice(null);setParentOpen(true)};
  const confirmParent=async()=>{if(!current||!parentChoice)return;try{await api.addRelation(parentChoice,current.id,"reference","");setParentOpen(false);setLineage(await api.lineage(current.id));setCompareRecord(await api.get(parentChoice));setStatus("父图已关联")}catch(e){setStatus("关联父图失败："+String(e))}};

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
  },[modal,parentOpen,managerOpen,current,assets,prompt,selectRecord]); // eslint-disable-line react-hooks/exhaustive-deps

  return <div className="app-shell">
    <div className="aero-background" aria-hidden="true"><i className="cloud a"/><i className="cloud b"/><i className="bubble a"/><i className="bubble b"/></div>
    {drop.active?<div className="drop-overlay" aria-live="polite"><div className="drop-card"><span className="drop-orb">⇩</span><strong>松开鼠标即可导入</strong><p>{drop.count?`检测到 ${drop.count} 个项目`:"正在识别拖入内容"} · 支持图片和文件夹</p><small>文件夹会递归扫描；已存在的图片会自动跳过</small></div></div>:null}
    <AppHeader version={version} query={filter.query} onQuery={query=>setFilter(f=>({...f,query}))} onImport={chooseImages} onFolder={chooseFolder}/>
    <main className="workspace" style={{gridTemplateColumns:`${leftWidth}px 8px minmax(360px,1fr) 8px ${rightWidth}px`}}>
      <LibraryPane assets={assets} total={total} currentId={current?.id} selected={selected} loading={loading} filter={filter} facets={facets} savedFilters={savedFilters} onFilter={setFilter} onAsset={onAsset} onLoadMore={loadMore} onBatchTags={()=>{setDialogText("");setModal({kind:"batch-tags"})}} onCollection={openCollection} onClearSelection={()=>setSelected(new Set())} onRefreshMissing={refreshMissing} onManage={openManager} onSaveView={saveCurrentView} onApplySavedView={applySavedView}/>
      <div className="splitter" onPointerDown={drag("left")}/>
      <PreviewPane asset={current} src={preview} mode={previewMode} onMode={setPreviewMode} onImport={chooseImages} onOpen={()=>current&&api.openExternal(current.id)} onFolder={()=>current&&api.openFolder(current.id)}/>
      <div className="splitter" onPointerDown={drag("right")}/>
      <InspectorPane asset={current} tab={tab} onTab={setTab} prompt={prompt} onPrompt={setPrompt} negative={negative} onNegative={setNegative} model={model} onModel={setModel} tagsText={tagsText} onTagsText={setTagsText} lineage={lineage} compareRecord={compareRecord} compareParentSrc={compareParentSrc} compareCurrentSrc={compareCurrentSrc} sessions={sessions} assetSession={assetSession} onCompare={compare} onCopy={copyPrompt} onSaveRevision={saveRevision} onHistory={openHistory} onFavorite={toggleFavorite} onRescan={rescan} onSidecar={exportSidecar} onCollection={openCollection} onRemove={()=>setModal({kind:"remove"})} onImportDerivative={importDerivative} onLinkParent={openParentPicker} onSetSession={setSession} onCreateSession={createSession} onEditSessionNote={editSessionNote} onEditRelationNote={editRelationNote} promptRef={promptRef}/>
    </main>
    <footer className="statusbar glass-surface"><span className={`runtime-dot ${isTauri?"native":"preview"}`}/><strong>{isTauri?"桌面版":"浏览器预览"}</strong><span>v{version}</span><span className="status-message">{status}</span>{importActive?<><progress max={Math.max(1,importProgress.total)} value={importProgress.processed}/><button onClick={cancelImportJob}>取消导入</button></>:null}<span>已加载 {assets.length}/{total}</span><button onClick={repairMissing} title="根据文件指纹查找移动后的文件">修复缺失文件</button><span className="shortcut">F6 提示词 · F7 预览 · Ctrl+S 保存版本</span></footer>
    <LibraryManager open={managerOpen} backups={backups} tags={facets.tags} collections={facets.collections} duplicates={duplicates} modelAliases={modelAliases} savedFilters={savedFilters} health={health} onClose={()=>setManagerOpen(false)} onBackup={createBackup} onRestore={restoreBackup} onRenameTag={renameTag} onDeleteTag={deleteTag} onRenameCollection={renameCollection} onDeleteCollection={deleteCollection} onUpsertModelAlias={upsertModelAlias} onDeleteModelAlias={deleteModelAlias} onDeleteSavedFilter={deleteSavedView}/>
    <ParentPicker open={parentOpen} query={parentQuery} results={parentResults} choice={parentChoice} loading={parentLoading} onQuery={setParentQuery} onChoice={setParentChoice} onClose={()=>setParentOpen(false)} onConfirm={confirmParent}/>
    <AppDialogs modal={modal} assets={assets} currentId={current?.id} text={dialogText} setText={setDialogText} choice={dialogChoice} setChoice={setDialogChoice} onClose={()=>setModal(null)} onConfirm={confirmModal}/>
  </div>
}
