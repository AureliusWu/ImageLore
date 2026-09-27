import { useCallback,useEffect,useRef,useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api,isTauri } from "../api";
import type { ImportProgress,ImportSummary } from "../types";

const empty:ImportProgress={job_id:0,processed:0,total:0,added:0,skipped:0,duplicates:0,failed:0,last_id:null,current_name:"",done:false,cancelled:false};

export function useImportJob(onDone:(summary:ImportSummary,cancelled:boolean)=>void,setStatus:(value:string)=>void){
  const[progress,setProgress]=useState<ImportProgress>(empty);
  const[starting,setStarting]=useState(false);
  const jobRef=useRef(0);
  const startingRef=useRef(false);
  const bufferedRef=useRef<ImportProgress|null>(null);
  const onDoneRef=useRef(onDone);
  const statusRef=useRef(setStatus);

  useEffect(()=>{onDoneRef.current=onDone},[onDone]);
  useEffect(()=>{statusRef.current=setStatus},[setStatus]);

  const applyProgress=useCallback((next:ImportProgress)=>{
    setProgress(prev=>({...prev,...next,total:next.total||prev.total,processed:next.done?prev.processed:next.processed}));
    if(next.done){
      const summary:ImportSummary={added:next.added,skipped:next.skipped,duplicates:next.duplicates,failed:next.failed,last_id:next.last_id};
      jobRef.current=0;
      setStarting(false);
      statusRef.current(next.cancelled?"导入已取消":"导入完成：新增 "+next.added+"，重复 "+next.duplicates+"，跳过 "+next.skipped+"，失败 "+next.failed);
      void onDoneRef.current(summary,next.cancelled);
    }else{
      statusRef.current("正在导入 "+next.processed+"/"+next.total+" · "+next.current_name);
    }
  },[]);

  useEffect(()=>{
    if(!isTauri)return;
    let disposed=false;
    let unlisten:(()=>void)|undefined;
    listen<ImportProgress>("imagelore://import-progress",event=>{
      const next=event.payload;
      if(jobRef.current===0){
        if(startingRef.current)bufferedRef.current=next;
        return;
      }
      if(next.job_id===jobRef.current)applyProgress(next);
    }).then(fn=>{if(disposed)fn();else unlisten=fn})
      .catch(e=>statusRef.current("导入进度监听启动失败："+String(e)));
    return()=>{disposed=true;unlisten?.()};
  },[applyProgress]);

  const takeBuffered=useCallback(()=>{const value=bufferedRef.current;bufferedRef.current=null;return value},[]);

  const start=useCallback(async(label:string,starter:()=>Promise<number>)=>{
    if(jobRef.current||startingRef.current)return;
    startingRef.current=true;
    setStarting(true);
    bufferedRef.current=null;
    setProgress(empty);
    statusRef.current(label);
    try{
      const id=await starter();
      jobRef.current=id;
      setProgress(p=>({...p,job_id:id}));
      const buffered=takeBuffered();
      if(buffered&&buffered.job_id===id)applyProgress(buffered);
    }catch(e){
      jobRef.current=0;
      setProgress(empty);
      statusRef.current("导入启动失败："+String(e));
    }finally{
      startingRef.current=false;
      setStarting(false);
    }
  },[applyProgress,takeBuffered]);

  const cancel=useCallback(async()=>{
    const id=jobRef.current;
    if(!id)return;
    statusRef.current("正在取消导入…");
    try{await api.cancelImport(id)}catch(e){statusRef.current("取消导入失败："+String(e))}
  },[]);

  return{active:starting||(progress.job_id!==0&&!progress.done),progress,start,cancel};
}
