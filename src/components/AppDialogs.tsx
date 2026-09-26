import type { Dispatch,SetStateAction } from "react";
import type { AssetSummary,CollectionRecord,Revision } from "../types";
import { Modal } from "./Modal";

export type AppModalState=
  |{kind:"batch-tags"}
  |{kind:"history";revisions:Revision[]}
  |{kind:"collection";collections:CollectionRecord[]}
  |{kind:"link-parent"}
  |{kind:"remove"}
  |null;

export function AppDialogs({modal,assets,currentId,text,setText,choice,setChoice,onClose,onConfirm}:{
  modal:AppModalState;assets:AssetSummary[];currentId?:number;text:string;setText:Dispatch<SetStateAction<string>>;
  choice:string;setChoice:Dispatch<SetStateAction<string>>;onClose:()=>void;onConfirm:()=>void|Promise<void>;
}){
  if(!modal)return null;
  const title=modal.kind==="batch-tags"?"添加标签":modal.kind==="history"?"提示词历史":modal.kind==="collection"?"加入集合":modal.kind==="link-parent"?"关联父图":"移除记录";
  const description=modal.kind==="batch-tags"?"为所有已选记录添加相同标签。":modal.kind==="history"?"恢复一个已保存的提示词快照；恢复前会先保留当前内容。":modal.kind==="collection"?"把当前选择加入一个项目集合。":modal.kind==="link-parent"?"选择另一条记录作为来源或参考父图。":"仅移除 ImageLore 中的记录与元数据，原始图片仍保留在磁盘上。";
  const confirmLabel=modal.kind==="remove"?"移除记录":modal.kind==="history"?"恢复":"确认";

  return <Modal title={title} description={description} onClose={onClose} onConfirm={onConfirm} confirmLabel={confirmLabel} danger={modal.kind==="remove"}>
    {modal.kind==="batch-tags"?<label className="modal-field"><span>标签</span><input autoFocus value={text} onChange={e=>setText(e.target.value)} placeholder="角色, 卧室, 参考图"/></label>:null}
    {modal.kind==="history"?<div className="choice-list">{modal.revisions.length?modal.revisions.map(r=><label className={choice===String(r.id)?"selected":""} key={r.id}><input type="radio" name="revision" value={r.id} checked={choice===String(r.id)} onChange={e=>setChoice(e.target.value)}/><div><strong>{new Date(r.created_at*1000).toLocaleString("zh-CN")}</strong><span>{r.note||"提示词快照"}</span><p>{r.prompt.slice(0,150)||"（空提示词）"}</p></div></label>):<p className="muted">还没有保存过历史版本。</p>}</div>:null}
    {modal.kind==="collection"?<><div className="choice-grid">{modal.collections.map(c=><label className={choice===String(c.id)?"selected":""} key={c.id}><input type="radio" name="collection" value={c.id} checked={choice===String(c.id)} onChange={e=>setChoice(e.target.value)}/><div><strong>{c.name}</strong><span>{c.count} 条记录</span></div></label>)}<label className={choice==="new"?"selected":""}><input type="radio" name="collection" value="new" checked={choice==="new"} onChange={e=>setChoice(e.target.value)}/><div><strong>＋ 新建集合</strong><span>创建一个项目分组</span></div></label></div>{choice==="new"?<label className="modal-field"><span>名称</span><input autoFocus value={text} onChange={e=>setText(e.target.value)} placeholder="角色研究"/></label>:null}</>:null}
    {modal.kind==="link-parent"?<div className="choice-list">{assets.filter(x=>x.id!==currentId).slice(0,150).map(a=><label className={choice===String(a.id)?"selected":""} key={a.id}><input type="radio" name="parent" value={a.id} checked={choice===String(a.id)} onChange={e=>setChoice(e.target.value)}/><div><strong>{a.name}</strong><span>{a.metadata_type}</span></div></label>)}</div>:null}
    {modal.kind==="remove"?<div className="warning-card"><span>!</span><div><strong>原始图片不会被删除</strong><p>只会移除这条 ImageLore 记录、历史版本、关系和集合归属。</p></div></div>:null}
  </Modal>
}
