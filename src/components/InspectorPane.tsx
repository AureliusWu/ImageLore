import { useEffect,useMemo,useState } from "react";
import type { AssetRecord,AssetSession,GenerationInfo,GenerationSession,Lineage,VisualDna,VisualDnaPatch } from "../types";
import { ComparePanel } from "./ComparePanel";

export type InspectorTab="prompt"|"dna"|"info"|"lineage";

function InfoRow({label,value}:{label:string;value:unknown}){return <div className="info-row"><span>{label}</span><strong>{value===undefined||value===null||value===""?"—":String(value)}</strong></div>}
const relationLabel=(value:string)=>({"derived_from":"派生","variation":"变体","edit":"编辑","upscale":"放大","reference":"参考"} as Record<string,string>)[value]||value;
const emptyDna:VisualDnaPatch={subject:"",character:"",outfit:"",pose:"",expression:"",composition:"",camera:"",lighting:"",environment:"",palette:"",material:"",style:"",source:"manual"};
const dnaFields:Array<[keyof Omit<VisualDnaPatch,"source">,string,string]>=[
  ["subject","主体","人物、物体或画面核心"],
  ["character","角色","角色身份、物种或固定设定"],
  ["outfit","服装","服装结构、配饰与造型"],
  ["pose","姿势","身体姿态与动作"],
  ["expression","表情","眼神、嘴部与整体情绪"],
  ["composition","构图","景别、视角与画面组织"],
  ["camera","镜头","机位、焦段、景深与摄影语言"],
  ["lighting","光线","光源、方向、软硬与时间感"],
  ["environment","环境","场景、地点与背景元素"],
  ["palette","色彩","主色、色温与调色倾向"],
  ["material","材质","皮肤、布料、金属、玻璃等"],
  ["style","风格","摄影、动漫、3D、年代或媒介语言"],
];
const sourceLabel=(value:string)=>({"manual":"手动","sidecar":"Sidecar","ai":"AI 分析"} as Record<string,string>)[value]||"手动";

export function InspectorPane({asset,tab,onTab,visualDna,onSaveVisualDna,prompt,onPrompt,negative,onNegative,model,onModel,tagsText,onTagsText,lineage,compareRecord,compareParentSrc,compareCurrentSrc,sessions,assetSession,onCompare,onCopy,onSaveRevision,onHistory,onFavorite,onFindSimilar,onRescan,onSidecar,onCollection,onRemove,onImportDerivative,onLinkParent,onSetSession,onCreateSession,onEditSessionNote,onEditRelationNote,promptRef}:{
  asset:AssetRecord|null;tab:InspectorTab;onTab:(t:InspectorTab)=>void;visualDna:VisualDna|null;onSaveVisualDna:(value:VisualDnaPatch)=>void|Promise<void>;
  prompt:string;onPrompt:(v:string)=>void;negative:string;onNegative:(v:string)=>void;model:string;onModel:(v:string)=>void;tagsText:string;onTagsText:(v:string)=>void;
  lineage:Lineage;compareRecord:AssetRecord|null;compareParentSrc:string;compareCurrentSrc:string;sessions:GenerationSession[];assetSession:AssetSession|null;
  onCompare:(id:number)=>void;onCopy:()=>void;onSaveRevision:()=>void;onHistory:()=>void;onFavorite:()=>void;onFindSimilar:()=>void;onRescan:()=>void;onSidecar:()=>void;onCollection:()=>void;onRemove:()=>void;onImportDerivative:()=>void;onLinkParent:()=>void;
  onSetSession:(sessionId:number|null)=>void;onCreateSession:()=>void;onEditSessionNote:()=>void;onEditRelationNote:(relationId:number,currentNote:string)=>void;
  promptRef:React.RefObject<HTMLTextAreaElement|null>;
}){
  const generation=useMemo<GenerationInfo>(()=>{try{return asset?JSON.parse(asset.generation_json||"{}"):{} }catch{return{}}},[asset?.generation_json]);
  const[dnaDraft,setDnaDraft]=useState<VisualDnaPatch>(emptyDna);
  useEffect(()=>{
    setDnaDraft(visualDna?{
      subject:visualDna.subject,character:visualDna.character,outfit:visualDna.outfit,pose:visualDna.pose,
      expression:visualDna.expression,composition:visualDna.composition,camera:visualDna.camera,lighting:visualDna.lighting,
      environment:visualDna.environment,palette:visualDna.palette,material:visualDna.material,style:visualDna.style,source:visualDna.source||"manual"
    }:emptyDna);
  },[asset?.id,visualDna?.updated_at]);
  const updateDna=(key:keyof Omit<VisualDnaPatch,"source">,value:string)=>setDnaDraft(prev=>({...prev,[key]:value,source:"manual"}));
  const clearDna=()=>setDnaDraft({...emptyDna});
  return <aside className="inspector-pane panel glass-surface">
    <div className="record-head"><div><span className="eyebrow">生成记录</span><strong title={asset?.name||"提示词与上下文"}>{asset?.name||"提示词与上下文"}</strong></div><button className={`favorite-button ${asset?.favorite?"on":""}`} disabled={!asset} onClick={onFavorite} title="收藏">{asset?.favorite?"★":"☆"}</button></div>
    <nav className="inspector-tabs"><button className={tab==="prompt"?"active":""} onClick={()=>onTab("prompt")}>✎ <span>提示词</span></button><button className={tab==="dna"?"active":""} onClick={()=>onTab("dna")}>◇ <span>视觉 DNA</span></button><button className={tab==="info"?"active":""} onClick={()=>onTab("info")}>ⓘ <span>生成信息</span></button><button className={tab==="lineage"?"active":""} onClick={()=>onTab("lineage")}>⑂ <span>谱系</span><b>{lineage.parents.length+lineage.children.length}</b></button></nav>
    <div className="inspector-body">
      {tab==="prompt"?<>
        <div className="field-head"><div><label>提示词</label><span>主要生成指令</span></div><div className="field-actions"><button disabled={!asset} onClick={onCopy}>复制</button><button disabled={!asset} onClick={onHistory}>历史</button><button className="save-button" disabled={!asset} onClick={onSaveRevision}>保存版本</button></div></div>
        <textarea ref={promptRef} className="prompt-box" disabled={!asset} value={prompt} onChange={e=>onPrompt(e.target.value)} placeholder="在这里输入或粘贴生成提示词…" spellCheck={false}/>
        <label className="field"><span>反向提示词 <small>排除内容</small></span><textarea className="small-area" disabled={!asset} value={negative} onChange={e=>onNegative(e.target.value)} placeholder="可选，用于描述不希望出现的内容" spellCheck={false}/></label>
        <div className="two-col"><label className="field"><span>标签 <small>使用逗号分隔</small></span><input disabled={!asset} value={tagsText} onChange={e=>onTagsText(e.target.value)} placeholder="角色, 卧室, 研究"/></label><label className="field"><span>模型 <small>生成来源</small></span><input disabled={!asset} value={model} onChange={e=>onModel(e.target.value)} placeholder="GPT Image / Flux…"/></label></div>
        {asset?.tags.length?<div className="tag-preview">{asset.tags.map(tag=><span key={tag}>{tag}</span>)}</div>:null}
        <div className="tool-card"><div><strong>记录工具</strong><span>低频操作集中放在这里，保持编辑区清爽。</span></div><div><button disabled={!asset} onClick={onFindSimilar}>◎ 查找相似图片</button><button disabled={!asset} onClick={onRescan}>重新读取元数据</button><button disabled={!asset} onClick={onSidecar}>导出 Sidecar v3</button><button disabled={!asset} onClick={onCollection}>加入集合</button></div></div>
        <button className="danger-link" disabled={!asset} onClick={onRemove}>从 ImageLore 中移除这条记录</button>
      </>:null}
      {tab==="dna"?<div className="dna-panel">
        <div className="dna-hero"><div><span className="eyebrow">VISUAL DNA</span><strong>把“这张图为什么像它”变成可搜索的结构</strong><p>这些字段会进入本地关键词索引，也会随 Sidecar 保存。下一阶段的 Image-to-Prompt 会自动填充这里。</p></div><span className="dna-source">{sourceLabel(visualDna?.source||dnaDraft.source)}</span></div>
        <div className="dna-grid">{dnaFields.map(([key,label,hint])=><label className="dna-field" key={key}><span>{label}<small>{hint}</small></span><textarea disabled={!asset} value={dnaDraft[key]} onChange={e=>updateDna(key,e.target.value)} rows={2} placeholder={"记录"+label+"…"}/></label>)}</div>
        <div className="dna-actions"><button disabled={!asset} onClick={clearDna}>清空草稿</button><button className="button primary" disabled={!asset} onClick={()=>void onSaveVisualDna(dnaDraft)}>保存 Visual DNA</button></div>
      </div>:null}
      {tab==="info"?<div className="info-list"><div className="info-hero"><span>i</span><div><strong>可复现上下文</strong><p>保留原始生成参数、稳定 Portable ID 与可移植 Sidecar。</p></div></div><InfoRow label="元数据来源" value={asset?.metadata_type}/><InfoRow label="模型" value={asset?.model}/><InfoRow label="尺寸" value={asset?.width&&asset.height?`${asset.width} × ${asset.height}`:"—"}/><InfoRow label="格式" value={asset?.format}/><InfoRow label="随机种子" value={generation.seed}/><InfoRow label="步数" value={generation.steps}/><InfoRow label="采样器" value={generation.sampler}/><InfoRow label="调度器" value={generation.scheduler}/><InfoRow label="CFG" value={generation.cfg_scale}/><InfoRow label="降噪强度" value={generation.denoise}/><InfoRow label="Portable ID" value={asset?.portable_id}/><InfoRow label="文件指纹" value={asset?.fingerprint?asset.fingerprint.slice(0,24)+"…":"—"}/><details><summary>原始生成 JSON</summary><pre>{asset?.generation_json||"{}"}</pre></details></div>:null}
      {tab==="lineage"?<div className="lineage-panel">
        <div className="lineage-hero"><span>⑂</span><div><strong>生成谱系</strong><p>记录生成会话、参考、编辑、变体和分支备注。</p></div></div>
        <div className="session-card">
          <div><strong>Generation Session</strong><small>{assetSession?.asset_note||assetSession?.session_note||"把同一轮实验归入同一会话。"}</small></div>
          <select disabled={!asset} value={assetSession?.session_id??""} onChange={e=>onSetSession(e.target.value?Number(e.target.value):null)}><option value="">未分配</option>{sessions.map(s=><option key={s.id} value={s.id}>{s.name} ({s.count})</option>)}</select>
          <button disabled={!asset} onClick={onCreateSession}>＋ 新建</button><button disabled={!assetSession} onClick={onEditSessionNote}>备注</button>
        </div>
        <div className="lineage-actions"><button className="button primary" disabled={!asset} onClick={onImportDerivative}>＋ 导入派生图</button><button className="button secondary" disabled={!asset} onClick={onLinkParent}>关联父图</button></div>
        <h4>父级 <b>{lineage.parents.length}</b></h4>
        {lineage.parents.length?lineage.parents.map(x=><div className={`edge-card ${compareRecord?.id===x.other_id?"active":""}`} key={x.id}><button className="edge-main" onClick={()=>onCompare(x.other_id)}><span>{relationLabel(x.relation_type)}</span><strong>{x.other_name}</strong><small>{x.note||"点击进行对比"}</small></button><button className="edge-note" onClick={()=>onEditRelationNote(x.id,x.note)}>备注</button></div>):<p className="muted">没有父级记录。这条记录可以作为谱系根节点。</p>}
        {asset&&compareRecord?<ComparePanel parent={compareRecord} current={asset} parentSrc={compareParentSrc} currentSrc={compareCurrentSrc}/>:null}
        <h4>子级 <b>{lineage.children.length}</b></h4>
        {lineage.children.length?lineage.children.map(x=><div className="edge-card child" key={x.id}><div className="edge-main"><span>{relationLabel(x.relation_type)}</span><strong>{x.other_name}</strong><small>{x.note||"暂无分支备注"}</small></div><button className="edge-note" onClick={()=>onEditRelationNote(x.id,x.note)}>备注</button></div>):<p className="muted">还没有派生记录。</p>}
      </div>:null}
    </div>
  </aside>
}
