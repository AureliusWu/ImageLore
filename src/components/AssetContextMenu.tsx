import { useEffect } from "react";
import type { AssetSummary } from "../types";
import { contextMenuPosition } from "../previewWorkflow";

export type AssetMenuAction=(asset:AssetSummary)=>void;

export function AssetContextMenu({asset,x,y,onClose,onOpen,onFolder,onCopyImage,onCopyPath,onSaveAs,onFavorite,onFindSimilar}:{
  asset:AssetSummary;x:number;y:number;onClose:()=>void;
  onOpen?:AssetMenuAction;onFolder?:AssetMenuAction;onCopyImage?:AssetMenuAction;onCopyPath?:AssetMenuAction;
  onSaveAs?:AssetMenuAction;onFavorite?:AssetMenuAction;onFindSimilar?:AssetMenuAction;
}){
  useEffect(()=>{
    const close=()=>onClose();
    const key=(e:KeyboardEvent)=>{if(e.key==="Escape")close()};
    window.addEventListener("pointerdown",close);
    window.addEventListener("keydown",key);
    window.addEventListener("resize",close);
    return()=>{
      window.removeEventListener("pointerdown",close);
      window.removeEventListener("keydown",key);
      window.removeEventListener("resize",close);
    };
  },[onClose]);

  const run=(fn?:AssetMenuAction)=>()=>{fn?.(asset);onClose()};
  const fileMissing=!!asset.missing;
  const position=contextMenuPosition(x,y,window.innerWidth,window.innerHeight);
  return <div className="asset-context-menu" style={position} onPointerDown={e=>e.stopPropagation()}>
    {onOpen?<button onClick={run(onOpen)} disabled={fileMissing}>↗ 打开图片</button>:null}
    {onFolder?<button onClick={run(onFolder)} disabled={fileMissing}>⌖ 打开文件所在位置</button>:null}
    {onOpen||onFolder?<hr/>:null}
    {onCopyImage?<button onClick={run(onCopyImage)} disabled={fileMissing}>▣ 复制图像</button>:null}
    {onCopyPath?<button onClick={run(onCopyPath)}>⧉ 复制文件路径</button>:null}
    {onSaveAs?<button onClick={run(onSaveAs)} disabled={fileMissing}>⇩ 另存为…</button>:null}
    {onFavorite||onFindSimilar?<hr/>:null}
    {onFavorite?<button onClick={run(onFavorite)}>{asset.favorite?"★ 取消收藏":"☆ 收藏"}</button>:null}
    {onFindSimilar?<button onClick={run(onFindSimilar)}>◎ 查找相似图片</button>:null}
  </div>
}
