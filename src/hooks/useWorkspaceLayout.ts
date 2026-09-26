import { useCallback, useEffect, useState } from "react";

export type PreviewMode="fit"|"actual";
type Side="left"|"right";

export function useWorkspaceLayout(){
  const[previewMode,setPreviewMode]=useState<PreviewMode>(()=>(localStorage.getItem("imagelore.preview")||"fit") as PreviewMode);
  const[leftWidth,setLeftWidth]=useState(()=>Number(localStorage.getItem("imagelore.left")||348));
  const[rightWidth,setRightWidth]=useState(()=>Number(localStorage.getItem("imagelore.right")||510));

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
