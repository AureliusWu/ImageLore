import { useCallback,useEffect,useRef,useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api,isTauri } from "../api";
import type { ImportProgress,ImportSummary } from "../types";

const empty:ImportProgress={job_id:0,processed:0,total:0,added:0,skipped:0,duplicates:0,failed:0,last_id:null,current_name:"",done:false,cancelled:false};

export function useImportJob(onDone:(summary:ImportSummary,cancelled:boolean)=>void,setStatus:(value:string)=>void){
  const[progress,setProgress]=useState<ImportProgress>(empty);
  const jobRef=useRef(0);
  useEffect(()=>{
    if(!isTauri)return;
    let disposed=false;let unlisten:(()=>void)|undefined;
    listen<ImportProgress>("imagelore://import-progress",event=>{
      const next=event.payload;
      if(next.job_id!==jobRef.current)return;
      setProgress(prev=>({...prev,...next,total:next.total||prev.total,processed:next.done?prev.processed:next.processed}));
      if(next.done){
        const summary:ImportSummary={added:next.added,skipped:next.skipped,duplicates:next.duplicates,failed:next.failed,last_id:next.last_id};
        jobRef.current=0;
        setStatus(next.cancelled?"导入已取消":"导入完成：新增 "+next.added+"，重复 "+next.duplicates+"，跳过 "+next.skipped+"，失败 "+next.failed);
        onDone(summary,next.cancelled);
      }else{
        setStatus("正在导入 "+next.processed+"/"+next.total+" · "+next.current_name);
      }
    }).then(fn=>{if(disposed)fn();else unlisten=fn});
    return()=>{disposed=true;unlisten?.()};
  },[onDone,setStatus]);
  const start=useCallback(async(label:string,starter:()=>Promise<number>)=>{
    if(jobRef.current)return;
    setStatus(label);setProgress(empty);
    const id=await starter();jobRef.current=id;setProgress(p=>({...p,job_id:id}));
  },[setStatus]);
  const cancel=useCallback(async()=>{if(jobRef.current)await api.cancelImport(jobRef.current)},[]);
  return{active:progress.job_id!==0&&!progress.done,progress,start,cancel};
}
