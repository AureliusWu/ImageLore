import { useMemo } from "react";
import type { AssetRecord, GenerationInfo, Lineage } from "../types";
import { ComparePanel } from "./ComparePanel";

export type InspectorTab="prompt"|"info"|"lineage";

function InfoRow({label,value}:{label:string;value:unknown}){return <div className="info-row"><span>{label}</span><strong>{value===undefined||value===null||value===""?"—":String(value)}</strong></div>}
const relationLabel=(value:string)=>({"derived_from":"派生","variation":"变体","edit":"编辑","upscale":"放大","reference":"参考"} as Record<string,string>)[value]||value;

export function InspectorPane({asset,tab,onTab,prompt,onPrompt,negative,onNegative,model,onModel,tagsText,onTagsText,lineage,compareRecord,onCompare,onCopy,onSaveRevision,onHistory,onFavorite,onRescan,onSidecar,onCollection,onRemove,onImportDerivative,onLinkParent,promptRef}:{
  asset:AssetRecord|null;tab:InspectorTab;onTab:(t:InspectorTab)=>void;prompt:string;onPrompt:(v:string)=>void;negative:string;onNegative:(v:string)=>void;model:string;onModel:(v:string)=>void;tagsText:string;onTagsText:(v:string)=>void;
  lineage:Lineage;compareRecord:AssetRecord|null;onCompare:(id:number)=>void;onCopy:()=>void;onSaveRevision:()=>void;onHistory:()=>void;onFavorite:()=>void;onRescan:()=>void;onSidecar:()=>void;onCollection:()=>void;onRemove:()=>void;onImportDerivative:()=>void;onLinkParent:()=>void;promptRef:React.RefObject<HTMLTextAreaElement|null>;
}){
  const generation=useMemo<GenerationInfo>(()=>{try{return asset?JSON.parse(asset.generation_json||"{}"):{} }catch{return{}}},[asset?.generation_json]);
  return <aside className="inspector-pane panel glass-surface">
    <div className="record-head"><div><span className="eyebrow">生成记录</span><strong>{asset?.name||"提示词与上下文"}</strong></div><button className={`favorite-button ${asset?.favorite?"on":""}`} disabled={!asset} onClick={onFavorite} title="收藏">{asset?.favorite?"★":"☆"}</button></div>
    <nav className="inspector-tabs"><button className={tab==="prompt"?"active":""} onClick={()=>onTab("prompt")}>✎ <span>提示词</span></button><button className={tab==="info"?"active":""} onClick={()=>onTab("info")}>ⓘ <span>生成信息</span></button><button className={tab==="lineage"?"active":""} onClick={()=>onTab("lineage")}>⑂ <span>谱系</span><b>{lineage.parents.length+lineage.children.length}</b></button></nav>
    <div className="inspector-body">
      {tab==="prompt"?<>
        <div className="field-head"><div><label>提示词</label><span>主要生成指令</span></div><div className="field-actions"><button disabled={!asset} onClick={onCopy}>复制</button><button disabled={!asset} onClick={onHistory}>历史</button><button className="save-button" disabled={!asset} onClick={onSaveRevision}>保存版本</button></div></div>
        <textarea ref={promptRef} className="prompt-box" disabled={!asset} value={prompt} onChange={e=>onPrompt(e.target.value)} placeholder="在这里输入或粘贴生成提示词…" spellCheck={false}/>
        <label className="field"><span>反向提示词 <small>排除内容</small></span><textarea className="small-area" disabled={!asset} value={negative} onChange={e=>onNegative(e.target.value)} placeholder="可选，用于描述不希望出现的内容" spellCheck={false}/></label>
        <div className="two-col"><label className="field"><span>标签 <small>使用逗号分隔</small></span><input disabled={!asset} value={tagsText} onChange={e=>onTagsText(e.target.value)} placeholder="角色, 卧室, 研究"/></label><label className="field"><span>模型 <small>生成来源</small></span><input disabled={!asset} value={model} onChange={e=>onModel(e.target.value)} placeholder="GPT Image / Flux…"/></label></div>
        {asset?.tags.length?<div className="tag-preview">{asset.tags.map(tag=><span key={tag}>{tag}</span>)}</div>:null}
        <div className="tool-card"><div><strong>记录工具</strong><span>低频操作集中放在这里，保持编辑区清爽。</span></div><div><button disabled={!asset} onClick={onRescan}>重新读取元数据</button><button disabled={!asset} onClick={onSidecar}>导出 Sidecar</button><button disabled={!asset} onClick={onCollection}>加入集合</button></div></div>
        <button className="danger-link" disabled={!asset} onClick={onRemove}>从 ImageLore 中移除这条记录</button>
      </>:null}
      {tab==="info"?<div className="info-list"><div className="info-hero"><span>i</span><div><strong>可复现上下文</strong><p>在不复制原始图片的前提下，把图片元数据和 ImageLore 记录保存在一起。</p></div></div><InfoRow label="元数据来源" value={asset?.metadata_type}/><InfoRow label="模型" value={asset?.model}/><InfoRow label="尺寸" value={asset?.width&&asset.height?`${asset.width} × ${asset.height}`:"—"}/><InfoRow label="格式" value={asset?.format}/><InfoRow label="随机种子" value={generation.seed}/><InfoRow label="步数" value={generation.steps}/><InfoRow label="采样器" value={generation.sampler}/><InfoRow label="CFG" value={generation.cfg_scale}/><InfoRow label="文件指纹" value={asset?.fingerprint?asset.fingerprint.slice(0,24)+"…":"—"}/><details><summary>原始生成 JSON</summary><pre>{asset?.generation_json||"{}"}</pre></details></div>:null}
      {tab==="lineage"?<div className="lineage-panel"><div className="lineage-hero"><span>⑂</span><div><strong>生成谱系</strong><p>明确记录参考、编辑、变体与派生关系。</p></div></div><div className="lineage-actions"><button className="button primary" disabled={!asset} onClick={onImportDerivative}>＋ 导入派生图</button><button className="button secondary" disabled={!asset} onClick={onLinkParent}>关联父图</button></div><h4>父级 <b>{lineage.parents.length}</b></h4>{lineage.parents.length?lineage.parents.map(x=><button className={`edge-card ${compareRecord?.id===x.other_id?"active":""}`} key={x.id} onClick={()=>onCompare(x.other_id)}><span>{relationLabel(x.relation_type)}</span><strong>{x.other_name}</strong><small>{x.note||"点击进行对比"}</small></button>):<p className="muted">没有父级记录。这条记录可以作为谱系根节点。</p>}{asset&&compareRecord?<ComparePanel parent={compareRecord} current={asset}/>:null}<h4>子级 <b>{lineage.children.length}</b></h4>{lineage.children.length?lineage.children.map(x=><div className="edge-card child" key={x.id}><span>{relationLabel(x.relation_type)}</span><strong>{x.other_name}</strong><small>{x.note}</small></div>):<p className="muted">还没有派生记录。</p>}</div>:null}
    </div>
  </aside>
}
