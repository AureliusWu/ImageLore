import { useEffect,useMemo,useState } from "react";
import type { AssetRecord,AssetSession,GenerationInfo,GenerationSession,ImagePromptAnalysis,Lineage,ReferenceSource,RemixDraft,RemixSource,VisualDna,VisualDnaPatch } from "../types";
import { ComparePanel } from "./ComparePanel";
import { RemixPanel } from "./RemixPanel";

export type InspectorTab="prompt"|"dna"|"remix"|"info"|"lineage";

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
const sourceLabel=(value:string)=>({"manual":"手动","sidecar":"Sidecar","ai":"AI 分析","mixed":"手动 + AI"} as Record<string,string>)[value]||"手动";

export function InspectorPane({asset,tab,onTab,references,onOpenReference,visualDna,onSaveVisualDna,imagePromptAnalysis,imagePromptLoading,visionModel,onAnalyzeImage,onApplyAnalysisDna,onOverwriteAnalysisDna,onUseAnalysisPrompt,onSaveAnalysisRevision,onOpenVisionSettings,remixDraft,remixSources,remixPrompt,onRemixPrompt,onAddRemixSource,onRemoveRemixSource,onToggleRemixField,onComposeRemix,onSaveRemix,onCopyRemix,onImportRemixResult,onResetRemix,prompt,onPrompt,negative,onNegative,model,onModel,tagsText,onTagsText,lineage,compareRecord,compareParentSrc,compareCurrentSrc,sessions,assetSession,onCompare,onCopy,onSaveRevision,onHistory,onFavorite,onFindSimilar,onRescan,onSidecar,onCollection,onRemove,onImportDerivative,onLinkParent,onSetSession,onCreateSession,onEditSessionNote,onEditRelationNote,promptRef}:{
  asset:AssetRecord|null;tab:InspectorTab;onTab:(t:InspectorTab)=>void;references:ReferenceSource[];onOpenReference:(url:string)=>void;visualDna:VisualDna|null;onSaveVisualDna:(value:VisualDnaPatch)=>void|Promise<void>;
  imagePromptAnalysis:ImagePromptAnalysis|null;imagePromptLoading:boolean;visionModel:string;
  onAnalyzeImage:()=>void;onApplyAnalysisDna:()=>void;onOverwriteAnalysisDna:()=>void;onUseAnalysisPrompt:()=>void;onSaveAnalysisRevision:()=>void;onOpenVisionSettings:()=>void;
  remixDraft:RemixDraft|null;remixSources:RemixSource[];remixPrompt:string;onRemixPrompt:(value:string)=>void;
  onAddRemixSource:()=>void;onRemoveRemixSource:(assetId:number)=>void;onToggleRemixField:(assetId:number,field:string)=>void;
  onComposeRemix:()=>void;onSaveRemix:()=>void;onCopyRemix:()=>void;onImportRemixResult:()=>void;onResetRemix:()=>void;
  prompt:string;onPrompt:(v:string)=>void;negative:string;onNegative:(v:string)=>void;model:string;onModel:(v:string)=>void;tagsText:string;onTagsText:(v:string)=>void;
  lineage:Lineage;compareRecord:AssetRecord|null;compareParentSrc:string;compareCurrentSrc:string;sessions:GenerationSession[];assetSession:AssetSession|null;
  onCompare:(id:number)=>void;onCopy:()=>void;onSaveRevision:()=>void;onHistory:()=>void;onFavorite:()=>void;onFindSimilar:()=>void;onRescan:()=>void;onSidecar:()=>void;onCollection:()=>void;onRemove:()=>void;onImportDerivative:()=>void;onLinkParent:()=>void;
  onSetSession:(sessionId:number|null)=>void;onCreateSession:()=>void;onEditSessionNote:()=>void;onEditRelationNote:(relationId:number,currentNote:string)=>void;
  promptRef:React.RefObject<HTMLTextAreaElement|null>;
}){
  const generation=useMemo<GenerationInfo>(()=>{try{return asset?JSON.parse(asset.generation_json||"{}"):{} }catch{return{}}},[asset?.generation_json]);
  const[dnaDraft,setDnaDraft]=useState<VisualDnaPatch>(emptyDna);
  const promptSummary=(prompt||"").trim().replace(/\s+/g," ").slice(0,180);
  const dnaSummary=[
    ["主体",visualDna?.subject],["角色",visualDna?.character],["服装",visualDna?.outfit],["姿势",visualDna?.pose],
    ["构图",visualDna?.composition],["镜头",visualDna?.camera],["光线",visualDna?.lighting],["环境",visualDna?.environment],
    ["色彩",visualDna?.palette],["风格",visualDna?.style]
  ].filter(([,value])=>Boolean(value?.trim())).slice(0,6) as Array<[string,string]>;
  const aspect=asset?.width&&asset.height?`${asset.width}:${asset.height}`:"—";
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
    <nav className="inspector-tabs"><button className={tab==="prompt"?"active":""} onClick={()=>onTab("prompt")}>✎ <span>提示词</span></button><button className={tab==="dna"?"active":""} onClick={()=>onTab("dna")}>◇ <span>视觉 DNA</span></button><button className={tab==="remix"?"active":""} onClick={()=>onTab("remix")}>⧉ <span>Remix</span></button><button className={tab==="info"?"active":""} onClick={()=>onTab("info")}>ⓘ <span>信息</span></button><button className={tab==="lineage"?"active":""} onClick={()=>onTab("lineage")}>⑂ <span>谱系</span><b>{lineage.parents.length+lineage.children.length}</b></button></nav>
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
        <div className="image-prompt-card">
          <div className="image-prompt-head"><div><span className="eyebrow">IMAGE TO PROMPT</span><strong>从图片提取 Visual DNA 与可生成 Prompt</strong><p>只有点击分析时才会把缩小后的图片发送到你配置的 Vision Provider。</p></div><button className="button secondary" onClick={onOpenVisionSettings}>配置</button></div>
          <div className="image-prompt-status"><span>模型</span><strong>{visionModel||"未配置"}</strong><button className="button primary" disabled={!asset||imagePromptLoading||!!asset?.missing} onClick={onAnalyzeImage}>{imagePromptLoading?"分析中…":imagePromptAnalysis?"重新分析":"分析当前图片"}</button></div>
          {imagePromptAnalysis?<div className="image-prompt-result">
            <div className="analysis-meta"><span>{imagePromptAnalysis.provider}</span><b>{imagePromptAnalysis.model}</b><time>{new Date(imagePromptAnalysis.created_at*1000).toLocaleString("zh-CN")}</time></div>
            {imagePromptAnalysis.summary?<p className="analysis-summary">{imagePromptAnalysis.summary}</p>:null}
            <label><span>生成 Prompt</span><textarea readOnly value={imagePromptAnalysis.prompt} rows={5}/></label>
            <div className="analysis-actions">
              <button onClick={onApplyAnalysisDna}>补充 Visual DNA</button>
              <button onClick={onUseAnalysisPrompt}>载入 Prompt 编辑器</button>
              <button onClick={onSaveAnalysisRevision}>保存为 Revision</button>
              <button className="danger-compact" onClick={onOverwriteAnalysisDna}>覆盖 DNA</button>
            </div>
            <small>“补充 Visual DNA”只填写当前为空的字段；“覆盖 DNA”才会替换已有手工内容。</small>
          </div>:<div className="image-prompt-empty"><span>◇</span><p>{visionModel?"还没有分析记录。":"先配置一个支持图片输入的 OpenAI-compatible 模型。"}</p></div>}
        </div>
        <div className="prompt-card">
          <div className="prompt-card-head"><div><span className="eyebrow">PROMPT CARD</span><strong>{asset?.name||"未选择图片"}</strong></div><span className="dna-source">{sourceLabel(visualDna?.source||dnaDraft.source)}</span></div>
          <div className="prompt-card-meta"><div><span>MODEL</span><strong>{model||asset?.model||"—"}</strong></div><div><span>SIZE</span><strong>{asset?.width&&asset.height?`${asset.width}×${asset.height}`:"—"}</strong></div><div><span>RATIO</span><strong>{aspect}</strong></div><div><span>LINEAGE</span><strong>{lineage.parents.length+" ↑ / "+lineage.children.length+" ↓"}</strong></div></div>
          <div className="prompt-card-prompt"><span>PROMPT</span><p>{promptSummary||"当前没有 Prompt。"}</p></div>
          <div className="prompt-card-dna"><span>VISUAL DNA</span>{dnaSummary.length?<div>{dnaSummary.map(([label,value])=><span key={label}><b>{label}</b>{value}</span>)}</div>:<p>还没有结构化视觉信息。可以在下面手动记录；下一阶段会由 Image-to-Prompt 自动填充。</p>}</div>
        </div>
        <div className="dna-hero"><div><span className="eyebrow">VISUAL DNA</span><strong>把“这张图为什么像它”变成可搜索的结构</strong><p>这些字段会进入本地关键词索引，也会随 Sidecar 保存；用于半年后仍能理解这张图的主体、构图、光线和风格。</p></div></div>
        <div className="dna-grid">{dnaFields.map(([key,label,hint])=><label className="dna-field" key={key}><span>{label}<small>{hint}</small></span><textarea disabled={!asset} value={dnaDraft[key]} onChange={e=>updateDna(key,e.target.value)} rows={2} placeholder={"记录"+label+"…"}/></label>)}</div>
        <div className="dna-actions"><button disabled={!asset} onClick={clearDna}>清空草稿</button><button className="button primary" disabled={!asset} onClick={()=>void onSaveVisualDna(dnaDraft)}>保存 Visual DNA</button></div>
      </div>:null}
      {tab==="remix"?<RemixPanel asset={asset} draft={remixDraft} sources={remixSources} prompt={remixPrompt} onPrompt={onRemixPrompt} onAddSource={onAddRemixSource} onRemoveSource={onRemoveRemixSource} onToggleField={onToggleRemixField} onCompose={onComposeRemix} onSave={onSaveRemix} onCopy={onCopyRemix} onImportResult={onImportRemixResult} onReset={onResetRemix}/>:null}
      {tab==="info"?<div className="info-list"><div className="info-hero"><span>i</span><div><strong>可复现上下文</strong><p>保留原始生成参数、稳定 Portable ID 与可移植 Sidecar。</p></div></div>
        {references.length?<div className="reference-stack"><div className="reference-stack-head"><strong>Reference Card</strong><span>{references.length+" 个网页来源"}</span></div>{references.map(item=>{
          let intent="网页参考";try{if(JSON.parse(item.metadata_json||"{}").intent==="remix")intent="Remix 参考"}catch{}
          const primary=item.page_url||item.source_url;
          return <div className="reference-card" key={item.id}><div className="reference-card-head"><span>{intent}</span><time>{item.captured_at?new Date(item.captured_at*1000).toLocaleString("zh-CN"):""}</time></div><strong title={item.page_title||primary}>{item.page_title||"网页来源"}</strong><small title={primary}>{primary}</small><div><button disabled={!primary} onClick={()=>primary&&onOpenReference(primary)}>打开页面</button>{item.source_url&&item.source_url!==primary?<button onClick={()=>onOpenReference(item.source_url)}>打开原图</button>:null}</div></div>
        })}</div>:null}<InfoRow label="元数据来源" value={asset?.metadata_type}/><InfoRow label="模型" value={asset?.model}/><InfoRow label="尺寸" value={asset?.width&&asset.height?`${asset.width} × ${asset.height}`:"—"}/><InfoRow label="格式" value={asset?.format}/><InfoRow label="随机种子" value={generation.seed}/><InfoRow label="步数" value={generation.steps}/><InfoRow label="采样器" value={generation.sampler}/><InfoRow label="调度器" value={generation.scheduler}/><InfoRow label="CFG" value={generation.cfg_scale}/><InfoRow label="降噪强度" value={generation.denoise}/><InfoRow label="Portable ID" value={asset?.portable_id}/><InfoRow label="文件指纹" value={asset?.fingerprint?asset.fingerprint.slice(0,24)+"…":"—"}/><details><summary>原始生成 JSON</summary><pre>{asset?.generation_json||"{}"}</pre></details></div>:null}
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
