import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "../api";
import type { AssetRecord } from "../types";

type Draft={assetId:number|null;prompt:string;negative:string;model:string;tagsText:string};
const empty:Draft={assetId:null,prompt:"",negative:"",model:"",tagsText:""};
const parseTags=(text:string)=>[...new Set(text.split(/[,，]/).map(x=>x.trim()).filter(Boolean))];
const sameTags=(a:string[],b:string[])=>JSON.stringify(a)===JSON.stringify(b);

export function useEditorDraft(
  asset:AssetRecord|null,
  onSaved:(asset:AssetRecord)=>void,
  onTagsSaved:()=>void,
  setStatus:(text:string)=>void,
){
  const[prompt,setPromptState]=useState("");
  const[negative,setNegativeState]=useState("");
  const[model,setModelState]=useState("");
  const[tagsText,setTagsTextState]=useState("");
  const draft=useRef<Draft>({...empty});
  const baseline=useRef<Draft>({...empty});
  const timer=useRef<number|undefined>(undefined);
  const saving=useRef<Promise<void>>(Promise.resolve());

  const hydrate=useCallback((next:AssetRecord|null)=>{
    const value:Draft=next?{assetId:next.id,prompt:next.prompt,negative:next.negative_prompt,model:next.model,tagsText:next.tags.join(", ")}:{...empty};
    draft.current=value;baseline.current=value;
    setPromptState(value.prompt);setNegativeState(value.negative);setModelState(value.model);setTagsTextState(value.tagsText);
  },[]);

  useEffect(()=>{hydrate(asset)},[asset?.id]); // eslint-disable-line react-hooks/exhaustive-deps

  const setPrompt=(v:string)=>{draft.current.prompt=v;setPromptState(v)};
  const setNegative=(v:string)=>{draft.current.negative=v;setNegativeState(v)};
  const setModel=(v:string)=>{draft.current.model=v;setModelState(v)};
  const setTagsText=(v:string)=>{draft.current.tagsText=v;setTagsTextState(v)};

  const flush=useCallback(async()=>{
    const snapshot={...draft.current};
    const assetId=snapshot.assetId;
    if(assetId===null)return;
    window.clearTimeout(timer.current);
    saving.current=saving.current.then(async()=>{
      const base=baseline.current;
      if(base.assetId!==assetId)return;
      let latest:AssetRecord|null=null;
      const textChanged=snapshot.prompt!==base.prompt||snapshot.negative!==base.negative||snapshot.model!==base.model;
      const tags=parseTags(snapshot.tagsText),baseTags=parseTags(base.tagsText);
      if(textChanged){
        setStatus("正在保存提示词…");
        latest=await api.updatePrompt(assetId,{prompt:snapshot.prompt,negative_prompt:snapshot.negative,model:snapshot.model});
      }
      if(!sameTags(tags,baseTags)){
        latest=await api.replaceTags(assetId,tags);
        onTagsSaved();
      }
      if(latest){
        baseline.current={assetId:latest.id,prompt:latest.prompt,negative:latest.negative_prompt,model:latest.model,tagsText:latest.tags.join(", ")};
        if(draft.current.assetId===latest.id){
          onSaved(latest);
          if(draft.current.prompt===snapshot.prompt&&draft.current.negative===snapshot.negative&&draft.current.model===snapshot.model&&draft.current.tagsText===snapshot.tagsText){
            baseline.current={...draft.current};
          }
        }
        setStatus("已保存");
      }
    }).catch(e=>setStatus(`保存失败：${String(e)}`));
    await saving.current;
  },[onSaved,onTagsSaved,setStatus]);

  useEffect(()=>{
    if(!draft.current.assetId)return;
    const base=baseline.current,current=draft.current;
    const dirty=current.prompt!==base.prompt||current.negative!==base.negative||current.model!==base.model||!sameTags(parseTags(current.tagsText),parseTags(base.tagsText));
    if(!dirty)return;
    window.clearTimeout(timer.current);
    timer.current=window.setTimeout(()=>{void flush()},700);
    return()=>window.clearTimeout(timer.current);
  },[prompt,negative,model,tagsText,flush]);

  useEffect(()=>()=>{window.clearTimeout(timer.current);void flush()},[flush]);

  return{prompt,negative,model,tagsText,setPrompt,setNegative,setModel,setTagsText,flush,load:hydrate};
}
