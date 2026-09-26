import { useEffect, useState } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { isTauri } from "../api";

export function useNativeDrop(onDrop:(paths:string[])=>Promise<void>|void,setStatus:(text:string)=>void){
  const[active,setActive]=useState(false);
  const[count,setCount]=useState(0);

  useEffect(()=>{
    if(!isTauri)return;
    let disposed=false;
    let unlisten:(()=>void)|undefined;

    getCurrentWebview().onDragDropEvent(event=>{
      const payload=event.payload;
      if(payload.type==="enter"){
        setActive(true);setCount(payload.paths.length);
        setStatus(`检测到 ${payload.paths.length} 个拖入项目`);
      }else if(payload.type==="over"){
        setActive(true);
      }else if(payload.type==="leave"){
        setActive(false);setCount(0);setStatus("就绪");
      }else if(payload.type==="drop"){
        setActive(false);setCount(0);
        if(payload.paths.length)void onDrop(payload.paths);
      }
    }).then(fn=>{if(disposed)fn();else unlisten=fn}).catch(e=>setStatus(`拖拽监听启动失败：${String(e)}`));

    return()=>{disposed=true;unlisten?.()};
  },[onDrop,setStatus]);

  return{active,count};
}
