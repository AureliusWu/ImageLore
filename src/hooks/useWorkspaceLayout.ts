import { useCallback, useEffect, useState } from "react";

export type PreviewMode="fit"|"actual";
type Side="left"|"right";

const storedNumber=(key:string,fallback:number,min:number,max:number)=>{
  const raw=localStorage.getItem(key);
  const value=raw===null?fallback:Number(raw);
  return Number.isFinite(value)?Math.min(max,Math.max(min,value)):fallback;
};
const storedPreview=():PreviewMode=>localStorage.getItem("imagelore.preview")==="actual"?"actual":"fit";

export function useWorkspaceLayout(){
  const[previewMode,setPreviewMode]=useState<PreviewMode>(storedPreview);
  const[leftWidth,setLeftWidth]=useState(()=>storedNumber("imagelore.left",348,280,590));
  const[rightWidth,setRightWidth]=useState(()=>storedNumber("imagelore.right",510,410,780));

  useEffect(()=>localStorage.setItem("imagelore.preview",previewMode),[previewMode]);
  useEffect(()=>localStorage.setItem("imagelore.left",String(leftWidth)),[leftWidth]);
  useEffect(()=>localStorage.setItem("imagelore.right",String(rightWidth)),[rightWidth]);

  const drag=useCallback((side:Side)=>(e:React.PointerEvent)=>{
    e.currentTarget.setPointerCapture(e.pointerId);
    const start=e.clientX;
    const initial=side==="left"?leftWidth:rightWidth;
    const move=(ev:PointerEvent)=>{
      const delta=ev.clientX-start;
      if(side==="left")setLeftWidth(Math.min(590,Math.max(280,initial+delta)));
      else setRightWidth(Math.min(780,Math.max(410,initial-delta)));
    };
    const up=()=>{window.removeEventListener("pointermove",move);window.removeEventListener("pointerup",up)};
    window.addEventListener("pointermove",move);window.addEventListener("pointerup",up);
  },[leftWidth,rightWidth]);

  return{previewMode,setPreviewMode,leftWidth,rightWidth,drag};
}
