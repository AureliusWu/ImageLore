import type { AssetRecord,RemixDraft,RemixSource } from "../types";
import { REMIX_DNA_FIELDS,remixFieldLabel } from "../remixWorkflow";

export function RemixPanel({asset,draft,sources,prompt,onPrompt,onAddSource,onRemoveSource,onToggleField,onCompose,onSave,onCopy,onImportResult,onReset}:{
  asset:AssetRecord|null;draft:RemixDraft|null;sources:RemixSource[];prompt:string;
  onPrompt:(value:string)=>void;onAddSource:()=>void;onRemoveSource:(assetId:number)=>void;
  onToggleField:(assetId:number,field:string)=>void;onCompose:()=>void;onSave:()=>void;
  onCopy:()=>void;onImportResult:()=>void;onReset:()=>void;
}){
  return <div className="remix-panel">
    <div className="remix-hero"><div><span className="eyebrow">REMIX WORKSPACE</span><strong>从多张参考图借用可复用的视觉片段</strong><p>草稿独立保存，不修改原图。导入生成结果时才把来源和实际使用的 DNA 字段写进 Generation Lineage。</p></div><button className="button primary" disabled={!asset} onClick={onAddSource}>＋ 添加参考图</button></div>
    <div className="remix-sources">
      {sources.map(source=>{
        const available=REMIX_DNA_FIELDS.filter(([key])=>String(source.visual_dna[key]||"").trim());
        const isBase=source.asset_id===asset?.id;
        return <article className="remix-source-card" key={source.asset_id}>
          <header><div><span>{isBase?"基础图片":"参考图片"}</span><strong title={source.asset_name}>{source.asset_name}</strong></div>{!isBase?<button onClick={()=>onRemoveSource(source.asset_id)}>移除</button>:null}</header>
          {available.length?<div className="remix-field-chips">{available.map(([key])=><label className={source.fields.includes(key)?"selected":""} key={key}><input type="checkbox" checked={source.fields.includes(key)} onChange={()=>onToggleField(source.asset_id,key)}/><span>{remixFieldLabel(key)}</span><small title={String(source.visual_dna[key])}>{String(source.visual_dna[key])}</small></label>)}</div>:<p className="muted">这张图片还没有 Visual DNA。可以先分析图片或手动填写。</p>}
        </article>
      })}
    </div>
    <div className="remix-compose-head"><div><strong>Remix Prompt 草稿</strong><span>{draft?"已保存 · "+new Date(draft.updated_at*1000).toLocaleString("zh-CN"):"尚未保存"}</span></div><button onClick={onCompose}>按所选 DNA 重新组合</button></div>
    <textarea className="remix-prompt" disabled={!asset} value={prompt} onChange={e=>onPrompt(e.target.value)} placeholder="组合后的 Prompt 会出现在这里，也可以继续手工修改…" spellCheck={false}/>
    <div className="remix-actions">
      <button disabled={!asset||!prompt.trim()} onClick={onCopy}>复制 Prompt</button>
      <button disabled={!asset||!sources.length} onClick={onSave}>保存草稿</button>
      <button className="button primary" disabled={!draft||!prompt.trim()} onClick={onImportResult}>导入 Remix 结果</button>
      <button className="danger-compact" disabled={!asset} onClick={onReset}>重置</button>
    </div>
    <p className="remix-note">“导入 Remix 结果”不会删除或覆盖参考图；若结果图片本身没有 Prompt 元数据，ImageLore 才会用本草稿 Prompt 补充记录。</p>
  </div>
}
