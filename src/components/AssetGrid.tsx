import { useEffect, useRef, useState } from "react";
import { api } from "../api";
import type { AssetRecord } from "../types";

function LazyThumb({asset,selected,onClick}:{asset:AssetRecord;selected:boolean;onClick:(e:React.MouseEvent)=>void}){
  const ref=useRef<HTMLButtonElement>(null);const[src,setSrc]=useState("");
  useEffect(()=>{const el=ref.current;if(!el)return;const obs=new IntersectionObserver(entries=>{if(entries.some(x=>x.isIntersecting)){api.preview(asset.id,420,true).then(setSrc).catch(()=>setSrc(""));obs.disconnect()}},{rootMargin:"300px"});obs.observe(el);return()=>obs.disconnect()},[asset.id,asset.file_mtime,asset.fingerprint]);
  return <button ref={ref} className={`asset-card ${selected?"selected":""}`} onClick={onClick} title={asset.path}>
    <div className="asset-thumb">{src?<img src={src} alt=""/>:<div className="thumb-skeleton"/>}{asset.favorite?<span className="asset-star">★</span>:null}{asset.missing?<span className="asset-missing">文件缺失</span>:null}</div>
    <div className="asset-copy"><strong>{asset.name}</strong><span>{asset.width&&asset.height?`${asset.width}×${asset.height}`:asset.format||"Image"}</span></div>
  </button>
}

export function AssetGrid({assets,total,currentId,selected,loading,onAsset,onLoadMore}:{assets:AssetRecord[];total:number;currentId?:number;selected:Set<number>;loading:boolean;onAsset:(asset:AssetRecord,e:React.MouseEvent)=>void;onLoadMore:()=>void}){
  const ref=useRef<HTMLDivElement>(null);const[size,setSize]=useState({w:320,h:600});const[scrollTop,setScrollTop]=useState(0);
  useEffect(()=>{const el=ref.current;if(!el)return;const ro=new ResizeObserver(([entry])=>setSize({w:entry.contentRect.width,h:entry.contentRect.height}));ro.observe(el);return()=>ro.disconnect()},[]);
  const pad=10,gap=9,min=128,rowH=151;const cols=Math.max(1,Math.floor((size.w-pad*2+gap)/(min+gap)));const itemW=Math.max(100,(size.w-pad*2-gap*(cols-1))/cols);const rows=Math.ceil(assets.length/cols);const first=Math.max(0,Math.floor(scrollTop/rowH)-2);const last=Math.min(rows,Math.ceil((scrollTop+size.h)/rowH)+3);const visible:number[]=[];
  for(let r=first;r<last;r++)for(let c=0;c<cols;c++){const i=r*cols+c;if(i<assets.length)visible.push(i)}
  const innerH=Math.max(size.h-1,rows*rowH+(assets.length<total?42:8));
  const handleScroll=(e:React.UIEvent<HTMLDivElement>)=>{const el=e.currentTarget;setScrollTop(el.scrollTop);if(!loading&&assets.length<total&&el.scrollTop+el.clientHeight>el.scrollHeight-750)onLoadMore()};
  return <div className="asset-grid" ref={ref} onScroll={handleScroll}><div className="asset-grid-inner" style={{height:innerH}}>
    {visible.map(i=>{const a=assets[i],r=Math.floor(i/cols),c=i%cols;return <div className="asset-grid-item" key={a.id} style={{left:pad+c*(itemW+gap),top:r*rowH,width:itemW}}><LazyThumb asset={a} selected={selected.has(a.id)||currentId===a.id} onClick={e=>onAsset(a,e)}/></div>})}
    {assets.length<total?<div className="load-marker" style={{top:rows*rowH}}>{loading?"加载中…":`${assets.length} / ${total}`}</div>:null}
  </div></div>
}
