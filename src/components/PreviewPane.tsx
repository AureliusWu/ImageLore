import type { AssetRecord } from "../types";

type PreviewMode="fit"|"actual";
const bytes=(n:number|null)=>!n?"":n<1024?`${n} B`:n<1024*1024?`${(n/1024).toFixed(1)} KB`:`${(n/1024/1024).toFixed(1)} MB`;

export function PreviewPane({asset,src,mode,onMode,onImport,onOpen,onFolder}:{asset:AssetRecord|null;src:string;mode:PreviewMode;onMode:(m:PreviewMode)=>void;onImport:()=>void;onOpen:()=>void;onFolder:()=>void}){
  return <section className="preview-pane panel glass-surface">
    <div className="preview-head"><div className="file-title"><span className={`status-dot ${asset?.missing?"bad":""}`}/><div><strong>{asset?.name||"No image selected"}</strong><small>{asset?`${asset.format||"Image"}${asset.width&&asset.height?` · ${asset.width}×${asset.height}`:""}`:"Choose a record from the library"}</small></div></div><div className="segmented"><button className={mode==="fit"?"active":""} onClick={()=>onMode("fit")}>Fit</button><button className={mode==="actual"?"active":""} onClick={()=>onMode("actual")}>100%</button></div></div>
    <div className={`image-stage ${mode}`}>{src?<img src={src} alt={asset?.name||"Preview"}/>:<div className="empty-state"><div className="empty-orb">✦</div><strong>Your visual memory starts here</strong><span>Import an image, attach the prompt, then let ImageLore keep the context around it.</span><button className="button primary" onClick={onImport}>＋ Import images</button></div>}</div>
    {asset?<div className="preview-foot"><span className="metadata-pill">{asset.metadata_type||"manual"}</span><span>{bytes(asset.file_size)}</span><span>{asset.model||"No model"}</span><span className="path" title={asset.path}>{asset.path}</span><button onClick={onOpen}>Open image</button><button onClick={onFolder}>Show in folder</button></div>:null}
  </section>
}
