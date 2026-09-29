import { useEffect,useRef,useState } from "react";
import type { AssetRecord } from "../types";

type PreviewMode="fit"|"actual";
const bytes=(n:number|null)=>!n?"":n<1024?`${n} B`:n<1024*1024?`${(n/1024).toFixed(1)} KB`:`${(n/1024/1024).toFixed(1)} MB`;
const metadataLabel=(value:string)=>({
  "manual":"手动",
  "a1111":"AUTOMATIC1111",
  "comfyui":"ComfyUI",
  "novelai":"NovelAI",
  "invokeai":"InvokeAI",
  "json":"通用 JSON",
  "none":"无生成元数据"
} as Record<string,string>)[value]||value;
const clampZoom=(value:number)=>Math.max(25,Math.min(400,Math.round(value)));

export function PreviewPane({asset,src,mode,onMode,onImport,onOpen,onFolder,onCopyImage,onCopyPath,onSaveAs,onFavorite,onFindSimilar}:{
  asset:AssetRecord|null;src:string;mode:PreviewMode;onMode:(m:PreviewMode)=>void;onImport:()=>void;
  onOpen:()=>void;onFolder:()=>void;onCopyImage:()=>void;onCopyPath:()=>void;onSaveAs:()=>void;onFavorite:()=>void;onFindSimilar:()=>void;
}){
  const[menu,setMenu]=useState<{x:number;y:number}|null>(null);
  const[zoom,setZoom]=useState(100);
  const[natural,setNatural]=useState({width:0,height:0});
  const[panning,setPanning]=useState(false);
  const stageRef=useRef<HTMLDivElement>(null);
  const panRef=useRef<{x:number;y:number;left:number;top:number}|null>(null);

  useEffect(()=>{
    if(!menu)return;
    const close=()=>setMenu(null);
    const key=(e:KeyboardEvent)=>{if(e.key==="Escape")close()};
    window.addEventListener("pointerdown",close);
    window.addEventListener("keydown",key);
    window.addEventListener("resize",close);
    return()=>{
      window.removeEventListener("pointerdown",close);
      window.removeEventListener("keydown",key);
      window.removeEventListener("resize",close);
    };
  },[menu]);

  useEffect(()=>{
    setMenu(null);setZoom(100);setNatural({width:0,height:0});setPanning(false);panRef.current=null;
    stageRef.current?.scrollTo({left:0,top:0});
  },[asset?.id]);

  useEffect(()=>{
    if(mode==="fit"){
      setZoom(100);setPanning(false);panRef.current=null;
      stageRef.current?.scrollTo({left:0,top:0});
    }
  },[mode]);

  const action=(fn:()=>void)=>()=>{fn();setMenu(null)};
  const setActual=(next:number)=>{
    const value=clampZoom(next);
    setZoom(value);
    if(mode!=="actual")onMode("actual");
  };
  const zoomBy=(delta:number)=>setActual(zoom+delta);
  const fit=()=>{setZoom(100);onMode("fit")};
  const actual=()=>{setZoom(100);onMode("actual")};
  const wheel=(e:React.WheelEvent<HTMLDivElement>)=>{
    if(!src)return;
    e.preventDefault();
    const direction=e.deltaY<0?1:-1;
    const step=zoom>=200?25:zoom>=100?10:5;
    setActual((mode==="fit"?100:zoom)+direction*step);
  };
  const doubleClick=()=>mode==="fit"?actual():fit();
  const pointerDown=(e:React.PointerEvent<HTMLDivElement>)=>{
    if(mode!=="actual"||e.button!==0)return;
    const stage=stageRef.current;if(!stage)return;
    e.currentTarget.setPointerCapture(e.pointerId);
    panRef.current={x:e.clientX,y:e.clientY,left:stage.scrollLeft,top:stage.scrollTop};
    setPanning(true);
  };
  const pointerMove=(e:React.PointerEvent<HTMLDivElement>)=>{
    const start=panRef.current,stage=stageRef.current;if(!start||!stage)return;
    stage.scrollLeft=start.left-(e.clientX-start.x);
    stage.scrollTop=start.top-(e.clientY-start.y);
  };
  const pointerUp=(e:React.PointerEvent<HTMLDivElement>)=>{
    if(!panRef.current)return;
    panRef.current=null;setPanning(false);
    try{e.currentTarget.releasePointerCapture(e.pointerId)}catch{}
  };

  const image=src?<img src={src} alt={asset?.name||"预览"} draggable={false}
    onLoad={e=>setNatural({width:e.currentTarget.naturalWidth,height:e.currentTarget.naturalHeight})}
    onDoubleClick={doubleClick}
    onContextMenu={e=>{e.preventDefault();e.stopPropagation();setMenu({x:Math.min(e.clientX,window.innerWidth-236),y:Math.min(e.clientY,window.innerHeight-310)})}}/>:null;

  return <section className="preview-pane panel glass-surface">
    <div className="preview-head">
      <div className="file-title"><span className={`status-dot ${asset?.missing?"bad":""}`}/><div><strong>{asset?.name||"未选择图片"}</strong><small>{asset?`${asset.format||"图片"}${asset.width&&asset.height?` · ${asset.width}×${asset.height}`:""}`:"请从左侧图库选择一条记录"}</small></div></div>
      <div className="preview-controls">
        <div className="zoom-controls" aria-label="预览缩放"><button onClick={()=>zoomBy(-10)} disabled={!src}>−</button><span>{mode==="fit"?"适应":`${zoom}%`}</span><button onClick={()=>zoomBy(10)} disabled={!src}>＋</button></div>
        <div className="segmented"><button className={mode==="fit"?"active":""} onClick={fit}>适应窗口</button><button className={mode==="actual"&&zoom===100?"active":""} onClick={actual}>100%</button></div>
      </div>
    </div>
    <div ref={stageRef} className={`image-stage ${mode} ${panning?"panning":""}`} onWheel={wheel} onPointerDown={pointerDown} onPointerMove={pointerMove} onPointerUp={pointerUp} onPointerCancel={pointerUp}>
      {src?(mode==="actual"?<div className="image-zoom-shell" style={{width:Math.max(1,natural.width*zoom/100),height:Math.max(1,natural.height*zoom/100)}}>{image}</div>:image):<div className="empty-state"><div className="empty-orb">✦</div><strong>从第一张图片开始建立你的生成记忆</strong><span>导入图片、记录提示词，ImageLore 会把与它有关的上下文保存下来。</span><button className="button primary" onClick={onImport}>＋ 导入图片</button></div>}
    </div>
    {menu&&asset?<div className="asset-context-menu" style={{left:menu.x,top:menu.y}} onPointerDown={e=>e.stopPropagation()}>
      <button onClick={action(onOpen)} disabled={!!asset.missing}>↗ 打开图片</button>
      <button onClick={action(onFolder)} disabled={!!asset.missing}>⌖ 打开文件所在位置</button>
      <hr/>
      <button onClick={action(onCopyImage)} disabled={!!asset.missing}>▣ 复制图像</button>
      <button onClick={action(onCopyPath)}>⧉ 复制文件路径</button>
      <button onClick={action(onSaveAs)} disabled={!!asset.missing}>⇩ 另存为…</button>
      <hr/>
      <button onClick={action(onFavorite)}>{asset.favorite?"★ 取消收藏":"☆ 收藏"}</button>
      <button onClick={action(onFindSimilar)}>◎ 查找相似图片</button>
    </div>:null}
    {asset?<div className="preview-foot"><span className="metadata-pill">{metadataLabel(asset.metadata_type||"manual")}</span><span>{bytes(asset.file_size)}</span><span>{asset.model||"未记录模型"}</span><span className="path" title={asset.path}>{asset.path}</span><button onClick={onOpen} disabled={!!asset.missing}>打开图片</button><button onClick={onFolder} disabled={!!asset.missing}>打开所在文件夹</button></div>:null}
  </section>
}
