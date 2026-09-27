import type { AssetSummary } from "../types";

export function ParentPicker({open,query,results,choice,loading,onQuery,onChoice,onClose,onConfirm}:{
  open:boolean;query:string;results:AssetSummary[];choice:number|null;loading:boolean;
  onQuery:(value:string)=>void;onChoice:(id:number)=>void;onClose:()=>void;onConfirm:()=>void;
}){
  if(!open)return null;
  return <div className="modal-backdrop"><section className="modal parent-picker glass-surface" role="dialog" aria-modal="true">
    <div className="modal-head"><div><strong>关联父图</strong><p>搜索整个 ImageLore 资料库，选择来源或参考父图。</p></div><button className="icon-button" onClick={onClose}>×</button></div>
    <div className="modal-body"><label className="parent-search"><span>搜索</span><input autoFocus value={query} onChange={e=>onQuery(e.target.value)} placeholder="文件名、提示词、标签或模型"/></label>
      <div className="parent-results">{loading?<p className="muted">正在搜索…</p>:results.map(a=><button key={a.id} className={choice===a.id?"selected":""} onClick={()=>onChoice(a.id)}><strong>{a.name}</strong><span>{a.metadata_type+(a.width&&a.height?" · "+a.width+"×"+a.height:"")}</span></button>)}</div>
    </div>
    <div className="modal-actions"><button className="button secondary" onClick={onClose}>取消</button><button className="button primary" disabled={!choice} onClick={onConfirm}>建立关联</button></div>
  </section></div>
}
