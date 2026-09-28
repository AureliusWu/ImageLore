import { useMemo,useState } from "react";
import type { BackupRecord,CollectionRecord,DuplicateGroup,FacetCount,LibraryHealth,ModelAlias,SavedFilter,SemanticProgress,SemanticStatus,SourceFolder } from "../types";

const bytes=(n:number)=>n<1024?n+" B":n<1024*1024?(n/1024).toFixed(1)+" KB":(n/1024/1024).toFixed(1)+" MB";

export function LibraryManager({open,backups,tags,collections,duplicates,modelAliases,savedFilters,sourceFolders,sourceSyncing,health,semanticStatus,semanticIndexing,semanticProgress,onRebuildSemantic,onCancelSemantic,onClearSemantic,onDeleteSemanticModels,onClose,onBackup,onRestore,onRenameTag,onDeleteTag,onRenameCollection,onDeleteCollection,onUpsertModelAlias,onDeleteModelAlias,onDeleteSavedFilter,onAddSourceFolder,onRemoveSourceFolder,onToggleSourceAutoSync,onSyncSourceFolders}:{
  open:boolean;backups:BackupRecord[];tags:FacetCount[];collections:CollectionRecord[];duplicates:DuplicateGroup[];modelAliases:ModelAlias[];savedFilters:SavedFilter[];sourceFolders:SourceFolder[];sourceSyncing:boolean;health:LibraryHealth|null;
  semanticStatus:SemanticStatus|null;semanticIndexing:boolean;semanticProgress:SemanticProgress;
  onRebuildSemantic:()=>void;onCancelSemantic:()=>void;onClearSemantic:()=>void;onDeleteSemanticModels:()=>void;
  onClose:()=>void;onBackup:()=>void;onRestore:(name:string)=>void;
  onRenameTag:(oldName:string,newName:string)=>void;onDeleteTag:(name:string)=>void;
  onRenameCollection:(id:number,name:string)=>void;onDeleteCollection:(id:number)=>void;
  onUpsertModelAlias:(alias:string,canonical:string)=>void;onDeleteModelAlias:(alias:string)=>void;onDeleteSavedFilter:(id:number)=>void;
  onAddSourceFolder:()=>void;onRemoveSourceFolder:(id:number)=>void;onToggleSourceAutoSync:(id:number,enabled:boolean)=>void;onSyncSourceFolders:(ids:number[])=>void;
}){
  const[tab,setTab]=useState<"safety"|"sources"|"tags"|"collections"|"intelligence"|"semantic"|"duplicates"|"health">("safety");
  const[tagSource,setTagSource]=useState("");const[tagTarget,setTagTarget]=useState("");
  const[alias,setAlias]=useState("");const[canonical,setCanonical]=useState("");
  const collectionMap=useMemo(()=>new Map(collections.map(x=>[x.id,x])),[collections]);
  if(!open)return null;
  return <div className="modal-backdrop"><section className="modal library-manager glass-surface" role="dialog" aria-modal="true">
    <div className="modal-head"><div><strong>资料库管理</strong><p>维护安全、命名规范、保存视图与资料库健康。</p></div><button className="icon-button" onClick={onClose}>×</button></div>
    <nav className="manager-tabs">
      <button className={tab==="safety"?"active":""} onClick={()=>setTab("safety")}>安全</button>
      <button className={tab==="sources"?"active":""} onClick={()=>setTab("sources")}>来源目录</button>
      <button className={tab==="tags"?"active":""} onClick={()=>setTab("tags")}>标签</button>
      <button className={tab==="collections"?"active":""} onClick={()=>setTab("collections")}>集合</button>
      <button className={tab==="intelligence"?"active":""} onClick={()=>setTab("intelligence")}>模型/视图</button>
      <button className={tab==="semantic"?"active":""} onClick={()=>setTab("semantic")}>AI 索引</button>
      <button className={tab==="duplicates"?"active":""} onClick={()=>setTab("duplicates")}>重复 {duplicates.length?"("+duplicates.length+")":""}</button>
      <button className={tab==="health"?"active":""} onClick={()=>setTab("health")}>健康</button>
    </nav>
    <div className="manager-body">
      {tab==="safety"?<section><div className="manager-toolbar"><div><strong>资料库备份</strong><span>最多保留最近 10 份；恢复会在下次启动时生效。</span></div><button className="button primary" onClick={onBackup}>立即备份</button></div>
        <div className="manager-list">{backups.length?backups.map(b=><div className="manager-row" key={b.name}><div><strong>{new Date(b.created_at*1000).toLocaleString("zh-CN")}</strong><span>{b.name+" · "+bytes(b.size)}</span></div><button onClick={()=>onRestore(b.name)}>恢复到此版本</button></div>):<p className="muted">还没有备份。ImageLore 每 24 小时会自动生成一份。</p>}</div></section>:null}
      {tab==="sources"?<section>
        <div className="manager-toolbar"><div><strong>资料源目录</strong><span>登记常用出图目录；自动同步只在 ImageLore 启动时扫描一次，不常驻监听系统。</span></div><div className="manager-toolbar-actions"><button className="button secondary" disabled={sourceSyncing||!sourceFolders.length} onClick={()=>onSyncSourceFolders(sourceFolders.map(x=>x.id))}>同步全部</button><button className="button primary" onClick={onAddSourceFolder}>＋ 添加目录</button></div></div>
        <div className="manager-list">{sourceFolders.length?sourceFolders.map(source=><div className="manager-row source-row" key={source.id}>
          <div><strong>{source.name}</strong><span title={source.path}>{source.path}</span><small>{source.last_scan_at?"上次扫描："+new Date(source.last_scan_at*1000).toLocaleString("zh-CN"):"尚未扫描"}</small></div>
          <label className="source-auto"><input type="checkbox" checked={source.auto_sync} onChange={e=>onToggleSourceAutoSync(source.id,e.target.checked)}/><span>启动时同步</span></label>
          <button disabled={sourceSyncing} onClick={()=>onSyncSourceFolders([source.id])}>同步</button>
          <button className="danger-compact" disabled={sourceSyncing} onClick={()=>onRemoveSourceFolder(source.id)}>移除</button>
        </div>):<div className="source-empty"><strong>还没有资料源目录</strong><p>把 ComfyUI、Stable Diffusion、NovelAI 导出目录或你的整理目录加入这里，以后无需反复选择文件夹。</p><button className="button primary" onClick={onAddSourceFolder}>＋ 添加第一个目录</button></div>}</div>
      </section>:null}
      {tab==="tags"?<section><div className="manager-toolbar"><div><strong>标签治理</strong><span>重命名到已有标签会自动合并。</span></div></div>
        <div className="manager-inline"><select value={tagSource} onChange={e=>setTagSource(e.target.value)}><option value="">选择标签</option>{tags.map(t=><option key={t.name} value={t.name}>{t.name+" ("+t.count+")"}</option>)}</select><input value={tagTarget} onChange={e=>setTagTarget(e.target.value)} placeholder="新名称或目标标签"/><button disabled={!tagSource||!tagTarget.trim()} onClick={()=>onRenameTag(tagSource,tagTarget)}>重命名/合并</button><button className="danger-compact" disabled={!tagSource} onClick={()=>onDeleteTag(tagSource)}>删除</button></div>
        <div className="chip-cloud">{tags.map(t=><span key={t.name}>{t.name}<b>{t.count}</b></span>)}</div></section>:null}
      {tab==="collections"?<section><div className="manager-toolbar"><div><strong>集合管理</strong><span>集合删除不会删除原始图片或记录。</span></div></div><div className="manager-list">{collections.map(c=><div className="manager-row" key={c.id}><div><strong>{c.name}</strong><span>{c.count+" 条记录"}</span></div><button onClick={()=>{const name=window.prompt("新的集合名称",collectionMap.get(c.id)?.name||"");if(name?.trim())onRenameCollection(c.id,name.trim())}}>重命名</button><button className="danger-compact" onClick={()=>onDeleteCollection(c.id)}>删除</button></div>)}</div></section>:null}
      {tab==="intelligence"?<section>
        <div className="manager-toolbar"><div><strong>模型别名</strong><span>保留原始模型名，同时让筛选器使用统一规范名称。</span></div></div>
        <div className="model-alias-form"><input value={alias} onChange={e=>setAlias(e.target.value)} placeholder="原始模型名 / 文件名"/><input value={canonical} onChange={e=>setCanonical(e.target.value)} placeholder="规范名称"/><button disabled={!alias.trim()||!canonical.trim()} onClick={()=>{onUpsertModelAlias(alias,canonical);setAlias("");setCanonical("")}}>保存别名</button></div>
        <div className="manager-list compact">{modelAliases.map(x=><div className="manager-row" key={x.alias}><div><strong>{x.canonical}</strong><span>{x.alias}</span></div><button className="danger-compact" onClick={()=>onDeleteModelAlias(x.alias)}>删除</button></div>)}</div>
        <div className="manager-toolbar section-gap"><div><strong>保存视图</strong><span>保存当前搜索、标签、模型和集合条件，随时恢复。</span></div></div>
        <div className="manager-list compact">{savedFilters.length?savedFilters.map(x=><div className="manager-row" key={x.id}><div><strong>{x.name}</strong><span>{[x.filter.query,x.filter.tag,x.filter.model].filter(Boolean).join(" · ")||"无文本条件"}</span></div><button className="danger-compact" onClick={()=>onDeleteSavedFilter(x.id)}>删除</button></div>):<p className="muted">还没有保存视图，可在左侧筛选器中保存。</p>}</div>
      </section>:null}
      {tab==="semantic"?<section>
        <div className="manager-toolbar"><div><strong>Semantic Recall</strong><span>本地 CLIP 图文向量索引。原图不会上传；Embedding 可删除、可重建。</span></div><div className="manager-toolbar-actions">{semanticIndexing?<button className="button secondary" onClick={onCancelSemantic}>取消索引</button>:<button className="button primary" onClick={onRebuildSemantic}>{semanticStatus?.indexed?"更新索引":"启用并建立索引"}</button>}</div></div>
        {semanticStatus?<>
          <div className="semantic-status-grid">
            <div><strong>{semanticStatus.indexed}</strong><span>已索引</span></div>
            <div><strong>{semanticStatus.total}</strong><span>可索引图片</span></div>
            <div><strong>{semanticStatus.stale}</strong><span>待增量更新</span></div>
            <div><strong>{bytes(semanticStatus.index_bytes)}</strong><span>向量索引</span></div>
            <div><strong>{bytes(semanticStatus.model_bytes)}</strong><span>模型缓存</span></div>
            <div><strong>{semanticStatus.model_ready?"就绪":semanticStatus.vision_ready?"视觉已就绪":"未完整下载"}</strong><span>模型状态</span></div>
          </div>
          <div className="semantic-model-card">
            <div><strong>CLIP ViT-B/32</strong><span>{semanticStatus.model_id}</span><small>视觉编码器 + 文本编码器共享 512 维空间；首次使用会下载本地模型，之后可离线运行。</small></div>
            <div className="semantic-model-flags"><span className={semanticStatus.vision_ready?"ready":""}>视觉 {semanticStatus.vision_ready?"✓":"—"}</span><span className={semanticStatus.text_ready?"ready":""}>文本 {semanticStatus.text_ready?"✓":"—"}</span></div>
          </div>
          {semanticIndexing?<div className="semantic-progress-card"><div><strong>正在建立索引</strong><span>{semanticProgress.processed+" / "+semanticProgress.total}</span></div><progress max={Math.max(1,semanticProgress.total)} value={semanticProgress.processed}/><small>{semanticProgress.current_name||"正在收尾…"}</small></div>:null}
          <div className="semantic-actions"><button disabled={semanticIndexing||!semanticStatus.indexed} onClick={onClearSemantic}>清空索引</button><button className="danger-compact" disabled={semanticIndexing||!semanticStatus.model_bytes} onClick={onDeleteSemanticModels}>删除模型缓存</button></div>
        </>:<p className="muted">正在读取语义索引状态…</p>}
      </section>:null}
      {tab==="duplicates"?<section><div className="manager-toolbar"><div><strong>重复内容报告</strong><span>新导入的完全相同文件会按 SHA-256 自动跳过。</span></div></div><div className="manager-list">{duplicates.length?duplicates.map(g=><div className="duplicate-row" key={g.fingerprint}><strong>{g.count+" 个相同内容"}</strong><span>{g.names.join(" · ")}</span><small>{g.fingerprint.slice(0,24)+"…"}</small></div>):<p className="muted">当前资料库没有发现重复内容。</p>}</div></section>:null}
      {tab==="health"?<section><div className="manager-toolbar"><div><strong>Library Health</strong><span>快速发现会影响可追溯性和长期维护的问题。</span></div></div>
        {health?<div className="health-grid">
          <div><strong>{health.total}</strong><span>总记录</span></div><div><strong>{health.missing}</strong><span>缺失文件</span></div><div><strong>{health.without_metadata}</strong><span>无生成元数据</span></div><div><strong>{health.without_fingerprint}</strong><span>无指纹</span></div><div><strong>{health.pending_relations}</strong><span>待恢复谱系</span></div><div><strong>{health.unassigned_session}</strong><span>未归会话</span></div><div><strong>{health.duplicate_groups}</strong><span>重复组</span></div><div><strong>{bytes(health.cache_bytes)}</strong><span>缓存占用</span></div>
        </div>:<p className="muted">正在读取资料库健康状态…</p>}
      </section>:null}
    </div>
    <div className="modal-actions"><button className="button secondary" onClick={onClose}>关闭</button></div>
  </section></div>
}
